use crate::models::{ServerPortCheck, ServerTestEvent, ServerTestResult, ServerTestStarted};
use crate::run_blocking;
use std::thread;
use tauri::Emitter;

mod batch;
mod logs;
mod ports;
mod preflight;
mod process;
mod runner;

use ports::check_zomboid_server_ports_impl;
use process::kill_processes_by_pid_impl;
pub(crate) use process::{kill_process_tree, spawn_output_reader};
use runner::{test_zomboid_server_impl, test_zomboid_server_impl_with_line_callback};

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
pub(crate) async fn check_zomboid_server_ports(
    server_id: String,
) -> Result<ServerPortCheck, String> {
    run_blocking(move || check_zomboid_server_ports_impl(&server_id)).await
}

#[tauri::command]
pub(crate) async fn kill_processes_by_pid(pids: Vec<u32>) -> Result<(), String> {
    run_blocking(move || kill_processes_by_pid_impl(pids)).await
}
