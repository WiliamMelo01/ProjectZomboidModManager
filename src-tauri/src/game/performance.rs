use super::paths::dedupe_paths;
use crate::util::read_text_lossy;
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) fn normalize_ram_gb(value: &str) -> Result<String, String> {
    let ram = value
        .trim()
        .replace(',', ".")
        .parse::<f64>()
        .map_err(|_| "Informe um valor valido de RAM.".to_string())?;

    if !ram.is_finite() || ram < 0.25 {
        return Err("A RAM precisa ser de pelo menos 0.25 GB.".to_string());
    }

    Ok(format!("{ram:.2}"))
}

fn ram_gb_to_mb(value: &str) -> Result<u32, String> {
    let ram = value
        .trim()
        .replace(',', ".")
        .parse::<f64>()
        .map_err(|_| "Informe um valor valido de RAM.".to_string())?;

    Ok((ram * 1024.0).round() as u32)
}

pub(crate) fn validate_game_executable_path(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!(
            "Executavel do Project Zomboid nao encontrado em {}.",
            path.display()
        ));
    }

    if !path.is_file() {
        return Err(format!(
            "O caminho {} nao aponta para um executavel.",
            path.display()
        ));
    }

    #[cfg(windows)]
    {
        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default();

        if !extension.eq_ignore_ascii_case("exe") {
            return Err("Selecione um arquivo .exe do Project Zomboid.".to_string());
        }
    }

    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;

        let metadata = fs::metadata(path).map_err(|error| {
            format!(
                "Nao foi possivel verificar as permissoes de {}: {error}",
                path.display()
            )
        })?;
        let mode = metadata.permissions().mode();
        if mode & 0o111 == 0 {
            return Err(format!(
                "O arquivo {} nao parece ser executavel. Marque permissoes de execucao ou selecione o launcher correto.",
                path.display()
            ));
        }
    }

    Ok(())
}

pub(crate) fn apply_performance_settings(
    game_executable: &Path,
    client_ram: &str,
    server_ram: &str,
) -> Result<(), String> {
    let game_dir = game_executable
        .parent()
        .ok_or_else(|| "Nao foi possivel localizar a pasta do executavel.".to_string())?;

    let client_mb = ram_gb_to_mb(client_ram)?;
    let server_mb = ram_gb_to_mb(server_ram)?;
    let client_configs = client_config_candidates(game_executable);
    let server_configs = server_config_candidates(game_dir);
    let updated_client = update_launcher_configs(&client_configs, client_mb)?;
    let mut updated_server = false;

    for candidate in server_configs {
        if !candidate.exists() || !candidate.is_file() {
            continue;
        }

        let extension = candidate
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_lowercase();

        if extension == "json" {
            update_launcher_json(&candidate, server_mb)?;
            updated_server = true;
        } else if extension == "bat" || extension == "sh" {
            updated_server = update_launcher_batch(&candidate, server_mb)? || updated_server;
        }
    }

    if !updated_client {
        return Err(format!(
            "Nao encontrei arquivos de configuracao do launcher ao lado de {}.",
            game_executable.display()
        ));
    }

    let _ = updated_server;

    Ok(())
}

pub(super) fn client_config_candidates(game_executable: &Path) -> Vec<PathBuf> {
    let game_dir = game_executable.parent().unwrap_or_else(|| Path::new(""));
    let mut candidates = Vec::new();

    if let Some(stem) = game_executable.file_stem().and_then(|name| name.to_str()) {
        candidates.push(game_dir.join(format!("{stem}.json")));
        candidates.push(game_dir.join(format!("{stem}.bat")));
        candidates.push(game_dir.join(format!("{stem}.sh")));
    }

    #[cfg(windows)]
    candidates.extend([
        game_dir.join("ProjectZomboid64.json"),
        game_dir.join("ProjectZomboid32.json"),
        game_dir.join("ProjectZomboid64.bat"),
        game_dir.join("ProjectZomboid32.bat"),
    ]);
    #[cfg(not(windows))]
    candidates.extend([
        game_dir.join("ProjectZomboid64.json"),
        game_dir.join("ProjectZomboid32.json"),
        game_dir.join("ProjectZomboid64.sh"),
        game_dir.join("ProjectZomboid32.sh"),
    ]);
    dedupe_paths(candidates)
}

pub(super) fn server_config_candidates(game_dir: &Path) -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        dedupe_paths(vec![game_dir.join("ProjectZomboidServer.bat")])
    }

    #[cfg(not(windows))]
    {
        dedupe_paths(vec![
            game_dir.join("start-server.sh"),
            game_dir.join("ProjectZomboidServer.sh"),
        ])
    }
}

fn update_launcher_configs(paths: &[PathBuf], ram_mb: u32) -> Result<bool, String> {
    let mut updated_any = false;

    for path in paths {
        if !path.exists() || !path.is_file() {
            continue;
        }

        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_lowercase();

        if extension == "json" {
            update_launcher_json(path, ram_mb)?;
            updated_any = true;
        } else if extension == "bat" {
            updated_any = update_launcher_batch(path, ram_mb)? || updated_any;
        }
    }

    Ok(updated_any)
}

fn update_launcher_json(path: &Path, ram_mb: u32) -> Result<(), String> {
    let content = read_text_lossy(path)?;
    let mut data = serde_json::from_str::<Value>(&content)
        .map_err(|error| format!("Nao foi possivel ler {} como JSON: {error}", path.display()))?;

    match data.get_mut("vmArgs") {
        Some(Value::Array(args)) => update_vm_args_array(args, ram_mb),
        Some(Value::String(args)) => {
            *args = update_vm_args_line(args, ram_mb);
        }
        _ => {
            if let Some(object) = data.as_object_mut() {
                object.insert(
                    "vmArgs".to_string(),
                    Value::Array(vec![
                        Value::String(format!("-Xms{ram_mb}m")),
                        Value::String(format!("-Xmx{ram_mb}m")),
                    ]),
                );
            }
        }
    }

    let content = serde_json::to_string_pretty(&data)
        .map_err(|error| format!("Nao foi possivel gerar JSON atualizado: {error}"))?;
    fs::write(path, format!("{content}\n"))
        .map_err(|error| format!("Nao foi possivel salvar {}: {error}", path.display()))
}

fn update_vm_args_array(args: &mut Vec<Value>, ram_mb: u32) {
    let mut has_xms = false;
    let mut has_xmx = false;

    for arg in args.iter_mut() {
        let Some(value) = arg.as_str() else {
            continue;
        };

        if is_memory_arg(value, "-Xms") {
            *arg = Value::String(format!("-Xms{ram_mb}m"));
            has_xms = true;
        } else if is_memory_arg(value, "-Xmx") {
            *arg = Value::String(format!("-Xmx{ram_mb}m"));
            has_xmx = true;
        }
    }

    if !has_xms {
        args.insert(0, Value::String(format!("-Xms{ram_mb}m")));
    }

    if !has_xmx {
        args.insert(1, Value::String(format!("-Xmx{ram_mb}m")));
    }
}

fn update_launcher_batch(path: &Path, ram_mb: u32) -> Result<bool, String> {
    let content = read_text_lossy(path)?;
    let lower_content = content.to_lowercase();

    if !lower_content.contains("-xms") && !lower_content.contains("-xmx") {
        return Ok(false);
    }

    let updated = content
        .lines()
        .map(|line| {
            let lower_line = line.to_lowercase();

            if lower_line.contains("-xms") || lower_line.contains("-xmx") {
                update_vm_args_line(line, ram_mb)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(if content.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        });

    fs::write(path, updated)
        .map_err(|error| format!("Nao foi possivel salvar {}: {error}", path.display()))?;

    Ok(true)
}

fn update_vm_args_line(content: &str, ram_mb: u32) -> String {
    let tokens = content
        .split_whitespace()
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    if let Some(java_index) = tokens.iter().position(|token| is_java_command_token(token)) {
        let mut java_index_after_filter = None;
        let mut filtered_tokens = Vec::new();

        for (index, token) in tokens.into_iter().enumerate() {
            if is_memory_arg(&token, "-Xms") || is_memory_arg(&token, "-Xmx") {
                continue;
            }

            if index == java_index {
                java_index_after_filter = Some(filtered_tokens.len());
            }

            filtered_tokens.push(token);
        }

        if let Some(index) = java_index_after_filter {
            filtered_tokens.insert(index + 1, format!("-Xmx{ram_mb}m"));
            filtered_tokens.insert(index + 1, format!("-Xms{ram_mb}m"));

            return filtered_tokens.join(" ");
        }
    }

    let mut has_xms = false;
    let mut has_xmx = false;
    let updated = content
        .split_whitespace()
        .map(|token| {
            if is_memory_arg(token, "-Xms") {
                has_xms = true;
                format!("-Xms{ram_mb}m")
            } else if is_memory_arg(token, "-Xmx") {
                has_xmx = true;
                format!("-Xmx{ram_mb}m")
            } else {
                token.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    match (has_xms, has_xmx) {
        (true, true) => updated,
        (false, true) => format!("-Xms{ram_mb}m {updated}"),
        (true, false) => updated.replace(
            &format!("-Xms{ram_mb}m"),
            &format!("-Xms{ram_mb}m -Xmx{ram_mb}m"),
        ),
        (false, false) => format!("-Xms{ram_mb}m -Xmx{ram_mb}m {updated}"),
    }
}

fn is_memory_arg(value: &str, prefix: &str) -> bool {
    let value = value.trim();

    value.len() > prefix.len()
        && value
            .get(..prefix.len())
            .is_some_and(|current_prefix| current_prefix.eq_ignore_ascii_case(prefix))
}

fn is_java_command_token(value: &str) -> bool {
    let value = value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .replace('/', "\\")
        .to_lowercase();

    value == "java"
        || value == "java.exe"
        || value.ends_with("\\java")
        || value.ends_with("\\java.exe")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_ram_values_with_comma() {
        assert_eq!(normalize_ram_gb("4,5"), Ok("4.50".to_string()));
    }

    #[test]
    fn replaces_memory_arguments_after_java_command() {
        assert_eq!(
            update_vm_args_line("java -Xms1g -Xmx2g -cp zomboid.jar", 4096),
            "java -Xms4096m -Xmx4096m -cp zomboid.jar"
        );
    }
}
