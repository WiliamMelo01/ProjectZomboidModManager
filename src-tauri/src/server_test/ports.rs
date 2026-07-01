use crate::i18n::text;
use crate::models::{PortUsage, ServerPortCheck};
use crate::util::hide_command_window;
use crate::util::{read_ini_value, read_text_lossy};
use crate::zomboid_server_dir;
use std::{collections::HashSet, process::Command};

pub(super) fn check_zomboid_server_ports_impl(server_id: &str) -> Result<ServerPortCheck, String> {
    let ports = server_ports_for_id(server_id)?;
    let usages = find_port_usages(&ports)?;

    Ok(ServerPortCheck { ports, usages })
}

pub(crate) fn server_ports_for_id(server_id: &str) -> Result<Vec<u16>, String> {
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

pub(crate) fn find_port_usages(ports: &[u16]) -> Result<Vec<PortUsage>, String> {
    let wanted_ports = ports.iter().copied().collect::<HashSet<_>>();
    let mut usages = Vec::new();
    let mut seen = HashSet::new();

    #[cfg(windows)]
    {
        let stdout = collect_port_listing_windows()?;
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
    }

    #[cfg(not(windows))]
    {
        for (protocol, stdout) in [
            ("TCP", collect_port_listing_unix("t")?),
            ("UDP", collect_port_listing_unix("u")?),
        ] {
            for line in stdout.lines() {
                let columns = line.split_whitespace().collect::<Vec<_>>();

                if columns.len() < 4 {
                    continue;
                }

                let local_address = columns[3];
                let Some(port) = parse_netstat_port(local_address) else {
                    continue;
                };
                if !wanted_ports.contains(&port) {
                    continue;
                }

                let Some(pid) = parse_ss_pid(line) else {
                    continue;
                };

                let key = format!("{protocol}:{port}:{pid}");

                if !seen.insert(key) {
                    continue;
                }

                usages.push(PortUsage {
                    port,
                    protocol: protocol.to_string(),
                    pid,
                    process_name: parse_ss_process_name(line, pid),
                });
            }
        }
    }

    Ok(usages)
}

#[cfg(windows)]
fn collect_port_listing_windows() -> Result<String, String> {
    #[cfg(windows)]
    {
        let mut command = Command::new("netstat");
        let output = hide_command_window(&mut command)
            .arg("-ano")
            .output()
            .map_err(|error| {
                format!(
                    "{}: {error}",
                    text(
                        "Could not check ports in use",
                        "Nao foi possivel verificar portas em uso"
                    )
                )
            })?;

        if !output.status.success() {
            return Err(text(
                "Could not check ports in use with netstat.",
                "Nao foi possivel verificar portas em uso com netstat.",
            )
            .to_string());
        }

        return Ok(String::from_utf8_lossy(&output.stdout).to_string());
    }

    #[cfg(not(windows))]
    {
        let output = Command::new("ss")
            .args(["-H", "-lntup"])
            .output()
            .map_err(|error| {
                format!(
                    "{}: {error}",
                    text(
                        "Could not check ports in use",
                        "Nao foi possivel verificar portas em uso"
                    )
                )
            })?;

        if !output.status.success() {
            return Err(text(
                "Could not check ports in use with ss.",
                "Nao foi possivel verificar portas em uso com ss.",
            )
            .to_string());
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[cfg(not(windows))]
fn collect_port_listing_unix(protocol_flag: &str) -> Result<String, String> {
    let output = Command::new("ss")
        .args(["-H", "-ln", &format!("-{protocol_flag}p")])
        .output()
        .map_err(|error| {
            format!(
                "{}: {error}",
                text(
                    "Could not check ports in use",
                    "Nao foi possivel verificar portas em uso"
                )
            )
        })?;

    if !output.status.success() {
        return Err(text(
            "Could not check ports in use with ss.",
            "Nao foi possivel verificar portas em uso com ss.",
        )
        .to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn parse_netstat_port(local_address: &str) -> Option<u16> {
    let port = local_address.rsplit_once(':')?.1;

    port.parse::<u16>().ok()
}

fn process_name_for_pid(pid: u32) -> String {
    #[cfg(windows)]
    {
        let mut command = Command::new("tasklist");
        let output = hide_command_window(&mut command)
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
            || line.eq_ignore_ascii_case(
                "INFO: No tasks are running which match the specified criteria.",
            )
        {
            return format!("PID {pid}");
        }

        return line
            .split(',')
            .next()
            .map(|value| value.trim_matches('"').to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| format!("PID {pid}"));
    }

    #[cfg(not(windows))]
    {
        let output = Command::new("sh")
            .args([
                "-lc",
                &format!("ss -H -lntup | grep 'pid={pid},' | head -n 1"),
            ])
            .output();
        let Ok(output) = output else {
            return format!("PID {pid}");
        };

        if !output.status.success() {
            return format!("PID {pid}");
        }

        let line = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if line.is_empty() {
            return format!("PID {pid}");
        }

        if let Some(name) = line
            .split("users:((\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
        {
            if !name.is_empty() {
                return name.to_string();
            }
        }

        format!("PID {pid}")
    }
}

#[cfg(not(windows))]
fn parse_ss_pid(line: &str) -> Option<u32> {
    let pid_index = line.find("pid=")? + 4;
    let digits = line[pid_index..]
        .chars()
        .take_while(|char| char.is_ascii_digit())
        .collect::<String>();

    digits.parse::<u32>().ok()
}

#[cfg(not(windows))]
fn parse_ss_process_name(line: &str, pid: u32) -> String {
    let Some(users_index) = line.find("users:") else {
        return format!("PID {pid}");
    };
    let users = &line[users_index..];

    if let Some(name) = users
        .split("(\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .filter(|value| !value.is_empty())
    {
        return name.to_string();
    }

    format!("PID {pid}")
}
