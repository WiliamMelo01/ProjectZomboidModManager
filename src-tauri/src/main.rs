#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};
use tauri::{path::BaseDirectory, Manager};
use util::hide_command_window;

rust_i18n::i18n!("locales", fallback = "en");

mod game;
mod i18n;
mod models;
mod mods;
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
use mods::{count_zomboid_mods, install_zomboid_mod, list_zomboid_mods};
use server_test::{
    check_zomboid_server_ports, kill_processes_by_pid, start_zomboid_server_test,
    test_zomboid_server,
};
use servers::{
    create_zomboid_server, delete_zomboid_server, get_zomboid_server_lua_settings,
    get_zomboid_server_settings, install_zomboid_server_map, list_zomboid_servers,
    open_zomboid_server_file, update_zomboid_server_build, update_zomboid_server_lua_settings,
    update_zomboid_server_mods, update_zomboid_server_settings,
};
use settings::{
    add_mod_location, detect_steamcmd_path, get_app_settings, get_mod_locations, push_mod_location,
    open_mod_location, save_app_settings, select_mod_folder, select_steamcmd_path,
};
use util::*;
use workshop::{
    cancel_steam_workshop_download, download_steam_workshop_collection,
    download_steam_workshop_item, download_steam_workshop_items, open_steam_workshop,
    open_steam_workshop_external, open_steam_workshop_steam_client,
};

const MANAGED_STEAMCMD_DIR_NAME: &str = "steamcmd";
const MANAGED_STEAMCMD_POOL_DIR_NAME: &str = "steamcmd-pool";
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

fn server_example_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let mut candidates = Vec::new();

    if let Ok(path) = app
        .path()
        .resolve("server-example/server_example", BaseDirectory::Resource)
    {
        candidates.push(path);
    }

    if let Ok(current_dir) = env::current_dir() {
        candidates.push(
            current_dir
                .join("resources")
                .join("server-example")
                .join("server_example"),
        );
        candidates.push(
            current_dir
                .join("..")
                .join("resources")
                .join("server-example")
                .join("server_example"),
        );
    }

    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("resources")
            .join("server-example")
            .join("server_example"),
    );

    for candidate in candidates {
        if candidate.exists() && candidate.is_dir() {
            return Ok(candidate);
        }
    }

    Err("Pasta de exemplo do servidor nao encontrada nos resources.".to_string())
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
    let config_root = env::var_os("LOCALAPPDATA")
        .or_else(|| env::var_os("APPDATA"))
        .or_else(|| env::var_os("USERPROFILE"))
        .or_else(|| env::var_os("HOME"))
        .ok_or_else(|| {
            "Nao foi possivel encontrar a pasta de configuracoes do usuario.".to_string()
        })?;

    Ok(PathBuf::from(config_root).join("ZomboidServerModManager"))
}

fn managed_steamcmd_dir() -> Result<PathBuf, String> {
    Ok(app_config_dir()?.join(MANAGED_STEAMCMD_DIR_NAME))
}

fn steamcmd_executable_name() -> &'static str {
    if cfg!(windows) {
        "steamcmd.exe"
    } else {
        "steamcmd"
    }
}

fn managed_steamcmd_path() -> Result<PathBuf, String> {
    Ok(managed_steamcmd_dir()?.join(steamcmd_executable_name()))
}

fn ensure_managed_steamcmd(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let steamcmd_path = managed_steamcmd_path()?;

    if steamcmd_path.exists() && steamcmd_path.is_file() {
        return Ok(steamcmd_path);
    }

    if !cfg!(windows) {
        return Err("SteamCMD gerenciado pelo app esta disponivel apenas no Windows.".to_string());
    }

    let steamcmd_dir = managed_steamcmd_dir()?;
    fs::create_dir_all(&steamcmd_dir).map_err(|error| {
        format!(
            "Nao foi possivel criar a pasta do SteamCMD em {}: {error}",
            steamcmd_dir.display()
        )
    })?;

    let zip_path = steamcmd_zip_resource_path(app)?;
    extract_zip_with_powershell(&zip_path, &steamcmd_dir)?;

    if steamcmd_path.exists() && steamcmd_path.is_file() {
        Ok(steamcmd_path)
    } else {
        Err(format!(
            "SteamCMD foi extraido, mas {} nao foi encontrado.",
            steamcmd_path.display()
        ))
    }
}

fn managed_steamcmd_pool_dir() -> Result<PathBuf, String> {
    Ok(app_config_dir()?.join(MANAGED_STEAMCMD_POOL_DIR_NAME))
}

fn managed_steamcmd_pool_instance_dir(instance_id: usize) -> Result<PathBuf, String> {
    Ok(managed_steamcmd_pool_dir()?.join(format!("instance-{instance_id}")))
}

fn managed_steamcmd_pool_instance_path(instance_id: usize) -> Result<PathBuf, String> {
    Ok(managed_steamcmd_pool_instance_dir(instance_id)?.join(steamcmd_executable_name()))
}

fn ensure_managed_steamcmd_pool(
    app: &tauri::AppHandle,
    instance_count: usize,
) -> Result<Vec<PathBuf>, String> {
    let instance_count = instance_count.clamp(1, MAX_MANAGED_STEAMCMD_POOL_INSTANCES);
    let mut steamcmd_paths = Vec::new();

    for instance_id in 1..=instance_count {
        steamcmd_paths.push(ensure_managed_steamcmd_pool_instance(app, instance_id)?);
    }

    Ok(steamcmd_paths)
}

fn ensure_managed_steamcmd_pool_instance(
    app: &tauri::AppHandle,
    instance_id: usize,
) -> Result<PathBuf, String> {
    let steamcmd_path = managed_steamcmd_pool_instance_path(instance_id)?;

    if steamcmd_path.exists() && steamcmd_path.is_file() {
        ensure_managed_steamcmd_pool_instance_layout(&steamcmd_path)?;
        return Ok(steamcmd_path);
    }

    if !cfg!(windows) {
        return Err(
            "Pool de SteamCMD gerenciado pelo app esta disponivel apenas no Windows.".to_string(),
        );
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

fn steamcmd_zip_resource_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
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

fn extract_zip_with_powershell(zip_path: &Path, target_dir: &Path) -> Result<(), String> {
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

fn find_steamcmd_path() -> Result<Option<PathBuf>, String> {
    if let Some(path) = read_configured_steamcmd_path()? {
        let path = PathBuf::from(path);

        if path.exists() && path.is_file() {
            return Ok(Some(path));
        }
    }

    if let Some(path) = env::var_os("STEAMCMD_PATH") {
        let path = PathBuf::from(path);

        if path.exists() && path.is_file() {
            return Ok(Some(path));
        }
    }

    if let Ok(path) = managed_steamcmd_path() {
        if path.exists() && path.is_file() {
            return Ok(Some(path));
        }
    }

    let executable_names = if cfg!(windows) {
        vec!["steamcmd.exe", "steamcmd"]
    } else {
        vec!["steamcmd"]
    };

    if let Some(paths) = env::var_os("PATH") {
        for dir in env::split_paths(&paths) {
            for executable_name in &executable_names {
                let candidate = dir.join(executable_name);

                if candidate.exists() && candidate.is_file() {
                    return Ok(Some(candidate));
                }
            }
        }
    }

    let mut candidates = vec![PathBuf::from(r"C:\steamcmd\steamcmd.exe")];

    if let Some(program_files_x86) = env::var_os("ProgramFiles(x86)") {
        candidates.push(
            PathBuf::from(program_files_x86)
                .join("SteamCMD")
                .join("steamcmd.exe"),
        );
    }

    if let Some(program_files) = env::var_os("ProgramFiles") {
        candidates.push(
            PathBuf::from(program_files)
                .join("SteamCMD")
                .join("steamcmd.exe"),
        );
    }

    for candidate in candidates {
        if candidate.exists() && candidate.is_file() {
            return Ok(Some(candidate));
        }
    }

    Ok(None)
}

fn read_configured_steamcmd_path() -> Result<Option<String>, String> {
    read_config_value("steamcmd_path")
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

fn validate_steamcmd_path(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("SteamCMD nao encontrado em {}.", path.display()));
    }

    if !path.is_file() {
        return Err(format!(
            "O caminho {} nao aponta para um arquivo.",
            path.display()
        ));
    }

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_lowercase();

    if file_name != "steamcmd.exe" && file_name != "steamcmd" {
        return Err("Selecione o executavel steamcmd.exe.".to_string());
    }

    Ok(())
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
            if let Err(error) = ensure_managed_steamcmd(app.handle()) {
                eprintln!("Nao foi possivel preparar o SteamCMD gerenciado: {error}");
            }

            refresh_native_menu(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_zomboid_servers,
            test_zomboid_server,
            start_zomboid_server_test,
            check_zomboid_server_ports,
            kill_processes_by_pid,
            create_zomboid_server,
            delete_zomboid_server,
            get_zomboid_server_settings,
            get_zomboid_server_lua_settings,
            open_zomboid_server_file,
            update_zomboid_server_build,
            update_zomboid_server_mods,
            update_zomboid_server_settings,
            update_zomboid_server_lua_settings,
            install_zomboid_server_map,
            list_zomboid_mods,
            count_zomboid_mods,
            install_zomboid_mod,
            download_steam_workshop_item,
            download_steam_workshop_collection,
            download_steam_workshop_items,
            cancel_steam_workshop_download,
            get_app_settings,
            get_mod_locations,
            save_app_settings,
            detect_steamcmd_path,
            select_steamcmd_path,
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
            open_steam_workshop_steam_client
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
