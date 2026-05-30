use crate::models::ZomboidMod;
use crate::util::{
    capitalize_first_letter, clean_mod_description, directory_size, format_size, read_ini_value,
    read_ini_values, read_text_lossy, split_mod_ids,
};
use base64::{engine::general_purpose, Engine as _};
use std::{collections::HashSet, fs, path::Path};

pub(super) fn add_mod_id_from_info(
    mod_info_path: &Path,
    workshop_id: Option<&str>,
    source: &str,
    seen: &mut HashSet<String>,
    installed_mod_ids: &HashSet<String>,
) -> Result<Option<String>, String> {
    let content = read_text_lossy(mod_info_path)?;
    let mod_id = read_ini_value(content.as_ref(), "id").unwrap_or_else(|| {
        mod_info_path
            .parent()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
            .to_string()
    });
    let normalized_mod_id = mod_id.to_lowercase();

    if source == "steam" && installed_mod_ids.contains(&normalized_mod_id) {
        return Ok(None);
    }

    let workshop_id = workshop_id.unwrap_or("");
    let seen_key = format!("{source}:{workshop_id}:{mod_id}");

    if seen.insert(seen_key) {
        Ok(Some(normalized_mod_id))
    } else {
        Ok(None)
    }
}

pub(super) fn add_mod_from_info(
    mod_info_path: &Path,
    workshop_id: Option<&str>,
    source: &str,
    mods: &mut Vec<ZomboidMod>,
    seen: &mut HashSet<String>,
    installed_mod_ids: &HashSet<String>,
) -> Result<Option<String>, String> {
    let content = read_text_lossy(mod_info_path)?;
    let mod_id = read_ini_value(content.as_ref(), "id").unwrap_or_else(|| {
        mod_info_path
            .parent()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
            .to_string()
    });
    let normalized_mod_id = mod_id.to_lowercase();

    if source == "steam" && installed_mod_ids.contains(&normalized_mod_id) {
        return Ok(None);
    }

    let workshop_id = workshop_id.unwrap_or("").to_string();
    let seen_key = format!("{source}:{workshop_id}:{mod_id}");

    if seen.contains(&seen_key) {
        return Ok(None);
    }

    seen.insert(seen_key);

    let name = read_ini_value(content.as_ref(), "name")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| capitalize_first_letter(&mod_id));
    let author = read_ini_value(content.as_ref(), "Authors")
        .or_else(|| read_ini_value(content.as_ref(), "author"))
        .unwrap_or_else(|| "Desconhecido".to_string());
    let description = read_ini_value(content.as_ref(), "description")
        .map(|value| clean_mod_description(&value))
        .unwrap_or_else(|| "Sem descricao disponivel.".to_string());
    let version = read_ini_value(content.as_ref(), "version").unwrap_or_else(|| "-".to_string());
    let dependencies = parse_mod_dependencies(content.as_ref());
    let mod_dir = mod_info_path.parent().unwrap_or(mod_info_path);
    let image_url = find_mod_image_url(content.as_ref(), mod_dir);

    mods.push(ZomboidMod {
        id: mod_id,
        name,
        author,
        version,
        workshop_id,
        description,
        size: format_size(directory_size(mod_dir)),
        is_installed: source == "local",
        source: source.to_string(),
        path: mod_dir.display().to_string(),
        image_url,
        dependencies,
    });

    Ok(Some(normalized_mod_id))
}

fn parse_mod_dependencies(content: &str) -> Vec<String> {
    let mut dependencies = Vec::new();
    let mut seen = HashSet::new();

    for value in read_ini_values(content, "require") {
        for dependency_id in split_mod_ids(&value) {
            if seen.insert(dependency_id.to_lowercase()) {
                dependencies.push(dependency_id);
            }
        }
    }

    dependencies
}

fn find_mod_image_url(content: &str, mod_dir: &Path) -> Option<String> {
    let candidates = read_ini_values(content, "poster")
        .into_iter()
        .chain(read_ini_values(content, "icon"))
        .filter(|value| !value.trim().is_empty());

    for candidate in candidates {
        let image_path = mod_dir.join(candidate);

        if image_path.exists() && image_path.is_file() {
            if let Some(data_url) = image_file_to_data_url(&image_path) {
                return Some(data_url);
            }
        }
    }

    None
}

fn image_file_to_data_url(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    let mime_type = match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        _ => "image/png",
    };
    let encoded = general_purpose::STANDARD.encode(bytes);

    Some(format!("data:{mime_type};base64,{encoded}"))
}
