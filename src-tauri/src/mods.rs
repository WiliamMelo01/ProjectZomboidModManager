use crate::models::ZomboidMod;
use crate::models::ZomboidModInstallResult;
use crate::run_blocking;
use crate::util::{directory_size, format_size};
use std::path::PathBuf;

mod cache;
mod catalog;
mod discovery;
mod install;
mod metadata;
mod server_values;

use catalog::count_zomboid_mods_impl;
pub(crate) use catalog::list_zomboid_mods_impl;
pub(crate) use discovery::steam_workshop_dirs;
pub(crate) use server_values::{
    normalize_server_values, parse_server_mod_ids, resolve_server_workshop_ids,
    serialize_server_mod_ids,
};

#[tauri::command]
pub(crate) async fn list_zomboid_mods() -> Result<Vec<ZomboidMod>, String> {
    run_blocking(list_zomboid_mods_impl).await
}

#[tauri::command]
pub(crate) async fn count_zomboid_mods() -> Result<usize, String> {
    run_blocking(count_zomboid_mods_impl).await
}

#[tauri::command]
pub(crate) async fn clear_zomboid_mods_cache() -> Result<(), String> {
    run_blocking(clear_zomboid_mods_cache_impl).await
}

pub(crate) fn clear_zomboid_mods_cache_impl() -> Result<(), String> {
    cache::clear_persisted_cache()
}

#[tauri::command]
pub(crate) async fn get_zomboid_mod_package_size(package_path: String) -> Result<String, String> {
    run_blocking(move || {
        let path = PathBuf::from(package_path);

        if !path.is_dir() {
            return Ok("-".to_string());
        }

        Ok(format_size(directory_size(&path)))
    })
    .await
}

#[tauri::command]
pub(crate) fn install_zomboid_mod(
    package_path: String,
    mod_id: String,
    workshop_id: String,
) -> Result<ZomboidModInstallResult, String> {
    install::install_zomboid_mod_impl(package_path, mod_id, workshop_id)
}
