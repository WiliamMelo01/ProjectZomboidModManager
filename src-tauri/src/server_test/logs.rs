use crate::i18n::text;
use crate::models::ServerTestResult;
use std::path::Path;

pub(super) fn server_test_setup_error(
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

pub(super) fn find_critical_server_lines(log_lines: &[String]) -> Vec<String> {
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

pub(super) fn summarize_known_server_error(log_lines: &[String]) -> Option<String> {
    let combined_log = log_lines.join("\n").to_lowercase();

    if combined_log.contains("raknet.startup() return code: 5")
        || combined_log.contains("connection startup failed. code: 5")
    {
        return Some(
            text(
                "Failed to start the server network: the configured port appears to be in use or blocked. Check whether another Project Zomboid server is running or change the profile ports.",
                "Falha ao iniciar a rede do servidor: a porta configurada parece estar em uso ou bloqueada. Verifique se outro servidor Project Zomboid ja esta rodando ou altere as portas do perfil.",
            ).to_string(),
        );
    }

    None
}

pub(super) fn is_server_started_line(normalized_line: &str) -> bool {
    normalized_line.contains("*** server started")
        || normalized_line.contains("server is listening on port")
        || normalized_line.contains("raknet.startup() return code: 0")
        || normalized_line.contains("luanet: initialization [done]")
}

pub(super) fn count_warning_server_lines(log_lines: &[String]) -> usize {
    log_lines
        .iter()
        .filter(|line| is_warning_log_line(&line.to_lowercase()))
        .count()
}

fn is_warning_log_line(normalized_line: &str) -> bool {
    normalized_line.contains("warn")
}

pub(super) fn tail_log_lines(log_lines: Vec<String>, max_lines: usize) -> Vec<String> {
    let start = log_lines.len().saturating_sub(max_lines);

    log_lines.into_iter().skip(start).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_b42_luanet_initialization_as_server_started() {
        assert!(is_server_started_line(
            "log  : lua > luanet: initialization [done], triggering events for 'luanet.oninitadd'."
        ));
    }
}
