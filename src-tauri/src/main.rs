#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use base64::{engine::general_purpose, Engine as _};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};
use tauri::{path::BaseDirectory, Manager};

mod models;
mod server_test;
mod util;
mod workshop;

use models::*;
use server_test::{
    check_zomboid_server_ports, kill_processes_by_pid, start_zomboid_server_test,
    test_zomboid_server,
};
use util::*;
use workshop::{
    cancel_steam_workshop_download, download_steam_workshop_collection,
    download_steam_workshop_item, download_steam_workshop_items, open_path_external,
    open_steam_workshop, open_steam_workshop_external, open_steam_workshop_steam_client,
};

const LOCAL_WORKSHOP_ID_FILE: &str = ".pzmm-workshop-id";
const MANAGED_STEAMCMD_DIR_NAME: &str = "steamcmd";

async fn run_blocking<T, F>(task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|error| format!("Falha ao executar tarefa em segundo plano: {error}"))?
}

#[tauri::command]
async fn list_zomboid_servers() -> Result<Vec<ZomboidServer>, String> {
    run_blocking(list_zomboid_servers_impl).await
}

fn list_zomboid_servers_impl() -> Result<Vec<ZomboidServer>, String> {
    let server_dir = zomboid_server_dir()?;

    if !server_dir.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(&server_dir)
        .map_err(|error| format!("Nao foi possivel ler {}: {error}", server_dir.display()))?;

    let mut servers = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();

        if path.extension().and_then(|extension| extension.to_str()) != Some("ini") {
            continue;
        }

        servers.push(read_zomboid_server_from_path(&path)?);
    }

    servers.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));

    Ok(servers)
}

fn read_zomboid_server_from_path(path: &Path) -> Result<ZomboidServer, String> {
    let content = read_text_lossy(path)?;
    let file_stem = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("server")
        .to_string();

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("server.ini")
        .to_string();

    let name = server_display_name(&file_stem, read_ini_value(&content, "PublicName"));
    let port = read_ini_value(&content, "DefaultPort").unwrap_or_else(|| "16261".to_string());
    let max_players = read_ini_value(&content, "MaxPlayers")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0);
    let active_mod_ids = read_ini_value(&content, "Mods")
        .map(|value| split_mod_ids(&value))
        .unwrap_or_default();
    let mods_count = active_mod_ids.len();

    Ok(ZomboidServer {
        id: file_stem,
        name,
        file_name,
        path: path.display().to_string(),
        port,
        max_players,
        mods_count,
        active_mod_ids,
        status: "offline".to_string(),
    })
}

#[tauri::command]
fn update_zomboid_server_mods(
    server_id: String,
    mod_ids: Vec<String>,
    workshop_ids: Vec<String>,
) -> Result<(), String> {
    let server_path = zomboid_server_dir()?.join(format!("{server_id}.ini"));

    if !server_path.exists() {
        return Err(format!(
            "Arquivo do servidor nao encontrado: {}",
            server_path.display()
        ));
    }

    let content = read_text_lossy(&server_path)?;
    let normalized_mods = normalize_server_values(&mod_ids).join(";");
    let normalized_workshop_ids = resolve_server_workshop_ids(&mod_ids, &workshop_ids)?.join(";");
    let updated_content = replace_or_append_ini_value(&content, "Mods", &normalized_mods);
    let updated_content =
        replace_or_append_ini_value(&updated_content, "WorkshopItems", &normalized_workshop_ids);

    fs::write(&server_path, updated_content)
        .map_err(|error| format!("Nao foi possivel salvar {}: {error}", server_path.display()))
}

#[tauri::command]
async fn create_zomboid_server(
    app: tauri::AppHandle,
    name: String,
    mod_ids: Vec<String>,
    workshop_ids: Vec<String>,
) -> Result<ZomboidServer, String> {
    run_blocking(move || create_zomboid_server_impl(&app, &name, &mod_ids, &workshop_ids)).await
}

fn create_zomboid_server_impl(
    app: &tauri::AppHandle,
    name: &str,
    mod_ids: &[String],
    workshop_ids: &[String],
) -> Result<ZomboidServer, String> {
    let name = name.trim();

    if name.is_empty() {
        return Err("Informe um nome para o servidor.".to_string());
    }

    let server_id = sanitize_server_id(name);

    if server_id.is_empty() {
        return Err("Use um nome de servidor com letras ou numeros.".to_string());
    }

    let server_dir = zomboid_server_dir()?;
    fs::create_dir_all(&server_dir)
        .map_err(|error| format!("Nao foi possivel criar {}: {error}", server_dir.display()))?;

    let server_path = server_dir.join(format!("{server_id}.ini"));

    if server_path.exists() {
        return Err(format!("Ja existe um servidor chamado '{server_id}'."));
    }

    let example_dir = server_example_dir(app)?;
    let template_ini = example_dir.join("servertest.ini");
    let template_sandbox = example_dir.join("servertest_SandboxVars.lua");
    let template_spawnregions = example_dir.join("servertest_spawnregions.lua");

    for template in [&template_ini, &template_sandbox, &template_spawnregions] {
        if !template.exists() {
            return Err(format!(
                "Arquivo de exemplo nao encontrado: {}.",
                template.display()
            ));
        }
    }

    let normalized_mod_ids = normalize_server_values(mod_ids);
    let normalized_workshop_ids = resolve_server_workshop_ids(mod_ids, workshop_ids)?;
    let ini_content = read_text_lossy(&template_ini)?;
    let ini_content = replace_or_append_ini_value(&ini_content, "PublicName", name);
    let ini_content =
        replace_or_append_ini_value(&ini_content, "Mods", &normalized_mod_ids.join(";"));
    let ini_content = replace_or_append_ini_value(
        &ini_content,
        "WorkshopItems",
        &normalized_workshop_ids.join(";"),
    );

    fs::write(&server_path, ini_content)
        .map_err(|error| format!("Nao foi possivel salvar {}: {error}", server_path.display()))?;
    fs::copy(
        &template_sandbox,
        server_dir.join(format!("{server_id}_SandboxVars.lua")),
    )
    .map_err(|error| format!("Nao foi possivel copiar SandboxVars: {error}"))?;
    fs::copy(
        &template_spawnregions,
        server_dir.join(format!("{server_id}_spawnregions.lua")),
    )
    .map_err(|error| format!("Nao foi possivel copiar spawnregions: {error}"))?;

    read_zomboid_server_from_path(&server_path)
}

fn server_display_name(file_stem: &str, public_name: Option<String>) -> String {
    if let Some(public_name) = public_name.map(|value| value.trim().to_string()) {
        if !public_name.is_empty() && !public_name.eq_ignore_ascii_case("My PZ Server") {
            return public_name;
        }
    }

    file_stem
        .replace(['_', '-'], " ")
        .split_whitespace()
        .map(capitalize_first_letter)
        .collect::<Vec<_>>()
        .join(" ")
}

#[tauri::command]
async fn list_zomboid_mods() -> Result<Vec<ZomboidMod>, String> {
    run_blocking(list_zomboid_mods_impl).await
}

fn list_zomboid_mods_impl() -> Result<Vec<ZomboidMod>, String> {
    let mut mods = Vec::new();
    let mut seen = HashSet::new();
    let mut installed_mod_ids = HashSet::new();

    if let Ok(local_mods_dir) = zomboid_mods_dir() {
        if local_mods_dir.exists() {
            collect_local_mods(
                &local_mods_dir,
                &mut mods,
                &mut seen,
                &mut installed_mod_ids,
            )?;
        }
    }

    for workshop_dir in steam_workshop_dirs() {
        if workshop_dir.exists() {
            collect_steam_workshop_mods(&workshop_dir, &mut mods, &mut seen, &installed_mod_ids)?;
        }
    }

    for custom_dir in saved_custom_mod_dirs()? {
        if custom_dir.exists() {
            collect_custom_mods(&custom_dir, &mut mods, &mut seen, &installed_mod_ids)?;
        }
    }

    mods.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));

    Ok(mods)
}

#[tauri::command]
async fn count_zomboid_mods() -> Result<usize, String> {
    run_blocking(count_zomboid_mods_impl).await
}

fn count_zomboid_mods_impl() -> Result<usize, String> {
    let mut seen = HashSet::new();
    let mut installed_mod_ids = HashSet::new();

    if let Ok(local_mods_dir) = zomboid_mods_dir() {
        if local_mods_dir.exists() {
            collect_local_mod_ids(&local_mods_dir, &mut seen, &mut installed_mod_ids)?;
        }
    }

    for workshop_dir in steam_workshop_dirs() {
        if workshop_dir.exists() {
            collect_steam_workshop_mod_ids(&workshop_dir, &mut seen, &installed_mod_ids)?;
        }
    }

    for custom_dir in saved_custom_mod_dirs()? {
        if custom_dir.exists() {
            collect_custom_mod_ids(&custom_dir, &mut seen, &installed_mod_ids)?;
        }
    }

    Ok(seen.len())
}

#[tauri::command]
fn install_zomboid_mod(
    mod_path: String,
    mod_id: String,
    workshop_id: String,
) -> Result<(), String> {
    let source = PathBuf::from(&mod_path);

    if !source.exists() || !source.is_dir() {
        return Err(format!("Pasta do mod nao encontrada: {}", source.display()));
    }

    let target_root = zomboid_mods_dir()?;
    fs::create_dir_all(&target_root)
        .map_err(|error| format!("Nao foi possivel criar {}: {error}", target_root.display()))?;

    install_mod(&source, &mod_id, &target_root, Some(&workshop_id))
}

#[tauri::command]
async fn get_app_settings(app: tauri::AppHandle) -> Result<AppSettings, String> {
    run_blocking(move || {
        let _ = ensure_managed_steamcmd(&app);
        load_app_settings()
    })
    .await
}

#[tauri::command]
async fn get_mod_locations() -> Result<Vec<ModLocation>, String> {
    run_blocking(get_mod_locations_impl).await
}

#[tauri::command]
async fn save_app_settings(
    steamcmd_path: String,
    game_executable_path: String,
    client_ram: String,
    server_ram: String,
) -> Result<AppSettings, String> {
    run_blocking(move || {
        save_app_settings_impl(
            &steamcmd_path,
            &game_executable_path,
            &client_ram,
            &server_ram,
        )
    })
    .await
}

#[tauri::command]
async fn detect_steamcmd_path(app: tauri::AppHandle) -> Result<Option<String>, String> {
    run_blocking(move || {
        let _ = ensure_managed_steamcmd(&app);
        Ok(find_steamcmd_path()?.map(|path| path.display().to_string()))
    })
    .await
}

#[tauri::command]
async fn select_steamcmd_path() -> Result<Option<String>, String> {
    run_blocking(select_steamcmd_path_impl).await
}

#[tauri::command]
async fn select_game_executable() -> Result<Option<String>, String> {
    run_blocking(select_game_executable_impl).await
}

#[tauri::command]
async fn get_system_ram() -> Result<u32, String> {
    run_blocking(get_system_ram_impl).await
}

#[tauri::command]
async fn scan_zomboid_installation(
    game_executable_path: Option<String>,
) -> Result<ZomboidInstallationStatus, String> {
    run_blocking(move || scan_zomboid_installation_impl(game_executable_path.as_deref())).await
}

#[tauri::command]
async fn open_steam_zomboid_folder() -> Result<String, String> {
    run_blocking(open_steam_zomboid_folder_impl).await
}

#[tauri::command]
async fn select_mod_folder() -> Result<Option<String>, String> {
    run_blocking(select_mod_folder_impl).await
}

#[tauri::command]
async fn add_mod_location(path: String) -> Result<Vec<ModLocation>, String> {
    run_blocking(move || add_mod_location_impl(&path)).await
}

fn load_app_settings() -> Result<AppSettings, String> {
    let configured_path = read_configured_steamcmd_path()?.unwrap_or_default();
    let resolved_steamcmd_path = find_steamcmd_path()?.map(|path| path.display().to_string());
    let is_steamcmd_configured = resolved_steamcmd_path.is_some();
    let game_executable_path = read_config_value("game_executable_path")?.unwrap_or_default();
    let client_ram = read_config_value("client_ram")?.unwrap_or_else(|| "4.00".to_string());
    let server_ram = read_config_value("server_ram")?.unwrap_or_else(|| "4.00".to_string());

    Ok(AppSettings {
        steamcmd_path: configured_path,
        resolved_steamcmd_path,
        is_steamcmd_configured,
        game_executable_path,
        client_ram,
        server_ram,
    })
}

fn get_mod_locations_impl() -> Result<Vec<ModLocation>, String> {
    let saved_locations = read_saved_mod_locations()?;
    let steamcmd_path = read_configured_steamcmd_path()?
        .or_else(|| {
            find_steamcmd_path()
                .ok()
                .flatten()
                .map(|path| path.display().to_string())
        })
        .unwrap_or_default();
    let mut locations = build_default_mod_locations(Some(&steamcmd_path))?;
    merge_custom_mod_locations(
        &mut locations,
        saved_locations
            .into_iter()
            .filter(|location| location.kind == "custom")
            .collect(),
    );
    let game_executable_path = read_config_value("game_executable_path")?.unwrap_or_default();
    let client_ram = read_config_value("client_ram")?.unwrap_or_else(|| "4.00".to_string());
    let server_ram = read_config_value("server_ram")?.unwrap_or_else(|| "4.00".to_string());
    write_app_settings_file(
        &steamcmd_path,
        &game_executable_path,
        &client_ram,
        &server_ram,
        &locations,
    )?;

    Ok(locations)
}

fn push_mod_location(
    locations: &mut Vec<ModLocation>,
    seen: &mut HashSet<String>,
    label: &str,
    kind: &str,
    path: PathBuf,
) {
    let key = path.display().to_string().to_lowercase();

    if !seen.insert(key) {
        return;
    }

    let exists = path.exists();

    locations.push(ModLocation {
        label: label.to_string(),
        path: path.display().to_string(),
        kind: kind.to_string(),
        exists,
    });
}

fn build_default_mod_locations(steamcmd_path: Option<&str>) -> Result<Vec<ModLocation>, String> {
    let mut locations = Vec::new();
    let mut seen = HashSet::new();

    push_mod_location(
        &mut locations,
        &mut seen,
        "Steam Workshop Project Zomboid",
        "steam",
        default_steam_workshop_dir(),
    );

    push_mod_location(
        &mut locations,
        &mut seen,
        "Mods locais do Zomboid",
        "local",
        zomboid_mods_dir()?,
    );

    if let Some(steamcmd_path) = steamcmd_path.map(str::trim).filter(|path| !path.is_empty()) {
        let steamcmd_path = PathBuf::from(steamcmd_path);

        if let Some(steamcmd_dir) = steamcmd_path.parent() {
            push_mod_location(
                &mut locations,
                &mut seen,
                "Downloads do SteamCMD",
                "steamcmd",
                steamcmd_dir
                    .join("steamapps")
                    .join("workshop")
                    .join("content")
                    .join("108600"),
            );
        }
    }

    Ok(locations)
}

fn merge_custom_mod_locations(
    locations: &mut Vec<ModLocation>,
    custom_locations: Vec<ModLocation>,
) {
    let mut seen = locations
        .iter()
        .map(|location| location.path.to_lowercase())
        .collect::<HashSet<_>>();

    for location in custom_locations {
        if location.kind != "custom" {
            continue;
        }

        let key = location.path.to_lowercase();

        if seen.insert(key) {
            locations.push(location);
        }
    }
}

fn default_steam_workshop_dir() -> PathBuf {
    if let Some(program_files_x86) = env::var_os("ProgramFiles(x86)") {
        return PathBuf::from(program_files_x86)
            .join("Steam")
            .join("steamapps")
            .join("workshop")
            .join("content")
            .join("108600");
    }

    if let Some(program_files) = env::var_os("ProgramFiles") {
        return PathBuf::from(program_files)
            .join("Steam")
            .join("steamapps")
            .join("workshop")
            .join("content")
            .join("108600");
    }

    PathBuf::from(r"C:\Program Files (x86)\Steam")
        .join("steamapps")
        .join("workshop")
        .join("content")
        .join("108600")
}

fn open_steam_zomboid_folder_impl() -> Result<String, String> {
    let Some(zomboid_dir) = steam_zomboid_game_dirs()
        .into_iter()
        .find(|path| path.exists())
    else {
        return Err(
            "Nao encontrei a pasta padrao do Project Zomboid na Steam. Verifique se o jogo esta instalado pela Steam."
                .to_string(),
        );
    };

    open_path_external(&zomboid_dir)?;

    Ok(zomboid_dir.display().to_string())
}

fn scan_zomboid_installation_impl(
    game_executable_path: Option<&str>,
) -> Result<ZomboidInstallationStatus, String> {
    let default_game_dir = steam_zomboid_game_dirs()
        .into_iter()
        .find(|path| path.exists())
        .unwrap_or_else(default_steam_zomboid_game_dir);
    let configured_executable = game_executable_path
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from);
    let detected_executable = configured_executable
        .as_ref()
        .filter(|path| path.exists() && path.is_file())
        .cloned()
        .or_else(|| find_zomboid_executable_in_dir(&default_game_dir));
    let config_dir = detected_executable
        .as_deref()
        .and_then(Path::parent)
        .unwrap_or(default_game_dir.as_path());
    let client_configs = detected_executable
        .as_ref()
        .map(|path| client_config_candidates(path))
        .unwrap_or_else(|| client_config_candidates(&config_dir.join("ProjectZomboid64.exe")));
    let server_configs = server_config_candidates(config_dir);

    Ok(ZomboidInstallationStatus {
        default_game_dir: default_game_dir.display().to_string(),
        detected_executable_path: detected_executable
            .as_ref()
            .map(|path| path.display().to_string()),
        is_game_dir_found: default_game_dir.exists() && default_game_dir.is_dir(),
        is_executable_found: detected_executable.is_some(),
        is_client_config_found: client_configs
            .iter()
            .any(|path| path.exists() && path.is_file()),
        is_server_config_found: server_configs
            .iter()
            .any(|path| path.exists() && path.is_file()),
    })
}

fn steam_zomboid_game_dirs() -> Vec<PathBuf> {
    let mut steamapps_dirs = Vec::new();
    let mut candidates = Vec::new();

    if let Some(program_files_x86) = env::var_os("ProgramFiles(x86)") {
        candidates.push(PathBuf::from(program_files_x86).join("Steam"));
    }

    if let Some(program_files) = env::var_os("ProgramFiles") {
        candidates.push(PathBuf::from(program_files).join("Steam"));
    }

    if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
        candidates.push(PathBuf::from(local_app_data).join("Steam"));
    }

    for steam_dir in candidates {
        let steamapps_dir = steam_dir.join("steamapps");

        if steamapps_dir.exists() {
            steamapps_dirs.push(steamapps_dir.clone());
            steamapps_dirs.extend(read_steam_library_dirs(
                &steamapps_dir.join("libraryfolders.vdf"),
            ));
        }
    }

    dedupe_paths(
        steamapps_dirs
            .into_iter()
            .map(|steamapps_dir| steamapps_dir.join("common").join("ProjectZomboid"))
            .collect(),
    )
}

fn default_steam_zomboid_game_dir() -> PathBuf {
    if let Some(program_files_x86) = env::var_os("ProgramFiles(x86)") {
        return PathBuf::from(program_files_x86)
            .join("Steam")
            .join("steamapps")
            .join("common")
            .join("ProjectZomboid");
    }

    PathBuf::from(r"C:\Program Files (x86)")
        .join("Steam")
        .join("steamapps")
        .join("common")
        .join("ProjectZomboid")
}

fn find_zomboid_executable_in_dir(game_dir: &Path) -> Option<PathBuf> {
    for file_name in [
        "ProjectZomboid64.exe",
        "ProjectZomboid32.exe",
        "ProjectZomboid.exe",
    ] {
        let candidate = game_dir.join(file_name);

        if candidate.exists() && candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

fn save_app_settings_impl(
    steamcmd_path: &str,
    game_executable_path: &str,
    client_ram: &str,
    server_ram: &str,
) -> Result<AppSettings, String> {
    let steamcmd_path = steamcmd_path.trim();
    let game_executable_path = game_executable_path.trim();
    let client_ram = normalize_ram_gb(client_ram)?;
    let server_ram = normalize_ram_gb(server_ram)?;

    if !steamcmd_path.is_empty() {
        validate_steamcmd_path(&PathBuf::from(steamcmd_path))?;
    }

    if !game_executable_path.is_empty() {
        let game_executable = PathBuf::from(game_executable_path);

        validate_game_executable_path(&game_executable)?;
        apply_performance_settings(&game_executable, &client_ram, &server_ram)?;
    }

    let default_steamcmd_path = if steamcmd_path.is_empty() {
        find_steamcmd_path()?
            .map(|path| path.display().to_string())
            .unwrap_or_default()
    } else {
        steamcmd_path.to_string()
    };
    let mut locations = build_default_mod_locations(Some(&default_steamcmd_path))?;
    merge_custom_mod_locations(&mut locations, read_saved_custom_mod_locations()?);
    write_app_settings_file(
        steamcmd_path,
        game_executable_path,
        &client_ram,
        &server_ram,
        &locations,
    )?;

    load_app_settings()
}

fn add_mod_location_impl(path: &str) -> Result<Vec<ModLocation>, String> {
    let path = path.trim();

    if path.is_empty() {
        return Err("Selecione uma pasta de mods.".to_string());
    }

    let path = PathBuf::from(path);

    if !path.exists() {
        return Err(format!("Pasta nao encontrada: {}.", path.display()));
    }

    if !path.is_dir() {
        return Err(format!(
            "O caminho {} nao aponta para uma pasta.",
            path.display()
        ));
    }

    let steamcmd_path = read_configured_steamcmd_path()?.unwrap_or_default();
    let mut locations = build_default_mod_locations(Some(&steamcmd_path))?;
    let mut custom_locations = read_saved_custom_mod_locations()?;
    let label = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .map(|name| format!("Pasta personalizada: {name}"))
        .unwrap_or_else(|| "Pasta personalizada".to_string());

    custom_locations.push(ModLocation {
        label,
        path: path.display().to_string(),
        kind: "custom".to_string(),
        exists: true,
    });
    merge_custom_mod_locations(&mut locations, custom_locations);
    let game_executable_path = read_config_value("game_executable_path")?.unwrap_or_default();
    let client_ram = read_config_value("client_ram")?.unwrap_or_else(|| "4.00".to_string());
    let server_ram = read_config_value("server_ram")?.unwrap_or_else(|| "4.00".to_string());
    write_app_settings_file(
        &steamcmd_path,
        &game_executable_path,
        &client_ram,
        &server_ram,
        &locations,
    )?;

    Ok(locations)
}

fn write_app_settings_file(
    steamcmd_path: &str,
    game_executable_path: &str,
    client_ram: &str,
    server_ram: &str,
    mod_locations: &[ModLocation],
) -> Result<(), String> {
    let settings_path = app_settings_path()?;

    if let Some(settings_dir) = settings_path.parent() {
        fs::create_dir_all(settings_dir).map_err(|error| {
            format!("Nao foi possivel criar {}: {error}", settings_dir.display())
        })?;
    }

    let mut content = format!(
        "steamcmd_path={steamcmd_path}\ngame_executable_path={game_executable_path}\nclient_ram={client_ram}\nserver_ram={server_ram}\n"
    );

    for location in mod_locations {
        content.push_str(&format!(
            "mod_location={}|{}|{}\n",
            location.kind, location.label, location.path
        ));
    }

    fs::write(&settings_path, content).map_err(|error| {
        format!(
            "Nao foi possivel salvar {}: {error}",
            settings_path.display()
        )
    })?;

    Ok(())
}

fn normalize_ram_gb(value: &str) -> Result<String, String> {
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

fn ram_gb_to_mb(value: &str) -> Result<u32, String> {
    let ram = value
        .trim()
        .replace(',', ".")
        .parse::<f64>()
        .map_err(|_| "Informe um valor valido de RAM.".to_string())?;

    Ok((ram * 1024.0).round() as u32)
}

fn validate_game_executable_path(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!(
            "Executavel do Project Zomboid nao encontrado em {}.",
            path.display()
        ));
    }

    if !path.is_file() {
        return Err(format!(
            "O caminho {} nao aponta para um executavel.",
            path.display()
        ));
    }

    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();

    if !extension.eq_ignore_ascii_case("exe") {
        return Err("Selecione um arquivo .exe do Project Zomboid.".to_string());
    }

    Ok(())
}

fn apply_performance_settings(
    game_executable: &Path,
    client_ram: &str,
    server_ram: &str,
) -> Result<(), String> {
    let game_dir = game_executable
        .parent()
        .ok_or_else(|| "Nao foi possivel localizar a pasta do executavel.".to_string())?;

    let client_mb = ram_gb_to_mb(client_ram)?;
    let server_mb = ram_gb_to_mb(server_ram)?;
    let client_configs = client_config_candidates(game_executable);
    let server_configs = server_config_candidates(game_dir);
    let updated_client = update_launcher_configs(&client_configs, client_mb)?;
    let mut updated_server = false;

    for candidate in server_configs {
        if !candidate.exists() || !candidate.is_file() {
            continue;
        }

        let extension = candidate
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_lowercase();

        if extension == "json" {
            update_launcher_json(&candidate, server_mb)?;
            updated_server = true;
        } else if extension == "bat" {
            updated_server = update_launcher_batch(&candidate, server_mb)? || updated_server;
        }
    }

    if !updated_client {
        return Err(format!(
            "Nao encontrei arquivos de configuracao do launcher ao lado de {}.",
            game_executable.display()
        ));
    }

    let _ = updated_server;

    Ok(())
}

fn client_config_candidates(game_executable: &Path) -> Vec<PathBuf> {
    let game_dir = game_executable.parent().unwrap_or_else(|| Path::new(""));
    let mut candidates = Vec::new();

    if let Some(stem) = game_executable.file_stem().and_then(|name| name.to_str()) {
        candidates.push(game_dir.join(format!("{stem}.json")));
        candidates.push(game_dir.join(format!("{stem}.bat")));
    }

    candidates.extend([
        game_dir.join("ProjectZomboid64.json"),
        game_dir.join("ProjectZomboid32.json"),
        game_dir.join("ProjectZomboid64.bat"),
        game_dir.join("ProjectZomboid32.bat"),
    ]);
    dedupe_paths(candidates)
}

fn server_config_candidates(game_dir: &Path) -> Vec<PathBuf> {
    dedupe_paths(vec![game_dir.join("ProjectZomboidServer.bat")])
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();

    paths
        .into_iter()
        .filter(|path| seen.insert(path.display().to_string().to_lowercase()))
        .collect()
}

fn update_launcher_configs(paths: &[PathBuf], ram_mb: u32) -> Result<bool, String> {
    let mut updated_any = false;

    for path in paths {
        if !path.exists() || !path.is_file() {
            continue;
        }

        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_lowercase();

        if extension == "json" {
            update_launcher_json(path, ram_mb)?;
            updated_any = true;
        } else if extension == "bat" {
            updated_any = update_launcher_batch(path, ram_mb)? || updated_any;
        }
    }

    Ok(updated_any)
}

fn update_launcher_json(path: &Path, ram_mb: u32) -> Result<(), String> {
    let content = read_text_lossy(path)?;
    let mut data = serde_json::from_str::<Value>(&content)
        .map_err(|error| format!("Nao foi possivel ler {} como JSON: {error}", path.display()))?;

    match data.get_mut("vmArgs") {
        Some(Value::Array(args)) => update_vm_args_array(args, ram_mb),
        Some(Value::String(args)) => {
            *args = update_vm_args_line(args, ram_mb);
        }
        _ => {
            if let Some(object) = data.as_object_mut() {
                object.insert(
                    "vmArgs".to_string(),
                    Value::Array(vec![
                        Value::String(format!("-Xms{ram_mb}m")),
                        Value::String(format!("-Xmx{ram_mb}m")),
                    ]),
                );
            }
        }
    }

    let content = serde_json::to_string_pretty(&data)
        .map_err(|error| format!("Nao foi possivel gerar JSON atualizado: {error}"))?;
    fs::write(path, format!("{content}\n"))
        .map_err(|error| format!("Nao foi possivel salvar {}: {error}", path.display()))
}

fn update_vm_args_array(args: &mut Vec<Value>, ram_mb: u32) {
    let mut has_xms = false;
    let mut has_xmx = false;

    for arg in args.iter_mut() {
        let Some(value) = arg.as_str() else {
            continue;
        };

        if is_memory_arg(value, "-Xms") {
            *arg = Value::String(format!("-Xms{ram_mb}m"));
            has_xms = true;
        } else if is_memory_arg(value, "-Xmx") {
            *arg = Value::String(format!("-Xmx{ram_mb}m"));
            has_xmx = true;
        }
    }

    if !has_xms {
        args.insert(0, Value::String(format!("-Xms{ram_mb}m")));
    }

    if !has_xmx {
        args.insert(1, Value::String(format!("-Xmx{ram_mb}m")));
    }
}

fn update_launcher_batch(path: &Path, ram_mb: u32) -> Result<bool, String> {
    let content = read_text_lossy(path)?;
    let lower_content = content.to_lowercase();

    if !lower_content.contains("-xms") && !lower_content.contains("-xmx") {
        return Ok(false);
    }

    let updated = content
        .lines()
        .map(|line| {
            let lower_line = line.to_lowercase();

            if lower_line.contains("-xms") || lower_line.contains("-xmx") {
                update_vm_args_line(line, ram_mb)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    fs::write(path, updated)
        .map_err(|error| format!("Nao foi possivel salvar {}: {error}", path.display()))?;

    Ok(true)
}

fn update_vm_args_line(content: &str, ram_mb: u32) -> String {
    let tokens = content
        .split_whitespace()
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    if let Some(java_index) = tokens.iter().position(|token| is_java_command_token(token)) {
        let mut java_index_after_filter = None;
        let mut filtered_tokens = Vec::new();

        for (index, token) in tokens.into_iter().enumerate() {
            if is_memory_arg(&token, "-Xms") || is_memory_arg(&token, "-Xmx") {
                continue;
            }

            if index == java_index {
                java_index_after_filter = Some(filtered_tokens.len());
            }

            filtered_tokens.push(token);
        }

        if let Some(index) = java_index_after_filter {
            filtered_tokens.insert(index + 1, format!("-Xmx{ram_mb}m"));
            filtered_tokens.insert(index + 1, format!("-Xms{ram_mb}m"));

            return filtered_tokens.join(" ");
        }
    }

    let mut has_xms = false;
    let mut has_xmx = false;
    let updated = content
        .split_whitespace()
        .map(|token| {
            if is_memory_arg(token, "-Xms") {
                has_xms = true;
                format!("-Xms{ram_mb}m")
            } else if is_memory_arg(token, "-Xmx") {
                has_xmx = true;
                format!("-Xmx{ram_mb}m")
            } else {
                token.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    match (has_xms, has_xmx) {
        (true, true) => updated,
        (false, true) => format!("-Xms{ram_mb}m {updated}"),
        (true, false) => updated.replace(
            &format!("-Xms{ram_mb}m"),
            &format!("-Xms{ram_mb}m -Xmx{ram_mb}m"),
        ),
        (false, false) => format!("-Xms{ram_mb}m -Xmx{ram_mb}m {updated}"),
    }
}

fn is_memory_arg(value: &str, prefix: &str) -> bool {
    let value = value.trim();

    value.len() > prefix.len()
        && value
            .get(..prefix.len())
            .is_some_and(|current_prefix| current_prefix.eq_ignore_ascii_case(prefix))
}

fn is_java_command_token(value: &str) -> bool {
    let value = value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .replace('/', "\\")
        .to_lowercase();

    value == "java"
        || value == "java.exe"
        || value.ends_with("\\java")
        || value.ends_with("\\java.exe")
}

#[cfg(windows)]
fn select_game_executable_impl() -> Result<Option<String>, String> {
    let script = r#"
Add-Type -AssemblyName System.Windows.Forms
$dialog = New-Object System.Windows.Forms.OpenFileDialog
$dialog.Title = 'Selecionar executavel do Project Zomboid'
$dialog.Filter = 'Project Zomboid (*.exe)|*.exe|Todos os arquivos (*.*)|*.*'
$dialog.CheckFileExists = $true
$dialog.Multiselect = $false
if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
  [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
  Write-Output $dialog.FileName
}
"#;

    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-STA",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ])
        .output()
        .map_err(|error| format!("Nao foi possivel abrir o seletor de arquivos: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        return Err(if stderr.is_empty() {
            "Nao foi possivel selecionar o executavel do Project Zomboid.".to_string()
        } else {
            stderr
        });
    }

    let selected_path = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if selected_path.is_empty() {
        return Ok(None);
    }

    validate_game_executable_path(&PathBuf::from(&selected_path))?;

    Ok(Some(selected_path))
}

#[cfg(not(windows))]
fn select_game_executable_impl() -> Result<Option<String>, String> {
    Err("Selecao de arquivo automatica esta disponivel apenas no Windows.".to_string())
}

#[cfg(windows)]
fn get_system_ram_impl() -> Result<u32, String> {
    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-Command",
            "[math]::Ceiling((Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory / 1GB)",
        ])
        .output()
        .map_err(|error| format!("Nao foi possivel detectar a RAM do sistema: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        return Err(if stderr.is_empty() {
            "Nao foi possivel detectar a RAM do sistema.".to_string()
        } else {
            stderr
        });
    }

    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u32>()
        .map(|ram| ram.max(1))
        .map_err(|_| "Nao foi possivel interpretar a RAM do sistema.".to_string())
}

#[cfg(not(windows))]
fn get_system_ram_impl() -> Result<u32, String> {
    let content = fs::read_to_string("/proc/meminfo").unwrap_or_default();

    for line in content.lines() {
        if !line.starts_with("MemTotal:") {
            continue;
        }

        let kb = line
            .split_whitespace()
            .nth(1)
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);

        if kb > 0 {
            return Ok(((kb as f64 / 1024.0 / 1024.0).ceil() as u32).max(1));
        }
    }

    Ok(16)
}

#[cfg(windows)]
fn select_steamcmd_path_impl() -> Result<Option<String>, String> {
    let script = r#"
Add-Type -AssemblyName System.Windows.Forms
$dialog = New-Object System.Windows.Forms.OpenFileDialog
$dialog.Title = 'Selecionar steamcmd.exe'
$dialog.Filter = 'SteamCMD (steamcmd.exe)|steamcmd.exe|Executaveis (*.exe)|*.exe|Todos os arquivos (*.*)|*.*'
$dialog.CheckFileExists = $true
$dialog.Multiselect = $false
if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
  [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
  Write-Output $dialog.FileName
}
"#;

    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-STA",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ])
        .output()
        .map_err(|error| format!("Nao foi possivel abrir o seletor de arquivos: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        return Err(if stderr.is_empty() {
            "Nao foi possivel selecionar o executavel do SteamCMD.".to_string()
        } else {
            stderr
        });
    }

    let selected_path = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if selected_path.is_empty() {
        return Ok(None);
    }

    validate_steamcmd_path(&PathBuf::from(&selected_path))?;

    Ok(Some(selected_path))
}

#[cfg(not(windows))]
fn select_steamcmd_path_impl() -> Result<Option<String>, String> {
    Err("Selecao de arquivo automatica esta disponivel apenas no Windows.".to_string())
}

#[cfg(windows)]
fn select_mod_folder_impl() -> Result<Option<String>, String> {
    let script = r#"
Add-Type -AssemblyName System.Windows.Forms
$dialog = New-Object System.Windows.Forms.FolderBrowserDialog
$dialog.Description = 'Selecionar pasta com mods do Project Zomboid'
$dialog.ShowNewFolderButton = $false
if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
  [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
  Write-Output $dialog.SelectedPath
}
"#;

    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-STA",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ])
        .output()
        .map_err(|error| format!("Nao foi possivel abrir o seletor de pastas: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        return Err(if stderr.is_empty() {
            "Nao foi possivel selecionar a pasta de mods.".to_string()
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
fn select_mod_folder_impl() -> Result<Option<String>, String> {
    Err("Selecao de pasta automatica esta disponivel apenas no Windows.".to_string())
}

fn install_mod(
    source: &Path,
    mod_id: &str,
    target_root: &Path,
    workshop_id: Option<&str>,
) -> Result<(), String> {
    let folder_name = if mod_id.trim().is_empty() {
        source
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("mod")
            .to_string()
    } else {
        sanitize_folder_name(&mod_id)
    };
    let target = target_root.join(folder_name);

    if !target.exists() {
        copy_dir_recursive(source, &target)?;
    }

    write_local_workshop_id(&target, workshop_id)?;

    Ok(())
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

fn managed_steamcmd_path() -> Result<PathBuf, String> {
    let executable_name = if cfg!(windows) {
        "steamcmd.exe"
    } else {
        "steamcmd"
    };

    Ok(managed_steamcmd_dir()?.join(executable_name))
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
    let output = Command::new("powershell.exe")
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

fn collect_local_mods(
    local_mods_dir: &Path,
    mods: &mut Vec<ZomboidMod>,
    seen: &mut HashSet<String>,
    installed_mod_ids: &mut HashSet<String>,
) -> Result<(), String> {
    let entries = fs::read_dir(local_mods_dir)
        .map_err(|error| format!("Nao foi possivel ler {}: {error}", local_mods_dir.display()))?;

    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let direct_mod_info = path.join("mod.info");

        if direct_mod_info.exists() {
            let workshop_id = read_local_workshop_id(path.as_path());
            if let Some(mod_id) = add_mod_from_info(
                &direct_mod_info,
                workshop_id.as_deref(),
                "local",
                mods,
                seen,
                installed_mod_ids,
            )? {
                installed_mod_ids.insert(mod_id);
            }
            continue;
        }

        for mod_info in find_mod_info_files(&path)? {
            let mod_dir = mod_info.parent().unwrap_or(mod_info.as_path());
            let workshop_id = read_local_workshop_id(mod_dir);

            if let Some(mod_id) = add_mod_from_info(
                &mod_info,
                workshop_id.as_deref(),
                "local",
                mods,
                seen,
                installed_mod_ids,
            )? {
                installed_mod_ids.insert(mod_id);
            }
        }
    }

    Ok(())
}

fn collect_steam_workshop_mods(
    workshop_dir: &Path,
    mods: &mut Vec<ZomboidMod>,
    seen: &mut HashSet<String>,
    installed_mod_ids: &HashSet<String>,
) -> Result<(), String> {
    let workshop_items = fs::read_dir(workshop_dir)
        .map_err(|error| format!("Nao foi possivel ler {}: {error}", workshop_dir.display()))?;

    for item in workshop_items {
        let item = item.map_err(|error| error.to_string())?;
        let item_path = item.path();

        if !item_path.is_dir() {
            continue;
        }

        let workshop_id = item_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();

        if workshop_id.is_empty() || !workshop_id.chars().all(|char| char.is_ascii_digit()) {
            continue;
        }

        let mods_dir = item_path.join("mods");

        if mods_dir.exists() {
            let mod_folders = fs::read_dir(&mods_dir)
                .map_err(|error| format!("Nao foi possivel ler {}: {error}", mods_dir.display()))?;
            let mut found_direct_mod = false;

            for mod_folder in mod_folders {
                let mod_folder = mod_folder.map_err(|error| error.to_string())?;
                let mod_folder_path = mod_folder.path();

                if !mod_folder_path.is_dir() {
                    continue;
                }

                let direct_mod_info = mod_folder_path.join("mod.info");

                if direct_mod_info.exists() {
                    found_direct_mod = true;
                    add_mod_from_info(
                        &direct_mod_info,
                        Some(&workshop_id),
                        "steam",
                        mods,
                        seen,
                        installed_mod_ids,
                    )?;
                }
            }

            if found_direct_mod {
                continue;
            }
        }

        for mod_info in find_mod_info_files(&item_path)? {
            add_mod_from_info(
                &mod_info,
                Some(&workshop_id),
                "steam",
                mods,
                seen,
                installed_mod_ids,
            )?;
        }
    }

    Ok(())
}

fn collect_custom_mods(
    custom_dir: &Path,
    mods: &mut Vec<ZomboidMod>,
    seen: &mut HashSet<String>,
    installed_mod_ids: &HashSet<String>,
) -> Result<(), String> {
    let entries = fs::read_dir(custom_dir)
        .map_err(|error| format!("Nao foi possivel ler {}: {error}", custom_dir.display()))?;

    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let direct_mod_info = path.join("mod.info");

        if direct_mod_info.exists() {
            add_mod_from_info(
                &direct_mod_info,
                None,
                "custom",
                mods,
                seen,
                installed_mod_ids,
            )?;
            continue;
        }

        let workshop_id = path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| name.chars().all(|char| char.is_ascii_digit()));
        let mods_dir = path.join("mods");

        if let Some(workshop_id) = workshop_id {
            if mods_dir.exists() {
                let mod_folders = fs::read_dir(&mods_dir).map_err(|error| {
                    format!("Nao foi possivel ler {}: {error}", mods_dir.display())
                })?;
                let mut found_direct_mod = false;

                for mod_folder in mod_folders {
                    let mod_folder = mod_folder.map_err(|error| error.to_string())?;
                    let mod_folder_path = mod_folder.path();

                    if !mod_folder_path.is_dir() {
                        continue;
                    }

                    let direct_mod_info = mod_folder_path.join("mod.info");

                    if direct_mod_info.exists() {
                        found_direct_mod = true;
                        add_mod_from_info(
                            &direct_mod_info,
                            Some(workshop_id),
                            "custom",
                            mods,
                            seen,
                            installed_mod_ids,
                        )?;
                    }
                }

                if found_direct_mod {
                    continue;
                }
            }
        }

        for mod_info in find_mod_info_files(&path)? {
            add_mod_from_info(
                &mod_info,
                workshop_id,
                "custom",
                mods,
                seen,
                installed_mod_ids,
            )?;
        }
    }

    Ok(())
}

fn collect_local_mod_ids(
    local_mods_dir: &Path,
    seen: &mut HashSet<String>,
    installed_mod_ids: &mut HashSet<String>,
) -> Result<(), String> {
    let entries = fs::read_dir(local_mods_dir)
        .map_err(|error| format!("Nao foi possivel ler {}: {error}", local_mods_dir.display()))?;

    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let direct_mod_info = path.join("mod.info");

        if direct_mod_info.exists() {
            if let Some(mod_id) =
                add_mod_id_from_info(&direct_mod_info, None, "local", seen, installed_mod_ids)?
            {
                installed_mod_ids.insert(mod_id);
            }
            continue;
        }

        for mod_info in find_mod_info_files(&path)? {
            if let Some(mod_id) =
                add_mod_id_from_info(&mod_info, None, "local", seen, installed_mod_ids)?
            {
                installed_mod_ids.insert(mod_id);
            }
        }
    }

    Ok(())
}

fn collect_steam_workshop_mod_ids(
    workshop_dir: &Path,
    seen: &mut HashSet<String>,
    installed_mod_ids: &HashSet<String>,
) -> Result<(), String> {
    let workshop_items = fs::read_dir(workshop_dir)
        .map_err(|error| format!("Nao foi possivel ler {}: {error}", workshop_dir.display()))?;

    for item in workshop_items {
        let item = item.map_err(|error| error.to_string())?;
        let item_path = item.path();

        if !item_path.is_dir() {
            continue;
        }

        let workshop_id = item_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();

        if workshop_id.is_empty() || !workshop_id.chars().all(|char| char.is_ascii_digit()) {
            continue;
        }

        let mods_dir = item_path.join("mods");

        if mods_dir.exists() {
            let mod_folders = fs::read_dir(&mods_dir)
                .map_err(|error| format!("Nao foi possivel ler {}: {error}", mods_dir.display()))?;
            let mut found_direct_mod = false;

            for mod_folder in mod_folders {
                let mod_folder = mod_folder.map_err(|error| error.to_string())?;
                let mod_folder_path = mod_folder.path();

                if !mod_folder_path.is_dir() {
                    continue;
                }

                let direct_mod_info = mod_folder_path.join("mod.info");

                if direct_mod_info.exists() {
                    found_direct_mod = true;
                    add_mod_id_from_info(
                        &direct_mod_info,
                        Some(&workshop_id),
                        "steam",
                        seen,
                        installed_mod_ids,
                    )?;
                }
            }

            if found_direct_mod {
                continue;
            }
        }

        for mod_info in find_mod_info_files(&item_path)? {
            add_mod_id_from_info(
                &mod_info,
                Some(&workshop_id),
                "steam",
                seen,
                installed_mod_ids,
            )?;
        }
    }

    Ok(())
}

fn collect_custom_mod_ids(
    custom_dir: &Path,
    seen: &mut HashSet<String>,
    installed_mod_ids: &HashSet<String>,
) -> Result<(), String> {
    let entries = fs::read_dir(custom_dir)
        .map_err(|error| format!("Nao foi possivel ler {}: {error}", custom_dir.display()))?;

    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let direct_mod_info = path.join("mod.info");

        if direct_mod_info.exists() {
            add_mod_id_from_info(&direct_mod_info, None, "custom", seen, installed_mod_ids)?;
            continue;
        }

        let workshop_id = path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| name.chars().all(|char| char.is_ascii_digit()));
        let mods_dir = path.join("mods");

        if let Some(workshop_id) = workshop_id {
            if mods_dir.exists() {
                let mod_folders = fs::read_dir(&mods_dir).map_err(|error| {
                    format!("Nao foi possivel ler {}: {error}", mods_dir.display())
                })?;
                let mut found_direct_mod = false;

                for mod_folder in mod_folders {
                    let mod_folder = mod_folder.map_err(|error| error.to_string())?;
                    let mod_folder_path = mod_folder.path();

                    if !mod_folder_path.is_dir() {
                        continue;
                    }

                    let direct_mod_info = mod_folder_path.join("mod.info");

                    if direct_mod_info.exists() {
                        found_direct_mod = true;
                        add_mod_id_from_info(
                            &direct_mod_info,
                            Some(workshop_id),
                            "custom",
                            seen,
                            installed_mod_ids,
                        )?;
                    }
                }

                if found_direct_mod {
                    continue;
                }
            }
        }

        for mod_info in find_mod_info_files(&path)? {
            add_mod_id_from_info(&mod_info, workshop_id, "custom", seen, installed_mod_ids)?;
        }
    }

    Ok(())
}

fn add_mod_id_from_info(
    mod_info_path: &Path,
    workshop_id: Option<&str>,
    source: &str,
    seen: &mut HashSet<String>,
    installed_mod_ids: &HashSet<String>,
) -> Result<Option<String>, String> {
    let content = read_text_lossy(mod_info_path)?;
    let mod_id = read_ini_value(content.as_ref(), "id").unwrap_or_else(|| {
        mod_info_path
            .parent()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
            .to_string()
    });
    let normalized_mod_id = mod_id.to_lowercase();

    if source == "steam" && installed_mod_ids.contains(&normalized_mod_id) {
        return Ok(None);
    }

    let workshop_id = workshop_id.unwrap_or("");
    let seen_key = format!("{source}:{workshop_id}:{mod_id}");

    if seen.insert(seen_key) {
        Ok(Some(normalized_mod_id))
    } else {
        Ok(None)
    }
}

fn add_mod_from_info(
    mod_info_path: &Path,
    workshop_id: Option<&str>,
    source: &str,
    mods: &mut Vec<ZomboidMod>,
    seen: &mut HashSet<String>,
    installed_mod_ids: &HashSet<String>,
) -> Result<Option<String>, String> {
    let content = read_text_lossy(mod_info_path)?;
    let mod_id = read_ini_value(content.as_ref(), "id").unwrap_or_else(|| {
        mod_info_path
            .parent()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
            .to_string()
    });
    let normalized_mod_id = mod_id.to_lowercase();

    if source == "steam" {
        if installed_mod_ids.contains(&normalized_mod_id) {
            return Ok(None);
        }
    }

    let workshop_id = workshop_id.unwrap_or("").to_string();
    let seen_key = format!("{source}:{workshop_id}:{mod_id}");

    if seen.contains(&seen_key) {
        return Ok(None);
    }

    seen.insert(seen_key);

    let name = read_ini_value(content.as_ref(), "name")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| capitalize_first_letter(&mod_id));
    let author = read_ini_value(content.as_ref(), "Authors")
        .or_else(|| read_ini_value(content.as_ref(), "author"))
        .unwrap_or_else(|| "Desconhecido".to_string());
    let description = read_ini_value(content.as_ref(), "description")
        .map(|value| clean_mod_description(&value))
        .unwrap_or_else(|| "Sem descricao disponivel.".to_string());
    let version = read_ini_value(content.as_ref(), "version").unwrap_or_else(|| "-".to_string());
    let dependencies = parse_mod_dependencies(content.as_ref());
    let mod_dir = mod_info_path.parent().unwrap_or(mod_info_path);
    let image_url = find_mod_image_url(content.as_ref(), mod_dir);

    mods.push(ZomboidMod {
        id: mod_id,
        name,
        author,
        version,
        workshop_id,
        description,
        size: format_size(directory_size(mod_dir)),
        is_installed: source == "local",
        source: source.to_string(),
        path: mod_dir.display().to_string(),
        image_url,
        dependencies,
    });

    Ok(Some(normalized_mod_id))
}

fn copy_dir_recursive(source: &Path, target: &Path) -> Result<(), String> {
    fs::create_dir_all(target)
        .map_err(|error| format!("Nao foi possivel criar {}: {error}", target.display()))?;

    let entries = fs::read_dir(source)
        .map_err(|error| format!("Nao foi possivel ler {}: {error}", source.display()))?;

    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());

        if source_path.is_dir() {
            copy_dir_recursive(&source_path, &target_path)?;
        } else {
            fs::copy(&source_path, &target_path).map_err(|error| {
                format!(
                    "Nao foi possivel copiar {} para {}: {error}",
                    source_path.display(),
                    target_path.display()
                )
            })?;
        }
    }

    Ok(())
}

fn read_local_workshop_id(mod_dir: &Path) -> Option<String> {
    let workshop_id = fs::read_to_string(mod_dir.join(LOCAL_WORKSHOP_ID_FILE)).ok()?;
    let workshop_id = workshop_id.trim();

    if workshop_id.chars().all(|char| char.is_ascii_digit()) {
        Some(workshop_id.to_string())
    } else {
        None
    }
}

fn write_local_workshop_id(mod_dir: &Path, workshop_id: Option<&str>) -> Result<(), String> {
    let Some(workshop_id) = workshop_id.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };

    if !workshop_id.chars().all(|char| char.is_ascii_digit()) {
        return Ok(());
    }

    let marker_path = mod_dir.join(LOCAL_WORKSHOP_ID_FILE);
    fs::write(&marker_path, format!("{workshop_id}\n"))
        .map_err(|error| format!("Nao foi possivel salvar {}: {error}", marker_path.display()))
}

fn parse_mod_dependencies(content: &str) -> Vec<String> {
    let mut dependencies = Vec::new();
    let mut seen = HashSet::new();

    for value in read_ini_values(content, "require") {
        for dependency_id in split_mod_ids(&value) {
            if seen.insert(dependency_id.to_lowercase()) {
                dependencies.push(dependency_id);
            }
        }
    }

    dependencies
}

fn sanitize_folder_name(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|char| match char {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ => char,
        })
        .collect::<String>();

    if sanitized.trim().is_empty() {
        "mod".to_string()
    } else {
        sanitized
    }
}

fn sanitize_server_id(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|char| {
            if char.is_ascii_alphanumeric() || char == '-' || char == '_' {
                char
            } else if char.is_whitespace() {
                '_'
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

fn normalize_server_values(values: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();

    for value in values {
        let value = value.trim();

        if value.is_empty() {
            continue;
        }

        let key = value.to_lowercase();

        if seen.insert(key) {
            normalized.push(value.to_string());
        }
    }

    normalized
}

fn resolve_server_workshop_ids(
    mod_ids: &[String],
    workshop_ids: &[String],
) -> Result<Vec<String>, String> {
    let mut resolved = normalize_server_values(workshop_ids);
    let mut seen = resolved
        .iter()
        .map(|workshop_id| workshop_id.to_lowercase())
        .collect::<HashSet<_>>();
    let workshop_lookup = build_workshop_id_lookup(mod_ids)?;

    for mod_id in normalize_server_values(mod_ids) {
        if let Some(workshop_id) = workshop_lookup.get(&mod_id.to_lowercase()) {
            if seen.insert(workshop_id.to_lowercase()) {
                resolved.push(workshop_id.clone());
            }
        }
    }

    Ok(resolved)
}

fn build_workshop_id_lookup(mod_ids: &[String]) -> Result<HashMap<String, String>, String> {
    let wanted_mod_ids = normalize_server_values(mod_ids)
        .into_iter()
        .map(|mod_id| mod_id.to_lowercase())
        .collect::<HashSet<_>>();
    let mut lookup = HashMap::new();

    if wanted_mod_ids.is_empty() {
        return Ok(lookup);
    }

    for workshop_dir in steam_workshop_dirs() {
        if workshop_dir.exists() {
            collect_workshop_id_lookup(&workshop_dir, &wanted_mod_ids, &mut lookup)?;
        }
    }

    for custom_dir in saved_custom_mod_dirs()? {
        if custom_dir.exists() {
            collect_workshop_id_lookup(&custom_dir, &wanted_mod_ids, &mut lookup)?;
        }
    }

    Ok(lookup)
}

fn collect_workshop_id_lookup(
    workshop_root: &Path,
    wanted_mod_ids: &HashSet<String>,
    lookup: &mut HashMap<String, String>,
) -> Result<(), String> {
    let entries = fs::read_dir(workshop_root)
        .map_err(|error| format!("Nao foi possivel ler {}: {error}", workshop_root.display()))?;

    for entry in entries {
        if lookup.len() >= wanted_mod_ids.len() {
            break;
        }

        let entry = entry.map_err(|error| error.to_string())?;
        let item_path = entry.path();

        if !item_path.is_dir() {
            continue;
        }

        let workshop_id = item_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();

        if workshop_id.is_empty() || !workshop_id.chars().all(|char| char.is_ascii_digit()) {
            continue;
        }

        let search_root = item_path.join("mods");
        let search_root = if search_root.exists() {
            search_root
        } else {
            item_path
        };

        for mod_info in find_mod_info_files(&search_root)? {
            let content = read_text_lossy(&mod_info)?;
            let mod_id = read_ini_value(content.as_ref(), "id").unwrap_or_else(|| {
                mod_info
                    .parent()
                    .and_then(|path| path.file_name())
                    .and_then(|name| name.to_str())
                    .unwrap_or("unknown")
                    .to_string()
            });
            let normalized_mod_id = mod_id.to_lowercase();

            if wanted_mod_ids.contains(&normalized_mod_id) {
                lookup
                    .entry(normalized_mod_id)
                    .or_insert_with(|| workshop_id.clone());
            }
        }
    }

    Ok(())
}

fn steam_workshop_dirs() -> Vec<PathBuf> {
    let mut steamapps_dirs = Vec::new();
    let mut candidates = Vec::new();

    if let Some(program_files_x86) = env::var_os("ProgramFiles(x86)") {
        candidates.push(PathBuf::from(program_files_x86).join("Steam"));
    }

    if let Some(program_files) = env::var_os("ProgramFiles") {
        candidates.push(PathBuf::from(program_files).join("Steam"));
    }

    if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
        candidates.push(PathBuf::from(local_app_data).join("Steam"));
    }

    for steam_dir in candidates {
        let steamapps_dir = steam_dir.join("steamapps");

        if steamapps_dir.exists() {
            steamapps_dirs.push(steamapps_dir.clone());
            steamapps_dirs.extend(read_steam_library_dirs(
                &steamapps_dir.join("libraryfolders.vdf"),
            ));
        }
    }

    if let Ok(Some(steamcmd_path)) = find_steamcmd_path() {
        if let Some(steamcmd_dir) = steamcmd_path.parent() {
            let steamapps_dir = steamcmd_dir.join("steamapps");

            if steamapps_dir.exists() {
                steamapps_dirs.push(steamapps_dir);
            }
        }
    }

    let mut seen = HashSet::new();

    steamapps_dirs
        .into_iter()
        .filter_map(|steamapps_dir| {
            let workshop_dir = steamapps_dir
                .join("workshop")
                .join("content")
                .join("108600");
            let key = workshop_dir.display().to_string().to_lowercase();

            if seen.insert(key) {
                Some(workshop_dir)
            } else {
                None
            }
        })
        .collect()
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

        if parts.len() != 3 {
            continue;
        }

        let kind = parts[0].trim();
        let label = parts[1].trim();
        let path = parts[2].trim();

        if kind.is_empty() || label.is_empty() || path.is_empty() {
            continue;
        }

        push_mod_location(&mut locations, &mut seen, label, kind, PathBuf::from(path));
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

fn find_mod_info_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    collect_mod_info_files(root, &mut files)?;
    Ok(files)
}

fn collect_mod_info_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(root)
        .map_err(|error| format!("Nao foi possivel ler {}: {error}", root.display()))?;

    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();

        if path.is_dir() {
            collect_mod_info_files(&path, files)?;
        } else if path.file_name().and_then(|name| name.to_str()) == Some("mod.info") {
            files.push(path);
        }
    }

    Ok(())
}

fn find_mod_image_url(content: &str, mod_dir: &Path) -> Option<String> {
    let candidates = read_ini_values(content, "poster")
        .into_iter()
        .chain(read_ini_values(content, "icon"))
        .filter(|value| !value.trim().is_empty());

    for candidate in candidates {
        let image_path = mod_dir.join(candidate);

        if image_path.exists() && image_path.is_file() {
            if let Some(data_url) = image_file_to_data_url(&image_path) {
                return Some(data_url);
            }
        }
    }

    None
}

fn image_file_to_data_url(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    let mime_type = match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        _ => "image/png",
    };
    let encoded = general_purpose::STANDARD.encode(bytes);

    Some(format!("data:{mime_type};base64,{encoded}"))
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            if let Err(error) = ensure_managed_steamcmd(app.handle()) {
                eprintln!("Nao foi possivel preparar o SteamCMD gerenciado: {error}");
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_zomboid_servers,
            test_zomboid_server,
            start_zomboid_server_test,
            check_zomboid_server_ports,
            kill_processes_by_pid,
            create_zomboid_server,
            update_zomboid_server_mods,
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
            open_steam_workshop,
            open_steam_workshop_external,
            open_steam_workshop_steam_client
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
