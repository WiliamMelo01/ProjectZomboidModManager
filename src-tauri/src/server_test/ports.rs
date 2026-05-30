use crate::models::{PortUsage, ServerPortCheck};
use crate::util::{read_ini_value, read_text_lossy};
use crate::zomboid_server_dir;
use std::{collections::HashSet, process::Command};

pub(super) fn check_zomboid_server_ports_impl(server_id: &str) -> Result<ServerPortCheck, String> {
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
