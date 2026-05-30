use crate::models::{WorkshopDownloadEvent, WorkshopDownloadFailedItem, WorkshopDownloadResult};
use crate::server_test::{kill_process_tree, spawn_output_reader};
use crate::{ensure_managed_steamcmd, find_steamcmd_path, normalize_server_values, run_blocking};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{mpsc, Mutex, OnceLock},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

struct ActiveWorkshopDownload {
    pid: Option<u32>,
    cancel_requested: bool,
}

static ACTIVE_WORKSHOP_DOWNLOAD: OnceLock<Mutex<Option<ActiveWorkshopDownload>>> = OnceLock::new();
#[tauri::command]
pub(crate) async fn download_steam_workshop_item(
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
pub(crate) async fn download_steam_workshop_collection(
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
pub(crate) async fn download_steam_workshop_items(
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
pub(crate) async fn cancel_steam_workshop_download() -> Result<(), String> {
    run_blocking(cancel_steam_workshop_download_impl).await
}

#[tauri::command]
pub(crate) fn open_steam_workshop(
    app: tauri::AppHandle,
    item_id_or_search: String,
) -> Result<(), String> {
    open_steam_workshop_impl(&app, &item_id_or_search)
}

#[tauri::command]
pub(crate) fn open_steam_workshop_external(item_id_or_search: String) -> Result<(), String> {
    open_steam_workshop_external_impl(&item_id_or_search)
}

#[tauri::command]
pub(crate) fn open_steam_workshop_steam_client(item_id_or_search: String) -> Result<(), String> {
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

    let first_pass =
        run_steamcmd_workshop_pass(app, &steamcmd_path, &workshop_ids, force_validate)?;
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
        lines.push(format!(
            "workshop_download_item 108600 {workshop_id}{validate}"
        ));
    }

    lines.push("quit".to_string());
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let script_path = env::temp_dir().join(format!("pzmm-steamcmd-{timestamp}.txt"));

    fs::write(&script_path, lines.join("\r\n")).map_err(|error| {
        format!("Nao foi possivel criar o script temporario do SteamCMD: {error}")
    })?;

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

fn fetch_steam_workshop_item_names(
    workshop_ids: &[String],
) -> Result<HashMap<String, String>, String> {
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

    serde_json::from_str(&stdout).map_err(|error| {
        format!("A Steam retornou uma resposta invalida ao tentar {action}: {error}")
    })
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

pub(crate) fn open_path_external(path: &Path) -> Result<(), String> {
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
