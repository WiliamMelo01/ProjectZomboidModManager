use crate::i18n::text;
use crate::models::{ZomboidMod, ZomboidModVariant, BUILD_41, BUILD_42};
use crate::util::{
    capitalize_first_letter, clean_mod_description, directory_size, format_size, read_ini_value,
    read_ini_values, read_text_lossy, split_mod_ids,
};
use base64::{engine::general_purpose, Engine as _};
use std::{collections::HashSet, fs, path::Path};

pub(super) fn read_mod_package(
    package_dir: &Path,
    workshop_id: Option<&str>,
    source: &str,
) -> Result<Option<ZomboidMod>, String> {
    let mut variants = Vec::new();
    let root_info = package_dir.join("mod.info");

    if root_info.is_file() {
        variants.push(read_variant(&root_info, BUILD_41)?);
    }

    let mut b42_infos = fs::read_dir(package_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir() && is_b42_dir(path))
        .map(|path| path.join("mod.info"))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    b42_infos.sort_by_key(|path| b42_version_key(path.parent().unwrap_or(path)));

    if let Some(mod_info) = b42_infos.last() {
        variants.push(read_variant(mod_info, BUILD_42)?);
    }

    if variants.is_empty() {
        return Ok(None);
    }

    let display = variants
        .iter()
        .find(|variant| variant.game_build == BUILD_41)
        .unwrap_or_else(|| variants.first().expect("variants cannot be empty"));
    let display_content = read_text_lossy(Path::new(&display.path).join("mod.info").as_path())?;
    let image_url = find_mod_image_url(&display_content, Path::new(&display.path))
        .or_else(|| find_package_image_url(package_dir));
    let compatible_builds = variants
        .iter()
        .map(|variant| variant.game_build.clone())
        .collect::<Vec<_>>();

    Ok(Some(ZomboidMod {
        id: display.id.clone(),
        name: read_ini_value(&display_content, "name")
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| capitalize_first_letter(&display.id)),
        author: read_ini_value(&display_content, "Authors")
            .or_else(|| read_ini_value(&display_content, "author"))
            .unwrap_or_else(|| text("Unknown", "Desconhecido").to_string()),
        version: read_ini_value(&display_content, "version").unwrap_or_else(|| "-".to_string()),
        workshop_id: workshop_id.unwrap_or("").to_string(),
        description: read_ini_value(&display_content, "description")
            .map(|value| clean_mod_description(&value))
            .unwrap_or_else(|| {
                text("No description available.", "Sem descricao disponivel.").to_string()
            }),
        size: format_size(directory_size(package_dir)),
        is_installed: source == "local",
        source: source.to_string(),
        path: display.path.clone(),
        image_url,
        dependencies: display.dependencies.clone(),
        map_names: display.map_names.clone(),
        compatible_builds,
        variants,
        package_path: package_dir.display().to_string(),
    }))
}

fn read_variant(mod_info: &Path, game_build: &str) -> Result<ZomboidModVariant, String> {
    let content = read_text_lossy(mod_info)?;
    let mod_dir = mod_info.parent().unwrap_or(mod_info);
    let id = read_ini_value(&content, "id").unwrap_or_else(|| {
        mod_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
            .to_string()
    });

    Ok(ZomboidModVariant {
        game_build: game_build.to_string(),
        id,
        path: mod_dir.display().to_string(),
        dependencies: parse_mod_dependencies(&content),
        map_names: find_mod_map_names(mod_dir),
    })
}

fn is_b42_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name == "42" || name.starts_with("42."))
        .unwrap_or(false)
}

fn b42_version_key(path: &Path) -> Vec<u32> {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .split('.')
        .map(|part| part.parse::<u32>().unwrap_or(0))
        .collect()
}

fn parse_mod_dependencies(content: &str) -> Vec<String> {
    let mut dependencies = Vec::new();
    let mut seen = HashSet::new();
    for value in read_ini_values(content, "require") {
        for dependency_id in split_mod_ids(&value) {
            let dependency_id = dependency_id
                .strip_prefix('\\')
                .unwrap_or(&dependency_id)
                .to_string();
            if seen.insert(dependency_id.to_lowercase()) {
                dependencies.push(dependency_id);
            }
        }
    }
    dependencies
}

fn find_mod_map_names(mod_dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(mod_dir.join("media").join("maps")) else {
        return Vec::new();
    };
    let mut map_names = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir() && path.join("map.info").is_file())
        .filter_map(|path| path.file_name()?.to_str().map(ToString::to_string))
        .collect::<Vec<_>>();
    map_names.sort_by_key(|name| name.to_lowercase());
    map_names.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    map_names
}

fn find_package_image_url(package_dir: &Path) -> Option<String> {
    ["poster.png", "poster.jpg", "icon.png", "icon.jpg"]
        .into_iter()
        .map(|name| package_dir.join(name))
        .find_map(|path| image_file_to_data_url(&path))
}

fn find_mod_image_url(content: &str, mod_dir: &Path) -> Option<String> {
    read_ini_values(content, "poster")
        .into_iter()
        .chain(read_ini_values(content, "icon"))
        .filter(|value| !value.trim().is_empty())
        .find_map(|candidate| image_file_to_data_url(&mod_dir.join(candidate)))
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
    Some(format!(
        "data:{mime_type};base64,{}",
        general_purpose::STANDARD.encode(bytes)
    ))
}

pub(super) fn variant_ids(mod_item: &ZomboidMod) -> Vec<String> {
    mod_item
        .variants
        .iter()
        .map(|variant| variant.id.to_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("pzmm-{label}-{timestamp}"))
    }

    #[test]
    fn reads_b42_package_once_with_latest_variant() {
        let package = temp_dir("b42-package");
        fs::create_dir_all(package.join("42")).unwrap();
        fs::create_dir_all(package.join("42.17")).unwrap();
        fs::create_dir_all(package.join("common")).unwrap();
        fs::write(
            package.join("42").join("mod.info"),
            "name=Example\nid=123/Example",
        )
        .unwrap();
        fs::write(
            package.join("42.17").join("mod.info"),
            "name=Example\nid=123/Example\nversionMin=42.17",
        )
        .unwrap();
        fs::write(
            package.join("common").join("mod.info"),
            "name=Shared\nid=Shared",
        )
        .unwrap();

        let mod_item = read_mod_package(&package, Some("123"), "steam")
            .unwrap()
            .unwrap();
        let _ = fs::remove_dir_all(package);

        assert_eq!(mod_item.compatible_builds, vec![BUILD_42.to_string()]);
        assert_eq!(mod_item.variants.len(), 1);
        assert!(mod_item.variants[0].path.ends_with("42.17"));
    }

    #[test]
    fn reads_hybrid_package_with_b41_and_b42_variants() {
        let package = temp_dir("hybrid-package");
        fs::create_dir_all(package.join("42")).unwrap();
        fs::write(package.join("mod.info"), "name=Example\nid=Example").unwrap();
        fs::write(
            package.join("42").join("mod.info"),
            "name=Example\nid=123/Example",
        )
        .unwrap();

        let mod_item = read_mod_package(&package, Some("123"), "steam")
            .unwrap()
            .unwrap();
        let _ = fs::remove_dir_all(package);

        assert_eq!(
            mod_item.compatible_builds,
            vec![BUILD_41.to_string(), BUILD_42.to_string()]
        );
        assert_eq!(mod_item.variants[0].id, "Example");
        assert_eq!(mod_item.variants[1].id, "123/Example");
    }

    #[test]
    fn removes_b42_prefix_from_dependencies() {
        assert_eq!(
            parse_mod_dependencies("require=\\damnlib;\\OtherDependency"),
            vec!["damnlib".to_string(), "OtherDependency".to_string()]
        );
    }
}
