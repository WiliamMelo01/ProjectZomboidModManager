use super::performance::validate_game_executable_path;
use crate::i18n::text;
#[cfg(windows)]
use crate::util::hide_command_window;
#[cfg(not(windows))]
use std::fs;
#[cfg(windows)]
use std::{path::PathBuf, process::Command};
#[cfg(not(windows))]
use std::{path::PathBuf, process::Command};

#[cfg(windows)]
pub(super) fn select_game_executable_impl() -> Result<Option<String>, String> {
    let script = format!(
        r#"
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
"#,
        text(
            "Select Project Zomboid executable",
            "Selecionar executavel do Project Zomboid"
        ),
        text(
            "Project Zomboid (*.exe)|*.exe|All files (*.*)|*.*",
            "Project Zomboid (*.exe)|*.exe|Todos os arquivos (*.*)|*.*"
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
                    "Could not open the file picker",
                    "Nao foi possivel abrir o seletor de arquivos"
                )
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        return Err(if stderr.is_empty() {
            text(
                "Could not select the Project Zomboid executable.",
                "Nao foi possivel selecionar o executavel do Project Zomboid.",
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

    validate_game_executable_path(&PathBuf::from(&selected_path))?;

    Ok(Some(selected_path))
}

#[cfg(not(windows))]
pub(super) fn select_game_executable_impl() -> Result<Option<String>, String> {
    let output = Command::new("sh")
        .args([
            "-lc",
            &format!(
                "command -v zenity >/dev/null 2>&1 && zenity --file-selection --title={} || command -v kdialog >/dev/null 2>&1 && kdialog --getopenfilename ~ '' || true",
                shell_quote(text(
                    "Select Project Zomboid executable",
                    "Selecionar executavel do Project Zomboid"
                ))
            ),
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
        return Err(text(
            "Could not select the Project Zomboid executable.",
            "Nao foi possivel selecionar o executavel do Project Zomboid.",
        )
        .to_string());
    }

    let selected_path = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if selected_path.is_empty() {
        return Ok(None);
    }

    validate_game_executable_path(&PathBuf::from(&selected_path))?;

    Ok(Some(selected_path))
}

#[cfg(windows)]
pub(super) fn get_system_ram_impl() -> Result<u32, String> {
    let mut command = Command::new("powershell.exe");
    let output = hide_command_window(&mut command)
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
pub(super) fn get_system_ram_impl() -> Result<u32, String> {
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

#[cfg(not(windows))]
fn shell_quote(value: String) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}
