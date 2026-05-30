use crate::game::{apply_performance_settings, normalize_ram_gb, validate_game_executable_path};
use crate::models::{AppSettings, ModLocation};
use crate::{
    app_settings_path, ensure_managed_steamcmd, find_steamcmd_path, read_config_value,
    read_configured_steamcmd_path, read_saved_custom_mod_locations, read_saved_mod_locations,
    run_blocking, validate_steamcmd_path, zomboid_mods_dir,
};
use std::{collections::HashSet, env, fs, path::PathBuf, process::Command};
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
) -> Result<AppSettings, String> {
    run_blocking(move || {
        save_app_settings_impl(
            &steamcmd_path,
            &game_executable_path,
            &client_ram,
            &server_ram,
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

    Ok(AppSettings {
        steamcmd_path: configured_path,
        resolved_steamcmd_path,
        is_steamcmd_configured,
        game_executable_path,
        client_ram,
        server_ram,
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
    write_app_settings_file(
        &steamcmd_path,
        &game_executable_path,
        &client_ram,
        &server_ram,
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
        "Steam Workshop Project Zomboid",
        "steam",
        default_steam_workshop_dir(),
    );

    push_mod_location(
        &mut locations,
        &mut seen,
        "Mods locais do Zomboid",
        "local",
        zomboid_mods_dir()?,
    );

    if let Some(steamcmd_path) = steamcmd_path.map(str::trim).filter(|path| !path.is_empty()) {
        let steamcmd_path = PathBuf::from(steamcmd_path);

        if let Some(steamcmd_dir) = steamcmd_path.parent() {
            push_mod_location(
                &mut locations,
                &mut seen,
                "Downloads do SteamCMD",
                "steamcmd",
                steamcmd_dir
                    .join("steamapps")
                    .join("workshop")
                    .join("content")
                    .join("108600"),
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
) -> Result<AppSettings, String> {
    let steamcmd_path = steamcmd_path.trim();
    let game_executable_path = game_executable_path.trim();
    let client_ram = normalize_ram_gb(client_ram)?;
    let server_ram = normalize_ram_gb(server_ram)?;

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
        &locations,
    )?;

    load_app_settings()
}

fn add_mod_location_impl(path: &str) -> Result<Vec<ModLocation>, String> {
    let path = path.trim();

    if path.is_empty() {
        return Err("Selecione uma pasta de mods.".to_string());
    }

    let path = PathBuf::from(path);

    if !path.exists() {
        return Err(format!("Pasta nao encontrada: {}.", path.display()));
    }

    if !path.is_dir() {
        return Err(format!(
            "O caminho {} nao aponta para uma pasta.",
            path.display()
        ));
    }

    let steamcmd_path = read_configured_steamcmd_path()?.unwrap_or_default();
    let mut locations = build_default_mod_locations(Some(&steamcmd_path))?;
    let mut custom_locations = read_saved_custom_mod_locations()?;
    let label = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .map(|name| format!("Pasta personalizada: {name}"))
        .unwrap_or_else(|| "Pasta personalizada".to_string());

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
    write_app_settings_file(
        &steamcmd_path,
        &game_executable_path,
        &client_ram,
        &server_ram,
        &locations,
    )?;

    Ok(locations)
}

fn write_app_settings_file(
    steamcmd_path: &str,
    game_executable_path: &str,
    client_ram: &str,
    server_ram: &str,
    mod_locations: &[ModLocation],
) -> Result<(), String> {
    let settings_path = app_settings_path()?;

    if let Some(settings_dir) = settings_path.parent() {
        fs::create_dir_all(settings_dir).map_err(|error| {
            format!("Nao foi possivel criar {}: {error}", settings_dir.display())
        })?;
    }

    let mut content = format!(
        "steamcmd_path={steamcmd_path}\ngame_executable_path={game_executable_path}\nclient_ram={client_ram}\nserver_ram={server_ram}\n"
    );

    for location in mod_locations {
        content.push_str(&format!(
            "mod_location={}|{}|{}\n",
            location.kind, location.label, location.path
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

#[cfg(windows)]
fn select_steamcmd_path_impl() -> Result<Option<String>, String> {
    let script = r#"
Add-Type -AssemblyName System.Windows.Forms
$dialog = New-Object System.Windows.Forms.OpenFileDialog
$dialog.Title = 'Selecionar steamcmd.exe'
$dialog.Filter = 'SteamCMD (steamcmd.exe)|steamcmd.exe|Executaveis (*.exe)|*.exe|Todos os arquivos (*.*)|*.*'
$dialog.CheckFileExists = $true
$dialog.Multiselect = $false
if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
  [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
  Write-Output $dialog.FileName
}
"#;

    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-STA",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ])
        .output()
        .map_err(|error| format!("Nao foi possivel abrir o seletor de arquivos: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        return Err(if stderr.is_empty() {
            "Nao foi possivel selecionar o executavel do SteamCMD.".to_string()
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
    Err("Selecao de arquivo automatica esta disponivel apenas no Windows.".to_string())
}

#[cfg(windows)]
fn select_mod_folder_impl() -> Result<Option<String>, String> {
    let script = r#"
Add-Type -AssemblyName System.Windows.Forms
$dialog = New-Object System.Windows.Forms.FolderBrowserDialog
$dialog.Description = 'Selecionar pasta com mods do Project Zomboid'
$dialog.ShowNewFolderButton = $false
if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
  [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
  Write-Output $dialog.SelectedPath
}
"#;

    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-STA",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ])
        .output()
        .map_err(|error| format!("Nao foi possivel abrir o seletor de pastas: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        return Err(if stderr.is_empty() {
            "Nao foi possivel selecionar a pasta de mods.".to_string()
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
    Err("Selecao de pasta automatica esta disponivel apenas no Windows.".to_string())
}
