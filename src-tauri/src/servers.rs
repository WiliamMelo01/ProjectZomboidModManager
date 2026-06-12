use crate::i18n::text;
use crate::models::{DeleteServerResult, ZomboidServer, BUILD_41, BUILD_42};
use crate::mods::{
    normalize_server_values, parse_server_mod_ids, resolve_server_workshop_ids,
    serialize_server_mod_ids,
};
use crate::util::*;
use crate::workshop::open_file_external;
use crate::{app_config_dir, run_blocking, server_example_dir, zomboid_server_dir};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
#[tauri::command]
pub(crate) async fn list_zomboid_servers() -> Result<Vec<ZomboidServer>, String> {
    run_blocking(list_zomboid_servers_impl).await
}

fn list_zomboid_servers_impl() -> Result<Vec<ZomboidServer>, String> {
    let server_dir = zomboid_server_dir()?;

    if !server_dir.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(&server_dir).map_err(|error| {
        format!(
            "{} {}: {error}",
            text("Could not read", "Nao foi possivel ler"),
            server_dir.display()
        )
    })?;

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

#[tauri::command]
pub(crate) fn open_zomboid_server_file(server_id: String) -> Result<(), String> {
    let server_dir = zomboid_server_dir()?;
    let server_path = server_dir.join(format!("{server_id}.ini"));

    if !server_path.exists() || !server_path.is_file() {
        return Err(format!(
            "{}: {}",
            text(
                "Server file not found",
                "Arquivo do servidor nao encontrado"
            ),
            server_path.display()
        ));
    }

    let canonical_server_dir = server_dir.canonicalize().map_err(|error| {
        format!(
            "{} {}: {error}",
            text("Could not access", "Nao foi possivel acessar"),
            server_dir.display()
        )
    })?;
    let canonical_server_path = server_path.canonicalize().map_err(|error| {
        format!(
            "{} {}: {error}",
            text("Could not access", "Nao foi possivel acessar"),
            server_path.display()
        )
    })?;

    if !canonical_server_path.starts_with(&canonical_server_dir) {
        return Err(text("Invalid server file.", "Arquivo de servidor invalido.").to_string());
    }

    open_file_external(&canonical_server_path)
}

#[tauri::command]
pub(crate) async fn delete_zomboid_server(server_id: String) -> Result<DeleteServerResult, String> {
    run_blocking(move || delete_zomboid_server_impl(&server_id)).await
}

fn delete_zomboid_server_impl(server_id: &str) -> Result<DeleteServerResult, String> {
    let server_dir = zomboid_server_dir()?;
    let server_path = server_dir.join(format!("{server_id}.ini"));

    if !server_path.exists() || !server_path.is_file() {
        return Err(format!(
            "{}: {}",
            text(
                "Server file not found",
                "Arquivo do servidor nao encontrado"
            ),
            server_path.display()
        ));
    }

    let files = server_profile_files(&server_dir, server_id)?;
    if files.is_empty() {
        return Err(text(
            "Server file not found",
            "Arquivo do servidor nao encontrado",
        )
        .to_string());
    }

    let backup_dir = unique_server_backup_dir(server_id)?;
    fs::create_dir_all(&backup_dir)
        .map_err(|error| format!("Nao foi possivel criar {}: {error}", backup_dir.display()))?;

    for source in files {
        let file_name = source.file_name().ok_or_else(|| {
            text("Invalid server file.", "Arquivo de servidor invalido.").to_string()
        })?;
        let target = backup_dir.join(file_name);
        fs::rename(&source, &target).map_err(|error| {
            format!(
                "{} {}: {error}",
                text("Could not move", "Nao foi possivel mover"),
                source.display()
            )
        })?;
    }

    remove_zomboid_server_build(server_id)?;

    Ok(DeleteServerResult {
        backup_path: backup_dir.display().to_string(),
    })
}

fn server_profile_files(server_dir: &Path, server_id: &str) -> Result<Vec<PathBuf>, String> {
    let canonical_server_dir = server_dir.canonicalize().map_err(|error| {
        format!(
            "{} {}: {error}",
            text("Could not access", "Nao foi possivel acessar"),
            server_dir.display()
        )
    })?;

    let entries = fs::read_dir(server_dir).map_err(|error| {
        format!(
            "{} {}: {error}",
            text("Could not read", "Nao foi possivel ler"),
            server_dir.display()
        )
    })?;

    let mut files = Vec::new();
    let ini_name = format!("{server_id}.ini");
    let lua_prefix = format!("{server_id}_");

    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let is_profile_ini = file_name == ini_name;
        let is_profile_lua = file_name.starts_with(&lua_prefix)
            && path.extension().and_then(|extension| extension.to_str()) == Some("lua");

        if !is_profile_ini && !is_profile_lua {
            continue;
        }

        let canonical_path = path.canonicalize().map_err(|error| {
            format!(
                "{} {}: {error}",
                text("Could not access", "Nao foi possivel acessar"),
                path.display()
            )
        })?;

        if !canonical_path.starts_with(&canonical_server_dir) || !canonical_path.is_file() {
            return Err(text("Invalid server file.", "Arquivo de servidor invalido.").to_string());
        }

        files.push(canonical_path);
    }

    files.sort();
    Ok(files)
}

fn unique_server_backup_dir(server_id: &str) -> Result<PathBuf, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("Falha ao gerar timestamp do backup: {error}"))?
        .as_secs();
    let backup_root = app_config_dir()?.join("server-backups");
    let backup_id = sanitize_server_id(server_id);
    let backup_id = if backup_id.is_empty() {
        "server"
    } else {
        &backup_id
    };
    let base_dir = backup_root.join(format!("{backup_id}-{timestamp}"));

    if !base_dir.exists() {
        return Ok(base_dir);
    }

    for suffix in 1..1000 {
        let candidate = backup_root.join(format!("{backup_id}-{timestamp}-{suffix}"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(text(
        "Could not create a unique backup folder.",
        "Nao foi possivel criar uma pasta de backup unica.",
    )
    .to_string())
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
    let game_build = read_zomboid_server_build(&file_stem)?;
    let configured_mods = read_ini_value(&content, "Mods").unwrap_or_default();
    let active_mod_ids = parse_server_mod_ids(&configured_mods);

    if game_build == BUILD_42 {
        let serialized_mod_ids = serialize_server_mod_ids(&active_mod_ids, &game_build);
        if configured_mods != serialized_mod_ids {
            let updated_content =
                replace_or_append_ini_value(&content, "Mods", &serialized_mod_ids);
            fs::write(path, updated_content)
                .map_err(|error| format!("Nao foi possivel salvar {}: {error}", path.display()))?;
        }
    }

    let mods_count = active_mod_ids.len();

    Ok(ZomboidServer {
        id: file_stem.clone(),
        name,
        file_name,
        path: path.display().to_string(),
        port,
        max_players,
        mods_count,
        active_mod_ids,
        status: "offline".to_string(),
        game_build,
    })
}

pub(crate) fn read_zomboid_server_build(server_id: &str) -> Result<String, String> {
    Ok(read_server_builds()?
        .remove(server_id)
        .unwrap_or_else(|| BUILD_41.to_string()))
}

#[tauri::command]
pub(crate) fn update_zomboid_server_build(
    server_id: String,
    game_build: String,
) -> Result<(), String> {
    let server_path = zomboid_server_dir()?.join(format!("{server_id}.ini"));
    if !server_path.is_file() {
        return Err(format!(
            "{}: {}",
            text(
                "Server file not found",
                "Arquivo do servidor nao encontrado"
            ),
            server_path.display()
        ));
    }
    let game_build = normalize_game_build(&game_build)?;
    let content = read_text_lossy(&server_path)?;
    let mod_ids = read_ini_value(&content, "Mods")
        .map(|value| parse_server_mod_ids(&value))
        .unwrap_or_default();
    let updated_content = replace_or_append_ini_value(
        &content,
        "Mods",
        &serialize_server_mod_ids(&mod_ids, game_build),
    );
    fs::write(&server_path, updated_content)
        .map_err(|error| format!("Nao foi possivel salvar {}: {error}", server_path.display()))?;
    write_zomboid_server_build(&server_id, game_build)
}

#[tauri::command]
pub(crate) async fn update_zomboid_server_mods(
    server_id: String,
    mod_ids: Vec<String>,
    workshop_ids: Vec<String>,
) -> Result<(), String> {
    run_blocking(move || update_zomboid_server_mods_impl(&server_id, &mod_ids, &workshop_ids)).await
}

fn update_zomboid_server_mods_impl(
    server_id: &str,
    mod_ids: &[String],
    workshop_ids: &[String],
) -> Result<(), String> {
    let server_path = zomboid_server_dir()?.join(format!("{server_id}.ini"));

    if !server_path.exists() {
        return Err(format!(
            "{}: {}",
            text(
                "Server file not found",
                "Arquivo do servidor nao encontrado"
            ),
            server_path.display()
        ));
    }

    let content = read_text_lossy(&server_path)?;
    let game_build = read_zomboid_server_build(server_id)?;
    let normalized_mods = serialize_server_mod_ids(mod_ids, &game_build);
    let normalized_workshop_ids = normalize_server_values(workshop_ids).join(";");
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
            "{}: {}",
            text(
                "Server file not found",
                "Arquivo do servidor nao encontrado"
            ),
            server_path.display()
        ));
    }

    let map_names = find_mod_map_names(Path::new(&mod_path))?;

    if map_names.is_empty() {
        return Err(text(
            "This mod has no maps in media/maps.",
            "Este mod nao possui mapas em media/maps.",
        )
        .to_string());
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
    game_build: String,
    max_players: u32,
) -> Result<ZomboidServer, String> {
    run_blocking(move || {
        create_zomboid_server_impl(
            &app,
            &name,
            &mod_ids,
            &workshop_ids,
            &game_build,
            max_players,
        )
    })
    .await
}

fn create_zomboid_server_impl(
    app: &tauri::AppHandle,
    name: &str,
    mod_ids: &[String],
    workshop_ids: &[String],
    game_build: &str,
    max_players: u32,
) -> Result<ZomboidServer, String> {
    let name = name.trim();

    if name.is_empty() {
        return Err(text("Enter a server name.", "Informe um nome para o servidor.").to_string());
    }
    let max_players = validate_max_players(max_players)?;

    let server_id = sanitize_server_id(name);

    if server_id.is_empty() {
        return Err(text(
            "Use a server name with letters or numbers.",
            "Use um nome de servidor com letras ou numeros.",
        )
        .to_string());
    }

    let server_dir = zomboid_server_dir()?;
    fs::create_dir_all(&server_dir)
        .map_err(|error| format!("Nao foi possivel criar {}: {error}", server_dir.display()))?;

    let server_path = server_dir.join(format!("{server_id}.ini"));

    if server_path.exists() {
        return Err(format!(
            "{} '{server_id}'.",
            text(
                "A server already exists with the name",
                "Ja existe um servidor chamado"
            )
        ));
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

    let game_build = normalize_game_build(game_build)?;
    let normalized_mod_ids = serialize_server_mod_ids(mod_ids, game_build);
    let normalized_workshop_ids = resolve_server_workshop_ids(mod_ids, workshop_ids)?;
    let ini_content = read_text_lossy(&template_ini)?;
    let ini_content = replace_or_append_ini_value(&ini_content, "PublicName", name);
    let ini_content =
        replace_or_append_ini_value(&ini_content, "MaxPlayers", &max_players.to_string());
    let ini_content = replace_or_append_ini_value(&ini_content, "Mods", &normalized_mod_ids);
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
    write_zomboid_server_build(&server_id, game_build)?;

    read_zomboid_server_from_path(&server_path)
}

#[tauri::command]
pub(crate) async fn update_zomboid_server_settings(
    server_id: String,
    public_name: String,
    max_players: u32,
    default_port: String,
) -> Result<ZomboidServer, String> {
    run_blocking(move || {
        update_zomboid_server_settings_impl(&server_id, &public_name, max_players, &default_port)
    })
    .await
}

fn update_zomboid_server_settings_impl(
    server_id: &str,
    public_name: &str,
    max_players: u32,
    default_port: &str,
) -> Result<ZomboidServer, String> {
    let public_name = public_name.trim();
    if public_name.is_empty() {
        return Err(text("Enter a server name.", "Informe um nome para o servidor.").to_string());
    }

    let max_players = validate_max_players(max_players)?;
    let default_port = validate_port(default_port, "DefaultPort")?;
    let server_dir = zomboid_server_dir()?;
    let server_path = server_dir.join(format!("{server_id}.ini"));

    if !server_path.exists() || !server_path.is_file() {
        return Err(format!(
            "{}: {}",
            text(
                "Server file not found",
                "Arquivo do servidor nao encontrado"
            ),
            server_path.display()
        ));
    }

    let canonical_server_dir = server_dir.canonicalize().map_err(|error| {
        format!(
            "{} {}: {error}",
            text("Could not access", "Nao foi possivel acessar"),
            server_dir.display()
        )
    })?;
    let canonical_server_path = server_path.canonicalize().map_err(|error| {
        format!(
            "{} {}: {error}",
            text("Could not access", "Nao foi possivel acessar"),
            server_path.display()
        )
    })?;

    if !canonical_server_path.starts_with(&canonical_server_dir) {
        return Err(text("Invalid server file.", "Arquivo de servidor invalido.").to_string());
    }

    let content = read_text_lossy(&canonical_server_path)?;
    let content = replace_or_append_ini_value(&content, "PublicName", public_name);
    let content = replace_or_append_ini_value(&content, "MaxPlayers", &max_players.to_string());
    let content = replace_or_append_ini_value(&content, "DefaultPort", &default_port.to_string());

    fs::write(&canonical_server_path, content).map_err(|error| {
        format!(
            "Nao foi possivel salvar {}: {error}",
            canonical_server_path.display()
        )
    })?;

    read_zomboid_server_from_path(&canonical_server_path)
}

fn server_builds_path() -> Result<PathBuf, String> {
    Ok(app_config_dir()?.join("server-builds.ini"))
}

fn validate_max_players(max_players: u32) -> Result<u32, String> {
    if (1..=100).contains(&max_players) {
        return Ok(max_players);
    }

    Err(text(
        "Max players must be between 1 and 100.",
        "A quantidade de jogadores deve ficar entre 1 e 100.",
    )
    .to_string())
}

fn validate_port(value: &str, label: &str) -> Result<u16, String> {
    let port = value.trim().parse::<u16>().map_err(|_| {
        format!(
            "{} {label}.",
            text("Enter a valid port for", "Informe uma porta valida para")
        )
    })?;

    if port == 0 {
        return Err(format!(
            "{} {label}.",
            text("Enter a valid port for", "Informe uma porta valida para")
        ));
    }

    Ok(port)
}

fn normalize_game_build(game_build: &str) -> Result<&'static str, String> {
    match game_build.trim().to_lowercase().as_str() {
        BUILD_41 => Ok(BUILD_41),
        BUILD_42 => Ok(BUILD_42),
        _ => Err(text(
            "Invalid build. Use b41 or b42.",
            "Build invalida. Use b41 ou b42.",
        )
        .to_string()),
    }
}

fn read_server_builds() -> Result<HashMap<String, String>, String> {
    let path = server_builds_path()?;
    if !path.is_file() {
        return Ok(HashMap::new());
    }
    let content = read_text_lossy(&path)?;
    Ok(content
        .lines()
        .filter_map(|line| {
            let (server_id, game_build) = line.split_once('=')?;
            normalize_game_build(game_build)
                .ok()
                .map(|build| (server_id.trim().to_string(), build.to_string()))
        })
        .collect())
}

fn write_zomboid_server_build(server_id: &str, game_build: &str) -> Result<(), String> {
    let game_build = normalize_game_build(game_build)?;
    let path = server_builds_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Nao foi possivel criar {}: {error}", parent.display()))?;
    }
    let mut builds = read_server_builds()?;
    builds.insert(server_id.to_string(), game_build.to_string());
    let mut entries = builds.into_iter().collect::<Vec<_>>();
    entries.sort_by(|left, right| left.0.to_lowercase().cmp(&right.0.to_lowercase()));
    let content = entries
        .into_iter()
        .map(|(id, build)| format!("{id}={build}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&path, format!("{content}\n"))
        .map_err(|error| format!("Nao foi possivel salvar {}: {error}", path.display()))
}

fn remove_zomboid_server_build(server_id: &str) -> Result<(), String> {
    let path = server_builds_path()?;
    if !path.is_file() {
        return Ok(());
    }

    let mut builds = read_server_builds()?;
    if builds.remove(server_id).is_none() {
        return Ok(());
    }

    let mut entries = builds.into_iter().collect::<Vec<_>>();
    entries.sort_by(|left, right| left.0.to_lowercase().cmp(&right.0.to_lowercase()));
    let content = entries
        .into_iter()
        .map(|(id, build)| format!("{id}={build}"))
        .collect::<Vec<_>>()
        .join("\n");

    fs::write(
        &path,
        if content.is_empty() {
            String::new()
        } else {
            format!("{content}\n")
        },
    )
    .map_err(|error| format!("Nao foi possivel salvar {}: {error}", path.display()))
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
    use std::{
        env,
        sync::{Mutex, OnceLock},
        time::{SystemTime, UNIX_EPOCH},
    };

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn unique_temp_dir(name: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        env::temp_dir().join(format!("{name}-{timestamp}"))
    }

    fn configure_test_env(root: &Path) {
        env::set_var("LOCALAPPDATA", root.join("localappdata"));
        env::set_var("USERPROFILE", root.join("profile"));
    }

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

    #[test]
    fn defaults_profiles_without_metadata_to_b41() {
        let _guard = env_lock()
            .lock()
            .expect("test env lock should be available");
        let root = unique_temp_dir("pzmm-build-default-test");
        configure_test_env(&root);

        assert_eq!(
            read_zomboid_server_build("pzmm-profile-without-metadata-test").unwrap(),
            BUILD_41
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn delete_server_moves_profile_files_to_backup_and_removes_build_metadata() {
        let _guard = env_lock()
            .lock()
            .expect("test env lock should be available");
        let root = unique_temp_dir("pzmm-delete-server-test");
        configure_test_env(&root);
        let server_dir = zomboid_server_dir().expect("server dir should resolve");
        fs::create_dir_all(&server_dir).expect("server dir should be created");

        fs::write(server_dir.join("MyServer.ini"), "PublicName=My Server")
            .expect("server ini should be written");
        fs::write(
            server_dir.join("MyServer_SandboxVars.lua"),
            "SandboxVars = {}",
        )
        .expect("sandbox file should be written");
        fs::write(server_dir.join("MyServer_spawnregions.lua"), "return {}")
            .expect("spawnregions file should be written");
        fs::write(server_dir.join("MyServer_spawnpoints.lua"), "return {}")
            .expect("spawnpoints file should be written");
        fs::write(server_dir.join("MyServer_notes.txt"), "keep")
            .expect("unrelated extension should be written");
        fs::write(server_dir.join("OtherServer.ini"), "PublicName=Other")
            .expect("other server should be written");
        write_zomboid_server_build("MyServer", BUILD_42).expect("build metadata should be saved");

        let result = delete_zomboid_server_impl("MyServer").expect("server should be moved");
        let backup_dir = PathBuf::from(result.backup_path);

        assert!(!server_dir.join("MyServer.ini").exists());
        assert!(!server_dir.join("MyServer_SandboxVars.lua").exists());
        assert!(!server_dir.join("MyServer_spawnregions.lua").exists());
        assert!(!server_dir.join("MyServer_spawnpoints.lua").exists());
        assert!(server_dir.join("MyServer_notes.txt").exists());
        assert!(server_dir.join("OtherServer.ini").exists());
        assert!(backup_dir.join("MyServer.ini").exists());
        assert!(backup_dir.join("MyServer_SandboxVars.lua").exists());
        assert!(backup_dir.join("MyServer_spawnregions.lua").exists());
        assert!(backup_dir.join("MyServer_spawnpoints.lua").exists());
        assert!(!read_server_builds()
            .expect("build metadata should be readable")
            .contains_key("MyServer"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn delete_server_fails_when_ini_is_missing() {
        let _guard = env_lock()
            .lock()
            .expect("test env lock should be available");
        let root = unique_temp_dir("pzmm-delete-missing-server-test");
        configure_test_env(&root);
        fs::create_dir_all(zomboid_server_dir().expect("server dir should resolve"))
            .expect("server dir should be created");

        let error =
            delete_zomboid_server_impl("MissingServer").expect_err("missing server should fail");

        assert!(error.contains("Server file not found") || error.contains("Arquivo do servidor"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn update_server_settings_writes_ini_values() {
        let _guard = env_lock()
            .lock()
            .expect("test env lock should be available");
        let root = unique_temp_dir("pzmm-update-server-settings-test");
        configure_test_env(&root);
        let server_dir = zomboid_server_dir().expect("server dir should resolve");
        fs::create_dir_all(&server_dir).expect("server dir should be created");
        let server_path = server_dir.join("SettingsServer.ini");

        fs::write(
            &server_path,
            "PublicName=Old Name\nMaxPlayers=8\nDefaultPort=16261\n",
        )
        .expect("server ini should be written");

        let server = update_zomboid_server_settings_impl(
            "SettingsServer",
            "New Name",
            24,
            "16271",
        )
        .expect("server settings should be updated");
        let content = read_text_lossy(&server_path).expect("server ini should be readable");

        assert_eq!(server.name, "New Name");
        assert_eq!(server.max_players, 24);
        assert_eq!(server.port, "16271");
        assert!(content.contains("PublicName=New Name"));
        assert!(content.contains("MaxPlayers=24"));
        assert!(content.contains("DefaultPort=16271"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn update_server_settings_rejects_invalid_player_count() {
        let _guard = env_lock()
            .lock()
            .expect("test env lock should be available");
        let root = unique_temp_dir("pzmm-update-server-settings-invalid-test");
        configure_test_env(&root);
        let server_dir = zomboid_server_dir().expect("server dir should resolve");
        fs::create_dir_all(&server_dir).expect("server dir should be created");
        fs::write(server_dir.join("SettingsServer.ini"), "PublicName=Old Name\n")
            .expect("server ini should be written");

        let error = update_zomboid_server_settings_impl("SettingsServer", "New Name", 0, "16261")
            .expect_err("invalid player count should fail");

        assert!(error.contains("Max players") || error.contains("jogadores"));

        let _ = fs::remove_dir_all(root);
    }
}
