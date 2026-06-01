use std::{fs, path::Path, process::Command};

pub(crate) fn hide_command_window(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    command
}

pub(crate) fn read_text_lossy(path: &Path) -> Result<String, String> {
    let content_bytes = fs::read(path)
        .map_err(|error| format!("Nao foi possivel ler {}: {error}", path.display()))?;

    Ok(String::from_utf8_lossy(&content_bytes).to_string())
}

pub(crate) fn read_ini_value(content: &str, key: &str) -> Option<String> {
    content.lines().find_map(|line| {
        let line = line.trim();

        if line.is_empty() || line.starts_with('#') {
            return None;
        }

        let (current_key, value) = line.split_once('=')?;

        if current_key.trim().eq_ignore_ascii_case(key) {
            Some(clean_ini_value(value))
        } else {
            None
        }
    })
}

pub(crate) fn replace_or_append_ini_value(content: &str, key: &str, value: &str) -> String {
    let mut replaced = false;
    let mut lines = content
        .lines()
        .map(|line| {
            let trimmed = line.trim();

            if trimmed.starts_with('#') {
                return line.to_string();
            }

            let current_key = trimmed
                .split_once('=')
                .map(|(current_key, _)| current_key)
                .unwrap_or(trimmed);

            if current_key.trim().eq_ignore_ascii_case(key) {
                replaced = true;
                return format!("{key}={value}");
            }

            line.to_string()
        })
        .collect::<Vec<_>>();

    if !replaced {
        lines.push(format!("{key}={value}"));
    }

    lines.join("\n")
}

pub(crate) fn read_ini_values(content: &str, key: &str) -> Vec<String> {
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();

            if line.is_empty() || line.starts_with('#') {
                return None;
            }

            let (current_key, value) = line.split_once('=')?;

            if current_key.trim().eq_ignore_ascii_case(key) {
                Some(clean_ini_value(value))
            } else {
                None
            }
        })
        .collect()
}

pub(crate) fn split_mod_ids(value: &str) -> Vec<String> {
    value
        .split([';', ','])
        .map(str::trim)
        .filter(|mod_id| !mod_id.is_empty())
        .map(ToString::to_string)
        .collect()
}

pub(crate) fn clean_ini_value(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

pub(crate) fn clean_mod_description(value: &str) -> String {
    value
        .replace("<LINE>", " ")
        .replace("<LINE><LINE>", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn directory_size(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };

    entries
        .filter_map(Result::ok)
        .map(|entry| {
            let path = entry.path();

            if path.is_dir() {
                directory_size(&path)
            } else {
                entry.metadata().map(|metadata| metadata.len()).unwrap_or(0)
            }
        })
        .sum()
}

pub(crate) fn format_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let bytes = bytes as f64;

    if bytes >= GB {
        format!("{:.1} GB", bytes / GB)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes / MB)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes / KB)
    } else {
        format!("{bytes:.0} B")
    }
}

pub(crate) fn capitalize_first_letter(value: &str) -> String {
    let mut chars = value.chars();

    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
