use super::api::{fetch_steam_workshop_item_names, validate_workshop_id};
use crate::i18n::text;
use crate::models::{
    WorkshopDownloadEvent, WorkshopDownloadFailedItem, WorkshopDownloadLogEvent,
    WorkshopDownloadResult,
};
use crate::mods::{normalize_server_values, steam_workshop_dirs};
use crate::server_test::{kill_process_tree, spawn_output_reader};
use crate::settings::read_max_concurrent_downloads;
use crate::util::hide_command_window;
use crate::{ensure_managed_steamcmd_pool, zomboid_mods_dir};
use std::{
    collections::{HashMap, HashSet},
    env, fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{mpsc, Mutex, OnceLock},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::Emitter;

struct ActiveWorkshopDownload {
    pids: HashSet<u32>,
    cancel_requested: bool,
}

static ACTIVE_WORKSHOP_DOWNLOAD: OnceLock<Mutex<Option<ActiveWorkshopDownload>>> = OnceLock::new();
const WORKSHOP_SCRIPT_CHUNK_SIZE: usize = 8;

pub(super) fn download_steam_workshop_items_impl(
    app: &tauri::AppHandle,
    workshop_ids: Vec<String>,
    force_validate: bool,
) -> Result<WorkshopDownloadResult, String> {
    let _guard = begin_workshop_download()?;
    let workshop_ids = normalize_server_values(&workshop_ids);
    let total_items = workshop_ids.len();

    if workshop_ids.is_empty() {
        return Err(text(
            "Enter at least one Steam Workshop item to download.",
            "Informe ao menos um item da Steam Workshop para baixar.",
        )
        .to_string());
    }

    let (skipped_ids, pending_ids) = split_already_downloaded_items(&workshop_ids, force_validate);
    let skipped_items = skipped_ids.len();

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
            skipped_items,
            failed_items: Vec::new(),
            cancelled_items: 0,
            was_cancelled: false,
        });
    }

    let max_concurrent_downloads = read_max_concurrent_downloads()? as usize;
    let steamcmd_paths = ensure_managed_steamcmd_pool(app, max_concurrent_downloads)?;

    let first_pass =
        run_steamcmd_workshop_passes(app, &steamcmd_paths, &pending_ids, force_validate)?;
    let was_cancelled = workshop_download_was_cancelled();
    let mut failed_items = first_pass.failed_items;

    if !was_cancelled && !failed_items.is_empty() {
        let retry_ids = failed_items
            .iter()
            .filter(|(_, error)| is_transient_steamcmd_download_error(error))
            .map(|(workshop_id, _)| workshop_id.clone())
            .collect::<Vec<_>>();

        for workshop_id in &retry_ids {
            emit_workshop_download_event(app, workshop_id, "retrying", None);
        }

        if !retry_ids.is_empty() {
            thread::sleep(Duration::from_secs(1));
            let retry_failed_items =
                run_steamcmd_workshop_passes(app, &steamcmd_paths, &retry_ids, force_validate)?
                    .failed_items;

            for retry_id in &retry_ids {
                failed_items.remove(retry_id);
            }
            failed_items.extend(retry_failed_items);
        }
    }

    let was_cancelled = workshop_download_was_cancelled();
    let cancelled_items = cancelled_item_count(
        was_cancelled,
        pending_ids.len(),
        &first_pass.completed_items,
        &failed_items,
    );
    let failed_items = enrich_workshop_download_failures(failed_items);

    if was_cancelled {
        let failed_ids = failed_items
            .iter()
            .map(|item| item.workshop_id.as_str())
            .collect::<HashSet<_>>();
        for workshop_id in &pending_ids {
            if !first_pass.completed_items.contains(workshop_id)
                && !failed_ids.contains(workshop_id.as_str())
            {
                emit_workshop_download_event(app, workshop_id, "cancelled", None);
            }
        }
    }

    Ok(WorkshopDownloadResult {
        total_items,
        downloaded_items: if was_cancelled {
            first_pass.completed_items.len()
        } else {
            pending_ids.len().saturating_sub(failed_items.len())
        },
        skipped_items,
        failed_items,
        cancelled_items,
        was_cancelled,
    })
}

fn cancelled_item_count(
    was_cancelled: bool,
    pending_item_count: usize,
    completed_items: &HashSet<String>,
    failed_items: &HashMap<String, String>,
) -> usize {
    if !was_cancelled {
        return 0;
    }

    pending_item_count
        .saturating_sub(completed_items.len())
        .saturating_sub(failed_items.len())
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
    let mut active_download = workshop_download_state().lock().map_err(|_| {
        text(
            "Could not access the download state.",
            "Nao foi possivel acessar o estado dos downloads.",
        )
        .to_string()
    })?;

    if active_download.is_some() {
        return Err(text(
            "A Steam Workshop download is already in progress.",
            "Ja existe um download da Steam Workshop em andamento.",
        )
        .to_string());
    }

    *active_download = Some(ActiveWorkshopDownload {
        pids: HashSet::new(),
        cancel_requested: false,
    });

    Ok(WorkshopDownloadGuard)
}

pub(super) fn cancel_steam_workshop_download_impl() -> Result<(), String> {
    let pids = {
        let mut active_download = workshop_download_state().lock().map_err(|_| {
            text(
                "Could not access the download state.",
                "Nao foi possivel acessar o estado dos downloads.",
            )
            .to_string()
        })?;
        let Some(active_download) = active_download.as_mut() else {
            return Ok(());
        };

        active_download.cancel_requested = true;
        active_download.pids.iter().copied().collect::<Vec<_>>()
    };

    for pid in pids {
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

fn split_already_downloaded_items(
    workshop_ids: &[String],
    force_validate: bool,
) -> (Vec<String>, Vec<String>) {
    if force_validate {
        return (Vec::new(), workshop_ids.to_vec());
    }

    split_already_downloaded_items_with_roots(workshop_ids, false, &steam_workshop_dirs())
}

fn split_already_downloaded_items_with_roots(
    workshop_ids: &[String],
    force_validate: bool,
    workshop_roots: &[PathBuf],
) -> (Vec<String>, Vec<String>) {
    if force_validate {
        return (Vec::new(), workshop_ids.to_vec());
    }

    let mut skipped_ids = Vec::new();
    let mut pending_ids = Vec::new();

    for workshop_id in workshop_ids {
        if is_workshop_item_downloaded(workshop_id, workshop_roots) {
            skipped_ids.push(workshop_id.clone());
        } else {
            pending_ids.push(workshop_id.clone());
        }
    }

    (skipped_ids, pending_ids)
}

fn is_workshop_item_downloaded(workshop_id: &str, workshop_roots: &[PathBuf]) -> bool {
    workshop_roots.iter().any(|root| {
        let item_dir = root.join(workshop_id);
        item_dir.is_dir() && item_has_mod_info(&item_dir)
    }) || is_local_workshop_item_installed(workshop_id)
}

fn item_has_mod_info(item_dir: &Path) -> bool {
    let mods_dir = item_dir.join("mods");
    let Ok(entries) = fs::read_dir(&mods_dir) else {
        return false;
    };

    entries.filter_map(Result::ok).any(|entry| {
        let package = entry.path();
        package.is_dir() && package.join("mod.info").is_file()
    })
}

fn is_local_workshop_item_installed(workshop_id: &str) -> bool {
    let Ok(local_dir) = zomboid_mods_dir() else {
        return false;
    };
    let Ok(entries) = fs::read_dir(local_dir) else {
        return false;
    };

    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path().join(".pzmm-workshop-id"))
        .filter_map(|marker_path| fs::read_to_string(marker_path).ok())
        .any(|value| value.trim() == workshop_id)
}

fn add_active_workshop_download_pid(pid: u32) {
    if let Ok(mut active_download) = workshop_download_state().lock() {
        if let Some(active_download) = active_download.as_mut() {
            active_download.pids.insert(pid);
        }
    }
}

fn remove_active_workshop_download_pid(pid: u32) {
    if let Ok(mut active_download) = workshop_download_state().lock() {
        if let Some(active_download) = active_download.as_mut() {
            active_download.pids.remove(&pid);
        }
    }
}

fn run_steamcmd_workshop_passes(
    app: &tauri::AppHandle,
    steamcmd_paths: &[PathBuf],
    workshop_ids: &[String],
    force_validate: bool,
) -> Result<WorkshopDownloadPassResult, String> {
    if steamcmd_paths.is_empty() {
        return Err(text(
            "No SteamCMD instance is available.",
            "Nenhuma instancia SteamCMD esta disponivel.",
        )
        .to_string());
    }

    let batches = build_workshop_download_batches(workshop_ids, steamcmd_paths.len());

    if batches.len() <= 1 {
        return run_steamcmd_workshop_queue(
            app,
            &steamcmd_paths[0],
            workshop_ids,
            force_validate,
            1,
        );
    }

    let mut handles = Vec::new();

    for (batch_index, batch) in batches.into_iter().enumerate() {
        let app = app.clone();
        let steamcmd_path = steamcmd_paths
            .get(batch_index)
            .unwrap_or(&steamcmd_paths[0])
            .to_path_buf();
        let instance_id = batch_index + 1;

        handles.push(thread::spawn(move || {
            run_steamcmd_workshop_queue(&app, &steamcmd_path, &batch, force_validate, instance_id)
        }));
    }

    let mut completed_items = HashSet::new();
    let mut failed_items = HashMap::new();
    let mut first_error = None;

    for handle in handles {
        match handle
            .join()
            .map_err(|_| text("SteamCMD worker failed.", "Tarefa do SteamCMD falhou.").to_string())
            .and_then(|result| result)
        {
            Ok(pass) => {
                completed_items.extend(pass.completed_items);
                failed_items.extend(pass.failed_items);
            }
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }

    if let Some(error) = first_error {
        return Err(error);
    }

    Ok(WorkshopDownloadPassResult {
        completed_items,
        failed_items,
    })
}

fn build_workshop_download_batches(
    workshop_ids: &[String],
    max_concurrent_downloads: usize,
) -> Vec<Vec<String>> {
    let batch_count = max_concurrent_downloads.clamp(1, workshop_ids.len().max(1));
    let mut batches = vec![Vec::new(); batch_count];

    for (index, workshop_id) in workshop_ids.iter().enumerate() {
        batches[index % batch_count].push(workshop_id.clone());
    }

    batches
        .into_iter()
        .filter(|batch| !batch.is_empty())
        .collect()
}

fn run_steamcmd_workshop_queue(
    app: &tauri::AppHandle,
    steamcmd_path: &Path,
    workshop_ids: &[String],
    force_validate: bool,
    instance_id: usize,
) -> Result<WorkshopDownloadPassResult, String> {
    emit_steamcmd_log_line(
        app,
        instance_id,
        &format!(
            "[PZMM] {}: {}",
            text(
                "Pending items assigned to this SteamCMD instance",
                "Itens pendentes atribuidos a esta instancia do SteamCMD"
            ),
            workshop_ids.len()
        ),
    );

    let mut completed_items = HashSet::new();
    let mut failed_items = HashMap::new();

    for (chunk_index, chunk) in workshop_ids.chunks(WORKSHOP_SCRIPT_CHUNK_SIZE).enumerate() {
        if workshop_download_was_cancelled() {
            break;
        }

        let first_item_number = chunk_index * WORKSHOP_SCRIPT_CHUNK_SIZE + 1;
        let last_item_number = (first_item_number + chunk.len()).saturating_sub(1);
        emit_steamcmd_log_line(
            app,
            instance_id,
            &format!(
                "[PZMM] {} {}-{} / {}",
                text("Downloading queued batch", "Baixando lote da fila"),
                first_item_number,
                last_item_number,
                workshop_ids.len(),
            ),
        );

        let pass =
            run_steamcmd_workshop_pass(app, steamcmd_path, chunk, force_validate, instance_id)?;

        completed_items.extend(pass.completed_items);
        failed_items.extend(pass.failed_items);
    }

    Ok(WorkshopDownloadPassResult {
        completed_items,
        failed_items,
    })
}

fn run_steamcmd_workshop_pass(
    app: &tauri::AppHandle,
    steamcmd_path: &Path,
    workshop_ids: &[String],
    force_validate: bool,
    instance_id: usize,
) -> Result<WorkshopDownloadPassResult, String> {
    let script_path = create_steamcmd_workshop_script(workshop_ids, force_validate)?;
    let mut command = Command::new(steamcmd_path);

    if let Some(steamcmd_dir) = steamcmd_path.parent() {
        command.current_dir(steamcmd_dir);
    }
    let mut log_tails = steamcmd_log_tails(steamcmd_path);

    emit_steamcmd_log_line(
        app,
        instance_id,
        &format!(
            "[PZMM] {}",
            text("Starting SteamCMD...", "Iniciando o SteamCMD...")
        ),
    );
    let child_result = hide_command_window(&mut command)
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
                "{} {}: {error}",
                text("Could not run", "Nao foi possivel executar"),
                steamcmd_path.display()
            ));
        }
    };
    let child_pid = child.id();
    add_active_workshop_download_pid(child_pid);

    let (sender, receiver) = mpsc::channel::<String>();
    spawn_output_reader(child.stdout.take(), "OUT", sender.clone());
    spawn_output_reader(child.stderr.take(), "ERR", sender);

    let wanted_ids = workshop_ids.iter().cloned().collect::<HashSet<_>>();
    let mut completed_items = HashSet::new();
    let mut failed_items = HashMap::new();
    let mut log_lines = Vec::new();
    let mut active_workshop_id =
        emit_next_pending_workshop_item(app, workshop_ids, &completed_items, &failed_items, None);

    loop {
        while let Ok(line) = receiver.try_recv() {
            emit_steamcmd_log_line(app, instance_id, &line);
            if let Some((workshop_id, status)) = process_steamcmd_workshop_line(
                app,
                &line,
                &wanted_ids,
                &mut completed_items,
                &mut failed_items,
            ) {
                if matches!(status, "completed" | "failed")
                    && active_workshop_id.as_deref() == Some(workshop_id.as_str())
                {
                    active_workshop_id = emit_next_pending_workshop_item(
                        app,
                        workshop_ids,
                        &completed_items,
                        &failed_items,
                        active_workshop_id.as_deref(),
                    );
                }
            }
            log_lines.push(line);
        }
        drain_steamcmd_log_tails(
            app,
            instance_id,
            &mut log_tails,
            &wanted_ids,
            &mut completed_items,
            &mut failed_items,
            &mut active_workshop_id,
            workshop_ids,
            &mut log_lines,
        );

        if child
            .try_wait()
            .map_err(|error| {
                format!(
                    "{}: {error}",
                    text(
                        "Could not inspect SteamCMD",
                        "Nao foi possivel consultar o SteamCMD"
                    )
                )
            })?
            .is_some()
        {
            break;
        }

        thread::sleep(Duration::from_millis(50));
    }

    remove_active_workshop_download_pid(child_pid);
    let _ = fs::remove_file(&script_path);

    while let Ok(line) = receiver.try_recv() {
        emit_steamcmd_log_line(app, instance_id, &line);
        if let Some((workshop_id, status)) = process_steamcmd_workshop_line(
            app,
            &line,
            &wanted_ids,
            &mut completed_items,
            &mut failed_items,
        ) {
            if matches!(status, "completed" | "failed")
                && active_workshop_id.as_deref() == Some(workshop_id.as_str())
            {
                active_workshop_id = emit_next_pending_workshop_item(
                    app,
                    workshop_ids,
                    &completed_items,
                    &failed_items,
                    active_workshop_id.as_deref(),
                );
            }
        }
        log_lines.push(line);
    }
    drain_steamcmd_log_tails(
        app,
        instance_id,
        &mut log_tails,
        &wanted_ids,
        &mut completed_items,
        &mut failed_items,
        &mut active_workshop_id,
        workshop_ids,
        &mut log_lines,
    );

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

fn emit_steamcmd_log_line(app: &tauri::AppHandle, instance_id: usize, line: &str) {
    let _ = app.emit(
        "workshop-download-log",
        build_steamcmd_log_event(instance_id, line),
    );
}

struct SteamCmdLogTail {
    label: &'static str,
    path: PathBuf,
    position: u64,
    pending_fragment: String,
}

fn steamcmd_log_tails(steamcmd_path: &Path) -> Vec<SteamCmdLogTail> {
    let Some(steamcmd_dir) = steamcmd_path.parent() else {
        return Vec::new();
    };
    let logs_dir = steamcmd_dir.join("logs");

    ["console_log.txt", "content_log.txt"]
        .into_iter()
        .map(|file_name| {
            let path = logs_dir.join(file_name);
            let position = fs::metadata(&path)
                .map(|metadata| metadata.len())
                .unwrap_or(0);

            SteamCmdLogTail {
                label: if file_name == "console_log.txt" {
                    "console"
                } else {
                    "content"
                },
                path,
                position,
                pending_fragment: String::new(),
            }
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn drain_steamcmd_log_tails(
    app: &tauri::AppHandle,
    instance_id: usize,
    log_tails: &mut [SteamCmdLogTail],
    wanted_ids: &HashSet<String>,
    completed_items: &mut HashSet<String>,
    failed_items: &mut HashMap<String, String>,
    active_workshop_id: &mut Option<String>,
    workshop_ids: &[String],
    log_lines: &mut Vec<String>,
) {
    for log_tail in log_tails {
        let Ok(mut file) = fs::File::open(&log_tail.path) else {
            continue;
        };
        let Ok(metadata) = file.metadata() else {
            continue;
        };

        if metadata.len() < log_tail.position {
            log_tail.position = 0;
            log_tail.pending_fragment.clear();
        }

        if metadata.len() == log_tail.position {
            continue;
        }

        if file.seek(SeekFrom::Start(log_tail.position)).is_err() {
            continue;
        }

        let mut content = String::new();
        if file.read_to_string(&mut content).is_err() {
            continue;
        }
        log_tail.position = metadata.len();

        if content.is_empty() {
            continue;
        }

        let combined = format!("{}{}", log_tail.pending_fragment, content);
        let ends_with_newline = combined.ends_with('\n');
        let mut lines = combined
            .split('\n')
            .map(|line| line.trim_end_matches('\r').to_string())
            .collect::<Vec<_>>();

        log_tail.pending_fragment = if ends_with_newline {
            String::new()
        } else {
            lines.pop().unwrap_or_default()
        };

        for line in lines {
            if line.trim().is_empty() || !should_emit_steamcmd_file_log_line(log_tail.label, &line)
            {
                continue;
            }

            let emitted_line = format!("[LOG:{}] {}", log_tail.label, line);
            emit_steamcmd_log_line(app, instance_id, &emitted_line);

            if let Some((workshop_id, status)) = process_steamcmd_workshop_line(
                app,
                &line,
                wanted_ids,
                completed_items,
                failed_items,
            ) {
                if matches!(status, "completed" | "failed")
                    && active_workshop_id.as_deref() == Some(workshop_id.as_str())
                {
                    *active_workshop_id = emit_next_pending_workshop_item(
                        app,
                        workshop_ids,
                        completed_items,
                        failed_items,
                        active_workshop_id.as_deref(),
                    );
                }
            }

            log_lines.push(emitted_line);
        }
    }
}

fn should_emit_steamcmd_file_log_line(label: &str, line: &str) -> bool {
    if label == "console" {
        return true;
    }

    let normalized_line = line.to_lowercase();
    normalized_line.contains("current download rate")
        || normalized_line.contains("update started")
        || normalized_line.contains("preallocated")
        || normalized_line.contains("scheduler finished")
        || normalized_line.contains("missing game files")
        || normalized_line.contains("download complete")
        || normalized_line.contains("downloaded")
}

fn build_steamcmd_log_event(instance_id: usize, line: &str) -> WorkshopDownloadLogEvent {
    WorkshopDownloadLogEvent {
        instance_id,
        label: format!("Instance {instance_id}"),
        color_key: steamcmd_log_color_key(instance_id).to_string(),
        line: line.to_string(),
    }
}

fn steamcmd_log_color_key(instance_id: usize) -> &'static str {
    match (instance_id.saturating_sub(1)) % 3 {
        0 => "orange",
        1 => "blue",
        _ => "green",
    }
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
        format!(
            "{}: {error}",
            text(
                "Could not create the temporary SteamCMD script",
                "Nao foi possivel criar o script temporario do SteamCMD"
            )
        )
    })?;

    Ok(script_path)
}

fn process_steamcmd_workshop_line(
    app: &tauri::AppHandle,
    line: &str,
    wanted_ids: &HashSet<String>,
    completed_items: &mut HashSet<String>,
    failed_items: &mut HashMap<String, String>,
) -> Option<(String, &'static str)> {
    let normalized_line = line.to_lowercase();
    let workshop_id = find_workshop_id_in_line(line, wanted_ids)?;

    if steamcmd_workshop_line_status(&normalized_line) == Some("completed") {
        failed_items.remove(&workshop_id);
        if completed_items.insert(workshop_id.clone()) {
            emit_workshop_download_event(app, &workshop_id, "completed", None);
        }
        Some((workshop_id, "completed"))
    } else if steamcmd_workshop_line_status(&normalized_line) == Some("failed") {
        failed_items.insert(workshop_id.clone(), line.to_string());
        emit_workshop_download_event(app, &workshop_id, "failed", Some(line.to_string()));
        Some((workshop_id, "failed"))
    } else if steamcmd_workshop_line_status(&normalized_line) == Some("downloading") {
        emit_workshop_download_event(app, &workshop_id, "downloading", None);
        Some((workshop_id, "downloading"))
    } else {
        None
    }
}

fn emit_next_pending_workshop_item(
    app: &tauri::AppHandle,
    workshop_ids: &[String],
    completed_items: &HashSet<String>,
    failed_items: &HashMap<String, String>,
    previous_active_id: Option<&str>,
) -> Option<String> {
    let next_workshop_id = workshop_ids
        .iter()
        .find(|workshop_id| {
            previous_active_id != Some(workshop_id.as_str())
                && !completed_items.contains(*workshop_id)
                && !failed_items.contains_key(*workshop_id)
        })?
        .clone();

    emit_workshop_download_event(app, &next_workshop_id, "downloading", None);
    Some(next_workshop_id)
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

fn is_transient_steamcmd_download_error(error: &str) -> bool {
    let normalized_error = error.to_lowercase();

    normalized_error.contains("locking failed")
        || normalized_error.contains("file locked")
        || normalized_error.contains("timeout")
        || normalized_error.contains("no connection")
        || normalized_error.contains("not logged on")
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

    failures.sort_by_key(|failure| failure.name.to_lowercase());
    failures
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        env::temp_dir().join(format!("pzmm-workshop-download-{label}-{timestamp}"))
    }

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

    #[test]
    fn builds_balanced_parallel_download_batches() {
        let workshop_ids = ["1", "2", "3", "4", "5"]
            .into_iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();

        assert_eq!(
            build_workshop_download_batches(&workshop_ids, 2),
            vec![
                vec!["1".to_string(), "3".to_string(), "5".to_string()],
                vec!["2".to_string(), "4".to_string()],
            ]
        );
        assert_eq!(
            build_workshop_download_batches(&workshop_ids, 10).len(),
            workshop_ids.len()
        );
        assert_eq!(
            build_workshop_download_batches(&workshop_ids, 0),
            vec![workshop_ids]
        );
    }

    #[test]
    fn skips_existing_workshop_item_with_mod_info() {
        let root = temp_dir("existing");
        let package = root.join("123").join("mods").join("ExampleMod");
        fs::create_dir_all(&package).unwrap();
        fs::write(package.join("mod.info"), "name=Example\nid=Example").unwrap();

        let (skipped, pending) = split_already_downloaded_items_with_roots(
            &["123".to_string()],
            false,
            std::slice::from_ref(&root),
        );
        let _ = fs::remove_dir_all(root);

        assert_eq!(skipped, vec!["123".to_string()]);
        assert!(pending.is_empty());
    }

    #[test]
    fn does_not_skip_incomplete_workshop_item() {
        let root = temp_dir("incomplete");
        fs::create_dir_all(root.join("123").join("mods").join("ExampleMod")).unwrap();

        let (skipped, pending) = split_already_downloaded_items_with_roots(
            &["123".to_string()],
            false,
            std::slice::from_ref(&root),
        );
        let _ = fs::remove_dir_all(root);

        assert!(skipped.is_empty());
        assert_eq!(pending, vec!["123".to_string()]);
    }

    #[test]
    fn force_validate_keeps_existing_items_pending() {
        let root = temp_dir("validate");
        let package = root.join("123").join("mods").join("ExampleMod");
        fs::create_dir_all(&package).unwrap();
        fs::write(package.join("mod.info"), "name=Example\nid=Example").unwrap();

        let (skipped, pending) = split_already_downloaded_items_with_roots(
            &["123".to_string()],
            true,
            std::slice::from_ref(&root),
        );
        let _ = fs::remove_dir_all(root);

        assert!(skipped.is_empty());
        assert_eq!(pending, vec!["123".to_string()]);
    }

    #[test]
    fn splits_mixed_collection_items() {
        let root = temp_dir("mixed");
        let package = root.join("123").join("mods").join("ExampleMod");
        fs::create_dir_all(&package).unwrap();
        fs::write(package.join("mod.info"), "name=Example\nid=Example").unwrap();

        let (skipped, pending) = split_already_downloaded_items_with_roots(
            &["123".to_string(), "456".to_string()],
            false,
            std::slice::from_ref(&root),
        );
        let _ = fs::remove_dir_all(root);

        assert_eq!(skipped, vec!["123".to_string()]);
        assert_eq!(pending, vec!["456".to_string()]);
    }

    #[test]
    fn cancelled_count_excludes_completed_and_failed_items() {
        let completed_items = ["1".to_string(), "2".to_string()]
            .into_iter()
            .collect::<HashSet<_>>();
        let failed_items = [("3".to_string(), "ERROR! Download item 3 failed".to_string())]
            .into_iter()
            .collect::<HashMap<_, _>>();

        assert_eq!(
            cancelled_item_count(true, 5, &completed_items, &failed_items),
            2
        );
        assert_eq!(
            cancelled_item_count(false, 5, &completed_items, &failed_items),
            0
        );
    }

    #[test]
    fn builds_structured_steamcmd_log_event() {
        let event = build_steamcmd_log_event(2, "[OUT] hello");

        assert_eq!(event.instance_id, 2);
        assert_eq!(event.label, "Instance 2");
        assert_eq!(event.color_key, "blue");
        assert_eq!(event.line, "[OUT] hello");
    }

    #[test]
    fn cycles_steamcmd_log_colors_by_instance() {
        assert_eq!(steamcmd_log_color_key(1), "orange");
        assert_eq!(steamcmd_log_color_key(2), "blue");
        assert_eq!(steamcmd_log_color_key(3), "green");
        assert_eq!(steamcmd_log_color_key(4), "orange");
    }
}
