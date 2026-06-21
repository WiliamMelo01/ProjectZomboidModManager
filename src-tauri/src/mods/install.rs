use super::discovery::write_local_workshop_id;
use crate::models::ZomboidModInstallResult;
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
    let source = PathBuf::from(&package_path);

    if !source.exists() || !source.is_dir() {
        return Err(format!("Pasta do mod nao encontrada: {}", source.display()));
    }

    let target_root = zomboid_mods_dir()?;
    fs::create_dir_all(&target_root)
        .map_err(|error| format!("Nao foi possivel criar {}: {error}", target_root.display()))?;

    install_mod(&source, &mod_id, &target_root, Some(&workshop_id))
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
