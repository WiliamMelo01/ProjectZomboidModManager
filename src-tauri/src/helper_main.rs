#![allow(dead_code, unused_imports)]

use base64::Engine;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::{
    collections::HashSet,
    env, fs,
    fs::OpenOptions,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::OnceLock,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

mod game;
mod models;
mod mods;
mod server_test;
mod servers;
mod util;

mod i18n {
    pub(crate) fn text(en: &'static str, _pt_br: &'static str) -> String {
        en.to_string()
    }
}

mod workshop {
    use std::path::Path;

    pub(crate) fn open_file_external(_path: &Path) -> Result<(), String> {
        Err("Opening files is not available in pzmm-helper.".to_string())
    }

    pub(crate) fn open_path_external(_path: &Path) -> Result<(), String> {
        Err("Opening folders is not available in pzmm-helper.".to_string())
    }
}

const MANAGED_STEAMCMD_POOL_DIR_NAME: &str = "steamcmd-pool";
static HELPER_SERVER_LAUNCH_PATH: OnceLock<String> = OnceLock::new();
static HELPER_SERVER_PROFILE_DIR: OnceLock<String> = OnceLock::new();

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateServerModsRequest {
    server_id: String,
    mod_ids: Vec<String>,
    workshop_ids: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerIdRequest {
    server_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestServerRequest {
    server_id: String,
    server_launch_path: Option<String>,
    server_profile_path: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerControlRequest {
    server_id: String,
    server_launch_path: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SendServerCommandRequest {
    server_id: String,
    command: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct HelperServerTestEvent {
    event: String,
    timeout_seconds: Option<u64>,
    line: Option<String>,
    result: Option<models::ServerTestResult>,
    error: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateServerBuildRequest {
    server_id: String,
    game_build: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateServerSettingsRequest {
    server_id: String,
    settings: models::ServerIniSettings,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateServerLuaSettingsRequest {
    server_id: String,
    settings: Vec<models::ServerLuaSetting>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstallModRequest {
    package_path: String,
    mod_id: String,
    workshop_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstallServerMapRequest {
    server_id: String,
    mod_path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateServerRequest {
    name: String,
    mod_ids: Vec<String>,
    workshop_ids: Vec<String>,
    game_build: String,
    max_players: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PathStatusRequest {
    paths: Vec<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PathStatus {
    path: String,
    exists: bool,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let command = env::args()
        .nth(1)
        .ok_or_else(|| "Missing helper command.".to_string())?;

    if command == "run-server-controller" {
        return run_server_controller_from_args();
    }

    match command.as_str() {
        "--version" => print_json(&serde_json::json!({
            "name": "pzmm-helper",
            "version": env!("CARGO_PKG_VERSION"),
        })),
        "list-mods" => print_json(&mods::list_zomboid_mods_impl()?),
        "clear-mods-cache" => {
            mods::clear_zomboid_mods_cache_impl()?;
            print_json(&serde_json::json!({ "ok": true }))
        }
        "get-system-ram" => print_json(&get_system_ram()?),
        "get-path-status" => {
            let request = read_request::<PathStatusRequest>()?;
            print_json(&get_path_status(request.paths)?)
        }
        "list-servers" => print_json(&servers::list_zomboid_servers_impl()?),
        "test-server" => {
            let request = read_request::<TestServerRequest>()?;
            if let Some(server_launch_path) = request
                .server_launch_path
                .as_deref()
                .map(str::trim)
                .filter(|path| !path.is_empty())
            {
                let _ = HELPER_SERVER_LAUNCH_PATH.set(server_launch_path.to_string());
            }
            if let Some(server_profile_path) = request
                .server_profile_path
                .as_deref()
                .map(str::trim)
                .filter(|path| !path.is_empty())
            {
                let _ = HELPER_SERVER_PROFILE_DIR.set(server_profile_path.to_string());
            }
            run_server_test(request.server_id)
        }
        "cancel-server-test" => {
            let request = read_request::<ServerIdRequest>()?;
            request_server_test_cancel(request.server_id)?;
            print_json(&serde_json::json!({ "ok": true }))
        }
        "check-server-firewall" => {
            let request = read_request::<ServerIdRequest>()?;
            print_json(&check_server_firewall(request.server_id)?)
        }
        "configure-server-firewall" => {
            let request = read_request::<ServerIdRequest>()?;
            print_json(&configure_server_firewall(request.server_id)?)
        }
        "start-server" => {
            let request = read_request::<ServerControlRequest>()?;
            if let Some(server_launch_path) = request
                .server_launch_path
                .as_deref()
                .map(str::trim)
                .filter(|path| !path.is_empty())
            {
                let _ = HELPER_SERVER_LAUNCH_PATH.set(server_launch_path.to_string());
            }
            print_json(&start_server(request.server_id)?)
        }
        "send-server-command" => {
            let request = read_request::<SendServerCommandRequest>()?;
            print_json(&send_server_command(request.server_id, request.command)?)
        }
        "create-server" => {
            let request = read_request::<CreateServerRequest>()?;
            let example_dir = ensure_embedded_server_example_dir()?;
            print_json(&servers::create_zomboid_server_from_template_impl(
                &example_dir,
                &request.name,
                &request.mod_ids,
                &request.workshop_ids,
                &request.game_build,
                request.max_players,
            )?)
        }
        "get-server-settings" => {
            let request = read_request::<ServerIdRequest>()?;
            print_json(&servers::get_zomboid_server_settings_impl(
                &request.server_id,
            )?)
        }
        "get-server-lua-settings" => {
            let request = read_request::<ServerIdRequest>()?;
            print_json(&servers::get_zomboid_server_lua_settings_impl(
                &request.server_id,
            )?)
        }
        "update-server-mods" => {
            let request = read_request::<UpdateServerModsRequest>()?;
            servers::update_zomboid_server_mods_impl(
                &request.server_id,
                &request.mod_ids,
                &request.workshop_ids,
            )?;
            print_json(&serde_json::json!({ "ok": true }))
        }
        "update-server-build" => {
            let request = read_request::<UpdateServerBuildRequest>()?;
            servers::update_zomboid_server_build(request.server_id, request.game_build)?;
            print_json(&serde_json::json!({ "ok": true }))
        }
        "update-server-settings" => {
            let request = read_request::<UpdateServerSettingsRequest>()?;
            print_json(&servers::update_zomboid_server_settings_impl(
                &request.server_id,
                &request.settings,
            )?)
        }
        "update-server-lua-settings" => {
            let request = read_request::<UpdateServerLuaSettingsRequest>()?;
            print_json(&servers::update_zomboid_server_lua_settings_impl(
                &request.server_id,
                &request.settings,
            )?)
        }
        "install-mod" => {
            let request = read_request::<InstallModRequest>()?;
            let result = mods::install_zomboid_mod(
                request.package_path,
                request.mod_id,
                request.workshop_id,
            )?;
            mods::clear_zomboid_mods_cache_impl()?;
            print_json(&result)
        }
        "install-server-map" => {
            let request = read_request::<InstallServerMapRequest>()?;
            servers::install_zomboid_server_map(request.server_id, request.mod_path)?;
            print_json(&serde_json::json!({ "ok": true }))
        }
        "delete-server" => {
            let request = read_request::<ServerIdRequest>()?;
            print_json(&servers::delete_zomboid_server_impl(&request.server_id)?)
        }
        _ => Err(format!("Unknown helper command: {command}")),
    }
}

fn read_request<T>() -> Result<T, String>
where
    T: DeserializeOwned,
{
    let encoded = env::args()
        .nth(2)
        .ok_or_else(|| "Missing helper request payload.".to_string())?;
    let encoded = if encoded == "-" {
        let mut stdin = String::new();
        std::io::stdin()
            .read_to_string(&mut stdin)
            .map_err(|error| {
                format!("Could not read helper request payload from stdin: {error}")
            })?;
        stdin
    } else {
        encoded
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim().as_bytes())
        .map_err(|error| format!("Could not decode helper request payload: {error}"))?;

    serde_json::from_slice(&bytes)
        .map_err(|error| format!("Could not parse helper request payload: {error}"))
}

fn print_json<T: serde::Serialize>(value: &T) -> Result<(), String> {
    let json = serde_json::to_string(value)
        .map_err(|error| format!("Could not serialize helper response: {error}"))?;
    println!("{json}");
    std::io::stdout()
        .flush()
        .map_err(|error| format!("Could not flush helper response: {error}"))?;
    Ok(())
}

fn run_server_test(server_id: String) -> Result<(), String> {
    clear_server_test_cancel(&server_id)?;
    print_json(&HelperServerTestEvent {
        event: "started".to_string(),
        timeout_seconds: None,
        line: None,
        result: None,
        error: None,
    })?;

    let result = server_test::test_zomboid_server_impl_with_line_callback_and_cancel(
        &server_id,
        |line| {
            let _ = print_json(&HelperServerTestEvent {
                event: "line".to_string(),
                timeout_seconds: None,
                line: Some(line.to_string()),
                result: None,
                error: None,
            });
        },
        || server_test_cancel_requested(&server_id),
    );
    let _ = clear_server_test_cancel(&server_id);

    match result {
        Ok(result) => print_json(&HelperServerTestEvent {
            event: "finished".to_string(),
            timeout_seconds: None,
            line: None,
            result: Some(result),
            error: None,
        }),
        Err(error) => print_json(&HelperServerTestEvent {
            event: "error".to_string(),
            timeout_seconds: None,
            line: None,
            result: None,
            error: Some(error),
        }),
    }
}

fn server_test_cancel_flag_path(server_id: &str) -> Result<PathBuf, String> {
    Ok(app_config_dir()?
        .join("server-test-cancel")
        .join(format!("{}.cancel", safe_server_test_id(server_id))))
}

fn safe_server_test_id(server_id: &str) -> String {
    let safe_id = server_id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();

    if safe_id.trim().is_empty() {
        "server".to_string()
    } else {
        safe_id
    }
}

fn clear_server_test_cancel(server_id: &str) -> Result<(), String> {
    let flag_path = server_test_cancel_flag_path(server_id)?;
    if flag_path.exists() {
        fs::remove_file(&flag_path).map_err(|error| {
            format!(
                "Could not clear remote server test cancel flag at {}: {error}",
                flag_path.display()
            )
        })?;
    }

    Ok(())
}

fn request_server_test_cancel(server_id: String) -> Result<(), String> {
    let flag_path = server_test_cancel_flag_path(&server_id)?;
    if let Some(parent) = flag_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Could not create remote server test cancel folder at {}: {error}",
                parent.display()
            )
        })?;
    }

    fs::write(&flag_path, "cancel").map_err(|error| {
        format!(
            "Could not request remote server test cancellation at {}: {error}",
            flag_path.display()
        )
    })
}

fn server_test_cancel_requested(server_id: &str) -> bool {
    server_test_cancel_flag_path(server_id)
        .map(|path| path.exists())
        .unwrap_or(false)
}

fn check_server_firewall(server_id: String) -> Result<models::RemoteServerFirewallCheck, String> {
    let server_id = server_id.trim().to_string();
    let ports = server_test::server_ports_for_id(&server_id)?;
    let mut rules = Vec::new();
    let mut logs = vec![format!(
        "Checking Windows Firewall for {} on ports {}.",
        server_id,
        ports
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    )];

    for protocol in firewall_protocols() {
        for port in &ports {
            let allowed = is_firewall_rule_allowed(protocol, *port)?;
            logs.push(format!(
                "{} {} {}.",
                protocol,
                port,
                if allowed {
                    "is allowed"
                } else {
                    "is blocked or missing"
                }
            ));
            rules.push(models::RemoteFirewallRuleStatus {
                protocol: protocol.to_string(),
                port: *port,
                allowed,
            });
        }
    }

    let missing_rules = rules
        .iter()
        .filter(|rule| !rule.allowed)
        .cloned()
        .collect::<Vec<_>>();
    let is_configured = missing_rules.is_empty();

    if is_configured {
        logs.push("Firewall is ready for inbound Project Zomboid connections.".to_string());
    } else {
        logs.push(format!(
            "Firewall needs {} inbound rule(s) before the server is started.",
            missing_rules.len()
        ));
    }

    Ok(models::RemoteServerFirewallCheck {
        server_id,
        ports,
        rules,
        missing_rules,
        is_configured,
        logs,
    })
}

fn configure_server_firewall(
    server_id: String,
) -> Result<models::RemoteServerActionResult, String> {
    let check = check_server_firewall(server_id.clone())?;
    let mut logs = check.logs;

    if check.missing_rules.is_empty() {
        return Ok(models::RemoteServerActionResult {
            success: true,
            message: "Firewall is already configured.".to_string(),
            command: "Get-NetFirewallRule".to_string(),
            logs,
        });
    }

    for rule in check.missing_rules {
        logs.push(format!(
            "Creating inbound firewall rule for {} {}.",
            rule.protocol, rule.port
        ));
        create_firewall_rule(&server_id, &rule.protocol, rule.port)?;
    }

    logs.push("Firewall rules created. Rechecking firewall state.".to_string());
    let updated = check_server_firewall(server_id)?;
    logs.extend(updated.logs);

    Ok(models::RemoteServerActionResult {
        success: updated.is_configured,
        message: if updated.is_configured {
            "Firewall configured successfully.".to_string()
        } else {
            "Firewall is still missing required inbound rules.".to_string()
        },
        command: "New-NetFirewallRule".to_string(),
        logs,
    })
}

fn start_server(server_id: String) -> Result<models::RemoteServerActionResult, String> {
    let server_id = server_id.trim().to_string();
    let launch_path = configured_server_launch_path()?;

    if !launch_path.is_file() {
        return Err(format!(
            "Remote server launcher not found: {}.",
            launch_path.display()
        ));
    }

    let helper_path = env::current_exe()
        .map_err(|error| format!("Could not resolve helper executable path: {error}"))?;
    let log_path = next_server_start_log_path(&server_id)?;
    let command_line = format!(
        "\"{}\" run-server-controller {} \"{}\" \"{}\"",
        helper_path.display(),
        server_id,
        launch_path.display(),
        log_path.display()
    );

    let mut command = Command::new(&helper_path);
    let mut controller = util::hide_command_window(&mut command)
        .arg("run-server-controller")
        .arg(&server_id)
        .arg(&launch_path)
        .arg(&log_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("Could not start remote server controller: {error}"))?;

    let controller_pid = controller.id();
    let mut logs = vec![
        format!("Launcher: {}", launch_path.display()),
        format!("Controller PID: {}.", controller_pid),
        format!("Startup log: {}", log_path.display()),
        "Watching startup output for up to 45 seconds...".to_string(),
    ];
    let started_at = Instant::now();
    let watch_timeout = Duration::from_secs(45);
    let mut seen_lines = 0usize;
    let mut server_started = false;
    let mut controller_exited = false;

    while started_at.elapsed() < watch_timeout {
        let current_lines = read_start_log_lines(&log_path);
        for line in current_lines.iter().skip(seen_lines) {
            if is_remote_server_started_line(line) {
                server_started = true;
            }
            logs.push(line.clone());
        }
        seen_lines = current_lines.len();

        if server_started {
            logs.push("Server startup signal detected. Console command channel is ready.".to_string());
            break;
        }

        if controller
            .try_wait()
            .map_err(|error| format!("Could not inspect remote server controller: {error}"))?
            .is_some()
        {
            controller_exited = true;
            logs.push("Remote server controller exited during startup watch.".to_string());
            break;
        }

        thread::sleep(Duration::from_millis(500));
    }

    if !server_started && !controller_exited {
        logs.push("Startup watch window finished. The controller is still running; commands can be sent while the server keeps starting.".to_string());
    }

    let message = if server_started {
        format!("Remote server started with controller PID {}.", controller_pid)
    } else if controller_exited {
        format!(
            "Remote server controller exited during startup. See {}.",
            log_path.display()
        )
    } else {
        format!("Remote server controller is running with PID {}. Startup confirmation was not detected yet.", controller_pid)
    };

    Ok(models::RemoteServerActionResult {
        success: !controller_exited,
        message,
        command: command_line,
        logs,
    })
}

fn run_server_controller_from_args() -> Result<(), String> {
    let mut args = env::args().skip(2);
    let server_id = args
        .next()
        .ok_or_else(|| "Missing server id for server controller.".to_string())?;
    let launch_path = args
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| "Missing launch path for server controller.".to_string())?;
    let log_path = args
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| "Missing log path for server controller.".to_string())?;

    run_server_controller(&server_id, &launch_path, &log_path)
}

fn run_server_controller(server_id: &str, launch_path: &Path, log_path: &Path) -> Result<(), String> {
    let working_dir = launch_path
        .parent()
        .ok_or_else(|| "Remote server launcher has no parent folder.".to_string())?;
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create server log folder: {error}"))?;
    }

    let command_dir = server_command_queue_dir(server_id)?;
    fs::create_dir_all(&command_dir)
        .map_err(|error| format!("Could not create server command queue: {error}"))?;
    clear_server_command_queue(&command_dir)?;

    append_controller_log(log_path, &format!("[PZMM] Controller started for {server_id}."));
    append_controller_log(log_path, &format!("[PZMM] Launcher: {}", launch_path.display()));
    append_controller_log(log_path, &format!("[PZMM] Command queue: {}", command_dir.display()));

    let stdout_log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map_err(|error| format!("Could not open server startup log: {error}"))?;
    let stderr_log = stdout_log
        .try_clone()
        .map_err(|error| format!("Could not clone server startup log handle: {error}"))?;

    let mut command = Command::new("cmd.exe");
    let mut child = util::hide_command_window(&mut command)
        .arg("/C")
        .arg(&format!("call \"{}\" -servername {}", launch_path.display(), server_id))
        .current_dir(working_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::from(stdout_log))
        .stderr(Stdio::from(stderr_log))
        .spawn()
        .map_err(|error| format!("Could not start remote server process: {error}"))?;

    let child_pid = child.id();
    write_server_controller_state(server_id, child_pid, log_path)?;
    append_controller_log(log_path, &format!("[PZMM] Server process PID {child_pid}."));

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "Could not open remote server stdin.".to_string())?;

    loop {
        process_server_command_queue(&command_dir, log_path, &mut stdin)?;

        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("Could not inspect remote server process: {error}"))?
        {
            append_controller_log(log_path, &format!("[PZMM] Server process exited: {status}."));
            let _ = remove_server_controller_state(server_id);
            return Ok(());
        }

        thread::sleep(Duration::from_millis(250));
    }
}

fn send_server_command(server_id: String, command: String) -> Result<models::RemoteServerActionResult, String> {
    let server_id = server_id.trim().to_string();
    let command = command.trim().to_string();

    if server_id.is_empty() {
        return Err("Server ID cannot be empty.".to_string());
    }
    if command.is_empty() {
        return Err("Server command cannot be empty.".to_string());
    }

    let command_dir = server_command_queue_dir(&server_id)?;
    fs::create_dir_all(&command_dir)
        .map_err(|error| format!("Could not create server command queue: {error}"))?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let command_path = command_dir.join(format!("{timestamp}-{}.cmd", std::process::id()));
    fs::write(&command_path, &command)
        .map_err(|error| format!("Could not queue server command: {error}"))?;

    Ok(models::RemoteServerActionResult {
        success: true,
        message: format!("Command queued for {server_id}."),
        command,
        logs: vec![format!("Queued command file: {}", command_path.display())],
    })
}

fn server_command_queue_dir(server_id: &str) -> Result<PathBuf, String> {
    Ok(app_config_dir()?
        .join("server-command-queues")
        .join(safe_server_test_id(server_id)))
}

fn server_controller_state_path(server_id: &str) -> Result<PathBuf, String> {
    Ok(app_config_dir()?
        .join("server-controllers")
        .join(format!("{}.state", safe_server_test_id(server_id))))
}

fn write_server_controller_state(server_id: &str, child_pid: u32, log_path: &Path) -> Result<(), String> {
    let state_path = server_controller_state_path(server_id)?;
    if let Some(parent) = state_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create server controller state folder: {error}"))?;
    }
    fs::write(
        &state_path,
        format!("serverId={server_id}\nprocessPid={child_pid}\nlogPath={}\n", log_path.display()),
    )
    .map_err(|error| format!("Could not write server controller state: {error}"))
}

fn remove_server_controller_state(server_id: &str) -> Result<(), String> {
    let state_path = server_controller_state_path(server_id)?;
    if state_path.exists() {
        fs::remove_file(state_path)
            .map_err(|error| format!("Could not remove server controller state: {error}"))?;
    }
    Ok(())
}

fn clear_server_command_queue(command_dir: &Path) -> Result<(), String> {
    if !command_dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(command_dir).map_err(|error| format!("Could not read command queue: {error}"))? {
        let entry = entry.map_err(|error| format!("Could not read command queue entry: {error}"))?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) == Some("cmd") {
            let _ = fs::remove_file(path);
        }
    }
    Ok(())
}

fn process_server_command_queue(command_dir: &Path, log_path: &Path, stdin: &mut impl Write) -> Result<(), String> {
    let mut commands = Vec::new();
    for entry in fs::read_dir(command_dir).map_err(|error| format!("Could not read command queue: {error}"))? {
        let entry = entry.map_err(|error| format!("Could not read command queue entry: {error}"))?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) == Some("cmd") {
            commands.push(path);
        }
    }
    commands.sort();

    for path in commands {
        let command = fs::read_to_string(&path)
            .map_err(|error| format!("Could not read queued server command: {error}"))?;
        let command = command.trim();
        if !command.is_empty() {
            writeln!(stdin, "{command}")
                .map_err(|error| format!("Could not write command to server stdin: {error}"))?;
            stdin
                .flush()
                .map_err(|error| format!("Could not flush server stdin: {error}"))?;
            append_controller_log(log_path, &format!("[PZMM] Console command sent: {command}"));
        }
        let _ = fs::remove_file(path);
    }

    Ok(())
}

fn append_controller_log(log_path: &Path, line: &str) {
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) {
        let _ = writeln!(file, "{line}");
    }
}
fn next_server_start_log_path(server_id: &str) -> Result<PathBuf, String> {
    let log_dir = app_config_dir()?.join("server-start-logs");
    fs::create_dir_all(&log_dir)
        .map_err(|error| format!("Could not create server start log dir: {error}"))?;
    let safe_id = server_id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();

    Ok(log_dir.join(format!("{safe_id}-{timestamp}.log")))
}

fn read_start_log_lines(log_path: &Path) -> Vec<String> {
    let Ok(content) = fs::read_to_string(log_path) else {
        return Vec::new();
    };

    content.lines().map(ToOwned::to_owned).collect()
}

fn is_remote_server_started_line(line: &str) -> bool {
    let normalized_line = line.to_lowercase();

    normalized_line.contains("*** server started")
        || normalized_line.contains("server is listening on port")
        || normalized_line.contains("raknet.startup() return code: 0")
        || normalized_line.contains("luanet: initialization [done]")
}
fn configured_server_launch_path() -> Result<PathBuf, String> {
    HELPER_SERVER_LAUNCH_PATH
        .get()
        .map(|path| PathBuf::from(path.trim()))
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| "Remote server launch path is not configured.".to_string())
}

fn firewall_protocols() -> [&'static str; 2] {
    ["UDP", "TCP"]
}

fn is_firewall_rule_allowed(protocol: &str, port: u16) -> Result<bool, String> {
    let port_text = port.to_string();
    let script = r#"$ErrorActionPreference='Stop'; $protocol=__PROTOCOL__; $port=__PORT__; $filters=Get-NetFirewallPortFilter -Protocol $protocol -ErrorAction SilentlyContinue | Where-Object { $_.LocalPort -eq 'Any' -or (($_.LocalPort -split ',') | ForEach-Object { $_.Trim() }) -contains $port }; $allowed=$false; foreach($filter in $filters){ $rule=$filter | Get-NetFirewallRule -ErrorAction SilentlyContinue; if($rule -and $rule.Enabled -eq 'True' -and $rule.Direction -eq 'Inbound' -and $rule.Action -eq 'Allow'){ $allowed=$true; break } }; if($allowed){ 'true' } else { 'false' }"#
        .replace("__PROTOCOL__", &quote_powershell_single_string(protocol))
        .replace("__PORT__", &quote_powershell_single_string(&port_text));
    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .output()
        .map_err(|error| format!("Could not inspect Windows Firewall: {error}"))?;

    if !output.status.success() {
        return Err(command_output_error(
            "Could not inspect Windows Firewall",
            &output,
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .trim()
        .eq_ignore_ascii_case("true"))
}

fn create_firewall_rule(server_id: &str, protocol: &str, port: u16) -> Result<(), String> {
    let port_text = port.to_string();
    let display_name = format!("PZMM Project Zomboid {server_id} {protocol} {port}");
    let script = r#"$ErrorActionPreference='Stop'; $protocol=__PROTOCOL__; $port=__PORT__; $displayName=__DISPLAY_NAME__; $existing=Get-NetFirewallRule -DisplayName $displayName -ErrorAction SilentlyContinue; if($existing){ Set-NetFirewallRule -DisplayName $displayName -Enabled True -Direction Inbound -Action Allow -Profile Any | Out-Null } else { New-NetFirewallRule -DisplayName $displayName -Direction Inbound -Action Allow -Protocol $protocol -LocalPort $port -Profile Any | Out-Null }; Write-Output $displayName"#
        .replace("__PROTOCOL__", &quote_powershell_single_string(protocol))
        .replace("__PORT__", &quote_powershell_single_string(&port_text))
        .replace("__DISPLAY_NAME__", &quote_powershell_single_string(&display_name));
    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .output()
        .map_err(|error| format!("Could not create Windows Firewall rule: {error}"))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(command_output_error(
            "Could not create Windows Firewall rule",
            &output,
        ))
    }
}

fn quote_powershell_single_string(value: &str) -> String {
    format!("'{}'", value.replace("'", "''"))
}
fn command_output_error(prefix: &str, output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let details = [stdout, stderr]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    if details.is_empty() {
        format!("{prefix}: {}", output.status)
    } else {
        format!("{prefix}:\n{details}")
    }
}

fn get_system_ram() -> Result<u32, String> {
    if cfg!(windows) {
        let output = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-Command",
                "[math]::Ceiling((Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory / 1GB)",
            ])
            .output()
            .map_err(|error| format!("Could not detect remote system RAM: {error}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(if stderr.is_empty() {
                "Could not detect remote system RAM.".to_string()
            } else {
                stderr
            });
        }

        return String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<u32>()
            .map(|ram| ram.max(1))
            .map_err(|_| "Could not parse remote system RAM.".to_string());
    }

    Ok(16)
}

fn get_path_status(paths: Vec<String>) -> Result<Vec<PathStatus>, String> {
    Ok(paths
        .into_iter()
        .map(|path| {
            let exists = PathBuf::from(&path).is_dir();
            PathStatus { path, exists }
        })
        .collect())
}

fn ensure_embedded_server_example_dir() -> Result<PathBuf, String> {
    let dir = app_config_dir()?.join("helper").join("server-example");
    fs::create_dir_all(&dir)
        .map_err(|error| format!("Could not create embedded server example dir: {error}"))?;
    fs::write(
        dir.join("servertest.ini"),
        include_bytes!("../../resources/server-example/server_example/servertest.ini"),
    )
    .map_err(|error| format!("Could not write embedded servertest.ini: {error}"))?;
    fs::write(
        dir.join("servertest_SandboxVars.lua"),
        include_bytes!("../../resources/server-example/server_example/servertest_SandboxVars.lua"),
    )
    .map_err(|error| format!("Could not write embedded SandboxVars.lua: {error}"))?;
    fs::write(
        dir.join("servertest_spawnregions.lua"),
        include_bytes!("../../resources/server-example/server_example/servertest_spawnregions.lua"),
    )
    .map_err(|error| format!("Could not write embedded spawnregions.lua: {error}"))?;

    Ok(dir)
}

async fn run_blocking<T, F>(task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    task()
}

fn app_config_dir() -> Result<PathBuf, String> {
    let config_root = env::var_os("LOCALAPPDATA")
        .or_else(|| env::var_os("APPDATA"))
        .or_else(|| env::var_os("USERPROFILE"))
        .or_else(|| env::var_os("HOME"))
        .ok_or_else(|| {
            "Nao foi possivel encontrar a pasta de configuracoes do usuario.".to_string()
        })?;

    Ok(PathBuf::from(config_root).join("ZomboidServerModManager"))
}

fn app_settings_path() -> Result<PathBuf, String> {
    Ok(app_config_dir()?.join("settings.ini"))
}

fn read_config_value(key: &str) -> Result<Option<String>, String> {
    if key == "server_launch_path" {
        if let Some(server_launch_path) = HELPER_SERVER_LAUNCH_PATH
            .get()
            .map(String::as_str)
            .map(str::trim)
            .filter(|path| !path.is_empty())
        {
            return Ok(Some(server_launch_path.to_string()));
        }
    }

    let settings_path = app_settings_path()?;

    if !settings_path.exists() {
        return Ok(None);
    }

    let content = util::read_text_lossy(&settings_path)?;

    Ok(util::read_ini_value(&content, key).filter(|value| !value.trim().is_empty()))
}

fn zomboid_mods_dir() -> Result<PathBuf, String> {
    let home = user_home_dir()?;
    Ok(home.join("Zomboid").join("mods"))
}

fn zomboid_server_dir() -> Result<PathBuf, String> {
    if let Some(server_profile_dir) = HELPER_SERVER_PROFILE_DIR
        .get()
        .map(String::as_str)
        .map(str::trim)
        .filter(|path| !path.is_empty())
    {
        return Ok(PathBuf::from(server_profile_dir));
    }

    let home = user_home_dir()?;
    Ok(home.join("Zomboid").join("Server"))
}

fn user_home_dir() -> Result<PathBuf, String> {
    env::var_os("USERPROFILE")
        .or_else(|| env::var_os("HOME"))
        .map(PathBuf::from)
        .ok_or_else(|| "Nao foi possivel encontrar a pasta do usuario.".to_string())
}

fn server_example_dir(_app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Err("Creating servers is not available through pzmm-helper yet.".to_string())
}

fn steamcmd_executable_name() -> &'static str {
    if cfg!(windows) {
        "steamcmd.exe"
    } else {
        "steamcmd"
    }
}

fn managed_steamcmd_pool_dir() -> Result<PathBuf, String> {
    Ok(app_config_dir()?.join(MANAGED_STEAMCMD_POOL_DIR_NAME))
}

fn managed_steamcmd_pool_workshop_dirs() -> Vec<PathBuf> {
    let Ok(pool_dir) = managed_steamcmd_pool_dir() else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(pool_dir) else {
        return Vec::new();
    };

    let mut entries = entries.filter_map(Result::ok).collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());

    entries
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.starts_with("instance-"))
                .unwrap_or(false)
        })
        .filter_map(|path| {
            path.join(steamcmd_executable_name())
                .parent()
                .map(|steamcmd_dir| {
                    steamcmd_dir
                        .join("steamapps")
                        .join("workshop")
                        .join("content")
                        .join("108600")
                })
        })
        .collect()
}

fn saved_custom_mod_dirs() -> Result<Vec<PathBuf>, String> {
    let settings_path = app_config_dir()?.join("settings.ini");
    if !settings_path.exists() {
        return Ok(Vec::new());
    }

    let content = util::read_text_lossy(&settings_path)?;
    Ok(util::read_ini_values(&content, "mod_location")
        .into_iter()
        .filter_map(|location| {
            let parts = location.splitn(3, '|').collect::<Vec<_>>();
            let kind = parts.first()?.trim();
            let path = parts.last()?.trim();

            (kind == "custom" && !path.is_empty()).then(|| PathBuf::from(path))
        })
        .collect())
}

fn read_steam_library_dirs(libraryfolders_path: &Path) -> Vec<PathBuf> {
    let Ok(content) = util::read_text_lossy(libraryfolders_path) else {
        return Vec::new();
    };

    content
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();

            if !trimmed.starts_with("\"path\"") {
                return None;
            }

            let parts: Vec<&str> = trimmed.split('"').collect();
            let path = parts.get(3)?;
            Some(PathBuf::from(path.replace("\\\\", "\\")).join("steamapps"))
        })
        .collect()
}
