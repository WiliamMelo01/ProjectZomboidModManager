use crate::models::ZomboidInstallationStatus;
use crate::run_blocking;

mod installation;
mod paths;
mod performance;
mod system;

pub(crate) use installation::steam_zomboid_game_dirs;
use installation::{open_steam_zomboid_folder_impl, scan_zomboid_installation_impl};
pub(crate) use performance::{
    apply_performance_settings, normalize_ram_gb, validate_game_executable_path,
};
use system::{get_system_ram_impl, select_game_executable_impl};

#[tauri::command]
pub(crate) async fn select_game_executable() -> Result<Option<String>, String> {
    run_blocking(select_game_executable_impl).await
}

#[tauri::command]
pub(crate) async fn get_system_ram() -> Result<u32, String> {
    run_blocking(get_system_ram_impl).await
}

#[tauri::command]
pub(crate) async fn scan_zomboid_installation(
    game_executable_path: Option<String>,
) -> Result<ZomboidInstallationStatus, String> {
    run_blocking(move || scan_zomboid_installation_impl(game_executable_path.as_deref())).await
}

#[tauri::command]
pub(crate) async fn open_steam_zomboid_folder() -> Result<String, String> {
    run_blocking(open_steam_zomboid_folder_impl).await
}
