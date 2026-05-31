use crate::i18n::text;
use crate::util::read_text_lossy;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

pub(super) fn create_server_test_batch(
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
        return Err(text(
            "Could not prepare the test: GameServer line not found in the .bat file.",
            "Nao foi possivel preparar o teste: linha GameServer nao encontrada no .bat.",
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
