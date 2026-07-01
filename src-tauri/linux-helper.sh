#!/usr/bin/env python3
from __future__ import annotations
import base64
import json
import os
import pathlib
import re
import shutil
import stat
import subprocess
import sys
import time
from typing import Any

CMD = sys.argv[1] if len(sys.argv) > 1 else ""
RAW_PAYLOAD = ""
if len(sys.argv) > 2 and sys.argv[2] == "-":
    RAW_PAYLOAD = sys.stdin.read().strip()
try:
    PAYLOAD = json.loads(base64.b64decode(RAW_PAYLOAD).decode("utf-8")) if RAW_PAYLOAD else {}
except Exception:
    PAYLOAD = {}

DATA_DIR = pathlib.Path(os.environ.get("PZMM_DATA_DIR", "/var/lib/pzmm"))
SERVER_DIR = pathlib.Path(os.environ.get("PZMM_SERVER_DIR", str(DATA_DIR / "Zomboid" / "Server")))
ZOMBOID_DIR = pathlib.Path(os.environ.get("PZMM_ZOMBOID_DIR", str(DATA_DIR / "zomboid-server")))
STEAMCMD_DIR = pathlib.Path(os.environ.get("PZMM_STEAMCMD_DIR", str(DATA_DIR / "steamcmd")))
UNIT_PREFIX = os.environ.get("PZMM_UNIT_PREFIX", "pzmm-zomboid")
SERVICE_USER = os.environ.get("PZMM_SERVICE_USER", "pzmm")


def emit(value: Any) -> None:
    print(json.dumps(value, separators=(",", ":")), flush=True)


def run(args: list[str], input_text: str | None = None, check: bool = False) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(args, input=input_text, text=True, capture_output=True)
    if check and result.returncode != 0:
        raise RuntimeError((result.stdout + "\n" + result.stderr).strip() or f"Command failed: {' '.join(args)}")
    return result


def sudo(args: list[str], input_text: str | None = None, check: bool = True) -> subprocess.CompletedProcess[str]:
    return run(["sudo", "-n", *args], input_text=input_text, check=check)


def safe_server_id(server_id: str) -> str:
    server_id = (server_id or "").strip()
    if not re.fullmatch(r"[A-Za-z0-9_.-]+", server_id):
        emit({"success": False, "message": "Invalid server id. Use letters, numbers, dot, underscore or dash.", "command": "validate-server-id", "logs": []})
        raise SystemExit(2)
    return server_id


def payload_server_id() -> str:
    return safe_server_id(str(PAYLOAD.get("serverId", "")))


def service_name(server_id: str) -> str:
    return f"{UNIT_PREFIX}@{server_id}.service"


def socket_name(server_id: str) -> str:
    return f"{UNIT_PREFIX}@{server_id}.socket"


def fifo_path(server_id: str) -> pathlib.Path:
    return pathlib.Path(f"/run/{UNIT_PREFIX}-{server_id}.control")


def server_ini(server_id: str) -> pathlib.Path:
    return SERVER_DIR / f"{server_id}.ini"


def ensure_dir(path: pathlib.Path) -> None:
    sudo(["install", "-d", "-o", SERVICE_USER, "-g", SERVICE_USER, str(path)])


def write_text_sudo(path: pathlib.Path, content: str) -> None:
    sudo(["install", "-d", "-o", SERVICE_USER, "-g", SERVICE_USER, str(path.parent)])
    sudo(["tee", str(path)], input_text=content)
    sudo(["chown", f"{SERVICE_USER}:{SERVICE_USER}", str(path)])


def remove_sudo(path: pathlib.Path) -> None:
    sudo(["rm", "-rf", str(path)], check=False)


def read_ini(path: pathlib.Path) -> dict[str, str]:
    values: dict[str, str] = {}
    if not path.exists():
        return values
    for line in path.read_text(errors="replace").splitlines():
        if "=" in line and not line.lstrip().startswith("#"):
            key, value = line.split("=", 1)
            values[key.strip()] = value.strip()
    return values


def write_ini(path: pathlib.Path, values: dict[str, str]) -> None:
    text = "\n".join(f"{key}={value}" for key, value in values.items()) + "\n"
    write_text_sudo(path, text)


def split_items(value: str) -> list[str]:
    return [item.strip() for item in value.replace(",", ";").split(";") if item.strip()]


def server_json(server_id: str) -> dict[str, Any]:
    values = read_ini(server_ini(server_id))
    active = run(["systemctl", "is-active", service_name(server_id)]).stdout.strip()
    return {
        "id": server_id,
        "name": values.get("PublicName") or server_id,
        "path": server_id,
        "fileName": f"{server_id}.ini",
        "mods": split_items(values.get("Mods", "")),
        "workshopItems": split_items(values.get("WorkshopItems", "")),
        "gameBuild": "stable",
        "port": int(values.get("DefaultPort", "16261") or 16261),
        "status": "running" if active == "active" else "stopped",
    }


def install_units() -> None:
    service = f"""[Unit]
Description=PZMM Project Zomboid Server %i
After=network-online.target
Wants=network-online.target
Requires={UNIT_PREFIX}@%i.socket

[Service]
Type=simple
User={SERVICE_USER}
Group={SERVICE_USER}
WorkingDirectory={ZOMBOID_DIR}
Environment=HOME={DATA_DIR}
Environment=ZOMBOID_HOME={DATA_DIR}
Sockets={UNIT_PREFIX}@%i.socket
StandardInput=socket
StandardOutput=journal
StandardError=journal
ExecStart=/bin/bash ./start-server.sh -servername %i
ExecStop=/bin/bash -lc 'fifo=/run/{UNIT_PREFIX}-%i.control; if [ -p "$fifo" ]; then printf "save\\n" > "$fifo"; sleep 5; printf "quit\\n" > "$fifo"; fi'
Restart=no
TimeoutStopSec=90

[Install]
WantedBy=multi-user.target
"""
    socket = f"""[Unit]
Description=PZMM Project Zomboid Console FIFO %i

[Socket]
ListenFIFO=/run/{UNIT_PREFIX}-%i.control
SocketUser={SERVICE_USER}
SocketGroup={SERVICE_USER}
SocketMode=0660
RemoveOnStop=true

[Install]
WantedBy=sockets.target
"""
    sudo(["tee", f"/etc/systemd/system/{UNIT_PREFIX}@.service"], input_text=service)
    sudo(["tee", f"/etc/systemd/system/{UNIT_PREFIX}@.socket"], input_text=socket)
    sudo(["systemctl", "daemon-reload"])


def list_mods() -> list[dict[str, Any]]:
    roots = [DATA_DIR / "Zomboid" / "mods", STEAMCMD_DIR / "steamapps" / "workshop" / "content" / "108600", ZOMBOID_DIR / "steamapps" / "workshop" / "content" / "108600"]
    mods: list[dict[str, Any]] = []
    seen: set[str] = set()
    for root in roots:
        if not root.exists():
            continue
        for info in root.rglob("mod.info"):
            folder = info.parent
            data: dict[str, str] = {}
            for line in info.read_text(errors="replace").splitlines():
                if "=" in line:
                    key, value = line.split("=", 1)
                    data[key.strip()] = value.strip()
            mod_id = data.get("id") or folder.name
            if mod_id in seen:
                continue
            seen.add(mod_id)
            workshop_id = ""
            parts = list(folder.parts)
            if "108600" in parts:
                index = parts.index("108600")
                if index + 1 < len(parts):
                    workshop_id = parts[index + 1]
            poster = data.get("poster", "")
            mods.append({
                "id": mod_id,
                "name": data.get("name", mod_id),
                "description": data.get("description", ""),
                "source": "steamcmd" if workshop_id else "local",
                "packagePath": str(folder),
                "imageUrl": str(folder / poster) if poster else "",
                "workshopId": workshop_id,
                "variants": [{"id": mod_id, "name": data.get("name", mod_id), "path": str(folder)}],
            })
    return mods


def stream_server(server_id: str, test_mode: bool) -> None:
    start_time = int(time.time())
    emit({"event": "line", "line": f"Starting systemd socket {socket_name(server_id)}"})
    sudo(["systemctl", "start", socket_name(server_id)])
    emit({"event": "line", "line": f"Starting systemd service {service_name(server_id)}"})
    sudo(["systemctl", "start", service_name(server_id)])
    emit({"event": "line", "line": f"Streaming journalctl for {service_name(server_id)}"})
    proc = subprocess.Popen(["sudo", "-n", "journalctl", "-u", service_name(server_id), "-f", "-n", "0", "--output=cat"], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    deadline = time.time() + 600
    ready_pattern = re.compile(r"\*\*\* SERVER STARTED \*\*\*|Server is listening on port|Startup version", re.I)
    try:
        while True:
            if proc.stdout is None:
                break
            line = proc.stdout.readline()
            if line:
                line = line.rstrip("\n")
                emit({"event": "line", "line": line})
                if ready_pattern.search(line):
                    duration = int(time.time()) - start_time
                    emit({"event": "finished", "result": {"status": "success", "summary": "Remote Project Zomboid server started.", "durationSeconds": duration, "batPath": str(ZOMBOID_DIR / "start-server.sh"), "command": f"systemctl start {socket_name(server_id)}", "warningCount": 0, "criticalCount": 0, "logLines": []}})
                    return
            elif proc.poll() is not None:
                emit({"event": "line", "line": "journalctl exited while watching startup."})
                return
            if time.time() > deadline:
                emit({"event": "line", "line": "Startup watch finished; service is still running. Use journalctl for continued logs."})
                return
    finally:
        if proc.poll() is None:
            proc.terminate()


def send_fifo(server_id: str, command_text: str) -> dict[str, Any]:
    fifo = fifo_path(server_id)
    try:
        mode = os.stat(fifo).st_mode
        is_fifo = stat.S_ISFIFO(mode)
    except FileNotFoundError:
        is_fifo = False
    if not is_fifo:
        return {"success": False, "message": "Server command FIFO is not available. Start the systemd socket/service first.", "command": f"test -p {fifo}", "logs": [f"Missing FIFO: {fifo}"]}
    if command_text in {"quit", "stop"}:
        sudo(["tee", str(fifo)], input_text="save\n")
        time.sleep(5)
        sudo(["tee", str(fifo)], input_text="quit\n")
        return {"success": True, "message": "Stop command sent through the FIFO.", "command": f"save; quit > {fifo}", "logs": ["save sent", "quit sent"]}
    sudo(["tee", str(fifo)], input_text=command_text + "\n")
    return {"success": True, "message": "Command sent through the server FIFO.", "command": f"echo command > {fifo}", "logs": [command_text]}


def main() -> None:
    if CMD == "--version":
        emit({"name": "pzmm-helper", "platform": "linux", "runtime": "python", "version": "0.4.0"})
    elif CMD == "get-system-ram":
        mem_total_kb = 0
        for line in pathlib.Path("/proc/meminfo").read_text().splitlines():
            if line.startswith("MemTotal:"):
                mem_total_kb = int(line.split()[1])
                break
        print(round(mem_total_kb / 1024 / 1024), flush=True)
    elif CMD == "get-path-status":
        emit([{"path": path, "exists": pathlib.Path(path).exists()} for path in PAYLOAD.get("paths", [])])
    elif CMD in {"clear-mods-cache", "clear-server-cache"}:
        cache = DATA_DIR / "cache"
        if cache.exists():
            for item in cache.glob("*.json"):
                remove_sudo(item)
        emit({"ok": True})
    elif CMD == "list-servers":
        ensure_dir(SERVER_DIR)
        emit([server_json(path.stem) for path in sorted(SERVER_DIR.glob("*.ini")) if re.fullmatch(r"[A-Za-z0-9_.-]+", path.stem)])
    elif CMD == "create-server":
        name = str(PAYLOAD.get("name", "")).strip()
        if not name:
            emit({"success": False, "message": "Server name is required", "command": "create-server", "logs": []})
            raise SystemExit(2)
        server_id = re.sub(r"[^A-Za-z0-9_.-]", "", name.replace(" ", "_")) or "server"
        path = server_ini(server_id)
        if not path.exists():
            write_ini(path, {"PublicName": name, "DefaultPort": "16261", "Mods": "", "WorkshopItems": "", "Map": "Muldraugh, KY", "MaxPlayers": "16", "PauseEmpty": "true"})
        emit(server_json(server_id))
    elif CMD == "delete-server":
        server_id = payload_server_id()
        for suffix in [".ini", ".lua", "_SandboxVars.lua", "_spawnregions.lua", "_spawnpoints.lua"]:
            remove_sudo(SERVER_DIR / f"{server_id}{suffix}")
        emit({"serverId": server_id, "deleted": True})
    elif CMD == "get-server-settings":
        server_id = payload_server_id()
        emit({"serverId": server_id, "settings": read_ini(server_ini(server_id))})
    elif CMD == "update-server-settings":
        server_id = payload_server_id()
        values = read_ini(server_ini(server_id))
        settings = PAYLOAD.get("settings", {})
        if isinstance(settings, dict):
            source = settings.get("settings", settings)
            if isinstance(source, dict):
                values.update({str(k): str(v) for k, v in source.items()})
        write_ini(server_ini(server_id), values)
        emit(server_json(server_id))
    elif CMD == "get-server-lua-settings" or CMD == "update-server-lua-settings":
        emit({"serverId": payload_server_id(), "settings": []})
    elif CMD == "update-server-mods":
        server_id = payload_server_id()
        values = read_ini(server_ini(server_id))
        values["Mods"] = ";".join(PAYLOAD.get("modIds", []))
        values["WorkshopItems"] = ";".join(PAYLOAD.get("workshopIds", []))
        write_ini(server_ini(server_id), values)
        emit({"ok": True})
    elif CMD == "update-server-build":
        emit({"ok": True})
    elif CMD == "install-mod":
        source = pathlib.Path(str(PAYLOAD.get("packagePath", "")))
        mod_id = str(PAYLOAD.get("modId", ""))
        workshop_id = str(PAYLOAD.get("workshopId", ""))
        target_root = DATA_DIR / "Zomboid" / "mods"
        target = target_root / (source.name if source.name else mod_id)
        copied = False
        if source.exists():
            remove_sudo(target)
            sudo(["install", "-d", "-o", SERVICE_USER, "-g", SERVICE_USER, str(target_root)])
            sudo(["cp", "-a", str(source), str(target_root)])
            sudo(["chown", "-R", f"{SERVICE_USER}:{SERVICE_USER}", str(target)])
            copied = True
        emit({"modId": mod_id, "workshopId": workshop_id, "targetPath": str(target), "wasCopied": copied})
    elif CMD == "install-server-map":
        emit({"ok": True})
    elif CMD == "list-mods":
        emit(list_mods())
    elif CMD == "check-server-firewall":
        server_id = payload_server_id()
        svc = pathlib.Path(f"/etc/systemd/system/{UNIT_PREFIX}@.service").exists()
        sock = pathlib.Path(f"/etc/systemd/system/{UNIT_PREFIX}@.socket").exists()
        launcher = (ZOMBOID_DIR / "start-server.sh").exists()
        configured = svc and sock and launcher
        logs = [f"Checking Linux systemd setup for {server_id}.", "systemd service template is installed." if svc else "systemd service template is missing.", "systemd FIFO socket template is installed." if sock else "systemd FIFO socket template is missing.", "Project Zomboid Linux launcher was found." if launcher else "Project Zomboid Linux launcher was not found yet."]
        emit({"serverId": server_id, "ports": [16261, 16262], "rules": [{"protocol": "systemd", "port": 0, "allowed": configured}], "missingRules": [] if configured else [{"protocol": "systemd", "port": 0, "allowed": False}], "isConfigured": configured, "logs": logs})
    elif CMD == "configure-server-firewall":
        payload_server_id()
        install_units()
        for path in [DATA_DIR, SERVER_DIR, ZOMBOID_DIR, STEAMCMD_DIR, DATA_DIR / "cache"]:
            ensure_dir(path)
        emit({"success": True, "message": "systemd templates are configured. You can start the server through the FIFO socket.", "command": "systemctl daemon-reload", "logs": [f"Installed {UNIT_PREFIX}@.service.", f"Installed {UNIT_PREFIX}@.socket."]})
    elif CMD in {"start-server-streaming", "test-server"}:
        stream_server(payload_server_id(), CMD == "test-server")
    elif CMD == "server-status":
        server_id = payload_server_id()
        active = run(["systemctl", "is-active", service_name(server_id)]).stdout.strip()
        sub = run(["systemctl", "show", service_name(server_id), "-p", "SubState", "--value"]).stdout.strip()
        success = active == "active"
        emit({"success": success, "message": "Remote server is running." if success else "Remote server is not running.", "command": f"systemctl is-active {service_name(server_id)}", "logs": [f"ActiveState: {active}", f"SubState: {sub}"]})
    elif CMD in {"send-server-command", "cancel-server-test"}:
        command_text = str(PAYLOAD.get("command", "quit")).strip() or "quit"
        emit(send_fifo(payload_server_id(), command_text))
    else:
        emit({"success": False, "message": f"Unknown helper command: {CMD}", "command": CMD, "logs": []})
        raise SystemExit(1)


try:
    main()
except Exception as error:
    emit({"success": False, "message": str(error), "command": CMD, "logs": []})
    raise SystemExit(1)
