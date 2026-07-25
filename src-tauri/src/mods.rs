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
pub(crate) use discovery::steamcmd_workshop_dirs;
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

#[tauri::command]
pub(crate) async fn get_workshop_mappings(
) -> Result<std::collections::HashMap<String, String>, String> {
    let config_dir = crate::app_config_dir()?;
    let db_path = config_dir.join("workshop-mods-db.json");
    if !db_path.exists() {
        return Ok(std::collections::HashMap::new());
    }
    let content = std::fs::read_to_string(&db_path)
        .map_err(|e| format!("Falha ao ler banco de dados: {}", e))?;
    let mappings: std::collections::HashMap<String, String> =
        serde_json::from_str(&content).unwrap_or_else(|_| std::collections::HashMap::new());
    Ok(mappings)
}

#[tauri::command]
pub(crate) async fn save_workshop_mapping(
    mod_id: String,
    workshop_id: String,
) -> Result<(), String> {
    let config_dir = crate::app_config_dir()?;
    let db_path = config_dir.join("workshop-mods-db.json");
    let mut mappings = if db_path.exists() {
        let content = std::fs::read_to_string(&db_path)
            .map_err(|e| format!("Falha ao ler banco de dados: {}", e))?;
        serde_json::from_str::<std::collections::HashMap<String, String>>(&content)
            .unwrap_or_else(|_| std::collections::HashMap::new())
    } else {
        std::collections::HashMap::new()
    };
    mappings.insert(mod_id, workshop_id);
    let serialized = serde_json::to_string_pretty(&mappings)
        .map_err(|e| format!("Falha ao serializar dados: {}", e))?;
    std::fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Falha ao criar pasta de configuracoes: {}", e))?;
    std::fs::write(&db_path, serialized)
        .map_err(|e| format!("Falha ao salvar banco de dados: {}", e))?;
    Ok(())
}

#[tauri::command]
pub(crate) async fn save_workshop_mappings(
    mappings: std::collections::HashMap<String, String>,
) -> Result<(), String> {
    let config_dir = crate::app_config_dir()?;
    let db_path = config_dir.join("workshop-mods-db.json");

    let serialized = serde_json::to_string_pretty(&mappings)
        .map_err(|e| format!("Falha ao serializar dados: {}", e))?;

    std::fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Falha ao criar pasta de configuracoes: {}", e))?;
    std::fs::write(&db_path, serialized)
        .map_err(|e| format!("Falha ao salvar banco de dados: {}", e))?;
    Ok(())
}

#[tauri::command]
pub(crate) async fn delete_workshop_mapping(mod_id: String) -> Result<(), String> {
    let config_dir = crate::app_config_dir()?;
    let db_path = config_dir.join("workshop-mods-db.json");
    if !db_path.exists() {
        return Ok(());
    }
    let content = std::fs::read_to_string(&db_path)
        .map_err(|e| format!("Falha ao ler banco de dados: {}", e))?;
    let mut mappings: std::collections::HashMap<String, String> =
        serde_json::from_str(&content).unwrap_or_else(|_| std::collections::HashMap::new());
    mappings.remove(&mod_id);
    let serialized = serde_json::to_string_pretty(&mappings)
        .map_err(|e| format!("Falha ao salvar banco de dados: {}", e))?;
    std::fs::write(&db_path, serialized)
        .map_err(|e| format!("Falha ao salvar banco de dados: {}", e))?;
    Ok(())
}
