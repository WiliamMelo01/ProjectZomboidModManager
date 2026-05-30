use crate::models::{
    PortUsage, ServerPortCheck, ServerTestEvent, ServerTestResult, ServerTestStarted,
};
use crate::util::{read_ini_value, read_text_lossy, split_mod_ids};
use crate::{
    list_zomboid_mods_impl, read_config_value, run_blocking, steam_zomboid_game_dirs,
    zomboid_server_dir,
};
use std::{
    collections::{HashMap, HashSet},
    env, fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use tauri::Emitter;
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

    if line.is_empty()
        || line
            .eq_ignore_ascii_case("INFO: No tasks are running which match the specified criteria.")
    {
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
        return Err(
            "O identificador do servidor contem caracteres invalidos para teste.".to_string(),
        );
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

            if line.contains("zombie.network.GameServer")
                && !line.to_lowercase().contains("-adminpassword")
            {
                line.push_str(" -adminpassword PzmmTestAdmin123!");
            }

            line
        })
        .collect::<Vec<_>>()
        .join("\r\n");

    if !injected_server_name && !updated_content.contains("-servername") {
        return Err(
            "Nao foi possivel preparar o teste: linha GameServer nao encontrada no .bat."
                .to_string(),
        );
    }

    let test_bat_path = env::temp_dir().join(format!("pzmm-test-{server_id}.bat"));

    fs::write(&test_bat_path, updated_content)
        .map_err(|error| format!("Nao foi possivel criar .bat temporario de teste: {error}"))?;

    Ok(test_bat_path)
}

pub(crate) fn spawn_output_reader<R>(
    stream: Option<R>,
    label: &'static str,
    sender: mpsc::Sender<String>,
) where
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

pub(crate) fn kill_process_tree(pid: u32) -> Result<(), String> {
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
