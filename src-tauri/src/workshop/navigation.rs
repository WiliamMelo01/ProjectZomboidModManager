use std::{
    path::{Path, PathBuf},
    process::Command,
};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

pub(super) fn open_steam_workshop_impl(
    app: &tauri::AppHandle,
    item_id_or_search: &str,
) -> Result<(), String> {
    let value = item_id_or_search.trim();

    if value.is_empty() {
        return Err("Informe o ID ou nome da dependencia para abrir a Steam Workshop.".to_string());
    }

    let url = build_steam_workshop_url(value);
    let app_url = PathBuf::from(format!(
        "index.html#/workshop?target={}&url={}",
        encode_url_query(value),
        encode_url_query(&url)
    ));

    if let Some(window) = app.get_webview_window("steam-workshop") {
        window.close().map_err(|error| {
            format!("Nao foi possivel atualizar a janela da Steam Workshop: {error}")
        })?;
    }

    WebviewWindowBuilder::new(app, "steam-workshop", WebviewUrl::App(app_url))
        .title("Steam Workshop")
        .inner_size(760.0, 560.0)
        .resizable(true)
        .build()
        .map_err(|error| format!("Nao foi possivel abrir a Steam Workshop no app: {error}"))?;

    Ok(())
}

pub(super) fn open_steam_workshop_external_impl(item_id_or_search: &str) -> Result<(), String> {
    let value = item_id_or_search.trim();

    if value.is_empty() {
        return Err("Informe o ID ou nome da dependencia para abrir a Steam Workshop.".to_string());
    }

    open_url_external(&build_steam_workshop_url(value))
}

pub(super) fn open_steam_workshop_steam_client_impl(item_id_or_search: &str) -> Result<(), String> {
    let value = item_id_or_search.trim();

    if value.is_empty() {
        return Err("Informe o ID ou nome da dependencia para abrir a Steam Workshop.".to_string());
    }

    open_url_external(&format!(
        "steam://openurl/{}",
        build_steam_workshop_url(value)
    ))
}

fn open_url_external(url: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        Command::new("rundll32.exe")
            .args(["url.dll,FileProtocolHandler", url])
            .spawn()
            .map_err(|error| format!("Nao foi possivel abrir o navegador: {error}"))?;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|error| format!("Nao foi possivel abrir o navegador: {error}"))?;
        Ok(())
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|error| format!("Nao foi possivel abrir o navegador: {error}"))?;
        Ok(())
    }
}

pub(crate) fn open_path_external(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        Command::new("explorer.exe")
            .arg(path)
            .spawn()
            .map_err(|error| format!("Nao foi possivel abrir o Explorer: {error}"))?;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|error| format!("Nao foi possivel abrir a pasta: {error}"))?;
        Ok(())
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|error| format!("Nao foi possivel abrir a pasta: {error}"))?;
        Ok(())
    }
}

pub(crate) fn open_file_external(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        Command::new("rundll32.exe")
            .arg("url.dll,FileProtocolHandler")
            .arg(path)
            .spawn()
            .map_err(|error| format!("Nao foi possivel abrir o arquivo: {error}"))?;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|error| format!("Nao foi possivel abrir o arquivo: {error}"))?;
        Ok(())
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|error| format!("Nao foi possivel abrir o arquivo: {error}"))?;
        Ok(())
    }
}

fn build_steam_workshop_url(value: &str) -> String {
    if value.chars().all(|char| char.is_ascii_digit()) {
        format!("https://steamcommunity.com/sharedfiles/filedetails/?id={value}")
    } else {
        format!(
            "https://steamcommunity.com/workshop/browse/?appid=108600&searchtext={}",
            encode_url_query(value)
        )
    }
}

fn encode_url_query(value: &str) -> String {
    let mut encoded = String::new();

    for byte in value.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(*byte as char)
            }
            b' ' => encoded.push('+'),
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }

    encoded
}
