use super::cache::{package_signature, ModsLibraryCache};
use super::discovery::{read_local_workshop_id, steam_workshop_dirs, steamcmd_workshop_dirs};
use super::metadata::{read_mod_package, variant_ids};
use crate::models::ZomboidMod;
use crate::{saved_custom_mod_dirs, zomboid_mods_dir};
use std::{collections::HashSet, fs, path::Path};

pub(crate) fn list_zomboid_mods_impl() -> Result<Vec<ZomboidMod>, String> {
    let mut mods = Vec::new();
    let mut installed_ids = HashSet::new();
    let mut cache = ModsLibraryCache::load();

    if let Ok(local_dir) = zomboid_mods_dir() {
        collect_flat_packages(
            &local_dir,
            "local",
            true,
            &mut mods,
            &mut installed_ids,
            &mut cache,
        )?;
    }
    let steamcmd_workshop_dir_keys = steamcmd_workshop_dirs()
        .into_iter()
        .map(|path| path_key(&path))
        .collect::<HashSet<_>>();
    for workshop_dir in steam_workshop_dirs() {
        let source = if steamcmd_workshop_dir_keys.contains(&path_key(&workshop_dir)) {
            "steamcmd"
        } else {
            "steam"
        };
        collect_workshop_items(
            &workshop_dir,
            source,
            &mut mods,
            &mut installed_ids,
            &mut cache,
        )?;
    }
    for custom_dir in saved_custom_mod_dirs()? {
        collect_custom_dir(&custom_dir, &mut mods, &mut installed_ids, &mut cache)?;
    }

    mods.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    cache.retain_active_and_save();
    Ok(mods)
}

pub(super) fn count_zomboid_mods_impl() -> Result<usize, String> {
    Ok(list_zomboid_mods_impl()?.len())
}

fn collect_flat_packages(
    root: &Path,
    source: &str,
    is_local: bool,
    mods: &mut Vec<ZomboidMod>,
    installed_ids: &mut HashSet<String>,
    cache: &mut ModsLibraryCache,
) -> Result<(), String> {
    if !root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(root)
        .map_err(|error| format!("Nao foi possivel ler {}: {error}", root.display()))?
    {
        let path = entry.map_err(|error| error.to_string())?.path();
        if !path.is_dir() {
            continue;
        }
        let workshop_id = if is_local {
            read_local_workshop_id(&path)
        } else {
            None
        };
        push_package(
            &path,
            workshop_id.as_deref(),
            source,
            mods,
            installed_ids,
            cache,
        )?;
    }
    Ok(())
}

fn collect_workshop_items(
    root: &Path,
    source: &str,
    mods: &mut Vec<ZomboidMod>,
    installed_ids: &mut HashSet<String>,
    cache: &mut ModsLibraryCache,
) -> Result<(), String> {
    if !root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(root)
        .map_err(|error| format!("Nao foi possivel ler {}: {error}", root.display()))?
    {
        let item = entry.map_err(|error| error.to_string())?.path();
        let workshop_id = item
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if !item.is_dir() || !workshop_id.chars().all(|char| char.is_ascii_digit()) {
            continue;
        }
        let mods_dir = item.join("mods");
        if mods_dir.is_dir() {
            for package in fs::read_dir(&mods_dir)
                .map_err(|error| format!("Nao foi possivel ler {}: {error}", mods_dir.display()))?
            {
                let package = package.map_err(|error| error.to_string())?.path();
                if package.is_dir() {
                    push_package(
                        &package,
                        Some(workshop_id),
                        source,
                        mods,
                        installed_ids,
                        cache,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn collect_custom_dir(
    root: &Path,
    mods: &mut Vec<ZomboidMod>,
    installed_ids: &mut HashSet<String>,
    cache: &mut ModsLibraryCache,
) -> Result<(), String> {
    if !root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(root)
        .map_err(|error| format!("Nao foi possivel ler {}: {error}", root.display()))?
    {
        let path = entry.map_err(|error| error.to_string())?.path();
        if !path.is_dir() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let mods_dir = path.join("mods");
        if name.chars().all(|char| char.is_ascii_digit()) && mods_dir.is_dir() {
            for package in fs::read_dir(&mods_dir)
                .map_err(|error| format!("Nao foi possivel ler {}: {error}", mods_dir.display()))?
            {
                let package = package.map_err(|error| error.to_string())?.path();
                if package.is_dir() {
                    push_package(&package, Some(name), "custom", mods, installed_ids, cache)?;
                }
            }
        } else {
            push_package(&path, None, "custom", mods, installed_ids, cache)?;
        }
    }
    Ok(())
}

fn push_package(
    package: &Path,
    workshop_id: Option<&str>,
    source: &str,
    mods: &mut Vec<ZomboidMod>,
    installed_ids: &mut HashSet<String>,
    cache: &mut ModsLibraryCache,
) -> Result<(), String> {
    let key = ModsLibraryCache::key(package, source, workshop_id);
    let signature = package_signature(package);
    let mod_item = if let Some(mod_item) = cache.get_valid(&key, &signature) {
        mod_item
    } else {
        let Some(mod_item) = read_mod_package(package, workshop_id, source)? else {
            return Ok(());
        };
        cache.store(key, signature, mod_item.clone());
        mod_item
    };
    let ids = variant_ids(&mod_item);
    if ids.iter().any(|id| installed_ids.contains(id)) {
        return Ok(());
    }
    installed_ids.extend(ids);
    mods.push(mod_item);
    Ok(())
}

fn path_key(path: &Path) -> String {
    path.display().to_string().to_lowercase()
}
