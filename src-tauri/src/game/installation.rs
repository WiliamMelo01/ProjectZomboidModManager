use super::paths::dedupe_paths;
use super::performance::{client_config_candidates, server_config_candidates};
use crate::models::ZomboidInstallationStatus;
use crate::read_steam_library_dirs;
use crate::workshop::open_path_external;
use std::{
    env,
    path::{Path, PathBuf},
};

pub(super) fn open_steam_zomboid_folder_impl() -> Result<String, String> {
    let Some(zomboid_dir) = steam_zomboid_game_dirs()
        .into_iter()
        .find(|path| path.exists())
    else {
        return Err(
            "Nao encontrei a pasta padrao do Project Zomboid na Steam. Verifique se o jogo esta instalado pela Steam."
                .to_string(),
        );
    };

    open_path_external(&zomboid_dir)?;

    Ok(zomboid_dir.display().to_string())
}

pub(super) fn scan_zomboid_installation_impl(
    game_executable_path: Option<&str>,
) -> Result<ZomboidInstallationStatus, String> {
    let default_game_dir = steam_zomboid_game_dirs()
        .into_iter()
        .find(|path| path.exists())
        .unwrap_or_else(default_steam_zomboid_game_dir);
    let configured_executable = game_executable_path
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from);
    let detected_executable = configured_executable
        .as_ref()
        .filter(|path| path.exists() && path.is_file())
        .cloned()
        .or_else(|| find_zomboid_executable_in_dir(&default_game_dir));
    let config_dir = detected_executable
        .as_deref()
        .and_then(Path::parent)
        .unwrap_or(default_game_dir.as_path());
    let client_configs = detected_executable
        .as_ref()
        .map(|path| client_config_candidates(path))
        .unwrap_or_else(|| client_config_candidates(&config_dir.join("ProjectZomboid64.exe")));
    let server_configs = server_config_candidates(config_dir);

    Ok(ZomboidInstallationStatus {
        default_game_dir: default_game_dir.display().to_string(),
        detected_executable_path: detected_executable
            .as_ref()
            .map(|path| path.display().to_string()),
        is_game_dir_found: default_game_dir.exists() && default_game_dir.is_dir(),
        is_executable_found: detected_executable.is_some(),
        is_client_config_found: client_configs
            .iter()
            .any(|path| path.exists() && path.is_file()),
        is_server_config_found: server_configs
            .iter()
            .any(|path| path.exists() && path.is_file()),
    })
}

pub(crate) fn steam_zomboid_game_dirs() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
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

        dedupe_paths(
            steamapps_dirs
                .into_iter()
                .map(|steamapps_dir| steamapps_dir.join("common").join("ProjectZomboid"))
                .collect(),
        )
    }

    #[cfg(not(windows))]
    {
        let mut steamapps_dirs = Vec::new();
        let mut candidates = Vec::new();

        if let Some(home) = env::var_os("HOME") {
            let home = PathBuf::from(home);
            candidates.push(home.join(".steam").join("steam"));
            candidates.push(home.join(".local").join("share").join("Steam"));
            candidates.push(
                home.join("snap")
                    .join("steam")
                    .join("common")
                    .join(".steam")
                    .join("steam"),
            );
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

        dedupe_paths(
            steamapps_dirs
                .into_iter()
                .map(|steamapps_dir| steamapps_dir.join("common").join("ProjectZomboid"))
                .collect(),
        )
    }
}

fn default_steam_zomboid_game_dir() -> PathBuf {
    #[cfg(windows)]
    {
        if let Some(program_files_x86) = env::var_os("ProgramFiles(x86)") {
            return PathBuf::from(program_files_x86)
                .join("Steam")
                .join("steamapps")
                .join("common")
                .join("ProjectZomboid");
        }

        PathBuf::from(r"C:\Program Files (x86)")
            .join("Steam")
            .join("steamapps")
            .join("common")
            .join("ProjectZomboid")
    }

    #[cfg(not(windows))]
    {
        if let Some(home) = env::var_os("HOME") {
            return PathBuf::from(home)
                .join(".steam")
                .join("steam")
                .join("steamapps")
                .join("common")
                .join("ProjectZomboid");
        }

        PathBuf::from("/usr/share/steam")
            .join("steamapps")
            .join("common")
            .join("ProjectZomboid")
    }
}

fn find_zomboid_executable_in_dir(game_dir: &Path) -> Option<PathBuf> {
    #[cfg(windows)]
    let file_names = [
        "ProjectZomboid64.exe",
        "ProjectZomboid32.exe",
        "ProjectZomboid.exe",
    ];

    #[cfg(not(windows))]
    let file_names = ["ProjectZomboid64", "ProjectZomboid32", "ProjectZomboid"];

    for file_name in file_names {
        let candidate = game_dir.join(file_name);

        if candidate.exists() && candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}
