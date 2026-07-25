use crate::command_runner::run_shell_command;
use crate::i18n::text;
use crate::models::{
    AppSettings, DeleteServerResult, ModLocation, RemoteAppSettingsRequest,
    RemoteHelperSetupResult, RemoteModLocationRequest, RemoteServerActionResult,
    RemoteServerConnectionRequest, RemoteServerConnectionResult, RemoteServerDeployRequest,
    RemoteServerDeployResult, RemoteServerFirewallCheck, RemoteServerLatencyResult,
    RemoteSetupLogEvent, RemoteSteamCmdUploadRequest, RemoteSteamCmdUploadResult,
    RemoteWorkspaceConfig, RemoteZomboidServerInstallRequest, RemoteZomboidServerInstallResult,
    RemoteZomboidServerPathRequest, ServerIniSettings, ServerLuaSetting, ServerLuaSettings,
    ServerTestEvent, ServerTestResult, ServerTestStarted, TerminalCommandRequest,
    TerminalCommandResult, WorkshopDownloadEvent, WorkshopDownloadFailedItem,
    WorkshopDownloadLogEvent, WorkshopDownloadResult, ZomboidModInstallResult, ZomboidServer,
};
use crate::mods::{list_zomboid_mods_impl, parse_server_mod_ids};
use crate::util::{
    hide_command_window, read_ini_value, read_ini_values, read_text_lossy,
    replace_or_append_ini_value,
};
use crate::workshop::api::{fetch_steam_workshop_collection_items, validate_workshop_id};
use crate::{app_config_dir, run_blocking};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::hash_map::DefaultHasher,
    collections::{HashMap, HashSet},
    fs,
    hash::{Hash, Hasher},
    io::{BufRead, BufReader, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::{mpsc, Mutex, OnceLock},
    thread,
    time::{Duration, Instant},
};
use tauri::Emitter;

const REMOTE_CONNECT_TIMEOUT_SECONDS: u64 = 5;
const REMOTE_LINUX_HELPER_DIR: &str = "/opt/pzmm";
const REMOTE_LINUX_HELPER_PATH: &str = "/opt/pzmm/pzmm-helper";
const REMOTE_LINUX_DATA_DIR: &str = "/var/lib/pzmm";
const REMOTE_LINUX_SERVER_PROFILE_DIR: &str = "/var/lib/pzmm/Zomboid/Server";
const REMOTE_LINUX_STEAMCMD_DIR: &str = "/var/lib/pzmm/steamcmd";
const REMOTE_LINUX_ZOMBOID_SERVER_DIR: &str = "/var/lib/pzmm/zomboid-server";
const REMOTE_LINUX_ZOMBOID_LAUNCHER: &str = "/var/lib/pzmm/zomboid-server/start-server.sh";
const REMOTE_LINUX_MANAGED_USER: &str = "pzmm";
const HELPER_RELEASE_REPOSITORY: &str = "WiliamMelo01/ProjectZomboidModManager";
static VERIFIED_REMOTE_HELPERS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteServerFileContent {
    pub(crate) server_id: String,
    pub(crate) file_name: String,
    pub(crate) path: String,
    pub(crate) content: String,
}

#[tauri::command]
pub(crate) async fn test_remote_server_connection(
    connection: RemoteServerConnectionRequest,
) -> Result<RemoteServerConnectionResult, String> {
    run_blocking(move || test_remote_server_connection_impl(connection)).await
}

fn run_remote_helper_json_with_sudo<T, P>(
    connection: &RemoteServerConnectionRequest,
    helper_command: &str,
    payload: Option<&P>,
) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
    P: Serialize + ?Sized,
{
    let helper_path = ensure_cached_remote_helper(connection)?;
    let encoded_payload = payload
        .map(|payload| {
            let json = serde_json::to_vec(payload)
                .map_err(|error| format!("Could not serialize helper payload: {error}"))?;
            Ok::<_, String>(base64::engine::general_purpose::STANDARD.encode(json))
        })
        .transpose()?;
    let command = match encoded_payload.as_ref() {
        Some(_) => format!(
            "{} {} -",
            remote_helper_sudo_command_prefix(connection, &helper_path),
            linux_shell_quote(helper_command),
        ),
        None => format!(
            "{} {}",
            remote_helper_sudo_command_prefix(connection, &helper_path),
            linux_shell_quote(helper_command),
        ),
    };
    let output = match encoded_payload {
        Some(encoded_payload) => run_ssh_capture_with_stdin(connection, &command, &encoded_payload),
        None => run_ssh_capture(connection, &command),
    };
    let output = match output {
        Ok(output) => output,
        Err(error) => {
            invalidate_remote_helper_cache(connection);
            return Err(error);
        }
    };
    let stdout = output.stdout.trim();

    if stdout.is_empty() {
        invalidate_remote_helper_cache(connection);
        let message = format!("pzmm Linux helper returned no JSON output for {helper_command}.");
        return Err(join_command_output(&[
            message.as_str(),
            "This usually means sudo rejected the command or the remote helper is unavailable.",
            output.stderr.as_str(),
        ]));
    }

    serde_json::from_str::<T>(stdout).map_err(|error| {
        invalidate_remote_helper_cache(connection);
        let message =
            format!("Could not parse pzmm Linux helper JSON output for {helper_command}: {error}");
        join_command_output(&[message.as_str(), stdout, output.stderr.as_str()])
    })
}

#[tauri::command]
pub(crate) async fn test_remote_server_latency(
    connection: RemoteServerConnectionRequest,
) -> Result<RemoteServerLatencyResult, String> {
    run_blocking(move || test_remote_server_latency_impl(connection)).await
}

#[tauri::command]
pub(crate) fn start_remote_zomboid_server_test(
    app: tauri::AppHandle,
    connection: RemoteServerConnectionRequest,
    server_id: String,
) -> Result<ServerTestStarted, String> {
    let server_id = server_id.trim().to_string();

    if server_id.is_empty() {
        return Err(text(
            "Invalid server for testing.",
            "Servidor invalido para teste.",
        )
        .to_string());
    }

    let config = get_remote_workspace_config_for_connection_impl(&connection)?
        .unwrap_or_else(default_remote_workspace_config);
    let mut helper_connection = connection.clone();
    helper_connection.server_path = config.server_path.clone();
    let server_launch_path = config.remote_zomboid_server_path.trim().to_string();

    if server_launch_path.is_empty() {
        return Err(text(
            "Configure the remote Project Zomboid server path before testing the server.",
            "Configure o caminho do servidor Project Zomboid remoto antes de testar o servidor.",
        )
        .to_string());
    }

    let server_launch_path = resolve_remote_zomboid_server_launch_path(
        &connection,
        &config.remote_zomboid_server_dir,
        &server_launch_path,
    )?;

    let event_server_id = server_id.clone();
    thread::spawn(move || {
        if let Err(error) = run_remote_zomboid_server_test_streaming(
            &app,
            &helper_connection,
            &event_server_id,
            &server_launch_path,
        ) {
            let _ = app.emit(
                "server-test-event",
                ServerTestEvent {
                    server_id: event_server_id,
                    event: "error".to_string(),
                    timeout_seconds: None,
                    line: None,
                    result: None,
                    error: Some(error),
                },
            );
        }
    });

    Ok(ServerTestStarted { server_id })
}

#[tauri::command]
pub(crate) async fn cancel_remote_zomboid_server_test(
    connection: RemoteServerConnectionRequest,
    server_id: String,
) -> Result<(), String> {
    run_blocking(move || {
        let _value: Value = run_remote_helper_json_with_sudo(
            &connection,
            "cancel-server-test",
            Some(&serde_json::json!({ "serverId": server_id })),
        )?;
        Ok(())
    })
    .await
}
#[tauri::command]
pub(crate) async fn check_remote_zomboid_server_firewall(
    connection: RemoteServerConnectionRequest,
    server_id: String,
) -> Result<RemoteServerFirewallCheck, String> {
    run_blocking(move || {
        run_remote_helper_json(
            &connection,
            "check-server-firewall",
            Some(&serde_json::json!({ "serverId": server_id })),
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn configure_remote_zomboid_server_firewall(
    connection: RemoteServerConnectionRequest,
    server_id: String,
) -> Result<RemoteServerActionResult, String> {
    run_blocking(move || {
        run_remote_helper_json(
            &connection,
            "configure-server-firewall",
            Some(&serde_json::json!({ "serverId": server_id })),
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn send_remote_zomboid_server_command(
    connection: RemoteServerConnectionRequest,
    server_id: String,
    command: String,
) -> Result<RemoteServerActionResult, String> {
    run_blocking(move || {
        run_remote_helper_json_with_sudo(
            &connection,
            "send-server-command",
            Some(&serde_json::json!({
                "serverId": server_id,
                "command": command,
            })),
        )
    })
    .await
}
#[tauri::command]
pub(crate) async fn read_remote_zomboid_server_file(
    connection: RemoteServerConnectionRequest,
    server_id: String,
) -> Result<RemoteServerFileContent, String> {
    run_blocking(move || {
        run_remote_helper_json_with_sudo(
            &connection,
            "read-server-file",
            Some(&serde_json::json!({ "serverId": server_id })),
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn list_remote_zomboid_server_logs(
    connection: RemoteServerConnectionRequest,
    server_id: String,
) -> Result<Vec<crate::servers::AvailableLogFile>, String> {
    run_blocking(move || list_remote_zomboid_server_logs_impl(&connection, &server_id)).await
}

#[tauri::command]
pub(crate) async fn read_remote_zomboid_server_log_file(
    connection: RemoteServerConnectionRequest,
    server_id: String,
    log_name: String,
) -> Result<RemoteServerFileContent, String> {
    run_blocking(move || {
        read_remote_zomboid_server_log_file_impl(&connection, &server_id, &log_name)
    })
    .await
}

#[tauri::command]
pub(crate) async fn check_remote_zomboid_server_status(
    connection: RemoteServerConnectionRequest,
    server_id: String,
) -> Result<RemoteServerActionResult, String> {
    run_blocking(move || {
        run_remote_helper_json_with_sudo(
            &connection,
            "server-status",
            Some(&serde_json::json!({ "serverId": server_id })),
        )
    })
    .await
}
#[tauri::command]
pub(crate) fn start_remote_zomboid_server(
    app: tauri::AppHandle,
    connection: RemoteServerConnectionRequest,
    server_id: String,
    no_steam: Option<bool>,
) -> Result<RemoteServerActionResult, String> {
    let server_id = server_id.trim().to_string();

    if server_id.is_empty() {
        return Err(text(
            "Invalid server for remote start.",
            "Servidor invalido para iniciar remotamente.",
        )
        .to_string());
    }

    let config = get_remote_workspace_config_for_connection_impl(&connection)?
        .unwrap_or_else(default_remote_workspace_config);
    let mut helper_connection = connection.clone();
    helper_connection.server_path = config.server_path.clone();
    let server_launch_path = config.remote_zomboid_server_path.trim().to_string();

    if server_launch_path.is_empty() {
        return Err(text(
            "Configure the remote Project Zomboid server path before starting the server.",
            "Configure o caminho do servidor Project Zomboid remoto antes de iniciar o servidor.",
        )
        .to_string());
    }

    let server_launch_path = resolve_remote_zomboid_server_launch_path(
        &connection,
        &config.remote_zomboid_server_dir,
        &server_launch_path,
    )?;
    let event_server_id = server_id.clone();
    let no_steam = no_steam.unwrap_or(false);

    thread::spawn(move || {
        if let Err(error) = run_remote_zomboid_server_start_streaming(
            &app,
            &helper_connection,
            &event_server_id,
            &server_launch_path,
            no_steam,
        ) {
            let _ = app.emit(
                "remote-server-start-event",
                ServerTestEvent {
                    server_id: event_server_id,
                    event: "error".to_string(),
                    timeout_seconds: None,
                    line: None,
                    result: None,
                    error: Some(error),
                },
            );
        }
    });

    Ok(RemoteServerActionResult {
        success: true,
        message: "Remote server start is running. Logs will stream in real time.".to_string(),
        command: "start-server-streaming".to_string(),
        logs: vec!["Remote server start command sent.".to_string()],
    })
}

#[tauri::command]
pub(crate) fn stream_remote_zomboid_server_logs(
    app: tauri::AppHandle,
    connection: RemoteServerConnectionRequest,
    server_id: String,
    follow_from_end: Option<bool>,
) -> Result<RemoteServerActionResult, String> {
    let server_id = server_id.trim().to_string();

    if server_id.is_empty() {
        return Err(text(
            "Invalid server for remote log streaming.",
            "Servidor invalido para transmissao de logs remotos.",
        )
        .to_string());
    }

    let event_server_id = server_id.clone();
    let config = get_remote_workspace_config_for_connection_impl(&connection)?
        .unwrap_or_else(default_remote_workspace_config);
    let mut helper_connection = connection.clone();
    helper_connection.server_path = config.server_path;

    let follow_from_end = follow_from_end.unwrap_or(false);

    thread::spawn(move || {
        if let Err(error) = run_remote_zomboid_server_logs_streaming(
            &app,
            &helper_connection,
            &event_server_id,
            follow_from_end,
        ) {
            let _ = app.emit(
                "remote-server-start-event",
                ServerTestEvent {
                    server_id: event_server_id,
                    event: "error".to_string(),
                    timeout_seconds: None,
                    line: None,
                    result: None,
                    error: Some(error),
                },
            );
        }
    });

    Ok(RemoteServerActionResult {
        success: true,
        message: "Remote server log streaming is running.".to_string(),
        command: "stream-server-logs".to_string(),
        logs: vec!["Remote server log streaming started.".to_string()],
    })
}

#[tauri::command]
pub(crate) async fn select_ssh_key_file() -> Result<Option<String>, String> {
    run_blocking(select_ssh_key_file_impl).await
}
#[tauri::command]
pub(crate) async fn generate_ssh_public_key(ssh_key_path: String) -> Result<String, String> {
    run_blocking(move || generate_ssh_public_key_impl(&ssh_key_path)).await
}

#[tauri::command]
pub(crate) async fn fix_ssh_key_permissions(ssh_key_path: String) -> Result<String, String> {
    run_blocking(move || fix_ssh_key_permissions_impl(&ssh_key_path)).await
}

fn generate_ssh_public_key_impl(ssh_key_path: &str) -> Result<String, String> {
    let key_path = PathBuf::from(required_field(ssh_key_path, "SSH key file")?);

    if !key_path.is_file() {
        return Err(format!("SSH key file not found: {}.", key_path.display()));
    }

    let mut command = Command::new(ssh_keygen_command_name());
    let output = hide_command_window(&mut command)
        .arg("-y")
        .arg("-f")
        .arg(&key_path)
        .output()
        .map_err(|error| format!("Could not run ssh-keygen: {error}"))?;

    if output.status.success() {
        let public_key = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if public_key.is_empty() {
            return Err("ssh-keygen returned an empty public key.".to_string());
        }
        return Ok(public_key);
    }

    Err(join_command_output(&[
        "Could not generate public key from the selected private key.",
        String::from_utf8_lossy(&output.stdout).as_ref(),
        String::from_utf8_lossy(&output.stderr).as_ref(),
    ]))
}

#[tauri::command]
pub(crate) async fn get_remote_workspace_config(
    connection: Option<RemoteServerConnectionRequest>,
) -> Result<Option<RemoteWorkspaceConfig>, String> {
    run_blocking(move || match connection.as_ref() {
        Some(connection) => get_remote_workspace_config_for_connection_impl(connection),
        None => get_remote_workspace_config_impl(),
    })
    .await
}

#[tauri::command]
pub(crate) async fn save_remote_workspace_config(
    config: RemoteWorkspaceConfig,
) -> Result<RemoteWorkspaceConfig, String> {
    run_blocking(move || save_remote_workspace_config_impl(config)).await
}

#[tauri::command]
pub(crate) async fn delete_remote_workspace_config(
    connection: RemoteServerConnectionRequest,
) -> Result<(), String> {
    run_blocking(move || delete_remote_workspace_config_impl(&connection)).await
}

#[tauri::command]
pub(crate) async fn delete_all_remote_data(
    connection: RemoteServerConnectionRequest,
    confirmation: String,
) -> Result<RemoteServerActionResult, String> {
    run_blocking(move || delete_all_remote_data_impl(&connection, &confirmation)).await
}

#[tauri::command]
pub(crate) async fn get_remote_app_settings(
    connection: RemoteServerConnectionRequest,
) -> Result<AppSettings, String> {
    run_blocking(move || get_remote_app_settings_impl(connection)).await
}

#[tauri::command]
pub(crate) async fn get_remote_system_ram(
    connection: RemoteServerConnectionRequest,
) -> Result<u32, String> {
    run_blocking(move || get_remote_system_ram_impl(&connection)).await
}

#[tauri::command]
pub(crate) async fn save_remote_app_settings(
    request: RemoteAppSettingsRequest,
) -> Result<AppSettings, String> {
    run_blocking(move || save_remote_app_settings_impl(request)).await
}

#[tauri::command]
pub(crate) async fn get_remote_mod_locations(
    connection: RemoteServerConnectionRequest,
) -> Result<Vec<ModLocation>, String> {
    run_blocking(move || get_remote_mod_locations_impl(connection)).await
}

#[tauri::command]
pub(crate) async fn add_remote_mod_location(
    request: RemoteModLocationRequest,
) -> Result<Vec<ModLocation>, String> {
    run_blocking(move || add_remote_mod_location_impl(request)).await
}

#[tauri::command]
pub(crate) async fn open_remote_mod_location(
    request: RemoteModLocationRequest,
) -> Result<(), String> {
    run_blocking(move || open_remote_mod_location_impl(request)).await
}

#[tauri::command]
pub(crate) async fn run_terminal_command(
    request: TerminalCommandRequest,
) -> Result<TerminalCommandResult, String> {
    run_blocking(move || run_terminal_command_impl(request)).await
}

#[tauri::command]
pub(crate) async fn upload_steamcmd_to_remote(
    app: tauri::AppHandle,
    request: RemoteSteamCmdUploadRequest,
) -> Result<RemoteSteamCmdUploadResult, String> {
    run_blocking(move || upload_steamcmd_to_remote_impl(&app, request)).await
}

#[tauri::command]
pub(crate) async fn verify_remote_steamcmd_available(
    connection: RemoteServerConnectionRequest,
) -> Result<RemoteSteamCmdUploadResult, String> {
    run_blocking(move || verify_remote_steamcmd_available_impl(connection)).await
}

#[tauri::command]
pub(crate) async fn setup_remote_helper(
    app: tauri::AppHandle,
    connection: RemoteServerConnectionRequest,
) -> Result<RemoteHelperSetupResult, String> {
    run_blocking(move || setup_remote_helper_impl(Some(&app), &connection)).await
}

#[tauri::command]
pub(crate) async fn save_remote_zomboid_server_path(
    request: RemoteZomboidServerPathRequest,
) -> Result<RemoteWorkspaceConfig, String> {
    run_blocking(move || save_remote_zomboid_server_path_impl(request)).await
}
#[tauri::command]
pub(crate) async fn install_zomboid_server_on_remote(
    app: tauri::AppHandle,
    request: RemoteZomboidServerInstallRequest,
) -> Result<RemoteZomboidServerInstallResult, String> {
    run_blocking(move || install_zomboid_server_on_remote_impl(&app, request)).await
}

#[tauri::command]
pub(crate) async fn list_remote_zomboid_servers(
    connection: RemoteServerConnectionRequest,
) -> Result<Vec<crate::models::ZomboidServer>, String> {
    run_blocking(move || list_remote_zomboid_servers_impl(connection)).await
}

#[tauri::command]
pub(crate) async fn list_remote_zomboid_mods(
    connection: RemoteServerConnectionRequest,
) -> Result<Vec<crate::models::ZomboidMod>, String> {
    run_blocking(move || list_remote_zomboid_mods_impl(connection)).await
}

#[tauri::command]
pub(crate) async fn clear_remote_zomboid_mods_cache(
    connection: RemoteServerConnectionRequest,
) -> Result<(), String> {
    run_blocking(move || {
        let _value: Value = run_remote_helper_json_with_sudo(
            &connection,
            "clear-mods-cache",
            Option::<&Value>::None,
        )?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub(crate) async fn clear_remote_zomboid_mods_and_images_cache(
    connection: RemoteServerConnectionRequest,
) -> Result<(), String> {
    run_blocking(move || {
        let _value: Value = run_remote_helper_json_with_sudo(
            &connection,
            "clear-mods-cache",
            Option::<&Value>::None,
        )?;
        clear_remote_image_cache(&connection)?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub(crate) async fn create_remote_zomboid_server(
    connection: RemoteServerConnectionRequest,
    name: String,
    mod_ids: Vec<String>,
    workshop_ids: Vec<String>,
    game_build: String,
    max_players: u32,
) -> Result<ZomboidServer, String> {
    run_blocking(move || {
        run_remote_helper_json_with_sudo(
            &connection,
            "create-server",
            Some(&serde_json::json!({
                "name": name,
                "modIds": mod_ids,
                "workshopIds": workshop_ids,
                "gameBuild": game_build,
                "maxPlayers": max_players,
            })),
        )
    })
    .await
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DeployProgressPayload {
    status: String,
    detail: Option<String>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct DeployModFolder {
    path: PathBuf,
    workshop_id: Option<String>,
}

#[tauri::command]
pub(crate) async fn deploy_local_zomboid_server_to_remote(
    app: tauri::AppHandle,
    request: RemoteServerDeployRequest,
) -> Result<RemoteServerDeployResult, String> {
    run_blocking(move || deploy_local_zomboid_server_to_remote_impl(&app, request)).await
}

fn deploy_local_zomboid_server_to_remote_impl(
    app: &tauri::AppHandle,
    request: RemoteServerDeployRequest,
) -> Result<RemoteServerDeployResult, String> {
    let connection = &request.connection;
    let server_id = &request.server_id;

    if server_id.trim().is_empty() {
        return Err("Server ID cannot be empty.".to_string());
    }

    let (remote_zomboid_dir, remote_data_owner) =
        remote_zomboid_data_dir_and_owner_for_connection(connection)?;

    // 1. Locate local configuration files
    let _ = app.emit(
        "deploy-progress",
        DeployProgressPayload {
            status: "locating_configs".to_string(),
            detail: None,
        },
    );
    let local_server_dir = crate::zomboid_server_dir()?;
    let local_zomboid_dir = local_server_dir
        .parent()
        .ok_or_else(|| "Could not resolve local Zomboid folder.".to_string())?
        .to_path_buf();
    let ini_path = local_server_dir.join(format!("{server_id}.ini"));
    let lua_path = local_server_dir.join(format!("{server_id}.lua"));
    let sandbox_path = local_server_dir.join(format!("{server_id}_SandboxVars.lua"));
    let spawnregions_path = local_server_dir.join(format!("{server_id}_spawnregions.lua"));
    let spawnpoints_path = local_server_dir.join(format!("{server_id}_spawnpoints.lua"));
    let save_path = local_zomboid_dir
        .join("Saves")
        .join("Multiplayer")
        .join(server_id);
    let db_path = local_zomboid_dir.join("db").join(format!("{server_id}.db"));

    if !ini_path.is_file() {
        return Err(format!(
            "Local server configuration file not found: {}",
            ini_path.display()
        ));
    }

    // 2. Determine mods to copy if include_mods is true
    let _ = app.emit(
        "deploy-progress",
        DeployProgressPayload {
            status: "scanning_mods".to_string(),
            detail: None,
        },
    );
    let mut folders_to_copy: HashSet<DeployModFolder> = HashSet::new();
    let mut skipped_mods = Vec::new();
    let mut active_mods_count = 0;

    if request.include_mods {
        let ini_content = read_text_lossy(&ini_path)?;
        let configured_mods = read_ini_value(&ini_content, "Mods").unwrap_or_default();
        let active_mod_ids = parse_server_mod_ids(&configured_mods);

        if !active_mod_ids.is_empty() {
            let all_mods = list_zomboid_mods_impl()?;
            let mut matched_active_mod_ids = HashSet::new();
            for mod_item in all_mods {
                if mod_item.source == "local" {
                    let matching_ids = matching_active_mod_ids(&mod_item, &active_mod_ids);
                    let workshop_id = mod_item.workshop_id.trim();
                    if !matching_ids.is_empty()
                        && !workshop_id.is_empty()
                        && workshop_id.chars().all(|char| char.is_ascii_digit())
                    {
                        matched_active_mod_ids.extend(matching_ids);
                        folders_to_copy.insert(DeployModFolder {
                            path: PathBuf::from(mod_item.package_path),
                            workshop_id: Some(workshop_id.to_string()),
                        });
                    }
                }
            }
            skipped_mods = active_mod_ids
                .iter()
                .filter(|active_id| !matched_active_mod_ids.contains(&active_id.to_lowercase()))
                .cloned()
                .collect();
            active_mods_count = folders_to_copy.len();

            if !folders_to_copy.is_empty() {
                let _ = app.emit(
                    "deploy-progress",
                    DeployProgressPayload {
                        status: "scanning_mods".to_string(),
                        detail: Some("Checking remote mods manifest...".to_string()),
                    },
                );

                let remote_mods_roots = [
                    remote_steamcmd_home_workshop_dir(connection),
                    remote_default_steam_workshop_dir(connection),
                    join_remote_unix_path(&remote_zomboid_dir, "mods"),
                ]
                .join("\n");
                let manifest_command = format!(
                    r#"PZMM_ROOTS={} python3 - <<'PY'
import json, os
roots = [root for root in os.environ.get("PZMM_ROOTS", "").splitlines() if root]
out = []
for root in roots:
  if root and os.path.isdir(root):
    for base, _dirs, files in os.walk(root):
        for name in files:
            path = os.path.join(base, name)
            rel = os.path.relpath(path, root).replace(os.sep, "/")
            stat = os.stat(path)
            out.append({{"p": rel, "l": stat.st_size, "t": int(stat.st_mtime)}})
print(json.dumps(out, separators=(",", ":")))
PY
"#,
                    linux_shell_quote(&remote_mods_roots)
                );
                let manifest_output = run_ssh_capture(connection, &manifest_command)
                    .map_err(|e| format!("Failed to read remote mods manifest: {e}"))?;
                let remote_files: Vec<RemoteFileItem> =
                    parse_remote_json_array(&manifest_output.stdout)
                        .map_err(|e| format!("Failed to parse remote mods manifest: {e}"))?;

                let mut remote_mods_files: HashMap<String, Vec<RemoteFileItem>> = HashMap::new();
                for mut item in remote_files {
                    let path_lower = item.p.to_lowercase();
                    let mod_path = path_lower
                        .split_once("/mods/")
                        .map(|(_, suffix)| suffix)
                        .unwrap_or(&path_lower)
                        .to_string();
                    item.p = mod_path.clone();
                    if let Some(idx) = mod_path.find('/') {
                        let mod_folder = mod_path[..idx].to_string();
                        remote_mods_files.entry(mod_folder).or_default().push(item);
                    }
                }

                let mut dirty_folders = HashSet::new();
                for deploy_folder in &folders_to_copy {
                    let folder_name = deploy_folder
                        .path
                        .file_name()
                        .ok_or_else(|| "Invalid mod folder name".to_string())?
                        .to_string_lossy()
                        .to_string();
                    let folder_name_lower = folder_name.to_lowercase();

                    let mut local_files = HashMap::new();
                    if let Some(parent) = deploy_folder.path.parent() {
                        let _ = collect_local_files_recursive(
                            &deploy_folder.path,
                            parent,
                            &mut local_files,
                        );
                    }

                    let remote_mod_files = remote_mods_files.get(&folder_name_lower);
                    let is_dirty = match remote_mod_files {
                        None => true,
                        Some(remotes) => {
                            if local_files.len() != remotes.len() {
                                true
                            } else {
                                let mut match_failed = false;
                                for (rel_path, (local_len, local_time)) in &local_files {
                                    let rel_path_lower = rel_path.to_lowercase();
                                    let remote_match = remotes
                                        .iter()
                                        .find(|r| r.p.to_lowercase() == rel_path_lower);
                                    match remote_match {
                                        None => {
                                            match_failed = true;
                                            break;
                                        }
                                        Some(remote_item) => {
                                            if remote_item.l != *local_len
                                                || remote_item.t != *local_time as i64
                                            {
                                                match_failed = true;
                                                break;
                                            }
                                        }
                                    }
                                }
                                match_failed
                            }
                        }
                    };

                    if is_dirty {
                        dirty_folders.insert(deploy_folder.clone());
                    }
                }

                folders_to_copy = dirty_folders;
                active_mods_count = folders_to_copy.len();
            }
        }
    }

    // 3. Prepare staging directory
    let _ = app.emit(
        "deploy-progress",
        DeployProgressPayload {
            status: "staging".to_string(),
            detail: None,
        },
    );
    let temp_root = app_config_dir()?.join("temp_deploy");
    let temp_dir = temp_root.join(server_id);

    if temp_dir.exists() {
        let _ = fs::remove_dir_all(&temp_dir);
    }

    let temp_server_bundle_dir = temp_dir.join("server-bundle");
    let temp_server_dir = temp_server_bundle_dir.join("Server");
    let temp_saves_multiplayer_dir = temp_server_bundle_dir.join("Saves").join("Multiplayer");
    let temp_db_dir = temp_server_bundle_dir.join("db");
    let temp_mods_dir = temp_dir.join("mods");

    fs::create_dir_all(&temp_server_dir)
        .map_err(|e| format!("Could not create local temp Server folder: {e}"))?;

    if !folders_to_copy.is_empty() {
        fs::create_dir_all(&temp_mods_dir)
            .map_err(|e| format!("Could not create local temp mods folder: {e}"))?;
    }

    // 4. Copy config files
    let _ = app.emit(
        "deploy-progress",
        DeployProgressPayload {
            status: "copying_configs".to_string(),
            detail: None,
        },
    );
    let mut deployed_server_files = 0;

    fs::copy(&ini_path, temp_server_dir.join(format!("{server_id}.ini")))
        .map_err(|e| format!("Could not copy ini file to temp: {e}"))?;
    deployed_server_files += 1;

    let optional_server_files = [
        (&lua_path, format!("{server_id}.lua"), "server lua file"),
        (
            &sandbox_path,
            format!("{server_id}_SandboxVars.lua"),
            "SandboxVars file",
        ),
        (
            &spawnregions_path,
            format!("{server_id}_spawnregions.lua"),
            "spawnregions file",
        ),
        (
            &spawnpoints_path,
            format!("{server_id}_spawnpoints.lua"),
            "spawnpoints file",
        ),
    ];

    for (source_path, target_name, label) in optional_server_files {
        if source_path.is_file() {
            fs::copy(source_path, temp_server_dir.join(target_name))
                .map_err(|e| format!("Could not copy {label} to temp: {e}"))?;
            deployed_server_files += 1;
        }
    }

    if save_path.is_dir() {
        let save_target = temp_saves_multiplayer_dir.join(server_id);
        copy_dir_all(&save_path, &save_target)?;
        deployed_server_files += count_files_recursive(&save_target)?;
    }

    if db_path.is_file() {
        fs::create_dir_all(&temp_db_dir)
            .map_err(|e| format!("Could not create local temp db folder: {e}"))?;
        fs::copy(&db_path, temp_db_dir.join(format!("{server_id}.db")))
            .map_err(|e| format!("Could not copy server db file to temp: {e}"))?;
        deployed_server_files += 1;
    }

    // 5. Copy local mods
    let total_mods = folders_to_copy.len();
    for (i, deploy_folder) in folders_to_copy.iter().enumerate() {
        let folder_name = deploy_folder
            .path
            .file_name()
            .ok_or_else(|| "Invalid mod folder name".to_string())?;
        let workshop_id = deploy_folder.workshop_id.as_deref().ok_or_else(|| {
            "A Workshop ID is required to deploy mods into the Steam folder.".to_string()
        })?;

        let _ = app.emit(
            "deploy-progress",
            DeployProgressPayload {
                status: "copying_mods".to_string(),
                detail: Some(format!(
                    "{} ({} / {})",
                    folder_name.to_string_lossy(),
                    i + 1,
                    total_mods
                )),
            },
        );

        let dest = temp_mods_dir
            .join(workshop_id)
            .join("mods")
            .join(folder_name);
        copy_dir_all(&deploy_folder.path, &dest)?;
    }

    // 6. Zip server state and mods separately. The server archive keeps the Zomboid folder
    // layout (Server, Saves, db) so it can be extracted directly at the remote Zomboid root.
    let _ = app.emit(
        "deploy-progress",
        DeployProgressPayload {
            status: "compressing".to_string(),
            detail: Some("server.zip".to_string()),
        },
    );
    let server_zip_path = temp_root.join(format!("{server_id}-server.zip"));
    if server_zip_path.is_file() {
        let _ = fs::remove_file(&server_zip_path);
    }
    compress_directory_to_zip(app, &temp_server_bundle_dir, &server_zip_path)?;

    let mods_zip_path = temp_root.join(format!("{server_id}-mods.zip"));
    let has_mods_zip = temp_mods_dir.is_dir() && active_mods_count > 0;
    if has_mods_zip {
        let _ = app.emit(
            "deploy-progress",
            DeployProgressPayload {
                status: "compressing".to_string(),
                detail: Some("mods.zip".to_string()),
            },
        );
        if mods_zip_path.is_file() {
            let _ = fs::remove_file(&mods_zip_path);
        }
        compress_directory_to_zip(app, &temp_mods_dir, &mods_zip_path)?;
    }

    // 7. Upload zips to VM via scp.exe
    let server_zip_size_mb = fs::metadata(&server_zip_path)
        .map(|meta| meta.len() as f64 / 1024.0 / 1024.0)
        .unwrap_or(0.0);
    let mods_zip_size_mb = if has_mods_zip {
        fs::metadata(&mods_zip_path)
            .map(|meta| meta.len() as f64 / 1024.0 / 1024.0)
            .unwrap_or(0.0)
    } else {
        0.0
    };

    let safe_deploy_id = server_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    let remote_server_zip_path = format!("/tmp/pzmm-{safe_deploy_id}-server.zip");
    let remote_mods_zip_path = format!("/tmp/pzmm-{safe_deploy_id}-mods.zip");

    let upload_err_handler = |e| {
        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::remove_file(&server_zip_path);
        let _ = fs::remove_file(&mods_zip_path);
        e
    };

    let _ = app.emit(
        "deploy-progress",
        DeployProgressPayload {
            status: "uploading".to_string(),
            detail: Some("Ensuring remote upload directory exists...".to_string()),
        },
    );
    let mkdir_command = format!(
        "set -e; sudo -n -u {} mkdir -p {} {}",
        linux_sudo_user_arg(&remote_data_owner),
        linux_shell_quote(&remote_zomboid_dir),
        linux_shell_quote(&join_remote_unix_path(&remote_zomboid_dir, "mods")),
    );
    let _ = run_ssh_capture(connection, &mkdir_command).map_err(upload_err_handler)?;

    let _ = app.emit(
        "deploy-progress",
        DeployProgressPayload {
            status: "uploading".to_string(),
            detail: Some(format!("server.zip {:.1} MB", server_zip_size_mb)),
        },
    );
    upload_bundle_to_remote(connection, &server_zip_path, &remote_server_zip_path)
        .map_err(upload_err_handler)?;

    if has_mods_zip {
        let _ = app.emit(
            "deploy-progress",
            DeployProgressPayload {
                status: "uploading".to_string(),
                detail: Some(format!("mods.zip {:.1} MB", mods_zip_size_mb)),
            },
        );
        upload_bundle_to_remote(connection, &mods_zip_path, &remote_mods_zip_path)
            .map_err(upload_err_handler)?;
    }
    // 8. Remote extraction script execution
    let _ = app.emit(
        "deploy-progress",
        DeployProgressPayload {
            status: "extracting".to_string(),
            detail: None,
        },
    );
    let overwrite_existing = if request.overwrite_existing_mods {
        "true"
    } else {
        "false"
    };
    let remote_script = format!(
        r#"set -e
zomboid_dir={zomboid_dir}
steam_mods_target={steam_mods_target}
server_zip={server_zip}
mods_zip={mods_zip}
overwrite={overwrite}
extract_archive() {{
  zip_path="$1"
  target_path="$2"
  label="$3"
  if [ ! -f "$zip_path" ]; then
    echo "PZMM_STEP|Skipping missing $label archive"
    return 0
  fi
  echo "PZMM_STEP|Extracting $label archive directly to $target_path"
  chmod 0644 "$zip_path" || true
  sudo -n -u {data_owner} mkdir -p "$target_path"
  sudo -n -u {data_owner} env PZMM_ZIP="$zip_path" PZMM_TARGET="$target_path" PZMM_OVERWRITE="$overwrite" python3 - <<'PY'
import os, pathlib, zipfile
zip_path = pathlib.Path(os.environ["PZMM_ZIP"])
target = pathlib.Path(os.environ["PZMM_TARGET"]).resolve()
overwrite = os.environ.get("PZMM_OVERWRITE") == "true"
with zipfile.ZipFile(zip_path) as archive:
    for item in archive.infolist():
        archive_path = pathlib.PurePosixPath(item.filename)
        if item.filename.startswith("/") or ".." in archive_path.parts:
            raise RuntimeError(f"Unsafe archive path: {{item.filename}}")
        dest = (target / item.filename).resolve()
        if not str(dest).startswith(str(target)):
            raise RuntimeError(f"Unsafe archive target: {{item.filename}}")
        if item.is_dir():
            dest.mkdir(parents=True, exist_ok=True)
            continue
        if dest.exists() and not overwrite:
            continue
        dest.parent.mkdir(parents=True, exist_ok=True)
        with archive.open(item) as source, open(dest, "wb") as output:
            output.write(source.read())
PY
}}
extract_archive "$server_zip" "$zomboid_dir" 'server data'
extract_archive "$mods_zip" "$steam_mods_target" 'mods'
echo 'DEPLOY_SUCCESS'
echo 'PZMM_STEP|Cleaning remote compressed deploy files'
rm -f "$server_zip" "$mods_zip"
"#,
        zomboid_dir = linux_shell_quote(&remote_zomboid_dir),
        steam_mods_target = linux_shell_quote(&remote_steamcmd_home_workshop_dir(connection)),
        server_zip = linux_shell_quote(&remote_server_zip_path),
        mods_zip = linux_shell_quote(&remote_mods_zip_path),
        overwrite = overwrite_existing,
        data_owner = linux_sudo_user_arg(&remote_data_owner),
    );
    let remote_command = remote_script.clone();

    let ssh_result = match run_ssh_deploy_streaming(app, connection, &remote_command) {
        Ok(res) => res,
        Err(e) => {
            let _ = fs::remove_dir_all(&temp_dir);
            let _ = fs::remove_file(&server_zip_path);
            let _ = fs::remove_file(&mods_zip_path);
            return Err(format!("Remote extraction failed: {e}"));
        }
    };

    // cleanup temp local dir/zip
    let _ = fs::remove_dir_all(&temp_dir);
    let _ = fs::remove_file(&server_zip_path);
    let _ = fs::remove_file(&mods_zip_path);

    if !ssh_result.success || !ssh_result.stdout.contains("DEPLOY_SUCCESS") {
        return Err(format!(
            "Remote extraction script failed.\nStdout: {}\nStderr: {}",
            ssh_result.stdout, ssh_result.stderr
        ));
    }

    Ok(RemoteServerDeployResult {
        success: true,
        server_id: server_id.clone(),
        deployed_server_files,
        deployed_mods: active_mods_count,
        skipped_mods,
        local_bundle_path: server_zip_path.display().to_string(),
        remote_bundle_path: if has_mods_zip {
            format!("{};{}", remote_server_zip_path, remote_mods_zip_path)
        } else {
            remote_server_zip_path
        },
        command: remote_script,
        stdout: ssh_result.stdout,
        stderr: ssh_result.stderr,
        logs: Vec::new(),
    })
}

fn run_ssh_deploy_streaming(
    app: &tauri::AppHandle,
    connection: &RemoteServerConnectionRequest,
    command_text: &str,
) -> Result<TerminalCommandResult, String> {
    let host = required_field(&connection.host, "host")?;
    let username = required_field(&connection.username, "SSH username")?;
    let port = connection
        .port
        .trim()
        .parse::<u16>()
        .map_err(|_| "Enter a valid remote port.".to_string())?;

    if connection.auth_method.trim() != "key" {
        return Err(
            "Remote command execution currently requires SSH private key authentication."
                .to_string(),
        );
    }

    let key_path = PathBuf::from(required_field(&connection.ssh_key_path, "SSH key file")?);
    if !key_path.is_file() {
        return Err(format!("SSH key file not found: {}.", key_path.display()));
    }

    let remote = format!("{username}@{host}");
    let mut ssh_command = Command::new(ssh_command_name());
    append_ssh_command_args(&mut ssh_command, connection, &key_path, port)?;
    let mut child = hide_command_window(&mut ssh_command)
        .args([&remote, command_text])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Could not run ssh: {error}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Could not capture remote deploy stdout.".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Could not capture remote deploy stderr.".to_string())?;
    let (sender, receiver) = mpsc::channel::<(&'static str, String)>();
    let stdout_sender = sender.clone();

    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let _ = stdout_sender.send(("stdout", line));
        }
    });

    thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let _ = sender.send(("stderr", line));
        }
    });

    let mut stdout_lines = Vec::new();
    let mut stderr_lines = Vec::new();

    loop {
        match receiver.recv_timeout(Duration::from_millis(120)) {
            Ok((stream, line)) => {
                emit_deploy_stream_line(app, stream, &line);
                if stream == "stdout" {
                    stdout_lines.push(line);
                } else {
                    stderr_lines.push(line);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Some(status) = child
                    .try_wait()
                    .map_err(|error| format!("Could not read remote deploy status: {error}"))?
                {
                    while let Ok((stream, line)) = receiver.try_recv() {
                        emit_deploy_stream_line(app, stream, &line);
                        if stream == "stdout" {
                            stdout_lines.push(line);
                        } else {
                            stderr_lines.push(line);
                        }
                    }

                    return Ok(TerminalCommandResult {
                        target: "remote".to_string(),
                        command: command_text.to_string(),
                        exit_code: status.code(),
                        success: status.success(),
                        stdout: stdout_lines.join("\n"),
                        stderr: stderr_lines.join("\n"),
                    });
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let status = child.wait().map_err(|error| {
                    format!("Could not wait for remote deploy command: {error}")
                })?;
                return Ok(TerminalCommandResult {
                    target: "remote".to_string(),
                    command: command_text.to_string(),
                    exit_code: status.code(),
                    success: status.success(),
                    stdout: stdout_lines.join("\n"),
                    stderr: stderr_lines.join("\n"),
                });
            }
        }
    }
}

fn emit_deploy_stream_line(app: &tauri::AppHandle, stream: &str, line: &str) {
    let detail = if let Some(value) = line.strip_prefix("PZMM_STEP|") {
        value.to_string()
    } else if let Some(value) = line.strip_prefix("PZMM_FILE|") {
        value.replace('|', " - ")
    } else if let Some(value) = line.strip_prefix("PZMM_MOD_START|") {
        let parts = value.split('|').collect::<Vec<_>>();
        if parts.len() >= 3 {
            format!("Installing {} ({} / {})", parts[0], parts[1], parts[2])
        } else {
            value.to_string()
        }
    } else if let Some(value) = line.strip_prefix("PZMM_MOD_DONE|") {
        let parts = value.split('|').collect::<Vec<_>>();
        if parts.len() >= 3 {
            format!("Installed {} ({} / {})", parts[0], parts[1], parts[2])
        } else {
            value.to_string()
        }
    } else if let Some(value) = line.strip_prefix("PZMM_MOD_SKIPPED|") {
        let parts = value.split('|').collect::<Vec<_>>();
        if parts.len() >= 3 {
            format!(
                "Skipped existing {} ({} / {})",
                parts[0], parts[1], parts[2]
            )
        } else {
            value.to_string()
        }
    } else if stream == "stderr" {
        format!("ERROR: {line}")
    } else {
        line.to_string()
    };

    let _ = app.emit(
        "deploy-progress",
        DeployProgressPayload {
            status: "extracting".to_string(),
            detail: Some(detail),
        },
    );
}
fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> Result<(), String> {
    fs::create_dir_all(&dst)
        .map_err(|e| format!("Could not create directory {}: {e}", dst.as_ref().display()))?;
    for entry in fs::read_dir(src).map_err(|e| format!("Could not read directory: {e}"))? {
        let entry = entry.map_err(|e| format!("Could not read entry: {e}"))?;
        let ty = entry
            .file_type()
            .map_err(|e| format!("Could not get file type: {e}"))?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))
                .map_err(|e| format!("Could not copy file: {e}"))?;
        }
    }
    Ok(())
}

fn count_files_recursive(path: impl AsRef<Path>) -> Result<usize, String> {
    let path = path.as_ref();
    if !path.exists() {
        return Ok(0);
    }

    let mut count = 0;
    for entry in fs::read_dir(path).map_err(|e| format!("Could not read directory: {e}"))? {
        let entry = entry.map_err(|e| format!("Could not read entry: {e}"))?;
        let ty = entry
            .file_type()
            .map_err(|e| format!("Could not get file type: {e}"))?;
        if ty.is_dir() {
            count += count_files_recursive(entry.path())?;
        } else {
            count += 1;
        }
    }
    Ok(count)
}

fn compress_directory_to_zip(
    app: &tauri::AppHandle,
    source_dir: &Path,
    zip_path: &Path,
) -> Result<(), String> {
    use std::io::Read;

    #[cfg(windows)]
    let mut child = {
        let script = format!(
            r#"$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Reflection.Assembly]::LoadWithPartialName('System.IO.Compression') | Out-Null
[Reflection.Assembly]::LoadWithPartialName('System.IO.Compression.FileSystem') | Out-Null

$source = '{}'
$zipPath = '{}'

$archive = [System.IO.Compression.ZipFile]::Open($zipPath, [System.IO.Compression.ZipArchiveMode]::Create)
$files = Get-ChildItem -Path $source -Recurse
$fileList = $files | Where-Object {{ -not $_.PSIsContainer }}
$total = $fileList.Count
$count = 0

foreach ($file in $fileList) {{
    $count++
    $relative = $file.FullName.Substring($source.Length + 1).Replace('\', '/')
    Write-Output "COMPRESS_PROGRESS|$relative|$count|$total"
    [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile($archive, $file.FullName, $relative) | Out-Null
}}

$archive.Dispose()
"#,
            quote_powershell_single_string(&source_dir.display().to_string()),
            quote_powershell_single_string(&zip_path.display().to_string())
        );

        let mut command = Command::new("powershell.exe");
        let command = hide_command_window(&mut command);

        command
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &script,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("Could not start powershell to compress: {error}"))?
    };

    #[cfg(not(windows))]
    let mut child = {
        let script = r#"
import pathlib
import sys
import zipfile

source = pathlib.Path(sys.argv[1])
zip_path = pathlib.Path(sys.argv[2])
files = [path for path in source.rglob('*') if path.is_file()]
total = len(files)
with zipfile.ZipFile(zip_path, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
    for index, path in enumerate(files, 1):
        relative = path.relative_to(source).as_posix()
        print(f'COMPRESS_PROGRESS|{relative}|{index}|{total}', flush=True)
        archive.write(path, relative)
"#;

        Command::new("python3")
            .arg("-c")
            .arg(script)
            .arg(source_dir)
            .arg(zip_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("Could not start python3 to compress: {error}"))?
    };

    let stdout = child
        .stdout
        .take()
        .ok_or("Could not capture compression stdout.")?;
    let stderr = child
        .stderr
        .take()
        .ok_or("Could not capture compression stderr.")?;

    let reader = BufReader::new(stdout);
    for line in reader.lines().map_while(Result::ok) {
        let line = line.trim();
        if line.starts_with("COMPRESS_PROGRESS|") {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 4 {
                let relative = parts[1];
                let current = parts[2];
                let total = parts[3];
                let _ = app.emit(
                    "deploy-progress",
                    DeployProgressPayload {
                        status: "compressing".to_string(),
                        detail: Some(format!("{} ({} / {})", relative, current, total)),
                    },
                );
            }
        }
    }

    let status = child
        .wait()
        .map_err(|error| format!("Could not wait for compression process: {error}"))?;

    if !status.success() {
        let mut err_str = String::new();
        let mut err_reader = BufReader::new(stderr);
        let _ = err_reader.read_to_string(&mut err_str);
        return Err(format!("Compression failed: {}", err_str));
    }

    Ok(())
}

fn upload_bundle_to_remote(
    connection: &RemoteServerConnectionRequest,
    local_path: &Path,
    remote_path: &str,
) -> Result<(), String> {
    let host = required_field(&connection.host, "host")?;
    let username = required_field(&connection.username, "SSH username")?;
    let port = connection
        .port
        .trim()
        .parse::<u16>()
        .map_err(|_| "Enter a valid remote port.".to_string())?;
    let key_path = PathBuf::from(required_field(&connection.ssh_key_path, "SSH key file")?);

    if !key_path.is_file() {
        return Err(format!("SSH key file not found: {}.", key_path.display()));
    }

    let remote = format!("{username}@{host}:{remote_path}");
    let mut scp_command = Command::new(scp_command_name());
    append_scp_command_args(&mut scp_command, connection, &key_path, port)?;
    let local_path_arg = scp_local_path_arg(local_path);
    let output = hide_command_window(&mut scp_command)
        .arg(&local_path_arg)
        .arg(&remote)
        .output()
        .map_err(|error| format!("Could not run scp: {error}"))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    Err(format!("scp failed: {}\n{}", stdout, stderr))
}
#[tauri::command]
pub(crate) async fn delete_remote_zomboid_server(
    connection: RemoteServerConnectionRequest,
    server_id: String,
) -> Result<DeleteServerResult, String> {
    run_blocking(move || {
        run_remote_helper_json_with_sudo(
            &connection,
            "delete-server",
            Some(&serde_json::json!({ "serverId": server_id })),
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn upload_local_mod_to_remote(
    app: tauri::AppHandle,
    connection: RemoteServerConnectionRequest,
    mod_id: String,
    workshop_id: Option<String>,
    local_mod_path: String,
) -> Result<(), String> {
    run_blocking(move || {
        let connection = &connection;
        let local_path = Path::new(&local_mod_path);
        if !local_path.is_dir() {
            return Err(format!("Local mod path is not a directory: {}", local_mod_path));
        }

        let folder_name = local_path.file_name().ok_or_else(|| "Invalid local mod path".to_string())?;

        // 1. Create local temp directory
        let temp_root = app_config_dir()?.join("temp_upload_mod");
        let temp_dir = temp_root.join(&mod_id);
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }
        let temp_mods_dir = temp_dir.join("mods");
        fs::create_dir_all(&temp_mods_dir).map_err(|e| format!("Could not create mods temp directory: {e}"))?;

        // 2. Copy mod files to temp directory
        let dest = temp_mods_dir.join(folder_name);
        copy_dir_all(local_path, &dest)?;

        // 3. Compress to zip
        let zip_file_name = format!("pzmm-mod-{}.zip", mod_id);
        let local_zip_path = temp_dir.join(&zip_file_name);
        compress_directory_to_zip(&app, &temp_mods_dir, &local_zip_path)?;

        // 4. Upload zip to remote home dir
        let remote_zip_path = format!("/tmp/{}", zip_file_name);
        upload_bundle_to_remote(connection, &local_zip_path, &remote_zip_path)?;

        // 5. Unzip on remote
        let (remote_zomboid_dir, remote_data_owner) =
            remote_zomboid_data_dir_and_owner_for_connection(connection)?;
        let workshop_id = workshop_id.unwrap_or_default();
        let workshop_id = workshop_id.trim();
        let mods_target = if workshop_id.chars().all(|char| char.is_ascii_digit()) && !workshop_id.is_empty() {
            join_remote_unix_path(
                &join_remote_unix_path(&remote_steamcmd_home_workshop_dir(connection), workshop_id),
                "mods",
            )
        } else {
            join_remote_unix_path(&remote_zomboid_dir, "mods")
        };

        let remote_script = format!(
            r#"set -e
zip_path={zip_path}
target_path={target_path}
chmod 644 "$zip_path"
sudo -n -u {data_owner} mkdir -p "$target_path"
sudo -n -u {data_owner} env PZMM_ZIP="$zip_path" PZMM_TARGET="$target_path" PZMM_OVERWRITE="true" python3 - <<'PY'
import os, pathlib, zipfile
zip_path = pathlib.Path(os.environ["PZMM_ZIP"])
target = pathlib.Path(os.environ["PZMM_TARGET"]).resolve()
with zipfile.ZipFile(zip_path) as archive:
    for item in archive.infolist():
        archive_path = pathlib.PurePosixPath(item.filename)
        if item.filename.startswith("/") or ".." in archive_path.parts:
            raise RuntimeError(f"Unsafe archive path: {{item.filename}}")
        dest = (target / item.filename).resolve()
        if not str(dest).startswith(str(target)):
            raise RuntimeError(f"Unsafe archive target: {{item.filename}}")
        if item.is_dir():
            dest.mkdir(parents=True, exist_ok=True)
            continue
        dest.parent.mkdir(parents=True, exist_ok=True)
        with archive.open(item) as source, open(dest, "wb") as output:
            output.write(source.read())
PY
rm -f "$zip_path"
"#,
            zip_path = linux_shell_quote(&remote_zip_path),
            target_path = linux_shell_quote(&mods_target),
            data_owner = linux_sudo_user_arg(&remote_data_owner),
        );

        let _ = run_ssh_capture(connection, &remote_script)?;

        // Clear remote helper mods cache
        let _ = run_remote_helper_json_with_sudo::<serde_json::Value, &serde_json::Value>(connection, "clear-mods-cache", None);

        // Cleanup local temp dir
        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }).await
}

#[tauri::command]
pub(crate) async fn get_remote_zomboid_server_settings(
    connection: RemoteServerConnectionRequest,
    server_id: String,
) -> Result<ServerIniSettings, String> {
    run_blocking(move || {
        run_remote_helper_json_with_sudo(
            &connection,
            "get-server-settings",
            Some(&serde_json::json!({ "serverId": server_id })),
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn get_remote_zomboid_server_lua_settings(
    connection: RemoteServerConnectionRequest,
    server_id: String,
) -> Result<ServerLuaSettings, String> {
    run_blocking(move || {
        run_remote_helper_json_with_sudo(
            &connection,
            "get-server-lua-settings",
            Some(&serde_json::json!({ "serverId": server_id })),
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn update_remote_zomboid_server_mods(
    connection: RemoteServerConnectionRequest,
    server_id: String,
    mod_ids: Vec<String>,
    workshop_ids: Vec<String>,
) -> Result<(), String> {
    run_blocking(move || {
        let _value: Value = run_remote_helper_json_with_sudo(
            &connection,
            "update-server-mods",
            Some(&serde_json::json!({
                "serverId": server_id,
                "modIds": mod_ids,
                "workshopIds": workshop_ids,
            })),
        )?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub(crate) async fn update_remote_zomboid_server_build(
    connection: RemoteServerConnectionRequest,
    server_id: String,
    game_build: String,
) -> Result<(), String> {
    run_blocking(move || {
        let _value: Value = run_remote_helper_json_with_sudo(
            &connection,
            "update-server-build",
            Some(&serde_json::json!({ "serverId": server_id, "gameBuild": game_build })),
        )?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub(crate) async fn update_remote_zomboid_server_settings(
    connection: RemoteServerConnectionRequest,
    server_id: String,
    settings: ServerIniSettings,
) -> Result<ZomboidServer, String> {
    run_blocking(move || {
        run_remote_helper_json_with_sudo(
            &connection,
            "update-server-settings",
            Some(&serde_json::json!({ "serverId": server_id, "settings": settings })),
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn update_remote_zomboid_server_lua_settings(
    connection: RemoteServerConnectionRequest,
    server_id: String,
    settings: Vec<ServerLuaSetting>,
) -> Result<ServerLuaSettings, String> {
    run_blocking(move || {
        run_remote_helper_json_with_sudo(
            &connection,
            "update-server-lua-settings",
            Some(&serde_json::json!({ "serverId": server_id, "settings": settings })),
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn install_remote_zomboid_mod(
    connection: RemoteServerConnectionRequest,
    package_path: String,
    mod_id: String,
    workshop_id: String,
) -> Result<ZomboidModInstallResult, String> {
    run_blocking(move || {
        let staged_package_path =
            stage_remote_mod_install_source(&connection, &package_path, &mod_id, &workshop_id)?;

        run_remote_helper_json_with_sudo(
            &connection,
            "install-mod",
            Some(&serde_json::json!({
                "packagePath": staged_package_path,
                "modId": mod_id,
                "workshopId": workshop_id,
            })),
        )
    })
    .await
}

fn stage_remote_mod_install_source(
    connection: &RemoteServerConnectionRequest,
    package_path: &str,
    mod_id: &str,
    workshop_id: &str,
) -> Result<String, String> {
    let requested_source = Path::new(package_path);
    let mods_root = requested_source
        .parent()
        .ok_or_else(|| format!("Could not determine workshop mods folder from {package_path}."))?;
    let requested_folder = requested_source
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| mod_id.trim());
    let cache_workshop_id = safe_remote_cache_segment(if workshop_id.trim().is_empty() {
        "unknown"
    } else {
        workshop_id.trim()
    });
    let (remote_zomboid_dir, data_owner) =
        remote_zomboid_data_dir_and_owner_for_connection(connection)?;
    let cache_root = remote_workspace_cache_dir(&remote_zomboid_dir);
    let staged_mods_root = join_remote_unix_path(
        &join_remote_unix_path(&cache_root, "install-sources"),
        &join_remote_unix_path(&cache_workshop_id, "mods"),
    );
    let staged_package_path = join_remote_unix_path(&staged_mods_root, requested_folder);
    let source_root = mods_root.display().to_string();
    let source_root_dot = join_remote_unix_path(&source_root, ".");

    let command = format!(
        "set -e; data_owner={data_owner}; sudo -n id -u \"$data_owner\" >/dev/null; sudo -n test -d {source_root} || {{ echo {missing_prefix} {source_root} >&2; exit 2; }}; sudo -n -u \"$data_owner\" mkdir -p {staged_root}; sudo -n cp -a {source_dot} {staged_root}/; sudo -n chown -R \"${data_owner}:${data_owner}\" {staged_root}; printf '%s\n' {staged_package}",
        data_owner = linux_sudo_user_arg(&data_owner),
        source_root = linux_shell_quote(&source_root),
        missing_prefix = linux_shell_quote("Workshop mods folder not found:"),
        staged_root = linux_shell_quote(&staged_mods_root),
        source_dot = linux_shell_quote(&source_root_dot),
        staged_package = linux_shell_quote(&staged_package_path),
    );
    let output = run_ssh_capture(connection, &command)?;

    if !output.success {
        return Err(join_command_output(&[
            "Could not stage workshop mod files.",
            output.stdout.as_str(),
            output.stderr.as_str(),
        ]));
    }

    let staged = output.stdout.trim();
    if staged.is_empty() {
        return Ok(staged_package_path);
    }

    Ok(staged.to_string())
}

fn remote_workspace_cache_dir(remote_zomboid_dir: &str) -> String {
    let managed_zomboid_dir = join_remote_unix_path(REMOTE_LINUX_DATA_DIR, "Zomboid");

    if remote_zomboid_dir.trim().trim_end_matches('/') == managed_zomboid_dir {
        join_remote_unix_path(REMOTE_LINUX_DATA_DIR, "cache")
    } else {
        join_remote_unix_path(remote_zomboid_dir, ".pzmm-cache")
    }
}

fn safe_remote_cache_segment(value: &str) -> String {
    let segment = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();

    if segment.trim().is_empty() {
        "unknown".to_string()
    } else {
        segment
    }
}

#[tauri::command]
pub(crate) async fn install_remote_zomboid_server_map(
    connection: RemoteServerConnectionRequest,
    server_id: String,
    mod_path: String,
) -> Result<(), String> {
    run_blocking(move || {
        let _value: Value = run_remote_helper_json_with_sudo(
            &connection,
            "install-server-map",
            Some(&serde_json::json!({ "serverId": server_id, "modPath": mod_path })),
        )?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub(crate) async fn download_remote_steam_workshop_item(
    app: tauri::AppHandle,
    connection: RemoteServerConnectionRequest,
    workshop_id: String,
    force_validate: Option<bool>,
) -> Result<WorkshopDownloadResult, String> {
    run_blocking(move || {
        let workshop_id = validate_workshop_id(&workshop_id, "item")?;
        download_remote_steam_workshop_items_impl(
            &app,
            connection,
            vec![workshop_id],
            force_validate.unwrap_or(false),
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn download_remote_steam_workshop_collection(
    app: tauri::AppHandle,
    connection: RemoteServerConnectionRequest,
    collection_id: String,
    force_validate: Option<bool>,
) -> Result<WorkshopDownloadResult, String> {
    run_blocking(move || {
        let workshop_ids = fetch_steam_workshop_collection_items(&collection_id)?;
        download_remote_steam_workshop_items_impl(
            &app,
            connection,
            workshop_ids,
            force_validate.unwrap_or(false),
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn download_remote_steam_workshop_items(
    app: tauri::AppHandle,
    connection: RemoteServerConnectionRequest,
    workshop_ids: Vec<String>,
    force_validate: Option<bool>,
) -> Result<WorkshopDownloadResult, String> {
    run_blocking(move || {
        let workshop_ids = workshop_ids
            .into_iter()
            .map(|workshop_id| validate_workshop_id(&workshop_id, "item"))
            .collect::<Result<Vec<_>, _>>()?;
        download_remote_steam_workshop_items_impl(
            &app,
            connection,
            workshop_ids,
            force_validate.unwrap_or(false),
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn cancel_remote_steam_workshop_download(
    connection: RemoteServerConnectionRequest,
) -> Result<(), String> {
    run_blocking(move || cancel_remote_steam_workshop_download_impl(connection)).await
}

fn parse_remote_setup_completed_step(content: &str) -> u8 {
    read_ini_value(content, "remote_setup_completed_step")
        .and_then(|value| value.trim().parse::<u8>().ok())
        .unwrap_or(0)
        .min(4)
}

fn mark_remote_setup_completed_step(
    connection: &RemoteServerConnectionRequest,
    completed_step: u8,
) -> Result<RemoteWorkspaceConfig, String> {
    let mut config = get_remote_workspace_config_for_connection_impl(connection)?
        .unwrap_or_else(default_remote_workspace_config);

    config.name = connection.name.clone();
    config.host = connection.host.clone();
    config.port = connection.port.clone();
    config.username = connection.username.clone();
    config.auth_method = connection.auth_method.clone();
    config.ssh_key_path = connection.ssh_key_path.clone();
    if config.server_path.trim().is_empty() {
        config.server_path = connection.server_path.clone();
    }
    config.remote_setup_completed_step = config
        .remote_setup_completed_step
        .max(completed_step.min(4));

    write_remote_workspace_config(&config)?;
    Ok(config)
}

fn get_remote_workspace_config_impl() -> Result<Option<RemoteWorkspaceConfig>, String> {
    read_remote_workspace_config_at_path(&remote_workspace_config_path()?)
}

fn get_remote_workspace_config_for_connection_impl(
    connection: &RemoteServerConnectionRequest,
) -> Result<Option<RemoteWorkspaceConfig>, String> {
    let scoped_path = remote_workspace_config_path_for_connection(connection)?;

    if scoped_path.is_file() {
        return read_remote_workspace_config_at_path(&scoped_path);
    }

    let legacy_config = get_remote_workspace_config_impl()?;
    Ok(legacy_config
        .filter(|config| remote_workspace_config_matches_connection(config, connection)))
}

fn remote_workspace_config_matches_connection(
    config: &RemoteWorkspaceConfig,
    connection: &RemoteServerConnectionRequest,
) -> bool {
    config
        .host
        .trim()
        .eq_ignore_ascii_case(connection.host.trim())
        && config.port.trim() == connection.port.trim()
        && config
            .username
            .trim()
            .eq_ignore_ascii_case(connection.username.trim())
}

fn read_remote_workspace_config_at_path(
    path: &Path,
) -> Result<Option<RemoteWorkspaceConfig>, String> {
    if !path.is_file() {
        return Ok(None);
    }

    let content = read_text_lossy(path)?;
    let remote_steamcmd_dir =
        normalize_legacy_remote_path(read_ini_value(&content, "remote_steamcmd_dir"))
            .unwrap_or_else(default_remote_steamcmd_dir);
    let remote_steamcmd_path =
        normalize_legacy_remote_path(read_ini_value(&content, "remote_steamcmd_path"))
            .unwrap_or_default();
    let remote_zomboid_server_dir =
        normalize_legacy_remote_path(read_ini_value(&content, "remote_zomboid_server_dir"))
            .unwrap_or_else(default_remote_zomboid_server_dir);
    let remote_zomboid_server_path =
        normalize_legacy_remote_path(read_ini_value(&content, "remote_zomboid_server_path"))
            .unwrap_or_default();

    Ok(Some(RemoteWorkspaceConfig {
        name: read_ini_value(&content, "name").unwrap_or_default(),
        host: read_ini_value(&content, "host").unwrap_or_default(),
        port: read_ini_value(&content, "port").unwrap_or_else(|| "22".to_string()),
        username: read_ini_value(&content, "username").unwrap_or_default(),
        auth_method: read_ini_value(&content, "auth_method").unwrap_or_else(|| "key".to_string()),
        ssh_key_path: read_ini_value(&content, "ssh_key_path").unwrap_or_default(),
        server_path: normalize_legacy_remote_path(read_ini_value(&content, "server_path"))
            .unwrap_or_else(|| REMOTE_LINUX_SERVER_PROFILE_DIR.to_string()),
        remote_steamcmd_dir,
        remote_steamcmd_path,
        remote_zomboid_server_dir,
        remote_zomboid_server_path,
        remote_zomboid_server_owner: read_ini_value(&content, "remote_zomboid_server_owner")
            .map(|owner| remote_workspace_owner_or_default(&owner))
            .unwrap_or_else(|| REMOTE_LINUX_MANAGED_USER.to_string()),
        remote_zomboid_data_owner: read_ini_value(&content, "remote_zomboid_data_owner")
            .map(|owner| remote_workspace_owner_or_default(&owner))
            .unwrap_or_else(|| REMOTE_LINUX_MANAGED_USER.to_string()),
        remote_client_ram: read_ini_value(&content, "remote_client_ram")
            .or_else(|| read_ini_value(&content, "client_ram"))
            .unwrap_or_else(|| "4.00".to_string()),
        remote_server_ram: read_ini_value(&content, "remote_server_ram")
            .or_else(|| read_ini_value(&content, "server_ram"))
            .unwrap_or_else(|| "4.00".to_string()),
        remote_setup_completed_step: parse_remote_setup_completed_step(&content),
        remote_mod_locations: read_ini_values(&content, "remote_mod_location"),
    }))
}

fn save_remote_zomboid_server_path_impl(
    request: RemoteZomboidServerPathRequest,
) -> Result<RemoteWorkspaceConfig, String> {
    let mut config = get_remote_workspace_config_for_connection_impl(&request.connection)?
        .unwrap_or_else(default_remote_workspace_config);
    let server_profile_path = resolve_remote_server_profile_path(
        request.server_profile_path.as_deref(),
        &request.connection.server_path,
    )?;
    let resolved_path = resolve_remote_zomboid_server_launch_path(
        &request.connection,
        &request.server_directory,
        &request.server_launch_path,
    )?;
    let resolved_dir = remote_unix_parent_path(&resolved_path)
        .unwrap_or_else(|| request.server_directory.trim().to_string());
    let remote_zomboid_data_dir = remote_unix_parent_path(&server_profile_path)
        .unwrap_or_else(|| join_remote_unix_path(REMOTE_LINUX_DATA_DIR, "Zomboid"));
    let server_owner = detect_remote_path_owner(&request.connection, &resolved_path)?;
    let data_owner = detect_remote_path_owner(&request.connection, &remote_zomboid_data_dir)?;

    config.name = request.connection.name;
    config.host = request.connection.host;
    config.port = request.connection.port;
    config.username = request.connection.username;
    config.auth_method = request.connection.auth_method;
    config.ssh_key_path = request.connection.ssh_key_path;
    config.server_path = server_profile_path;
    config.remote_zomboid_server_dir = resolved_dir;
    config.remote_zomboid_server_path = resolved_path;
    config.remote_zomboid_server_owner = server_owner;
    config.remote_zomboid_data_owner = data_owner;
    config.remote_setup_completed_step = config.remote_setup_completed_step.max(3);
    write_remote_workspace_config(&config)?;
    Ok(config)
}
fn save_remote_workspace_config_impl(
    mut config: RemoteWorkspaceConfig,
) -> Result<RemoteWorkspaceConfig, String> {
    normalize_remote_workspace_config_owners(&mut config)?;
    write_remote_workspace_config(&config)?;
    Ok(config)
}

fn delete_remote_workspace_config_impl(
    connection: &RemoteServerConnectionRequest,
) -> Result<(), String> {
    let scoped_path = remote_workspace_config_path_for_connection(connection)?;
    if scoped_path.is_file() {
        fs::remove_file(&scoped_path).map_err(|error| {
            format!(
                "Nao foi possivel remover {}: {error}",
                scoped_path.display()
            )
        })?;
    }

    let legacy_path = remote_workspace_config_path()?;
    if legacy_path.is_file() {
        if let Some(config) = read_remote_workspace_config_at_path(&legacy_path)? {
            if remote_workspace_config_matches_connection(&config, connection) {
                fs::remove_file(&legacy_path).map_err(|error| {
                    format!(
                        "Nao foi possivel remover {}: {error}",
                        legacy_path.display()
                    )
                })?;
            }
        }
    }

    Ok(())
}

fn delete_all_remote_data_impl(
    connection: &RemoteServerConnectionRequest,
    confirmation: &str,
) -> Result<RemoteServerActionResult, String> {
    const DELETE_ALL_CONFIRMATION: &str = "DELETE ALL PZMM DATA";

    if !delete_all_enabled() {
        return Err("PZMM_DELETEALL_ENABLED is not enabled.".to_string());
    }

    if confirmation.trim() != DELETE_ALL_CONFIRMATION {
        return Err(format!("Type {DELETE_ALL_CONFIRMATION} to confirm."));
    }

    let config = get_remote_workspace_config_for_connection_impl(connection)?
        .unwrap_or_else(default_remote_workspace_config);
    let mut targets = Vec::new();

    let server_profile_path = if config.server_path.trim().is_empty() {
        remote_server_profile_path_or_default(&connection.server_path)
    } else {
        remote_server_profile_path_or_default(&config.server_path)
    };
    if let Some(zomboid_dir) = remote_unix_parent_path(&server_profile_path) {
        push_safe_remote_delete_target(&mut targets, &zomboid_dir);
        if let Some(data_parent) = remote_unix_parent_path(&zomboid_dir) {
            push_safe_remote_delete_target(
                &mut targets,
                &join_remote_unix_path(&data_parent, "Steam/steamapps/workshop/content/108600"),
            );
            push_safe_remote_delete_target(
                &mut targets,
                &join_remote_unix_path(
                    &data_parent,
                    ".local/share/Steam/steamapps/workshop/content/108600",
                ),
            );
        }
    }

    push_safe_remote_delete_target(&mut targets, &config.remote_steamcmd_dir);
    push_safe_remote_delete_target(&mut targets, &config.remote_zomboid_server_dir);

    push_safe_remote_delete_target(&mut targets, REMOTE_LINUX_HELPER_DIR);
    push_safe_remote_delete_target(
        &mut targets,
        &join_remote_unix_path(REMOTE_LINUX_DATA_DIR, "cache"),
    );

    if targets.is_empty() {
        delete_remote_workspace_config_impl(connection)?;
        return Ok(RemoteServerActionResult {
            success: true,
            message: "No remote paths found to delete.".to_string(),
            command: String::new(),
            logs: vec!["No remote paths found to delete.".to_string()],
        });
    }

    targets.sort_by_key(|target| std::cmp::Reverse(target.len()));

    let quoted_targets = targets
        .iter()
        .map(|target| linux_shell_quote(target))
        .collect::<Vec<_>>()
        .join(" ");

    let command = format!(
        r#"sudo -n true || {{ echo 'PZMM_REMOTE_DELETE_FAILED=sudo authentication is required' >&2; exit 1; }}
pgrep -f '[p]zmm-helper' | sudo -n xargs -r kill 2>/dev/null || true
pgrep -f '[s]teamcmd' | sudo -n xargs -r kill 2>/dev/null || true
failures=0
for target in {targets}; do
  case "$target" in
    ""|"/"|"/home"|"/root"|"/opt"|"/var"|"/var/lib"|"/usr"|"/tmp")
      printf 'PZMM_REMOTE_REFUSED=%s\n' "$target" >&2
      failures=$((failures + 1))
      continue
      ;;
  esac
  if sudo -n test -e "$target"; then
    printf 'PZMM_REMOTE_DELETE_ATTEMPT=%s\n' "$target"
    if sudo -n rm -rf -- "$target"; then
      if sudo -n test -e "$target"; then
        printf 'PZMM_REMOTE_DELETE_FAILED=%s still exists after rm\n' "$target" >&2
        failures=$((failures + 1))
      else
        printf 'PZMM_REMOTE_DELETED=%s\n' "$target"
      fi
    else
      printf 'PZMM_REMOTE_DELETE_FAILED=%s rm failed\n' "$target" >&2
      failures=$((failures + 1))
    fi
  else
    printf 'PZMM_REMOTE_SKIPPED=%s\n' "$target"
  fi
done
if [ "$failures" -gt 0 ]; then
  exit 1
fi"#,
        targets = quoted_targets,
    );
    let output = run_ssh_capture_raw(connection, &command)?;

    if !output.success {
        return Err(join_command_output(&[
            "Remote delete-all command failed.",
            output.stdout.as_str(),
            output.stderr.as_str(),
        ]));
    }

    delete_remote_workspace_config_impl(connection)?;

    let mut logs = output
        .stdout
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    logs.push("Remote workspace config removed from this app.".to_string());

    Ok(RemoteServerActionResult {
        success: true,
        message: "Remote PZMM data deleted. Reopen the workspace setup before using this VM again."
            .to_string(),
        command,
        logs,
    })
}

fn delete_all_enabled() -> bool {
    std::env::var("PZMM_DELETEALL_ENABLED")
        .ok()
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            !normalized.is_empty() && !matches!(normalized.as_str(), "0" | "false" | "no" | "off")
        })
        .unwrap_or(false)
}

fn push_safe_remote_delete_target(targets: &mut Vec<String>, path: &str) {
    let normalized = path.trim().trim_end_matches('/');

    if !looks_like_linux_path(normalized) || normalized.is_empty() || normalized == "/" {
        return;
    }

    if matches!(
        normalized,
        "/home" | "/root" | "/opt" | "/var" | "/var/lib" | "/usr" | "/tmp"
    ) {
        return;
    }

    if targets.iter().any(|target| target == normalized) {
        return;
    }

    targets.push(normalized.to_string());
}

fn get_remote_app_settings_impl(
    connection: RemoteServerConnectionRequest,
) -> Result<AppSettings, String> {
    let config = get_remote_workspace_config_for_connection_impl(&connection)?
        .unwrap_or_else(default_remote_workspace_config);

    Ok(remote_app_settings_from_config(&config))
}

fn get_remote_system_ram_impl(connection: &RemoteServerConnectionRequest) -> Result<u32, String> {
    let result = run_ssh_capture(
        connection,
        "awk '/^MemTotal:/ { print int(($2 + 1048575) / 1048576); exit }' /proc/meminfo",
    )?;

    if result.success {
        let ram = result
            .stdout
            .trim()
            .parse::<u32>()
            .map_err(|_| "Could not parse remote system RAM.".to_string())?;

        if ram > 0 {
            return Ok(ram);
        }
    }

    run_remote_helper_json(connection, "get-system-ram", Option::<&Value>::None)
}

fn save_remote_app_settings_impl(request: RemoteAppSettingsRequest) -> Result<AppSettings, String> {
    let client_ram = normalize_remote_ram_gb(&request.client_ram)?;
    let server_ram = normalize_remote_ram_gb(&request.server_ram)?;
    let mut config = get_remote_workspace_config_for_connection_impl(&request.connection)?
        .unwrap_or_else(default_remote_workspace_config);
    let server_path = request.game_executable_path.trim();
    let server_path = if server_path.is_empty() {
        config.remote_zomboid_server_path.trim().to_string()
    } else {
        server_path.to_string()
    };

    if server_path.is_empty() {
        return Err(
            "Configure the remote Project Zomboid server path before saving performance settings."
                .to_string(),
        );
    }

    let server_owner = remote_workspace_owner_or_default(&config.remote_zomboid_server_owner);
    apply_remote_performance_settings(
        &request.connection,
        &server_path,
        &server_ram,
        &server_owner,
    )?;

    config.name = request.connection.name;
    config.host = request.connection.host;
    config.port = request.connection.port;
    config.username = request.connection.username;
    config.auth_method = request.connection.auth_method;
    config.ssh_key_path = request.connection.ssh_key_path;
    if config.server_path.trim().is_empty() {
        config.server_path = remote_server_profile_path_or_default(&request.connection.server_path);
    }
    config.remote_zomboid_server_path = server_path.clone();
    config.remote_zomboid_server_dir = remote_unix_parent_path(&server_path)
        .unwrap_or_else(|| config.remote_zomboid_server_dir.clone());
    config.remote_client_ram = client_ram;
    config.remote_server_ram = server_ram;
    config.remote_setup_completed_step = config.remote_setup_completed_step.max(4);
    write_remote_workspace_config(&config)?;

    Ok(remote_app_settings_from_config(&config))
}

fn remote_app_settings_from_config(config: &RemoteWorkspaceConfig) -> AppSettings {
    AppSettings {
        steamcmd_path: config.remote_steamcmd_path.clone(),
        resolved_steamcmd_path: if config.remote_steamcmd_path.trim().is_empty() {
            None
        } else {
            Some(config.remote_steamcmd_path.clone())
        },
        is_steamcmd_configured: !config.remote_steamcmd_path.trim().is_empty(),
        game_executable_path: config.remote_zomboid_server_path.clone(),
        client_ram: config.remote_client_ram.clone(),
        server_ram: config.remote_server_ram.clone(),
        max_concurrent_downloads: 1,
        language_preference: "auto".to_string(),
    }
}

fn remote_data_dir_for_workshop_paths(connection: &RemoteServerConnectionRequest) -> String {
    remote_zomboid_data_dir_and_owner_for_connection(connection)
        .ok()
        .and_then(|(zomboid_dir, _)| remote_unix_parent_path(&zomboid_dir))
        .unwrap_or_else(|| REMOTE_LINUX_DATA_DIR.to_string())
}

fn remote_default_steam_workshop_dir(connection: &RemoteServerConnectionRequest) -> String {
    join_remote_unix_path(
        &remote_data_dir_for_workshop_paths(connection),
        "Steam/steamapps/workshop/content/108600",
    )
}

fn remote_steamcmd_home_workshop_dir(connection: &RemoteServerConnectionRequest) -> String {
    join_remote_unix_path(
        &remote_data_dir_for_workshop_paths(connection),
        ".local/share/Steam/steamapps/workshop/content/108600",
    )
}

fn remote_default_zomboid_mods_dir(connection: &RemoteServerConnectionRequest) -> String {
    join_remote_unix_path(
        &remote_data_dir_for_workshop_paths(connection),
        "Zomboid/mods",
    )
}

fn remote_steam_workshop_dir_candidates(
    connection: &RemoteServerConnectionRequest,
) -> Vec<(String, String, String)> {
    vec![
        (
            crate::i18n::mod_location_label("steam", None),
            remote_steamcmd_home_workshop_dir(connection),
            "steam".to_string(),
        ),
        (
            crate::i18n::mod_location_label("steam", None),
            remote_default_steam_workshop_dir(connection),
            "steam".to_string(),
        ),
    ]
}

fn remote_extra_steam_workshop_dirs(connection: &RemoteServerConnectionRequest) -> String {
    remote_steam_workshop_dir_candidates(connection)
        .into_iter()
        .map(|(_, path, _)| path)
        .collect::<Vec<_>>()
        .join("\n")
}

fn ensure_remote_steamcmd_workshop_dir_prepared(
    connection: &RemoteServerConnectionRequest,
) -> Result<(), String> {
    let (remote_zomboid_dir, data_owner) =
        remote_zomboid_data_dir_and_owner_for_connection(connection)?;
    let remote_data_dir = remote_unix_parent_path(&remote_zomboid_dir)
        .unwrap_or_else(|| REMOTE_LINUX_DATA_DIR.to_string());
    let target_dir = remote_default_steam_workshop_dir(connection);
    let home_target_dir = remote_steamcmd_home_workshop_dir(connection);
    let steam_root = join_remote_unix_path(&remote_data_dir, "Steam");
    let home_steam_root = join_remote_unix_path(&remote_data_dir, ".local/share/Steam");
    let script = format!(
        "set -e; data_owner={}; sudo -n id -u \"$data_owner\" >/dev/null; sudo -n -u \"$data_owner\" mkdir -p {} {}; sudo -n chown -R \"${data_owner}:${data_owner}\" {} {}",
        linux_sudo_user_arg(&data_owner),
        linux_shell_quote(&target_dir),
        linux_shell_quote(&home_target_dir),
        linux_shell_quote(&steam_root),
        linux_shell_quote(&home_steam_root),
    );
    let output = run_ssh_capture(connection, &script)?;

    if output.success {
        Ok(())
    } else {
        Err(join_command_output(&[
            "Could not prepare the remote SteamCMD Workshop folder.",
            output.stdout.as_str(),
            output.stderr.as_str(),
        ]))
    }
}

fn get_remote_mod_locations_impl(
    connection: RemoteServerConnectionRequest,
) -> Result<Vec<ModLocation>, String> {
    let mut entries = remote_steam_workshop_dir_candidates(&connection);
    entries.push((
        crate::i18n::mod_location_label("local", None),
        remote_default_zomboid_mods_dir(&connection),
        "local".to_string(),
    ));

    // Workshop folders can live inside another user's home directory. Always
    // check them through the helper running as the saved data owner, otherwise
    // an SSH user such as ubuntu may see a false "missing" result for /home/user.
    let paths = entries
        .iter()
        .map(|(_, path, _)| path.clone())
        .collect::<Vec<_>>();
    let remote_paths: Vec<RemotePathExists> = run_remote_helper_json_with_sudo(
        &connection,
        "get-path-status",
        Some(&serde_json::json!({ "paths": paths })),
    )?;

    Ok(entries
        .into_iter()
        .map(|(label, path, kind)| {
            let exists = remote_paths
                .iter()
                .find(|item| item.path == path)
                .map(|item| item.exists)
                .unwrap_or(false);

            ModLocation {
                label,
                path,
                kind,
                exists,
            }
        })
        .collect())
}

fn add_remote_mod_location_impl(
    request: RemoteModLocationRequest,
) -> Result<Vec<ModLocation>, String> {
    let path = required_field(&request.path, "remote mod folder")?;
    if !looks_like_linux_path(&path) {
        return Err(
            "Use an absolute Linux remote mod folder path, for example /var/lib/pzmm/Zomboid/mods."
                .to_string(),
        );
    }

    let mut config = get_remote_workspace_config_for_connection_impl(&request.connection)?
        .unwrap_or_else(default_remote_workspace_config);
    if !config
        .remote_mod_locations
        .iter()
        .any(|current| current == &path)
    {
        config.remote_mod_locations.push(path);
        write_remote_workspace_config(&config)?;
    }

    get_remote_mod_locations_impl(request.connection)
}
fn open_remote_mod_location_impl(request: RemoteModLocationRequest) -> Result<(), String> {
    let path = required_field(&request.path, "remote mod folder")?;
    let statuses: Vec<RemotePathExists> = run_remote_helper_json_with_sudo(
        &request.connection,
        "get-path-status",
        Some(&serde_json::json!({ "paths": [path.clone()] })),
    )?;
    let exists = statuses
        .first()
        .map(|status| status.exists)
        .unwrap_or(false);

    if !exists {
        return Err(format!("Remote folder not found: {path}"));
    }

    Ok(())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemotePathExists {
    path: String,
    exists: bool,
}

fn remote_workspace_config_path() -> Result<PathBuf, String> {
    Ok(app_config_dir()?.join("remote-workspace.ini"))
}

fn remote_workspace_identity_file_name(host: &str, port: &str, username: &str) -> String {
    let identity = format!(
        "{}|{}|{}",
        host.trim().to_lowercase(),
        port.trim(),
        username.trim().to_lowercase()
    );
    let mut hasher = DefaultHasher::new();
    identity.hash(&mut hasher);
    format!("remote-workspace-{:016x}.ini", hasher.finish())
}

fn remote_workspace_config_path_for_identity(
    host: &str,
    port: &str,
    username: &str,
) -> Result<PathBuf, String> {
    if host.trim().is_empty() || username.trim().is_empty() {
        return remote_workspace_config_path();
    }

    Ok(app_config_dir()?.join(remote_workspace_identity_file_name(host, port, username)))
}

fn remote_workspace_config_path_for_connection(
    connection: &RemoteServerConnectionRequest,
) -> Result<PathBuf, String> {
    remote_workspace_config_path_for_identity(
        &connection.host,
        &connection.port,
        &connection.username,
    )
}

fn remote_workspace_config_path_for_config(
    config: &RemoteWorkspaceConfig,
) -> Result<PathBuf, String> {
    remote_workspace_config_path_for_identity(&config.host, &config.port, &config.username)
}

fn write_remote_workspace_config(config: &RemoteWorkspaceConfig) -> Result<(), String> {
    let path = remote_workspace_config_path_for_config(config)?;
    let mut config = config.clone();
    normalize_remote_workspace_config_owners(&mut config)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Nao foi possivel criar {}: {error}", parent.display()))?;
    }

    let mut content = String::new();
    for (key, value) in [
        ("name", config.name.as_str()),
        ("host", config.host.as_str()),
        ("port", config.port.as_str()),
        ("username", config.username.as_str()),
        ("auth_method", config.auth_method.as_str()),
        ("ssh_key_path", config.ssh_key_path.as_str()),
        ("server_path", config.server_path.as_str()),
        ("remote_steamcmd_dir", config.remote_steamcmd_dir.as_str()),
        ("remote_steamcmd_path", config.remote_steamcmd_path.as_str()),
        (
            "remote_zomboid_server_dir",
            config.remote_zomboid_server_dir.as_str(),
        ),
        (
            "remote_zomboid_server_path",
            config.remote_zomboid_server_path.as_str(),
        ),
        (
            "remote_zomboid_server_owner",
            config.remote_zomboid_server_owner.as_str(),
        ),
        (
            "remote_zomboid_data_owner",
            config.remote_zomboid_data_owner.as_str(),
        ),
        ("remote_client_ram", config.remote_client_ram.as_str()),
        ("remote_server_ram", config.remote_server_ram.as_str()),
    ] {
        content = replace_or_append_ini_value(&content, key, value);
    }

    content = replace_or_append_ini_value(
        &content,
        "remote_setup_completed_step",
        &config.remote_setup_completed_step.to_string(),
    );

    for location in &config.remote_mod_locations {
        if !location.trim().is_empty() {
            content.push_str(&format!("\nremote_mod_location={}", location.trim()));
        }
    }

    fs::write(&path, format!("{content}\n"))
        .map_err(|error| format!("Nao foi possivel salvar {}: {error}", path.display()))
}

fn normalize_remote_workspace_config_owners(
    config: &mut RemoteWorkspaceConfig,
) -> Result<(), String> {
    config.remote_zomboid_server_owner =
        remote_workspace_owner_or_default(&config.remote_zomboid_server_owner);
    config.remote_zomboid_data_owner =
        remote_workspace_owner_or_default(&config.remote_zomboid_data_owner);
    validate_linux_username(&config.remote_zomboid_server_owner)?;
    validate_linux_username(&config.remote_zomboid_data_owner)?;
    Ok(())
}

fn test_remote_server_connection_impl(
    connection: RemoteServerConnectionRequest,
) -> Result<RemoteServerConnectionResult, String> {
    let name = required_field(&connection.name, "connection name")?;
    let host = required_field(&connection.host, "host")?;
    let _username = required_field(&connection.username, "SSH username")?;
    validate_authentication(&connection)?;
    let port = connection
        .port
        .trim()
        .parse::<u16>()
        .map_err(|_| "Enter a valid remote port.".to_string())?;

    if connection.auth_method.trim() != "key" {
        return Err("Linux remote workspaces require SSH private key authentication.".to_string());
    }

    let latency = measure_remote_tcp_latency(&host, port)?;
    let diagnostic_log = verify_ssh_key_authentication(&connection, port)?;
    let os_probe = run_ssh_capture(
        &connection,
        "set -e; uname -s; . /etc/os-release 2>/dev/null || true; printf 'PZMM_OS=%s %s\\n' \"${ID:-unknown}\" \"${VERSION_ID:-unknown}\"; test -d /run/systemd/system; command -v sudo >/dev/null; sudo -n true; printf 'PZMM_LINUX_READY\\n'",
    )?;

    if !os_probe.stdout.contains("Linux") || !os_probe.stdout.contains("PZMM_LINUX_READY") {
        return Err(join_command_output(&[
            "The remote host is reachable, but it does not look like a sudo-enabled Linux systemd server.",
            os_probe.stdout.as_str(),
            os_probe.stderr.as_str(),
        ]));
    }

    Ok(RemoteServerConnectionResult {
        name,
        host,
        port,
        server_path: if connection.server_path.trim().is_empty() {
            REMOTE_LINUX_SERVER_PROFILE_DIR.to_string()
        } else {
            connection.server_path.trim().to_string()
        },
        message:
            "Linux SSH host is reachable. systemd and sudo are ready for remote workspace setup."
                .to_string(),
        latency_ms: latency.as_millis(),
        diagnostic_log: join_command_output(&[diagnostic_log.as_str(), os_probe.stdout.as_str()]),
    })
}
fn test_remote_server_latency_impl(
    connection: RemoteServerConnectionRequest,
) -> Result<RemoteServerLatencyResult, String> {
    let host = required_field(&connection.host, "host")?;
    let port = connection
        .port
        .trim()
        .parse::<u16>()
        .map_err(|_| "Enter a valid remote port.".to_string())?;

    Ok(match measure_remote_tcp_latency(&host, port) {
        Ok(latency) => RemoteServerLatencyResult {
            host,
            port,
            success: true,
            latency_ms: Some(latency.as_millis()),
            error: None,
        },
        Err(error) => RemoteServerLatencyResult {
            host,
            port,
            success: false,
            latency_ms: None,
            error: Some(error),
        },
    })
}

pub(crate) fn measure_remote_tcp_latency(host: &str, port: u16) -> Result<Duration, String> {
    let address = format!("{host}:{port}");
    let mut addresses = address
        .to_socket_addrs()
        .map_err(|error| format!("Could not resolve {host}: {error}"))?;
    let socket_address = addresses
        .next()
        .ok_or_else(|| format!("Could not resolve {host}."))?;
    let started_at = Instant::now();

    TcpStream::connect_timeout(
        &socket_address,
        Duration::from_secs(REMOTE_CONNECT_TIMEOUT_SECONDS),
    )
    .map_err(|error| format!("Could not connect to {host}:{port}: {error}"))?;

    Ok(started_at.elapsed())
}

fn resolve_remote_zomboid_server_launch_path(
    connection: &RemoteServerConnectionRequest,
    server_directory: &str,
    server_launch_path: &str,
) -> Result<String, String> {
    let directory = if server_directory.trim().is_empty() {
        REMOTE_LINUX_ZOMBOID_SERVER_DIR
    } else {
        server_directory.trim()
    };
    let launch_path = if server_launch_path.trim().is_empty() {
        REMOTE_LINUX_ZOMBOID_LAUNCHER
    } else {
        server_launch_path.trim()
    };
    let mut candidates = [
        launch_path.to_string(),
        join_remote_unix_path(directory, "start-server.sh"),
        join_remote_unix_path(directory, "ProjectZomboid64"),
    ]
    .into_iter()
    .filter(|candidate| looks_like_linux_path(candidate))
    .collect::<Vec<_>>();
    let mut seen = std::collections::HashSet::new();
    candidates.retain(|x| seen.insert(x.clone()));

    let test_script = format!(
        "set -e; sudo -n true; for candidate in {}; do if sudo -n test -f \"$candidate\"; then printf 'PZMM_ZOMBOID_SERVER_PATH=%s\\n' \"$candidate\"; exit 0; fi; done; printf 'checked with sudo: {}\\n' >&2; exit 1",
        candidates
            .iter()
            .map(|candidate| linux_shell_quote(candidate))
            .collect::<Vec<_>>()
            .join(" "),
        candidates.join(", ")
    );
    let result = run_ssh_capture_raw(connection, &test_script)?;

    if !result.success {
        return Err(join_command_output(&[
            "Could not find the remote Project Zomboid Linux launcher path.",
            result.stdout.as_str(),
            result.stderr.as_str(),
        ]));
    }

    result
        .stdout
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("PZMM_ZOMBOID_SERVER_PATH=")
                .map(str::to_string)
        })
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            join_command_output(&[
                "Could not parse the remote Project Zomboid Linux launcher path.",
                result.stdout.as_str(),
                result.stderr.as_str(),
            ])
        })
}
fn apply_remote_performance_settings(
    connection: &RemoteServerConnectionRequest,
    server_path: &str,
    server_ram: &str,
    server_owner: &str,
) -> Result<(), String> {
    let server_mb = remote_ram_gb_to_mb(server_ram)?;
    let script = build_remote_performance_settings_script(server_path, server_mb, server_owner);
    let result = run_ssh_capture(connection, &script)?;

    if result.success {
        Ok(())
    } else {
        Err(join_command_output(&[
            "Could not apply remote Linux performance settings.",
            result.stdout.as_str(),
            result.stderr.as_str(),
        ]))
    }
}

fn build_remote_performance_settings_script(
    server_path: &str,
    server_mb: u32,
    server_owner: &str,
) -> String {
    let server_dirs = remote_zomboid_server_config_dirs(server_path);
    let quoted_dirs = server_dirs
        .iter()
        .map(|path| linux_shell_quote(path))
        .collect::<Vec<_>>()
        .join(" ");
    let server_owner = linux_sudo_user_arg(server_owner);

    format!(
        r#"set -e
ram={}
server_owner={}
for server_dir in {}; do
  for json_name in ProjectZomboid64.json ProjectZomboid32.json; do
    json_path="$server_dir/$json_name"
    if sudo -n test -f "$json_path"; then
      detected_owner=$(sudo -n stat -c '%U' "$json_path")
      if [ -n "$detected_owner" ] && [ "$detected_owner" != UNKNOWN ]; then
        server_owner="$detected_owner"
        break 2
      fi
    fi
  done
done
sudo -n id -u "$server_owner" >/dev/null
owner_home=$(getent passwd "$server_owner" | cut -d: -f6)
if [ -z "$owner_home" ]; then
  owner_home=/tmp
fi
cd /
sudo -n -u "$server_owner" env HOME="$owner_home" PZMM_DATA_DIR="$owner_home" python3 - "$ram" {} <<'PY'
import json, pathlib, re, sys
ram = sys.argv[1]
server_dirs = [pathlib.Path(value) for value in sys.argv[2:]]
json_names = ("ProjectZomboid64.json", "ProjectZomboid32.json")
checked = []
updated = []

def update_json(json_path):
    data = json.loads(json_path.read_text(errors="replace"))
    vm_args = data.get("vmArgs")

    def update_arg(value):
        if isinstance(value, str) and value.startswith("-Xms"):
            return "-Xms" + ram + "m"
        if isinstance(value, str) and value.startswith("-Xmx"):
            return "-Xmx" + ram + "m"
        return value

    if isinstance(vm_args, list):
        updated_args = [update_arg(value) for value in vm_args]
        if not any(isinstance(value, str) and value.startswith("-Xms") for value in updated_args):
            updated_args.insert(0, "-Xms" + ram + "m")
        if not any(isinstance(value, str) and value.startswith("-Xmx") for value in updated_args):
            updated_args.insert(1, "-Xmx" + ram + "m")
        data["vmArgs"] = updated_args
    elif isinstance(vm_args, str):
        vm_args, n1 = re.subn(r"-Xms\S+", "-Xms" + ram + "m", vm_args)
        vm_args, n2 = re.subn(r"-Xmx\S+", "-Xmx" + ram + "m", vm_args)
        if n1 == 0:
            vm_args = "-Xms" + ram + "m " + vm_args
        if n2 == 0:
            vm_args = "-Xmx" + ram + "m " + vm_args
        data["vmArgs"] = vm_args
    else:
        data["vmArgs"] = ["-Xms" + ram + "m", "-Xmx" + ram + "m"]

    json_path.write_text(json.dumps(data, indent=2) + "\n")
    updated.append(str(json_path))

for server_dir in server_dirs:
    for json_name in json_names:
        json_path = server_dir / json_name
        checked.append(str(json_path))
        if json_path.is_file():
            update_json(json_path)

if not updated:
    raise SystemExit("Remote Project Zomboid JSON launcher config not found. Checked: " + ", ".join(checked))

print("PZMM_REMOTE_PERFORMANCE_UPDATED=" + ",".join(updated))
PY
"#,
        server_mb, server_owner, quoted_dirs, quoted_dirs,
    )
}

fn remote_zomboid_server_config_dirs(server_path: &str) -> Vec<String> {
    let trimmed = server_path.trim().trim_end_matches('/');
    let configured_dir = if trimmed.is_empty() {
        REMOTE_LINUX_ZOMBOID_SERVER_DIR.to_string()
    } else if looks_like_remote_zomboid_launcher_file(trimmed) {
        remote_unix_parent_path(trimmed)
            .unwrap_or_else(|| REMOTE_LINUX_ZOMBOID_SERVER_DIR.to_string())
    } else {
        trimmed.to_string()
    };

    let mut dirs = Vec::new();
    push_unique_string(&mut dirs, configured_dir);
    push_unique_string(&mut dirs, REMOTE_LINUX_ZOMBOID_SERVER_DIR.to_string());
    dirs
}

fn looks_like_remote_zomboid_launcher_file(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();

    name.ends_with(".sh")
        || name.ends_with(".json")
        || name == "projectzomboid64"
        || name == "projectzomboid32"
}

fn push_unique_string(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn resolve_remote_server_profile_path(
    requested_profile_path: Option<&str>,
    connection_profile_path: &str,
) -> Result<String, String> {
    let value = requested_profile_path
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .unwrap_or_else(|| connection_profile_path.trim());
    let profile_path = remote_server_profile_path_or_default(value);

    if !looks_like_linux_path(&profile_path) {
        return Err(
            "Use an absolute Linux Zomboid data folder, for example /var/lib/pzmm/Zomboid."
                .to_string(),
        );
    }

    Ok(profile_path)
}

fn detect_remote_path_owner(
    connection: &RemoteServerConnectionRequest,
    remote_path: &str,
) -> Result<String, String> {
    let script = format!(
        "set -e; path={}; owner=$(sudo -n stat -c '%U' \"$path\"); if [ -z \"$owner\" ] || [ \"$owner\" = UNKNOWN ]; then echo \"Could not resolve Linux owner for $path.\" >&2; exit 1; fi; sudo -n id -u \"$owner\" >/dev/null; printf 'PZMM_PATH_OWNER=%s\\n' \"$owner\"",
        linux_shell_quote(remote_path),
    );
    let result = run_ssh_capture(connection, &script)?;

    if !result.success {
        return Err(join_command_output(&[
            "Could not detect the Linux owner for the selected remote path.",
            result.stdout.as_str(),
            result.stderr.as_str(),
        ]));
    }

    let owner = result
        .stdout
        .lines()
        .find_map(|line| line.trim().strip_prefix("PZMM_PATH_OWNER="))
        .map(str::trim)
        .filter(|owner| !owner.is_empty())
        .ok_or_else(|| {
            join_command_output(&[
                "Could not parse the Linux owner for the selected remote path.",
                result.stdout.as_str(),
                result.stderr.as_str(),
            ])
        })?;

    validate_linux_username(owner)?;
    Ok(owner.to_string())
}

fn validate_linux_username(username: &str) -> Result<(), String> {
    let username = username.trim();

    if username.is_empty()
        || username == "UNKNOWN"
        || username.starts_with('-')
        || !username
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
    {
        return Err(format!(
            "Invalid Linux owner '{username}'. Configure the folder with a valid Linux user."
        ));
    }

    Ok(())
}

fn remote_workspace_owner_or_default(owner: &str) -> String {
    let owner = normalize_remote_owner_value(owner);

    if owner.is_empty() {
        REMOTE_LINUX_MANAGED_USER.to_string()
    } else {
        owner
    }
}

fn normalize_remote_owner_value(owner: &str) -> String {
    let trimmed = owner.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Older builds persisted the output of shell quoting instead of the username
    // itself, for example "$'\\''pzalt'\\'''". Recover only shell punctuation;
    // reject any other characters later through validate_linux_username.
    let mut normalized = String::new();
    let mut had_shell_punctuation = false;
    for character in trimmed.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
            normalized.push(character);
        } else if matches!(character, '$' | '\'' | '"' | '\\') {
            had_shell_punctuation = true;
        } else {
            return trimmed.to_string();
        }
    }

    if had_shell_punctuation && !normalized.is_empty() {
        normalized
    } else {
        trimmed.to_string()
    }
}

fn linux_sudo_user_arg(owner: &str) -> String {
    remote_workspace_owner_or_default(owner)
}

fn remote_zomboid_data_dir_and_owner_for_connection(
    connection: &RemoteServerConnectionRequest,
) -> Result<(String, String), String> {
    let config = get_remote_workspace_config_for_connection_impl(connection)?
        .unwrap_or_else(default_remote_workspace_config);
    let server_profile_path = if config.server_path.trim().is_empty() {
        remote_server_profile_path_or_default(&connection.server_path)
    } else {
        remote_server_profile_path_or_default(&config.server_path)
    };
    let remote_zomboid_dir = remote_unix_parent_path(&server_profile_path)
        .unwrap_or_else(|| join_remote_unix_path(REMOTE_LINUX_DATA_DIR, "Zomboid"));
    let owner = remote_workspace_owner_or_default(&config.remote_zomboid_data_owner);
    validate_linux_username(&owner)?;
    Ok((remote_zomboid_dir, owner))
}

fn list_remote_zomboid_server_logs_impl(
    connection: &RemoteServerConnectionRequest,
    _server_id: &str,
) -> Result<Vec<crate::servers::AvailableLogFile>, String> {
    let (remote_zomboid_dir, owner) = remote_zomboid_data_dir_and_owner_for_connection(connection)?;
    let logs_dir = join_remote_unix_path(&remote_zomboid_dir, "Logs");
    let command = format!(
        r#"set -e
logs_dir={logs_dir}
owner={owner}
sudo -n id -u "$owner" >/dev/null
if ! sudo -n test -d "$logs_dir"; then printf '[]\n'; exit 0; fi
sudo -n -u "$owner" env LOGS_DIR="$logs_dir" python3 - <<'PY'
import json, os, pathlib
logs_dir = pathlib.Path(os.environ["LOGS_DIR"])
items = []
for path in logs_dir.iterdir():
    if not path.is_file() or path.suffix not in (".txt", ".log"):
        continue
    stat = path.stat()
    items.append({{
        "name": path.name,
        "path": str(path),
        "sizeBytes": stat.st_size,
        "lastModified": int(stat.st_mtime),
    }})
items.sort(key=lambda item: item["lastModified"], reverse=True)
print(json.dumps(items))
PY"#,
        logs_dir = linux_shell_quote(&logs_dir),
        owner = linux_sudo_user_arg(&owner),
    );
    let output = run_ssh_capture(connection, &command)?;
    serde_json::from_str(output.stdout.trim()).map_err(|error| {
        format!(
            "Could not parse remote server logs list: {error}\n\n{}",
            join_command_output(&[output.stdout.as_str(), output.stderr.as_str()])
        )
    })
}

fn read_remote_zomboid_server_log_file_impl(
    connection: &RemoteServerConnectionRequest,
    server_id: &str,
    log_name: &str,
) -> Result<RemoteServerFileContent, String> {
    let clean_log_name = Path::new(log_name)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| "Invalid server log name.".to_string())?;

    if !clean_log_name.ends_with(".txt") && !clean_log_name.ends_with(".log") {
        return Err("Invalid server log extension.".to_string());
    }

    let (remote_zomboid_dir, owner) = remote_zomboid_data_dir_and_owner_for_connection(connection)?;
    let logs_dir = join_remote_unix_path(&remote_zomboid_dir, "Logs");
    let command = format!(
        r#"set -e
logs_dir={logs_dir}
owner={owner}
log_name={log_name}
sudo -n id -u "$owner" >/dev/null
sudo -n -u "$owner" env LOGS_DIR="$logs_dir" LOG_NAME="$log_name" SERVER_ID={server_id} python3 - <<'PY'
import json, os, pathlib
logs_dir = pathlib.Path(os.environ["LOGS_DIR"])
log_name = pathlib.Path(os.environ["LOG_NAME"]).name
if not log_name.endswith((".txt", ".log")):
    raise SystemExit("Invalid server log extension.")
path = logs_dir / log_name
if not path.is_file():
    raise SystemExit(f"Server log file not found: {{path}}")
print(json.dumps({{
    "serverId": os.environ["SERVER_ID"],
    "fileName": log_name,
    "path": str(path),
    "content": path.read_text(encoding="utf-8", errors="replace"),
}}))
PY"#,
        logs_dir = linux_shell_quote(&logs_dir),
        owner = linux_sudo_user_arg(&owner),
        log_name = linux_shell_quote(&clean_log_name),
        server_id = linux_shell_quote(server_id),
    );
    let output = run_ssh_capture(connection, &command)?;
    serde_json::from_str(output.stdout.trim()).map_err(|error| {
        format!(
            "Could not parse remote server log file: {error}\n\n{}",
            join_command_output(&[output.stdout.as_str(), output.stderr.as_str()])
        )
    })
}

fn remote_server_profile_path_or_default(path: &str) -> String {
    let trimmed = path.trim().trim_end_matches('/');

    if trimmed.is_empty() {
        return REMOTE_LINUX_SERVER_PROFILE_DIR.to_string();
    }

    let normalized = trimmed.to_ascii_lowercase();
    if normalized.ends_with("/server") {
        trimmed.to_string()
    } else if normalized.ends_with("/zomboid") {
        join_remote_unix_path(trimmed, "Server")
    } else {
        trimmed.to_string()
    }
}

fn remote_data_dir_from_server_profile_path(server_profile_path: &str) -> String {
    let profile_path = remote_server_profile_path_or_default(server_profile_path);
    let Some(zomboid_dir) = remote_unix_parent_path(&profile_path) else {
        return REMOTE_LINUX_DATA_DIR.to_string();
    };

    remote_unix_parent_path(&zomboid_dir).unwrap_or_else(|| REMOTE_LINUX_DATA_DIR.to_string())
}
fn matching_active_mod_ids(
    mod_item: &crate::models::ZomboidMod,
    active_mod_ids: &[String],
) -> Vec<String> {
    let mut mod_ids = vec![mod_item.id.to_lowercase()];
    mod_ids.extend(
        mod_item
            .variants
            .iter()
            .map(|variant| variant.id.to_lowercase()),
    );

    active_mod_ids
        .iter()
        .filter_map(|active_id| {
            let normalized = active_id.to_lowercase();
            mod_ids.contains(&normalized).then_some(normalized)
        })
        .collect()
}

fn normalize_remote_ram_gb(value: &str) -> Result<String, String> {
    let ram = value
        .trim()
        .replace(',', ".")
        .parse::<f64>()
        .map_err(|_| "Informe um valor valido de RAM.".to_string())?;

    if !ram.is_finite() || ram < 0.25 {
        return Err("A RAM precisa ser de pelo menos 0.25 GB.".to_string());
    }

    Ok(format!("{ram:.2}"))
}

fn remote_ram_gb_to_mb(value: &str) -> Result<u32, String> {
    let normalized = normalize_remote_ram_gb(value)?;
    let ram = normalized
        .parse::<f64>()
        .map_err(|_| "Informe um valor valido de RAM.".to_string())?;

    Ok((ram * 1024.0).round() as u32)
}

fn list_remote_zomboid_servers_impl(
    connection: RemoteServerConnectionRequest,
) -> Result<Vec<crate::models::ZomboidServer>, String> {
    let mut servers: Vec<crate::models::ZomboidServer> =
        run_remote_helper_json_with_sudo(&connection, "list-servers", Option::<&Value>::None)?;

    let host = connection.host.trim();
    let port_str = connection.port.trim();
    if !host.is_empty() {
        if let Ok(port) = port_str.parse::<u16>() {
            if let Ok(latency) = measure_remote_tcp_latency(host, port) {
                let latency_ms = latency.as_millis() as u64;
                for server in &mut servers {
                    server.ping_ms = Some(latency_ms);
                }
            }
        }
    }

    Ok(servers)
}

fn list_remote_zomboid_mods_impl(
    connection: RemoteServerConnectionRequest,
) -> Result<Vec<crate::models::ZomboidMod>, String> {
    let mut mods: Vec<crate::models::ZomboidMod> =
        run_remote_helper_json_with_sudo(&connection, "list-mods", Option::<&Value>::None)?;
    let mut seen_keys = HashSet::new();
    mods.retain(|mod_item| register_remote_mod_identity(&mut seen_keys, mod_item));

    mods.sort_by_key(|mod_item| mod_item.name.to_lowercase());
    hydrate_remote_mod_images(&connection, &mut mods);
    Ok(mods)
}

fn register_remote_mod_identity(
    seen_keys: &mut HashSet<String>,
    mod_item: &crate::models::ZomboidMod,
) -> bool {
    let identities = remote_mod_identity_keys(mod_item);
    if identities
        .iter()
        .any(|identity| seen_keys.contains(identity))
    {
        return false;
    }

    seen_keys.extend(identities);
    true
}

fn remote_mod_identity_keys(mod_item: &crate::models::ZomboidMod) -> Vec<String> {
    let mut identities = Vec::new();
    push_remote_mod_identity(&mut identities, "id", &mod_item.id);
    push_remote_mod_identity(&mut identities, "workshop", &mod_item.workshop_id);
    for variant in &mod_item.variants {
        push_remote_mod_identity(&mut identities, "id", &variant.id);
    }

    if identities.is_empty() {
        push_remote_mod_identity(&mut identities, "path", &mod_item.package_path);
    }

    identities.sort();
    identities.dedup();
    identities
}

fn push_remote_mod_identity(identities: &mut Vec<String>, prefix: &str, value: &str) {
    let normalized = value.trim().to_lowercase();
    if !normalized.is_empty() {
        identities.push(format!("{prefix}:{normalized}"));
    }
}

fn download_remote_steam_workshop_items_impl(
    app: &tauri::AppHandle,
    connection: RemoteServerConnectionRequest,
    workshop_ids: Vec<String>,
    force_validate: bool,
) -> Result<WorkshopDownloadResult, String> {
    let workshop_ids = dedupe_workshop_ids(workshop_ids);
    let total_items = workshop_ids.len();

    if workshop_ids.is_empty() {
        return Err(text(
            "Enter at least one Steam Workshop item to download.",
            "Informe ao menos um item da Steam Workshop para baixar.",
        )
        .to_string());
    }

    let config = get_remote_workspace_config_for_connection_impl(&connection)?
        .unwrap_or_else(default_remote_workspace_config);
    let steamcmd_path = required_field(&config.remote_steamcmd_path, "remote SteamCMD path")?;
    ensure_remote_steamcmd_workshop_dir_prepared(&connection)?;
    let steamcmd_workshop_dirs = remote_steam_workshop_dir_candidates(&connection)
        .into_iter()
        .map(|(_, path, _)| path)
        .collect::<Vec<_>>();

    let mut skipped_ids = Vec::new();
    let mut pending_ids = workshop_ids.clone();

    if !force_validate {
        let existing_ids =
            remote_existing_workshop_ids_with_mod_info(&connection, &steamcmd_workshop_dirs, &workshop_ids)?;
        skipped_ids = workshop_ids
            .iter()
            .filter(|workshop_id| existing_ids.contains(*workshop_id))
            .cloned()
            .collect();
        pending_ids = workshop_ids
            .iter()
            .filter(|workshop_id| !existing_ids.contains(*workshop_id))
            .cloned()
            .collect();
    }

    for workshop_id in &skipped_ids {
        emit_workshop_download_event(app, workshop_id, "skipped", None);
    }

    for workshop_id in &pending_ids {
        emit_workshop_download_event(app, workshop_id, "queued", None);
    }

    if pending_ids.is_empty() {
        return Ok(WorkshopDownloadResult {
            total_items,
            downloaded_items: 0,
            skipped_items: skipped_ids.len(),
            failed_items: Vec::new(),
            cancelled_items: 0,
            was_cancelled: false,
        });
    }

    let mut completed_items = HashSet::new();
    let mut failed_items = Vec::new();

    for chunk in pending_ids.chunks(8) {
        let result = run_remote_steamcmd_workshop_chunk(
            app,
            &connection,
            &steamcmd_path,
            chunk,
            force_validate,
        )?;

        let output = join_command_output(&[result.stdout.as_str(), result.stderr.as_str()]);
        let failed_by_log = parse_remote_steamcmd_failed_items(&output, chunk);
        let completed_by_log = parse_remote_steamcmd_completed_items(&output, chunk);
        let installed_ids = remote_existing_workshop_ids_with_mod_info(
            &connection,
            &steamcmd_workshop_dirs,
            chunk,
        )?;

        for workshop_id in chunk {
            if let Some(error) = failed_by_log.get(workshop_id.as_str()) {
                emit_workshop_download_event(app, workshop_id, "failed", Some(error));
                failed_items.push(WorkshopDownloadFailedItem {
                    workshop_id: workshop_id.clone(),
                    name: workshop_id.clone(),
                    error: error.clone(),
                });
            } else if completed_by_log.contains(workshop_id) || installed_ids.contains(workshop_id)
            {
                completed_items.insert(workshop_id.clone());
                emit_workshop_download_event(app, workshop_id, "completed", None);
            } else {
                let error = if result.success {
                    "SteamCMD finished without installing this Workshop item.".to_string()
                } else {
                    output.clone()
                };
                emit_workshop_download_event(app, workshop_id, "failed", Some(&error));
                failed_items.push(WorkshopDownloadFailedItem {
                    workshop_id: workshop_id.clone(),
                    name: workshop_id.clone(),
                    error,
                });
            }
        }
    }

    Ok(WorkshopDownloadResult {
        total_items,
        downloaded_items: completed_items.len(),
        skipped_items: skipped_ids.len(),
        failed_items,
        cancelled_items: 0,
        was_cancelled: false,
    })
}

fn run_remote_steamcmd_workshop_chunk(
    app: &tauri::AppHandle,
    connection: &RemoteServerConnectionRequest,
    steamcmd_path: &str,
    workshop_ids: &[String],
    force_validate: bool,
) -> Result<TerminalCommandResult, String> {
    for workshop_id in workshop_ids {
        emit_workshop_download_event(app, workshop_id, "downloading", None);
    }

    let mut script_lines = vec![
        "@ShutdownOnFailedCommand 0".to_string(),
        "@NoPromptForPassword 1".to_string(),
        "login anonymous".to_string(),
    ];
    for workshop_id in workshop_ids {
        let validate = if force_validate { " validate" } else { "" };
        script_lines.push(format!(
            "workshop_download_item 108600 {workshop_id}{validate}"
        ));
    }
    script_lines.push("quit".to_string());
    let script_text = script_lines.join("\\n");
    let (remote_zomboid_dir, data_owner) =
        remote_zomboid_data_dir_and_owner_for_connection(connection)?;
    let remote_data_dir = remote_unix_parent_path(&remote_zomboid_dir)
        .unwrap_or_else(|| REMOTE_LINUX_DATA_DIR.to_string());
    validate_linux_username(&data_owner)?;
    let steam_root = join_remote_unix_path(&remote_data_dir, "Steam");
    let steam_apps = join_remote_unix_path(&steam_root, "steamapps");
    let home_steam_root = join_remote_unix_path(&remote_data_dir, ".local/share/Steam");
    let home_steam_apps = join_remote_unix_path(&home_steam_root, "steamapps");
    let command = format!(
        "set -e; steamcmd={}; data_owner={}; remote_data_dir={}; script_path=$(mktemp /tmp/pzmm-steamcmd-XXXXXX.txt); cleanup() {{ rm -f \"$script_path\"; }}; trap cleanup EXIT; if [ ! -x \"$steamcmd\" ] && ! command -v \"$steamcmd\" >/dev/null 2>&1; then echo \"SteamCMD not found: $steamcmd\" >&2; exit 1; fi; sudo -n id -u \"$data_owner\" >/dev/null; sudo -n -u \"$data_owner\" mkdir -p {} {} {} {}; sudo -n chown -R \"${data_owner}:${data_owner}\" {} {}; printf '%b\\n' {} > \"$script_path\"; chmod 0644 \"$script_path\"; sudo -n -u \"$data_owner\" env HOME=\"$remote_data_dir\" PZMM_DATA_DIR=\"$remote_data_dir\" \"$steamcmd\" +runscript \"$script_path\"",
        linux_shell_quote(steamcmd_path),
        linux_sudo_user_arg(&data_owner),
        linux_shell_quote(&remote_data_dir),
        linux_shell_quote(&steam_root),
        linux_shell_quote(&steam_apps),
        linux_shell_quote(&home_steam_root),
        linux_shell_quote(&home_steam_apps),
        linux_shell_quote(&steam_root),
        linux_shell_quote(&home_steam_root),
        linux_shell_quote(&script_text),
    );
    run_ssh_workshop_streaming(app, connection, &command)
}
fn remote_existing_workshop_ids_with_mod_info(
    connection: &RemoteServerConnectionRequest,
    workshop_dirs: &[String],
    workshop_ids: &[String],
) -> Result<HashSet<String>, String> {
    let mut script = String::from("set -e\n");
    for workshop_id in workshop_ids {
        let safe_workshop_id = safe_remote_cache_segment(workshop_id);
        script.push_str(&format!(
            "found=0; item_id={id}; for root in {roots}; do if sudo -n find \"$root/$item_id/mods\" -name mod.info -type f -print -quit 2>/dev/null | grep -q .; then found=1; break; fi; done; if [ \"$found\" = 1 ]; then printf 'PZMM_WORKSHOP_READY=%s\\n' \"$item_id\"; fi\n",
            roots = workshop_dirs
                .iter()
                .map(|path| linux_shell_quote(path))
                .collect::<Vec<_>>()
                .join(" "),
            id = linux_shell_quote(&safe_workshop_id),
        ));
    }

    let output = run_ssh_capture_raw(connection, &script)?;
    if !output.success {
        return Err(join_command_output(&[
            "Could not verify downloaded remote Workshop items.",
            output.stdout.as_str(),
            output.stderr.as_str(),
        ]));
    }

    Ok(output
        .stdout
        .lines()
        .filter_map(|line| line.trim().strip_prefix("PZMM_WORKSHOP_READY="))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect())
}

fn parse_remote_steamcmd_failed_items(
    output: &str,
    wanted_ids: &[String],
) -> HashMap<String, String> {
    let wanted = wanted_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut failed = HashMap::new();

    for line in output.lines() {
        let normalized = line.to_lowercase();
        if !normalized.contains("download item")
            || !(normalized.contains("error") || normalized.contains("failed"))
        {
            continue;
        }

        if let Some(workshop_id) = line
            .split(|char: char| !char.is_ascii_digit())
            .find(|value| wanted.contains(*value))
        {
            failed.insert(workshop_id.to_string(), line.trim().to_string());
        }
    }

    failed
}

fn parse_remote_steamcmd_completed_items(output: &str, wanted_ids: &[String]) -> HashSet<String> {
    let wanted = wanted_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut completed = HashSet::new();

    for line in output.lines() {
        let normalized = line.to_lowercase();
        if !(normalized.contains("success") || normalized.contains("downloaded item")) {
            continue;
        }

        if let Some(workshop_id) = line
            .split(|char: char| !char.is_ascii_digit())
            .find(|value| wanted.contains(*value))
        {
            completed.insert(workshop_id.to_string());
        }
    }

    completed
}
fn cancel_remote_steam_workshop_download_impl(
    connection: RemoteServerConnectionRequest,
) -> Result<(), String> {
    let _ = run_ssh_capture(&connection, "pkill -f steamcmd || true")?;
    Ok(())
}
fn run_remote_helper_json<T, P>(
    connection: &RemoteServerConnectionRequest,
    helper_command: &str,
    payload: Option<&P>,
) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
    P: Serialize + ?Sized,
{
    let helper_path = ensure_cached_remote_helper(connection)?;
    let encoded_payload = payload
        .map(|payload| {
            let json = serde_json::to_vec(payload)
                .map_err(|error| format!("Could not serialize helper payload: {error}"))?;
            Ok::<_, String>(base64::engine::general_purpose::STANDARD.encode(json))
        })
        .transpose()?;
    let command = match encoded_payload.as_ref() {
        Some(_) => format!(
            "{} {} -",
            remote_helper_command_prefix(&helper_path),
            linux_shell_quote(helper_command),
        ),
        None => format!(
            "{} {}",
            remote_helper_command_prefix(&helper_path),
            linux_shell_quote(helper_command),
        ),
    };
    let output = match encoded_payload {
        Some(encoded_payload) => run_ssh_capture_with_stdin(connection, &command, &encoded_payload),
        None => run_ssh_capture(connection, &command),
    };
    let output = match output {
        Ok(output) => output,
        Err(error) => {
            invalidate_remote_helper_cache(connection);
            return Err(error);
        }
    };
    let stdout = output.stdout.trim();

    if stdout.is_empty() {
        invalidate_remote_helper_cache(connection);
        let message = format!("pzmm Linux helper returned no JSON output for {helper_command}.");
        return Err(join_command_output(&[
            message.as_str(),
            "This usually means the remote helper is missing, outdated, or sudo rejected the command.",
            output.stderr.as_str(),
        ]));
    }

    serde_json::from_str::<T>(stdout).map_err(|error| {
        invalidate_remote_helper_cache(connection);
        let message =
            format!("Could not parse pzmm Linux helper JSON output for {helper_command}: {error}");
        join_command_output(&[message.as_str(), stdout, output.stderr.as_str()])
    })
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteServerTestRequest<'a> {
    server_id: &'a str,
    server_launch_path: Option<&'a str>,
    server_profile_path: Option<&'a str>,
    no_steam: bool,
    follow_from_end: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteHelperServerTestEvent {
    event: String,
    timeout_seconds: Option<u64>,
    line: Option<String>,
    result: Option<ServerTestResult>,
    error: Option<String>,
}

fn run_remote_zomboid_server_start_streaming(
    app: &tauri::AppHandle,
    connection: &RemoteServerConnectionRequest,
    server_id: &str,
    server_launch_path: &str,
    no_steam: bool,
) -> Result<(), String> {
    let helper_path = ensure_cached_remote_helper(connection)?;
    let server_profile_path = connection.server_path.trim();
    let payload = RemoteServerTestRequest {
        server_id,
        server_launch_path: Some(server_launch_path),
        server_profile_path: (!server_profile_path.is_empty()).then_some(server_profile_path),
        no_steam,
        follow_from_end: None,
    };
    let json = serde_json::to_vec(&payload)
        .map_err(|error| format!("Could not serialize remote server start payload: {error}"))?;
    let encoded_payload = base64::engine::general_purpose::STANDARD.encode(json);
    let command = format!(
        "{} start-server-streaming -",
        remote_helper_sudo_command_prefix(connection, &helper_path),
    );

    stream_remote_server_event_command(
        app,
        connection,
        server_id,
        &command,
        &encoded_payload,
        "remote-server-start-event",
        "remote Linux server start",
    )
    .inspect_err(|_| {
        invalidate_remote_helper_cache(connection);
    })
}
fn run_remote_zomboid_server_logs_streaming(
    app: &tauri::AppHandle,
    connection: &RemoteServerConnectionRequest,
    server_id: &str,
    follow_from_end: bool,
) -> Result<(), String> {
    let helper_path = ensure_cached_remote_helper(connection)?;
    let payload = RemoteServerTestRequest {
        server_id,
        server_launch_path: None,
        server_profile_path: None,
        no_steam: false,
        follow_from_end: Some(follow_from_end),
    };
    let json = serde_json::to_vec(&payload)
        .map_err(|error| format!("Could not serialize remote server logs payload: {error}"))?;
    let encoded_payload = base64::engine::general_purpose::STANDARD.encode(json);
    let command = format!(
        "{} stream-server-logs -",
        remote_helper_sudo_command_prefix(connection, &helper_path),
    );

    stream_remote_server_event_command(
        app,
        connection,
        server_id,
        &command,
        &encoded_payload,
        "remote-server-start-event",
        "remote Linux server log stream",
    )
    .inspect_err(|_| {
        invalidate_remote_helper_cache(connection);
    })
}
fn run_remote_zomboid_server_test_streaming(
    app: &tauri::AppHandle,
    connection: &RemoteServerConnectionRequest,
    server_id: &str,
    server_launch_path: &str,
) -> Result<(), String> {
    let helper_path = ensure_cached_remote_helper(connection)?;
    let server_profile_path = connection.server_path.trim();
    let payload = RemoteServerTestRequest {
        server_id,
        server_launch_path: Some(server_launch_path),
        server_profile_path: (!server_profile_path.is_empty()).then_some(server_profile_path),
        no_steam: false,
        follow_from_end: None,
    };
    let json = serde_json::to_vec(&payload)
        .map_err(|error| format!("Could not serialize remote server test payload: {error}"))?;
    let encoded_payload = base64::engine::general_purpose::STANDARD.encode(json);
    let command = format!(
        "{} test-server -",
        remote_helper_sudo_command_prefix(connection, &helper_path)
    );

    stream_remote_server_event_command(
        app,
        connection,
        server_id,
        &command,
        &encoded_payload,
        "server-test-event",
        "remote Linux server test",
    )
    .inspect_err(|_| {
        invalidate_remote_helper_cache(connection);
    })
}
fn stream_remote_server_event_command(
    app: &tauri::AppHandle,
    connection: &RemoteServerConnectionRequest,
    server_id: &str,
    command_text: &str,
    stdin_text: &str,
    event_name: &str,
    action_label: &str,
) -> Result<(), String> {
    let host = required_field(&connection.host, "host")?;
    let username = required_field(&connection.username, "SSH username")?;
    let port = connection
        .port
        .trim()
        .parse::<u16>()
        .map_err(|_| "Enter a valid remote port.".to_string())?;

    if connection.auth_method.trim() != "key" {
        return Err(
            "Remote server testing currently requires SSH private key authentication.".to_string(),
        );
    }

    let key_path = PathBuf::from(required_field(&connection.ssh_key_path, "SSH key file")?);
    if !key_path.is_file() {
        return Err(format!("SSH key file not found: {}.", key_path.display()));
    }

    let remote = format!("{username}@{host}");
    let mut ssh_command = Command::new(ssh_command_name());
    append_ssh_command_args(&mut ssh_command, connection, &key_path, port)?;
    let mut child = hide_command_window(&mut ssh_command)
        .args([&remote, command_text])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Could not run ssh: {error}"))?;

    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Could not open remote server command stdin.".to_string())?;
        stdin
            .write_all(stdin_text.as_bytes())
            .map_err(|error| format!("Could not write remote server command stdin: {error}"))?;
    }

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Could not capture remote server command stdout.".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Could not capture remote server command stderr.".to_string())?;
    let (sender, receiver) = mpsc::channel::<(&'static str, String)>();
    let stdout_sender = sender.clone();

    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let _ = stdout_sender.send(("stdout", line));
        }
    });

    thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let _ = sender.send(("stderr", line));
        }
    });

    loop {
        match receiver.recv_timeout(Duration::from_millis(120)) {
            Ok((stream, line)) => {
                if stream == "stdout" {
                    emit_remote_server_event_stdout(app, event_name, server_id, &line);
                } else {
                    emit_remote_server_event_line(
                        app,
                        event_name,
                        server_id,
                        &format!("[ERR] {line}"),
                    );
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Some(status) = child
                    .try_wait()
                    .map_err(|error| format!("Could not read {action_label} status: {error}"))?
                {
                    while let Ok((stream, line)) = receiver.try_recv() {
                        if stream == "stdout" {
                            emit_remote_server_event_stdout(app, event_name, server_id, &line);
                        } else {
                            emit_remote_server_event_line(
                                app,
                                event_name,
                                server_id,
                                &format!("[ERR] {line}"),
                            );
                        }
                    }

                    if status.success() {
                        return Ok(());
                    }

                    return Err(format!("{} command failed: {}.", action_label, status));
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let status = child
                    .wait()
                    .map_err(|error| format!("Could not wait for {action_label}: {error}"))?;

                if status.success() {
                    return Ok(());
                }

                return Err(format!("{} command failed: {}.", action_label, status));
            }
        }
    }
}

fn emit_remote_server_event_stdout(
    app: &tauri::AppHandle,
    event_name: &str,
    server_id: &str,
    line: &str,
) {
    match serde_json::from_str::<RemoteHelperServerTestEvent>(line) {
        Ok(event) => {
            let _ = app.emit(
                event_name,
                ServerTestEvent {
                    server_id: server_id.to_string(),
                    event: event.event,
                    timeout_seconds: event.timeout_seconds,
                    line: event.line,
                    result: event.result,
                    error: event.error,
                },
            );
        }
        Err(_) => {
            emit_remote_server_event_line(app, event_name, server_id, &format!("[OUT] {line}"))
        }
    }
}

fn emit_remote_server_event_line(
    app: &tauri::AppHandle,
    event_name: &str,
    server_id: &str,
    line: &str,
) {
    let _ = app.emit(
        event_name,
        ServerTestEvent {
            server_id: server_id.to_string(),
            event: "line".to_string(),
            timeout_seconds: None,
            line: Some(line.to_string()),
            result: None,
            error: None,
        },
    );
}

fn hydrate_remote_mod_images(
    connection: &RemoteServerConnectionRequest,
    mods: &mut [crate::models::ZomboidMod],
) {
    hydrate_remote_mod_images_with_cache(connection, mods)
}

fn hydrate_remote_mod_images_with_cache(
    connection: &RemoteServerConnectionRequest,
    mods: &mut [crate::models::ZomboidMod],
) {
    let mut resolved_paths: HashMap<String, PathBuf> = HashMap::new();
    let mut missing_paths = Vec::new();

    for mod_item in mods.iter() {
        let Some(remote_image_path) = mod_item
            .image_url
            .as_deref()
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .filter(|path| is_remote_file_image_path(path))
            .map(ToOwned::to_owned)
        else {
            continue;
        };

        if resolved_paths.contains_key(&remote_image_path)
            || missing_paths
                .iter()
                .any(|path: &String| path.eq_ignore_ascii_case(&remote_image_path))
        {
            continue;
        }

        match remote_image_cache_path(connection, &remote_image_path) {
            Ok(cache_path) if is_valid_cached_remote_image(&cache_path) => {
                resolved_paths.insert(remote_image_path, cache_path);
            }
            Ok(_) => missing_paths.push(remote_image_path),
            Err(_) => {}
        }
    }

    for chunk in missing_paths.chunks(4) {
        let handles = chunk
            .iter()
            .cloned()
            .map(|remote_path| {
                let connection = connection.clone();
                thread::spawn(move || {
                    ensure_cached_remote_file(&connection, &remote_path)
                        .map(|local_path| (remote_path, local_path))
                })
            })
            .collect::<Vec<_>>();

        for handle in handles {
            if let Ok(Ok((remote_path, local_path))) = handle.join() {
                resolved_paths.insert(remote_path, local_path);
            }
        }
    }

    for mod_item in mods {
        let Some(remote_image_path) = mod_item
            .image_url
            .as_deref()
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(ToOwned::to_owned)
        else {
            continue;
        };

        if !is_remote_file_image_path(&remote_image_path) {
            continue;
        }

        mod_item.image_url = resolved_paths
            .get(&remote_image_path)
            .map(|local_path| local_path.display().to_string());
    }
}

fn is_remote_file_image_path(path: &str) -> bool {
    let normalized = path.trim().to_lowercase();

    if normalized.starts_with("http://")
        || normalized.starts_with("https://")
        || normalized.starts_with("data:")
        || normalized.starts_with("asset:")
        || normalized.starts_with("blob:")
    {
        return false;
    }

    looks_like_linux_path(path)
        || looks_like_windows_path(path)
        || normalized.contains(":\\")
        || normalized.contains(":/")
}

fn ensure_cached_remote_file(
    connection: &RemoteServerConnectionRequest,
    remote_path: &str,
) -> Result<PathBuf, String> {
    let cache_path = remote_image_cache_path(connection, remote_path)?;

    if is_valid_cached_remote_image(&cache_path) {
        return Ok(cache_path);
    }

    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create remote image cache: {error}"))?;
    }

    download_remote_file(connection, remote_path, &cache_path)?;
    Ok(cache_path)
}

fn is_valid_cached_remote_image(path: &Path) -> bool {
    fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.len() > 0)
        .unwrap_or(false)
}

fn remote_image_cache_path(
    connection: &RemoteServerConnectionRequest,
    remote_path: &str,
) -> Result<PathBuf, String> {
    let mut hasher = DefaultHasher::new();
    remote_path.to_lowercase().hash(&mut hasher);
    let path_hash = hasher.finish();
    let remote_path_buf = PathBuf::from(remote_path);
    let extension = remote_path_buf
        .extension()
        .and_then(|extension| extension.to_str())
        .filter(|extension| !extension.trim().is_empty())
        .unwrap_or("img")
        .to_string();

    Ok(app_config_dir()?.join("remote-image-cache").join(format!(
        "{}-{path_hash:016x}.{extension}",
        remote_image_cache_prefix(connection)
    )))
}

fn clear_remote_image_cache(connection: &RemoteServerConnectionRequest) -> Result<(), String> {
    let cache_root = app_config_dir()?.join("remote-image-cache");

    if !cache_root.is_dir() {
        return Ok(());
    }

    let prefix = remote_image_cache_prefix(connection);
    for entry in fs::read_dir(&cache_root)
        .map_err(|error| format!("Could not read remote image cache: {error}"))?
    {
        let path = entry
            .map_err(|error| format!("Could not read remote image cache entry: {error}"))?
            .path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };

        if file_name.starts_with(&prefix) {
            let _ = fs::remove_file(path);
        }
    }

    Ok(())
}

fn remote_image_cache_prefix(connection: &RemoteServerConnectionRequest) -> String {
    let mut hasher = DefaultHasher::new();
    connection.host.to_lowercase().hash(&mut hasher);
    connection.username.to_lowercase().hash(&mut hasher);

    format!("{:016x}", hasher.finish())
}

fn download_remote_file(
    connection: &RemoteServerConnectionRequest,
    remote_path: &str,
    local_path: &PathBuf,
) -> Result<(), String> {
    let host = required_field(&connection.host, "host")?;
    let username = required_field(&connection.username, "SSH username")?;
    let port = connection
        .port
        .trim()
        .parse::<u16>()
        .map_err(|_| "Enter a valid remote port.".to_string())?;
    let key_path = PathBuf::from(required_field(&connection.ssh_key_path, "SSH key file")?);

    if !key_path.is_file() {
        return Err(format!("SSH key file not found: {}.", key_path.display()));
    }

    let remote = format!(
        "{username}@{host}:\"{}\"",
        remote_path.replace('\\', "/").replace('"', "\\\"")
    );
    let mut scp_command = Command::new(scp_command_name());
    append_scp_command_args(&mut scp_command, connection, &key_path, port)?;
    let local_path_arg = scp_local_path_arg(local_path);
    let output = hide_command_window(&mut scp_command)
        .arg(&remote)
        .arg(&local_path_arg)
        .output()
        .map_err(|error| format!("Could not run scp for remote file: {error}"))?;

    if output.status.success() {
        return Ok(());
    }

    download_remote_file_via_base64(connection, remote_path, local_path).map_err(|fallback_error| {
        join_command_output(&[
            "Could not download remote file.",
            String::from_utf8_lossy(&output.stdout).as_ref(),
            String::from_utf8_lossy(&output.stderr).as_ref(),
            fallback_error.as_str(),
        ])
    })
}

fn download_remote_file_via_base64(
    connection: &RemoteServerConnectionRequest,
    remote_path: &str,
    local_path: &PathBuf,
) -> Result<(), String> {
    let command = format!(
        "set -e; sudo -n test -f {}; sudo -n base64 -w 0 {}",
        linux_shell_quote(remote_path),
        linux_shell_quote(remote_path)
    );
    let output = run_ssh_capture_raw(connection, &command)?;

    if !output.success {
        return Err(join_command_output(&[
            "Could not read remote file with sudo.",
            output.stdout.as_str(),
            output.stderr.as_str(),
        ]));
    }

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(output.stdout.trim().as_bytes())
        .map_err(|error| format!("Could not decode remote file bytes: {error}"))?;

    fs::write(local_path, bytes).map_err(|error| {
        format!(
            "Could not write cached remote file {}: {error}",
            local_path.display()
        )
    })
}
fn ensure_cached_remote_helper(
    connection: &RemoteServerConnectionRequest,
) -> Result<String, String> {
    let cache_key = remote_helper_connection_cache_key(connection);
    let remote_path = REMOTE_LINUX_HELPER_PATH.to_string();
    let cache = VERIFIED_REMOTE_HELPERS.get_or_init(|| Mutex::new(HashSet::new()));

    if cache
        .lock()
        .map_err(|_| "Could not lock remote helper cache.".to_string())?
        .contains(&cache_key)
    {
        return Ok(remote_path);
    }

    let helper_path = ensure_remote_helper(connection)?;
    cache
        .lock()
        .map_err(|_| "Could not lock remote helper cache.".to_string())?
        .insert(cache_key);

    Ok(helper_path)
}
fn invalidate_remote_helper_cache(connection: &RemoteServerConnectionRequest) {
    let cache_key = remote_helper_connection_cache_key(connection);
    if let Some(cache) = VERIFIED_REMOTE_HELPERS.get() {
        if let Ok(mut cache) = cache.lock() {
            cache.remove(&cache_key);
        }
    }
}

fn remote_helper_connection_cache_key(connection: &RemoteServerConnectionRequest) -> String {
    format!(
        "{}|{}|{}|{}",
        connection.host.trim().to_lowercase(),
        connection.port.trim(),
        connection.username.trim().to_lowercase(),
        REMOTE_LINUX_HELPER_PATH,
    )
}

fn ensure_remote_helper(connection: &RemoteServerConnectionRequest) -> Result<String, String> {
    if validate_existing_remote_helper(connection)? {
        return Ok(REMOTE_LINUX_HELPER_PATH.to_string());
    }

    let result = setup_remote_helper_impl(None, connection)?;

    if result.success {
        Ok(result.remote_path)
    } else {
        Err(join_command_output(&[
            "Could not prepare the remote Linux helper component.",
            result.stdout.as_str(),
            result.stderr.as_str(),
        ]))
    }
}

fn validate_existing_remote_helper(
    connection: &RemoteServerConnectionRequest,
) -> Result<bool, String> {
    let command = format!(
        "test -x {helper_path} && {helper_command} --version >/dev/null",
        helper_path = linux_shell_quote(REMOTE_LINUX_HELPER_PATH),
        helper_command = remote_helper_command_prefix(REMOTE_LINUX_HELPER_PATH),
    );

    run_ssh_capture_raw(connection, &command).map(|result| result.success)
}

fn setup_remote_helper_impl(
    app: Option<&tauri::AppHandle>,
    connection: &RemoteServerConnectionRequest,
) -> Result<RemoteHelperSetupResult, String> {
    if connection.auth_method.trim() != "key" {
        return Err(
            "Remote Linux helper setup requires SSH private key authentication.".to_string(),
        );
    }

    validate_authentication(connection)?;
    let local_helper_path = local_helper_binary_path_for_setup()?;
    let remote_path = REMOTE_LINUX_HELPER_PATH.to_string();
    let home_dir = if connection.username.trim() == "root" {
        "/root".to_string()
    } else {
        format!("/home/{}", connection.username.trim())
    };
    let upload_path = format!("{}/pzmm-helper-upload-{}", home_dir, std::process::id());
    emit_optional_remote_setup_log(
        app,
        "helper",
        "info",
        "Checking Linux remote prerequisites (sudo, systemd).",
    );
    let prereq_command = "set -e; uname -s; test -d /run/systemd/system; command -v sudo >/dev/null; sudo -n true; sudo -n useradd --system --create-home --home-dir /var/lib/pzmm --shell /usr/sbin/nologin pzmm 2>/dev/null || true; sudo -n install -d -o pzmm -g pzmm /opt/pzmm /var/lib/pzmm /var/lib/pzmm/cache /var/lib/pzmm/Zomboid /var/lib/pzmm/Zomboid/Server /var/lib/pzmm/Zomboid/mods /var/lib/pzmm/steamcmd /var/lib/pzmm/zomboid-server; sudo -n chown -R pzmm:pzmm /var/lib/pzmm/cache /var/lib/pzmm/Zomboid; printf 'PZMM_LINUX_HELPER_PREREQS_READY\\n'";
    let prereq = run_ssh_capture(connection, prereq_command)?;
    emit_optional_remote_setup_output(app, "helper", "stdout", &prereq.stdout);
    emit_optional_remote_setup_output(app, "helper", "stderr", &prereq.stderr);

    emit_optional_remote_setup_log(
        app,
        "helper",
        "info",
        &format!(
            "Uploading Rust helper {} to {remote_path}",
            local_helper_path.display()
        ),
    );
    let upload_result =
        upload_helper_binary_to_remote(connection, &local_helper_path, &upload_path);
    if let Err(error) = upload_result {
        return Ok(RemoteHelperSetupResult {
            local_path: local_helper_path.display().to_string(),
            remote_path,
            command: format!(
                "{prereq_command}\nscp {} {upload_path}",
                local_helper_path.display()
            ),
            exit_code: None,
            success: false,
            stdout: prereq.stdout,
            stderr: join_command_output(&[prereq.stderr.as_str(), error.as_str()]),
        });
    }

    let install_command = format!(
        "set -e; sudo -n install -d -o root -g root {helper_dir}; if [ ! -f {upload_path} ]; then echo 'Uploaded helper file not found at {upload_path_raw}' >&2; if [ -x {remote_path} ]; then echo 'Existing helper is present; validating it instead.' >&2; {helper_command} --version; exit 0; fi; exit 1; fi; sudo -n install -m 0755 -o root -g root {upload_path} {remote_path}; rm -f {upload_path}; {helper_command} --version",
        helper_dir = linux_shell_quote(REMOTE_LINUX_HELPER_DIR),
        upload_path = linux_shell_quote(&upload_path),
        upload_path_raw = upload_path,
        remote_path = linux_shell_quote(REMOTE_LINUX_HELPER_PATH),
        helper_command = remote_helper_command_prefix(REMOTE_LINUX_HELPER_PATH),
    );
    let install = run_ssh_capture_raw(connection, &install_command)?;
    emit_optional_remote_setup_output(app, "helper", "stdout", &install.stdout);
    emit_optional_remote_setup_output(app, "helper", "stderr", &install.stderr);

    let success = install.success;
    if success {
        mark_remote_setup_completed_step(connection, 1)?;
        emit_optional_remote_setup_log(
            app,
            "helper",
            "info",
            "Remote Rust helper setup completed.",
        );
    } else {
        emit_optional_remote_setup_log(app, "helper", "stderr", "Remote Rust helper setup failed.");
    }

    Ok(RemoteHelperSetupResult {
        local_path: local_helper_path.display().to_string(),
        remote_path,
        command: format!(
            "{prereq_command}\nscp {} {upload_path}\n{install_command}",
            local_helper_path.display()
        ),
        exit_code: install.exit_code,
        success,
        stdout: join_command_output(&[prereq.stdout.as_str(), install.stdout.as_str()]),
        stderr: join_command_output(&[prereq.stderr.as_str(), install.stderr.as_str()]),
    })
}

fn local_helper_binary_path_for_setup() -> Result<PathBuf, String> {
    if pzmm_dev_mode_enabled() {
        return local_dev_helper_binary_path();
    }

    remove_cached_release_helper_binary()?;
    download_release_helper_binary()
}

fn local_dev_helper_binary_path() -> Result<PathBuf, String> {
    let mut invalid_candidates = Vec::new();
    let mut searched_candidates = Vec::new();

    for candidate in local_helper_binary_candidates() {
        searched_candidates.push(candidate.display().to_string());

        if !candidate.is_file() {
            continue;
        }

        match validate_linux_helper_binary(&candidate) {
            Ok(()) => {
                return candidate.canonicalize().map_err(|error| {
                    format!("Could not canonicalize helper binary path: {error}")
                });
            }
            Err(error) => invalid_candidates.push(format!("{}: {error}", candidate.display())),
        }
    }

    let mut details = vec![
        "PZMM_DEV is enabled, but no valid local Linux helper was found.".to_string(),
        "Build the helper on Linux with `npm run build:helper:release`, copy the extensionless ELF helper into src-tauri/target/release, or unset PZMM_DEV to use the GitHub release download.".to_string(),
    ];

    if !invalid_candidates.is_empty() {
        details.push(format!(
            "Invalid local helper candidate(s):\n{}",
            invalid_candidates.join("\n")
        ));
    }

    details.push(format!(
        "Searched local helper path(s):\n{}",
        searched_candidates.join("\n")
    ));

    Err(details.join("\n"))
}

fn local_helper_binary_candidates() -> Vec<PathBuf> {
    let binary_names = vec!["pzmm-helper-linux-x86_64", "pzmm-helper"];
    let mut candidates = Vec::new();
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    for binary_name in &binary_names {
        candidates.push(
            manifest_dir
                .join("target")
                .join("release")
                .join(binary_name),
        );
    }

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(current_dir) = current_exe.parent() {
            for binary_name in &binary_names {
                candidates.push(current_dir.join(binary_name));
                candidates.push(current_dir.join("..").join(binary_name));
            }
        }
    }

    for binary_name in binary_names {
        candidates.push(manifest_dir.join("target").join("debug").join(binary_name));
    }

    candidates
}

fn pzmm_dev_mode_enabled() -> bool {
    std::env::var("PZMM_DEV")
        .ok()
        .or_else(|| pzmm_env_file_value("PZMM_DEV"))
        .map(|value| env_truthy(&value))
        .unwrap_or(false)
}

fn pzmm_env_file_value(key: &str) -> Option<String> {
    for env_file in pzmm_env_file_candidates() {
        let Ok(content) = fs::read_to_string(env_file) else {
            continue;
        };

        for line in content.lines() {
            let line = line.trim();

            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let Some((current_key, value)) = line.split_once('=') else {
                continue;
            };

            if current_key.trim() == key {
                return Some(clean_env_value(value));
            }
        }
    }

    None
}

fn pzmm_env_file_candidates() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Ok(current_dir) = std::env::current_dir() {
        roots.push(current_dir);
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(repo_root) = manifest_dir.parent() {
        roots.push(repo_root.to_path_buf());
    }
    roots.push(manifest_dir);

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(current_dir) = current_exe.parent() {
            roots.push(current_dir.to_path_buf());
        }
    }

    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    for root in roots {
        for file_name in [".env.local", ".env"] {
            let candidate = root.join(file_name);
            if seen.insert(candidate.clone()) {
                candidates.push(candidate);
            }
        }
    }

    candidates
}

fn clean_env_value(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

fn env_truthy(value: &str) -> bool {
    let normalized = value.trim().to_lowercase();
    !matches!(normalized.as_str(), "" | "0" | "false" | "no" | "off")
}

fn download_release_helper_binary() -> Result<PathBuf, String> {
    let helper_dir = release_helper_cache_dir()?;
    fs::create_dir_all(&helper_dir).map_err(|error| {
        format!(
            "Could not create helper cache folder {}: {error}",
            helper_dir.display()
        )
    })?;

    let helper_path = helper_dir.join("pzmm-helper");
    if helper_path.is_file()
        && fs::metadata(&helper_path)
            .map(|metadata| metadata.len())
            .unwrap_or(0)
            > 0
    {
        if validate_linux_helper_binary(&helper_path).is_ok() {
            make_helper_executable(&helper_path)?;
            return helper_path
                .canonicalize()
                .map_err(|error| format!("Could not canonicalize cached helper path: {error}"));
        }

        let _ = fs::remove_file(&helper_path);
    }

    let version = env!("CARGO_PKG_VERSION");
    let tag = format!("v{version}");
    let asset_names = release_helper_asset_names();
    let mut errors = Vec::new();

    for asset_name in asset_names {
        let url = format!(
            "https://github.com/{}/releases/download/{}/{}",
            HELPER_RELEASE_REPOSITORY, tag, asset_name
        );
        let temp_path = helper_dir.join(format!("{asset_name}.download"));
        let mut command = Command::new(curl_command_name());
        let output = hide_command_window(&mut command)
            .args(["-fL", "--retry", "2", "--connect-timeout", "15", "-o"])
            .arg(&temp_path)
            .arg(&url)
            .output();

        match output {
            Ok(output) if output.status.success() => {
                if fs::metadata(&temp_path)
                    .map(|metadata| metadata.len())
                    .unwrap_or(0)
                    == 0
                {
                    let _ = fs::remove_file(&temp_path);
                    errors.push(format!("{url}: downloaded file was empty"));
                    continue;
                }
                validate_linux_helper_binary(&temp_path)?;
                fs::rename(&temp_path, &helper_path).map_err(|error| {
                    format!(
                        "Could not store downloaded helper at {}: {error}",
                        helper_path.display()
                    )
                })?;
                make_helper_executable(&helper_path)?;
                return helper_path.canonicalize().map_err(|error| {
                    format!("Could not canonicalize downloaded helper path: {error}")
                });
            }
            Ok(output) => {
                let _ = fs::remove_file(&temp_path);
                errors.push(format!(
                    "{url}: curl exited with status {}. {}{}",
                    output.status,
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            Err(error) => {
                let _ = fs::remove_file(&temp_path);
                errors.push(format!("{url}: could not run curl: {error}"));
            }
        }
    }

    Err(format!(
        "pzmm-helper binary was not found locally and could not be downloaded from release {tag}. Expected one of these release assets: {}. For development, run `npm run build:helper`. Download attempts:\n{}",
        release_helper_asset_names().join(", "),
        errors.join("\n")
    ))
}

fn release_helper_cache_dir() -> Result<PathBuf, String> {
    Ok(app_config_dir()?
        .join("helpers")
        .join(format!("v{}", env!("CARGO_PKG_VERSION"))))
}

fn remove_cached_release_helper_binary() -> Result<(), String> {
    let helper_dir = release_helper_cache_dir()?;

    for file_name in ["pzmm-helper", "pzmm-helper.download"] {
        let path = helper_dir.join(file_name);
        if path.exists() {
            fs::remove_file(&path).map_err(|error| {
                format!(
                    "Could not remove cached helper {} before refresh: {error}",
                    path.display()
                )
            })?;
        }
    }

    for asset_name in release_helper_asset_names() {
        let path = helper_dir.join(format!("{asset_name}.download"));
        if path.exists() {
            fs::remove_file(&path).map_err(|error| {
                format!(
                    "Could not remove cached helper download {} before refresh: {error}",
                    path.display()
                )
            })?;
        }
    }

    Ok(())
}

fn release_helper_asset_names() -> Vec<&'static str> {
    vec![
        "pzmm-helper-linux-x86_64",
        "pzmm-helper-x86_64-unknown-linux-gnu",
        "pzmm-helper",
    ]
}

fn validate_linux_helper_binary(path: &Path) -> Result<(), String> {
    let mut file = fs::File::open(path).map_err(|error| {
        format!(
            "Could not open Linux helper candidate {}: {error}",
            path.display()
        )
    })?;
    let mut magic = [0_u8; 4];
    file.read_exact(&mut magic).map_err(|error| {
        format!(
            "Could not read Linux helper candidate {}: {error}",
            path.display()
        )
    })?;

    if magic == [0x7f, b'E', b'L', b'F'] {
        return Ok(());
    }

    if magic[0] == b'M' && magic[1] == b'Z' {
        return Err("found a Windows executable, expected a Linux ELF binary".to_string());
    }

    Err("expected a Linux ELF helper binary".to_string())
}

fn make_helper_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)
            .map_err(|error| format!("Could not inspect helper permissions: {error}"))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)
            .map_err(|error| format!("Could not make helper executable: {error}"))?;
    }

    #[cfg(not(unix))]
    {
        let _ = path;
    }

    Ok(())
}

fn upload_helper_binary_to_remote(
    connection: &RemoteServerConnectionRequest,
    local_path: &Path,
    remote_path: &str,
) -> Result<(), String> {
    upload_bundle_to_remote(connection, local_path, remote_path)
}

fn scp_local_path_arg(path: &Path) -> String {
    let path = path.display().to_string();

    #[cfg(windows)]
    {
        if let Some(stripped) = path.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{stripped}");
        }

        if let Some(stripped) = path.strip_prefix(r"\\?\") {
            return stripped.to_string();
        }
    }

    path
}

fn remote_helper_environment_prefix() -> String {
    format!(
        "PZMM_DATA_DIR={} PZMM_SERVER_PROFILE_DIR={} PZMM_SERVER_LAUNCH_PATH={}",
        linux_shell_quote(REMOTE_LINUX_DATA_DIR),
        linux_shell_quote(REMOTE_LINUX_SERVER_PROFILE_DIR),
        linux_shell_quote(REMOTE_LINUX_ZOMBOID_LAUNCHER),
    )
}

fn remote_helper_command_prefix(helper_path: &str) -> String {
    format!(
        "{} {}",
        remote_helper_environment_prefix(),
        linux_shell_quote(helper_path)
    )
}

fn remote_helper_sudo_command_prefix(
    connection: &RemoteServerConnectionRequest,
    helper_path: &str,
) -> String {
    let saved_config = get_remote_workspace_config_for_connection_impl(connection)
        .ok()
        .flatten();
    let server_profile_source = saved_config
        .as_ref()
        .map(|config| config.server_path.as_str())
        .unwrap_or(connection.server_path.as_str());
    let server_profile_dir = remote_server_profile_path_or_default(server_profile_source);
    let remote_data_dir = remote_data_dir_from_server_profile_path(&server_profile_dir);
    let remote_zomboid_dir = remote_unix_parent_path(&server_profile_dir)
        .unwrap_or_else(|| join_remote_unix_path(&remote_data_dir, "Zomboid"));
    let data_owner_value = saved_config
        .as_ref()
        .map(|config| config.remote_zomboid_data_owner.as_str())
        .unwrap_or(REMOTE_LINUX_MANAGED_USER);
    let server_owner_value = saved_config
        .as_ref()
        .map(|config| config.remote_zomboid_server_owner.as_str())
        .unwrap_or(REMOTE_LINUX_MANAGED_USER);
    format!(
        r#"set -e
server_profile_dir={server_profile_dir}
remote_data_dir={remote_data_dir}
remote_zomboid_dir={remote_zomboid_dir}
managed_zomboid_dir={managed_zomboid_dir}
managed_profile_dir={managed_profile_dir}
data_owner={data_owner}
server_owner={server_owner}
ssh_user={ssh_user}
if [ "$remote_zomboid_dir" != "$managed_zomboid_dir" ] && ! sudo -n test -e "$remote_zomboid_dir"; then
  for candidate_user in "$data_owner" "$server_owner" "$ssh_user"; do
    case "$candidate_user" in ''|UNKNOWN|-*|*[!A-Za-z0-9_-]*|pzmm) continue ;; esac
    candidate_home=$(getent passwd "$candidate_user" | cut -d: -f6 || true)
    candidate_zomboid="$candidate_home/Zomboid"
    if [ -n "$candidate_home" ] && sudo -n test -d "$candidate_zomboid/Server"; then
      remote_zomboid_dir="$candidate_zomboid"
      server_profile_dir="$candidate_zomboid/Server"
      remote_data_dir="$candidate_home"
      data_owner=$(sudo -n stat -c '%U' "$remote_zomboid_dir")
      break
    fi
  done
fi
if [ "$remote_zomboid_dir" != "$managed_zomboid_dir" ] && ! sudo -n test -e "$remote_zomboid_dir"; then
  found_profile=$(sudo -n find /home -mindepth 3 -maxdepth 3 -type d -path '*/Zomboid/Server' -print -quit 2>/dev/null || true)
  if [ -n "$found_profile" ]; then
    server_profile_dir="$found_profile"
    remote_zomboid_dir=$(dirname "$server_profile_dir")
    remote_data_dir=$(dirname "$remote_zomboid_dir")
    data_owner=$(sudo -n stat -c '%U' "$remote_zomboid_dir")
  fi
fi
if {{ [ -z "$data_owner" ] || [ "$data_owner" = {managed_user} ]; }} && [ "$remote_zomboid_dir" != "$managed_zomboid_dir" ] && sudo -n test -e "$remote_zomboid_dir"; then
  data_owner=$(sudo -n stat -c '%U' "$remote_zomboid_dir")
fi
case "$data_owner" in ''|UNKNOWN|-*|*[!A-Za-z0-9_-]*) echo "Invalid Linux owner for $remote_zomboid_dir: $data_owner" >&2; exit 1 ;; esac
sudo -n id -u "$data_owner" >/dev/null
if [ "$remote_zomboid_dir" = "$managed_zomboid_dir" ]; then
  cache_dir={managed_cache_dir}
  sudo -n install -d -o pzmm -g pzmm {managed_data_dir} "$cache_dir" "$managed_zomboid_dir" "$managed_profile_dir"
else
  if ! sudo -n test -d "$remote_zomboid_dir"; then echo "Remote Zomboid data folder not found: $remote_zomboid_dir" >&2; exit 1; fi
  cache_dir="$remote_zomboid_dir/.pzmm-cache"
  sudo -n -u "$data_owner" mkdir -p "$cache_dir" "$server_profile_dir"
fi
sudo -n -u "$data_owner" env HOME="$remote_data_dir" PZMM_DATA_DIR="$remote_data_dir" PZMM_CACHE_DIR="$cache_dir" PZMM_SERVER_PROFILE_DIR="$server_profile_dir" PZMM_SERVER_LAUNCH_PATH={launch_path} PZMM_EXTRA_STEAM_WORKSHOP_DIRS={extra_workshop_dirs} {helper_path}"#,
        server_profile_dir = linux_shell_quote(&server_profile_dir),
        remote_data_dir = linux_shell_quote(&remote_data_dir),
        remote_zomboid_dir = linux_shell_quote(&remote_zomboid_dir),
        managed_zomboid_dir =
            linux_shell_quote(&join_remote_unix_path(REMOTE_LINUX_DATA_DIR, "Zomboid")),
        managed_profile_dir = linux_shell_quote(REMOTE_LINUX_SERVER_PROFILE_DIR),
        data_owner = linux_sudo_user_arg(data_owner_value),
        server_owner = linux_sudo_user_arg(server_owner_value),
        ssh_user = linux_shell_quote(&connection.username),
        managed_user = linux_shell_quote(REMOTE_LINUX_MANAGED_USER),
        managed_cache_dir =
            linux_shell_quote(&join_remote_unix_path(REMOTE_LINUX_DATA_DIR, "cache")),
        managed_data_dir = linux_shell_quote(REMOTE_LINUX_DATA_DIR),
        launch_path = linux_shell_quote(REMOTE_LINUX_ZOMBOID_LAUNCHER),
        extra_workshop_dirs = linux_shell_quote(&remote_extra_steam_workshop_dirs(connection)),
        helper_path = linux_shell_quote(helper_path),
    )
}
fn append_ssh_common_args(
    command: &mut Command,
    _connection: &RemoteServerConnectionRequest,
    key_path: &Path,
) -> Result<(), String> {
    command
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "StrictHostKeyChecking=accept-new",
            "-o",
            "ControlMaster=no",
            "-o",
            "ConnectionAttempts=2",
            "-o",
            "IdentitiesOnly=yes",
            "-i",
        ])
        .arg(key_path);

    Ok(())
}

fn ssh_command_name() -> &'static str {
    if cfg!(windows) {
        "ssh.exe"
    } else {
        "ssh"
    }
}

fn scp_command_name() -> &'static str {
    if cfg!(windows) {
        "scp.exe"
    } else {
        "scp"
    }
}

fn ssh_keygen_command_name() -> &'static str {
    if cfg!(windows) {
        "ssh-keygen.exe"
    } else {
        "ssh-keygen"
    }
}

fn curl_command_name() -> &'static str {
    if cfg!(windows) {
        "curl.exe"
    } else {
        "curl"
    }
}

fn append_ssh_command_args(
    command: &mut Command,
    connection: &RemoteServerConnectionRequest,
    key_path: &Path,
    port: u16,
) -> Result<(), String> {
    append_ssh_common_args(command, connection, key_path)?;
    command.args(["-p", &port.to_string()]);
    Ok(())
}

fn append_scp_command_args(
    command: &mut Command,
    connection: &RemoteServerConnectionRequest,
    key_path: &Path,
    port: u16,
) -> Result<(), String> {
    append_ssh_common_args(command, connection, key_path)?;
    command.args(["-P", &port.to_string()]);
    Ok(())
}
pub(crate) fn run_ssh_capture(
    connection: &RemoteServerConnectionRequest,
    command_text: &str,
) -> Result<TerminalCommandResult, String> {
    SshCommandRunner {
        connection: connection.clone(),
    }
    .run(command_text)
    .and_then(|result| {
        if result.success {
            Ok(result)
        } else {
            Err(format!(
                "Remote command failed.\n\n{}",
                join_command_output(&[result.stdout.as_str(), result.stderr.as_str()])
            ))
        }
    })
}

fn run_ssh_capture_raw(
    connection: &RemoteServerConnectionRequest,
    command_text: &str,
) -> Result<TerminalCommandResult, String> {
    SshCommandRunner {
        connection: connection.clone(),
    }
    .run(command_text)
}

fn run_ssh_capture_with_stdin(
    connection: &RemoteServerConnectionRequest,
    command_text: &str,
    stdin_text: &str,
) -> Result<TerminalCommandResult, String> {
    let result = run_ssh_with_stdin(connection, command_text, stdin_text)?;

    if result.success {
        Ok(result)
    } else {
        Err(format!(
            "Remote command failed.\n\n{}",
            join_command_output(&[result.stdout.as_str(), result.stderr.as_str()])
        ))
    }
}

fn run_ssh_with_stdin(
    connection: &RemoteServerConnectionRequest,
    command_text: &str,
    stdin_text: &str,
) -> Result<TerminalCommandResult, String> {
    let host = required_field(&connection.host, "host")?;
    let username = required_field(&connection.username, "SSH username")?;
    let port = connection
        .port
        .trim()
        .parse::<u16>()
        .map_err(|_| "Enter a valid remote port.".to_string())?;

    if connection.auth_method.trim() != "key" {
        return Err(
            "Remote command execution currently requires SSH private key authentication."
                .to_string(),
        );
    }

    let key_path = PathBuf::from(required_field(&connection.ssh_key_path, "SSH key file")?);
    if !key_path.is_file() {
        return Err(format!("SSH key file not found: {}.", key_path.display()));
    }

    let remote = format!("{username}@{host}");
    let mut ssh_command = Command::new(ssh_command_name());
    append_ssh_command_args(&mut ssh_command, connection, &key_path, port)?;
    let mut child = hide_command_window(&mut ssh_command)
        .args([&remote, command_text])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Could not run ssh: {error}"))?;

    {
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Could not open remote command stdin.".to_string())?;
        let mut stdin = stdin;
        stdin
            .write_all(stdin_text.as_bytes())
            .map_err(|error| format!("Could not write remote command stdin: {error}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|error| format!("Could not wait for ssh: {error}"))?;

    Ok(command_result("remote", command_text, output))
}

fn run_ssh_streaming(
    app: &tauri::AppHandle,
    connection: &RemoteServerConnectionRequest,
    command_text: &str,
    phase: &str,
) -> Result<TerminalCommandResult, String> {
    let host = required_field(&connection.host, "host")?;
    let username = required_field(&connection.username, "SSH username")?;
    let port = connection
        .port
        .trim()
        .parse::<u16>()
        .map_err(|_| "Enter a valid remote port.".to_string())?;

    if connection.auth_method.trim() != "key" {
        return Err(
            "Remote command execution currently requires SSH private key authentication."
                .to_string(),
        );
    }

    let key_path = PathBuf::from(required_field(&connection.ssh_key_path, "SSH key file")?);
    if !key_path.is_file() {
        return Err(format!("SSH key file not found: {}.", key_path.display()));
    }

    let remote = format!("{username}@{host}");
    let mut ssh_command = Command::new(ssh_command_name());
    append_ssh_command_args(&mut ssh_command, connection, &key_path, port)?;
    let mut child = hide_command_window(&mut ssh_command)
        .args([&remote, command_text])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Could not run ssh: {error}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Could not capture remote command stdout.".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Could not capture remote command stderr.".to_string())?;
    let (sender, receiver) = mpsc::channel::<(&'static str, String)>();
    let stdout_sender = sender.clone();

    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let _ = stdout_sender.send(("stdout", line));
        }
    });

    thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let _ = sender.send(("stderr", line));
        }
    });

    let mut stdout_lines = Vec::new();
    let mut stderr_lines = Vec::new();

    loop {
        match receiver.recv_timeout(Duration::from_millis(120)) {
            Ok((stream, line)) => {
                emit_remote_setup_log(app, phase, stream, &line);
                if stream == "stdout" {
                    stdout_lines.push(line);
                } else {
                    stderr_lines.push(line);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Some(status) = child
                    .try_wait()
                    .map_err(|error| format!("Could not read remote command status: {error}"))?
                {
                    while let Ok((stream, line)) = receiver.try_recv() {
                        emit_remote_setup_log(app, phase, stream, &line);
                        if stream == "stdout" {
                            stdout_lines.push(line);
                        } else {
                            stderr_lines.push(line);
                        }
                    }

                    return Ok(TerminalCommandResult {
                        target: "remote".to_string(),
                        command: command_text.to_string(),
                        exit_code: status.code(),
                        success: status.success(),
                        stdout: stdout_lines.join("\n"),
                        stderr: stderr_lines.join("\n"),
                    });
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let status = child
                    .wait()
                    .map_err(|error| format!("Could not wait for remote command: {error}"))?;
                return Ok(TerminalCommandResult {
                    target: "remote".to_string(),
                    command: command_text.to_string(),
                    exit_code: status.code(),
                    success: status.success(),
                    stdout: stdout_lines.join("\n"),
                    stderr: stderr_lines.join("\n"),
                });
            }
        }
    }
}

fn run_ssh_workshop_streaming(
    app: &tauri::AppHandle,
    connection: &RemoteServerConnectionRequest,
    command_text: &str,
) -> Result<TerminalCommandResult, String> {
    let host = required_field(&connection.host, "host")?;
    let username = required_field(&connection.username, "SSH username")?;
    let port = connection
        .port
        .trim()
        .parse::<u16>()
        .map_err(|_| "Enter a valid remote port.".to_string())?;

    if connection.auth_method.trim() != "key" {
        return Err(
            "Remote command execution currently requires SSH private key authentication."
                .to_string(),
        );
    }

    let key_path = PathBuf::from(required_field(&connection.ssh_key_path, "SSH key file")?);
    if !key_path.is_file() {
        return Err(format!("SSH key file not found: {}.", key_path.display()));
    }

    let remote = format!("{username}@{host}");
    let mut ssh_command = Command::new(ssh_command_name());
    append_ssh_command_args(&mut ssh_command, connection, &key_path, port)?;
    let mut child = hide_command_window(&mut ssh_command)
        .args([&remote, command_text])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Could not run ssh: {error}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Could not capture remote command stdout.".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Could not capture remote command stderr.".to_string())?;
    let (sender, receiver) = mpsc::channel::<(&'static str, String)>();
    let stdout_sender = sender.clone();

    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let _ = stdout_sender.send(("stdout", line));
        }
    });

    thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let _ = sender.send(("stderr", line));
        }
    });

    let mut stdout_lines = Vec::new();
    let mut stderr_lines = Vec::new();

    loop {
        match receiver.recv_timeout(Duration::from_millis(120)) {
            Ok((stream, line)) => {
                emit_workshop_log_line(app, 1, stream, &line);
                if stream == "stdout" {
                    stdout_lines.push(line);
                } else {
                    stderr_lines.push(line);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Some(status) = child
                    .try_wait()
                    .map_err(|error| format!("Could not read remote command status: {error}"))?
                {
                    while let Ok((stream, line)) = receiver.try_recv() {
                        emit_workshop_log_line(app, 1, stream, &line);
                        if stream == "stdout" {
                            stdout_lines.push(line);
                        } else {
                            stderr_lines.push(line);
                        }
                    }

                    return Ok(TerminalCommandResult {
                        target: "remote".to_string(),
                        command: command_text.to_string(),
                        exit_code: status.code(),
                        success: status.success(),
                        stdout: stdout_lines.join("\n"),
                        stderr: stderr_lines.join("\n"),
                    });
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let status = child
                    .wait()
                    .map_err(|error| format!("Could not wait for remote command: {error}"))?;
                return Ok(TerminalCommandResult {
                    target: "remote".to_string(),
                    command: command_text.to_string(),
                    exit_code: status.code(),
                    success: status.success(),
                    stdout: stdout_lines.join("\n"),
                    stderr: stderr_lines.join("\n"),
                });
            }
        }
    }
}

fn parse_remote_json_array<T>(stdout: &str) -> Result<Vec<T>, String>
where
    T: serde::de::DeserializeOwned,
{
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    match serde_json::from_str::<Vec<T>>(trimmed) {
        Ok(values) => Ok(values),
        Err(_) => serde_json::from_str::<T>(trimmed)
            .map(|value| vec![value])
            .map_err(|error| format!("Could not parse remote JSON output: {error}\n\n{trimmed}")),
    }
}

fn verify_ssh_key_authentication(
    connection: &RemoteServerConnectionRequest,
    port: u16,
) -> Result<String, String> {
    let host = required_field(&connection.host, "host")?;
    let username = required_field(&connection.username, "SSH username")?;
    let key_path = PathBuf::from(required_field(&connection.ssh_key_path, "SSH key file")?);

    if !key_path.is_file() {
        return Err(format!("SSH key file not found: {}.", key_path.display()));
    }

    let remote = format!("{username}@{host}");
    let remote_command = "echo pzmm-ready";
    let command_display =
        ssh_connection_test_command_display(&key_path, port, &remote, remote_command);
    let mut ssh_command = Command::new(ssh_command_name());
    append_simple_ssh_connection_args(&mut ssh_command, &key_path, port);
    let output = hide_command_window(&mut ssh_command)
        .args([&remote, remote_command])
        .output()
        .map_err(|error| format!("Could not run ssh: {error}\n\n[COMMAND]\n{command_display}"))?;

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let diagnostic_log = ssh_connection_test_diagnostic_log(
        &command_display,
        output.status.code(),
        &stdout,
        &stderr,
    );

    if output.status.success() {
        return Ok(diagnostic_log);
    }

    let details = join_command_output(&[stdout.as_str(), stderr.as_str()]);

    if details.contains("UNPROTECTED PRIVATE KEY FILE")
        || details.contains("bad permissions")
        || details.contains("Permissions for")
    {
        return Err(format!(
            "{}\n\n{}\n\n{}\n{}",
            "SSH refused this private key because its file permissions are too open.",
            "Fix the key permissions, then try connecting again:",
            ssh_key_permissions_fix_command(&key_path),
            diagnostic_log
        ));
    }

    Err(format!(
        "SSH authentication failed. Check the username, key file, and the server authorized_keys file.\n\n{diagnostic_log}"
    ))
}
fn append_simple_ssh_connection_args(command: &mut Command, key_path: &Path, port: u16) {
    command
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "StrictHostKeyChecking=accept-new",
            "-o",
            "ControlMaster=no",
            "-o",
            "ConnectionAttempts=2",
            "-o",
            "IdentitiesOnly=yes",
            "-i",
        ])
        .arg(key_path);
    if port != 22 {
        command.args(["-p", &port.to_string()]);
    }
}

fn ssh_connection_test_command_display(
    key_path: &Path,
    port: u16,
    remote: &str,
    remote_command: &str,
) -> String {
    let mut parts = vec![
        ssh_command_name().to_string(),
        "-o".to_string(),
        "BatchMode=yes".to_string(),
        "-o".to_string(),
        "ConnectTimeout=10".to_string(),
        "-o".to_string(),
        "StrictHostKeyChecking=accept-new".to_string(),
        "-o".to_string(),
        "ControlMaster=no".to_string(),
        "-o".to_string(),
        "ConnectionAttempts=2".to_string(),
        "-o".to_string(),
        "IdentitiesOnly=yes".to_string(),
        "-i".to_string(),
        shell_quote(&key_path.display().to_string()),
    ];
    if port != 22 {
        parts.push("-p".to_string());
        parts.push(port.to_string());
    }
    parts.push(shell_quote(remote));
    parts.push(shell_quote(remote_command));
    parts.join(" ")
}
fn ssh_connection_test_diagnostic_log(
    command_display: &str,
    exit_code: Option<i32>,
    stdout: &str,
    stderr: &str,
) -> String {
    format!(
        "[COMMAND]\n{}\n\n[EXIT CODE]\n{}\n\n[STDOUT]\n{}\n\n[STDERR]\n{}",
        command_display,
        exit_code
            .map(|code| code.to_string())
            .unwrap_or_else(|| "terminated by signal".to_string()),
        if stdout.trim().is_empty() {
            "<empty>"
        } else {
            stdout.trim_end()
        },
        if stderr.trim().is_empty() {
            "<empty>"
        } else {
            stderr.trim_end()
        },
    )
}

fn shell_quote(value: &str) -> String {
    if value.chars().all(|ch| {
        ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | '/' | ':' | '@' | '\\')
    }) {
        value.to_string()
    } else {
        format!("\"{}\"", value.replace('"', "\\\""))
    }
}
fn fix_ssh_key_permissions_impl(ssh_key_path: &str) -> Result<String, String> {
    let key_path = PathBuf::from(required_field(ssh_key_path, "SSH key file")?);

    if !key_path.is_file() {
        return Err(format!("SSH key file not found: {}.", key_path.display()));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(&key_path)
            .map_err(|error| format!("Could not read SSH key permissions: {error}"))?
            .permissions();
        permissions.set_mode(0o600);
        fs::set_permissions(&key_path, permissions)
            .map_err(|error| format!("Could not update SSH key permissions: {error}"))?;

        return Ok(format!(
            "SSH key permissions fixed with chmod 600: {}",
            key_path.display()
        ));
    }

    #[cfg(windows)]
    {
        let script = ssh_key_permissions_fix_command(&key_path);
        let mut command = Command::new("powershell.exe");
        let output = hide_command_window(&mut command)
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &script,
            ])
            .output()
            .map_err(|error| {
                format!("Could not run PowerShell to fix SSH key permissions: {error}")
            })?;

        if output.status.success() {
            return Ok(format!(
                "SSH key permissions fixed with icacls: {}",
                key_path.display()
            ));
        }

        return Err(join_command_output(&[
            "Could not fix SSH key permissions.",
            String::from_utf8_lossy(&output.stdout).as_ref(),
            String::from_utf8_lossy(&output.stderr).as_ref(),
        ]));
    }

    #[allow(unreachable_code)]
    Err("Automatic SSH key permission fix is not supported on this platform.".to_string())
}

fn ssh_key_permissions_fix_command(key_path: &Path) -> String {
    let key_path = key_path.display().to_string();

    if cfg!(windows) {
        format!(
            "icacls \"{key_path}\" /inheritance:r\nicacls \"{key_path}\" /remove \"Users\" \"Authenticated Users\" \"Everyone\" \"CodexSandboxUsers\"\nicacls \"{key_path}\" /grant:r \"${{env:USERNAME}}:R\""
        )
    } else {
        format!("chmod 600 {}", linux_shell_quote(&key_path))
    }
}

fn run_terminal_command_impl(
    request: TerminalCommandRequest,
) -> Result<TerminalCommandResult, String> {
    let command_text = required_field(&request.command, "command")?;
    let runner = command_runner_for(request)?;

    runner.run(&command_text)
}

struct LocalCommandRunner {
    working_directory: String,
}

struct SshCommandRunner {
    connection: RemoteServerConnectionRequest,
}

trait TerminalCommandRunner {
    fn target(&self) -> &'static str;
    fn run(&self, command_text: &str) -> Result<TerminalCommandResult, String>;
}

fn command_runner_for(
    request: TerminalCommandRequest,
) -> Result<Box<dyn TerminalCommandRunner>, String> {
    match request.target.trim() {
        "local" => Ok(Box::new(LocalCommandRunner {
            working_directory: request.working_directory,
        })),
        "remote" => Ok(Box::new(SshCommandRunner {
            connection: request
                .connection
                .ok_or_else(|| "Configure the remote SSH connection first.".to_string())?,
        })),
        _ => Err("Choose a valid terminal target.".to_string()),
    }
}

impl TerminalCommandRunner for LocalCommandRunner {
    fn target(&self) -> &'static str {
        "local"
    }

    fn run(&self, command_text: &str) -> Result<TerminalCommandResult, String> {
        let working_directory = self.working_directory.trim();
        let working_directory = if working_directory.is_empty() {
            None
        } else {
            let working_directory = PathBuf::from(working_directory);
            if !working_directory.is_dir() {
                return Err(format!(
                    "Local working directory not found: {}.",
                    working_directory.display()
                ));
            }
            Some(working_directory)
        };

        let output = run_shell_command(command_text, working_directory.as_deref())?;

        Ok(command_result(self.target(), command_text, output))
    }
}

impl TerminalCommandRunner for SshCommandRunner {
    fn target(&self) -> &'static str {
        "remote"
    }

    fn run(&self, command_text: &str) -> Result<TerminalCommandResult, String> {
        let host = required_field(&self.connection.host, "host")?;
        let username = required_field(&self.connection.username, "SSH username")?;
        let port = self
            .connection
            .port
            .trim()
            .parse::<u16>()
            .map_err(|_| "Enter a valid remote port.".to_string())?;

        if self.connection.auth_method.trim() != "key" {
            return Err(
                "Remote command execution currently requires SSH private key authentication."
                    .to_string(),
            );
        }

        let key_path = required_field(&self.connection.ssh_key_path, "SSH key file")?;
        let key_path = PathBuf::from(key_path);
        if !key_path.is_file() {
            return Err(format!("SSH key file not found: {}.", key_path.display()));
        }

        let remote = format!("{username}@{host}");
        let mut ssh_command = Command::new(ssh_command_name());
        append_ssh_command_args(&mut ssh_command, &self.connection, &key_path, port)?;
        let output = hide_command_window(&mut ssh_command)
            .args([&remote, command_text])
            .output()
            .map_err(|error| format!("Could not run ssh: {error}"))?;

        Ok(command_result(self.target(), command_text, output))
    }
}

fn command_result(target: &str, command_text: &str, output: Output) -> TerminalCommandResult {
    TerminalCommandResult {
        target: target.to_string(),
        command: command_text.to_string(),
        exit_code: output.status.code(),
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    }
}

fn verify_remote_steamcmd_available_impl(
    connection: RemoteServerConnectionRequest,
) -> Result<RemoteSteamCmdUploadResult, String> {
    if connection.auth_method.trim() != "key" {
        return Err(
            "Remote Linux SteamCMD verification requires SSH private key authentication."
                .to_string(),
        );
    }

    validate_authentication(&connection)?;
    let existing_config = get_remote_workspace_config_for_connection_impl(&connection)?
        .unwrap_or_else(default_remote_workspace_config);
    let saved_steamcmd_path = existing_config.remote_steamcmd_path.trim().to_string();
    let managed_steamcmd_path = join_remote_unix_path(REMOTE_LINUX_STEAMCMD_DIR, "steamcmd.sh");
    let command = format!(
        r#"set -e
saved_steamcmd={saved_steamcmd}
managed_steamcmd={managed_steamcmd}
steamcmd_path=""
if [ -n "$saved_steamcmd" ] && [ -x "$saved_steamcmd" ]; then
  steamcmd_path="$saved_steamcmd"
elif [ -x "$managed_steamcmd" ]; then
  steamcmd_path="$managed_steamcmd"
else
  steamcmd_path=$(command -v steamcmd 2>/dev/null || which steamcmd 2>/dev/null || true)
fi
if [ -z "$steamcmd_path" ]; then
  echo 'SteamCMD was not found in the saved app-managed path or in PATH. Install SteamCMD with the app-managed install first.' >&2
  exit 127
fi
printf 'PZMM_STEAMCMD_PATH=%s
' "$steamcmd_path"
"#,
        saved_steamcmd = linux_shell_quote(&saved_steamcmd_path),
        managed_steamcmd = linux_shell_quote(&managed_steamcmd_path),
    );
    let result = run_ssh_capture_raw(&connection, &command)?;
    let steamcmd_executable_path = result
        .stdout
        .lines()
        .find_map(|line| line.trim().strip_prefix("PZMM_STEAMCMD_PATH="))
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_default();
    let remote_path = remote_unix_parent_path(&steamcmd_executable_path)
        .unwrap_or_else(|| steamcmd_executable_path.clone());
    let setup_result = RemoteSteamCmdUploadResult {
        local_path: "remote saved path or PATH".to_string(),
        remote_path,
        steamcmd_executable_path: steamcmd_executable_path.clone(),
        command,
        exit_code: result.exit_code,
        success: result.success && !steamcmd_executable_path.trim().is_empty(),
        stdout: result.stdout,
        stderr: result.stderr,
    };

    if setup_result.success {
        write_remote_workspace_config(&RemoteWorkspaceConfig {
            name: connection.name,
            host: connection.host,
            port: connection.port,
            username: connection.username,
            auth_method: connection.auth_method,
            ssh_key_path: connection.ssh_key_path,
            server_path: connection.server_path,
            remote_steamcmd_dir: existing_config.remote_steamcmd_dir,
            remote_steamcmd_path: steamcmd_executable_path,
            remote_zomboid_server_dir: existing_config.remote_zomboid_server_dir,
            remote_zomboid_server_path: existing_config.remote_zomboid_server_path,
            remote_zomboid_server_owner: existing_config.remote_zomboid_server_owner,
            remote_zomboid_data_owner: existing_config.remote_zomboid_data_owner,
            remote_client_ram: existing_config.remote_client_ram,
            remote_server_ram: existing_config.remote_server_ram,
            remote_setup_completed_step: existing_config.remote_setup_completed_step.max(2),
            remote_mod_locations: existing_config.remote_mod_locations,
        })?;
    }

    Ok(setup_result)
}

fn upload_steamcmd_to_remote_impl(
    app: &tauri::AppHandle,
    request: RemoteSteamCmdUploadRequest,
) -> Result<RemoteSteamCmdUploadResult, String> {
    let connection = request.connection;
    let existing_config = get_remote_workspace_config_for_connection_impl(&connection)?
        .unwrap_or_else(default_remote_workspace_config);
    let connection_for_config = connection.clone();
    if connection.auth_method.trim() != "key" {
        return Err(
            "Remote Linux SteamCMD setup requires SSH private key authentication.".to_string(),
        );
    }

    validate_authentication(&connection)?;
    let remote_directory = if request.remote_directory.trim().is_empty() {
        REMOTE_LINUX_STEAMCMD_DIR.to_string()
    } else {
        required_field(&request.remote_directory, "remote SteamCMD folder")?
    };

    if !looks_like_linux_path(&remote_directory) {
        return Err(
            "Use an absolute Linux SteamCMD folder, for example /var/lib/pzmm/steamcmd."
                .to_string(),
        );
    }

    emit_remote_setup_log(
        app,
        "steamcmd",
        "info",
        "Installing SteamCMD on the Linux remote host.",
    );
    let script = format!(
        r#"set -e
steamcmd_dir={steamcmd_dir}
steamcmd_path="$steamcmd_dir/steamcmd.sh"
temp_archive=/tmp/pzmm-steamcmd-linux.tar.gz
managed_user={managed_user}
managed_data_dir={managed_data_dir}

sudo -n install -d -o "$managed_user" -g "$managed_user" "$steamcmd_dir" "$managed_data_dir"
sudo -n apt-get update
sudo -n env DEBIAN_FRONTEND=noninteractive apt-get install -y lib32gcc-s1 ca-certificates curl tar gzip

if [ ! -x "$steamcmd_path" ]; then
  curl -fsSL https://steamcdn-a.akamaihd.net/client/installer/steamcmd_linux.tar.gz -o "$temp_archive"
  sudo -n -u "$managed_user" tar -xzf "$temp_archive" -C "$steamcmd_dir"
  sudo -n chmod 0755 "$steamcmd_path"
fi

sudo -n chown -R "${managed_user}:${managed_user}" "$steamcmd_dir"
sudo -n -u "$managed_user" env HOME="$managed_data_dir" PZMM_DATA_DIR="$managed_data_dir" "$steamcmd_path" +quit >/dev/null
printf 'PZMM_STEAMCMD_PATH=%s\n' "$steamcmd_path"
"#,
        steamcmd_dir = linux_shell_quote(&remote_directory),
        managed_user = linux_shell_quote(REMOTE_LINUX_MANAGED_USER),
        managed_data_dir = linux_shell_quote(REMOTE_LINUX_DATA_DIR),
    );
    let result = run_ssh_streaming(app, &connection, &script, "steamcmd")?;
    let steamcmd_executable_path = result
        .stdout
        .lines()
        .find_map(|line| line.trim().strip_prefix("PZMM_STEAMCMD_PATH="))
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| join_remote_unix_path(&remote_directory, "steamcmd.sh"));

    let setup_result = RemoteSteamCmdUploadResult {
        local_path: "apt/curl remote install".to_string(),
        remote_path: remote_directory.clone(),
        steamcmd_executable_path: steamcmd_executable_path.clone(),
        command: script,
        exit_code: result.exit_code,
        success: result.success,
        stdout: result.stdout,
        stderr: result.stderr,
    };

    if setup_result.success {
        write_remote_workspace_config(&RemoteWorkspaceConfig {
            name: connection_for_config.name,
            host: connection_for_config.host,
            port: connection_for_config.port,
            username: connection_for_config.username,
            auth_method: connection_for_config.auth_method,
            ssh_key_path: connection_for_config.ssh_key_path,
            server_path: connection_for_config.server_path,
            remote_steamcmd_dir: remote_directory,
            remote_steamcmd_path: steamcmd_executable_path,
            remote_zomboid_server_dir: existing_config.remote_zomboid_server_dir,
            remote_zomboid_server_path: existing_config.remote_zomboid_server_path,
            remote_zomboid_server_owner: existing_config.remote_zomboid_server_owner,
            remote_zomboid_data_owner: existing_config.remote_zomboid_data_owner,
            remote_client_ram: existing_config.remote_client_ram,
            remote_server_ram: existing_config.remote_server_ram,
            remote_setup_completed_step: existing_config.remote_setup_completed_step.max(2),
            remote_mod_locations: existing_config.remote_mod_locations,
        })?;
        emit_remote_setup_log(app, "steamcmd", "info", "Linux SteamCMD setup completed.");
    } else {
        emit_remote_setup_log(app, "steamcmd", "stderr", "Linux SteamCMD setup failed.");
    }

    Ok(setup_result)
}
fn install_zomboid_server_on_remote_impl(
    app: &tauri::AppHandle,
    request: RemoteZomboidServerInstallRequest,
) -> Result<RemoteZomboidServerInstallResult, String> {
    let connection = request.connection;
    if connection.auth_method.trim() != "key" {
        return Err(
            "Remote Linux Project Zomboid installation requires SSH private key authentication."
                .to_string(),
        );
    }

    validate_authentication(&connection)?;
    let steamcmd_path = if request.steamcmd_path.trim().is_empty() {
        get_remote_workspace_config_for_connection_impl(&connection)?
            .and_then(|config| {
                let path = config.remote_steamcmd_path.trim().to_string();
                if path.is_empty() {
                    None
                } else {
                    Some(path)
                }
            })
            .unwrap_or_else(|| join_remote_unix_path(REMOTE_LINUX_STEAMCMD_DIR, "steamcmd.sh"))
    } else {
        required_field(&request.steamcmd_path, "remote SteamCMD path")?
    };
    let install_directory = if request.install_directory.trim().is_empty() {
        REMOTE_LINUX_ZOMBOID_SERVER_DIR.to_string()
    } else {
        required_field(
            &request.install_directory,
            "remote Project Zomboid server folder",
        )?
    };

    if !looks_like_linux_path(&install_directory) {
        return Err("Use an absolute Linux Project Zomboid server folder, for example /var/lib/pzmm/zomboid-server.".to_string());
    }

    let requested_branch = request
        .branch
        .as_deref()
        .unwrap_or("default")
        .trim()
        .to_ascii_lowercase();
    let (branch_label, steamcmd_branch_args) = match requested_branch.as_str() {
        "" | "default" | "public" | "stable" => ("default", ""),
        "unstable" | "latest-unstable" | "latest_unstable" => ("latest unstable", "-beta unstable"),
        _ => {
            return Err(
                "Unsupported Project Zomboid server branch. Choose default or unstable."
                    .to_string(),
            );
        }
    };
    let launcher_path = join_remote_unix_path(&install_directory, "start-server.sh");
    let script = format!(
        r#"set -e
steamcmd={steamcmd}
install_dir={install_dir}
managed_user={managed_user}
managed_data_dir={managed_data_dir}
if [ ! -x "$steamcmd" ] && ! command -v "$steamcmd" >/dev/null 2>&1; then
  echo "SteamCMD not found: $steamcmd" >&2
  exit 1
fi

if [ -f "$install_dir/start-server.sh" ]; then
  sudo -n chmod +x "$install_dir/start-server.sh" || true
  echo "Existing Project Zomboid dedicated server install found. Skipping SteamCMD download."
  printf 'PZMM_SERVER_PATH=%s\n' "$install_dir/start-server.sh"
  exit 0
fi

sudo -n install -d -o "$managed_user" -g "$managed_user" "$install_dir" "$managed_data_dir"
echo "PZMM_STEAMCMD_APP_UPDATE={branch_label}"
sudo -n -u "$managed_user" env HOME="$managed_data_dir" PZMM_DATA_DIR="$managed_data_dir" "$steamcmd" +force_install_dir "$install_dir" +login anonymous +app_update 380870 {branch_args} validate +quit
if [ ! -f "$install_dir/start-server.sh" ]; then
  echo "Linux launcher not found after install: $install_dir/start-server.sh" >&2
  exit 1
fi
sudo -n chmod +x "$install_dir/start-server.sh"
printf 'PZMM_SERVER_PATH=%s\n' "$install_dir/start-server.sh"
"#,
        steamcmd = linux_shell_quote(&steamcmd_path),
        install_dir = linux_shell_quote(&install_directory),
        managed_user = linux_shell_quote(REMOTE_LINUX_MANAGED_USER),
        managed_data_dir = linux_shell_quote(REMOTE_LINUX_DATA_DIR),
        branch_args = steamcmd_branch_args,
    );
    let install_message = format!(
        "Checking for an existing Project Zomboid dedicated server install before downloading ({branch_label} branch)."
    );
    emit_remote_setup_log(app, "zomboid-server", "info", &install_message);
    let result = run_ssh_streaming(app, &connection, &script, "zomboid-server")?;
    let server_executable_path =
        extract_remote_server_path(&result.stdout).unwrap_or_else(|| launcher_path.clone());
    let install_result = RemoteZomboidServerInstallResult {
        install_directory: install_directory.clone(),
        server_executable_path: server_executable_path.clone(),
        command: script,
        exit_code: result.exit_code,
        success: result.success,
        stdout: result.stdout,
        stderr: result.stderr,
    };

    if install_result.success {
        let connection_for_helper = connection.clone();
        let existing_config = get_remote_workspace_config_for_connection_impl(&connection)?
            .unwrap_or_else(default_remote_workspace_config);
        let server_owner = detect_remote_path_owner(&connection, &server_executable_path)
            .unwrap_or_else(|_| REMOTE_LINUX_MANAGED_USER.to_string());
        let managed_zomboid_dir = join_remote_unix_path(REMOTE_LINUX_DATA_DIR, "Zomboid");
        let data_owner = detect_remote_path_owner(&connection, &managed_zomboid_dir)
            .unwrap_or_else(|_| REMOTE_LINUX_MANAGED_USER.to_string());
        write_remote_workspace_config(&RemoteWorkspaceConfig {
            name: connection.name,
            host: connection.host,
            port: connection.port,
            username: connection.username,
            auth_method: connection.auth_method,
            ssh_key_path: connection.ssh_key_path,
            server_path: REMOTE_LINUX_SERVER_PROFILE_DIR.to_string(),
            remote_steamcmd_dir: existing_config.remote_steamcmd_dir,
            remote_steamcmd_path: steamcmd_path,
            remote_zomboid_server_dir: install_directory,
            remote_zomboid_server_path: server_executable_path,
            remote_zomboid_server_owner: server_owner,
            remote_zomboid_data_owner: data_owner,
            remote_client_ram: existing_config.remote_client_ram,
            remote_server_ram: existing_config.remote_server_ram,
            remote_setup_completed_step: existing_config.remote_setup_completed_step.max(3),
            remote_mod_locations: existing_config.remote_mod_locations,
        })?;
        let _ = run_remote_helper_json::<RemoteServerActionResult, _>(
            &connection_for_helper,
            "configure-server-firewall",
            Some(&serde_json::json!({ "serverId": "servertest" })),
        );
        emit_remote_setup_log(
            app,
            "zomboid-server",
            "info",
            "Project Zomboid Linux server path saved.",
        );
    }

    Ok(install_result)
}
fn extract_remote_server_path(stdout: &str) -> Option<String> {
    stdout
        .lines()
        .find_map(|line| line.trim().strip_prefix("PZMM_SERVER_PATH="))
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(ToOwned::to_owned)
}

fn dedupe_workshop_ids(workshop_ids: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();

    for workshop_id in workshop_ids {
        if seen.insert(workshop_id.clone()) {
            deduped.push(workshop_id);
        }
    }

    deduped
}

fn emit_workshop_download_event(
    app: &tauri::AppHandle,
    workshop_id: &str,
    status: &str,
    error: Option<&str>,
) {
    let _ = app.emit(
        "workshop-download-event",
        WorkshopDownloadEvent {
            workshop_id: workshop_id.to_string(),
            name: workshop_id.to_string(),
            status: status.to_string(),
            error: error.map(ToOwned::to_owned),
        },
    );
}

fn emit_workshop_log_line(app: &tauri::AppHandle, instance_id: usize, stream: &str, line: &str) {
    let line = if stream == "stderr" {
        format!("[ERR] {line}")
    } else {
        format!("[OUT] {line}")
    };

    let _ = app.emit(
        "workshop-download-log",
        WorkshopDownloadLogEvent {
            instance_id,
            label: format!("Remote {instance_id}"),
            line,
            color_key: "cyan".to_string(),
        },
    );
}

fn default_remote_workspace_config() -> RemoteWorkspaceConfig {
    RemoteWorkspaceConfig {
        name: String::new(),
        host: String::new(),
        port: "22".to_string(),
        username: String::new(),
        auth_method: "key".to_string(),
        ssh_key_path: String::new(),
        server_path: REMOTE_LINUX_SERVER_PROFILE_DIR.to_string(),
        remote_steamcmd_dir: default_remote_steamcmd_dir(),
        remote_steamcmd_path: String::new(),
        remote_zomboid_server_dir: default_remote_zomboid_server_dir(),
        remote_zomboid_server_path: String::new(),
        remote_zomboid_server_owner: REMOTE_LINUX_MANAGED_USER.to_string(),
        remote_zomboid_data_owner: REMOTE_LINUX_MANAGED_USER.to_string(),
        remote_client_ram: "4.00".to_string(),
        remote_server_ram: "4.00".to_string(),
        remote_setup_completed_step: 0,
        remote_mod_locations: Vec::new(),
    }
}

fn default_remote_steamcmd_dir() -> String {
    REMOTE_LINUX_STEAMCMD_DIR.to_string()
}

fn default_remote_zomboid_server_dir() -> String {
    REMOTE_LINUX_ZOMBOID_SERVER_DIR.to_string()
}
fn normalize_legacy_remote_path(value: Option<String>) -> Option<String> {
    let value = value?.trim().to_string();

    if value.is_empty() || is_legacy_pzmanager_path(&value) {
        None
    } else {
        Some(value)
    }
}

fn is_legacy_pzmanager_path(value: &str) -> bool {
    let normalized = value.trim().replace('/', "\\").to_lowercase();
    normalized.starts_with("c:\\pzmanager\\")
        || normalized
            .starts_with("c:\\users\\administrator\\appdata\\local\\zomboidservermodmanager")
        || normalized.starts_with("c:\\users\\administrator\\zomboid")
}

fn join_command_output(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn emit_remote_setup_log(app: &tauri::AppHandle, phase: &str, stream: &str, line: &str) {
    if line.trim().is_empty() {
        return;
    }

    let _ = app.emit(
        "remote-setup-log",
        RemoteSetupLogEvent {
            phase: phase.to_string(),
            stream: stream.to_string(),
            line: line.to_string(),
        },
    );
}

fn emit_optional_remote_setup_log(
    app: Option<&tauri::AppHandle>,
    phase: &str,
    stream: &str,
    line: &str,
) {
    if let Some(app) = app {
        emit_remote_setup_log(app, phase, stream, line);
    }
}
fn emit_optional_remote_setup_output(
    app: Option<&tauri::AppHandle>,
    phase: &str,
    stream: &str,
    output: &str,
) {
    for line in output.lines() {
        emit_optional_remote_setup_log(app, phase, stream, line);
    }
}

fn join_remote_unix_path(remote_directory: &str, file_name: &str) -> String {
    let directory = remote_directory.trim().trim_end_matches('/').to_string();
    let file_name = file_name.trim_start_matches('/');
    format!("{directory}/{file_name}")
}

fn remote_unix_parent_path(path: &str) -> Option<String> {
    let normalized = path.trim().trim_end_matches('/');
    let index = normalized.rfind('/')?;

    if index == 0 {
        return Some("/".to_string());
    }

    Some(normalized[..index].to_string())
}

pub(crate) fn linux_shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn looks_like_linux_path(value: &str) -> bool {
    value.trim().starts_with('/')
}
#[cfg(windows)]
fn quote_powershell_single_string(value: &str) -> String {
    value.replace('\'', "''")
}

fn validate_authentication(connection: &RemoteServerConnectionRequest) -> Result<(), String> {
    match connection.auth_method.trim() {
        "password" => {
            if connection.password.trim().is_empty() {
                return Err("Enter the SSH password or choose key file authentication.".to_string());
            }
            Ok(())
        }
        "key" => {
            let key_path = required_field(&connection.ssh_key_path, "SSH key file")?;
            let key_path = PathBuf::from(key_path);

            if !key_path.is_file() {
                return Err(format!("SSH key file not found: {}.", key_path.display()));
            }

            Ok(())
        }
        _ => Err("Choose a valid SSH authentication method.".to_string()),
    }
}

fn required_field(value: &str, label: &str) -> Result<String, String> {
    let value = value.trim();

    if value.is_empty() {
        return Err(format!("Enter the remote {label}."));
    }

    Ok(value.to_string())
}

fn looks_like_windows_path(value: &str) -> bool {
    let value = value.trim();
    let bytes = value.as_bytes();
    let has_drive_prefix = bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/');

    has_drive_prefix || value.starts_with("\\\\")
}

#[cfg(windows)]
fn select_ssh_key_file_impl() -> Result<Option<String>, String> {
    let script = r#"
Add-Type -AssemblyName System.Windows.Forms
$dialog = New-Object System.Windows.Forms.OpenFileDialog
$dialog.Title = 'Select SSH private key'
$dialog.Filter = 'SSH private keys|id_*;*.pem;*.key;*.ppk|All files|*.*'
$dialog.CheckFileExists = $true
$dialog.Multiselect = $false
if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
  [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
  Write-Output $dialog.FileName
}
"#;

    let mut command = Command::new("powershell.exe");
    let output = hide_command_window(&mut command)
        .args([
            "-NoProfile",
            "-STA",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ])
        .output()
        .map_err(|error| format!("Could not open the SSH key picker: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        return Err(if stderr.is_empty() {
            "Could not select the SSH key file.".to_string()
        } else {
            stderr
        });
    }

    let selected_path = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if selected_path.is_empty() {
        return Ok(None);
    }

    Ok(Some(selected_path))
}

#[cfg(not(windows))]
fn select_ssh_key_file_impl() -> Result<Option<String>, String> {
    let output = Command::new("sh")
        .args([
            "-lc",
            &format!(
                "command -v zenity >/dev/null 2>&1 && zenity --file-selection --title={} || command -v kdialog >/dev/null 2>&1 && kdialog --getopenfilename ~ '' || true",
                linux_shell_quote("Select SSH private key")
            ),
        ])
        .output()
        .map_err(|error| format!("Could not open the SSH key picker: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "Could not select the SSH key file.".to_string()
        } else {
            stderr
        });
    }

    let selected_path = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if selected_path.is_empty() {
        return Ok(None);
    }

    Ok(Some(selected_path))
}

#[derive(serde::Deserialize, Debug)]
struct RemoteFileItem {
    p: String, // relative path
    l: u64,    // length (size)
    t: i64,    // timestamp (seconds since epoch)
}

fn collect_local_files_recursive(
    dir: &Path,
    base_dir: &Path,
    files: &mut HashMap<String, (u64, u64)>,
) -> Result<(), String> {
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.is_dir() {
                collect_local_files_recursive(&path, base_dir, files)?;
            } else {
                let metadata = std::fs::metadata(&path).map_err(|e| e.to_string())?;
                let len = metadata.len();
                let modified = metadata
                    .modified()
                    .map(|t| {
                        t.duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0)
                    })
                    .unwrap_or(0);

                let relative_path = path
                    .strip_prefix(base_dir)
                    .map_err(|e| e.to_string())?
                    .to_string_lossy()
                    .replace('\\', "/");

                files.insert(relative_path, (len, modified));
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn delete_zomboid_mod_command(
    package_path: String,
    workshop_id: Option<String>,
    connection: Option<RemoteServerConnectionRequest>,
) -> Result<(), String> {
    crate::run_blocking(move || {
        let is_remote = connection.is_some();
        if let Some(conn) = connection {
            let mut paths_to_delete = vec![linux_shell_quote(&package_path)];

            if let Some(ref ws_id) = workshop_id {
                let candidates = remote_steam_workshop_dir_candidates(&conn);
                for (_, ws_dir, _) in candidates {
                    let full_path = format!("{}/{}", ws_dir, ws_id);
                    paths_to_delete.push(linux_shell_quote(&full_path));
                }
            }

            for path in paths_to_delete {
                let cmd = format!("sudo -n rm -rf {}", path);
                let _ = run_ssh_capture(&conn, &cmd)?;
            }
        } else {
            let path = std::path::PathBuf::from(&package_path);
            if path.exists() {
                if path.is_dir() {
                    std::fs::remove_dir_all(&path).map_err(|error| {
                        format!(
                            "Nao foi possivel excluir a pasta do mod em {}: {error}",
                            path.display()
                        )
                    })?;
                } else {
                    std::fs::remove_file(&path).map_err(|error| {
                        format!(
                            "Nao foi possivel excluir o arquivo do mod em {}: {error}",
                            path.display()
                        )
                    })?;
                }
            }

            if let Some(ref ws_id) = workshop_id {
                let mut ws_paths = crate::mods::steam_workshop_dirs();
                ws_paths.extend(crate::mods::steamcmd_workshop_dirs());
                for ws_root in ws_paths {
                    let ws_item_dir = ws_root.join(ws_id);
                    if ws_item_dir.exists() {
                        let _ = std::fs::remove_dir_all(&ws_item_dir);
                    }
                }
            }
        }

        if !is_remote {
            let _ = crate::mods::clear_zomboid_mods_cache_impl();
        }

        Ok(())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_ram_config_dirs_include_default_fallback_for_legacy_launcher_path() {
        let dirs = remote_zomboid_server_config_dirs("/opt/pzserver/start-server.sh");

        assert_eq!(dirs[0], "/opt/pzserver");
        assert!(dirs.contains(&REMOTE_LINUX_ZOMBOID_SERVER_DIR.to_string()));
    }

    #[test]
    fn remote_ram_config_dirs_treat_directory_as_directory() {
        let dirs = remote_zomboid_server_config_dirs("/var/lib/pzmm/zomboid-server");

        assert_eq!(dirs, vec![REMOTE_LINUX_ZOMBOID_SERVER_DIR.to_string()]);
    }

    #[test]
    fn remote_ram_script_updates_json_configs_without_writing_launcher_script() {
        let script = build_remote_performance_settings_script(
            "/opt/pzserver/start-server.sh",
            8192,
            "steam",
        );

        assert!(script.contains("ProjectZomboid64.json"));
        assert!(script.contains("ProjectZomboid32.json"));
        assert!(script.contains("server_owner='steam'"));
        assert!(script.contains("detected_owner=$(sudo -n stat -c '%U' \"$json_path\")"));
        assert!(script.contains("sudo -n -u \"$server_owner\""));
        assert!(script.contains("-Xms\" + ram + \"m"));
        assert!(script.contains("-Xmx\" + ram + \"m"));
        assert!(!script.contains("path.write_text(text)"));
        assert!(!script.contains("launcher="));
    }

    #[test]
    fn remote_server_profile_path_accepts_zomboid_data_folder() {
        let profile_path = resolve_remote_server_profile_path(Some("/srv/pz/Zomboid"), "").unwrap();

        assert_eq!(profile_path, "/srv/pz/Zomboid/Server");
    }

    #[test]
    fn remote_data_dir_is_parent_of_zomboid_folder() {
        assert_eq!(
            remote_data_dir_from_server_profile_path("/srv/pz/Zomboid/Server"),
            "/srv/pz"
        );
    }

    #[test]
    fn remote_steam_workshop_candidates_include_steamcmd_home_library() {
        let connection = RemoteServerConnectionRequest {
            name: "test".to_string(),
            host: "example.com".to_string(),
            port: "22".to_string(),
            username: "ubuntu".to_string(),
            auth_method: "key".to_string(),
            password: String::new(),
            ssh_key_path: String::new(),
            server_path: String::new(),
        };
        let paths = remote_steam_workshop_dir_candidates(&connection)
            .into_iter()
            .map(|(_, path, _)| path)
            .collect::<Vec<_>>();

        assert!(paths.contains(&remote_steamcmd_home_workshop_dir(&connection)));
        assert!(paths.contains(&remote_default_steam_workshop_dir(&connection)));
    }

    #[test]
    fn remote_workspace_owner_defaults_to_managed_user() {
        assert_eq!(
            remote_workspace_owner_or_default(""),
            REMOTE_LINUX_MANAGED_USER
        );
    }

    #[test]
    fn remote_workspace_owner_strips_shell_quotes() {
        assert_eq!(remote_workspace_owner_or_default("'pzalt'"), "pzalt");
        assert_eq!(remote_workspace_owner_or_default("\"pzalt\""), "pzalt");
        assert_eq!(remote_workspace_owner_or_default("$'pzalt'"), "pzalt");
        assert_eq!(
            remote_workspace_owner_or_default("$'\\''pzalt'\\''"),
            "pzalt"
        );
        assert_eq!(
            remote_workspace_owner_or_default("'$\\''pzalt'\\'''"),
            "pzalt"
        );
        assert_eq!(linux_sudo_user_arg("'$\\''pzalt'\\'''"), "'pzalt'");
    }

    #[test]
    fn linux_username_validation_rejects_unknown_owner() {
        assert!(validate_linux_username("UNKNOWN").is_err());
        assert!(validate_linux_username("steam").is_ok());
    }
}
