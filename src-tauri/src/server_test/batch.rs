use crate::i18n::text;
#[cfg(windows)]
use crate::util::read_text_lossy;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

const SERVER_TEST_ADMIN_PASSWORD: &str = "admin";

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ServerLaunchOptions {
    pub(crate) no_steam: bool,
}

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
    create_server_launch_batch(
        game_dir,
        bat_path,
        server_id,
        ServerLaunchOptions::default(),
    )
}

#[cfg(windows)]
pub(crate) fn create_server_launch_batch(
    game_dir: &Path,
    bat_path: &Path,
    server_id: &str,
    options: ServerLaunchOptions,
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
    let (updated_content, injected_server_name) =
        build_windows_server_launch_batch_content(&content, &game_dir_text, server_id, options);

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
    create_server_launch_batch(
        game_dir,
        launcher_path,
        server_id,
        ServerLaunchOptions::default(),
    )
}

#[cfg(not(windows))]
pub(crate) fn create_server_launch_batch(
    game_dir: &Path,
    launcher_path: &Path,
    server_id: &str,
    options: ServerLaunchOptions,
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
    let updated_content = build_unix_server_launch_script_content(
        &game_dir.display().to_string(),
        &launcher_path.display().to_string(),
        server_id,
        options,
        launcher_already_contains_no_steam(launcher_path),
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

#[cfg(any(not(windows), test))]
fn build_unix_server_launch_script_content(
    game_dir: &str,
    launcher_path: &str,
    server_id: &str,
    options: ServerLaunchOptions,
    launcher_has_no_steam: bool,
) -> String {
    let no_steam_arg = if options.no_steam && !launcher_has_no_steam {
        " -nosteam"
    } else {
        ""
    };

    format!(
        "#!/usr/bin/env sh\nset -eu\ncd {}\nexec {} -servername {} -adminpassword {}{}\n",
        shell_quote(game_dir.to_string()),
        shell_quote(launcher_path.to_string()),
        shell_quote(server_id.to_string()),
        shell_quote(SERVER_TEST_ADMIN_PASSWORD.to_string()),
        no_steam_arg
    )
}

#[cfg(windows)]
fn build_windows_server_launch_batch_content(
    content: &str,
    game_dir_text: &str,
    server_id: &str,
    options: ServerLaunchOptions,
) -> (String, bool) {
    let mut injected_server_name = false;
    let updated_content = content
        .lines()
        .map(|line| {
            if line.trim().eq_ignore_ascii_case("PAUSE") {
                return "REM PAUSE disabled by PZMM server test".to_string();
            }

            let mut line = line.replace("%~dp0", game_dir_text);

            if line.contains("zombie.network.GameServer") {
                line = replace_servername_argument(&line, server_id);
                injected_server_name = true;
            }

            if line.contains("zombie.network.GameServer")
                && !line.to_lowercase().contains("-adminpassword")
            {
                line.push_str(&format!(" -adminpassword {SERVER_TEST_ADMIN_PASSWORD}"));
            }

            if options.no_steam
                && line.contains("zombie.network.GameServer")
                && !line.to_lowercase().contains("-nosteam")
            {
                line.push_str(" -nosteam");
            }

            line
        })
        .collect::<Vec<_>>()
        .join("\r\n");

    (updated_content, injected_server_name)
}

#[cfg(windows)]
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
fn launcher_already_contains_no_steam(launcher_path: &Path) -> bool {
    fs::read_to_string(launcher_path)
        .map(|content| content.to_lowercase().contains("-nosteam"))
        .unwrap_or(false)
}

#[cfg(any(not(windows), test))]
fn shell_quote(value: String) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn windows_launch_batch_adds_nosteam_when_requested() {
        let content = r#"java -cp . zombie.network.GameServer -servername old"#;

        let (updated, _) = build_windows_server_launch_batch_content(
            content,
            "C:\\PZ",
            "servertest",
            ServerLaunchOptions { no_steam: true },
        );

        assert!(updated.contains("-servername servertest"));
        assert!(updated.contains("-adminpassword admin"));
        assert!(updated.contains("-nosteam"));
    }

    #[cfg(windows)]
    #[test]
    fn windows_launch_batch_does_not_duplicate_nosteam() {
        let content = r#"java -cp . zombie.network.GameServer -servername old -nosteam"#;

        let (updated, _) = build_windows_server_launch_batch_content(
            content,
            "C:\\PZ",
            "servertest",
            ServerLaunchOptions { no_steam: true },
        );

        assert_eq!(updated.to_lowercase().matches("-nosteam").count(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn windows_launch_batch_omits_nosteam_by_default() {
        let content = r#"java -cp . zombie.network.GameServer -servername old"#;

        let (updated, _) = build_windows_server_launch_batch_content(
            content,
            "C:\\PZ",
            "servertest",
            ServerLaunchOptions { no_steam: false },
        );

        assert!(!updated.contains("-nosteam"));
        assert!(updated.contains("-servername servertest"));
        assert!(updated.contains("-adminpassword admin"));
    }

    #[test]
    fn unix_launch_script_adds_nosteam_when_requested() {
        let script = build_unix_server_launch_script_content(
            "/opt/pz",
            "/opt/pz/start-server.sh",
            "servertest",
            ServerLaunchOptions { no_steam: true },
            false,
        );

        assert!(script.contains("exec '/opt/pz/start-server.sh'"));
        assert!(script.contains("-servername 'servertest'"));
        assert!(script.contains("-adminpassword 'admin'"));
        assert!(script.contains("-nosteam"));
    }

    #[test]
    fn unix_launch_script_does_not_duplicate_nosteam() {
        let script = build_unix_server_launch_script_content(
            "/opt/pz",
            "/opt/pz/start-server.sh",
            "servertest",
            ServerLaunchOptions { no_steam: true },
            true,
        );

        assert_eq!(script.matches("-nosteam").count(), 0);
    }

    #[test]
    fn unix_launch_script_omits_nosteam_by_default() {
        let script = build_unix_server_launch_script_content(
            "/opt/pz",
            "/opt/pz/start-server.sh",
            "servertest",
            ServerLaunchOptions { no_steam: false },
            false,
        );

        assert!(!script.contains("-nosteam"));
        assert!(script.contains("-servername 'servertest'"));
        assert!(script.contains("-adminpassword 'admin'"));
    }
}
