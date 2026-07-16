use crate::{managed_steamcmd_pool_workshop_dirs, read_steam_library_dirs};
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

pub(crate) fn steam_workshop_dirs() -> Vec<PathBuf> {
    #[cfg(not(windows))]
    {
        if let Some(extra_val) = env::var_os("PZMM_EXTRA_STEAM_WORKSHOP_DIRS") {
            let mut dirs = Vec::new();
            for line in extra_val.to_string_lossy().lines() {
                let path = line.trim();
                if !path.is_empty() {
                    dirs.push(PathBuf::from(path));
                }
            }
            return dedupe_paths(dirs);
        }
    }

    let mut steamapps_dirs = Vec::new();
    let mut candidates = Vec::new();

    #[cfg(windows)]
    {
        if let Some(program_files_x86) = env::var_os("ProgramFiles(x86)") {
            candidates.push(PathBuf::from(program_files_x86).join("Steam"));
        }

        if let Some(program_files) = env::var_os("ProgramFiles") {
            candidates.push(PathBuf::from(program_files).join("Steam"));
        }

        if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
            candidates.push(PathBuf::from(local_app_data).join("Steam"));
        }
    }

    #[cfg(not(windows))]
    {
        if let Some(home) = env::var_os("HOME").map(PathBuf::from) {
            candidates.extend([
                home.join("Steam"),
                home.join(".local").join("share").join("Steam"),
                home.join(".steam").join("steam"),
                home.join(".steam").join("root"),
                home.join("snap")
                    .join("steam")
                    .join("common")
                    .join(".steam")
                    .join("steam"),
            ]);
        }
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

    let mut workshop_dirs = steamapps_dirs
        .into_iter()
        .map(|steamapps_dir| {
            steamapps_dir
                .join("workshop")
                .join("content")
                .join("108600")
        })
        .collect::<Vec<_>>();

    #[cfg(not(windows))]
    {
        workshop_dirs.extend(extra_steam_workshop_dirs_from_env());
    }

    #[cfg(windows)]
    {
        workshop_dirs.extend(steamcmd_workshop_dirs());
        dedupe_paths(workshop_dirs)
    }

    #[cfg(not(windows))]
    {
        dedupe_paths(workshop_dirs)
    }
}

pub(crate) fn steamcmd_workshop_dirs() -> Vec<PathBuf> {
    let mut dirs = dedupe_paths(managed_steamcmd_pool_workshop_dirs());
    dirs.truncate(1);
    dirs
}

#[cfg(not(windows))]
fn extra_steam_workshop_dirs_from_env() -> Vec<PathBuf> {
    env::var_os("PZMM_EXTRA_STEAM_WORKSHOP_DIRS")
        .map(|value| {
            value
                .to_string_lossy()
                .lines()
                .map(str::trim)
                .filter(|path| !path.is_empty())
                .map(PathBuf::from)
                .collect()
        })
        .unwrap_or_default()
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();

    paths
        .into_iter()
        .filter_map(|path| {
            let resolved_path = fs::canonicalize(&path).unwrap_or(path);
            let key = resolved_path.display().to_string().to_lowercase();

            seen.insert(key).then_some(resolved_path)
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
