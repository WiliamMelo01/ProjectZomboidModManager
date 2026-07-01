use super::batch::create_server_test_batch;
use super::batch::default_server_launcher_name;
use super::logs::{
    count_warning_server_lines, find_critical_server_lines, is_server_started_line,
    server_test_setup_error, summarize_known_server_error, tail_log_lines,
};
use super::ports::{find_port_usages, server_ports_for_id};
use super::preflight::{resolve_zomboid_game_dir, validate_server_mod_dependencies};
use super::process::{kill_process_tree, spawn_output_reader};
use crate::i18n::text;
use crate::models::ServerTestResult;
use crate::util::hide_command_window;
use crate::{read_config_value, zomboid_server_dir};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

pub(crate) fn test_zomboid_server_impl(server_id: &str) -> Result<ServerTestResult, String> {
    test_zomboid_server_impl_with_line_callback(server_id, |_| {})
}

pub(crate) fn test_zomboid_server_impl_with_line_callback<F>(
    server_id: &str,
    on_line: F,
) -> Result<ServerTestResult, String>
where
    F: FnMut(&str),
{
    test_zomboid_server_impl_with_line_callback_and_cancel(server_id, on_line, || false)
}

pub(crate) fn test_zomboid_server_impl_with_line_callback_and_cancel<F, C>(
    server_id: &str,
    mut on_line: F,
    should_cancel: C,
) -> Result<ServerTestResult, String>
where
    F: FnMut(&str),
    C: Fn() -> bool,
{
    let server_id = server_id.trim();

    if server_id.is_empty() {
        return Ok(server_test_setup_error(
            &text(
                "Invalid server for testing.",
                "Servidor invalido para teste.",
            ),
            Path::new(default_server_launcher_name()),
            "",
            0,
        ));
    }

    let server_path = zomboid_server_dir()?.join(format!("{server_id}.ini"));

    if !server_path.exists() {
        return Ok(server_test_setup_error(
            &format!(
                "{}: {}.",
                text(
                    "Server file not found",
                    "Arquivo do servidor nao encontrado"
                ),
                server_path.display()
            ),
            Path::new(default_server_launcher_name()),
            "",
            0,
        ));
    }

    if let Some(dependency_result) = validate_server_mod_dependencies(server_id, &server_path)? {
        return Ok(dependency_result);
    }

    let configured_launch_path = read_config_value("server_launch_path")?
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from);
    let (game_dir, launcher_path) = if let Some(launch_path) = configured_launch_path {
        if launch_path.is_dir() {
            let launcher_path = launch_path.join(default_server_launcher_name());
            let Some(parent) = launcher_path
                .parent()
                .filter(|path| path.exists() && path.is_dir())
            else {
                return Ok(server_test_setup_error(
                    &format!(
                        "{}: {}.",
                        text(
                            "Configured server launcher folder was not found",
                            "A pasta configurada do inicializador do servidor nao foi encontrada"
                        ),
                        launch_path.display()
                    ),
                    &launcher_path,
                    "",
                    0,
                ));
            };
            (parent.to_path_buf(), launcher_path)
        } else {
            let Some(parent) = launch_path
                .parent()
                .filter(|path| path.exists() && path.is_dir())
            else {
                return Ok(server_test_setup_error(
                    &format!(
                        "{}: {}.",
                        text(
                            "Configured server launcher folder was not found",
                            "A pasta configurada do inicializador do servidor nao foi encontrada"
                        ),
                        launch_path.display()
                    ),
                    &launch_path,
                    "",
                    0,
                ));
            };

            (parent.to_path_buf(), launch_path)
        }
    } else {
        let Some(game_dir) = resolve_zomboid_game_dir()? else {
            return Ok(server_test_setup_error(
                &text(
                    "Project Zomboid folder not found. Configure the game executable in settings.",
                    "Pasta do Project Zomboid nao encontrada. Configure o executavel do jogo nas configuracoes.",
                ),
                Path::new(default_server_launcher_name()),
                "",
                0,
            ));
        };

        let launcher_path = game_dir.join(default_server_launcher_name());
        (game_dir, launcher_path)
    };
    let mut command = if cfg!(windows) {
        format!(
            "cmd.exe /C call \"{}\" -servername {}",
            launcher_path.display(),
            server_id
        )
    } else {
        format!(
            "sh \"{}\" -servername {}",
            launcher_path.display(),
            server_id
        )
    };

    if !launcher_path.exists() || !launcher_path.is_file() {
        return Ok(server_test_setup_error(
            &format!(
                "{} {}.",
                text(
                    "Server launcher not found in",
                    "Inicializador do servidor nao encontrado em"
                ),
                game_dir.display()
            ),
            &launcher_path,
            &command,
            0,
        ));
    }

    let test_bat_path = create_server_test_batch(&game_dir, &launcher_path, server_id)?;
    command = if cfg!(windows) {
        format!("cmd.exe /C call \"{}\"", test_bat_path.display())
    } else {
        format!("sh \"{}\"", test_bat_path.display())
    };
    let started_at = Instant::now();
    let mut child = {
        #[cfg(windows)]
        {
            let mut command_process = Command::new("cmd.exe");
            hide_command_window(&mut command_process)
                .arg("/C")
                .arg("call")
                .arg(&test_bat_path)
                .current_dir(&game_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .stdin(Stdio::null())
                .spawn()
                .map_err(|error| {
                    format!(
                        "{}: {error}",
                        text(
                            "Could not start the server test",
                            "Nao foi possivel iniciar o teste do servidor"
                        )
                    )
                })?
        }

        #[cfg(not(windows))]
        {
            Command::new("sh")
                .arg(&test_bat_path)
                .current_dir(&game_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .stdin(Stdio::null())
                .spawn()
                .map_err(|error| {
                    format!(
                        "{}: {error}",
                        text(
                            "Could not start the server test",
                            "Nao foi possivel iniciar o teste do servidor"
                        )
                    )
                })?
        }
    };

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (sender, receiver) = mpsc::channel::<String>();

    spawn_output_reader(stdout, "OUT", sender.clone());
    spawn_output_reader(stderr, "ERR", sender);

    let server_ports = server_ports_for_id(server_id).unwrap_or_default();
    let mut log_lines = Vec::new();
    let mut process_exited = false;
    let mut server_started = false;
    let mut was_cancelled = false;
    let mut last_port_probe = Instant::now();
    let mut last_output_at = Instant::now();
    let mut last_wait_message_at = Instant::now();

    loop {
        while let Ok(line) = receiver.try_recv() {
            if is_server_started_line(&line.to_lowercase()) {
                server_started = true;
            }
            last_output_at = Instant::now();
            on_line(&line);
            log_lines.push(line);
        }

        if server_started {
            break;
        }

        if should_cancel() {
            was_cancelled = true;
            let line = "[INFO] Server test cancelled by user.";
            on_line(line);
            log_lines.push(line.to_string());
            break;
        }

        if last_port_probe.elapsed() >= Duration::from_secs(2) {
            last_port_probe = Instant::now();
            if let Ok(usages) = find_port_usages(&server_ports) {
                if !usages.is_empty() {
                    server_started = true;
                    let ports = usages
                        .iter()
                        .map(|usage| format!("{} {} PID {}", usage.protocol, usage.port, usage.pid))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let line =
                        format!("[INFO] Detected Project Zomboid server port activity: {ports}.");
                    on_line(&line);
                    log_lines.push(line);
                    break;
                }
            }
        }

        if last_output_at.elapsed() >= Duration::from_secs(60)
            && last_wait_message_at.elapsed() >= Duration::from_secs(60)
        {
            last_wait_message_at = Instant::now();
            let line = "[INFO] Still waiting for server startup confirmation. Use Cancel to stop this test.";
            on_line(line);
            log_lines.push(line.to_string());
        }

        if child
            .try_wait()
            .map_err(|error| {
                format!(
                    "{}: {error}",
                    text(
                        "Could not inspect the server process",
                        "Nao foi possivel consultar o processo do servidor"
                    )
                )
            })?
            .is_some()
        {
            process_exited = true;
            break;
        }

        thread::sleep(Duration::from_millis(50));
    }

    while let Ok(line) = receiver.try_recv() {
        if is_server_started_line(&line.to_lowercase()) {
            server_started = true;
        }
        on_line(&line);
        log_lines.push(line);
    }

    if !process_exited {
        let _ = kill_process_tree(child.id());
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
    let critical_lines = if server_started || was_cancelled {
        Vec::new()
    } else {
        find_critical_server_lines(&log_lines)
    };
    let warning_count = count_warning_server_lines(&log_lines);
    let status = if server_started { "passed" } else { "failed" };
    let summary = if server_started {
        text(
            "Server started successfully: network active and port listening. The test was stopped automatically.",
            "Servidor iniciado com sucesso: rede ativa e porta escutando. O teste foi encerrado automaticamente.",
        ).to_string()
    } else if was_cancelled {
        text(
            "Server test cancelled by user.",
            "Teste do servidor cancelado pelo usuario.",
        )
        .to_string()
    } else if let Some(network_error_summary) = summarize_known_server_error(&log_lines) {
        network_error_summary
    } else if process_exited {
        text(
            "The server process exited unexpectedly before starting.",
            "O processo do servidor terminou inesperadamente antes de iniciar.",
        )
        .to_string()
    } else if critical_lines.is_empty() {
        if warning_count == 0 {
            text(
                "Server test stopped without detecting startup and no critical failures were captured.",
                "O teste do servidor parou sem detectar inicializacao e nenhuma falha critica foi capturada.",
            )
            .to_string()
        } else {
            format!(
                "{} {warning_count} {}.",
                text(
                    "Server test stopped without detecting startup. Captured",
                    "O teste do servidor parou sem detectar inicializacao. Foram capturados"
                ),
                text("warning(s)", "aviso(s)")
            )
        }
    } else {
        format!(
            "{} {} {}.",
            text("Test found", "Teste encontrou"),
            critical_lines.len(),
            text(
                "line(s) with possible critical failures",
                "linha(s) com possiveis falhas criticas"
            )
        )
    };

    Ok(ServerTestResult {
        status: status.to_string(),
        summary,
        duration_seconds,
        bat_path: launcher_path.display().to_string(),
        command,
        warning_count,
        critical_count: critical_lines.len(),
        log_lines: tail_log_lines(log_lines, 240),
    })
}
