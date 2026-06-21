use crate::i18n::text;
use crate::models::{ServerPortCheck, ServerTestEvent, ServerTestResult, ServerTestStarted};
use crate::run_blocking;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
    thread,
};
use tauri::Emitter;

mod batch;
mod logs;
pub(crate) mod ports;
mod preflight;
mod process;
mod runner;

use ports::check_zomboid_server_ports_impl;
#[allow(unused_imports)]
pub(crate) use ports::server_ports_for_id;
use process::kill_processes_by_pid_impl;
pub(crate) use process::{kill_process_tree, spawn_output_reader};
pub(crate) use runner::{
    test_zomboid_server_impl, test_zomboid_server_impl_with_line_callback_and_cancel,
};

static ACTIVE_SERVER_TESTS: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> = OnceLock::new();

fn active_server_tests() -> &'static Mutex<HashMap<String, Arc<AtomicBool>>> {
    ACTIVE_SERVER_TESTS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[tauri::command]
pub(crate) async fn test_zomboid_server(server_id: String) -> Result<ServerTestResult, String> {
    run_blocking(move || test_zomboid_server_impl(&server_id)).await
}

#[tauri::command]
pub(crate) fn start_zomboid_server_test(
    app: tauri::AppHandle,
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

    let cancel_flag = Arc::new(AtomicBool::new(false));
    active_server_tests()
        .lock()
        .map_err(|_| "Could not access active server tests.".to_string())?
        .insert(server_id.clone(), cancel_flag.clone());

    let event_server_id = server_id.clone();

    thread::spawn(move || {
        let _ = app.emit(
            "server-test-event",
            ServerTestEvent {
                server_id: event_server_id.clone(),
                event: "started".to_string(),
                timeout_seconds: None,
                line: None,
                result: None,
                error: None,
            },
        );

        let app_for_lines = app.clone();
        let line_server_id = event_server_id.clone();
        let cancel_flag_for_runner = cancel_flag.clone();
        let result = test_zomboid_server_impl_with_line_callback_and_cancel(
            &event_server_id,
            |line| {
                let _ = app_for_lines.emit(
                    "server-test-event",
                    ServerTestEvent {
                        server_id: line_server_id.clone(),
                        event: "line".to_string(),
                        timeout_seconds: None,
                        line: Some(line.to_string()),
                        result: None,
                        error: None,
                    },
                );
            },
            || cancel_flag_for_runner.load(Ordering::SeqCst),
        );

        if let Ok(mut active_tests) = active_server_tests().lock() {
            active_tests.remove(&event_server_id);
        }

        match result {
            Ok(result) => {
                let _ = app.emit(
                    "server-test-event",
                    ServerTestEvent {
                        server_id: event_server_id,
                        event: "finished".to_string(),
                        timeout_seconds: None,
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
                        timeout_seconds: None,
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
pub(crate) async fn cancel_zomboid_server_test(server_id: String) -> Result<(), String> {
    run_blocking(move || {
        let server_id = server_id.trim().to_string();
        if server_id.is_empty() {
            return Ok(());
        }

        if let Some(cancel_flag) = active_server_tests()
            .lock()
            .map_err(|_| "Could not access active server tests.".to_string())?
            .get(&server_id)
            .cloned()
        {
            cancel_flag.store(true, Ordering::SeqCst);
        }

        Ok(())
    })
    .await
}

#[tauri::command]
pub(crate) async fn check_zomboid_server_ports(
    server_id: String,
) -> Result<ServerPortCheck, String> {
    run_blocking(move || check_zomboid_server_ports_impl(&server_id)).await
}

#[tauri::command]
pub(crate) async fn kill_processes_by_pid(pids: Vec<u32>) -> Result<(), String> {
    run_blocking(move || kill_processes_by_pid_impl(pids)).await
}
