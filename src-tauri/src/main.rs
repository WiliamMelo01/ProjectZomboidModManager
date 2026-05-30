#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use base64::{engine::general_purpose, Engine as _};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    env, fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{mpsc, Mutex, OnceLock},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{path::BaseDirectory, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

const LOCAL_WORKSHOP_ID_FILE: &str = ".pzmm-workshop-id";
const MANAGED_STEAMCMD_DIR_NAME: &str = "steamcmd";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ZomboidServer {
    id: String,
    name: String,
    file_name: String,
    path: String,
    port: String,
    max_players: u32,
    mods_count: usize,
    active_mod_ids: Vec<String>,
    status: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ZomboidMod {
    id: String,
    name: String,
    author: String,
    version: String,
    workshop_id: String,
    description: String,
    size: String,
    is_installed: bool,
    source: String,
    path: String,
    image_url: Option<String>,
    dependencies: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppSettings {
    steamcmd_path: String,
    resolved_steamcmd_path: Option<String>,
    is_steamcmd_configured: bool,
    game_executable_path: String,
    client_ram: String,
    server_ram: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkshopDownloadResult {
    total_items: usize,
    downloaded_items: usize,
    failed_items: Vec<WorkshopDownloadFailedItem>,
    cancelled_items: usize,
    was_cancelled: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct WorkshopDownloadFailedItem {
    workshop_id: String,
    name: String,
    error: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct WorkshopDownloadEvent {
    workshop_id: String,
    name: String,
    status: String,
    error: Option<String>,
}

struct ActiveWorkshopDownload {
    pid: Option<u32>,
    cancel_requested: bool,
}

static ACTIVE_WORKSHOP_DOWNLOAD: OnceLock<Mutex<Option<ActiveWorkshopDownload>>> = OnceLock::new();

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ModLocation {
    label: String,
    path: String,
    kind: String,
    exists: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ZomboidInstallationStatus {
    default_game_dir: String,
    detected_executable_path: Option<String>,
    is_game_dir_found: bool,
    is_executable_found: bool,
    is_client_config_found: bool,
    is_server_config_found: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ServerTestResult {
    status: String,
    summary: String,
    duration_seconds: u64,
    bat_path: String,
    command: String,
    warning_count: usize,
    critical_count: usize,
    log_lines: Vec<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ServerTestStarted {
    server_id: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PortUsage {
    port: u16,
    protocol: String,
    pid: u32,
    process_name: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ServerPortCheck {
    ports: Vec<u16>,
    usages: Vec<PortUsage>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ServerTestEvent {
    server_id: String,
    event: String,
    line: Option<String>,
    result: Option<ServerTestResult>,
    error: Option<String>,
}

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

#[tauri::command]
async fn test_zomboid_server(server_id: String) -> Result<ServerTestResult, String> {
    run_blocking(move || test_zomboid_server_impl(&server_id)).await
}

#[tauri::command]
fn start_zomboid_server_test(
    app: tauri::AppHandle,
    server_id: String,
) -> Result<ServerTestStarted, String> {
    let server_id = server_id.trim().to_string();

    if server_id.is_empty() {
        return Err("Servidor invalido para teste.".to_string());
    }

    let event_server_id = server_id.clone();

    thread::spawn(move || {
        let _ = app.emit(
            "server-test-event",
            ServerTestEvent {
                server_id: event_server_id.clone(),
                event: "started".to_string(),
                line: None,
                result: None,
                error: None,
            },
        );

        let app_for_lines = app.clone();
        let line_server_id = event_server_id.clone();
        let result = test_zomboid_server_impl_with_line_callback(&event_server_id, |line| {
            let _ = app_for_lines.emit(
                "server-test-event",
                ServerTestEvent {
                    server_id: line_server_id.clone(),
                    event: "line".to_string(),
                    line: Some(line.to_string()),
                    result: None,
                    error: None,
                },
            );
        });

        match result {
            Ok(result) => {
                let _ = app.emit(
                    "server-test-event",
                    ServerTestEvent {
                        server_id: event_server_id,
                        event: "finished".to_string(),
                        line: None,
                        result: Some(result),
                        error: None,
                    },
                );
            }
            Err(error) => {
                let _ = app.emit(
                    "server-test-event",
                    ServerTestEvent {
                        server_id: event_server_id,
                        event: "error".to_string(),
                        line: None,
                        result: None,
                        error: Some(error),
                    },
                );
            }
        }
    });

    Ok(ServerTestStarted { server_id })
}

#[tauri::command]
async fn check_zomboid_server_ports(server_id: String) -> Result<ServerPortCheck, String> {
    run_blocking(move || check_zomboid_server_ports_impl(&server_id)).await
}

#[tauri::command]
async fn kill_processes_by_pid(pids: Vec<u32>) -> Result<(), String> {
    run_blocking(move || {
        let mut seen = HashSet::new();

        for pid in pids {
            if pid == 0 || !seen.insert(pid) {
                continue;
            }

            kill_process_tree(pid)?;
        }

        Ok(())
    })
    .await
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
async fn download_steam_workshop_item(
    app: tauri::AppHandle,
    workshop_id: String,
    force_validate: Option<bool>,
) -> Result<WorkshopDownloadResult, String> {
    run_blocking(move || {
        let workshop_id = validate_workshop_id(&workshop_id, "item")?;
        download_steam_workshop_items_impl(&app, vec![workshop_id], force_validate.unwrap_or(false))
    })
    .await
}

#[tauri::command]
async fn download_steam_workshop_collection(
    app: tauri::AppHandle,
    collection_id: String,
    force_validate: Option<bool>,
) -> Result<WorkshopDownloadResult, String> {
    run_blocking(move || {
        let workshop_ids = fetch_steam_workshop_collection_items(&collection_id)?;
        download_steam_workshop_items_impl(&app, workshop_ids, force_validate.unwrap_or(false))
    })
    .await
}

#[tauri::command]
async fn download_steam_workshop_items(
    app: tauri::AppHandle,
    workshop_ids: Vec<String>,
    force_validate: Option<bool>,
) -> Result<WorkshopDownloadResult, String> {
    run_blocking(move || {
        download_steam_workshop_items_impl(&app, workshop_ids, force_validate.unwrap_or(false))
    })
    .await
}

#[tauri::command]
async fn cancel_steam_workshop_download() -> Result<(), String> {
    run_blocking(cancel_steam_workshop_download_impl).await
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

#[tauri::command]
fn open_steam_workshop(app: tauri::AppHandle, item_id_or_search: String) -> Result<(), String> {
    open_steam_workshop_impl(&app, &item_id_or_search)
}

#[tauri::command]
fn open_steam_workshop_external(item_id_or_search: String) -> Result<(), String> {
    open_steam_workshop_external_impl(&item_id_or_search)
}

#[tauri::command]
fn open_steam_workshop_steam_client(item_id_or_search: String) -> Result<(), String> {
    open_steam_workshop_steam_client_impl(&item_id_or_search)
}

fn download_steam_workshop_items_impl(
    app: &tauri::AppHandle,
    workshop_ids: Vec<String>,
    force_validate: bool,
) -> Result<WorkshopDownloadResult, String> {
    let _guard = begin_workshop_download()?;
    let steamcmd_path = match find_steamcmd_path()? {
        Some(path) => path,
        None => ensure_managed_steamcmd(app)?,
    };
    let workshop_ids = normalize_server_values(&workshop_ids);
    let total_items = workshop_ids.len();

    if workshop_ids.is_empty() {
        return Err("Informe ao menos um item da Steam Workshop para baixar.".to_string());
    }

    for workshop_id in &workshop_ids {
        emit_workshop_download_event(app, workshop_id, "queued", None);
    }

    let first_pass = run_steamcmd_workshop_pass(app, &steamcmd_path, &workshop_ids, force_validate)?;
    let was_cancelled = workshop_download_was_cancelled();
    let mut failed_items = first_pass.failed_items;

    if !was_cancelled && !failed_items.is_empty() {
        let retry_ids = failed_items.keys().cloned().collect::<Vec<_>>();

        for workshop_id in &retry_ids {
            emit_workshop_download_event(app, workshop_id, "retrying", None);
        }

        failed_items = run_steamcmd_workshop_pass(app, &steamcmd_path, &retry_ids, force_validate)?
            .failed_items;
    }

    let was_cancelled = workshop_download_was_cancelled();
    let cancelled_items = if was_cancelled {
        total_items.saturating_sub(first_pass.completed_items.len())
    } else {
        0
    };
    let failed_items = enrich_workshop_download_failures(failed_items);

    if was_cancelled {
        for workshop_id in &workshop_ids {
            if !first_pass.completed_items.contains(workshop_id) {
                emit_workshop_download_event(app, workshop_id, "cancelled", None);
            }
        }
    }

    Ok(WorkshopDownloadResult {
        total_items,
        downloaded_items: if was_cancelled {
            first_pass.completed_items.len()
        } else {
            total_items.saturating_sub(failed_items.len())
        },
        failed_items,
        cancelled_items,
        was_cancelled,
    })
}

struct WorkshopDownloadPassResult {
    completed_items: HashSet<String>,
    failed_items: HashMap<String, String>,
}

struct WorkshopDownloadGuard;

impl Drop for WorkshopDownloadGuard {
    fn drop(&mut self) {
        if let Ok(mut active_download) = workshop_download_state().lock() {
            *active_download = None;
        }
    }
}

fn workshop_download_state() -> &'static Mutex<Option<ActiveWorkshopDownload>> {
    ACTIVE_WORKSHOP_DOWNLOAD.get_or_init(|| Mutex::new(None))
}

fn begin_workshop_download() -> Result<WorkshopDownloadGuard, String> {
    let mut active_download = workshop_download_state()
        .lock()
        .map_err(|_| "Nao foi possivel acessar o estado dos downloads.".to_string())?;

    if active_download.is_some() {
        return Err("Ja existe um download da Steam Workshop em andamento.".to_string());
    }

    *active_download = Some(ActiveWorkshopDownload {
        pid: None,
        cancel_requested: false,
    });

    Ok(WorkshopDownloadGuard)
}

fn cancel_steam_workshop_download_impl() -> Result<(), String> {
    let pid = {
        let mut active_download = workshop_download_state()
            .lock()
            .map_err(|_| "Nao foi possivel acessar o estado dos downloads.".to_string())?;
        let Some(active_download) = active_download.as_mut() else {
            return Ok(());
        };

        active_download.cancel_requested = true;
        active_download.pid
    };

    if let Some(pid) = pid {
        kill_process_tree(pid)?;
    }

    Ok(())
}

fn workshop_download_was_cancelled() -> bool {
    workshop_download_state()
        .lock()
        .ok()
        .and_then(|active_download| {
            active_download
                .as_ref()
                .map(|active_download| active_download.cancel_requested)
        })
        .unwrap_or(false)
}

fn set_active_workshop_download_pid(pid: Option<u32>) {
    if let Ok(mut active_download) = workshop_download_state().lock() {
        if let Some(active_download) = active_download.as_mut() {
            active_download.pid = pid;
        }
    }
}

fn run_steamcmd_workshop_pass(
    app: &tauri::AppHandle,
    steamcmd_path: &Path,
    workshop_ids: &[String],
    force_validate: bool,
) -> Result<WorkshopDownloadPassResult, String> {
    let script_path = create_steamcmd_workshop_script(workshop_ids, force_validate)?;
    let mut command = Command::new(steamcmd_path);

    if let Some(steamcmd_dir) = steamcmd_path.parent() {
        command.current_dir(steamcmd_dir);
    }

    let child_result = command
        .args(["+runscript", &script_path.display().to_string()])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn();
    let mut child = match child_result {
        Ok(child) => child,
        Err(error) => {
            let _ = fs::remove_file(&script_path);
            return Err(format!(
                "Nao foi possivel executar {}: {error}",
                steamcmd_path.display()
            ));
        }
    };
    set_active_workshop_download_pid(Some(child.id()));

    let (sender, receiver) = mpsc::channel::<String>();
    spawn_output_reader(child.stdout.take(), "OUT", sender.clone());
    spawn_output_reader(child.stderr.take(), "ERR", sender);

    let wanted_ids = workshop_ids.iter().cloned().collect::<HashSet<_>>();
    let mut completed_items = HashSet::new();
    let mut failed_items = HashMap::new();
    let mut log_lines = Vec::new();

    loop {
        while let Ok(line) = receiver.try_recv() {
            process_steamcmd_workshop_line(
                app,
                &line,
                &wanted_ids,
                &mut completed_items,
                &mut failed_items,
            );
            log_lines.push(line);
        }

        if child
            .try_wait()
            .map_err(|error| format!("Nao foi possivel consultar o SteamCMD: {error}"))?
            .is_some()
        {
            break;
        }

        thread::sleep(Duration::from_millis(100));
    }

    set_active_workshop_download_pid(None);
    let _ = fs::remove_file(&script_path);

    while let Ok(line) = receiver.try_recv() {
        process_steamcmd_workshop_line(
            app,
            &line,
            &wanted_ids,
            &mut completed_items,
            &mut failed_items,
        );
        log_lines.push(line);
    }

    if workshop_download_was_cancelled() {
        return Ok(WorkshopDownloadPassResult {
            completed_items,
            failed_items,
        });
    }

    for workshop_id in workshop_ids {
        if failed_items.contains_key(workshop_id) {
            continue;
        }

        if completed_items.insert(workshop_id.clone()) {
            emit_workshop_download_event(app, workshop_id, "completed", None);
        }
    }

    Ok(WorkshopDownloadPassResult {
        completed_items,
        failed_items,
    })
}

fn create_steamcmd_workshop_script(
    workshop_ids: &[String],
    force_validate: bool,
) -> Result<PathBuf, String> {
    let mut lines = vec![
        "@ShutdownOnFailedCommand 0".to_string(),
        "@NoPromptForPassword 1".to_string(),
        "login anonymous".to_string(),
    ];

    for workshop_id in workshop_ids {
        let workshop_id = validate_workshop_id(workshop_id, "item")?;
        let validate = if force_validate { " validate" } else { "" };
        lines.push(format!("workshop_download_item 108600 {workshop_id}{validate}"));
    }

    lines.push("quit".to_string());
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let script_path = env::temp_dir().join(format!("pzmm-steamcmd-{timestamp}.txt"));

    fs::write(&script_path, lines.join("\r\n"))
        .map_err(|error| format!("Nao foi possivel criar o script temporario do SteamCMD: {error}"))?;

    Ok(script_path)
}

fn process_steamcmd_workshop_line(
    app: &tauri::AppHandle,
    line: &str,
    wanted_ids: &HashSet<String>,
    completed_items: &mut HashSet<String>,
    failed_items: &mut HashMap<String, String>,
) {
    let normalized_line = line.to_lowercase();
    let Some(workshop_id) = find_workshop_id_in_line(line, wanted_ids) else {
        return;
    };

    if steamcmd_workshop_line_status(&normalized_line) == Some("completed") {
        failed_items.remove(&workshop_id);
        if completed_items.insert(workshop_id.clone()) {
            emit_workshop_download_event(app, &workshop_id, "completed", None);
        }
    } else if steamcmd_workshop_line_status(&normalized_line) == Some("failed") {
        failed_items.insert(workshop_id.clone(), line.to_string());
        emit_workshop_download_event(app, &workshop_id, "failed", Some(line.to_string()));
    } else if steamcmd_workshop_line_status(&normalized_line) == Some("downloading") {
        emit_workshop_download_event(app, &workshop_id, "downloading", None);
    }
}

fn steamcmd_workshop_line_status(normalized_line: &str) -> Option<&'static str> {
    if normalized_line.contains("success") || normalized_line.contains("downloaded item") {
        Some("completed")
    } else if normalized_line.contains("error") || normalized_line.contains("failed") {
        Some("failed")
    } else if normalized_line.contains("download") {
        Some("downloading")
    } else {
        None
    }
}

fn find_workshop_id_in_line(line: &str, wanted_ids: &HashSet<String>) -> Option<String> {
    line.split(|char: char| !char.is_ascii_digit())
        .find(|value| wanted_ids.contains(*value))
        .map(ToString::to_string)
}

fn emit_workshop_download_event(
    app: &tauri::AppHandle,
    workshop_id: &str,
    status: &str,
    error: Option<String>,
) {
    let _ = app.emit(
        "workshop-download-event",
        WorkshopDownloadEvent {
            workshop_id: workshop_id.to_string(),
            name: format!("Workshop item {workshop_id}"),
            status: status.to_string(),
            error,
        },
    );
}

fn enrich_workshop_download_failures(
    failures: HashMap<String, String>,
) -> Vec<WorkshopDownloadFailedItem> {
    let details = fetch_steam_workshop_item_names(&failures.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    let mut failures = failures
        .into_iter()
        .map(|(workshop_id, error)| WorkshopDownloadFailedItem {
            name: details
                .get(&workshop_id)
                .cloned()
                .unwrap_or_else(|| format!("Workshop item {workshop_id}")),
            workshop_id,
            error,
        })
        .collect::<Vec<_>>();

    failures.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    failures
}

fn fetch_steam_workshop_item_names(workshop_ids: &[String]) -> Result<HashMap<String, String>, String> {
    if workshop_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut body = format!("itemcount = '{}'; ", workshop_ids.len());

    for (index, workshop_id) in workshop_ids.iter().enumerate() {
        let workshop_id = validate_workshop_id(workshop_id, "item")?;
        body.push_str(&format!("'publishedfileids[{index}]' = '{workshop_id}'; "));
    }

    let script = format!(
        "$ErrorActionPreference = 'Stop'; \
         $body = @{{ {body} }}; \
         $response = Invoke-RestMethod -Method Post \
           -Uri 'https://api.steampowered.com/ISteamRemoteStorage/GetPublishedFileDetails/v1/' \
           -Body $body; \
         $response | ConvertTo-Json -Depth 8 -Compress"
    );
    let response = run_powershell_json_request(&script, "consultar os detalhes dos mods")?;
    let mut names = HashMap::new();

    if let Some(items) = response
        .get("response")
        .and_then(|value| value.get("publishedfiledetails"))
        .and_then(Value::as_array)
    {
        for item in items {
            if let (Some(workshop_id), Some(name)) = (
                item.get("publishedfileid").and_then(Value::as_str),
                item.get("title").and_then(Value::as_str),
            ) {
                names.insert(workshop_id.to_string(), name.to_string());
            }
        }
    }

    Ok(names)
}

fn validate_workshop_id(value: &str, item_label: &str) -> Result<String, String> {
    let value = value.trim();

    if value.is_empty() || !value.chars().all(|char| char.is_ascii_digit()) {
        return Err(format!(
            "Informe um Workshop ID numerico para a {item_label}."
        ));
    }

    Ok(value.to_string())
}

fn fetch_steam_workshop_collection_items(collection_id: &str) -> Result<Vec<String>, String> {
    let collection_id = validate_workshop_id(collection_id, "colecao")?;
    let script = format!(
        "$ErrorActionPreference = 'Stop'; \
         $body = @{{ collectioncount = '1'; 'publishedfileids[0]' = '{collection_id}' }}; \
         $response = Invoke-RestMethod -Method Post \
           -Uri 'https://api.steampowered.com/ISteamRemoteStorage/GetCollectionDetails/v1/' \
           -Body $body; \
         $response | ConvertTo-Json -Depth 8 -Compress"
    );
    let response = run_powershell_json_request(&script, "consultar a colecao na Steam")?;
    let children = response
        .get("response")
        .and_then(|value| value.get("collectiondetails"))
        .and_then(Value::as_array)
        .and_then(|collections| collections.first())
        .and_then(|collection| collection.get("children"))
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "A Steam nao encontrou itens nessa colecao. Confirme se o ID pertence a uma colecao publica."
                .to_string()
        })?;
    let mut seen = HashSet::new();
    let workshop_ids = children
        .iter()
        .filter_map(|child| child.get("publishedfileid").and_then(Value::as_str))
        .filter(|workshop_id| workshop_id.chars().all(|char| char.is_ascii_digit()))
        .filter(|workshop_id| seen.insert((*workshop_id).to_string()))
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    if workshop_ids.is_empty() {
        return Err("A colecao informada nao possui itens para baixar.".to_string());
    }

    Ok(workshop_ids)
}

fn run_powershell_json_request(script: &str, action: &str) -> Result<Value, String> {
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .map_err(|error| format!("Nao foi possivel {action}: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        let details = stderr.trim();
        return Err(if details.is_empty() {
            format!("Nao foi possivel {action}.")
        } else {
            format!("Nao foi possivel {action}:\n{details}")
        });
    }

    serde_json::from_str(&stdout)
        .map_err(|error| format!("A Steam retornou uma resposta invalida ao tentar {action}: {error}"))
}

fn open_steam_workshop_impl(app: &tauri::AppHandle, item_id_or_search: &str) -> Result<(), String> {
    let value = item_id_or_search.trim();

    if value.is_empty() {
        return Err("Informe o ID ou nome da dependencia para abrir a Steam Workshop.".to_string());
    }

    let url = build_steam_workshop_url(value);
    let app_url = PathBuf::from(format!(
        "index.html#/workshop?target={}&url={}",
        encode_url_query(value),
        encode_url_query(&url)
    ));

    if let Some(window) = app.get_webview_window("steam-workshop") {
        window.close().map_err(|error| {
            format!("Nao foi possivel atualizar a janela da Steam Workshop: {error}")
        })?;
    }

    WebviewWindowBuilder::new(app, "steam-workshop", WebviewUrl::App(app_url))
        .title("Steam Workshop")
        .inner_size(760.0, 560.0)
        .resizable(true)
        .build()
        .map_err(|error| format!("Nao foi possivel abrir a Steam Workshop no app: {error}"))?;

    Ok(())
}

fn open_steam_workshop_external_impl(item_id_or_search: &str) -> Result<(), String> {
    let value = item_id_or_search.trim();

    if value.is_empty() {
        return Err("Informe o ID ou nome da dependencia para abrir a Steam Workshop.".to_string());
    }

    open_url_external(&build_steam_workshop_url(value))
}

fn open_steam_workshop_steam_client_impl(item_id_or_search: &str) -> Result<(), String> {
    let value = item_id_or_search.trim();

    if value.is_empty() {
        return Err("Informe o ID ou nome da dependencia para abrir a Steam Workshop.".to_string());
    }

    open_url_external(&format!(
        "steam://openurl/{}",
        build_steam_workshop_url(value)
    ))
}

fn open_url_external(url: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        Command::new("rundll32.exe")
            .args(["url.dll,FileProtocolHandler", url])
            .spawn()
            .map_err(|error| format!("Nao foi possivel abrir o navegador: {error}"))?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|error| format!("Nao foi possivel abrir o navegador: {error}"))?;
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|error| format!("Nao foi possivel abrir o navegador: {error}"))?;
        return Ok(());
    }
}

fn open_path_external(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        Command::new("explorer.exe")
            .arg(path)
            .spawn()
            .map_err(|error| format!("Nao foi possivel abrir o Explorer: {error}"))?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|error| format!("Nao foi possivel abrir a pasta: {error}"))?;
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|error| format!("Nao foi possivel abrir a pasta: {error}"))?;
        return Ok(());
    }
}

fn build_steam_workshop_url(value: &str) -> String {
    if value.chars().all(|char| char.is_ascii_digit()) {
        format!("https://steamcommunity.com/sharedfiles/filedetails/?id={value}")
    } else {
        format!(
            "https://steamcommunity.com/workshop/browse/?appid=108600&searchtext={}",
            encode_url_query(value)
        )
    }
}

fn encode_url_query(value: &str) -> String {
    let mut encoded = String::new();

    for byte in value.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(*byte as char)
            }
            b' ' => encoded.push('+'),
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }

    encoded
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
        .or_else(|| find_steamcmd_path().ok().flatten().map(|path| path.display().to_string()))
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
    let Some(zomboid_dir) = steam_zomboid_game_dirs().into_iter().find(|path| path.exists()) else {
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
        is_client_config_found: client_configs.iter().any(|path| path.exists() && path.is_file()),
        is_server_config_found: server_configs.iter().any(|path| path.exists() && path.is_file()),
    })
}

fn test_zomboid_server_impl(server_id: &str) -> Result<ServerTestResult, String> {
    test_zomboid_server_impl_with_line_callback(server_id, |_| {})
}

fn test_zomboid_server_impl_with_line_callback<F>(
    server_id: &str,
    mut on_line: F,
) -> Result<ServerTestResult, String>
where
    F: FnMut(&str),
{
    const TEST_TIMEOUT: Duration = Duration::from_secs(180);

    let server_id = server_id.trim();

    if server_id.is_empty() {
        return Ok(server_test_setup_error(
            "Servidor invalido para teste.",
            Path::new("ProjectZomboidServer.bat"),
            "",
            0,
        ));
    }

    let server_path = zomboid_server_dir()?.join(format!("{server_id}.ini"));

    if !server_path.exists() {
        return Ok(server_test_setup_error(
            &format!("Arquivo do servidor nao encontrado: {}.", server_path.display()),
            Path::new("ProjectZomboidServer.bat"),
            "",
            0,
        ));
    }

    if let Some(dependency_result) = validate_server_mod_dependencies(server_id, &server_path)? {
        return Ok(dependency_result);
    }

    let Some(game_dir) = resolve_zomboid_game_dir()? else {
        return Ok(server_test_setup_error(
            "Pasta do Project Zomboid nao encontrada. Configure o executavel do jogo nas configuracoes.",
            Path::new("ProjectZomboidServer.bat"),
            "",
            0,
        ));
    };
    let bat_path = game_dir.join("ProjectZomboidServer.bat");
    let mut command = format!(
        "cmd.exe /C call \"{}\" -servername {}",
        bat_path.display(),
        server_id
    );

    if !bat_path.exists() || !bat_path.is_file() {
        return Ok(server_test_setup_error(
            &format!("ProjectZomboidServer.bat nao encontrado em {}.", game_dir.display()),
            &bat_path,
            &command,
            0,
        ));
    }

    let test_bat_path = create_server_test_batch(&game_dir, &bat_path, server_id)?;
    command = format!("cmd.exe /C call \"{}\"", test_bat_path.display());
    let started_at = Instant::now();
    let mut child = Command::new("cmd.exe")
        .arg("/C")
        .arg("call")
        .arg(&test_bat_path)
        .current_dir(&game_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
        .map_err(|error| format!("Nao foi possivel iniciar o teste do servidor: {error}"))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (sender, receiver) = mpsc::channel::<String>();

    spawn_output_reader(stdout, "OUT", sender.clone());
    spawn_output_reader(stderr, "ERR", sender);

    let mut log_lines = Vec::new();
    let mut process_exited = false;
    let mut server_started = false;

    while started_at.elapsed() < TEST_TIMEOUT {
        while let Ok(line) = receiver.try_recv() {
            if is_server_started_line(&line.to_lowercase()) {
                server_started = true;
            }
            on_line(&line);
            log_lines.push(line);
        }

        if server_started {
            break;
        }

        if child
            .try_wait()
            .map_err(|error| format!("Nao foi possivel consultar o processo do servidor: {error}"))?
            .is_some()
        {
            process_exited = true;
            break;
        }

        thread::sleep(Duration::from_millis(200));
    }

    while let Ok(line) = receiver.try_recv() {
        if is_server_started_line(&line.to_lowercase()) {
            server_started = true;
        }
        on_line(&line);
        log_lines.push(line);
    }

    if !process_exited {
        if let Some(pid) = child.id().checked_into() {
            let _ = kill_process_tree(pid);
        } else {
            let _ = child.kill();
        }
        let _ = child.wait();
    }
    let _ = fs::remove_file(&test_bat_path);

    thread::sleep(Duration::from_millis(200));

    while let Ok(line) = receiver.try_recv() {
        if is_server_started_line(&line.to_lowercase()) {
            server_started = true;
        }
        on_line(&line);
        log_lines.push(line);
    }

    let duration_seconds = started_at.elapsed().as_secs();
    let critical_lines = if server_started {
        Vec::new()
    } else {
        find_critical_server_lines(&log_lines)
    };
    let warning_count = count_warning_server_lines(&log_lines);
    let status = if critical_lines.is_empty() {
        "passed"
    } else {
        "failed"
    };
    let summary = if server_started {
        "Servidor iniciado com sucesso: rede ativa e porta escutando. O teste foi encerrado automaticamente.".to_string()
    } else if let Some(network_error_summary) = summarize_known_server_error(&log_lines) {
        network_error_summary
    } else if critical_lines.is_empty() {
        if warning_count == 0 {
            "Teste rapido concluido em 180s: nenhuma falha critica detectada nos logs capturados.".to_string()
        } else {
            format!(
                "Teste rapido concluido em 180s: nenhuma falha critica detectada. Foram capturados {warning_count} aviso(s)."
            )
        }
    } else {
        format!(
            "Teste encontrou {} linha(s) com possiveis falhas criticas.",
            critical_lines.len()
        )
    };

    Ok(ServerTestResult {
        status: status.to_string(),
        summary,
        duration_seconds,
        bat_path: bat_path.display().to_string(),
        command,
        warning_count,
        critical_count: critical_lines.len(),
        log_lines: tail_log_lines(log_lines, 240),
    })
}

trait CheckedIntoU32 {
    fn checked_into(self) -> Option<u32>;
}

impl CheckedIntoU32 for u32 {
    fn checked_into(self) -> Option<u32> {
        Some(self)
    }
}

fn resolve_zomboid_game_dir() -> Result<Option<PathBuf>, String> {
    if let Some(game_executable_path) = read_config_value("game_executable_path")? {
        let executable = PathBuf::from(game_executable_path);

        if let Some(game_dir) = executable.parent() {
            if game_dir.exists() && game_dir.is_dir() {
                return Ok(Some(game_dir.to_path_buf()));
            }
        }
    }

    Ok(steam_zomboid_game_dirs()
        .into_iter()
        .find(|path| path.exists() && path.is_dir()))
}

fn validate_server_mod_dependencies(
    server_id: &str,
    server_path: &Path,
) -> Result<Option<ServerTestResult>, String> {
    let content = read_text_lossy(server_path)?;
    let active_mod_ids = read_ini_value(&content, "Mods")
        .map(|value| split_mod_ids(&value))
        .unwrap_or_default();

    if active_mod_ids.is_empty() {
        return Ok(None);
    }

    let active_positions = active_mod_ids
        .iter()
        .enumerate()
        .map(|(index, mod_id)| (mod_id.to_lowercase(), index))
        .collect::<HashMap<_, _>>();
    let mods_by_id = list_zomboid_mods_impl()?
        .into_iter()
        .map(|zomboid_mod| (zomboid_mod.id.to_lowercase(), zomboid_mod))
        .collect::<HashMap<_, _>>();
    let mut issues = Vec::new();

    for (mod_index, mod_id) in active_mod_ids.iter().enumerate() {
        let normalized_mod_id = mod_id.to_lowercase();
        let Some(zomboid_mod) = mods_by_id.get(&normalized_mod_id) else {
            issues.push(format!(
                "[ERR] Mod '{mod_id}' esta ativo em {server_id}, mas nao foi encontrado nas bibliotecas locais."
            ));
            continue;
        };

        for dependency_id in &zomboid_mod.dependencies {
            let normalized_dependency_id = dependency_id.to_lowercase();
            let Some(dependency_index) = active_positions.get(&normalized_dependency_id) else {
                issues.push(format!(
                    "[ERR] Mod '{mod_id}' requer '{dependency_id}', mas essa dependencia nao esta ativa no servidor."
                ));
                continue;
            };

            if *dependency_index > mod_index {
                issues.push(format!(
                    "[ERR] Ordem invalida: '{mod_id}' esta antes de sua dependencia '{dependency_id}'. Coloque '{dependency_id}' antes de '{mod_id}' em Mods=."
                ));
            }
        }
    }

    if issues.is_empty() {
        return Ok(None);
    }

    Ok(Some(ServerTestResult {
        status: "failed".to_string(),
        summary: format!(
            "Validacao de dependencias encontrou {} problema(s) antes de iniciar o servidor.",
            issues.len()
        ),
        duration_seconds: 0,
        bat_path: "ProjectZomboidServer.bat".to_string(),
        command: "preflight: validar dependencias e ordem de Mods=".to_string(),
        warning_count: 0,
        critical_count: issues.len(),
        log_lines: tail_log_lines(issues, 240),
    }))
}

fn check_zomboid_server_ports_impl(server_id: &str) -> Result<ServerPortCheck, String> {
    let ports = server_ports_for_id(server_id)?;
    let usages = find_port_usages(&ports)?;

    Ok(ServerPortCheck { ports, usages })
}

fn server_ports_for_id(server_id: &str) -> Result<Vec<u16>, String> {
    let server_id = server_id.trim();
    let server_path = zomboid_server_dir()?.join(format!("{server_id}.ini"));

    if !server_path.exists() {
        return Ok(vec![16261, 16262]);
    }

    let content = read_text_lossy(&server_path)?;
    let default_port = read_ini_value(&content, "DefaultPort")
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(16261);
    let udp_port = read_ini_value(&content, "UDPPort")
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(default_port.saturating_add(1));
    let mut ports = vec![default_port, udp_port];

    ports.sort_unstable();
    ports.dedup();

    Ok(ports)
}

fn find_port_usages(ports: &[u16]) -> Result<Vec<PortUsage>, String> {
    let output = Command::new("netstat")
        .arg("-ano")
        .output()
        .map_err(|error| format!("Nao foi possivel verificar portas em uso: {error}"))?;

    if !output.status.success() {
        return Err("Nao foi possivel verificar portas em uso com netstat.".to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let wanted_ports = ports.iter().copied().collect::<HashSet<_>>();
    let mut usages = Vec::new();
    let mut seen = HashSet::new();

    for line in stdout.lines() {
        let columns = line.split_whitespace().collect::<Vec<_>>();

        if columns.len() < 4 {
            continue;
        }

        let protocol = columns[0].to_uppercase();

        if protocol != "TCP" && protocol != "UDP" {
            continue;
        }

        let local_address = columns[1];
        let pid_column = if protocol == "TCP" {
            columns.get(4).copied()
        } else {
            columns.get(3).copied()
        };
        let Some(port) = parse_netstat_port(local_address) else {
            continue;
        };
        let Some(pid) = pid_column.and_then(|value| value.parse::<u32>().ok()) else {
            continue;
        };

        if !wanted_ports.contains(&port) {
            continue;
        }

        let key = format!("{protocol}:{port}:{pid}");

        if !seen.insert(key) {
            continue;
        }

        usages.push(PortUsage {
            port,
            protocol,
            pid,
            process_name: process_name_for_pid(pid),
        });
    }

    Ok(usages)
}

fn parse_netstat_port(local_address: &str) -> Option<u16> {
    let port = local_address.rsplit_once(':')?.1;

    port.parse::<u16>().ok()
}

fn process_name_for_pid(pid: u32) -> String {
    let output = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .output();
    let Ok(output) = output else {
        return format!("PID {pid}");
    };

    if !output.status.success() {
        return format!("PID {pid}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().next().unwrap_or_default().trim();

    if line.is_empty() || line.eq_ignore_ascii_case("INFO: No tasks are running which match the specified criteria.") {
        return format!("PID {pid}");
    }

    line.split(',')
        .next()
        .map(|value| value.trim_matches('"').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("PID {pid}"))
}

fn server_test_setup_error(
    summary: &str,
    bat_path: &Path,
    command: &str,
    duration_seconds: u64,
) -> ServerTestResult {
    ServerTestResult {
        status: "setup_error".to_string(),
        summary: summary.to_string(),
        duration_seconds,
        bat_path: bat_path.display().to_string(),
        command: command.to_string(),
        warning_count: 0,
        critical_count: 0,
        log_lines: Vec::new(),
    }
}

fn create_server_test_batch(
    game_dir: &Path,
    bat_path: &Path,
    server_id: &str,
) -> Result<PathBuf, String> {
    if !server_id
        .chars()
        .all(|char| char.is_ascii_alphanumeric() || char == '_' || char == '-')
    {
        return Err("O identificador do servidor contem caracteres invalidos para teste.".to_string());
    }

    let content = read_text_lossy(bat_path)?;
    let game_dir_text = game_dir.display().to_string();
    let mut injected_server_name = false;
    let updated_content = content
        .lines()
        .map(|line| {
            if line.trim().eq_ignore_ascii_case("PAUSE") {
                return "REM PAUSE disabled by PZMM server test".to_string();
            }

            let mut line = line.replace("%~dp0", &game_dir_text);

            if line.contains("zombie.network.GameServer") && !line.contains("-servername") {
                line.push_str(&format!(" -servername {server_id}"));
                injected_server_name = true;
            }

            if line.contains("zombie.network.GameServer") && !line.to_lowercase().contains("-adminpassword") {
                line.push_str(" -adminpassword PzmmTestAdmin123!");
            }

            line
        })
        .collect::<Vec<_>>()
        .join("\r\n");

    if !injected_server_name && !updated_content.contains("-servername") {
        return Err("Nao foi possivel preparar o teste: linha GameServer nao encontrada no .bat.".to_string());
    }

    let test_bat_path = env::temp_dir().join(format!("pzmm-test-{server_id}.bat"));

    fs::write(&test_bat_path, updated_content)
        .map_err(|error| format!("Nao foi possivel criar .bat temporario de teste: {error}"))?;

    Ok(test_bat_path)
}

fn spawn_output_reader<R>(stream: Option<R>, label: &'static str, sender: mpsc::Sender<String>)
where
    R: std::io::Read + Send + 'static,
{
    let Some(stream) = stream else {
        return;
    };

    thread::spawn(move || {
        let reader = BufReader::new(stream);

        for line in reader.lines().map_while(Result::ok) {
            let _ = sender.send(format!("[{label}] {line}"));
        }
    });
}

fn kill_process_tree(pid: u32) -> Result<(), String> {
    Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("Nao foi possivel encerrar o processo do teste: {error}"))?;

    Ok(())
}

fn find_critical_server_lines(log_lines: &[String]) -> Vec<String> {
    let patterns = [
        "exception",
        "java.lang",
        "error",
        "failed",
        "required mod",
        "workshop item",
        "nullpointerexception",
    ];

    log_lines
        .iter()
        .filter(|line| {
            let normalized = line.to_lowercase();
            if is_warning_log_line(&normalized) {
                return false;
            }

            patterns.iter().any(|pattern| normalized.contains(pattern))
                || normalized.contains("missing mod")
                || normalized.contains("missing required")
        })
        .cloned()
        .collect()
}

fn summarize_known_server_error(log_lines: &[String]) -> Option<String> {
    let combined_log = log_lines.join("\n").to_lowercase();

    if combined_log.contains("raknet.startup() return code: 5")
        || combined_log.contains("connection startup failed. code: 5")
    {
        return Some(
            "Falha ao iniciar a rede do servidor: a porta configurada parece estar em uso ou bloqueada. Verifique se outro servidor Project Zomboid ja esta rodando ou altere as portas do perfil."
                .to_string(),
        );
    }

    None
}

fn is_server_started_line(normalized_line: &str) -> bool {
    normalized_line.contains("*** server started")
        || normalized_line.contains("server is listening on port")
        || normalized_line.contains("raknet.startup() return code: 0")
}

fn count_warning_server_lines(log_lines: &[String]) -> usize {
    log_lines
        .iter()
        .filter(|line| is_warning_log_line(&line.to_lowercase()))
        .count()
}

fn is_warning_log_line(normalized_line: &str) -> bool {
    normalized_line.contains("warn")
}

fn tail_log_lines(log_lines: Vec<String>, max_lines: usize) -> Vec<String> {
    let start = log_lines.len().saturating_sub(max_lines);

    log_lines.into_iter().skip(start).collect()
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
        (true, false) => updated.replace(&format!("-Xms{ram_mb}m"), &format!("-Xms{ram_mb}m -Xmx{ram_mb}m")),
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

    Ok(PathBuf::from(config_root)
        .join("ZomboidServerModManager"))
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
        candidates.push(current_dir.join("resources").join("steacmd").join("steamcmd.zip"));
        candidates.push(current_dir.join("resources").join("steamcmd").join("steamcmd.zip"));
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

fn read_text_lossy(path: &Path) -> Result<String, String> {
    let content_bytes = fs::read(path)
        .map_err(|error| format!("Nao foi possivel ler {}: {error}", path.display()))?;

    Ok(String::from_utf8_lossy(&content_bytes).to_string())
}

fn read_ini_value(content: &str, key: &str) -> Option<String> {
    content.lines().find_map(|line| {
        let line = line.trim();

        if line.is_empty() || line.starts_with('#') {
            return None;
        }

        let (current_key, value) = line.split_once('=')?;

        if current_key.trim().eq_ignore_ascii_case(key) {
            Some(clean_ini_value(value))
        } else {
            None
        }
    })
}

fn replace_or_append_ini_value(content: &str, key: &str, value: &str) -> String {
    let mut replaced = false;
    let mut lines = content
        .lines()
        .map(|line| {
            let trimmed = line.trim();

            if trimmed.starts_with('#') {
                return line.to_string();
            }

            let current_key = trimmed
                .split_once('=')
                .map(|(current_key, _)| current_key)
                .unwrap_or(trimmed);

            if current_key.trim().eq_ignore_ascii_case(key) {
                replaced = true;
                return format!("{key}={value}");
            }

            line.to_string()
        })
        .collect::<Vec<_>>();

    if !replaced {
        lines.push(format!("{key}={value}"));
    }

    lines.join("\n")
}

fn read_ini_values(content: &str, key: &str) -> Vec<String> {
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();

            if line.is_empty() || line.starts_with('#') {
                return None;
            }

            let (current_key, value) = line.split_once('=')?;

            if current_key.trim().eq_ignore_ascii_case(key) {
                Some(clean_ini_value(value))
            } else {
                None
            }
        })
        .collect()
}

fn split_mod_ids(value: &str) -> Vec<String> {
    value
        .split([';', ','])
        .map(str::trim)
        .filter(|mod_id| !mod_id.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn clean_ini_value(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
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

fn clean_mod_description(value: &str) -> String {
    value
        .replace("<LINE>", " ")
        .replace("<LINE><LINE>", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn directory_size(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };

    entries
        .filter_map(Result::ok)
        .map(|entry| {
            let path = entry.path();

            if path.is_dir() {
                directory_size(&path)
            } else {
                entry.metadata().map(|metadata| metadata.len()).unwrap_or(0)
            }
        })
        .sum()
}

fn format_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let bytes = bytes as f64;

    if bytes >= GB {
        format!("{:.1} GB", bytes / GB)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes / MB)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes / KB)
    } else {
        format!("{bytes:.0} B")
    }
}

fn capitalize_first_letter(value: &str) -> String {
    let mut chars = value.chars();

    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_fast_steamcmd_script_without_validate_by_default() {
        let script_path =
            create_steamcmd_workshop_script(&["123".to_string(), "456".to_string()], false)
                .expect("script should be created");
        let script = fs::read_to_string(&script_path).expect("script should be readable");
        let _ = fs::remove_file(script_path);

        assert!(script.contains("login anonymous"));
        assert!(script.contains("workshop_download_item 108600 123\r\n"));
        assert!(!script.contains("123 validate"));
    }

    #[test]
    fn adds_validate_when_full_validation_is_requested() {
        let script_path = create_steamcmd_workshop_script(&["123".to_string()], true)
            .expect("script should be created");
        let script = fs::read_to_string(&script_path).expect("script should be readable");
        let _ = fs::remove_file(script_path);

        assert!(script.contains("workshop_download_item 108600 123 validate"));
    }

    #[test]
    fn identifies_workshop_ids_and_output_states() {
        let wanted_ids = ["123".to_string()].into_iter().collect::<HashSet<_>>();

        assert_eq!(
            find_workshop_id_in_line("Success. Downloaded item 123", &wanted_ids),
            Some("123".to_string())
        );
        assert_eq!(
            steamcmd_workshop_line_status("success. downloaded item 123"),
            Some("completed")
        );
        assert_eq!(
            steamcmd_workshop_line_status("error! download item 123 failed"),
            Some("failed")
        );
    }
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
