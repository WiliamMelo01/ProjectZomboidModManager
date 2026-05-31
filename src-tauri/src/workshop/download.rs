use super::api::{fetch_steam_workshop_item_names, validate_workshop_id};
use crate::i18n::text;
use crate::models::{WorkshopDownloadEvent, WorkshopDownloadFailedItem, WorkshopDownloadResult};
use crate::mods::normalize_server_values;
use crate::server_test::{kill_process_tree, spawn_output_reader};
use crate::{ensure_managed_steamcmd, find_steamcmd_path};
use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{mpsc, Mutex, OnceLock},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::Emitter;

struct ActiveWorkshopDownload {
    pid: Option<u32>,
    cancel_requested: bool,
}

static ACTIVE_WORKSHOP_DOWNLOAD: OnceLock<Mutex<Option<ActiveWorkshopDownload>>> = OnceLock::new();

pub(super) fn download_steam_workshop_items_impl(
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
        return Err(text(
            "Enter at least one Steam Workshop item to download.",
            "Informe ao menos um item da Steam Workshop para baixar.",
        )
        .to_string());
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
        pid: None,
        cancel_requested: false,
    });

    Ok(WorkshopDownloadGuard)
}

pub(super) fn cancel_steam_workshop_download_impl() -> Result<(), String> {
    let pid = {
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
                "{} {}: {error}",
                text("Could not run", "Nao foi possivel executar"),
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
