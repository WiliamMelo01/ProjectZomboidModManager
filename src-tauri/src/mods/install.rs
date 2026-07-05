use super::discovery::write_local_workshop_id;
use crate::models::ZomboidModInstallResult;
use crate::util::{read_ini_value, read_text_lossy};
use crate::zomboid_mods_dir;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) fn install_zomboid_mod_impl(
    package_path: String,
    mod_id: String,
    workshop_id: String,
) -> Result<ZomboidModInstallResult, String> {
    let requested_source = PathBuf::from(&package_path);
    let source = resolve_install_source(&requested_source, &mod_id)?;

    let target_root = zomboid_mods_dir()?;
    fs::create_dir_all(&target_root)
        .map_err(|error| format!("Nao foi possivel criar {}: {error}", target_root.display()))?;

    install_mod(&source, &mod_id, &target_root, Some(&workshop_id))
}


fn resolve_install_source(requested_source: &Path, mod_id: &str) -> Result<PathBuf, String> {
    if requested_source.is_dir() {
        return Ok(requested_source.to_path_buf());
    }

    let Some(mods_root) = requested_source.parent() else {
        return Err(format!(
            "Pasta do mod nao encontrada: {}",
            requested_source.display()
        ));
    };

    if !mods_root.is_dir() {
        return Err(format!(
            "Pasta do mod nao encontrada: {}",
            requested_source.display()
        ));
    }

    let mut candidates = fs::read_dir(mods_root)
        .map_err(|error| format!("Nao foi possivel ler {}: {error}", mods_root.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    candidates.sort_by_key(|path| path.display().to_string().to_lowercase());

    let normalized_mod_id = normalize_mod_id(mod_id);

    if let Some(candidate) = candidates.iter().find(|candidate| {
        candidate
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| normalize_mod_id(name) == normalized_mod_id)
            .unwrap_or(false)
    }) {
        return Ok(candidate.clone());
    }

    if let Some(candidate) = candidates
        .iter()
        .find(|candidate| package_contains_mod_id(candidate, &normalized_mod_id))
    {
        return Ok(candidate.clone());
    }

    if candidates.len() == 1 {
        return Ok(candidates.remove(0));
    }

    Err(format!(
        "Pasta do mod nao encontrada: {}. Pastas disponiveis em {}: {}",
        requested_source.display(),
        mods_root.display(),
        candidates
            .iter()
            .filter_map(|path| path.file_name()?.to_str().map(ToString::to_string))
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

fn package_contains_mod_id(package_dir: &Path, normalized_mod_id: &str) -> bool {
    package_mod_info_ids(package_dir)
        .iter()
        .any(|id| normalize_mod_id(id) == normalized_mod_id)
}

fn package_mod_info_ids(package_dir: &Path) -> Vec<String> {
    let mut ids = Vec::new();
    collect_mod_info_id(&package_dir.join("mod.info"), &mut ids);

    if let Ok(entries) = fs::read_dir(package_dir) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                collect_mod_info_id(&path.join("mod.info"), &mut ids);
            }
        }
    }

    ids
}

fn collect_mod_info_id(mod_info: &Path, ids: &mut Vec<String>) {
    if !mod_info.is_file() {
        return;
    }

    if let Ok(content) = read_text_lossy(mod_info) {
        if let Some(id) = read_ini_value(&content, "id") {
            ids.push(id);
        }
    }
}

fn normalize_mod_id(value: &str) -> String {
    value.trim().trim_start_matches('\\').trim_start_matches('+').to_lowercase()
}

fn install_mod(
    source: &Path,
    mod_id: &str,
    target_root: &Path,
    workshop_id: Option<&str>,
) -> Result<ZomboidModInstallResult, String> {
    let folder_name = if mod_id.trim().is_empty() {
        source
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("mod")
            .to_string()
    } else {
        sanitize_folder_name(mod_id)
    };
    let target = target_root.join(folder_name);
    let was_copied = !target.exists();

    if was_copied {
        copy_dir_recursive(source, &target)?;
    }

    write_local_workshop_id(&target, workshop_id)?;

    Ok(ZomboidModInstallResult {
        mod_id: mod_id.to_string(),
        workshop_id: workshop_id.unwrap_or_default().to_string(),
        target_path: target.display().to_string(),
        was_copied,
    })
}

fn copy_dir_recursive(source: &Path, target: &Path) -> Result<(), String> {
    fs::create_dir_all(target)
        .map_err(|error| format!("Nao foi possivel criar {}: {error}", target.display()))?;

    let entries = fs::read_dir(source)
        .map_err(|error| format!("Nao foi possivel ler {}: {error}", source.display()))?;

    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());

        if source_path.is_dir() {
            copy_dir_recursive(&source_path, &target_path)?;
        } else {
            fs::copy(&source_path, &target_path).map_err(|error| {
                format!(
                    "Nao foi possivel copiar {} para {}: {error}",
                    source_path.display(),
                    target_path.display()
                )
            })?;
        }
    }

    Ok(())
}

fn sanitize_folder_name(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|char| match char {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ => char,
        })
        .collect::<String>();

    if sanitized.trim().is_empty() {
        "mod".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn resolves_missing_source_from_sibling_mod_info_id() {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("pzmm-install-resolve-{timestamp}"));
        let mods_root = root.join("workshop").join("3077900375").join("mods");
        let real_source = mods_root.join("ChuckleberryFinnAlertSystem");
        let requested_source = mods_root.join("chuckleberryModdingAlertSystem");
        fs::create_dir_all(&real_source).unwrap();
        fs::write(
            real_source.join("mod.info"),
            "name=Optional Mod Update and Alert System
id=ChuckleberryFinnAlertSystem",
        )
        .unwrap();

        let resolved = resolve_install_source(&requested_source, "ChuckleberryFinnAlertSystem").unwrap();

        assert_eq!(resolved, real_source);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn copies_complete_versioned_package_tree() {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("pzmm-install-{timestamp}"));
        let source = root.join("source");
        let target = root.join("target");
        fs::create_dir_all(source.join("42.17").join("media")).unwrap();
        fs::create_dir_all(source.join("common")).unwrap();
        fs::write(source.join("mod.info"), "id=Example").unwrap();
        fs::write(source.join("42.17").join("mod.info"), "id=123/Example").unwrap();
        fs::write(source.join("common").join("shared.txt"), "shared").unwrap();

        install_mod(&source, "Example", &target, Some("123")).unwrap();

        assert!(target.join("Example").join("mod.info").is_file());
        assert!(target
            .join("Example")
            .join("42.17")
            .join("mod.info")
            .is_file());
        assert!(target
            .join("Example")
            .join("common")
            .join("shared.txt")
            .is_file());
        assert_eq!(
            fs::read_to_string(target.join("Example").join(".pzmm-workshop-id"))
                .unwrap()
                .trim(),
            "123"
        );
        let _ = fs::remove_dir_all(root);
    }
}
