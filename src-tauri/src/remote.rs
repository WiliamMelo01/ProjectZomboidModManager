use crate::i18n::text;
use crate::models::{
    AppSettings, DeleteServerResult, ModLocation, RemoteAppSettingsRequest,
    RemoteHelperSetupResult, RemoteModLocationRequest, RemoteServerActionResult,
    RemoteServerConnectionRequest, RemoteServerConnectionResult, RemoteServerDeployRequest,
    RemoteServerDeployResult, RemoteServerFirewallCheck, RemoteServerLatencyResult,
    RemoteSetupLogEvent, RemoteSteamCmdUploadRequest, RemoteSteamCmdUploadResult,
    RemoteWorkspaceConfig, RemoteZomboidServerInstallRequest, RemoteZomboidServerInstallResult,
    ServerIniSettings, ServerLuaSetting, ServerLuaSettings, ServerTestEvent, ServerTestResult,
    ServerTestStarted, TerminalCommandRequest, TerminalCommandResult, WorkshopDownloadEvent,
    WorkshopDownloadFailedItem, WorkshopDownloadLogEvent, WorkshopDownloadResult,
    ZomboidModInstallResult, ZomboidServer,
};
use crate::mods::{list_zomboid_mods_impl, parse_server_mod_ids};
#[cfg(windows)]
use crate::util::hide_command_window;
use crate::util::{read_ini_value, read_ini_values, read_text_lossy, replace_or_append_ini_value};
use crate::workshop::api::{fetch_steam_workshop_collection_items, validate_workshop_id};
use crate::{app_config_dir, run_blocking, steamcmd_zip_resource_path};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::hash_map::DefaultHasher,
    collections::{HashMap, HashSet},
    env, fs,
    hash::{Hash, Hasher},
    io::{BufRead, BufReader, Write},
    net::{TcpStream, ToSocketAddrs},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::{mpsc, Mutex, OnceLock},
    thread,
    time::{Duration, Instant},
};
use tauri::Emitter;

const REMOTE_CONNECT_TIMEOUT_SECONDS: u64 = 5;

static VERIFIED_REMOTE_HELPERS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

#[tauri::command]
pub(crate) async fn test_remote_server_connection(
    connection: RemoteServerConnectionRequest,
) -> Result<RemoteServerConnectionResult, String> {
    run_blocking(move || test_remote_server_connection_impl(connection)).await
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

    let config =
        get_remote_workspace_config_impl()?.unwrap_or_else(default_remote_workspace_config);
    let server_launch_path = config.remote_zomboid_server_path.trim().to_string();

    if server_launch_path.is_empty() {
        return Err(text(
            "Configure the remote Project Zomboid server path before testing the server.",
            "Configure o caminho do servidor Project Zomboid remoto antes de testar o servidor.",
        )
        .to_string());
    }

    let event_server_id = server_id.clone();
    thread::spawn(move || {
        if let Err(error) = run_remote_zomboid_server_test_streaming(
            &app,
            &connection,
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
        let _value: Value = run_remote_helper_json(
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
pub(crate) async fn start_remote_zomboid_server(
    connection: RemoteServerConnectionRequest,
    server_id: String,
) -> Result<RemoteServerActionResult, String> {
    run_blocking(move || {
        let config = get_remote_workspace_config_impl()?.unwrap_or_else(default_remote_workspace_config);
        let server_launch_path = config.remote_zomboid_server_path.trim().to_string();

        if server_launch_path.is_empty() {
            return Err(text(
                "Configure the remote Project Zomboid server path before starting the server.",
                "Configure o caminho do servidor Project Zomboid remoto antes de iniciar o servidor.",
            )
            .to_string());
        }

        run_remote_helper_json(
            &connection,
            "start-server",
            Some(&serde_json::json!({
                "serverId": server_id,
                "serverLaunchPath": server_launch_path,
            })),
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn select_ssh_key_file() -> Result<Option<String>, String> {
    run_blocking(select_ssh_key_file_impl).await
}

#[tauri::command]
pub(crate) async fn get_remote_workspace_config() -> Result<Option<RemoteWorkspaceConfig>, String> {
    run_blocking(get_remote_workspace_config_impl).await
}

#[tauri::command]
pub(crate) async fn save_remote_workspace_config(
    config: RemoteWorkspaceConfig,
) -> Result<RemoteWorkspaceConfig, String> {
    run_blocking(move || save_remote_workspace_config_impl(config)).await
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
pub(crate) async fn setup_remote_helper(
    app: tauri::AppHandle,
    connection: RemoteServerConnectionRequest,
) -> Result<RemoteHelperSetupResult, String> {
    run_blocking(move || setup_remote_helper_impl(Some(&app), &connection)).await
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
        let _value: Value =
            run_remote_helper_json(&connection, "clear-mods-cache", Option::<&Value>::None)?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub(crate) async fn clear_remote_zomboid_mods_and_images_cache(
    connection: RemoteServerConnectionRequest,
) -> Result<(), String> {
    run_blocking(move || {
        let _value: Value =
            run_remote_helper_json(&connection, "clear-mods-cache", Option::<&Value>::None)?;
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
        run_remote_helper_json(
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

#[tauri::command]
pub(crate) async fn deploy_local_zomboid_server_to_remote(
    app: tauri::AppHandle,
    request: RemoteServerDeployRequest,
) -> Result<RemoteServerDeployResult, String> {
    run_blocking(move || deploy_local_zomboid_server_to_remote_impl(&app, request)).await
}

fn load_mods_from_cache() -> Option<Vec<crate::models::ZomboidMod>> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct CachedModsFileMin {
        version: u32,
        entries: HashMap<String, CachedModEntryMin>,
    }

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct CachedModEntryMin {
        mod_item: crate::models::ZomboidMod,
    }

    let config_dir = app_config_dir().ok()?;
    let cache_file = config_dir.join("mods-library-cache.json");
    if !cache_file.is_file() {
        return None;
    }
    let content = fs::read_to_string(cache_file).ok()?;
    let parsed: CachedModsFileMin = serde_json::from_str(&content).ok()?;
    if parsed.version != 1 {
        return None;
    }
    let mods = parsed
        .entries
        .into_values()
        .map(|entry| entry.mod_item)
        .collect();
    Some(mods)
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
    let mut folders_to_copy = HashSet::new();
    let mut active_mods_count = 0;

    if request.include_mods {
        let ini_content = read_text_lossy(&ini_path)?;
        let configured_mods = read_ini_value(&ini_content, "Mods").unwrap_or_default();
        let active_mod_ids = parse_server_mod_ids(&configured_mods);

        if !active_mod_ids.is_empty() {
            let all_mods = load_mods_from_cache()
                .unwrap_or_else(|| list_zomboid_mods_impl().unwrap_or_default());
            for mod_item in all_mods {
                if mod_item.source == "local" {
                    let is_active = mod_item.variants.iter().any(|v| {
                        active_mod_ids
                            .iter()
                            .any(|active_id| active_id.eq_ignore_ascii_case(&v.id))
                    }) || active_mod_ids
                        .iter()
                        .any(|active_id| active_id.eq_ignore_ascii_case(&mod_item.id));

                    if is_active {
                        folders_to_copy.insert(PathBuf::from(mod_item.package_path));
                    }
                }
            }
            active_mods_count = folders_to_copy.len();
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
        (&sandbox_path, format!("{server_id}_SandboxVars.lua"), "SandboxVars file"),
        (&spawnregions_path, format!("{server_id}_spawnregions.lua"), "spawnregions file"),
        (&spawnpoints_path, format!("{server_id}_spawnpoints.lua"), "spawnpoints file"),
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
    for (i, folder) in folders_to_copy.iter().enumerate() {
        let folder_name = folder
            .file_name()
            .ok_or_else(|| "Invalid mod folder name".to_string())?;

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

        let dest = temp_mods_dir.join(folder_name);
        copy_dir_all(folder, &dest)?;
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

    let remote_zomboid_dir = {
        let server_path_normalized = connection.server_path.replace('/', "\\");
        if let Some(idx) = server_path_normalized.rfind('\\') {
            server_path_normalized[..idx].to_string()
        } else {
            format!("C:\\Users\\{}\\Zomboid", connection.username.trim())
        }
    };

    let remote_server_zip_path = join_remote_windows_path(&remote_zomboid_dir, "deploy-server.zip");
    let remote_mods_zip_path = join_remote_windows_path(&remote_zomboid_dir, "deploy-mods.zip");

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
        "$true"
    } else {
        "$false"
    };
    let remote_script = format!(
        r#"$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$zomboidDir = '{}'
$overwriteExisting = {}
$serverZipPath = Join-Path $zomboidDir 'deploy-server.zip'
$modsZipPath = Join-Path $zomboidDir 'deploy-mods.zip'
$modsTarget = Join-Path $zomboidDir 'mods'

function Expand-PzmmArchive([string]$zipPath, [string]$targetPath, [string]$label) {{
    if (!(Test-Path -LiteralPath $zipPath -PathType Leaf)) {{
        Write-Output "PZMM_STEP|Skipping missing $label archive"
        return
    }}

    Write-Output "PZMM_STEP|Extracting $label archive directly to $targetPath"
    New-Item -ItemType Directory -Force -Path $targetPath | Out-Null

    if (Get-Command tar.exe -ErrorAction SilentlyContinue) {{
        if ($overwriteExisting) {{
            tar.exe -xf $zipPath -C $targetPath
        }} else {{
            tar.exe -k -xf $zipPath -C $targetPath
        }}
        if ($LASTEXITCODE -ne 0) {{ throw "tar.exe extraction failed for $label with exit code $LASTEXITCODE" }}
    }} else {{
        if ($overwriteExisting) {{
            Expand-Archive -LiteralPath $zipPath -DestinationPath $targetPath -Force
        }} else {{
            Expand-Archive -LiteralPath $zipPath -DestinationPath $targetPath
        }}
    }}
}}

try {{
    Expand-PzmmArchive $serverZipPath $zomboidDir 'server data'
    Expand-PzmmArchive $modsZipPath $modsTarget 'mods'
    Write-Output 'DEPLOY_SUCCESS'
}} finally {{
    Write-Output 'PZMM_STEP|Cleaning remote compressed deploy files'
    Remove-Item -LiteralPath $serverZipPath -Force -Confirm:$false -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $modsZipPath -Force -Confirm:$false -ErrorAction SilentlyContinue
}}
"#,
        quote_powershell_single_string(&remote_zomboid_dir),
        overwrite_existing,
    );
    let remote_command = powershell_encoded_command(&remote_script);

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
        skipped_mods: Vec::new(),
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
    if !cfg!(windows) {
        return Err(text(
            "Remote terminal commands are available only on Windows for now.",
            "Comandos remotos no terminal estao disponiveis apenas no Windows por enquanto.",
        )
        .to_string());
    }

    let host = required_field(&connection.host, "host")?;
    let username = required_field(&connection.username, "Windows username")?;
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
    let mut child = Command::new("ssh.exe")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "StrictHostKeyChecking=accept-new",
            "-i",
        ])
        .arg(&key_path)
        .args(["-p", &port.to_string(), &remote, command_text])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Could not run ssh.exe: {error}"))?;

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
    #[cfg(windows)]
    let command = hide_command_window(&mut command);

    let mut child = command
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
        .map_err(|error| format!("Could not start powershell to compress: {error}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or("Could not capture compression stdout.")?;
    let stderr = child
        .stderr
        .take()
        .ok_or("Could not capture compression stderr.")?;

    let reader = BufReader::new(stdout);
    for line_result in reader.lines() {
        if let Ok(line) = line_result {
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
    let username = required_field(&connection.username, "Windows username")?;
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
    let output = Command::new("scp.exe")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "StrictHostKeyChecking=accept-new",
            "-i",
        ])
        .arg(&key_path)
        .args(["-P", &port.to_string()])
        .arg(local_path)
        .arg(&remote)
        .output()
        .map_err(|error| format!("Could not run scp.exe: {error}"))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    Err(format!("scp.exe failed: {}\n{}", stdout, stderr))
}
#[tauri::command]
pub(crate) async fn delete_remote_zomboid_server(
    connection: RemoteServerConnectionRequest,
    server_id: String,
) -> Result<DeleteServerResult, String> {
    run_blocking(move || {
        run_remote_helper_json(
            &connection,
            "delete-server",
            Some(&serde_json::json!({ "serverId": server_id })),
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn get_remote_zomboid_server_settings(
    connection: RemoteServerConnectionRequest,
    server_id: String,
) -> Result<ServerIniSettings, String> {
    run_blocking(move || {
        run_remote_helper_json(
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
        run_remote_helper_json(
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
        let _value: Value = run_remote_helper_json(
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
        let _value: Value = run_remote_helper_json(
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
        run_remote_helper_json(
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
        run_remote_helper_json(
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
        run_remote_helper_json(
            &connection,
            "install-mod",
            Some(&serde_json::json!({
                "packagePath": package_path,
                "modId": mod_id,
                "workshopId": workshop_id,
            })),
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn install_remote_zomboid_server_map(
    connection: RemoteServerConnectionRequest,
    server_id: String,
    mod_path: String,
) -> Result<(), String> {
    run_blocking(move || {
        let _value: Value = run_remote_helper_json(
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

fn get_remote_workspace_config_impl() -> Result<Option<RemoteWorkspaceConfig>, String> {
    let path = remote_workspace_config_path()?;

    if !path.is_file() {
        return Ok(None);
    }

    let content = read_text_lossy(&path)?;
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
        server_path: read_ini_value(&content, "server_path")
            .unwrap_or_else(|| "C:\\Users\\Administrator\\Zomboid\\Server".to_string()),
        remote_steamcmd_dir,
        remote_steamcmd_path,
        remote_zomboid_server_dir,
        remote_zomboid_server_path,
        remote_client_ram: read_ini_value(&content, "remote_client_ram")
            .or_else(|| read_ini_value(&content, "client_ram"))
            .unwrap_or_else(|| "4.00".to_string()),
        remote_server_ram: read_ini_value(&content, "remote_server_ram")
            .or_else(|| read_ini_value(&content, "server_ram"))
            .unwrap_or_else(|| "4.00".to_string()),
        remote_mod_locations: read_ini_values(&content, "remote_mod_location"),
    }))
}

fn save_remote_workspace_config_impl(
    config: RemoteWorkspaceConfig,
) -> Result<RemoteWorkspaceConfig, String> {
    write_remote_workspace_config(&config)?;
    Ok(config)
}

fn get_remote_app_settings_impl(
    _connection: RemoteServerConnectionRequest,
) -> Result<AppSettings, String> {
    let config =
        get_remote_workspace_config_impl()?.unwrap_or_else(default_remote_workspace_config);

    Ok(remote_app_settings_from_config(&config))
}

fn get_remote_system_ram_impl(connection: &RemoteServerConnectionRequest) -> Result<u32, String> {
    run_remote_helper_json(connection, "get-system-ram", Option::<&Value>::None)
}

fn save_remote_app_settings_impl(request: RemoteAppSettingsRequest) -> Result<AppSettings, String> {
    let client_ram = normalize_remote_ram_gb(&request.client_ram)?;
    let server_ram = normalize_remote_ram_gb(&request.server_ram)?;
    let mut config =
        get_remote_workspace_config_impl()?.unwrap_or_else(default_remote_workspace_config);
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

    apply_remote_performance_settings(&request.connection, &server_path, &server_ram)?;

    config.name = request.connection.name;
    config.host = request.connection.host;
    config.port = request.connection.port;
    config.username = request.connection.username;
    config.auth_method = request.connection.auth_method;
    config.ssh_key_path = request.connection.ssh_key_path;
    config.server_path = request.connection.server_path;
    config.remote_zomboid_server_path = server_path.clone();
    config.remote_zomboid_server_dir = remote_windows_parent_path(&server_path)
        .unwrap_or_else(|| config.remote_zomboid_server_dir.clone());
    config.remote_client_ram = client_ram;
    config.remote_server_ram = server_ram;
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

fn get_remote_mod_locations_impl(
    connection: RemoteServerConnectionRequest,
) -> Result<Vec<ModLocation>, String> {
    let config =
        get_remote_workspace_config_impl()?.unwrap_or_else(default_remote_workspace_config);
    let steamcmd_workshop_dir = join_remote_windows_path(
        &config.remote_steamcmd_dir,
        "steamapps/workshop/content/108600",
    );
    let mut entries = vec![
        (
            "SteamCMD 1".to_string(),
            steamcmd_workshop_dir,
            "steamcmd".to_string(),
        ),
        (
            "Local mods".to_string(),
            format!("C:\\Users\\{}\\Zomboid\\mods", connection.username.trim()),
            "local".to_string(),
        ),
    ];

    for path in config.remote_mod_locations {
        entries.push((remote_mod_location_label(&path), path, "custom".to_string()));
    }

    let remote_paths: Vec<RemotePathExists> = run_remote_helper_json(
        &connection,
        "get-path-status",
        Some(&serde_json::json!({
            "paths": entries.iter().map(|(_, path, _)| path).collect::<Vec<_>>(),
        })),
    )?;

    Ok(entries
        .into_iter()
        .map(|(label, path, kind)| {
            let exists = remote_paths
                .iter()
                .find(|item| item.path.eq_ignore_ascii_case(&path))
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
    if !looks_like_windows_path(&path) && !path.starts_with("$env:") {
        return Err("Use a Windows remote mod folder path.".to_string());
    }

    let mut config =
        get_remote_workspace_config_impl()?.unwrap_or_else(default_remote_workspace_config);
    if !config
        .remote_mod_locations
        .iter()
        .any(|current| current.eq_ignore_ascii_case(&path))
    {
        config.remote_mod_locations.push(path);
        write_remote_workspace_config(&config)?;
    }

    get_remote_mod_locations_impl(request.connection)
}

fn open_remote_mod_location_impl(request: RemoteModLocationRequest) -> Result<(), String> {
    let path = required_field(&request.path, "remote mod folder")?;
    let statuses: Vec<RemotePathExists> = run_remote_helper_json(
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

fn remote_mod_location_label(path: &str) -> String {
    path.trim()
        .replace('/', "\\")
        .rsplit('\\')
        .find(|part| !part.trim().is_empty())
        .unwrap_or("Custom")
        .to_string()
}

fn remote_workspace_config_path() -> Result<PathBuf, String> {
    Ok(app_config_dir()?.join("remote-workspace.ini"))
}

fn write_remote_workspace_config(config: &RemoteWorkspaceConfig) -> Result<(), String> {
    let path = remote_workspace_config_path()?;

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
        ("remote_client_ram", config.remote_client_ram.as_str()),
        ("remote_server_ram", config.remote_server_ram.as_str()),
    ] {
        content = replace_or_append_ini_value(&content, key, value);
    }

    for location in &config.remote_mod_locations {
        if !location.trim().is_empty() {
            content.push_str(&format!("\nremote_mod_location={}", location.trim()));
        }
    }

    fs::write(&path, format!("{content}\n"))
        .map_err(|error| format!("Nao foi possivel salvar {}: {error}", path.display()))
}

fn test_remote_server_connection_impl(
    connection: RemoteServerConnectionRequest,
) -> Result<RemoteServerConnectionResult, String> {
    if !cfg!(windows) {
        return Err(text(
            "Remote workspaces are available only on Windows for now.",
            "Workspaces remotos estao disponiveis apenas no Windows por enquanto.",
        )
        .to_string());
    }

    let name = required_field(&connection.name, "connection name")?;
    let host = required_field(&connection.host, "host")?;
    let username = required_field(&connection.username, "Windows username")?;
    let server_path = required_field(&connection.server_path, "server profile folder")?;
    validate_authentication(&connection)?;
    let port = connection
        .port
        .trim()
        .parse::<u16>()
        .map_err(|_| "Enter a valid remote port.".to_string())?;

    if username.contains(['/', '\\']) {
        return Err("Use a Windows username without path separators.".to_string());
    }

    if !looks_like_windows_path(&server_path) {
        return Err("Use a Windows server profile folder, for example C:\\Users\\Administrator\\Zomboid\\Server.".to_string());
    }

    let latency = measure_remote_tcp_latency(&host, port)?;

    if connection.auth_method.trim() == "key" {
        verify_ssh_key_authentication(&connection, port)?;
    }

    Ok(RemoteServerConnectionResult {
        name,
        host,
        port,
        server_path,
        message: "Remote host is reachable. File access setup is ready for the next step."
            .to_string(),
        latency_ms: latency.as_millis(),
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

fn measure_remote_tcp_latency(host: &str, port: u16) -> Result<Duration, String> {
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

fn apply_remote_performance_settings(
    connection: &RemoteServerConnectionRequest,
    server_path: &str,
    server_ram: &str,
) -> Result<(), String> {
    let server_mb = remote_ram_gb_to_mb(server_ram)?;
    let server_dir = remote_windows_parent_path(server_path)
        .ok_or_else(|| "Could not resolve the remote Project Zomboid server folder.".to_string())?;
    let script = format!(
        r#"$ErrorActionPreference='Stop'; $serverPath='{}'; $serverDir='{}'; $ramMb={}; if (!(Test-Path -LiteralPath $serverPath)) {{ throw "Remote Project Zomboid server path not found: $serverPath" }}; function Update-Line([string]$line, [int]$ram) {{ $line = [regex]::Replace($line, '-Xms\S+', "-Xms${{ram}}m", 'IgnoreCase'); $line = [regex]::Replace($line, '-Xmx\S+', "-Xmx${{ram}}m", 'IgnoreCase'); if ($line -notmatch '-Xms') {{ $line = "-Xms${{ram}}m $line" }}; if ($line -notmatch '-Xmx') {{ $line = "-Xmx${{ram}}m $line" }}; return $line }}; function Update-Bat([string]$path, [int]$ram) {{ if (!(Test-Path -LiteralPath $path -PathType Leaf)) {{ return $false }}; $content = Get-Content -LiteralPath $path -Raw; if ($content -notmatch '-Xms' -and $content -notmatch '-Xmx') {{ return $false }}; $lines = $content -split "`r?
" | ForEach-Object {{ if ($_ -match '-Xms|-Xmx') {{ Update-Line $_ $ram }} else {{ $_ }} }}; Set-Content -LiteralPath $path -Value ($lines -join "`r
") -Encoding UTF8; return $true }}; function Update-Json([string]$path, [int]$ram) {{ if (!(Test-Path -LiteralPath $path -PathType Leaf)) {{ return $false }}; $json = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json; $args = @("-Xms${{ram}}m", "-Xmx${{ram}}m"); if ($json.PSObject.Properties.Name -contains 'vmArgs') {{ if ($json.vmArgs -is [array]) {{ $other = @($json.vmArgs | Where-Object {{ $_ -notmatch '^-Xm[sx]' }}); $json.vmArgs = @($args + $other) }} else {{ $json.vmArgs = (Update-Line ([string]$json.vmArgs) $ram) }} }} else {{ $json | Add-Member -NotePropertyName vmArgs -NotePropertyValue $args }}; $json | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $path -Encoding UTF8; return $true }}; $candidates = @($serverPath, (Join-Path $serverDir 'StartServer64.bat'), (Join-Path $serverDir 'ProjectZomboidServer.bat'), (Join-Path $serverDir 'ProjectZomboidServer64.json')); $updated = $false; foreach ($candidate in $candidates) {{ $ext = [IO.Path]::GetExtension($candidate).ToLowerInvariant(); if ($ext -eq '.bat') {{ $updated = (Update-Bat $candidate $ramMb) -or $updated }} elseif ($ext -eq '.json') {{ $updated = (Update-Json $candidate $ramMb) -or $updated }} }}; if (!$updated) {{ throw "Could not find -Xms/-Xmx settings in remote server launch files." }}; Write-Output "PZMM_REMOTE_PERFORMANCE_UPDATED=$serverDir""#,
        quote_powershell_single_string(server_path),
        quote_powershell_single_string(&server_dir),
        server_mb,
    );
    let command = powershell_encoded_command(&script);
    let result = run_ssh_capture(connection, &command)?;

    if result.success {
        Ok(())
    } else {
        Err(join_command_output(&[
            "Could not apply remote performance settings.",
            result.stdout.as_str(),
            result.stderr.as_str(),
        ]))
    }
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
    run_remote_helper_json(&connection, "list-servers", Option::<&Value>::None)
}

fn list_remote_zomboid_mods_impl(
    connection: RemoteServerConnectionRequest,
) -> Result<Vec<crate::models::ZomboidMod>, String> {
    let mut mods: Vec<crate::models::ZomboidMod> =
        run_remote_helper_json(&connection, "list-mods", Option::<&Value>::None)?;
    hydrate_remote_mod_images(&connection, &mut mods);
    Ok(mods)
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

    let config =
        get_remote_workspace_config_impl()?.unwrap_or_else(default_remote_workspace_config);
    let steamcmd_path = required_field(&config.remote_steamcmd_path, "remote SteamCMD path")?;
    let steamcmd_workshop_dir = join_remote_windows_path(
        &config.remote_steamcmd_dir,
        "steamapps/workshop/content/108600",
    );

    let mut skipped_ids = Vec::new();
    let mut pending_ids = workshop_ids.clone();

    if !force_validate {
        let existing_ids =
            remote_existing_workshop_ids(&connection, &steamcmd_workshop_dir, &workshop_ids)?;
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

        if result.success {
            for workshop_id in chunk {
                completed_items.insert(workshop_id.clone());
                emit_workshop_download_event(app, workshop_id, "completed", None);
            }
        } else {
            let output = join_command_output(&[result.stdout.as_str(), result.stderr.as_str()]);
            for workshop_id in chunk {
                emit_workshop_download_event(app, workshop_id, "failed", Some(&output));
                failed_items.push(WorkshopDownloadFailedItem {
                    workshop_id: workshop_id.clone(),
                    name: workshop_id.clone(),
                    error: output.clone(),
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

    let mut steamcmd_args = String::from("+login anonymous");
    for workshop_id in workshop_ids {
        steamcmd_args.push_str(&format!(" +workshop_download_item 108600 {workshop_id}"));
        if force_validate {
            steamcmd_args.push_str(" validate");
        }
    }
    steamcmd_args.push_str(" +quit");

    let script = format!(
        r#"$ErrorActionPreference='Stop'; $steamcmd='{}'; if (!(Test-Path -LiteralPath $steamcmd -PathType Leaf)) {{ throw "SteamCMD not found: $steamcmd" }}; & $steamcmd {steamcmd_args}; if ($LASTEXITCODE -ne 0) {{ exit $LASTEXITCODE }}"#,
        quote_powershell_single_string(steamcmd_path)
    );
    let command = powershell_encoded_command(&script);
    run_ssh_workshop_streaming(app, connection, &command)
}

fn remote_existing_workshop_ids(
    connection: &RemoteServerConnectionRequest,
    workshop_dir: &str,
    workshop_ids: &[String],
) -> Result<HashSet<String>, String> {
    let ids_literal = workshop_ids
        .iter()
        .map(|workshop_id| format!("'{}'", quote_powershell_single_string(workshop_id)))
        .collect::<Vec<_>>()
        .join(",");
    let script = format!(
        r#"$ErrorActionPreference='Stop'; $root='{}'; $ids=@({ids_literal}); $ids | ForEach-Object {{ [pscustomobject]@{{ path=$_; exists=(Test-Path -LiteralPath (Join-Path $root $_) -PathType Container) }} }} | ConvertTo-Json -Compress -Depth 3"#,
        quote_powershell_single_string(workshop_dir)
    );
    let command = powershell_encoded_command(&script);
    let output = run_ssh_capture(connection, &command)?;
    let remote_paths = parse_remote_json_array::<RemotePathExists>(&output.stdout)?;

    Ok(remote_paths
        .into_iter()
        .filter(|item| item.exists)
        .map(|item| item.path)
        .collect())
}

fn cancel_remote_steam_workshop_download_impl(
    connection: RemoteServerConnectionRequest,
) -> Result<(), String> {
    let script =
        r#"$ErrorActionPreference='SilentlyContinue'; taskkill /F /IM steamcmd.exe /T; exit 0"#;
    let command = powershell_encoded_command(script);
    let _ = run_ssh_capture(&connection, &command)?;
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
    let script = match encoded_payload.as_ref() {
        Some(_) => format!(
            r#"$ErrorActionPreference='Stop'; & '{}' '{}' '-'"#,
            quote_powershell_single_string(&helper_path),
            quote_powershell_single_string(helper_command),
        ),
        None => format!(
            r#"$ErrorActionPreference='Stop'; & '{}' '{}'"#,
            quote_powershell_single_string(&helper_path),
            quote_powershell_single_string(helper_command),
        ),
    };
    let command = powershell_encoded_command(&script);
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
        let message = format!("pzmm-helper returned no JSON output for {helper_command}.");
        return Err(join_command_output(&[
            message.as_str(),
            "This usually means the remote helper is outdated, missing, or crashed before writing a response.",
            output.stderr.as_str(),
        ]));
    }

    serde_json::from_str::<T>(stdout).map_err(|error| {
        invalidate_remote_helper_cache(connection);
        let message =
            format!("Could not parse pzmm-helper JSON output for {helper_command}: {error}");
        join_command_output(&[message.as_str(), stdout, output.stderr.as_str()])
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteServerTestRequest<'a> {
    server_id: &'a str,
    server_launch_path: Option<&'a str>,
    server_profile_path: Option<&'a str>,
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
    };
    let json = serde_json::to_vec(&payload)
        .map_err(|error| format!("Could not serialize remote server test payload: {error}"))?;
    let encoded_payload = base64::engine::general_purpose::STANDARD.encode(json);
    let script = format!(
        r#"$ErrorActionPreference='Stop'; & '{}' 'test-server' '-'"#,
        quote_powershell_single_string(&helper_path),
    );
    let command = powershell_encoded_command(&script);

    stream_remote_server_test_command(app, connection, server_id, &command, &encoded_payload)
        .map_err(|error| {
            invalidate_remote_helper_cache(connection);
            error
        })
}

fn stream_remote_server_test_command(
    app: &tauri::AppHandle,
    connection: &RemoteServerConnectionRequest,
    server_id: &str,
    command_text: &str,
    stdin_text: &str,
) -> Result<(), String> {
    if !cfg!(windows) {
        return Err(text(
            "Remote server testing is available only on Windows for now.",
            "O teste remoto de servidor esta disponivel apenas no Windows por enquanto.",
        )
        .to_string());
    }

    let host = required_field(&connection.host, "host")?;
    let username = required_field(&connection.username, "Windows username")?;
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
    let mut child = Command::new("ssh.exe")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "StrictHostKeyChecking=accept-new",
            "-i",
        ])
        .arg(&key_path)
        .args(["-p", &port.to_string(), &remote, command_text])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Could not run ssh.exe: {error}"))?;

    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Could not open remote server test stdin.".to_string())?;
        stdin
            .write_all(stdin_text.as_bytes())
            .map_err(|error| format!("Could not write remote server test stdin: {error}"))?;
    }

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Could not capture remote server test stdout.".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Could not capture remote server test stderr.".to_string())?;
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
                    emit_remote_server_test_stdout(app, server_id, &line);
                } else {
                    emit_remote_server_test_line(app, server_id, &format!("[ERR] {line}"));
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Some(status) = child
                    .try_wait()
                    .map_err(|error| format!("Could not read remote server test status: {error}"))?
                {
                    while let Ok((stream, line)) = receiver.try_recv() {
                        if stream == "stdout" {
                            emit_remote_server_test_stdout(app, server_id, &line);
                        } else {
                            emit_remote_server_test_line(app, server_id, &format!("[ERR] {line}"));
                        }
                    }

                    if status.success() {
                        return Ok(());
                    }

                    return Err(format!("Remote server test command failed: {}.", status));
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let status = child
                    .wait()
                    .map_err(|error| format!("Could not wait for remote server test: {error}"))?;

                if status.success() {
                    return Ok(());
                }

                return Err(format!("Remote server test command failed: {}.", status));
            }
        }
    }
}

fn emit_remote_server_test_stdout(app: &tauri::AppHandle, server_id: &str, line: &str) {
    match serde_json::from_str::<RemoteHelperServerTestEvent>(line) {
        Ok(event) => {
            let _ = app.emit(
                "server-test-event",
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
        Err(_) => emit_remote_server_test_line(app, server_id, &format!("[OUT] {line}")),
    }
}

fn emit_remote_server_test_line(app: &tauri::AppHandle, server_id: &str, line: &str) {
    let _ = app.emit(
        "server-test-event",
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
            Ok(cache_path) if cache_path.is_file() => {
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

    looks_like_windows_path(path) || normalized.contains(":\\") || normalized.contains(":/")
}

fn ensure_cached_remote_file(
    connection: &RemoteServerConnectionRequest,
    remote_path: &str,
) -> Result<PathBuf, String> {
    let cache_path = remote_image_cache_path(connection, remote_path)?;

    if cache_path.is_file() {
        return Ok(cache_path);
    }

    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create remote image cache: {error}"))?;
    }

    download_remote_file(connection, remote_path, &cache_path)?;
    Ok(cache_path)
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
    let username = required_field(&connection.username, "Windows username")?;
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
    let output = Command::new("scp.exe")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "StrictHostKeyChecking=accept-new",
            "-i",
        ])
        .arg(&key_path)
        .args(["-P", &port.to_string()])
        .arg(&remote)
        .arg(local_path)
        .output()
        .map_err(|error| format!("Could not run scp.exe for remote file: {error}"))?;

    if output.status.success() {
        return Ok(());
    }

    download_remote_file_via_powershell(connection, remote_path, local_path).map_err(
        |fallback_error| {
            join_command_output(&[
                "Could not download remote file.",
                String::from_utf8_lossy(&output.stdout).as_ref(),
                String::from_utf8_lossy(&output.stderr).as_ref(),
                fallback_error.as_str(),
            ])
        },
    )
}

fn download_remote_file_via_powershell(
    connection: &RemoteServerConnectionRequest,
    remote_path: &str,
    local_path: &PathBuf,
) -> Result<(), String> {
    let script = format!(
        r#"$ErrorActionPreference='Stop'; $path='{}'; if (!(Test-Path -LiteralPath $path -PathType Leaf)) {{ throw "Remote file not found: $path" }}; [Convert]::ToBase64String([System.IO.File]::ReadAllBytes($path))"#,
        quote_powershell_single_string(remote_path)
    );
    let command = powershell_encoded_command(&script);
    let output = run_ssh_capture(connection, &command)?;
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
    let cache_key = remote_helper_cache_key(connection)?;
    let remote_path = join_remote_windows_path(
        &default_remote_helper_dir(&connection.username),
        "pzmm-helper.exe",
    );
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
    if let Ok(cache_key) = remote_helper_cache_key(connection) {
        if let Some(cache) = VERIFIED_REMOTE_HELPERS.get() {
            if let Ok(mut cache) = cache.lock() {
                cache.remove(&cache_key);
            }
        }
    }
}

fn remote_helper_cache_key(connection: &RemoteServerConnectionRequest) -> Result<String, String> {
    let local_path = local_helper_executable_path()?;
    let local_len = fs::metadata(&local_path)
        .map_err(|error| format!("Could not read {}: {error}", local_path.display()))?
        .len();

    Ok(format!(
        "{}|{}|{}|{}|{}",
        connection.host.trim().to_lowercase(),
        connection.port.trim(),
        connection.username.trim().to_lowercase(),
        default_remote_helper_dir(&connection.username).to_lowercase(),
        local_len
    ))
}

fn ensure_remote_helper(connection: &RemoteServerConnectionRequest) -> Result<String, String> {
    let result = setup_remote_helper_impl(None, connection)?;

    if result.success {
        Ok(result.remote_path)
    } else {
        Err(join_command_output(&[
            "Could not prepare pzmm-helper.exe on the remote host.",
            result.stdout.as_str(),
            result.stderr.as_str(),
        ]))
    }
}

fn setup_remote_helper_impl(
    app: Option<&tauri::AppHandle>,
    connection: &RemoteServerConnectionRequest,
) -> Result<RemoteHelperSetupResult, String> {
    if !cfg!(windows) {
        return Err(text(
            "Remote helper setup is available only on Windows for now.",
            "A configuracao do helper remoto esta disponivel apenas no Windows por enquanto.",
        )
        .to_string());
    }

    if connection.auth_method.trim() != "key" {
        return Err(
            "Remote helper setup currently requires SSH private key authentication.".to_string(),
        );
    }

    let local_path = local_helper_executable_path()?;
    let local_len = fs::metadata(&local_path)
        .map_err(|error| format!("Could not read {}: {error}", local_path.display()))?
        .len();
    let remote_dir = default_remote_helper_dir(&connection.username);
    let remote_path = join_remote_windows_path(&remote_dir, "pzmm-helper.exe");
    let check_script = format!(
        r#"$ErrorActionPreference='Stop'; $path='{}'; if ((Test-Path -LiteralPath $path -PathType Leaf) -and ((Get-Item -LiteralPath $path).Length -eq {})) {{ Write-Output 'PZMM_HELPER_READY' }} else {{ Write-Output 'PZMM_HELPER_UPLOAD' }}"#,
        quote_powershell_single_string(&remote_path),
        local_len,
    );
    let check_command = powershell_encoded_command(&check_script);
    emit_optional_remote_setup_log(
        app,
        "helper",
        "info",
        &format!("Checking remote helper at {remote_path}"),
    );
    let check = run_ssh_capture(connection, &check_command)?;

    if check.stdout.contains("PZMM_HELPER_READY") {
        emit_optional_remote_setup_log(app, "helper", "info", "Remote helper is already ready.");
        return Ok(RemoteHelperSetupResult {
            local_path: local_path.display().to_string(),
            remote_path,
            command: check.command,
            exit_code: check.exit_code,
            success: true,
            stdout: join_command_output(&[
                "Remote helper is already up to date.",
                check.stdout.as_str(),
            ]),
            stderr: check.stderr,
        });
    }

    upload_remote_helper(app, connection, &local_path, &remote_dir, &remote_path)
}

fn upload_remote_helper(
    app: Option<&tauri::AppHandle>,
    connection: &RemoteServerConnectionRequest,
    local_path: &PathBuf,
    remote_dir: &str,
    remote_path: &str,
) -> Result<RemoteHelperSetupResult, String> {
    let host = required_field(&connection.host, "host")?;
    let username = required_field(&connection.username, "Windows username")?;
    let port = connection
        .port
        .trim()
        .parse::<u16>()
        .map_err(|_| "Enter a valid remote port.".to_string())?;
    let key_path = PathBuf::from(required_field(&connection.ssh_key_path, "SSH key file")?);

    if !key_path.is_file() {
        return Err(format!("SSH key file not found: {}.", key_path.display()));
    }

    let create_dir_script = format!(
        r#"$ErrorActionPreference='Stop'; New-Item -ItemType Directory -Force -Path '{}' | Out-Null"#,
        quote_powershell_single_string(remote_dir)
    );
    let create_dir_command = powershell_encoded_command(&create_dir_script);
    emit_optional_remote_setup_log(
        app,
        "helper",
        "info",
        &format!("Preparing remote helper folder: {remote_dir}"),
    );
    let create_dir_result = run_ssh_capture(connection, &create_dir_command)?;

    let stop_helper_script = r#"$ErrorActionPreference='SilentlyContinue'; Get-Process -Name 'pzmm-helper' | Stop-Process -Force; Start-Sleep -Milliseconds 300; exit 0"#;
    let stop_helper_command = powershell_encoded_command(stop_helper_script);
    emit_optional_remote_setup_log(
        app,
        "helper",
        "info",
        "Stopping any previous remote setup process before upload.",
    );
    let stop_helper_result = run_ssh_capture(connection, &stop_helper_command)?;
    emit_optional_remote_setup_output(app, "helper", "stdout", &stop_helper_result.stdout);
    emit_optional_remote_setup_output(app, "helper", "stderr", &stop_helper_result.stderr);

    let remote = format!("{username}@{host}:{remote_path}");
    emit_optional_remote_setup_log(
        app,
        "helper",
        "info",
        &format!("Uploading pzmm-helper.exe to {remote_path}"),
    );
    let output = Command::new("scp.exe")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "StrictHostKeyChecking=accept-new",
            "-i",
        ])
        .arg(&key_path)
        .args(["-P", &port.to_string()])
        .arg(local_path)
        .arg(&remote)
        .output()
        .map_err(|error| format!("Could not run scp.exe for pzmm-helper: {error}"))?;
    let scp_stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let scp_stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let stdout = join_command_output(&[
        create_dir_result.stdout.as_str(),
        stop_helper_result.stdout.as_str(),
        scp_stdout.as_str(),
        if output.status.success() {
            "Remote helper upload completed."
        } else {
            ""
        },
    ]);
    let stderr = join_command_output(&[
        create_dir_result.stderr.as_str(),
        stop_helper_result.stderr.as_str(),
        scp_stderr.as_str(),
    ]);
    let command = format!(
        "{}\n{}\nscp.exe -i \"{}\" -P {} \"{}\" \"{}\"",
        create_dir_command,
        stop_helper_command,
        key_path.display(),
        port,
        local_path.display(),
        remote
    );

    if output.status.success() {
        emit_optional_remote_setup_log(app, "helper", "info", "Remote helper setup completed.");
    } else {
        emit_optional_remote_setup_log(app, "helper", "stderr", "Remote helper upload failed.");
        emit_optional_remote_setup_output(app, "helper", "stdout", &scp_stdout);
        emit_optional_remote_setup_output(app, "helper", "stderr", &scp_stderr);
    }

    Ok(RemoteHelperSetupResult {
        local_path: local_path.display().to_string(),
        remote_path: remote_path.to_string(),
        command,
        exit_code: output.status.code(),
        success: output.status.success(),
        stdout,
        stderr,
    })
}

fn local_helper_executable_path() -> Result<PathBuf, String> {
    let executable_name = if cfg!(windows) {
        "pzmm-helper.exe"
    } else {
        "pzmm-helper"
    };
    let mut candidates = Vec::new();

    if let Ok(current_exe) = env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            candidates.push(parent.join(executable_name));
        }
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    candidates.push(
        manifest_dir
            .join("target")
            .join("debug")
            .join(executable_name),
    );
    candidates.push(
        manifest_dir
            .join("target")
            .join("release")
            .join(executable_name),
    );

    for candidate in candidates {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    Err("pzmm-helper.exe was not found. Build it with `cargo build --bin pzmm-helper` before using remote workspace helper features.".to_string())
}

fn default_remote_helper_dir(username: &str) -> String {
    let username = username.trim();
    let username = if username.is_empty() {
        "Administrator"
    } else {
        username
    };

    format!("C:\\Users\\{username}\\AppData\\Local\\ZomboidServerModManager\\helper")
}

fn run_ssh_capture(
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
                "Remote command failed while listing workspace data.\n\n{}",
                join_command_output(&[result.stdout.as_str(), result.stderr.as_str()])
            ))
        }
    })
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
            "Remote command failed while listing workspace data.\n\n{}",
            join_command_output(&[result.stdout.as_str(), result.stderr.as_str()])
        ))
    }
}

fn run_ssh_with_stdin(
    connection: &RemoteServerConnectionRequest,
    command_text: &str,
    stdin_text: &str,
) -> Result<TerminalCommandResult, String> {
    if !cfg!(windows) {
        return Err(text(
            "Remote terminal commands are available only on Windows for now.",
            "Comandos remotos no terminal estao disponiveis apenas no Windows por enquanto.",
        )
        .to_string());
    }

    let host = required_field(&connection.host, "host")?;
    let username = required_field(&connection.username, "Windows username")?;
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
    let mut child = Command::new("ssh.exe")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "StrictHostKeyChecking=accept-new",
            "-i",
        ])
        .arg(&key_path)
        .args(["-p", &port.to_string(), &remote, command_text])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Could not run ssh.exe: {error}"))?;

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
        .map_err(|error| format!("Could not wait for ssh.exe: {error}"))?;

    Ok(command_result("remote", command_text, output))
}

fn run_ssh_streaming(
    app: &tauri::AppHandle,
    connection: &RemoteServerConnectionRequest,
    command_text: &str,
    phase: &str,
) -> Result<TerminalCommandResult, String> {
    if !cfg!(windows) {
        return Err(text(
            "Remote terminal commands are available only on Windows for now.",
            "Comandos remotos no terminal estao disponiveis apenas no Windows por enquanto.",
        )
        .to_string());
    }

    let host = required_field(&connection.host, "host")?;
    let username = required_field(&connection.username, "Windows username")?;
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
    let mut child = Command::new("ssh.exe")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "StrictHostKeyChecking=accept-new",
            "-i",
        ])
        .arg(&key_path)
        .args(["-p", &port.to_string(), &remote, command_text])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Could not run ssh.exe: {error}"))?;
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
    if !cfg!(windows) {
        return Err(text(
            "Remote terminal commands are available only on Windows for now.",
            "Comandos remotos no terminal estao disponiveis apenas no Windows por enquanto.",
        )
        .to_string());
    }

    let host = required_field(&connection.host, "host")?;
    let username = required_field(&connection.username, "Windows username")?;
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
    let mut child = Command::new("ssh.exe")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "StrictHostKeyChecking=accept-new",
            "-i",
        ])
        .arg(&key_path)
        .args(["-p", &port.to_string(), &remote, command_text])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Could not run ssh.exe: {error}"))?;
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

fn is_transient_steamcmd_server_install_error(result: &TerminalCommandResult) -> bool {
    let output = format!("{}\n{}", result.stdout, result.stderr).to_lowercase();

    output.contains("missing configuration")
        || output.contains("state is 0x602")
        || output.contains("update state (0x0) unknown")
        || output.contains("update state (0x401) stopping")
}

fn steamcmd_server_install_retry_reason(result: &TerminalCommandResult) -> &'static str {
    let output = format!("{}\n{}", result.stdout, result.stderr).to_lowercase();

    if output.contains("state is 0x602") {
        "Steam app state 0x602"
    } else if output.contains("missing configuration") {
        "Missing configuration"
    } else if output.contains("update state (0x0) unknown") {
        "unknown update state"
    } else if output.contains("update state (0x401) stopping") {
        "stopping update state"
    } else {
        "a transient SteamCMD error"
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
) -> Result<(), String> {
    let host = required_field(&connection.host, "host")?;
    let username = required_field(&connection.username, "Windows username")?;
    let key_path = PathBuf::from(required_field(&connection.ssh_key_path, "SSH key file")?);

    if !key_path.is_file() {
        return Err(format!("SSH key file not found: {}.", key_path.display()));
    }

    let remote = format!("{username}@{host}");
    let output = Command::new("ssh.exe")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "StrictHostKeyChecking=accept-new",
            "-i",
        ])
        .arg(&key_path)
        .args(["-p", &port.to_string(), &remote, "echo pzmm-ready"])
        .output()
        .map_err(|error| format!("Could not run ssh.exe: {error}"))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let details = join_command_output(&[stdout.as_str(), stderr.as_str()]);

    if details.contains("UNPROTECTED PRIVATE KEY FILE")
        || details.contains("bad permissions")
        || details.contains("Permissions for")
    {
        return Err(format!(
            "{}\n\n{}\n\n{}\n{}",
            "SSH refused this private key because its Windows file permissions are too open.",
            "Fix it in PowerShell, then try connecting again:",
            ssh_key_permissions_fix_command(&key_path),
            details
        ));
    }

    Err(if details.is_empty() {
        "SSH authentication failed. Check the username, key file, and the server authorized_keys file."
            .to_string()
    } else {
        format!(
            "SSH authentication failed. Check the username, key file, and the server authorized_keys file.\n\n{details}"
        )
    })
}

fn ssh_key_permissions_fix_command(key_path: &PathBuf) -> String {
    let key_path = key_path.display().to_string();

    format!(
        "icacls \"{key_path}\" /inheritance:r\nicacls \"{key_path}\" /remove \"Users\" \"Authenticated Users\" \"Everyone\" \"CodexSandboxUsers\"\nicacls \"{key_path}\" /grant:r \"$env:USERNAME:R\""
    )
}

fn run_terminal_command_impl(
    request: TerminalCommandRequest,
) -> Result<TerminalCommandResult, String> {
    let command_text = required_field(&request.command, "command")?;
    let runner = command_runner_for(request)?;

    runner.run(&command_text)
}

trait CommandRunner {
    fn target(&self) -> &'static str;
    fn run(&self, command_text: &str) -> Result<TerminalCommandResult, String>;
}

struct LocalCommandRunner {
    working_directory: String,
}

struct SshCommandRunner {
    connection: RemoteServerConnectionRequest,
}

fn command_runner_for(request: TerminalCommandRequest) -> Result<Box<dyn CommandRunner>, String> {
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

impl CommandRunner for LocalCommandRunner {
    fn target(&self) -> &'static str {
        "local"
    }

    fn run(&self, command_text: &str) -> Result<TerminalCommandResult, String> {
        let mut command = if cfg!(windows) {
            let mut command = Command::new("powershell.exe");
            command.args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                command_text,
            ]);
            command
        } else {
            let mut command = Command::new("sh");
            command.args(["-lc", command_text]);
            command
        };

        let working_directory = self.working_directory.trim();
        if !working_directory.is_empty() {
            let working_directory = PathBuf::from(working_directory);
            if !working_directory.is_dir() {
                return Err(format!(
                    "Local working directory not found: {}.",
                    working_directory.display()
                ));
            }
            command.current_dir(working_directory);
        }

        let output = command
            .output()
            .map_err(|error| format!("Could not run the local command: {error}"))?;

        Ok(command_result(self.target(), command_text, output))
    }
}

impl CommandRunner for SshCommandRunner {
    fn target(&self) -> &'static str {
        "remote"
    }

    fn run(&self, command_text: &str) -> Result<TerminalCommandResult, String> {
        if !cfg!(windows) {
            return Err(text(
                "Remote terminal commands are available only on Windows for now.",
                "Comandos remotos no terminal estao disponiveis apenas no Windows por enquanto.",
            )
            .to_string());
        }

        let host = required_field(&self.connection.host, "host")?;
        let username = required_field(&self.connection.username, "Windows username")?;
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
        let output = Command::new("ssh.exe")
            .args([
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=10",
                "-o",
                "StrictHostKeyChecking=accept-new",
                "-i",
            ])
            .arg(&key_path)
            .args(["-p", &port.to_string(), &remote, command_text])
            .output()
            .map_err(|error| format!("Could not run ssh.exe: {error}"))?;

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

fn upload_steamcmd_to_remote_impl(
    app: &tauri::AppHandle,
    request: RemoteSteamCmdUploadRequest,
) -> Result<RemoteSteamCmdUploadResult, String> {
    if !cfg!(windows) {
        return Err(text(
            "Remote SteamCMD upload is available only on Windows for now.",
            "Upload remoto do SteamCMD esta disponivel apenas no Windows por enquanto.",
        )
        .to_string());
    }

    let existing_config =
        get_remote_workspace_config_impl()?.unwrap_or_else(default_remote_workspace_config);
    let connection = request.connection;
    let connection_for_config = connection.clone();
    if connection.auth_method.trim() != "key" {
        return Err(
            "SteamCMD upload currently requires SSH private key authentication.".to_string(),
        );
    }

    let host = required_field(&connection.host, "host")?;
    let username = required_field(&connection.username, "Windows username")?;
    let remote_directory = required_field(&request.remote_directory, "remote SteamCMD folder")?;
    let port = connection
        .port
        .trim()
        .parse::<u16>()
        .map_err(|_| "Enter a valid remote port.".to_string())?;
    let key_path = PathBuf::from(required_field(&connection.ssh_key_path, "SSH key file")?);

    if !key_path.is_file() {
        return Err(format!("SSH key file not found: {}.", key_path.display()));
    }

    if !looks_like_windows_path(&remote_directory) {
        return Err(
            "Use a Windows remote SteamCMD folder inside AppData, for example C:\\Users\\Administrator\\AppData\\Local\\ZomboidServerModManager\\steamcmd-pool\\instance-1."
                .to_string(),
        );
    }

    let steamcmd_zip = steamcmd_zip_resource_path(app)?;
    let remote_path = join_remote_windows_path(&remote_directory, "steamcmd.zip");
    let steamcmd_executable_path = join_remote_windows_path(&remote_directory, "steamcmd.exe");
    let remote_host = format!("{username}@{host}");
    emit_remote_setup_log(
        app,
        "steamcmd",
        "info",
        &format!("Preparing remote folder: {remote_directory}"),
    );
    let create_remote_dir = format!(
        "powershell -NoProfile -ExecutionPolicy Bypass -Command \"New-Item -ItemType Directory -Force -Path '{}' | Out-Null\"",
        quote_powershell_single_string(&remote_directory)
    );
    let mkdir_output = Command::new("ssh.exe")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "StrictHostKeyChecking=accept-new",
            "-i",
        ])
        .arg(&key_path)
        .args(["-p", &port.to_string(), &remote_host, &create_remote_dir])
        .output()
        .map_err(|error| format!("Could not prepare the remote SteamCMD folder: {error}"))?;

    if !mkdir_output.status.success() {
        emit_remote_setup_log(
            app,
            "steamcmd",
            "stderr",
            "Could not create the remote SteamCMD folder.",
        );
        return Ok(RemoteSteamCmdUploadResult {
            local_path: steamcmd_zip.display().to_string(),
            remote_path,
            steamcmd_executable_path,
            command: create_remote_dir,
            exit_code: mkdir_output.status.code(),
            success: false,
            stdout: String::from_utf8_lossy(&mkdir_output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&mkdir_output.stderr).to_string(),
        });
    }

    let remote = format!("{username}@{host}:{remote_path}");
    emit_remote_setup_log(
        app,
        "steamcmd",
        "info",
        &format!("Uploading steamcmd.zip to {remote_path}"),
    );
    let output = Command::new("scp.exe")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "StrictHostKeyChecking=accept-new",
            "-i",
        ])
        .arg(&key_path)
        .args(["-P", &port.to_string()])
        .arg(&steamcmd_zip)
        .arg(&remote)
        .output()
        .map_err(|error| format!("Could not run scp.exe: {error}"))?;

    if !output.status.success() {
        emit_remote_setup_log(app, "steamcmd", "stderr", "SteamCMD upload failed.");
        return Ok(RemoteSteamCmdUploadResult {
            local_path: steamcmd_zip.display().to_string(),
            remote_path,
            steamcmd_executable_path,
            command: format!(
                "scp.exe -i \"{}\" -P {} \"{}\" \"{}\"",
                key_path.display(),
                port,
                steamcmd_zip.display(),
                remote
            ),
            exit_code: output.status.code(),
            success: false,
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    emit_remote_setup_log(
        app,
        "steamcmd",
        "info",
        "Extracting steamcmd.zip on the remote host.",
    );
    let expand_remote_zip = format!(
        "powershell -NoProfile -ExecutionPolicy Bypass -Command \"Expand-Archive -LiteralPath '{}' -DestinationPath '{}' -Force\"",
        quote_powershell_single_string(&remote_path),
        quote_powershell_single_string(&remote_directory)
    );
    let expand_output = Command::new("ssh.exe")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "StrictHostKeyChecking=accept-new",
            "-i",
        ])
        .arg(&key_path)
        .args(["-p", &port.to_string(), &remote_host, &expand_remote_zip])
        .output()
        .map_err(|error| format!("Could not extract SteamCMD on the remote host: {error}"))?;
    let stdout = join_command_output(&[
        String::from_utf8_lossy(&output.stdout).as_ref(),
        String::from_utf8_lossy(&expand_output.stdout).as_ref(),
    ]);
    let stderr = join_command_output(&[
        String::from_utf8_lossy(&output.stderr).as_ref(),
        String::from_utf8_lossy(&expand_output.stderr).as_ref(),
    ]);

    let upload_result = RemoteSteamCmdUploadResult {
        local_path: steamcmd_zip.display().to_string(),
        remote_path: remote_path.clone(),
        steamcmd_executable_path: steamcmd_executable_path.clone(),
        command: format!(
            "scp.exe -i \"{}\" -P {} \"{}\" \"{}\"\n{}",
            key_path.display(),
            port,
            steamcmd_zip.display(),
            remote,
            expand_remote_zip
        ),
        exit_code: expand_output.status.code(),
        success: expand_output.status.success(),
        stdout,
        stderr,
    };

    if upload_result.success {
        emit_remote_setup_log(
            app,
            "steamcmd",
            "info",
            &format!("Saving SteamCMD path: {steamcmd_executable_path}"),
        );
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
            remote_client_ram: existing_config.remote_client_ram,
            remote_server_ram: existing_config.remote_server_ram,
            remote_mod_locations: existing_config.remote_mod_locations,
        })?;
        emit_remote_setup_log(app, "steamcmd", "info", "SteamCMD setup completed.");
    } else {
        emit_remote_setup_log(app, "steamcmd", "stderr", "SteamCMD extraction failed.");
    }

    Ok(upload_result)
}

fn install_zomboid_server_on_remote_impl(
    app: &tauri::AppHandle,
    request: RemoteZomboidServerInstallRequest,
) -> Result<RemoteZomboidServerInstallResult, String> {
    if !cfg!(windows) {
        return Err(text(
            "Remote Project Zomboid server installation is available only on Windows for now.",
            "A instalacao remota do servidor Project Zomboid esta disponivel apenas no Windows por enquanto.",
        )
        .to_string());
    }

    let connection = request.connection;
    if connection.auth_method.trim() != "key" {
        return Err(
            "Remote Project Zomboid server installation currently requires SSH private key authentication."
                .to_string(),
        );
    }

    let steamcmd_path = required_field(&request.steamcmd_path, "remote SteamCMD path")?;
    let install_directory = required_field(
        &request.install_directory,
        "remote Project Zomboid server folder",
    )?;

    if !looks_like_windows_path(&steamcmd_path) {
        return Err("Use a Windows remote SteamCMD executable path inside AppData, for example C:\\Users\\Administrator\\AppData\\Local\\ZomboidServerModManager\\steamcmd-pool\\instance-1\\steamcmd.exe.".to_string());
    }

    if !looks_like_windows_path(&install_directory) {
        return Err("Use a Windows remote Project Zomboid server folder, for example C:\\Users\\Administrator\\AppData\\Local\\ZomboidServerModManager\\zomboid-server.".to_string());
    }

    let start_bat_path = join_remote_windows_path(&install_directory, "StartServer64.bat");
    let fallback_exe_path =
        join_remote_windows_path(&install_directory, "ProjectZomboidServer64.exe");
    let script = build_remote_zomboid_install_script(
        &install_directory,
        &steamcmd_path,
        &start_bat_path,
        &fallback_exe_path,
        false,
        0,
    );
    let command = powershell_encoded_command(&script);
    emit_remote_setup_log(
        app,
        "zomboid-server",
        "info",
        "Starting SteamCMD bootstrap and server download.",
    );
    let mut result = run_ssh_streaming(app, &connection, &command, "zomboid-server")?;
    let mut command_for_result = command.clone();

    for retry_index in 1..=2 {
        if result.success || !is_transient_steamcmd_server_install_error(&result) {
            break;
        }

        let reason = steamcmd_server_install_retry_reason(&result);
        emit_remote_setup_log(
            app,
            "zomboid-server",
            "info",
            &format!(
                "SteamCMD returned {reason}. Waiting a moment, refreshing app info, and retrying ({retry_index}/2)."
            ),
        );
        let retry_script = build_remote_zomboid_install_script(
            &install_directory,
            &steamcmd_path,
            &start_bat_path,
            &fallback_exe_path,
            true,
            retry_index,
        );
        let retry_command = powershell_encoded_command(&retry_script);
        let retry_result = run_ssh_streaming(app, &connection, &retry_command, "zomboid-server")?;
        command_for_result =
            format!("{command_for_result}\n\n# retry {retry_index}\n{retry_command}");
        result = TerminalCommandResult {
            stdout: join_command_output(&[result.stdout.as_str(), retry_result.stdout.as_str()]),
            stderr: join_command_output(&[result.stderr.as_str(), retry_result.stderr.as_str()]),
            ..retry_result
        };
    }

    let server_executable_path =
        extract_remote_server_path(&result.stdout).unwrap_or_else(|| start_bat_path.clone());
    let install_result = RemoteZomboidServerInstallResult {
        install_directory: install_directory.clone(),
        server_executable_path: server_executable_path.clone(),
        command: command_for_result,
        exit_code: result.exit_code,
        success: result.success,
        stdout: result.stdout,
        stderr: result.stderr,
    };

    if install_result.success {
        let existing_config =
            get_remote_workspace_config_impl()?.unwrap_or_else(default_remote_workspace_config);
        write_remote_workspace_config(&RemoteWorkspaceConfig {
            name: connection.name,
            host: connection.host,
            port: connection.port,
            username: connection.username,
            auth_method: connection.auth_method,
            ssh_key_path: connection.ssh_key_path,
            server_path: connection.server_path,
            remote_steamcmd_dir: existing_config.remote_steamcmd_dir,
            remote_steamcmd_path: steamcmd_path,
            remote_zomboid_server_dir: install_directory,
            remote_zomboid_server_path: server_executable_path,
            remote_client_ram: existing_config.remote_client_ram,
            remote_server_ram: existing_config.remote_server_ram,
            remote_mod_locations: existing_config.remote_mod_locations,
        })?;
        emit_remote_setup_log(
            app,
            "zomboid-server",
            "info",
            "Project Zomboid server path saved.",
        );
    }

    Ok(install_result)
}

fn build_remote_zomboid_install_script(
    install_directory: &str,
    steamcmd_path: &str,
    start_bat_path: &str,
    fallback_exe_path: &str,
    refresh_app_info: bool,
    retry_index: u8,
) -> String {
    let app_info_command = if refresh_app_info {
        let wait_seconds = 5 + u16::from(retry_index) * 5;
        format!("Start-Sleep -Seconds {wait_seconds}; & $steamcmd +login anonymous +app_info_update 1 +quit; Start-Sleep -Seconds 2; ")
    } else {
        String::new()
    };

    format!(
        r#"$ErrorActionPreference='Stop'; $ProgressPreference='SilentlyContinue'; $installDir='{}'; $steamcmd='{}'; if (!(Test-Path -LiteralPath $steamcmd -PathType Leaf)) {{ throw "SteamCMD not found: $steamcmd" }}; New-Item -ItemType Directory -Force -Path $installDir | Out-Null; & $steamcmd +quit; {}& $steamcmd +force_install_dir $installDir +login anonymous +app_update 380870 validate +quit; if ($LASTEXITCODE -ne 0) {{ exit $LASTEXITCODE }}; $startPath='{}'; $fallbackPath='{}'; if (Test-Path -LiteralPath $startPath -PathType Leaf) {{ Write-Output "PZMM_SERVER_PATH=$startPath" }} elseif (Test-Path -LiteralPath $fallbackPath -PathType Leaf) {{ Write-Output "PZMM_SERVER_PATH=$fallbackPath" }} else {{ Write-Output "PZMM_SERVER_PATH=$installDir" }}"#,
        quote_powershell_single_string(&install_directory),
        quote_powershell_single_string(&steamcmd_path),
        app_info_command,
        quote_powershell_single_string(&start_bat_path),
        quote_powershell_single_string(&fallback_exe_path),
    )
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
        server_path: "C:\\Users\\Administrator\\Zomboid\\Server".to_string(),
        remote_steamcmd_dir: default_remote_steamcmd_dir(),
        remote_steamcmd_path: String::new(),
        remote_zomboid_server_dir: default_remote_zomboid_server_dir(),
        remote_zomboid_server_path: String::new(),
        remote_client_ram: "4.00".to_string(),
        remote_server_ram: "4.00".to_string(),
        remote_mod_locations: Vec::new(),
    }
}

fn default_remote_steamcmd_dir() -> String {
    "C:\\Users\\Administrator\\AppData\\Local\\ZomboidServerModManager\\steamcmd-pool\\instance-1"
        .to_string()
}

fn default_remote_zomboid_server_dir() -> String {
    "C:\\Users\\Administrator\\AppData\\Local\\ZomboidServerModManager\\zomboid-server".to_string()
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
    value
        .trim()
        .replace('/', "\\")
        .to_lowercase()
        .starts_with("c:\\pzmanager\\")
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

fn join_remote_windows_path(remote_directory: &str, file_name: &str) -> String {
    let directory = remote_directory
        .trim()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string();

    format!("{directory}/{file_name}")
}

fn remote_windows_parent_path(path: &str) -> Option<String> {
    let normalized = path.trim().replace('/', "\\");
    let index = normalized.rfind('\\')?;

    if index == 0 {
        return None;
    }

    Some(normalized[..index].to_string())
}

fn quote_powershell_single_string(value: &str) -> String {
    value.replace('\'', "''")
}

fn powershell_encoded_command(script: &str) -> String {
    let mut bytes = Vec::with_capacity(script.len() * 2);
    for unit in script.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }

    format!(
        "powershell.exe -NoProfile -ExecutionPolicy Bypass -EncodedCommand {}",
        base64_encode(&bytes)
    )
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);

    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        let value = ((first as u32) << 16) | ((second as u32) << 8) | third as u32;

        encoded.push(TABLE[((value >> 18) & 0x3f) as usize] as char);
        encoded.push(TABLE[((value >> 12) & 0x3f) as usize] as char);
        encoded.push(if chunk.len() > 1 {
            TABLE[((value >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            TABLE[(value & 0x3f) as usize] as char
        } else {
            '='
        });
    }

    encoded
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
    Err(text(
        "Automatic file selection is available only on Windows.",
        "Selecao automatica de arquivo esta disponivel apenas no Windows.",
    )
    .to_string())
}
