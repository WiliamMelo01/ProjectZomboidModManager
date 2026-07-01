use crate::i18n::text;
use crate::util::read_text_lossy;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

pub(crate) fn default_server_launcher_name() -> &'static str {
    if cfg!(windows) {
        "ProjectZomboidServer.bat"
    } else {
        "start-server.sh"
    }
}

#[cfg(windows)]
pub(crate) fn create_server_test_batch(
    game_dir: &Path,
    bat_path: &Path,
    server_id: &str,
) -> Result<PathBuf, String> {
    if !server_id
        .chars()
        .all(|char| char.is_ascii_alphanumeric() || char == '_' || char == '-')
    {
        return Err(text(
            "The server identifier contains invalid characters for testing.",
            "O identificador do servidor contem caracteres invalidos para teste.",
        )
        .to_string());
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

            if line.contains("zombie.network.GameServer") {
                line = replace_servername_argument(&line, server_id);
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
        return Err(text(
            "Could not prepare the test: GameServer line not found in the launcher script.",
            "Nao foi possivel preparar o teste: linha GameServer nao encontrada no launcher.",
        )
        .to_string());
    }

    let test_bat_path = env::temp_dir().join(format!("pzmm-test-{server_id}.bat"));

    fs::write(&test_bat_path, updated_content).map_err(|error| {
        format!(
            "{}: {error}",
            text(
                "Could not create the temporary test .bat file",
                "Nao foi possivel criar .bat temporario de teste"
            )
        )
    })?;

    Ok(test_bat_path)
}

#[cfg(not(windows))]
pub(crate) fn create_server_test_batch(
    game_dir: &Path,
    launcher_path: &Path,
    server_id: &str,
) -> Result<PathBuf, String> {
    if !server_id
        .chars()
        .all(|char| char.is_ascii_alphanumeric() || char == '_' || char == '-')
    {
        return Err(text(
            "The server identifier contains invalid characters for testing.",
            "O identificador do servidor contem caracteres invalidos para teste.",
        )
        .to_string());
    }

    if !launcher_path.exists() || !launcher_path.is_file() {
        return Err(text(
            "Could not prepare the test: server launcher not found.",
            "Nao foi possivel preparar o teste: inicializador do servidor nao encontrado.",
        )
        .to_string());
    }

    let test_script_path = env::temp_dir().join(format!("pzmm-test-{server_id}.sh"));
    let updated_content = format!(
        "#!/usr/bin/env sh\nset -eu\ncd {}\nexec {} -servername {}\n",
        shell_quote(game_dir.display().to_string()),
        shell_quote(launcher_path.display().to_string()),
        shell_quote(server_id.to_string())
    );

    fs::write(&test_script_path, updated_content).map_err(|error| {
        format!(
            "{}: {error}",
            text(
                "Could not create the temporary test script",
                "Nao foi possivel criar o script temporario de teste"
            )
        )
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let metadata = fs::metadata(&test_script_path).map_err(|error| {
            format!(
                "{}: {error}",
                text(
                    "Could not update the temporary test script permissions",
                    "Nao foi possivel atualizar as permissoes do script temporario de teste"
                )
            )
        })?;
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&test_script_path, permissions).map_err(|error| {
            format!(
                "{}: {error}",
                text(
                    "Could not update the temporary test script permissions",
                    "Nao foi possivel atualizar as permissoes do script temporario de teste"
                )
            )
        })?;
    }

    Ok(test_script_path)
}

fn replace_servername_argument(line: &str, server_id: &str) -> String {
    let lower_line = line.to_lowercase();
    let Some(start) = lower_line.find("-servername") else {
        return format!("{line} -servername {server_id}");
    };

    let after_flag = start + "-servername".len();
    let bytes = line.as_bytes();
    let mut value_start = after_flag;

    while value_start < bytes.len() && bytes[value_start].is_ascii_whitespace() {
        value_start += 1;
    }

    let mut value_end = value_start;
    if value_start < bytes.len() && bytes[value_start] == b'"' {
        value_end += 1;
        while value_end < bytes.len() && bytes[value_end] != b'"' {
            value_end += 1;
        }
        if value_end < bytes.len() {
            value_end += 1;
        }
    } else {
        while value_end < bytes.len() && !bytes[value_end].is_ascii_whitespace() {
            value_end += 1;
        }
    }

    format!(
        "{}-servername {}{}",
        &line[..start],
        server_id,
        &line[value_end..]
    )
}

#[cfg(not(windows))]
fn shell_quote(value: String) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}
