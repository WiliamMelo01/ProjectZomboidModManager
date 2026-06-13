use super::logs::tail_log_lines;
use crate::game::steam_zomboid_game_dirs;
use crate::i18n::text;
use crate::models::{ServerTestResult, ZomboidMod, ZomboidModVariant};
use crate::mods::{list_zomboid_mods_impl, parse_server_mod_ids};
use crate::read_config_value;
use crate::servers::read_zomboid_server_build;
use crate::util::{read_ini_value, read_text_lossy};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

pub(crate) fn resolve_zomboid_game_dir() -> Result<Option<PathBuf>, String> {
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

pub(crate) fn validate_server_mod_dependencies(
    server_id: &str,
    server_path: &Path,
) -> Result<Option<ServerTestResult>, String> {
    let content = read_text_lossy(server_path)?;
    let game_build = read_zomboid_server_build(server_id)?;
    let active_mod_ids = read_ini_value(&content, "Mods")
        .map(|value| parse_server_mod_ids(&value))
        .unwrap_or_default();

    if active_mod_ids.is_empty() {
        return Ok(None);
    }

    let active_positions = active_mod_ids
        .iter()
        .enumerate()
        .map(|(index, mod_id)| (mod_id.to_lowercase(), index))
        .collect::<HashMap<_, _>>();
    let mods_by_id = build_variant_lookup(list_zomboid_mods_impl()?, &game_build);
    let mut issues = Vec::new();

    for (mod_index, mod_id) in active_mod_ids.iter().enumerate() {
        let normalized_mod_id = mod_id.to_lowercase();
        let Some(zomboid_mod) = mods_by_id.get(&normalized_mod_id) else {
            issues.push(format!(
                "[ERR] {} '{mod_id}' {} {server_id}, {} {}.",
                text("Mod", "Mod"),
                text("is active in", "esta ativo em"),
                text(
                    "but was not found as compatible with",
                    "mas nao foi encontrado como compativel com"
                ),
                game_build.to_uppercase()
            ));
            continue;
        };

        for dependency_id in &zomboid_mod.dependencies {
            let normalized_dependency_id = dependency_id
                .strip_prefix('\\')
                .unwrap_or(dependency_id)
                .to_lowercase();
            let Some(dependency_index) = active_positions.get(&normalized_dependency_id) else {
                issues.push(format!(
                    "[ERR] {} '{mod_id}' {} '{dependency_id}', {}.",
                    text("Mod", "Mod"),
                    text("requires", "requer"),
                    text(
                        "but this dependency is not active on the server",
                        "mas essa dependencia nao esta ativa no servidor"
                    )
                ));
                continue;
            };

            if *dependency_index > mod_index {
                issues.push(format!(
                    "[ERR] {}: '{mod_id}' {} '{dependency_id}'. {} '{dependency_id}' {} '{mod_id}' {} Mods=.",
                    text("Invalid order", "Ordem invalida"),
                    text("is before its dependency", "esta antes de sua dependencia"),
                    text("Place", "Coloque"),
                    text("before", "antes de"),
                    text("in", "em")
                ));
            }
        }
    }

    if issues.is_empty() {
        return Ok(None);
    }

    let issue_count = issues.len();
    let mut log_lines = vec![
        format!(
            "[HELP] {}",
            text(
                "Fix the items below before starting the server.",
                "Corrija os itens abaixo antes de iniciar o servidor."
            )
        ),
        format!(
            "[HELP] {}",
            text(
                "Dependencies must be active and appear before the mods that require them in Mods=.",
                "As dependencias precisam estar ativas e aparecer antes dos mods que dependem delas em Mods=."
            )
        ),
    ];
    log_lines.extend(issues);

    Ok(Some(ServerTestResult {
        status: "failed".to_string(),
        summary: format!(
            "{} {} {}.",
            text(
                "Dependency validation found",
                "Validacao de dependencias encontrou"
            ),
            issue_count,
            text(
                "issue(s) before starting the server",
                "problema(s) antes de iniciar o servidor"
            )
        ),
        duration_seconds: 0,
        bat_path: "ProjectZomboidServer.bat".to_string(),
        command: format!(
            "preflight: {}",
            text(
                "validate dependencies and Mods= order",
                "validar dependencias e ordem de Mods="
            )
        ),
        warning_count: 0,
        critical_count: log_lines
            .iter()
            .filter(|line| line.starts_with("[ERR]"))
            .count(),
        log_lines: tail_log_lines(log_lines, 240),
    }))
}

fn build_variant_lookup(
    mods: Vec<ZomboidMod>,
    game_build: &str,
) -> HashMap<String, ZomboidModVariant> {
    mods.into_iter()
        .flat_map(|mod_item| mod_item.variants)
        .filter(|variant| variant.game_build == game_build)
        .map(|variant| (variant.id.to_lowercase(), variant))
        .collect()
}
