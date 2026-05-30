use super::batch::create_server_test_batch;
use super::logs::{
    count_warning_server_lines, find_critical_server_lines, is_server_started_line,
    server_test_setup_error, summarize_known_server_error, tail_log_lines,
};
use super::preflight::{resolve_zomboid_game_dir, validate_server_mod_dependencies};
use super::process::{kill_process_tree, spawn_output_reader};
use crate::models::ServerTestResult;
use crate::zomboid_server_dir;
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

pub(super) fn test_zomboid_server_impl(server_id: &str) -> Result<ServerTestResult, String> {
    test_zomboid_server_impl_with_line_callback(server_id, |_| {})
}

pub(super) fn test_zomboid_server_impl_with_line_callback<F>(
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
            &format!(
                "Arquivo do servidor nao encontrado: {}.",
                server_path.display()
            ),
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
            &format!(
                "ProjectZomboidServer.bat nao encontrado em {}.",
                game_dir.display()
            ),
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
            "Teste rapido concluido em 180s: nenhuma falha critica detectada nos logs capturados."
                .to_string()
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
