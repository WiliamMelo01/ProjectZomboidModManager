use crate::i18n::text;
use crate::models::{
    AppSettings, ModLocation, RemoteAppSettingsRequest, RemoteHelperSetupResult,
    RemoteModLocationRequest, RemoteServerConnectionRequest, RemoteServerConnectionResult,
    RemoteSetupLogEvent, RemoteSteamCmdUploadRequest, RemoteSteamCmdUploadResult,
    RemoteWorkspaceConfig, RemoteZomboidServerInstallRequest, RemoteZomboidServerInstallResult,
    ServerIniSettings, ServerLuaSetting, ServerLuaSettings, TerminalCommandRequest,
    TerminalCommandResult, WorkshopDownloadEvent, WorkshopDownloadFailedItem,
    WorkshopDownloadLogEvent, WorkshopDownloadResult, ZomboidServer,
};
#[cfg(windows)]
use crate::util::hide_command_window;
use crate::util::{read_ini_value, read_ini_values, read_text_lossy, replace_or_append_ini_value};
use crate::workshop::api::{fetch_steam_workshop_collection_items, validate_workshop_id};
use crate::{app_config_dir, run_blocking, steamcmd_zip_resource_path};
use base64::Engine;
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::hash_map::DefaultHasher,
    collections::{HashMap, HashSet},
    env, fs,
    hash::{Hash, Hasher},
    io::{BufRead, BufReader, Write},
    net::{TcpStream, ToSocketAddrs},
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};
use tauri::Emitter;

const REMOTE_CONNECT_TIMEOUT_SECONDS: u64 = 5;

#[tauri::command]
pub(crate) async fn test_remote_server_connection(
    connection: RemoteServerConnectionRequest,
) -> Result<RemoteServerConnectionResult, String> {
    run_blocking(move || test_remote_server_connection_impl(connection)).await
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
) -> Result<(), String> {
    run_blocking(move || {
        let _value: Value = run_remote_helper_json(
            &connection,
            "install-mod",
            Some(&serde_json::json!({
                "packagePath": package_path,
                "modId": mod_id,
                "workshopId": workshop_id,
            })),
        )?;
        Ok(())
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

    let address = format!("{host}:{port}");
    let mut addresses = address
        .to_socket_addrs()
        .map_err(|error| format!("Could not resolve {host}: {error}"))?;
    let socket_address = addresses
        .next()
        .ok_or_else(|| format!("Could not resolve {host}."))?;

    TcpStream::connect_timeout(
        &socket_address,
        Duration::from_secs(REMOTE_CONNECT_TIMEOUT_SECONDS),
    )
    .map_err(|error| format!("Could not connect to {host}:{port}: {error}"))?;

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
    })
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
        r#"$ErrorActionPreference='Stop'; $serverPath='{}'; $serverDir='{}'; $ramMb={}; if (!(Test-Path -LiteralPath $serverPath)) {{ throw "Remote Project Zomboid server path not found: $serverPath" }}; function Update-Line([string]$line, [int]$ram) {{ $line = [regex]::Replace($line, '-Xms\S+', "-Xms${{ram}}m", 'IgnoreCase'); $line = [regex]::Replace($line, '-Xmx\S+', "-Xmx${{ram}}m", 'IgnoreCase'); if ($line -notmatch '-Xms') {{ $line = "-Xms${{ram}}m $line" }}; if ($line -notmatch '-Xmx') {{ $line = "-Xmx${{ram}}m $line" }}; return $line }}; function Update-Bat([string]$path, [int]$ram) {{ if (!(Test-Path -LiteralPath $path -PathType Leaf)) {{ return $false }}; $content = Get-Content -LiteralPath $path -Raw; if ($content -notmatch '-Xms' -and $content -notmatch '-Xmx') {{ return $false }}; $lines = $content -split "`r?`n" | ForEach-Object {{ if ($_ -match '-Xms|-Xmx') {{ Update-Line $_ $ram }} else {{ $_ }} }}; Set-Content -LiteralPath $path -Value ($lines -join "`r`n") -Encoding UTF8; return $true }}; function Update-Json([string]$path, [int]$ram) {{ if (!(Test-Path -LiteralPath $path -PathType Leaf)) {{ return $false }}; $json = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json; $args = @("-Xms${{ram}}m", "-Xmx${{ram}}m"); if ($json.PSObject.Properties.Name -contains 'vmArgs') {{ if ($json.vmArgs -is [array]) {{ $other = @($json.vmArgs | Where-Object {{ $_ -notmatch '^-Xm[sx]' }}); $json.vmArgs = @($args + $other) }} else {{ $json.vmArgs = (Update-Line ([string]$json.vmArgs) $ram) }} }} else {{ $json | Add-Member -NotePropertyName vmArgs -NotePropertyValue $args }}; $json | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $path -Encoding UTF8; return $true }}; $candidates = @($serverPath, (Join-Path $serverDir 'StartServer64.bat'), (Join-Path $serverDir 'ProjectZomboidServer.bat'), (Join-Path $serverDir 'ProjectZomboidServer64.json')); $updated = $false; foreach ($candidate in $candidates) {{ $ext = [IO.Path]::GetExtension($candidate).ToLowerInvariant(); if ($ext -eq '.bat') {{ $updated = (Update-Bat $candidate $ramMb) -or $updated }} elseif ($ext -eq '.json') {{ $updated = (Update-Json $candidate $ramMb) -or $updated }} }}; if (!$updated) {{ throw "Could not find -Xms/-Xmx settings in remote server launch files." }}; Write-Output "PZMM_REMOTE_PERFORMANCE_UPDATED=$serverDir""#,
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
    let helper_path = ensure_remote_helper(connection)?;
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
        Some(encoded_payload) => {
            run_ssh_capture_with_stdin(connection, &command, &encoded_payload)?
        }
        None => run_ssh_capture(connection, &command)?,
    };
    let stdout = output.stdout.trim();

    if stdout.is_empty() {
        let message = format!("pzmm-helper returned no JSON output for {helper_command}.");
        return Err(join_command_output(&[
            message.as_str(),
            "This usually means the remote helper is outdated, missing, or crashed before writing a response.",
            output.stderr.as_str(),
        ]));
    }

    serde_json::from_str::<T>(stdout).map_err(|error| {
        let message =
            format!("Could not parse pzmm-helper JSON output for {helper_command}: {error}");
        join_command_output(&[message.as_str(), stdout, output.stderr.as_str()])
    })
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
    let stdout = join_command_output(&[
        create_dir_result.stdout.as_str(),
        String::from_utf8_lossy(&output.stdout).as_ref(),
        if output.status.success() {
            "Remote helper upload completed."
        } else {
            ""
        },
    ]);
    let stderr = join_command_output(&[
        create_dir_result.stderr.as_str(),
        String::from_utf8_lossy(&output.stderr).as_ref(),
    ]);
    let command = format!(
        "{}\nscp.exe -i \"{}\" -P {} \"{}\" \"{}\"",
        create_dir_command,
        key_path.display(),
        port,
        local_path.display(),
        remote
    );

    if output.status.success() {
        emit_optional_remote_setup_log(app, "helper", "info", "Remote helper setup completed.");
    } else {
        emit_optional_remote_setup_log(app, "helper", "stderr", "Remote helper upload failed.");
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
