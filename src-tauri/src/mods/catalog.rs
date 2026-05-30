use super::discovery::{find_mod_info_files, read_local_workshop_id, steam_workshop_dirs};
use super::metadata::{add_mod_from_info, add_mod_id_from_info};
use crate::models::ZomboidMod;
use crate::{saved_custom_mod_dirs, zomboid_mods_dir};
use std::{collections::HashSet, fs, path::Path};

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

pub(super) fn count_zomboid_mods_impl() -> Result<usize, String> {
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
