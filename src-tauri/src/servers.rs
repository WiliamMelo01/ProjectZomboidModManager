use crate::models::ZomboidServer;
use crate::mods::{normalize_server_values, resolve_server_workshop_ids};
use crate::util::*;
use crate::{run_blocking, server_example_dir, zomboid_server_dir};
use std::{fs, path::Path};
#[tauri::command]
pub(crate) async fn list_zomboid_servers() -> Result<Vec<ZomboidServer>, String> {
    run_blocking(list_zomboid_servers_impl).await
}

fn list_zomboid_servers_impl() -> Result<Vec<ZomboidServer>, String> {
    let server_dir = zomboid_server_dir()?;

    if !server_dir.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(&server_dir)
        .map_err(|error| format!("Nao foi possivel ler {}: {error}", server_dir.display()))?;

    let mut servers = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();

        if path.extension().and_then(|extension| extension.to_str()) != Some("ini") {
            continue;
        }

        servers.push(read_zomboid_server_from_path(&path)?);
    }

    servers.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));

    Ok(servers)
}

fn read_zomboid_server_from_path(path: &Path) -> Result<ZomboidServer, String> {
    let content = read_text_lossy(path)?;
    let file_stem = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("server")
        .to_string();

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("server.ini")
        .to_string();

    let name = server_display_name(&file_stem, read_ini_value(&content, "PublicName"));
    let port = read_ini_value(&content, "DefaultPort").unwrap_or_else(|| "16261".to_string());
    let max_players = read_ini_value(&content, "MaxPlayers")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0);
    let active_mod_ids = read_ini_value(&content, "Mods")
        .map(|value| split_mod_ids(&value))
        .unwrap_or_default();
    let mods_count = active_mod_ids.len();

    Ok(ZomboidServer {
        id: file_stem,
        name,
        file_name,
        path: path.display().to_string(),
        port,
        max_players,
        mods_count,
        active_mod_ids,
        status: "offline".to_string(),
    })
}

#[tauri::command]
pub(crate) fn update_zomboid_server_mods(
    server_id: String,
    mod_ids: Vec<String>,
    workshop_ids: Vec<String>,
) -> Result<(), String> {
    let server_path = zomboid_server_dir()?.join(format!("{server_id}.ini"));

    if !server_path.exists() {
        return Err(format!(
            "Arquivo do servidor nao encontrado: {}",
            server_path.display()
        ));
    }

    let content = read_text_lossy(&server_path)?;
    let normalized_mods = normalize_server_values(&mod_ids).join(";");
    let normalized_workshop_ids = resolve_server_workshop_ids(&mod_ids, &workshop_ids)?.join(";");
    let updated_content = replace_or_append_ini_value(&content, "Mods", &normalized_mods);
    let updated_content =
        replace_or_append_ini_value(&updated_content, "WorkshopItems", &normalized_workshop_ids);

    fs::write(&server_path, updated_content)
        .map_err(|error| format!("Nao foi possivel salvar {}: {error}", server_path.display()))
}

#[tauri::command]
pub(crate) fn install_zomboid_server_map(
    server_id: String,
    mod_path: String,
) -> Result<(), String> {
    let server_path = zomboid_server_dir()?.join(format!("{server_id}.ini"));

    if !server_path.exists() {
        return Err(format!(
            "Arquivo do servidor nao encontrado: {}",
            server_path.display()
        ));
    }

    let map_names = find_mod_map_names(Path::new(&mod_path))?;

    if map_names.is_empty() {
        return Err("Este mod nao possui mapas em media/maps.".to_string());
    }

    let content = read_text_lossy(&server_path)?;
    let current_maps = read_ini_value(&content, "Map")
        .map(|value| split_mod_ids(&value))
        .unwrap_or_default();
    let maps = normalize_server_values(
        &map_names
            .into_iter()
            .chain(current_maps)
            .collect::<Vec<_>>(),
    );
    let updated_content = replace_or_append_ini_value(&content, "Map", &maps.join(";"));

    fs::write(&server_path, updated_content)
        .map_err(|error| format!("Nao foi possivel salvar {}: {error}", server_path.display()))
}

fn find_mod_map_names(mod_path: &Path) -> Result<Vec<String>, String> {
    let maps_dir = mod_path.join("media").join("maps");

    if !maps_dir.exists() || !maps_dir.is_dir() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(&maps_dir)
        .map_err(|error| format!("Nao foi possivel ler {}: {error}", maps_dir.display()))?;
    let mut map_names = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();

        if !path.is_dir() || !path.join("map.info").is_file() {
            continue;
        }

        if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
            map_names.push(name.to_string());
        }
    }

    Ok(normalize_server_values(&map_names))
}

#[tauri::command]
pub(crate) async fn create_zomboid_server(
    app: tauri::AppHandle,
    name: String,
    mod_ids: Vec<String>,
    workshop_ids: Vec<String>,
) -> Result<ZomboidServer, String> {
    run_blocking(move || create_zomboid_server_impl(&app, &name, &mod_ids, &workshop_ids)).await
}

fn create_zomboid_server_impl(
    app: &tauri::AppHandle,
    name: &str,
    mod_ids: &[String],
    workshop_ids: &[String],
) -> Result<ZomboidServer, String> {
    let name = name.trim();

    if name.is_empty() {
        return Err("Informe um nome para o servidor.".to_string());
    }

    let server_id = sanitize_server_id(name);

    if server_id.is_empty() {
        return Err("Use um nome de servidor com letras ou numeros.".to_string());
    }

    let server_dir = zomboid_server_dir()?;
    fs::create_dir_all(&server_dir)
        .map_err(|error| format!("Nao foi possivel criar {}: {error}", server_dir.display()))?;

    let server_path = server_dir.join(format!("{server_id}.ini"));

    if server_path.exists() {
        return Err(format!("Ja existe um servidor chamado '{server_id}'."));
    }

    let example_dir = server_example_dir(app)?;
    let template_ini = example_dir.join("servertest.ini");
    let template_sandbox = example_dir.join("servertest_SandboxVars.lua");
    let template_spawnregions = example_dir.join("servertest_spawnregions.lua");

    for template in [&template_ini, &template_sandbox, &template_spawnregions] {
        if !template.exists() {
            return Err(format!(
                "Arquivo de exemplo nao encontrado: {}.",
                template.display()
            ));
        }
    }

    let normalized_mod_ids = normalize_server_values(mod_ids);
    let normalized_workshop_ids = resolve_server_workshop_ids(mod_ids, workshop_ids)?;
    let ini_content = read_text_lossy(&template_ini)?;
    let ini_content = replace_or_append_ini_value(&ini_content, "PublicName", name);
    let ini_content =
        replace_or_append_ini_value(&ini_content, "Mods", &normalized_mod_ids.join(";"));
    let ini_content = replace_or_append_ini_value(
        &ini_content,
        "WorkshopItems",
        &normalized_workshop_ids.join(";"),
    );

    fs::write(&server_path, ini_content)
        .map_err(|error| format!("Nao foi possivel salvar {}: {error}", server_path.display()))?;
    fs::copy(
        &template_sandbox,
        server_dir.join(format!("{server_id}_SandboxVars.lua")),
    )
    .map_err(|error| format!("Nao foi possivel copiar SandboxVars: {error}"))?;
    fs::copy(
        &template_spawnregions,
        server_dir.join(format!("{server_id}_spawnregions.lua")),
    )
    .map_err(|error| format!("Nao foi possivel copiar spawnregions: {error}"))?;

    read_zomboid_server_from_path(&server_path)
}

fn server_display_name(file_stem: &str, public_name: Option<String>) -> String {
    if let Some(public_name) = public_name.map(|value| value.trim().to_string()) {
        if !public_name.is_empty() && !public_name.eq_ignore_ascii_case("My PZ Server") {
            return public_name;
        }
    }

    file_stem
        .replace(['_', '-'], " ")
        .split_whitespace()
        .map(capitalize_first_letter)
        .collect::<Vec<_>>()
        .join(" ")
}

fn sanitize_server_id(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|char| {
            if char.is_ascii_alphanumeric() || char == '-' || char == '_' {
                char
            } else if char.is_whitespace() {
                '_'
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn finds_only_map_folders_with_map_info() {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let mod_dir = std::env::temp_dir().join(format!("pzmm-map-test-{timestamp}"));
        let valid_map_dir = mod_dir.join("media").join("maps").join("BedfordFalls");
        let ignored_map_dir = mod_dir.join("media").join("maps").join("IncompleteMap");

        fs::create_dir_all(&valid_map_dir).expect("valid map directory should be created");
        fs::create_dir_all(&ignored_map_dir).expect("ignored map directory should be created");
        fs::write(valid_map_dir.join("map.info"), "title=Bedford Falls")
            .expect("map.info should be created");

        let map_names = find_mod_map_names(&mod_dir).expect("map folders should be read");
        let _ = fs::remove_dir_all(mod_dir);

        assert_eq!(map_names, vec!["BedfordFalls".to_string()]);
    }
}
