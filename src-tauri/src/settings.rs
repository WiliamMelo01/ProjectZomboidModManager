#[cfg(not(windows))]
use crate::app_config_dir;
use crate::game::{
    apply_performance_settings, normalize_ram_gb, resolve_zomboid_executable_path,
    validate_game_executable_path,
};
use crate::i18n::{mod_location_label, text, validate_language_preference, LANGUAGE_AUTO};
use crate::models::{AppSettings, ModLocation};
#[cfg(windows)]
use crate::util::hide_command_window;
use crate::workshop::open_path_external;
use crate::{
    app_settings_path, ensure_managed_steamcmd_pool, read_config_value,
    read_saved_custom_mod_locations, run_blocking, zomboid_mods_dir,
};
#[cfg(windows)]
use crate::{
    managed_steamcmd_pool_instance_path, managed_steamcmd_pool_workshop_dirs,
    read_saved_mod_locations,
};
#[cfg(windows)]
use std::process::Command;
#[cfg(not(windows))]
use std::process::Command;
use std::{collections::HashSet, env, fs, path::PathBuf};

pub(crate) const DEFAULT_MAX_CONCURRENT_DOWNLOADS: u32 = 1;
pub(crate) const MAX_CONCURRENT_DOWNLOADS_LIMIT: u32 = 1;

#[cfg(not(windows))]
const LINUX_STEAMCMD_DOWNLOAD_URL: &str =
    "https://steamcdn-a.akamaihd.net/client/installer/steamcmd_linux.tar.gz";

#[tauri::command]
pub(crate) async fn get_app_settings(app: tauri::AppHandle) -> Result<AppSettings, String> {
    run_blocking(move || {
        let max_concurrent_downloads = read_max_concurrent_downloads()?;
        let _ = ensure_managed_steamcmd_pool(&app, max_concurrent_downloads as usize);
        load_app_settings()
    })
    .await
}

#[tauri::command]
pub(crate) async fn get_mod_locations() -> Result<Vec<ModLocation>, String> {
    run_blocking(get_mod_locations_impl).await
}

#[tauri::command]
pub(crate) async fn save_app_settings(
    steamcmd_path: String,
    game_executable_path: String,
    client_ram: String,
    server_ram: String,
    max_concurrent_downloads: Option<u32>,
) -> Result<AppSettings, String> {
    run_blocking(move || {
        save_app_settings_impl(
            &steamcmd_path,
            &game_executable_path,
            &client_ram,
            &server_ram,
            max_concurrent_downloads.unwrap_or(DEFAULT_MAX_CONCURRENT_DOWNLOADS),
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn install_linux_steamcmd() -> Result<AppSettings, String> {
    run_blocking(install_linux_steamcmd_impl).await
}

#[tauri::command]
pub(crate) async fn select_mod_folder() -> Result<Option<String>, String> {
    run_blocking(select_mod_folder_impl).await
}

#[tauri::command]
pub(crate) async fn add_mod_location(path: String) -> Result<Vec<ModLocation>, String> {
    run_blocking(move || add_mod_location_impl(&path)).await
}

#[tauri::command]
pub(crate) async fn open_mod_location(path: String) -> Result<(), String> {
    run_blocking(move || open_mod_location_impl(&path)).await
}

#[tauri::command]
pub(crate) async fn is_delete_all_enabled() -> Result<bool, String> {
    run_blocking(|| Ok(delete_all_enabled())).await
}

#[cfg(windows)]
fn install_linux_steamcmd_impl() -> Result<AppSettings, String> {
    Err("A instalacao automatica do SteamCMD pelo app esta disponivel apenas no Linux.".to_string())
}

#[cfg(not(windows))]
fn install_linux_steamcmd_impl() -> Result<AppSettings, String> {
    let steamcmd_dir = app_config_dir()?;
    fs::create_dir_all(&steamcmd_dir).map_err(|error| {
        format!(
            "Nao foi possivel criar a pasta do SteamCMD em {}: {error}",
            steamcmd_dir.display()
        )
    })?;

    let archive_path = env::temp_dir().join("pzmm-steamcmd-linux.tar.gz");
    let curl_status = Command::new("curl")
        .args([
            "-fsSL",
            LINUX_STEAMCMD_DOWNLOAD_URL,
            "-o",
            &archive_path.display().to_string(),
        ])
        .status()
        .map_err(|error| format!("Nao foi possivel baixar o SteamCMD com curl: {error}"))?;

    if !curl_status.success() {
        return Err(format!(
            "Download do SteamCMD falhou com status {curl_status}. Verifique sua conexao e tente novamente."
        ));
    }

    let tar_status = Command::new("tar")
        .args([
            "-xzf",
            &archive_path.display().to_string(),
            "-C",
            &steamcmd_dir.display().to_string(),
        ])
        .status()
        .map_err(|error| format!("Nao foi possivel extrair o SteamCMD com tar: {error}"))?;

    let _ = fs::remove_file(&archive_path);

    if !tar_status.success() {
        return Err(format!(
            "Extracao do SteamCMD falhou com status {tar_status}."
        ));
    }

    let steamcmd_path = linux_steamcmd_executable_path()?;
    if !steamcmd_path.is_file() {
        return Err(format!(
            "SteamCMD foi extraido, mas {} nao foi encontrado.",
            steamcmd_path.display()
        ));
    }

    let chmod_status = Command::new("chmod")
        .args(["0755", &steamcmd_path.display().to_string()])
        .status()
        .map_err(|error| format!("Nao foi possivel ajustar permissao do SteamCMD: {error}"))?;

    if !chmod_status.success() {
        return Err(format!(
            "Ajuste de permissao do SteamCMD falhou com status {chmod_status}."
        ));
    }

    load_app_settings()
}

fn load_app_settings() -> Result<AppSettings, String> {
    let configured_path = read_config_value("steamcmd_path")?.unwrap_or_default();
    let resolved_steamcmd_path = resolve_steamcmd_path();
    let is_steamcmd_configured = resolved_steamcmd_path.is_some();
    let saved_game_executable_path = read_config_value("game_executable_path")?.unwrap_or_default();
    let game_executable_path =
        resolve_game_executable_setting_value(&saved_game_executable_path).unwrap_or_default();
    let client_ram = read_config_value("client_ram")?.unwrap_or_else(|| "4.00".to_string());
    let server_ram = read_config_value("server_ram")?.unwrap_or_else(|| "4.00".to_string());
    let max_concurrent_downloads = read_max_concurrent_downloads()?;
    let language_preference = read_language_preference()?;

    Ok(AppSettings {
        steamcmd_path: configured_path,
        resolved_steamcmd_path,
        is_steamcmd_configured,
        game_executable_path,
        client_ram,
        server_ram,
        max_concurrent_downloads,
        language_preference,
    })
}

fn resolve_game_executable_setting_value(saved_path: &str) -> Option<String> {
    resolve_zomboid_executable_path(Some(saved_path)).map(|path| path.display().to_string())
}

fn get_mod_locations_impl() -> Result<Vec<ModLocation>, String> {
    let locations = build_default_mod_locations()?;
    #[cfg(windows)]
    let locations = {
        let mut locations = locations;
        let saved_locations = read_saved_mod_locations()?;
        merge_custom_mod_locations(
            &mut locations,
            saved_locations
                .into_iter()
                .filter(|location| location.kind == "custom")
                .collect(),
        );
        locations
    };
    let game_executable_path = read_config_value("game_executable_path")?.unwrap_or_default();
    let client_ram = read_config_value("client_ram")?.unwrap_or_else(|| "4.00".to_string());
    let server_ram = read_config_value("server_ram")?.unwrap_or_else(|| "4.00".to_string());
    let max_concurrent_downloads = read_max_concurrent_downloads()?;
    let language_preference = read_language_preference()?;
    write_app_settings_file(
        "",
        &game_executable_path,
        &client_ram,
        &server_ram,
        max_concurrent_downloads,
        &language_preference,
        &locations,
    )?;

    Ok(locations)
}

pub(crate) fn push_mod_location(
    locations: &mut Vec<ModLocation>,
    seen: &mut HashSet<String>,
    label: &str,
    kind: &str,
    path: PathBuf,
) {
    let key = path.display().to_string().to_lowercase();

    if !seen.insert(key) {
        return;
    }

    let exists = path.exists();

    locations.push(ModLocation {
        label: label.to_string(),
        path: path.display().to_string(),
        kind: kind.to_string(),
        exists,
    });
}

fn build_default_mod_locations() -> Result<Vec<ModLocation>, String> {
    let mut locations = Vec::new();
    let mut seen = HashSet::new();

    // 1. Zomboid default local mods directory
    let local_mods_dir = zomboid_mods_dir()?;
    let _ = fs::create_dir_all(&local_mods_dir);
    push_mod_location(
        &mut locations,
        &mut seen,
        &mod_location_label("local", None),
        "local",
        local_mods_dir,
    );

    // 2. Steam Client workshop directory
    push_mod_location(
        &mut locations,
        &mut seen,
        &mod_location_label("steam", None),
        "steam",
        default_steam_workshop_dir(),
    );

    // 3. SteamCMD workshop directory (on Windows, return first pool directory)
    #[cfg(windows)]
    {
        let steamcmd_dirs = default_steamcmd_workshop_dirs();
        if let Some(first_steamcmd_dir) = steamcmd_dirs.into_iter().next() {
            push_mod_location(
                &mut locations,
                &mut seen,
                &mod_location_label("steamcmd", None),
                "steamcmd",
                first_steamcmd_dir,
            );
        }
    }

    Ok(locations)
}

fn merge_custom_mod_locations(
    locations: &mut Vec<ModLocation>,
    custom_locations: Vec<ModLocation>,
) {
    let mut seen = locations
        .iter()
        .map(|location| location.path.to_lowercase())
        .collect::<HashSet<_>>();

    for location in custom_locations {
        if location.kind != "custom" {
            continue;
        }

        let key = location.path.to_lowercase();

        if seen.insert(key) {
            locations.push(location);
        }
    }
}

pub(crate) fn default_steam_workshop_dir() -> PathBuf {
    #[cfg(not(windows))]
    {
        linux_steam_roots()
            .into_iter()
            .find(|path| path.exists() && path.is_dir())
            .unwrap_or_else(|| {
                home_dir()
                    .unwrap_or_else(|| PathBuf::from("~"))
                    .join(".steam")
                    .join("steam")
            })
            .join("steamapps")
            .join("workshop")
            .join("content")
            .join("108600")
    }

    #[cfg(windows)]
    {
        if let Some(program_files_x86) = env::var_os("ProgramFiles(x86)") {
            return PathBuf::from(program_files_x86)
                .join("Steam")
                .join("steamapps")
                .join("workshop")
                .join("content")
                .join("108600");
        }

        if let Some(program_files) = env::var_os("ProgramFiles") {
            return PathBuf::from(program_files)
                .join("Steam")
                .join("steamapps")
                .join("workshop")
                .join("content")
                .join("108600");
        }

        PathBuf::from(r"C:\Program Files (x86)\Steam")
            .join("steamapps")
            .join("workshop")
            .join("content")
            .join("108600")
    }
}

fn resolve_steamcmd_path() -> Option<String> {
    #[cfg(windows)]
    {
        managed_steamcmd_pool_instance_path(1)
            .ok()
            .filter(|path| path.exists())
            .map(|path| path.display().to_string())
    }

    #[cfg(not(windows))]
    {
        let command_exists = find_command_path("steamcmd").is_some()
            || linux_steamcmd_executable_path()
                .ok()
                .filter(|path| path.is_file())
                .is_some();

        if command_exists {
            Some(default_steam_workshop_dir().display().to_string())
        } else {
            None
        }
    }
}

#[cfg(windows)]
fn default_steamcmd_workshop_dirs() -> Vec<PathBuf> {
    managed_steamcmd_pool_workshop_dirs()
        .into_iter()
        .take(1)
        .collect()
}

#[cfg(not(windows))]
fn linux_steamcmd_executable_path() -> Result<PathBuf, String> {
    Ok(app_config_dir()?.join("steamcmd.sh"))
}

#[cfg(not(windows))]
fn find_command_path(command_name: &str) -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;

    for path_dir in env::split_paths(&path_var) {
        let candidate = path_dir.join(command_name);

        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

#[cfg(not(windows))]
fn linux_steam_roots() -> Vec<PathBuf> {
    let Some(home) = home_dir() else {
        return Vec::new();
    };

    [
        home.join("Steam"),
        home.join(".local").join("share").join("Steam"),
        home.join(".steam").join("steam"),
        home.join(".steam").join("root"),
        home.join("snap")
            .join("steam")
            .join("common")
            .join(".steam")
            .join("steam"),
    ]
    .into_iter()
    .collect()
}

#[cfg(not(windows))]
fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

fn save_app_settings_impl(
    steamcmd_path: &str,
    game_executable_path: &str,
    client_ram: &str,
    server_ram: &str,
    max_concurrent_downloads: u32,
) -> Result<AppSettings, String> {
    let _steamcmd_path = steamcmd_path.trim();
    let game_executable_path = game_executable_path.trim();
    let client_ram = normalize_ram_gb(client_ram)?;
    let server_ram = normalize_ram_gb(server_ram)?;
    let max_concurrent_downloads = validate_max_concurrent_downloads(max_concurrent_downloads)?;

    if !game_executable_path.is_empty() {
        let game_executable = PathBuf::from(game_executable_path);

        validate_game_executable_path(&game_executable)?;
        apply_performance_settings(&game_executable, &client_ram, &server_ram)?;
    }

    let mut locations = build_default_mod_locations()?;
    merge_custom_mod_locations(&mut locations, read_saved_custom_mod_locations()?);
    write_app_settings_file(
        "",
        game_executable_path,
        &client_ram,
        &server_ram,
        max_concurrent_downloads,
        &read_language_preference()?,
        &locations,
    )?;

    load_app_settings()
}

fn delete_all_enabled() -> bool {
    env::var("PZMM_DELETEALL_ENABLED")
        .ok()
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            !normalized.is_empty() && !matches!(normalized.as_str(), "0" | "false" | "no" | "off")
        })
        .unwrap_or(false)
}

fn add_mod_location_impl(path: &str) -> Result<Vec<ModLocation>, String> {
    let path = path.trim();

    if path.is_empty() {
        return Err(text("Select a mod folder.", "Selecione uma pasta de mods.").to_string());
    }

    let path = PathBuf::from(path);

    if !path.exists() {
        return Err(format!(
            "{}: {}.",
            text("Folder not found", "Pasta nao encontrada"),
            path.display()
        ));
    }

    if !path.is_dir() {
        return Err(format!(
            "{} {}.",
            text(
                "The path does not point to a folder:",
                "O caminho nao aponta para uma pasta:"
            ),
            path.display()
        ));
    }

    let mut locations = build_default_mod_locations()?;
    let mut custom_locations = read_saved_custom_mod_locations()?;
    let label = mod_location_label("custom", path.file_name().and_then(|name| name.to_str()));

    custom_locations.push(ModLocation {
        label,
        path: path.display().to_string(),
        kind: "custom".to_string(),
        exists: true,
    });
    merge_custom_mod_locations(&mut locations, custom_locations);
    let game_executable_path = read_config_value("game_executable_path")?.unwrap_or_default();
    let client_ram = read_config_value("client_ram")?.unwrap_or_else(|| "4.00".to_string());
    let server_ram = read_config_value("server_ram")?.unwrap_or_else(|| "4.00".to_string());
    let max_concurrent_downloads = read_max_concurrent_downloads()?;
    let language_preference = read_language_preference()?;
    let steamcmd_path = read_config_value("steamcmd_path")?.unwrap_or_default();
    write_app_settings_file(
        &steamcmd_path,
        &game_executable_path,
        &client_ram,
        &server_ram,
        max_concurrent_downloads,
        &language_preference,
        &locations,
    )?;

    Ok(locations)
}

fn write_app_settings_file(
    steamcmd_path: &str,
    game_executable_path: &str,
    client_ram: &str,
    server_ram: &str,
    max_concurrent_downloads: u32,
    language_preference: &str,
    mod_locations: &[ModLocation],
) -> Result<(), String> {
    let settings_path = app_settings_path()?;

    if let Some(settings_dir) = settings_path.parent() {
        fs::create_dir_all(settings_dir).map_err(|error| {
            format!("Nao foi possivel criar {}: {error}", settings_dir.display())
        })?;
    }

    let mut content = format!(
        "steamcmd_path={steamcmd_path}\ngame_executable_path={game_executable_path}\nclient_ram={client_ram}\nserver_ram={server_ram}\nmax_concurrent_downloads={max_concurrent_downloads}\nlanguage={language_preference}\n"
    );

    for location in mod_locations {
        content.push_str(&format!(
            "mod_location={}|{}\n",
            location.kind, location.path
        ));
    }

    fs::write(&settings_path, content).map_err(|error| {
        format!(
            "Nao foi possivel salvar {}: {error}",
            settings_path.display()
        )
    })?;

    Ok(())
}

pub(crate) fn read_max_concurrent_downloads() -> Result<u32, String> {
    let Some(value) = read_config_value("max_concurrent_downloads")? else {
        return Ok(DEFAULT_MAX_CONCURRENT_DOWNLOADS);
    };

    let parsed = value
        .parse::<u32>()
        .unwrap_or(DEFAULT_MAX_CONCURRENT_DOWNLOADS);

    Ok(parsed.clamp(1, MAX_CONCURRENT_DOWNLOADS_LIMIT))
}

fn validate_max_concurrent_downloads(value: u32) -> Result<u32, String> {
    if (1..=MAX_CONCURRENT_DOWNLOADS_LIMIT).contains(&value) {
        Ok(value)
    } else {
        Err(format!(
            "{} 1 e {MAX_CONCURRENT_DOWNLOADS_LIMIT}.",
            text(
                "Choose a simultaneous download limit between",
                "Escolha um limite de downloads simultaneos entre"
            )
        ))
    }
}

pub(crate) fn read_language_preference() -> Result<String, String> {
    let preference = read_config_value("language")?.unwrap_or_else(|| LANGUAGE_AUTO.to_string());
    Ok(validate_language_preference(&preference)
        .unwrap_or(LANGUAGE_AUTO)
        .to_string())
}

pub(crate) fn save_language_preference(preference: &str) -> Result<(), String> {
    let preference = validate_language_preference(preference)?;
    let game_executable_path = read_config_value("game_executable_path")?.unwrap_or_default();
    let client_ram = read_config_value("client_ram")?.unwrap_or_else(|| "4.00".to_string());
    let server_ram = read_config_value("server_ram")?.unwrap_or_else(|| "4.00".to_string());
    let max_concurrent_downloads = read_max_concurrent_downloads()?;
    let mut locations = build_default_mod_locations()?;
    merge_custom_mod_locations(&mut locations, read_saved_custom_mod_locations()?);
    let steamcmd_path = read_config_value("steamcmd_path")?.unwrap_or_default();
    write_app_settings_file(
        &steamcmd_path,
        &game_executable_path,
        &client_ram,
        &server_ram,
        max_concurrent_downloads,
        preference,
        &locations,
    )
}

fn open_mod_location_impl(path: &str) -> Result<(), String> {
    let path = PathBuf::from(path.trim());

    if !path.exists() {
        return Err(format!(
            "{}: {}.",
            text("Folder not found", "Pasta nao encontrada"),
            path.display()
        ));
    }

    if !path.is_dir() {
        return Err(format!(
            "{} {}.",
            text(
                "The path does not point to a folder:",
                "O caminho nao aponta para uma pasta:"
            ),
            path.display()
        ));
    }

    open_path_external(&path)
}

#[cfg(windows)]
fn select_mod_folder_impl() -> Result<Option<String>, String> {
    let script = format!(
        r#"
Add-Type -AssemblyName System.Windows.Forms
$dialog = New-Object System.Windows.Forms.FolderBrowserDialog
$dialog.Description = '{}'
$dialog.ShowNewFolderButton = $false
if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {{
  [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
  Write-Output $dialog.SelectedPath
}}
"#,
        text(
            "Select folder with Project Zomboid mods",
            "Selecionar pasta com mods do Project Zomboid"
        )
    );

    let mut command = Command::new("powershell.exe");
    let output = hide_command_window(&mut command)
        .args([
            "-NoProfile",
            "-STA",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .output()
        .map_err(|error| {
            format!(
                "{}: {error}",
                text(
                    "Could not open the folder picker",
                    "Nao foi possivel abrir o seletor de pastas"
                )
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        return Err(if stderr.is_empty() {
            text(
                "Could not select the mod folder.",
                "Nao foi possivel selecionar a pasta de mods.",
            )
            .to_string()
        } else {
            stderr
        });
    }

    let selected_path = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if selected_path.is_empty() {
        return Ok(None);
    }

    Ok(Some(selected_path))
}

#[cfg(not(windows))]
fn select_mod_folder_impl() -> Result<Option<String>, String> {
    let output = Command::new("sh")
        .args([
            "-lc",
            &format!(
                "command -v zenity >/dev/null 2>&1 && zenity --file-selection --directory --title={} || command -v kdialog >/dev/null 2>&1 && kdialog --getexistingdirectory ~ || true",
                shell_quote(text(
                    "Select folder with Project Zomboid mods",
                    "Selecionar pasta com mods do Project Zomboid"
                ))
            ),
        ])
        .output()
        .map_err(|error| {
            format!(
                "{}: {error}",
                text(
                    "Could not open the folder picker",
                    "Nao foi possivel abrir o seletor de pastas"
                )
            )
        })?;

    if !output.status.success() {
        return Err(text(
            "Could not select the mod folder.",
            "Nao foi possivel selecionar a pasta de mods.",
        )
        .to_string());
    }

    let selected_path = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if selected_path.is_empty() {
        return Ok(None);
    }

    Ok(Some(selected_path))
}

#[cfg(not(windows))]
fn shell_quote(value: String) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}
