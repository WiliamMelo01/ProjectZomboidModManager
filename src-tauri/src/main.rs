#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::Command;
use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
};
use tauri::Emitter;
#[cfg(windows)]
use tauri::{path::BaseDirectory, Manager};
#[cfg(windows)]
use util::hide_command_window;

rust_i18n::i18n!("locales", fallback = "en");

mod command_runner;
mod game;
mod i18n;
mod models;
mod mods;
mod remote;
mod server_test;
mod servers;
mod settings;
mod util;
mod workshop;

use game::{
    get_system_ram, open_steam_zomboid_folder, scan_zomboid_installation, select_game_executable,
};
use i18n::{
    emit_native_menu, get_language_preference, refresh_native_menu, set_language_preference,
    sync_effective_language,
};
use models::*;
use mods::{
    clear_zomboid_mods_cache, count_zomboid_mods, delete_workshop_mapping, get_workshop_mappings,
    get_zomboid_mod_package_size, install_zomboid_mod, list_zomboid_mods, save_workshop_mapping,
    save_workshop_mappings,
};
use remote::{
    add_remote_mod_location, cancel_remote_steam_workshop_download,
    cancel_remote_zomboid_server_test, check_remote_zomboid_server_firewall,
    check_remote_zomboid_server_status, clear_remote_zomboid_mods_and_images_cache,
    clear_remote_zomboid_mods_cache, configure_remote_zomboid_server_firewall,
    create_remote_zomboid_server, delete_all_remote_data, delete_remote_workspace_config,
    delete_remote_zomboid_server, delete_zomboid_mod_command,
    deploy_local_zomboid_server_to_remote, download_remote_steam_workshop_collection,
    download_remote_steam_workshop_item, download_remote_steam_workshop_items,
    fix_ssh_key_permissions, generate_ssh_public_key, get_remote_app_settings,
    get_remote_mod_locations, get_remote_system_ram, get_remote_workspace_config,
    get_remote_zomboid_server_lua_settings, get_remote_zomboid_server_settings,
    install_remote_zomboid_mod, install_remote_zomboid_server_map,
    install_zomboid_server_on_remote, list_remote_zomboid_mods, list_remote_zomboid_server_logs,
    list_remote_zomboid_servers, open_remote_mod_location, read_remote_zomboid_server_file,
    read_remote_zomboid_server_log_file, run_terminal_command, save_remote_app_settings,
    save_remote_workspace_config, save_remote_zomboid_server_path, select_ssh_key_file,
    send_remote_zomboid_server_command, setup_remote_helper, start_remote_zomboid_server,
    start_remote_zomboid_server_test, stream_remote_zomboid_server_logs,
    test_remote_server_connection, test_remote_server_latency, update_remote_zomboid_server_build,
    update_remote_zomboid_server_lua_settings, update_remote_zomboid_server_mods,
    update_remote_zomboid_server_settings, upload_local_mod_to_remote, upload_steamcmd_to_remote,
    verify_remote_steamcmd_available,
};
use server_test::{
    cancel_zomboid_server_test, check_zomboid_server_ports, kill_processes_by_pid,
    start_zomboid_server_test, test_zomboid_server,
};
use servers::{
    create_zomboid_server, delete_zomboid_server, get_zomboid_server_lua_settings,
    get_zomboid_server_settings, install_zomboid_server_map, list_zomboid_server_logs,
    list_zomboid_servers, open_zomboid_server_file, read_zomboid_server_file,
    read_zomboid_server_log_file, update_zomboid_server_build, update_zomboid_server_lua_settings,
    update_zomboid_server_mods, update_zomboid_server_settings,
};
use settings::{
    add_mod_location, get_app_settings, get_mod_locations, install_linux_steamcmd,
    is_delete_all_enabled, open_mod_location, push_mod_location, save_app_settings,
    select_mod_folder, DEFAULT_MAX_CONCURRENT_DOWNLOADS,
};
use util::*;
use workshop::{
    cancel_steam_workshop_download, download_steam_workshop_collection,
    download_steam_workshop_item, download_steam_workshop_items, open_steam_workshop,
    open_steam_workshop_external, open_steam_workshop_steam_client,
};

#[cfg(windows)]
const MANAGED_STEAMCMD_POOL_DIR_NAME: &str = "steamcmd-pool";
#[cfg(windows)]
const MAX_MANAGED_STEAMCMD_POOL_INSTANCES: usize = 3;

async fn run_blocking<T, F>(task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|error| format!("Falha ao executar tarefa em segundo plano: {error}"))?
}

fn zomboid_server_dir() -> Result<PathBuf, String> {
    let home = env::var_os("USERPROFILE")
        .or_else(|| env::var_os("HOME"))
        .ok_or_else(|| "Nao foi possivel encontrar a pasta do usuario.".to_string())?;

    Ok(PathBuf::from(home).join("Zomboid").join("Server"))
}

const SERVER_EXAMPLE_FILES: [(&str, &str); 3] = [
    (
        "servertest.ini",
        "https://raw.githubusercontent.com/WiliamMelo01/ProjectZomboidModManager/main/resources/server-example/server_example/servertest.ini",
    ),
    (
        "servertest_SandboxVars.lua",
        "https://raw.githubusercontent.com/WiliamMelo01/ProjectZomboidModManager/main/resources/server-example/server_example/servertest_SandboxVars.lua",
    ),
    (
        "servertest_spawnregions.lua",
        "https://raw.githubusercontent.com/WiliamMelo01/ProjectZomboidModManager/main/resources/server-example/server_example/servertest_spawnregions.lua",
    ),
];

fn download_file_from_url(url: &str, destination: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        let mut curl_cmd = Command::new("curl.exe");
        if let Ok(output) = hide_command_window(&mut curl_cmd)
            .args(["-fsSL", url, "-o"])
            .arg(destination)
            .output()
        {
            if output.status.success() {
                return Ok(());
            }
        }

        let mut command = Command::new("powershell.exe");
        let output = hide_command_window(&mut command)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "& { param($u, $d) $ProgressPreference = 'SilentlyContinue'; Invoke-WebRequest -Uri $u -OutFile $d -UseBasicParsing }",
            ])
            .arg(url)
            .arg(destination)
            .output()
            .map_err(|error| format!("Falha ao baixar {url}: {error}"))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    #[cfg(not(windows))]
    {
        let output = Command::new("curl")
            .args(["-fsSL", url, "-o"])
            .arg(destination)
            .output()
            .map_err(|error| format!("Falha ao baixar {url}: {error}"))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }
}

pub(crate) fn ensure_server_example_cache_dir() -> Result<PathBuf, String> {
    let cache_dir = app_config_dir()?.join("server-example");
    fs::create_dir_all(&cache_dir).map_err(|error| {
        format!(
            "Nao foi possivel criar a pasta de cache do servidor de exemplo em {}: {error}",
            cache_dir.display()
        )
    })?;

    for (file_name, url) in SERVER_EXAMPLE_FILES {
        let file_path = cache_dir.join(file_name);
        if !file_path.is_file() || file_path.metadata().map(|m| m.len() == 0).unwrap_or(true) {
            let temp_file = cache_dir.join(format!("{file_name}.download"));
            download_file_from_url(url, &temp_file)?;
            fs::rename(&temp_file, &file_path).map_err(|error| {
                format!(
                    "Nao foi possivel salvar arquivo de exemplo {}: {error}",
                    file_path.display()
                )
            })?;
        }
    }

    Ok(cache_dir)
}

#[tauri::command]
fn is_server_example_cached() -> bool {
    let Ok(cache_dir) = app_config_dir().map(|d| d.join("server-example")) else {
        return false;
    };
    for (file_name, _) in SERVER_EXAMPLE_FILES {
        let file_path = cache_dir.join(file_name);
        if !file_path.is_file() || file_path.metadata().map(|m| m.len() == 0).unwrap_or(true) {
            return false;
        }
    }
    true
}

pub(crate) fn server_example_dir(_app: &tauri::AppHandle) -> Result<PathBuf, String> {
    ensure_server_example_cache_dir()
}


fn zomboid_mods_dir() -> Result<PathBuf, String> {
    let home = env::var_os("USERPROFILE")
        .or_else(|| env::var_os("HOME"))
        .ok_or_else(|| "Nao foi possivel encontrar a pasta do usuario.".to_string())?;

    Ok(PathBuf::from(home).join("Zomboid").join("mods"))
}

fn app_settings_path() -> Result<PathBuf, String> {
    Ok(app_config_dir()?.join("settings.ini"))
}

fn app_config_dir() -> Result<PathBuf, String> {
    #[cfg(not(windows))]
    {
        let home = env::var_os("HOME").ok_or_else(|| {
            "Nao foi possivel encontrar a pasta de configuracoes do usuario.".to_string()
        })?;

        Ok(PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("ZomboidServerModManager"))
    }

    #[cfg(windows)]
    {
        let config_root = env::var_os("LOCALAPPDATA")
            .or_else(|| env::var_os("APPDATA"))
            .or_else(|| env::var_os("USERPROFILE"))
            .ok_or_else(|| {
                "Nao foi possivel encontrar a pasta de configuracoes do usuario.".to_string()
            })?;

        Ok(PathBuf::from(config_root).join("ZomboidServerModManager"))
    }
}

#[cfg(windows)]
fn steamcmd_executable_name() -> &'static str {
    if cfg!(windows) {
        "steamcmd.exe"
    } else {
        "steamcmd"
    }
}

#[cfg(windows)]
fn managed_steamcmd_pool_dir() -> Result<PathBuf, String> {
    Ok(app_config_dir()?.join(MANAGED_STEAMCMD_POOL_DIR_NAME))
}

#[cfg(windows)]
fn managed_steamcmd_pool_instance_dir(instance_id: usize) -> Result<PathBuf, String> {
    Ok(managed_steamcmd_pool_dir()?.join(format!("instance-{instance_id}")))
}

#[cfg(windows)]
fn managed_steamcmd_pool_instance_path(instance_id: usize) -> Result<PathBuf, String> {
    Ok(managed_steamcmd_pool_instance_dir(instance_id)?.join(steamcmd_executable_name()))
}

#[cfg(not(windows))]
fn linux_steamcmd_executable_path() -> Result<PathBuf, String> {
    Ok(app_config_dir()?.join("steamcmd.sh"))
}

#[cfg(not(windows))]
fn linux_steamcmd_command_path() -> Result<PathBuf, String> {
    if let Some(path_var) = env::var_os("PATH") {
        for path_dir in env::split_paths(&path_var) {
            let candidate = path_dir.join("steamcmd");

            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }

    let local_steamcmd = linux_steamcmd_executable_path()?;

    if local_steamcmd.is_file() {
        return Ok(local_steamcmd);
    }

    Err(format!(
        "SteamCMD nao encontrado no PATH nem em {}. Use a aba Downloads para instalar o SteamCMD local do app.",
        local_steamcmd.display()
    ))
}

#[cfg(not(windows))]
fn linux_steamcmd_runtime_dir() -> Result<PathBuf, String> {
    let workshop_dir = linux_steamcmd_workshop_dir()?;
    workshop_dir
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            format!(
                "Nao foi possivel resolver a pasta da Steam a partir de {}.",
                workshop_dir.display()
            )
        })
}

#[cfg(not(windows))]
fn linux_steamcmd_workshop_dir() -> Result<PathBuf, String> {
    Ok(crate::settings::default_steam_workshop_dir())
}

#[cfg(not(windows))]
fn ensure_linux_steamcmd_runtime_layout() -> Result<(), String> {
    let runtime_dir = linux_steamcmd_runtime_dir()?;

    for path in [
        linux_steamcmd_workshop_dir()?,
        runtime_dir.join("downloads"),
        runtime_dir.join("logs"),
    ] {
        fs::create_dir_all(&path).map_err(|error| {
            format!(
                "Nao foi possivel criar a pasta SteamCMD Linux em {}: {error}",
                path.display()
            )
        })?;
    }

    Ok(())
}

fn ensure_managed_steamcmd_pool(
    app: &tauri::AppHandle,
    instance_count: usize,
) -> Result<Vec<PathBuf>, String> {
    #[cfg(not(windows))]
    {
        let _ = app;
        let _ = instance_count;
        ensure_linux_steamcmd_runtime_layout()?;
        Ok(vec![linux_steamcmd_command_path()?])
    }

    #[cfg(windows)]
    {
        let instance_count = instance_count.clamp(1, MAX_MANAGED_STEAMCMD_POOL_INSTANCES);
        let mut steamcmd_paths = Vec::new();

        for instance_id in 1..=instance_count {
            steamcmd_paths.push(ensure_managed_steamcmd_pool_instance(app, instance_id)?);
        }

        Ok(steamcmd_paths)
    }
}

#[cfg(windows)]
fn ensure_managed_steamcmd_pool_instance(
    app: &tauri::AppHandle,
    instance_id: usize,
) -> Result<PathBuf, String> {
    #[cfg(not(windows))]
    {
        let _ = app;
        let _ = instance_id;
        Err("Pool de SteamCMD gerenciado pelo app esta disponivel apenas no Windows.".to_string())
    }

    #[cfg(windows)]
    {
        let steamcmd_path = managed_steamcmd_pool_instance_path(instance_id)?;

        if steamcmd_path.exists() && steamcmd_path.is_file() {
            ensure_managed_steamcmd_pool_instance_layout(&steamcmd_path)?;
            return Ok(steamcmd_path);
        }

        let steamcmd_dir = managed_steamcmd_pool_instance_dir(instance_id)?;
        fs::create_dir_all(&steamcmd_dir).map_err(|error| {
            format!(
                "Nao foi possivel criar a pasta da instancia SteamCMD em {}: {error}",
                steamcmd_dir.display()
            )
        })?;

        let zip_path = steamcmd_zip_resource_path(app)?;
        extract_zip_with_powershell(&zip_path, &steamcmd_dir)?;

        if steamcmd_path.exists() && steamcmd_path.is_file() {
            ensure_managed_steamcmd_pool_instance_layout(&steamcmd_path)?;
            Ok(steamcmd_path)
        } else {
            Err(format!(
                "SteamCMD foi extraido, mas {} nao foi encontrado.",
                steamcmd_path.display()
            ))
        }
    }
}

#[cfg(windows)]
fn ensure_managed_steamcmd_pool_instance_layout(steamcmd_path: &Path) -> Result<(), String> {
    let steamcmd_dir = steamcmd_path.parent().ok_or_else(|| {
        format!(
            "Nao foi possivel resolver a pasta da instancia SteamCMD em {}.",
            steamcmd_path.display()
        )
    })?;

    for path in [
        steamcmd_dir
            .join("steamapps")
            .join("workshop")
            .join("content")
            .join("108600"),
        steamcmd_dir.join("downloads"),
        steamcmd_dir.join("logs"),
    ] {
        fs::create_dir_all(&path).map_err(|error| {
            format!(
                "Nao foi possivel criar a pasta da instancia SteamCMD em {}: {error}",
                path.display()
            )
        })?;
    }

    Ok(())
}

#[cfg(windows)]
fn steamcmd_workshop_dir_from_executable(steamcmd_path: &Path) -> Option<PathBuf> {
    let steamcmd_dir = steamcmd_path.parent()?;

    Some(
        steamcmd_dir
            .join("steamapps")
            .join("workshop")
            .join("content")
            .join("108600"),
    )
}

fn managed_steamcmd_pool_workshop_dirs() -> Vec<PathBuf> {
    #[cfg(not(windows))]
    {
        Vec::new()
    }

    #[cfg(windows)]
    {
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
                steamcmd_workshop_dir_from_executable(&path.join(steamcmd_executable_name()))
            })
            .collect()
    }
}

#[cfg(windows)]
pub(crate) fn steamcmd_zip_resource_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let mut candidates = Vec::new();

    for relative_path in ["steacmd/steamcmd.zip", "steamcmd/steamcmd.zip"] {
        if let Ok(path) = app.path().resolve(relative_path, BaseDirectory::Resource) {
            candidates.push(path);
        }
    }

    if let Ok(current_dir) = env::current_dir() {
        candidates.push(
            current_dir
                .join("resources")
                .join("steacmd")
                .join("steamcmd.zip"),
        );
        candidates.push(
            current_dir
                .join("resources")
                .join("steamcmd")
                .join("steamcmd.zip"),
        );
        candidates.push(
            current_dir
                .join("..")
                .join("resources")
                .join("steacmd")
                .join("steamcmd.zip"),
        );
        candidates.push(
            current_dir
                .join("..")
                .join("resources")
                .join("steamcmd")
                .join("steamcmd.zip"),
        );
    }

    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("resources")
            .join("steacmd")
            .join("steamcmd.zip"),
    );
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("resources")
            .join("steamcmd")
            .join("steamcmd.zip"),
    );

    for candidate in candidates {
        if candidate.exists() && candidate.is_file() {
            return Ok(candidate);
        }
    }

    Err("steamcmd.zip nao encontrado nos resources.".to_string())
}

#[cfg(windows)]
fn extract_zip_with_powershell(zip_path: &Path, target_dir: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        let mut command = Command::new("powershell.exe");
        let output = hide_command_window(&mut command)
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                "& { param($zipPath, $targetDir) Expand-Archive -LiteralPath $zipPath -DestinationPath $targetDir -Force }",
            ])
            .arg(zip_path)
            .arg(target_dir)
            .output()
            .map_err(|error| format!("Nao foi possivel extrair steamcmd.zip: {error}"))?;

        finish_zip_extraction(output)
    }

    #[cfg(not(windows))]
    {
        let output = Command::new("unzip")
            .arg("-o")
            .arg(zip_path)
            .arg("-d")
            .arg(target_dir)
            .output()
            .map_err(|error| format!("Nao foi possivel extrair steamcmd.zip: {error}"))?;

        finish_zip_extraction(output)
    }
}

#[cfg(windows)]
fn finish_zip_extraction(output: std::process::Output) -> Result<(), String> {
    if output.status.success() {
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let details = [stdout.trim(), stderr.trim()]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    Err(if details.is_empty() {
        format!("Falha ao extrair steamcmd.zip: {}", output.status)
    } else {
        format!("Falha ao extrair steamcmd.zip:\n{details}")
    })
}

fn read_config_value(key: &str) -> Result<Option<String>, String> {
    let settings_path = app_settings_path()?;

    if !settings_path.exists() {
        return Ok(None);
    }

    let content = read_text_lossy(&settings_path)?;

    Ok(read_ini_value(&content, key).filter(|value| !value.trim().is_empty()))
}

fn read_saved_mod_locations() -> Result<Vec<ModLocation>, String> {
    let settings_path = app_settings_path()?;

    if !settings_path.exists() {
        return Ok(Vec::new());
    }

    let content = read_text_lossy(&settings_path)?;
    let mut locations = Vec::new();
    let mut seen = HashSet::new();

    for location in read_ini_values(&content, "mod_location") {
        let parts = location.splitn(3, '|').collect::<Vec<_>>();

        if parts.len() < 2 {
            continue;
        }

        let kind = parts[0].trim();
        let path = parts.last().copied().unwrap_or_default().trim();
        let custom_name = Path::new(path).file_name().and_then(|name| name.to_str());
        let label = i18n::mod_location_label(kind, custom_name);

        if kind.is_empty() || path.is_empty() {
            continue;
        }

        push_mod_location(&mut locations, &mut seen, &label, kind, PathBuf::from(path));
    }

    Ok(locations)
}

fn read_saved_custom_mod_locations() -> Result<Vec<ModLocation>, String> {
    Ok(read_saved_mod_locations()?
        .into_iter()
        .filter(|location| location.kind == "custom")
        .collect())
}

fn saved_custom_mod_dirs() -> Result<Vec<PathBuf>, String> {
    Ok(read_saved_custom_mod_locations()?
        .into_iter()
        .map(|location| PathBuf::from(location.path))
        .collect())
}

fn read_steam_library_dirs(libraryfolders_path: &Path) -> Vec<PathBuf> {
    let Ok(content) = read_text_lossy(libraryfolders_path) else {
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

fn main() {
    tauri::Builder::default()
        .on_menu_event(|app, event| emit_native_menu(app, event.id().as_ref()))
        .setup(|app| {
            if let Err(error) = ensure_managed_steamcmd_pool(
                app.handle(),
                DEFAULT_MAX_CONCURRENT_DOWNLOADS as usize,
            ) {
                eprintln!("Nao foi possivel preparar o pool SteamCMD gerenciado: {error}");
            }

            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                let _ = app_handle.emit("server_example_sync_status", "downloading");
                match ensure_server_example_cache_dir() {
                    Ok(_) => {
                        let _ = app_handle.emit("server_example_sync_status", "ready");
                    }
                    Err(error) => {
                        eprintln!("Aviso: Nao foi possivel pre-baixar arquivos de exemplo do servidor: {error}");
                        let _ = app_handle.emit("server_example_sync_status", "error");
                    }
                }
            });

            refresh_native_menu(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_zomboid_servers,
            test_zomboid_server,
            start_zomboid_server_test,
            cancel_zomboid_server_test,
            check_zomboid_server_ports,
            kill_processes_by_pid,
            create_zomboid_server,
            delete_zomboid_server,
            get_zomboid_server_settings,
            get_zomboid_server_lua_settings,
            open_zomboid_server_file,
            read_zomboid_server_file,
            list_zomboid_server_logs,
            read_zomboid_server_log_file,
            update_zomboid_server_build,
            update_zomboid_server_mods,
            update_zomboid_server_settings,
            update_zomboid_server_lua_settings,
            install_zomboid_server_map,
            get_remote_workspace_config,
            get_remote_app_settings,
            get_remote_system_ram,
            get_remote_mod_locations,
            add_remote_mod_location,
            open_remote_mod_location,
            download_remote_steam_workshop_item,
            download_remote_steam_workshop_collection,
            download_remote_steam_workshop_items,
            cancel_remote_steam_workshop_download,
            list_remote_zomboid_mods,
            clear_remote_zomboid_mods_cache,
            clear_remote_zomboid_mods_and_images_cache,
            list_remote_zomboid_servers,
            create_remote_zomboid_server,
            delete_remote_zomboid_server,
            deploy_local_zomboid_server_to_remote,
            upload_local_mod_to_remote,
            get_remote_zomboid_server_settings,
            get_remote_zomboid_server_lua_settings,
            update_remote_zomboid_server_mods,
            update_remote_zomboid_server_build,
            update_remote_zomboid_server_settings,
            update_remote_zomboid_server_lua_settings,
            install_remote_zomboid_mod,
            install_remote_zomboid_server_map,
            run_terminal_command,
            save_remote_app_settings,
            save_remote_workspace_config,
            delete_remote_workspace_config,
            delete_all_remote_data,
            save_remote_zomboid_server_path,
            select_ssh_key_file,
            generate_ssh_public_key,
            fix_ssh_key_permissions,
            test_remote_server_connection,
            test_remote_server_latency,
            start_remote_zomboid_server_test,
            cancel_remote_zomboid_server_test,
            check_remote_zomboid_server_firewall,
            check_remote_zomboid_server_status,
            configure_remote_zomboid_server_firewall,
            send_remote_zomboid_server_command,
            read_remote_zomboid_server_file,
            list_remote_zomboid_server_logs,
            read_remote_zomboid_server_log_file,
            start_remote_zomboid_server,
            stream_remote_zomboid_server_logs,
            setup_remote_helper,
            upload_steamcmd_to_remote,
            verify_remote_steamcmd_available,
            install_zomboid_server_on_remote,
            list_zomboid_mods,
            count_zomboid_mods,
            clear_zomboid_mods_cache,
            get_zomboid_mod_package_size,
            install_zomboid_mod,
            get_workshop_mappings,
            save_workshop_mapping,
            save_workshop_mappings,
            delete_workshop_mapping,
            delete_zomboid_mod_command,
            download_steam_workshop_item,
            download_steam_workshop_collection,
            download_steam_workshop_items,
            cancel_steam_workshop_download,
            get_app_settings,
            get_mod_locations,
            save_app_settings,
            is_delete_all_enabled,
            install_linux_steamcmd,
            select_game_executable,
            get_system_ram,
            scan_zomboid_installation,
            open_steam_zomboid_folder,
            select_mod_folder,
            add_mod_location,
            open_mod_location,
            get_language_preference,
            set_language_preference,
            sync_effective_language,
            open_steam_workshop,
            open_steam_workshop_external,
            open_steam_workshop_steam_client,
            is_server_example_cached
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_example_cache_downloads_and_caches_files() {
        let cache_dir = ensure_server_example_cache_dir().expect("Failed to ensure cache dir");
        assert!(cache_dir.is_dir());

        let ini = cache_dir.join("servertest.ini");
        let sandbox = cache_dir.join("servertest_SandboxVars.lua");
        let spawn = cache_dir.join("servertest_spawnregions.lua");

        assert!(ini.is_file(), "servertest.ini must exist");
        assert!(sandbox.is_file(), "servertest_SandboxVars.lua must exist");
        assert!(spawn.is_file(), "servertest_spawnregions.lua must exist");

        assert!(ini.metadata().unwrap().len() > 0, "servertest.ini must not be empty");
        assert!(sandbox.metadata().unwrap().len() > 0, "SandboxVars must not be empty");
        assert!(spawn.metadata().unwrap().len() > 0, "spawnregions must not be empty");
    }
}
