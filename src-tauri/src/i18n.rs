use tauri::{
    menu::{MenuBuilder, MenuItem, SubmenuBuilder},
    Emitter,
};

use crate::settings::{read_language_preference, save_language_preference};

pub(crate) const LANGUAGE_AUTO: &str = "auto";
pub(crate) const LANGUAGE_EN: &str = "en";
pub(crate) const LANGUAGE_PT_BR: &str = "pt-BR";

pub(crate) fn effective_language() -> String {
    rust_i18n::locale().to_string()
}

fn set_effective_language(language: &str) -> Result<(), String> {
    match language {
        LANGUAGE_EN | LANGUAGE_PT_BR => {
            rust_i18n::set_locale(language);
            Ok(())
        }
        _ => return Err("Invalid language. Use en or pt-BR.".to_string()),
    }
}

pub(crate) fn validate_language_preference(preference: &str) -> Result<&str, String> {
    match preference {
        LANGUAGE_AUTO | LANGUAGE_EN | LANGUAGE_PT_BR => Ok(preference),
        _ => Err("Invalid language preference. Use auto, en or pt-BR.".to_string()),
    }
}

pub(crate) fn text(en: &'static str, pt_br: &'static str) -> String {
    let translated = rust_i18n::t!(en).to_string();

    if effective_language() == LANGUAGE_PT_BR && translated == en {
        pt_br.to_string()
    } else {
        translated
    }
}

pub(crate) fn mod_location_label(kind: &str, custom_name: Option<&str>) -> String {
    match kind {
        "steam" => "Steam Workshop Project Zomboid".to_string(),
        "local" => text("Local Zomboid mods", "Mods locais do Zomboid"),
        "steamcmd" => text("SteamCMD downloads", "Downloads do SteamCMD"),
        "custom" => match custom_name.filter(|name| !name.trim().is_empty()) {
            Some(name) => format!("{}: {name}", text("Custom folder", "Pasta personalizada")),
            None => text("Custom folder", "Pasta personalizada"),
        },
        _ => kind.to_string(),
    }
}

pub(crate) fn refresh_native_menu(app: &tauri::AppHandle) -> Result<(), String> {
    let new_server = MenuItem::with_id(
        app,
        "new_server",
        text("New server", "Novo servidor"),
        true,
        Some("Ctrl+N"),
    )
    .map_err(|error| error.to_string())?;
    let file = SubmenuBuilder::new(app, text("File", "Arquivo"))
        .item(&new_server)
        .build()
        .map_err(|error| error.to_string())?;
    let navigate = SubmenuBuilder::new(app, text("Navigate", "Navegar"))
        .text("show_dashboard", text("Servers", "Servidores"))
        .text("show_mods", "Mods")
        .text("show_downloads", text("Downloads", "Downloads"))
        .text("show_settings", text("Settings", "Configuracoes"))
        .build()
        .map_err(|error| error.to_string())?;
    let menu = MenuBuilder::new(app)
        .item(&file)
        .item(&navigate)
        .build()
        .map_err(|error| error.to_string())?;

    app.set_menu(menu).map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub(crate) fn get_language_preference() -> Result<String, String> {
    read_language_preference()
}

#[tauri::command]
pub(crate) fn sync_effective_language(
    app: tauri::AppHandle,
    effective_language: String,
) -> Result<(), String> {
    set_effective_language(&effective_language)?;
    refresh_native_menu(&app)
}

#[tauri::command]
pub(crate) fn set_language_preference(
    app: tauri::AppHandle,
    preference: String,
    effective_language: String,
) -> Result<(), String> {
    validate_language_preference(&preference)?;
    set_effective_language(&effective_language)?;
    save_language_preference(&preference)?;
    refresh_native_menu(&app)
}

pub(crate) fn emit_native_menu(app: &tauri::AppHandle, menu_id: &str) {
    let _ = app.emit("native-menu", menu_id);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_LANGUAGE_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn validates_supported_preferences() {
        assert_eq!(validate_language_preference("auto"), Ok("auto"));
        assert_eq!(validate_language_preference("en"), Ok("en"));
        assert_eq!(validate_language_preference("pt-BR"), Ok("pt-BR"));
        assert!(validate_language_preference("pt-PT").is_err());
    }

    #[test]
    fn translates_generated_mod_location_labels() {
        let _guard = TEST_LANGUAGE_LOCK.lock().unwrap();
        set_effective_language("en").unwrap();
        assert_eq!(mod_location_label("local", None), "Local Zomboid mods");
        assert_eq!(
            mod_location_label("custom", Some("extra")),
            "Custom folder: extra"
        );

        set_effective_language("pt-BR").unwrap();
        assert_eq!(mod_location_label("local", None), "Mods locais do Zomboid");
        assert_eq!(
            mod_location_label("custom", Some("extra")),
            "Pasta personalizada: extra"
        );
    }

    #[test]
    fn prefers_rust_i18n_catalog_over_legacy_fallback() {
        let _guard = TEST_LANGUAGE_LOCK.lock().unwrap();
        set_effective_language("pt-BR").unwrap();
        assert_eq!(text("New server", "fallback"), "Novo servidor");
        assert_eq!(effective_language(), "pt-BR");
    }
}
