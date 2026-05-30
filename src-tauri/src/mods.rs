use crate::models::ZomboidMod;
use crate::util::*;
use crate::{
    find_steamcmd_path, read_steam_library_dirs, run_blocking, saved_custom_mod_dirs,
    zomboid_mods_dir,
};
use base64::{engine::general_purpose, Engine as _};
use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
};

const LOCAL_WORKSHOP_ID_FILE: &str = ".pzmm-workshop-id";
#[tauri::command]
pub(crate) async fn list_zomboid_mods() -> Result<Vec<ZomboidMod>, String> {
    run_blocking(list_zomboid_mods_impl).await
}

pub(crate) fn list_zomboid_mods_impl() -> Result<Vec<ZomboidMod>, String> {
    let mut mods = Vec::new();
    let mut seen = HashSet::new();
    let mut installed_mod_ids = HashSet::new();

    if let Ok(local_mods_dir) = zomboid_mods_dir() {
        if local_mods_dir.exists() {
            collect_local_mods(
                &local_mods_dir,
                &mut mods,
                &mut seen,
                &mut installed_mod_ids,
            )?;
        }
    }

    for workshop_dir in steam_workshop_dirs() {
        if workshop_dir.exists() {
            collect_steam_workshop_mods(&workshop_dir, &mut mods, &mut seen, &installed_mod_ids)?;
        }
    }

    for custom_dir in saved_custom_mod_dirs()? {
        if custom_dir.exists() {
            collect_custom_mods(&custom_dir, &mut mods, &mut seen, &installed_mod_ids)?;
        }
    }

    mods.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));

    Ok(mods)
}

#[tauri::command]
pub(crate) async fn count_zomboid_mods() -> Result<usize, String> {
    run_blocking(count_zomboid_mods_impl).await
}

fn count_zomboid_mods_impl() -> Result<usize, String> {
    let mut seen = HashSet::new();
    let mut installed_mod_ids = HashSet::new();

    if let Ok(local_mods_dir) = zomboid_mods_dir() {
        if local_mods_dir.exists() {
            collect_local_mod_ids(&local_mods_dir, &mut seen, &mut installed_mod_ids)?;
        }
    }

    for workshop_dir in steam_workshop_dirs() {
        if workshop_dir.exists() {
            collect_steam_workshop_mod_ids(&workshop_dir, &mut seen, &installed_mod_ids)?;
        }
    }

    for custom_dir in saved_custom_mod_dirs()? {
        if custom_dir.exists() {
            collect_custom_mod_ids(&custom_dir, &mut seen, &installed_mod_ids)?;
        }
    }

    Ok(seen.len())
}

#[tauri::command]
pub(crate) fn install_zomboid_mod(
    mod_path: String,
    mod_id: String,
    workshop_id: String,
) -> Result<(), String> {
    let source = PathBuf::from(&mod_path);

    if !source.exists() || !source.is_dir() {
        return Err(format!("Pasta do mod nao encontrada: {}", source.display()));
    }

    let target_root = zomboid_mods_dir()?;
    fs::create_dir_all(&target_root)
        .map_err(|error| format!("Nao foi possivel criar {}: {error}", target_root.display()))?;

    install_mod(&source, &mod_id, &target_root, Some(&workshop_id))
}

fn install_mod(
    source: &Path,
    mod_id: &str,
    target_root: &Path,
    workshop_id: Option<&str>,
) -> Result<(), String> {
    let folder_name = if mod_id.trim().is_empty() {
        source
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("mod")
            .to_string()
    } else {
        sanitize_folder_name(&mod_id)
    };
    let target = target_root.join(folder_name);

    if !target.exists() {
        copy_dir_recursive(source, &target)?;
    }

    write_local_workshop_id(&target, workshop_id)?;

    Ok(())
}

fn collect_local_mods(
    local_mods_dir: &Path,
    mods: &mut Vec<ZomboidMod>,
    seen: &mut HashSet<String>,
    installed_mod_ids: &mut HashSet<String>,
) -> Result<(), String> {
    let entries = fs::read_dir(local_mods_dir)
        .map_err(|error| format!("Nao foi possivel ler {}: {error}", local_mods_dir.display()))?;

    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let direct_mod_info = path.join("mod.info");

        if direct_mod_info.exists() {
            let workshop_id = read_local_workshop_id(path.as_path());
            if let Some(mod_id) = add_mod_from_info(
                &direct_mod_info,
                workshop_id.as_deref(),
                "local",
                mods,
                seen,
                installed_mod_ids,
            )? {
                installed_mod_ids.insert(mod_id);
            }
            continue;
        }

        for mod_info in find_mod_info_files(&path)? {
            let mod_dir = mod_info.parent().unwrap_or(mod_info.as_path());
            let workshop_id = read_local_workshop_id(mod_dir);

            if let Some(mod_id) = add_mod_from_info(
                &mod_info,
                workshop_id.as_deref(),
                "local",
                mods,
                seen,
                installed_mod_ids,
            )? {
                installed_mod_ids.insert(mod_id);
            }
        }
    }

    Ok(())
}

fn collect_steam_workshop_mods(
    workshop_dir: &Path,
    mods: &mut Vec<ZomboidMod>,
    seen: &mut HashSet<String>,
    installed_mod_ids: &HashSet<String>,
) -> Result<(), String> {
    let workshop_items = fs::read_dir(workshop_dir)
        .map_err(|error| format!("Nao foi possivel ler {}: {error}", workshop_dir.display()))?;

    for item in workshop_items {
        let item = item.map_err(|error| error.to_string())?;
        let item_path = item.path();

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

        let mods_dir = item_path.join("mods");

        if mods_dir.exists() {
            let mod_folders = fs::read_dir(&mods_dir)
                .map_err(|error| format!("Nao foi possivel ler {}: {error}", mods_dir.display()))?;
            let mut found_direct_mod = false;

            for mod_folder in mod_folders {
                let mod_folder = mod_folder.map_err(|error| error.to_string())?;
                let mod_folder_path = mod_folder.path();

                if !mod_folder_path.is_dir() {
                    continue;
                }

                let direct_mod_info = mod_folder_path.join("mod.info");

                if direct_mod_info.exists() {
                    found_direct_mod = true;
                    add_mod_from_info(
                        &direct_mod_info,
                        Some(&workshop_id),
                        "steam",
                        mods,
                        seen,
                        installed_mod_ids,
                    )?;
                }
            }

            if found_direct_mod {
                continue;
            }
        }

        for mod_info in find_mod_info_files(&item_path)? {
            add_mod_from_info(
                &mod_info,
                Some(&workshop_id),
                "steam",
                mods,
                seen,
                installed_mod_ids,
            )?;
        }
    }

    Ok(())
}

fn collect_custom_mods(
    custom_dir: &Path,
    mods: &mut Vec<ZomboidMod>,
    seen: &mut HashSet<String>,
    installed_mod_ids: &HashSet<String>,
) -> Result<(), String> {
    let entries = fs::read_dir(custom_dir)
        .map_err(|error| format!("Nao foi possivel ler {}: {error}", custom_dir.display()))?;

    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let direct_mod_info = path.join("mod.info");

        if direct_mod_info.exists() {
            add_mod_from_info(
                &direct_mod_info,
                None,
                "custom",
                mods,
                seen,
                installed_mod_ids,
            )?;
            continue;
        }

        let workshop_id = path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| name.chars().all(|char| char.is_ascii_digit()));
        let mods_dir = path.join("mods");

        if let Some(workshop_id) = workshop_id {
            if mods_dir.exists() {
                let mod_folders = fs::read_dir(&mods_dir).map_err(|error| {
                    format!("Nao foi possivel ler {}: {error}", mods_dir.display())
                })?;
                let mut found_direct_mod = false;

                for mod_folder in mod_folders {
                    let mod_folder = mod_folder.map_err(|error| error.to_string())?;
                    let mod_folder_path = mod_folder.path();

                    if !mod_folder_path.is_dir() {
                        continue;
                    }

                    let direct_mod_info = mod_folder_path.join("mod.info");

                    if direct_mod_info.exists() {
                        found_direct_mod = true;
                        add_mod_from_info(
                            &direct_mod_info,
                            Some(workshop_id),
                            "custom",
                            mods,
                            seen,
                            installed_mod_ids,
                        )?;
                    }
                }

                if found_direct_mod {
                    continue;
                }
            }
        }

        for mod_info in find_mod_info_files(&path)? {
            add_mod_from_info(
                &mod_info,
                workshop_id,
                "custom",
                mods,
                seen,
                installed_mod_ids,
            )?;
        }
    }

    Ok(())
}

fn collect_local_mod_ids(
    local_mods_dir: &Path,
    seen: &mut HashSet<String>,
    installed_mod_ids: &mut HashSet<String>,
) -> Result<(), String> {
    let entries = fs::read_dir(local_mods_dir)
        .map_err(|error| format!("Nao foi possivel ler {}: {error}", local_mods_dir.display()))?;

    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let direct_mod_info = path.join("mod.info");

        if direct_mod_info.exists() {
            if let Some(mod_id) =
                add_mod_id_from_info(&direct_mod_info, None, "local", seen, installed_mod_ids)?
            {
                installed_mod_ids.insert(mod_id);
            }
            continue;
        }

        for mod_info in find_mod_info_files(&path)? {
            if let Some(mod_id) =
                add_mod_id_from_info(&mod_info, None, "local", seen, installed_mod_ids)?
            {
                installed_mod_ids.insert(mod_id);
            }
        }
    }

    Ok(())
}

fn collect_steam_workshop_mod_ids(
    workshop_dir: &Path,
    seen: &mut HashSet<String>,
    installed_mod_ids: &HashSet<String>,
) -> Result<(), String> {
    let workshop_items = fs::read_dir(workshop_dir)
        .map_err(|error| format!("Nao foi possivel ler {}: {error}", workshop_dir.display()))?;

    for item in workshop_items {
        let item = item.map_err(|error| error.to_string())?;
        let item_path = item.path();

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

        let mods_dir = item_path.join("mods");

        if mods_dir.exists() {
            let mod_folders = fs::read_dir(&mods_dir)
                .map_err(|error| format!("Nao foi possivel ler {}: {error}", mods_dir.display()))?;
            let mut found_direct_mod = false;

            for mod_folder in mod_folders {
                let mod_folder = mod_folder.map_err(|error| error.to_string())?;
                let mod_folder_path = mod_folder.path();

                if !mod_folder_path.is_dir() {
                    continue;
                }

                let direct_mod_info = mod_folder_path.join("mod.info");

                if direct_mod_info.exists() {
                    found_direct_mod = true;
                    add_mod_id_from_info(
                        &direct_mod_info,
                        Some(&workshop_id),
                        "steam",
                        seen,
                        installed_mod_ids,
                    )?;
                }
            }

            if found_direct_mod {
                continue;
            }
        }

        for mod_info in find_mod_info_files(&item_path)? {
            add_mod_id_from_info(
                &mod_info,
                Some(&workshop_id),
                "steam",
                seen,
                installed_mod_ids,
            )?;
        }
    }

    Ok(())
}

fn collect_custom_mod_ids(
    custom_dir: &Path,
    seen: &mut HashSet<String>,
    installed_mod_ids: &HashSet<String>,
) -> Result<(), String> {
    let entries = fs::read_dir(custom_dir)
        .map_err(|error| format!("Nao foi possivel ler {}: {error}", custom_dir.display()))?;

    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let direct_mod_info = path.join("mod.info");

        if direct_mod_info.exists() {
            add_mod_id_from_info(&direct_mod_info, None, "custom", seen, installed_mod_ids)?;
            continue;
        }

        let workshop_id = path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| name.chars().all(|char| char.is_ascii_digit()));
        let mods_dir = path.join("mods");

        if let Some(workshop_id) = workshop_id {
            if mods_dir.exists() {
                let mod_folders = fs::read_dir(&mods_dir).map_err(|error| {
                    format!("Nao foi possivel ler {}: {error}", mods_dir.display())
                })?;
                let mut found_direct_mod = false;

                for mod_folder in mod_folders {
                    let mod_folder = mod_folder.map_err(|error| error.to_string())?;
                    let mod_folder_path = mod_folder.path();

                    if !mod_folder_path.is_dir() {
                        continue;
                    }

                    let direct_mod_info = mod_folder_path.join("mod.info");

                    if direct_mod_info.exists() {
                        found_direct_mod = true;
                        add_mod_id_from_info(
                            &direct_mod_info,
                            Some(workshop_id),
                            "custom",
                            seen,
                            installed_mod_ids,
                        )?;
                    }
                }

                if found_direct_mod {
                    continue;
                }
            }
        }

        for mod_info in find_mod_info_files(&path)? {
            add_mod_id_from_info(&mod_info, workshop_id, "custom", seen, installed_mod_ids)?;
        }
    }

    Ok(())
}

fn add_mod_id_from_info(
    mod_info_path: &Path,
    workshop_id: Option<&str>,
    source: &str,
    seen: &mut HashSet<String>,
    installed_mod_ids: &HashSet<String>,
) -> Result<Option<String>, String> {
    let content = read_text_lossy(mod_info_path)?;
    let mod_id = read_ini_value(content.as_ref(), "id").unwrap_or_else(|| {
        mod_info_path
            .parent()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
            .to_string()
    });
    let normalized_mod_id = mod_id.to_lowercase();

    if source == "steam" && installed_mod_ids.contains(&normalized_mod_id) {
        return Ok(None);
    }

    let workshop_id = workshop_id.unwrap_or("");
    let seen_key = format!("{source}:{workshop_id}:{mod_id}");

    if seen.insert(seen_key) {
        Ok(Some(normalized_mod_id))
    } else {
        Ok(None)
    }
}

fn add_mod_from_info(
    mod_info_path: &Path,
    workshop_id: Option<&str>,
    source: &str,
    mods: &mut Vec<ZomboidMod>,
    seen: &mut HashSet<String>,
    installed_mod_ids: &HashSet<String>,
) -> Result<Option<String>, String> {
    let content = read_text_lossy(mod_info_path)?;
    let mod_id = read_ini_value(content.as_ref(), "id").unwrap_or_else(|| {
        mod_info_path
            .parent()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
            .to_string()
    });
    let normalized_mod_id = mod_id.to_lowercase();

    if source == "steam" {
        if installed_mod_ids.contains(&normalized_mod_id) {
            return Ok(None);
        }
    }

    let workshop_id = workshop_id.unwrap_or("").to_string();
    let seen_key = format!("{source}:{workshop_id}:{mod_id}");

    if seen.contains(&seen_key) {
        return Ok(None);
    }

    seen.insert(seen_key);

    let name = read_ini_value(content.as_ref(), "name")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| capitalize_first_letter(&mod_id));
    let author = read_ini_value(content.as_ref(), "Authors")
        .or_else(|| read_ini_value(content.as_ref(), "author"))
        .unwrap_or_else(|| "Desconhecido".to_string());
    let description = read_ini_value(content.as_ref(), "description")
        .map(|value| clean_mod_description(&value))
        .unwrap_or_else(|| "Sem descricao disponivel.".to_string());
    let version = read_ini_value(content.as_ref(), "version").unwrap_or_else(|| "-".to_string());
    let dependencies = parse_mod_dependencies(content.as_ref());
    let mod_dir = mod_info_path.parent().unwrap_or(mod_info_path);
    let image_url = find_mod_image_url(content.as_ref(), mod_dir);

    mods.push(ZomboidMod {
        id: mod_id,
        name,
        author,
        version,
        workshop_id,
        description,
        size: format_size(directory_size(mod_dir)),
        is_installed: source == "local",
        source: source.to_string(),
        path: mod_dir.display().to_string(),
        image_url,
        dependencies,
    });

    Ok(Some(normalized_mod_id))
}

fn copy_dir_recursive(source: &Path, target: &Path) -> Result<(), String> {
    fs::create_dir_all(target)
        .map_err(|error| format!("Nao foi possivel criar {}: {error}", target.display()))?;

    let entries = fs::read_dir(source)
        .map_err(|error| format!("Nao foi possivel ler {}: {error}", source.display()))?;

    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());

        if source_path.is_dir() {
            copy_dir_recursive(&source_path, &target_path)?;
        } else {
            fs::copy(&source_path, &target_path).map_err(|error| {
                format!(
                    "Nao foi possivel copiar {} para {}: {error}",
                    source_path.display(),
                    target_path.display()
                )
            })?;
        }
    }

    Ok(())
}

fn read_local_workshop_id(mod_dir: &Path) -> Option<String> {
    let workshop_id = fs::read_to_string(mod_dir.join(LOCAL_WORKSHOP_ID_FILE)).ok()?;
    let workshop_id = workshop_id.trim();

    if workshop_id.chars().all(|char| char.is_ascii_digit()) {
        Some(workshop_id.to_string())
    } else {
        None
    }
}

fn write_local_workshop_id(mod_dir: &Path, workshop_id: Option<&str>) -> Result<(), String> {
    let Some(workshop_id) = workshop_id.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };

    if !workshop_id.chars().all(|char| char.is_ascii_digit()) {
        return Ok(());
    }

    let marker_path = mod_dir.join(LOCAL_WORKSHOP_ID_FILE);
    fs::write(&marker_path, format!("{workshop_id}\n"))
        .map_err(|error| format!("Nao foi possivel salvar {}: {error}", marker_path.display()))
}

fn parse_mod_dependencies(content: &str) -> Vec<String> {
    let mut dependencies = Vec::new();
    let mut seen = HashSet::new();

    for value in read_ini_values(content, "require") {
        for dependency_id in split_mod_ids(&value) {
            if seen.insert(dependency_id.to_lowercase()) {
                dependencies.push(dependency_id);
            }
        }
    }

    dependencies
}

fn sanitize_folder_name(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|char| match char {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ => char,
        })
        .collect::<String>();

    if sanitized.trim().is_empty() {
        "mod".to_string()
    } else {
        sanitized
    }
}

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

fn steam_workshop_dirs() -> Vec<PathBuf> {
    let mut steamapps_dirs = Vec::new();
    let mut candidates = Vec::new();

    if let Some(program_files_x86) = env::var_os("ProgramFiles(x86)") {
        candidates.push(PathBuf::from(program_files_x86).join("Steam"));
    }

    if let Some(program_files) = env::var_os("ProgramFiles") {
        candidates.push(PathBuf::from(program_files).join("Steam"));
    }

    if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
        candidates.push(PathBuf::from(local_app_data).join("Steam"));
    }

    for steam_dir in candidates {
        let steamapps_dir = steam_dir.join("steamapps");

        if steamapps_dir.exists() {
            steamapps_dirs.push(steamapps_dir.clone());
            steamapps_dirs.extend(read_steam_library_dirs(
                &steamapps_dir.join("libraryfolders.vdf"),
            ));
        }
    }

    if let Ok(Some(steamcmd_path)) = find_steamcmd_path() {
        if let Some(steamcmd_dir) = steamcmd_path.parent() {
            let steamapps_dir = steamcmd_dir.join("steamapps");

            if steamapps_dir.exists() {
                steamapps_dirs.push(steamapps_dir);
            }
        }
    }

    let mut seen = HashSet::new();

    steamapps_dirs
        .into_iter()
        .filter_map(|steamapps_dir| {
            let workshop_dir = steamapps_dir
                .join("workshop")
                .join("content")
                .join("108600");
            let key = workshop_dir.display().to_string().to_lowercase();

            if seen.insert(key) {
                Some(workshop_dir)
            } else {
                None
            }
        })
        .collect()
}

fn find_mod_info_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    collect_mod_info_files(root, &mut files)?;
    Ok(files)
}

fn collect_mod_info_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(root)
        .map_err(|error| format!("Nao foi possivel ler {}: {error}", root.display()))?;

    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();

        if path.is_dir() {
            collect_mod_info_files(&path, files)?;
        } else if path.file_name().and_then(|name| name.to_str()) == Some("mod.info") {
            files.push(path);
        }
    }

    Ok(())
}

fn find_mod_image_url(content: &str, mod_dir: &Path) -> Option<String> {
    let candidates = read_ini_values(content, "poster")
        .into_iter()
        .chain(read_ini_values(content, "icon"))
        .filter(|value| !value.trim().is_empty());

    for candidate in candidates {
        let image_path = mod_dir.join(candidate);

        if image_path.exists() && image_path.is_file() {
            if let Some(data_url) = image_file_to_data_url(&image_path) {
                return Some(data_url);
            }
        }
    }

    None
}

fn image_file_to_data_url(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    let mime_type = match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        _ => "image/png",
    };
    let encoded = general_purpose::STANDARD.encode(bytes);

    Some(format!("data:{mime_type};base64,{encoded}"))
}
