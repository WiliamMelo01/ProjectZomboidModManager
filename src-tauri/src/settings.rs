use crate::game::{apply_performance_settings, normalize_ram_gb, validate_game_executable_path};
use crate::i18n::{mod_location_label, text, validate_language_preference, LANGUAGE_AUTO};
use crate::models::{AppSettings, ModLocation};
use crate::util::hide_command_window;
use crate::{
    app_settings_path, ensure_managed_steamcmd, find_steamcmd_path, read_config_value,
    managed_steamcmd_pool_workshop_dirs, read_configured_steamcmd_path,
    read_saved_custom_mod_locations, read_saved_mod_locations, run_blocking,
    validate_steamcmd_path, zomboid_mods_dir,
};
use std::{collections::HashSet, env, fs, path::PathBuf, process::Command};

pub(crate) const DEFAULT_MAX_CONCURRENT_DOWNLOADS: u32 = 2;
pub(crate) const MAX_CONCURRENT_DOWNLOADS_LIMIT: u32 = 3;

#[tauri::command]
pub(crate) async fn get_app_settings(app: tauri::AppHandle) -> Result<AppSettings, String> {
    run_blocking(move || {
        let _ = ensure_managed_steamcmd(&app);
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
pub(crate) async fn detect_steamcmd_path(app: tauri::AppHandle) -> Result<Option<String>, String> {
    run_blocking(move || {
        let _ = ensure_managed_steamcmd(&app);
        Ok(find_steamcmd_path()?.map(|path| path.display().to_string()))
    })
    .await
}

#[tauri::command]
pub(crate) async fn select_steamcmd_path() -> Result<Option<String>, String> {
    run_blocking(select_steamcmd_path_impl).await
}

#[tauri::command]
pub(crate) async fn select_mod_folder() -> Result<Option<String>, String> {
    run_blocking(select_mod_folder_impl).await
}

#[tauri::command]
pub(crate) async fn add_mod_location(path: String) -> Result<Vec<ModLocation>, String> {
    run_blocking(move || add_mod_location_impl(&path)).await
}

fn load_app_settings() -> Result<AppSettings, String> {
    let configured_path = read_configured_steamcmd_path()?.unwrap_or_default();
    let resolved_steamcmd_path = find_steamcmd_path()?.map(|path| path.display().to_string());
    let is_steamcmd_configured = resolved_steamcmd_path.is_some();
    let game_executable_path = read_config_value("game_executable_path")?.unwrap_or_default();
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

fn get_mod_locations_impl() -> Result<Vec<ModLocation>, String> {
    let saved_locations = read_saved_mod_locations()?;
    let steamcmd_path = read_configured_steamcmd_path()?
        .or_else(|| {
            find_steamcmd_path()
                .ok()
                .flatten()
                .map(|path| path.display().to_string())
        })
        .unwrap_or_default();
    let mut locations = build_default_mod_locations(Some(&steamcmd_path))?;
    merge_custom_mod_locations(
        &mut locations,
        saved_locations
            .into_iter()
            .filter(|location| location.kind == "custom")
            .collect(),
    );
    let game_executable_path = read_config_value("game_executable_path")?.unwrap_or_default();
    let client_ram = read_config_value("client_ram")?.unwrap_or_else(|| "4.00".to_string());
    let server_ram = read_config_value("server_ram")?.unwrap_or_else(|| "4.00".to_string());
    let max_concurrent_downloads = read_max_concurrent_downloads()?;
    let language_preference = read_language_preference()?;
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

fn build_default_mod_locations(steamcmd_path: Option<&str>) -> Result<Vec<ModLocation>, String> {
    let mut locations = Vec::new();
    let mut seen = HashSet::new();

    push_mod_location(
        &mut locations,
        &mut seen,
        &mod_location_label("steam", None),
        "steam",
        default_steam_workshop_dir(),
    );

    push_mod_location(
        &mut locations,
        &mut seen,
        &mod_location_label("local", None),
        "local",
        zomboid_mods_dir()?,
    );

    if let Some(steamcmd_path) = steamcmd_path.map(str::trim).filter(|path| !path.is_empty()) {
        let steamcmd_path = PathBuf::from(steamcmd_path);

        if let Some(steamcmd_dir) = steamcmd_path.parent() {
            push_mod_location(
                &mut locations,
                &mut seen,
                &mod_location_label("steamcmd", None),
                "steamcmd",
                steamcmd_dir
                    .join("steamapps")
                    .join("workshop")
                    .join("content")
                    .join("108600"),
            );
        }
    }

    let steamcmd_label = mod_location_label("steamcmd", None);
    for (index, pool_workshop_dir) in managed_steamcmd_pool_workshop_dirs().into_iter().enumerate()
    {
        push_mod_location(
            &mut locations,
            &mut seen,
            &format!("{} {}", steamcmd_label, index + 1),
            "steamcmd",
            pool_workshop_dir,
        );
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

fn save_app_settings_impl(
    steamcmd_path: &str,
    game_executable_path: &str,
    client_ram: &str,
    server_ram: &str,
    max_concurrent_downloads: u32,
) -> Result<AppSettings, String> {
    let steamcmd_path = steamcmd_path.trim();
    let game_executable_path = game_executable_path.trim();
    let client_ram = normalize_ram_gb(client_ram)?;
    let server_ram = normalize_ram_gb(server_ram)?;
    let max_concurrent_downloads = validate_max_concurrent_downloads(max_concurrent_downloads)?;

    if !steamcmd_path.is_empty() {
        validate_steamcmd_path(&PathBuf::from(steamcmd_path))?;
    }

    if !game_executable_path.is_empty() {
        let game_executable = PathBuf::from(game_executable_path);

        validate_game_executable_path(&game_executable)?;
        apply_performance_settings(&game_executable, &client_ram, &server_ram)?;
    }

    let default_steamcmd_path = if steamcmd_path.is_empty() {
        find_steamcmd_path()?
            .map(|path| path.display().to_string())
            .unwrap_or_default()
    } else {
        steamcmd_path.to_string()
    };
    let mut locations = build_default_mod_locations(Some(&default_steamcmd_path))?;
    merge_custom_mod_locations(&mut locations, read_saved_custom_mod_locations()?);
    write_app_settings_file(
        steamcmd_path,
        game_executable_path,
        &client_ram,
        &server_ram,
        max_concurrent_downloads,
        &read_language_preference()?,
        &locations,
    )?;

    load_app_settings()
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

    let steamcmd_path = read_configured_steamcmd_path()?.unwrap_or_default();
    let mut locations = build_default_mod_locations(Some(&steamcmd_path))?;
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
    let steamcmd_path = read_configured_steamcmd_path()?.unwrap_or_default();
    let game_executable_path = read_config_value("game_executable_path")?.unwrap_or_default();
    let client_ram = read_config_value("client_ram")?.unwrap_or_else(|| "4.00".to_string());
    let server_ram = read_config_value("server_ram")?.unwrap_or_else(|| "4.00".to_string());
    let max_concurrent_downloads = read_max_concurrent_downloads()?;
    let mut locations = build_default_mod_locations(Some(&steamcmd_path))?;
    merge_custom_mod_locations(&mut locations, read_saved_custom_mod_locations()?);
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

#[cfg(windows)]
fn select_steamcmd_path_impl() -> Result<Option<String>, String> {
    let script = format!(r#"
Add-Type -AssemblyName System.Windows.Forms
$dialog = New-Object System.Windows.Forms.OpenFileDialog
$dialog.Title = '{}'
$dialog.Filter = '{}'
$dialog.CheckFileExists = $true
$dialog.Multiselect = $false
if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {{
  [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
  Write-Output $dialog.FileName
}}
"#, text("Select steamcmd.exe", "Selecionar steamcmd.exe"), text("SteamCMD (steamcmd.exe)|steamcmd.exe|Executables (*.exe)|*.exe|All files (*.*)|*.*", "SteamCMD (steamcmd.exe)|steamcmd.exe|Executaveis (*.exe)|*.exe|Todos os arquivos (*.*)|*.*"));

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
                    "Could not open the file picker",
                    "Nao foi possivel abrir o seletor de arquivos"
                )
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        return Err(if stderr.is_empty() {
            text(
                "Could not select the SteamCMD executable.",
                "Nao foi possivel selecionar o executavel do SteamCMD.",
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

    validate_steamcmd_path(&PathBuf::from(&selected_path))?;

    Ok(Some(selected_path))
}

#[cfg(not(windows))]
fn select_steamcmd_path_impl() -> Result<Option<String>, String> {
    Err(text(
        "Automatic file selection is available only on Windows.",
        "Selecao de arquivo automatica esta disponivel apenas no Windows.",
    )
    .to_string())
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
    Err(text(
        "Automatic folder selection is available only on Windows.",
        "Selecao de pasta automatica esta disponivel apenas no Windows.",
    )
    .to_string())
}
