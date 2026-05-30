use crate::models::ZomboidInstallationStatus;
use crate::util::read_text_lossy;
use crate::workshop::open_path_external;
use crate::{read_steam_library_dirs, run_blocking};
use serde_json::Value;
use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};
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

fn open_steam_zomboid_folder_impl() -> Result<String, String> {
    let Some(zomboid_dir) = steam_zomboid_game_dirs()
        .into_iter()
        .find(|path| path.exists())
    else {
        return Err(
            "Nao encontrei a pasta padrao do Project Zomboid na Steam. Verifique se o jogo esta instalado pela Steam."
                .to_string(),
        );
    };

    open_path_external(&zomboid_dir)?;

    Ok(zomboid_dir.display().to_string())
}

fn scan_zomboid_installation_impl(
    game_executable_path: Option<&str>,
) -> Result<ZomboidInstallationStatus, String> {
    let default_game_dir = steam_zomboid_game_dirs()
        .into_iter()
        .find(|path| path.exists())
        .unwrap_or_else(default_steam_zomboid_game_dir);
    let configured_executable = game_executable_path
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from);
    let detected_executable = configured_executable
        .as_ref()
        .filter(|path| path.exists() && path.is_file())
        .cloned()
        .or_else(|| find_zomboid_executable_in_dir(&default_game_dir));
    let config_dir = detected_executable
        .as_deref()
        .and_then(Path::parent)
        .unwrap_or(default_game_dir.as_path());
    let client_configs = detected_executable
        .as_ref()
        .map(|path| client_config_candidates(path))
        .unwrap_or_else(|| client_config_candidates(&config_dir.join("ProjectZomboid64.exe")));
    let server_configs = server_config_candidates(config_dir);

    Ok(ZomboidInstallationStatus {
        default_game_dir: default_game_dir.display().to_string(),
        detected_executable_path: detected_executable
            .as_ref()
            .map(|path| path.display().to_string()),
        is_game_dir_found: default_game_dir.exists() && default_game_dir.is_dir(),
        is_executable_found: detected_executable.is_some(),
        is_client_config_found: client_configs
            .iter()
            .any(|path| path.exists() && path.is_file()),
        is_server_config_found: server_configs
            .iter()
            .any(|path| path.exists() && path.is_file()),
    })
}

pub(crate) fn steam_zomboid_game_dirs() -> Vec<PathBuf> {
    let mut steamapps_dirs = Vec::new();
    let mut candidates = Vec::new();

    if let Some(program_files_x86) = env::var_os("ProgramFiles(x86)") {
        candidates.push(PathBuf::from(program_files_x86).join("Steam"));
    }

    if let Some(program_files) = env::var_os("ProgramFiles") {
        candidates.push(PathBuf::from(program_files).join("Steam"));
    }

    if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
        candidates.push(PathBuf::from(local_app_data).join("Steam"));
    }

    for steam_dir in candidates {
        let steamapps_dir = steam_dir.join("steamapps");

        if steamapps_dir.exists() {
            steamapps_dirs.push(steamapps_dir.clone());
            steamapps_dirs.extend(read_steam_library_dirs(
                &steamapps_dir.join("libraryfolders.vdf"),
            ));
        }
    }

    dedupe_paths(
        steamapps_dirs
            .into_iter()
            .map(|steamapps_dir| steamapps_dir.join("common").join("ProjectZomboid"))
            .collect(),
    )
}

fn default_steam_zomboid_game_dir() -> PathBuf {
    if let Some(program_files_x86) = env::var_os("ProgramFiles(x86)") {
        return PathBuf::from(program_files_x86)
            .join("Steam")
            .join("steamapps")
            .join("common")
            .join("ProjectZomboid");
    }

    PathBuf::from(r"C:\Program Files (x86)")
        .join("Steam")
        .join("steamapps")
        .join("common")
        .join("ProjectZomboid")
}

fn find_zomboid_executable_in_dir(game_dir: &Path) -> Option<PathBuf> {
    for file_name in [
        "ProjectZomboid64.exe",
        "ProjectZomboid32.exe",
        "ProjectZomboid.exe",
    ] {
        let candidate = game_dir.join(file_name);

        if candidate.exists() && candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

pub(crate) fn normalize_ram_gb(value: &str) -> Result<String, String> {
    let ram = value
        .trim()
        .replace(',', ".")
        .parse::<f64>()
        .map_err(|_| "Informe um valor valido de RAM.".to_string())?;

    if !ram.is_finite() || ram < 0.25 {
        return Err("A RAM precisa ser de pelo menos 0.25 GB.".to_string());
    }

    Ok(format!("{ram:.2}"))
}

fn ram_gb_to_mb(value: &str) -> Result<u32, String> {
    let ram = value
        .trim()
        .replace(',', ".")
        .parse::<f64>()
        .map_err(|_| "Informe um valor valido de RAM.".to_string())?;

    Ok((ram * 1024.0).round() as u32)
}

pub(crate) fn validate_game_executable_path(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!(
            "Executavel do Project Zomboid nao encontrado em {}.",
            path.display()
        ));
    }

    if !path.is_file() {
        return Err(format!(
            "O caminho {} nao aponta para um executavel.",
            path.display()
        ));
    }

    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();

    if !extension.eq_ignore_ascii_case("exe") {
        return Err("Selecione um arquivo .exe do Project Zomboid.".to_string());
    }

    Ok(())
}

pub(crate) fn apply_performance_settings(
    game_executable: &Path,
    client_ram: &str,
    server_ram: &str,
) -> Result<(), String> {
    let game_dir = game_executable
        .parent()
        .ok_or_else(|| "Nao foi possivel localizar a pasta do executavel.".to_string())?;

    let client_mb = ram_gb_to_mb(client_ram)?;
    let server_mb = ram_gb_to_mb(server_ram)?;
    let client_configs = client_config_candidates(game_executable);
    let server_configs = server_config_candidates(game_dir);
    let updated_client = update_launcher_configs(&client_configs, client_mb)?;
    let mut updated_server = false;

    for candidate in server_configs {
        if !candidate.exists() || !candidate.is_file() {
            continue;
        }

        let extension = candidate
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_lowercase();

        if extension == "json" {
            update_launcher_json(&candidate, server_mb)?;
            updated_server = true;
        } else if extension == "bat" {
            updated_server = update_launcher_batch(&candidate, server_mb)? || updated_server;
        }
    }

    if !updated_client {
        return Err(format!(
            "Nao encontrei arquivos de configuracao do launcher ao lado de {}.",
            game_executable.display()
        ));
    }

    let _ = updated_server;

    Ok(())
}

fn client_config_candidates(game_executable: &Path) -> Vec<PathBuf> {
    let game_dir = game_executable.parent().unwrap_or_else(|| Path::new(""));
    let mut candidates = Vec::new();

    if let Some(stem) = game_executable.file_stem().and_then(|name| name.to_str()) {
        candidates.push(game_dir.join(format!("{stem}.json")));
        candidates.push(game_dir.join(format!("{stem}.bat")));
    }

    candidates.extend([
        game_dir.join("ProjectZomboid64.json"),
        game_dir.join("ProjectZomboid32.json"),
        game_dir.join("ProjectZomboid64.bat"),
        game_dir.join("ProjectZomboid32.bat"),
    ]);
    dedupe_paths(candidates)
}

fn server_config_candidates(game_dir: &Path) -> Vec<PathBuf> {
    dedupe_paths(vec![game_dir.join("ProjectZomboidServer.bat")])
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();

    paths
        .into_iter()
        .filter(|path| seen.insert(path.display().to_string().to_lowercase()))
        .collect()
}

fn update_launcher_configs(paths: &[PathBuf], ram_mb: u32) -> Result<bool, String> {
    let mut updated_any = false;

    for path in paths {
        if !path.exists() || !path.is_file() {
            continue;
        }

        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_lowercase();

        if extension == "json" {
            update_launcher_json(path, ram_mb)?;
            updated_any = true;
        } else if extension == "bat" {
            updated_any = update_launcher_batch(path, ram_mb)? || updated_any;
        }
    }

    Ok(updated_any)
}

fn update_launcher_json(path: &Path, ram_mb: u32) -> Result<(), String> {
    let content = read_text_lossy(path)?;
    let mut data = serde_json::from_str::<Value>(&content)
        .map_err(|error| format!("Nao foi possivel ler {} como JSON: {error}", path.display()))?;

    match data.get_mut("vmArgs") {
        Some(Value::Array(args)) => update_vm_args_array(args, ram_mb),
        Some(Value::String(args)) => {
            *args = update_vm_args_line(args, ram_mb);
        }
        _ => {
            if let Some(object) = data.as_object_mut() {
                object.insert(
                    "vmArgs".to_string(),
                    Value::Array(vec![
                        Value::String(format!("-Xms{ram_mb}m")),
                        Value::String(format!("-Xmx{ram_mb}m")),
                    ]),
                );
            }
        }
    }

    let content = serde_json::to_string_pretty(&data)
        .map_err(|error| format!("Nao foi possivel gerar JSON atualizado: {error}"))?;
    fs::write(path, format!("{content}\n"))
        .map_err(|error| format!("Nao foi possivel salvar {}: {error}", path.display()))
}

fn update_vm_args_array(args: &mut Vec<Value>, ram_mb: u32) {
    let mut has_xms = false;
    let mut has_xmx = false;

    for arg in args.iter_mut() {
        let Some(value) = arg.as_str() else {
            continue;
        };

        if is_memory_arg(value, "-Xms") {
            *arg = Value::String(format!("-Xms{ram_mb}m"));
            has_xms = true;
        } else if is_memory_arg(value, "-Xmx") {
            *arg = Value::String(format!("-Xmx{ram_mb}m"));
            has_xmx = true;
        }
    }

    if !has_xms {
        args.insert(0, Value::String(format!("-Xms{ram_mb}m")));
    }

    if !has_xmx {
        args.insert(1, Value::String(format!("-Xmx{ram_mb}m")));
    }
}

fn update_launcher_batch(path: &Path, ram_mb: u32) -> Result<bool, String> {
    let content = read_text_lossy(path)?;
    let lower_content = content.to_lowercase();

    if !lower_content.contains("-xms") && !lower_content.contains("-xmx") {
        return Ok(false);
    }

    let updated = content
        .lines()
        .map(|line| {
            let lower_line = line.to_lowercase();

            if lower_line.contains("-xms") || lower_line.contains("-xmx") {
                update_vm_args_line(line, ram_mb)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    fs::write(path, updated)
        .map_err(|error| format!("Nao foi possivel salvar {}: {error}", path.display()))?;

    Ok(true)
}

fn update_vm_args_line(content: &str, ram_mb: u32) -> String {
    let tokens = content
        .split_whitespace()
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    if let Some(java_index) = tokens.iter().position(|token| is_java_command_token(token)) {
        let mut java_index_after_filter = None;
        let mut filtered_tokens = Vec::new();

        for (index, token) in tokens.into_iter().enumerate() {
            if is_memory_arg(&token, "-Xms") || is_memory_arg(&token, "-Xmx") {
                continue;
            }

            if index == java_index {
                java_index_after_filter = Some(filtered_tokens.len());
            }

            filtered_tokens.push(token);
        }

        if let Some(index) = java_index_after_filter {
            filtered_tokens.insert(index + 1, format!("-Xmx{ram_mb}m"));
            filtered_tokens.insert(index + 1, format!("-Xms{ram_mb}m"));

            return filtered_tokens.join(" ");
        }
    }

    let mut has_xms = false;
    let mut has_xmx = false;
    let updated = content
        .split_whitespace()
        .map(|token| {
            if is_memory_arg(token, "-Xms") {
                has_xms = true;
                format!("-Xms{ram_mb}m")
            } else if is_memory_arg(token, "-Xmx") {
                has_xmx = true;
                format!("-Xmx{ram_mb}m")
            } else {
                token.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    match (has_xms, has_xmx) {
        (true, true) => updated,
        (false, true) => format!("-Xms{ram_mb}m {updated}"),
        (true, false) => updated.replace(
            &format!("-Xms{ram_mb}m"),
            &format!("-Xms{ram_mb}m -Xmx{ram_mb}m"),
        ),
        (false, false) => format!("-Xms{ram_mb}m -Xmx{ram_mb}m {updated}"),
    }
}

fn is_memory_arg(value: &str, prefix: &str) -> bool {
    let value = value.trim();

    value.len() > prefix.len()
        && value
            .get(..prefix.len())
            .is_some_and(|current_prefix| current_prefix.eq_ignore_ascii_case(prefix))
}

fn is_java_command_token(value: &str) -> bool {
    let value = value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .replace('/', "\\")
        .to_lowercase();

    value == "java"
        || value == "java.exe"
        || value.ends_with("\\java")
        || value.ends_with("\\java.exe")
}

#[cfg(windows)]
fn select_game_executable_impl() -> Result<Option<String>, String> {
    let script = r#"
Add-Type -AssemblyName System.Windows.Forms
$dialog = New-Object System.Windows.Forms.OpenFileDialog
$dialog.Title = 'Selecionar executavel do Project Zomboid'
$dialog.Filter = 'Project Zomboid (*.exe)|*.exe|Todos os arquivos (*.*)|*.*'
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
            "Nao foi possivel selecionar o executavel do Project Zomboid.".to_string()
        } else {
            stderr
        });
    }

    let selected_path = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if selected_path.is_empty() {
        return Ok(None);
    }

    validate_game_executable_path(&PathBuf::from(&selected_path))?;

    Ok(Some(selected_path))
}

#[cfg(not(windows))]
fn select_game_executable_impl() -> Result<Option<String>, String> {
    Err("Selecao de arquivo automatica esta disponivel apenas no Windows.".to_string())
}

#[cfg(windows)]
fn get_system_ram_impl() -> Result<u32, String> {
    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-Command",
            "[math]::Ceiling((Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory / 1GB)",
        ])
        .output()
        .map_err(|error| format!("Nao foi possivel detectar a RAM do sistema: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        return Err(if stderr.is_empty() {
            "Nao foi possivel detectar a RAM do sistema.".to_string()
        } else {
            stderr
        });
    }

    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u32>()
        .map(|ram| ram.max(1))
        .map_err(|_| "Nao foi possivel interpretar a RAM do sistema.".to_string())
}

#[cfg(not(windows))]
fn get_system_ram_impl() -> Result<u32, String> {
    let content = fs::read_to_string("/proc/meminfo").unwrap_or_default();

    for line in content.lines() {
        if !line.starts_with("MemTotal:") {
            continue;
        }

        let kb = line
            .split_whitespace()
            .nth(1)
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);

        if kb > 0 {
            return Ok(((kb as f64 / 1024.0 / 1024.0).ceil() as u32).max(1));
        }
    }

    Ok(16)
}
