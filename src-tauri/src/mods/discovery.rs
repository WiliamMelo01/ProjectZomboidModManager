use crate::{find_steamcmd_path, read_steam_library_dirs};
use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
};

const LOCAL_WORKSHOP_ID_FILE: &str = ".pzmm-workshop-id";

pub(super) fn read_local_workshop_id(mod_dir: &Path) -> Option<String> {
    let workshop_id = fs::read_to_string(mod_dir.join(LOCAL_WORKSHOP_ID_FILE)).ok()?;
    let workshop_id = workshop_id.trim();

    if workshop_id.chars().all(|char| char.is_ascii_digit()) {
        Some(workshop_id.to_string())
    } else {
        None
    }
}

pub(super) fn write_local_workshop_id(
    mod_dir: &Path,
    workshop_id: Option<&str>,
) -> Result<(), String> {
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

pub(super) fn steam_workshop_dirs() -> Vec<PathBuf> {
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

pub(super) fn find_mod_info_files(root: &Path) -> Result<Vec<PathBuf>, String> {
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
