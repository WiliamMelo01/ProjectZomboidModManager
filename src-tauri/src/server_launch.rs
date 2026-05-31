use crate::models::ZomboidServerStarted;
use crate::server_test::{resolve_zomboid_game_dir, validate_server_mod_dependencies};
use crate::{run_blocking, zomboid_server_dir};
use std::{
    path::Path,
    process::{Command, Stdio},
};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

#[tauri::command]
pub(crate) async fn start_zomboid_server(
    server_id: String,
) -> Result<ZomboidServerStarted, String> {
    run_blocking(move || start_zomboid_server_impl(&server_id)).await
}

fn start_zomboid_server_impl(server_id: &str) -> Result<ZomboidServerStarted, String> {
    let server_id = server_id.trim();
    validate_server_id(server_id)?;

    let server_path = zomboid_server_dir()?.join(format!("{server_id}.ini"));

    if !server_path.exists() {
        return Err(format!(
            "Arquivo do servidor nao encontrado: {}.",
            server_path.display()
        ));
    }

    if let Some(result) = validate_server_mod_dependencies(server_id, &server_path)? {
        return Err(format!("{} {}", result.summary, result.log_lines.join(" ")));
    }

    let game_dir = resolve_zomboid_game_dir()?.ok_or_else(|| {
        "Pasta do Project Zomboid nao encontrada. Configure o executavel do jogo nas configuracoes."
            .to_string()
    })?;
    let bat_path = game_dir.join("ProjectZomboidServer.bat");

    if !bat_path.exists() || !bat_path.is_file() {
        return Err(format!(
            "ProjectZomboidServer.bat nao encontrado em {}.",
            game_dir.display()
        ));
    }

    let mut command = server_command(&game_dir, &bat_path, server_id);

    #[cfg(windows)]
    command.creation_flags(CREATE_NEW_CONSOLE);

    let child = command
        .spawn()
        .map_err(|error| format!("Nao foi possivel iniciar o servidor: {error}"))?;

    Ok(ZomboidServerStarted {
        server_id: server_id.to_string(),
        pid: child.id(),
    })
}

fn server_command(game_dir: &Path, bat_path: &Path, server_id: &str) -> Command {
    let mut command = Command::new("cmd.exe");
    command
        .arg("/C")
        .arg("call")
        .arg(bat_path)
        .arg("-servername")
        .arg(server_id)
        .current_dir(game_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

fn validate_server_id(server_id: &str) -> Result<(), String> {
    if server_id.is_empty()
        || !server_id
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || char == '_' || char == '-')
    {
        return Err("Identificador de servidor invalido.".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_server_id;

    #[test]
    fn accepts_safe_server_ids() {
        assert!(validate_server_id("meu-servidor_01").is_ok());
    }

    #[test]
    fn rejects_server_ids_with_shell_characters() {
        assert!(validate_server_id("server & calc").is_err());
    }
}
