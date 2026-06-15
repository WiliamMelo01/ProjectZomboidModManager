use crate::models::ZomboidMod;
use crate::run_blocking;

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
pub(crate) fn install_zomboid_mod(
    package_path: String,
    mod_id: String,
    workshop_id: String,
) -> Result<(), String> {
    install::install_zomboid_mod_impl(package_path, mod_id, workshop_id)
}
