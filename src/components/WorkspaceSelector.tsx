import { ArrowLeft, CheckCircle2, FileKey2, Folder, KeyRound, Lock, MonitorCog, Network, Server, ShieldAlert, Wifi } from "lucide-react"
import { useEffect, useMemo, useState } from "react"

import type { RemoteConnectionDraft, RemoteWorkspaceConfig } from "@/lib/commandRunner"
import { getErrorMessage } from "@/lib/errors"
import { invokeTauri } from "@/lib/tauri"

type WorkspaceSelectorProps = {
  onSelectLocal: () => void
  onSelectRemote: (connection: RemoteConnectionDraft) => void
}

type RemoteServerConnectionResult = {
  name: string
  host: string
  port: number
  serverPath: string
  message: string
}

const initialRemoteConnection: RemoteConnectionDraft = {
  name: "",
  host: "",
  port: "22",
  username: "",
  authMethod: "password",
  password: "",
  sshKeyPath: "",
  serverPath: "C:\\Users\\Administrator\\Zomboid\\Server",
}

function remoteAppDataBase(username: string) {
  const account = username.trim() || "Administrator"
  return `C:\\Users\\${account}\\AppData\\Local\\ZomboidServerModManager`
}

function isLegacyPzManagerPath(path?: string) {
  return Boolean(path?.trim().replace(/\//g, "\\").toLowerCase().startsWith("c:\\pzmanager\\"))
}

function cleanLegacyPath(path?: string) {
  return path && !isLegacyPzManagerPath(path) ? path : ""
}

export function WorkspaceSelector({ onSelectLocal, onSelectRemote }: WorkspaceSelectorProps) {
  const [mode, setMode] = useState<"choose" | "remote">("choose")

  if (mode === "remote") {
    return <RemoteWorkspaceSetup onBack={() => setMode("choose")} onConnected={onSelectRemote} />
  }

  return (
    <main className="flex min-h-screen bg-[#22272b] text-white">
      <section className="flex w-full flex-col justify-center px-6 py-10 sm:px-10 lg:px-16">
        <div className="mx-auto flex w-full max-w-5xl flex-col gap-10">
          <div className="max-w-2xl">
            <div className="mb-5 flex h-14 w-14 items-center justify-center rounded-2xl border border-orange-400/20 bg-orange-500/10 text-orange-300 shadow-[0_0_24px_rgba(249,115,22,0.12)]">
              <Server size={28} />
            </div>
            <p className="text-xs font-black uppercase tracking-[0.24em] text-orange-300">PZ Manager 0.4.0</p>
            <h1 className="mt-3 text-4xl font-black tracking-tight text-white sm:text-5xl">Choose your workspace</h1>
            <p className="mt-4 max-w-xl text-base leading-7 text-gray-400">
              Work with server profiles on this PC, or start a remote Windows server connection for hosted Project Zomboid setups.
            </p>
          </div>

          <div className="grid gap-5 lg:grid-cols-2">
            <button
              type="button"
              onClick={onSelectLocal}
              className="group min-h-[260px] rounded-[8px] border border-white/10 bg-[#2b3238] p-7 text-left transition-all hover:border-orange-400/40 hover:bg-[#333b42] hover:shadow-[0_0_24px_rgba(249,115,22,0.08)]"
            >
              <div className="flex items-start justify-between gap-6">
                <div className="flex h-12 w-12 items-center justify-center rounded-[8px] bg-[#1e2327] text-orange-300 transition-colors group-hover:bg-orange-500 group-hover:text-white">
                  <Folder size={24} />
                </div>
                <span className="rounded-full border border-green-400/20 bg-green-500/10 px-3 py-1 text-[10px] font-black uppercase tracking-widest text-green-300">
                  Ready
                </span>
              </div>
              <h2 className="mt-8 text-2xl font-black tracking-tight">Local workspace</h2>
              <p className="mt-3 text-sm leading-6 text-gray-400">
                Open the app normally and manage the Project Zomboid server files, mods, downloads, and tests from this Windows user profile.
              </p>
              <div className="mt-7 flex items-center gap-2 text-sm font-bold text-orange-300">
                <CheckCircle2 size={17} />
                Uses the current app flow
              </div>
            </button>

            <button
              type="button"
              onClick={() => setMode("remote")}
              className="group min-h-[260px] rounded-[8px] border border-white/10 bg-[#2b3238] p-7 text-left transition-all hover:border-cyan-300/40 hover:bg-[#303941] hover:shadow-[0_0_24px_rgba(34,211,238,0.08)]"
            >
              <div className="flex items-start justify-between gap-6">
                <div className="flex h-12 w-12 items-center justify-center rounded-[8px] bg-[#1e2327] text-cyan-300 transition-colors group-hover:bg-cyan-500 group-hover:text-white">
                  <Network size={24} />
                </div>
                <span className="rounded-full border border-cyan-300/20 bg-cyan-500/10 px-3 py-1 text-[10px] font-black uppercase tracking-widest text-cyan-200">
                  Windows
                </span>
              </div>
              <h2 className="mt-8 text-2xl font-black tracking-tight">Remote workspace</h2>
              <p className="mt-3 text-sm leading-6 text-gray-400">
              Prepare an SSH connection to a remote Windows host so PZ Manager can manage server profiles outside this machine.
              </p>
              <div className="mt-7 flex items-center gap-2 text-sm font-bold text-cyan-200">
                <Wifi size={17} />
                Configure connection
              </div>
            </button>
          </div>
        </div>
      </section>
    </main>
  )
}

function RemoteWorkspaceSetup({
  onBack,
  onConnected,
}: {
  onBack: () => void
  onConnected: (connection: RemoteConnectionDraft) => void
}) {
  const [connection, setConnection] = useState(initialRemoteConnection)
  const [status, setStatus] = useState<"idle" | "connecting" | "connected" | "error">("idle")
  const [feedback, setFeedback] = useState<string | null>(null)
  const isWindows = useMemo(() => navigator.platform.toLowerCase().includes("win"), [])
  const hasAuthentication =
    connection.authMethod === "password"
      ? connection.password.trim().length > 0
      : connection.sshKeyPath.trim().length > 0
  const canConnect =
    isWindows &&
    connection.name.trim().length > 0 &&
    connection.host.trim().length > 0 &&
    connection.port.trim().length > 0 &&
    connection.username.trim().length > 0 &&
    connection.serverPath.trim().length > 0 &&
    hasAuthentication

  useEffect(() => {
    let isMounted = true

    void invokeTauri<RemoteWorkspaceConfig | null>("get_remote_workspace_config")
      .then((config) => {
        if (!isMounted || !config) return

        setConnection({
          name: config.name,
          host: config.host,
          port: config.port || "22",
          username: config.username,
          authMethod: config.authMethod === "password" ? "password" : "key",
          password: "",
          sshKeyPath: config.sshKeyPath,
          serverPath: config.serverPath,
        })
      })
      .catch((error) => {
        if (!isMounted) return
        setStatus("error")
        setFeedback(getErrorMessage(error))
      })

    return () => {
      isMounted = false
    }
  }, [])

  function updateField<K extends keyof RemoteConnectionDraft>(field: K, value: RemoteConnectionDraft[K]) {
    setConnection((current) => ({ ...current, [field]: value }))
    setStatus("idle")
    setFeedback(null)
  }

  async function selectSshKeyFile() {
    setFeedback(null)

    try {
      const selectedPath = await invokeTauri<string | null>("select_ssh_key_file")

      if (selectedPath) {
        updateField("sshKeyPath", selectedPath)
      }
    } catch (error) {
      setStatus("error")
      setFeedback(getErrorMessage(error))
    }
  }

  async function connectRemote() {
    if (!canConnect) return

    setStatus("connecting")
    setFeedback(null)

    try {
      const result = await invokeTauri<RemoteServerConnectionResult>("test_remote_server_connection", {
        connection,
      })
      const connectedConnection = {
        ...connection,
        host: result.host,
        port: String(result.port),
        serverPath: result.serverPath,
      }

      const savedConfig = await invokeTauri<RemoteWorkspaceConfig | null>("get_remote_workspace_config")

      await invokeTauri<RemoteWorkspaceConfig>("save_remote_workspace_config", {
        config: {
          name: result.name,
          host: result.host,
          port: String(result.port),
          username: connection.username,
          authMethod: connection.authMethod,
          sshKeyPath: connection.authMethod === "key" ? connection.sshKeyPath : "",
          serverPath: result.serverPath,
          remoteSteamcmdDir: cleanLegacyPath(savedConfig?.remoteSteamcmdDir) || `${remoteAppDataBase(connection.username)}\\steamcmd-pool\\instance-1`,
          remoteSteamcmdPath: cleanLegacyPath(savedConfig?.remoteSteamcmdPath),
          remoteZomboidServerDir: cleanLegacyPath(savedConfig?.remoteZomboidServerDir) || `${remoteAppDataBase(connection.username)}\\zomboid-server`,
          remoteZomboidServerPath: cleanLegacyPath(savedConfig?.remoteZomboidServerPath),
          remoteClientRam: savedConfig?.remoteClientRam || "4.00",
          remoteServerRam: savedConfig?.remoteServerRam || "4.00",
          remoteModLocations: savedConfig?.remoteModLocations || [],
        },
      })
      onConnected(connectedConnection)
    } catch (error) {
      setStatus("error")
      setFeedback(getErrorMessage(error))
    }
  }

  return (
    <main className="flex min-h-screen bg-[#22272b] text-white">
      <section className="flex w-full justify-center px-6 py-8 sm:px-10 lg:px-16">
        <div className="grid w-full max-w-6xl gap-8 lg:grid-cols-[0.85fr_1.15fr]">
          <aside className="flex flex-col justify-between rounded-[8px] border border-white/10 bg-[#1e2327] p-7">
            <div>
              <button
                type="button"
                onClick={onBack}
                className="mb-8 flex items-center gap-2 rounded-[8px] border border-white/10 px-3 py-2 text-sm font-bold text-gray-300 transition-colors hover:bg-white/5 hover:text-white"
              >
                <ArrowLeft size={17} />
                Back
              </button>

              <div className="flex h-14 w-14 items-center justify-center rounded-[8px] border border-cyan-300/20 bg-cyan-500/10 text-cyan-200">
                <MonitorCog size={28} />
              </div>
              <p className="mt-6 text-xs font-black uppercase tracking-[0.24em] text-cyan-200">Remote workspace</p>
              <h1 className="mt-3 text-3xl font-black tracking-tight">Connect to a Windows server</h1>
              <p className="mt-4 text-sm leading-6 text-gray-400">
                This starts the remote workflow with the SSH information PZ Manager needs to reach a hosted server profile. Remote file sync and validation come next.
              </p>
            </div>

            <div className="mt-8 rounded-[8px] border border-yellow-400/20 bg-yellow-500/10 p-4 text-yellow-100">
              <div className="flex items-start gap-3">
                <ShieldAlert size={20} className="mt-0.5 shrink-0" />
                <p className="text-sm leading-6">
                  Remote workspaces are limited to Windows hosts in 0.4.0. Use a Windows account with SSH access and permission to read the Project Zomboid server profile folder.
                </p>
              </div>
            </div>
          </aside>

          <form
            className="rounded-[8px] border border-white/10 bg-[#2b3238] p-7"
            onSubmit={(event) => {
              event.preventDefault()
              void connectRemote()
            }}
          >
            <div className="mb-7 flex items-start justify-between gap-6">
              <div>
                <h2 className="text-2xl font-black tracking-tight">Connection details</h2>
                <p className="mt-2 text-sm text-gray-400">Fill in the SSH host, authentication, and server profile location.</p>
              </div>
              <span className={`rounded-full border px-3 py-1 text-[10px] font-black uppercase tracking-widest ${
                isWindows
                  ? "border-green-400/20 bg-green-500/10 text-green-300"
                  : "border-red-400/20 bg-red-500/10 text-red-300"
              }`}>
                {isWindows ? "Windows client" : "Unsupported"}
              </span>
            </div>

            <div className="grid gap-5 md:grid-cols-2">
              <RemoteInput
                label="Connection name"
                value={connection.name}
                placeholder="Production server"
                onChange={(value) => updateField("name", value)}
              />
              <RemoteInput
                label="Host or IP"
                value={connection.host}
                placeholder="192.168.0.50"
                onChange={(value) => updateField("host", value)}
              />
              <RemoteInput
                label="SSH port"
                value={connection.port}
                placeholder="22"
                onChange={(value) => updateField("port", value)}
              />
              <RemoteInput
                label="Windows username"
                value={connection.username}
                placeholder="Administrator"
                onChange={(value) => updateField("username", value)}
              />
              <RemoteInput
                label="Server profile folder"
                value={connection.serverPath}
                placeholder="C:\\Users\\Administrator\\Zomboid\\Server"
                onChange={(value) => updateField("serverPath", value)}
              />
            </div>

            <div className="mt-6 rounded-[8px] border border-white/5 bg-[#1e2327] p-4">
              <p className="ml-1 text-[9px] font-black uppercase tracking-[0.2em] text-gray-500">SSH authentication</p>
              <div className="mt-3 grid gap-2 sm:grid-cols-2">
                <button
                  type="button"
                  onClick={() => updateField("authMethod", "password")}
                  className={`flex items-center gap-3 rounded-[8px] border px-4 py-3 text-left transition-all ${
                    connection.authMethod === "password"
                      ? "border-cyan-300/40 bg-cyan-500/10 text-cyan-100"
                      : "border-white/5 bg-[#22272b] text-gray-400 hover:border-white/10 hover:text-white"
                  }`}
                >
                  <Lock size={17} />
                  <span className="text-sm font-bold">Password</span>
                </button>
                <button
                  type="button"
                  onClick={() => updateField("authMethod", "key")}
                  className={`flex items-center gap-3 rounded-[8px] border px-4 py-3 text-left transition-all ${
                    connection.authMethod === "key"
                      ? "border-cyan-300/40 bg-cyan-500/10 text-cyan-100"
                      : "border-white/5 bg-[#22272b] text-gray-400 hover:border-white/10 hover:text-white"
                  }`}
                >
                  <FileKey2 size={17} />
                  <span className="text-sm font-bold">Private key file</span>
                </button>
              </div>

              {connection.authMethod === "password" ? (
                <div className="mt-4">
                  <RemoteInput
                    label="SSH password"
                    type="password"
                    value={connection.password}
                    placeholder="Password for the Windows SSH account"
                    onChange={(value) => updateField("password", value)}
                  />
                </div>
              ) : (
                <div className="mt-4 grid gap-3 sm:grid-cols-[1fr_auto] sm:items-end">
                  <RemoteInput
                    label="SSH private key file"
                    value={connection.sshKeyPath}
                    placeholder="C:\\Users\\You\\.ssh\\id_ed25519"
                    onChange={(value) => updateField("sshKeyPath", value)}
                  />
                  <button
                    type="button"
                    onClick={() => void selectSshKeyFile()}
                    className="flex h-[46px] items-center justify-center gap-2 rounded-[8px] border border-white/10 px-4 text-sm font-bold text-gray-300 transition-colors hover:bg-white/5 hover:text-white"
                  >
                    <Folder size={17} />
                    Choose file
                  </button>
                </div>
              )}
            </div>

            {status === "error" && feedback && (
              <div className="mt-6 rounded-[8px] border border-red-400/20 bg-red-500/10 px-4 py-3 text-sm font-medium text-red-200">
                {feedback}
              </div>
            )}

            {!isWindows && (
              <div className="mt-6 rounded-[8px] border border-red-400/20 bg-red-500/10 px-4 py-3 text-sm font-medium text-red-200">
                Remote workspace setup is available only from Windows for now.
              </div>
            )}

            <div className="mt-8 flex flex-col-reverse gap-3 sm:flex-row sm:justify-end">
              <button
                type="button"
                onClick={onBack}
                className="rounded-[8px] border border-white/10 px-5 py-3 text-sm font-bold text-gray-300 transition-colors hover:bg-white/5 hover:text-white"
              >
                Cancel
              </button>
              <button
                type="submit"
                disabled={!canConnect || status === "connecting"}
                className="flex items-center justify-center gap-2 rounded-[8px] bg-cyan-500 px-6 py-3 text-sm font-black text-white transition-colors hover:bg-cyan-400 disabled:cursor-not-allowed disabled:bg-gray-700 disabled:text-gray-500"
              >
                <KeyRound size={17} />
                {status === "connecting" ? "Connecting..." : "Connect remote"}
              </button>
            </div>
          </form>
        </div>
      </section>
    </main>
  )
}

function RemoteInput({
  label,
  value,
  placeholder,
  type = "text",
  onChange,
}: {
  label: string
  value: string
  placeholder: string
  type?: "text" | "password"
  onChange: (value: string) => void
}) {
  return (
    <label className="space-y-2">
      <span className="ml-1 text-[9px] font-black uppercase tracking-[0.2em] text-gray-500">{label}</span>
      <input
        type={type}
        value={value}
        placeholder={placeholder}
        onChange={(event) => onChange(event.target.value)}
        className="w-full rounded-[8px] border border-white/5 bg-[#1e2327] px-4 py-3 text-sm font-semibold text-white transition-all placeholder:text-gray-600 focus:border-cyan-300/50 focus:outline-none focus:ring-1 focus:ring-cyan-300/20"
      />
    </label>
  )
}
