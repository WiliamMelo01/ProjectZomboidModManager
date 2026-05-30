use super::logs::tail_log_lines;
use crate::game::steam_zomboid_game_dirs;
use crate::models::ServerTestResult;
use crate::mods::list_zomboid_mods_impl;
use crate::read_config_value;
use crate::util::{read_ini_value, read_text_lossy, split_mod_ids};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

pub(super) fn resolve_zomboid_game_dir() -> Result<Option<PathBuf>, String> {
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

pub(super) fn validate_server_mod_dependencies(
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
