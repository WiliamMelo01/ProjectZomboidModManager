#![allow(dead_code, unused_imports)]

use base64::Engine;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::{
    collections::HashSet,
    env, fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};

mod models;
mod mods;
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
}

const MANAGED_STEAMCMD_POOL_DIR_NAME: &str = "steamcmd-pool";

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
            mods::install_zomboid_mod(request.package_path, request.mod_id, request.workshop_id)?;
            mods::clear_zomboid_mods_cache_impl()?;
            print_json(&serde_json::json!({ "ok": true }))
        }
        "install-server-map" => {
            let request = read_request::<InstallServerMapRequest>()?;
            servers::install_zomboid_server_map(request.server_id, request.mod_path)?;
            print_json(&serde_json::json!({ "ok": true }))
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
    Ok(())
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

fn zomboid_mods_dir() -> Result<PathBuf, String> {
    let home = user_home_dir()?;
    Ok(home.join("Zomboid").join("mods"))
}

fn zomboid_server_dir() -> Result<PathBuf, String> {
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
