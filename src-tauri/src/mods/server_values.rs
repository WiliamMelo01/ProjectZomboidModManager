use super::discovery::{find_mod_info_files, steam_workshop_dirs};
use crate::saved_custom_mod_dirs;
use crate::util::{read_ini_value, read_text_lossy};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

pub(crate) fn normalize_server_values(values: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();

    for value in values {
        let value = value.trim();

        if value.is_empty() {
            continue;
        }

        let key = value.to_lowercase();

        if seen.insert(key) {
            normalized.push(value.to_string());
        }
    }

    normalized
}

pub(crate) fn resolve_server_workshop_ids(
    mod_ids: &[String],
    workshop_ids: &[String],
) -> Result<Vec<String>, String> {
    let mut resolved = normalize_server_values(workshop_ids);
    let mut seen = resolved
        .iter()
        .map(|workshop_id| workshop_id.to_lowercase())
        .collect::<HashSet<_>>();
    let workshop_lookup = build_workshop_id_lookup(mod_ids)?;

    for mod_id in normalize_server_values(mod_ids) {
        if let Some(workshop_id) = workshop_lookup.get(&mod_id.to_lowercase()) {
            if seen.insert(workshop_id.to_lowercase()) {
                resolved.push(workshop_id.clone());
            }
        }
    }

    Ok(resolved)
}

fn build_workshop_id_lookup(mod_ids: &[String]) -> Result<HashMap<String, String>, String> {
    let wanted_mod_ids = normalize_server_values(mod_ids)
        .into_iter()
        .map(|mod_id| mod_id.to_lowercase())
        .collect::<HashSet<_>>();
    let mut lookup = HashMap::new();

    if wanted_mod_ids.is_empty() {
        return Ok(lookup);
    }

    for workshop_dir in steam_workshop_dirs() {
        if workshop_dir.exists() {
            collect_workshop_id_lookup(&workshop_dir, &wanted_mod_ids, &mut lookup)?;
        }
    }

    for custom_dir in saved_custom_mod_dirs()? {
        if custom_dir.exists() {
            collect_workshop_id_lookup(&custom_dir, &wanted_mod_ids, &mut lookup)?;
        }
    }

    Ok(lookup)
}

fn collect_workshop_id_lookup(
    workshop_root: &Path,
    wanted_mod_ids: &HashSet<String>,
    lookup: &mut HashMap<String, String>,
) -> Result<(), String> {
    let entries = fs::read_dir(workshop_root)
        .map_err(|error| format!("Nao foi possivel ler {}: {error}", workshop_root.display()))?;

    for entry in entries {
        if lookup.len() >= wanted_mod_ids.len() {
            break;
        }

        let entry = entry.map_err(|error| error.to_string())?;
        let item_path = entry.path();

        if !item_path.is_dir() {
            continue;
        }

        let workshop_id = item_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();

        if workshop_id.is_empty() || !workshop_id.chars().all(|char| char.is_ascii_digit()) {
            continue;
        }

        let search_root = item_path.join("mods");
        let search_root = if search_root.exists() {
            search_root
        } else {
            item_path
        };

        for mod_info in find_mod_info_files(&search_root)? {
            let content = read_text_lossy(&mod_info)?;
            let mod_id = read_ini_value(content.as_ref(), "id").unwrap_or_else(|| {
                mod_info
                    .parent()
                    .and_then(|path| path.file_name())
                    .and_then(|name| name.to_str())
                    .unwrap_or("unknown")
                    .to_string()
            });
            let normalized_mod_id = mod_id.to_lowercase();

            if wanted_mod_ids.contains(&normalized_mod_id) {
                lookup
                    .entry(normalized_mod_id)
                    .or_insert_with(|| workshop_id.clone());
            }
        }
    }

    Ok(())
}
