use crate::models::WorkshopDownloadResult;
use crate::run_blocking;

mod api;
mod download;
mod navigation;

use api::{fetch_steam_workshop_collection_items, validate_workshop_id};
use download::{cancel_steam_workshop_download_impl, download_steam_workshop_items_impl};
pub(crate) use navigation::{open_file_external, open_path_external};
use navigation::{
    open_steam_workshop_external_impl, open_steam_workshop_impl,
    open_steam_workshop_steam_client_impl,
};

#[tauri::command]
pub(crate) async fn download_steam_workshop_item(
    app: tauri::AppHandle,
    workshop_id: String,
    force_validate: Option<bool>,
) -> Result<WorkshopDownloadResult, String> {
    run_blocking(move || {
        let workshop_id = validate_workshop_id(&workshop_id, "item")?;
        download_steam_workshop_items_impl(&app, vec![workshop_id], force_validate.unwrap_or(false))
    })
    .await
}

#[tauri::command]
pub(crate) async fn download_steam_workshop_collection(
    app: tauri::AppHandle,
    collection_id: String,
    force_validate: Option<bool>,
) -> Result<WorkshopDownloadResult, String> {
    run_blocking(move || {
        let workshop_ids = fetch_steam_workshop_collection_items(&collection_id)?;
        download_steam_workshop_items_impl(&app, workshop_ids, force_validate.unwrap_or(false))
    })
    .await
}

#[tauri::command]
pub(crate) async fn download_steam_workshop_items(
    app: tauri::AppHandle,
    workshop_ids: Vec<String>,
    force_validate: Option<bool>,
) -> Result<WorkshopDownloadResult, String> {
    run_blocking(move || {
        download_steam_workshop_items_impl(&app, workshop_ids, force_validate.unwrap_or(false))
    })
    .await
}

#[tauri::command]
pub(crate) async fn cancel_steam_workshop_download() -> Result<(), String> {
    run_blocking(cancel_steam_workshop_download_impl).await
}

#[tauri::command]
pub(crate) fn open_steam_workshop(
    app: tauri::AppHandle,
    item_id_or_search: String,
) -> Result<(), String> {
    open_steam_workshop_impl(&app, &item_id_or_search)
}

#[tauri::command]
pub(crate) fn open_steam_workshop_external(item_id_or_search: String) -> Result<(), String> {
    open_steam_workshop_external_impl(&item_id_or_search)
}

#[tauri::command]
pub(crate) fn open_steam_workshop_steam_client(item_id_or_search: String) -> Result<(), String> {
    open_steam_workshop_steam_client_impl(&item_id_or_search)
}
