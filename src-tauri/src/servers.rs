use crate::i18n::text;
use crate::models::{
    DeleteServerResult, ServerIniSettings, ServerLuaSetting, ServerLuaSettingOption,
    ServerLuaSettings, ZomboidServer, BUILD_41, BUILD_42,
};
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

pub(crate) fn list_zomboid_servers_impl() -> Result<Vec<ZomboidServer>, String> {
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

    servers.sort_by_key(|server| server.name.to_lowercase());

    Ok(servers)
}

#[tauri::command]
pub(crate) fn open_zomboid_server_file(server_id: String) -> Result<(), String> {
    open_file_external(&canonical_zomboid_server_path(&server_id)?)
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

#[tauri::command]
pub(crate) async fn get_zomboid_server_settings(
    server_id: String,
) -> Result<ServerIniSettings, String> {
    run_blocking(move || get_zomboid_server_settings_impl(&server_id)).await
}

pub(crate) fn get_zomboid_server_settings_impl(server_id: &str) -> Result<ServerIniSettings, String> {
    let server_path = canonical_zomboid_server_path(server_id)?;
    let content = read_text_lossy(&server_path)?;

    Ok(read_server_ini_settings(&content))
}

#[tauri::command]
pub(crate) async fn get_zomboid_server_lua_settings(
    server_id: String,
) -> Result<ServerLuaSettings, String> {
    run_blocking(move || get_zomboid_server_lua_settings_impl(&server_id)).await
}

pub(crate) fn get_zomboid_server_lua_settings_impl(
    server_id: &str,
) -> Result<ServerLuaSettings, String> {
    let sandbox_path = canonical_zomboid_server_sandbox_path(server_id)?;
    let file_name = sandbox_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("SandboxVars.lua")
        .to_string();
    let content = read_text_lossy(&sandbox_path)?;

    Ok(ServerLuaSettings {
        file_name,
        settings: read_server_lua_settings(&content),
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

pub(crate) fn update_zomboid_server_mods_impl(
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
        let example_dir = server_example_dir(&app)?;
        create_zomboid_server_from_template_impl(
            &example_dir,
            &name,
            &mod_ids,
            &workshop_ids,
            &game_build,
            max_players,
        )
    })
    .await
}

pub(crate) fn create_zomboid_server_from_template_impl(
    example_dir: &Path,
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
    settings: ServerIniSettings,
) -> Result<ZomboidServer, String> {
    run_blocking(move || update_zomboid_server_settings_impl(&server_id, &settings)).await
}

pub(crate) fn update_zomboid_server_settings_impl(
    server_id: &str,
    settings: &ServerIniSettings,
) -> Result<ZomboidServer, String> {
    let public_name = settings.public_name.trim();
    if public_name.is_empty() {
        return Err(text("Enter a server name.", "Informe um nome para o servidor.").to_string());
    }

    validate_server_ini_settings(settings)?;
    let canonical_server_path = canonical_zomboid_server_path(server_id)?;

    let content = read_text_lossy(&canonical_server_path)?;
    let content = write_server_ini_settings(&content, settings);

    fs::write(&canonical_server_path, content).map_err(|error| {
        format!(
            "Nao foi possivel salvar {}: {error}",
            canonical_server_path.display()
        )
    })?;

    read_zomboid_server_from_path(&canonical_server_path)
}

#[tauri::command]
pub(crate) async fn update_zomboid_server_lua_settings(
    server_id: String,
    settings: Vec<ServerLuaSetting>,
) -> Result<ServerLuaSettings, String> {
    run_blocking(move || update_zomboid_server_lua_settings_impl(&server_id, &settings)).await
}

pub(crate) fn update_zomboid_server_lua_settings_impl(
    server_id: &str,
    settings: &[ServerLuaSetting],
) -> Result<ServerLuaSettings, String> {
    let sandbox_path = canonical_zomboid_server_sandbox_path(server_id)?;
    let content = read_text_lossy(&sandbox_path)?;
    let content = write_server_lua_settings(&content, settings)?;

    fs::write(&sandbox_path, content).map_err(|error| {
        format!(
            "Nao foi possivel salvar {}: {error}",
            sandbox_path.display()
        )
    })?;

    get_zomboid_server_lua_settings_impl(server_id)
}

fn canonical_zomboid_server_path(server_id: &str) -> Result<PathBuf, String> {
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

    Ok(canonical_server_path)
}

fn canonical_zomboid_server_sandbox_path(server_id: &str) -> Result<PathBuf, String> {
    let server_dir = zomboid_server_dir()?;
    let sandbox_path = server_dir.join(format!("{server_id}_SandboxVars.lua"));

    if !sandbox_path.exists() || !sandbox_path.is_file() {
        return Err(format!(
            "{}: {}",
            text(
                "Server SandboxVars file not found",
                "Arquivo SandboxVars do servidor nao encontrado"
            ),
            sandbox_path.display()
        ));
    }

    let canonical_server_dir = server_dir.canonicalize().map_err(|error| {
        format!(
            "{} {}: {error}",
            text("Could not access", "Nao foi possivel acessar"),
            server_dir.display()
        )
    })?;
    let canonical_sandbox_path = sandbox_path.canonicalize().map_err(|error| {
        format!(
            "{} {}: {error}",
            text("Could not access", "Nao foi possivel acessar"),
            sandbox_path.display()
        )
    })?;

    if !canonical_sandbox_path.starts_with(&canonical_server_dir) {
        return Err(text("Invalid server file.", "Arquivo de servidor invalido.").to_string());
    }

    Ok(canonical_sandbox_path)
}

fn read_server_ini_settings(content: &str) -> ServerIniSettings {
    ServerIniSettings {
        public_name: read_ini_string(content, "PublicName", "My PZ Server"),
        public_description: read_ini_string(content, "PublicDescription", ""),
        password: read_ini_string(content, "Password", ""),
        max_players: read_ini_u32(content, "MaxPlayers", 32),
        default_port: read_ini_string(content, "DefaultPort", "16261"),
        udp_port: read_ini_string(content, "UDPPort", "16262"),
        is_public: read_ini_bool(content, "Public", false),
        is_open: read_ini_bool(content, "Open", true),
        pvp: read_ini_bool(content, "PVP", true),
        pause_empty: read_ini_bool(content, "PauseEmpty", true),
        global_chat: read_ini_bool(content, "GlobalChat", true),
        display_user_name: read_ini_bool(content, "DisplayUserName", true),
        safety_system: read_ini_bool(content, "SafetySystem", true),
        voice_enable: read_ini_bool(content, "VoiceEnable", true),
        steam_vac: read_ini_bool(content, "SteamVAC", true),
        upnp: read_ini_bool(content, "UPnP", true),
        ping_limit: read_ini_u32(content, "PingLimit", 400),
        save_world_every_minutes: read_ini_u32(content, "SaveWorldEveryMinutes", 0),
        hours_for_loot_respawn: read_ini_u32(content, "HoursForLootRespawn", 0),
        player_safehouse: read_ini_bool(content, "PlayerSafehouse", false),
        admin_safehouse: read_ini_bool(content, "AdminSafehouse", false),
        backups_count: read_ini_u32(content, "BackupsCount", 5),
        backups_on_start: read_ini_bool(content, "BackupsOnStart", true),
        backups_period: read_ini_u32(content, "BackupsPeriod", 0),
    }
}

fn write_server_ini_settings(content: &str, settings: &ServerIniSettings) -> String {
    let values = [
        ("PublicName", settings.public_name.trim().to_string()),
        (
            "PublicDescription",
            settings.public_description.trim().to_string(),
        ),
        ("Password", settings.password.trim().to_string()),
        ("MaxPlayers", settings.max_players.to_string()),
        ("DefaultPort", settings.default_port.trim().to_string()),
        ("UDPPort", settings.udp_port.trim().to_string()),
        ("Public", bool_ini_value(settings.is_public).to_string()),
        ("Open", bool_ini_value(settings.is_open).to_string()),
        ("PVP", bool_ini_value(settings.pvp).to_string()),
        (
            "PauseEmpty",
            bool_ini_value(settings.pause_empty).to_string(),
        ),
        (
            "GlobalChat",
            bool_ini_value(settings.global_chat).to_string(),
        ),
        (
            "DisplayUserName",
            bool_ini_value(settings.display_user_name).to_string(),
        ),
        (
            "SafetySystem",
            bool_ini_value(settings.safety_system).to_string(),
        ),
        (
            "VoiceEnable",
            bool_ini_value(settings.voice_enable).to_string(),
        ),
        ("SteamVAC", bool_ini_value(settings.steam_vac).to_string()),
        ("UPnP", bool_ini_value(settings.upnp).to_string()),
        ("PingLimit", settings.ping_limit.to_string()),
        (
            "SaveWorldEveryMinutes",
            settings.save_world_every_minutes.to_string(),
        ),
        (
            "HoursForLootRespawn",
            settings.hours_for_loot_respawn.to_string(),
        ),
        (
            "PlayerSafehouse",
            bool_ini_value(settings.player_safehouse).to_string(),
        ),
        (
            "AdminSafehouse",
            bool_ini_value(settings.admin_safehouse).to_string(),
        ),
        ("BackupsCount", settings.backups_count.to_string()),
        (
            "BackupsOnStart",
            bool_ini_value(settings.backups_on_start).to_string(),
        ),
        ("BackupsPeriod", settings.backups_period.to_string()),
    ];

    values
        .into_iter()
        .fold(content.to_string(), |current, (key, value)| {
            replace_or_append_ini_value(&current, key, &value)
        })
}

fn validate_server_ini_settings(settings: &ServerIniSettings) -> Result<(), String> {
    validate_max_players(settings.max_players)?;
    validate_port(&settings.default_port, "DefaultPort")?;
    validate_port(&settings.udp_port, "UDPPort")?;
    validate_range(settings.ping_limit, 100, u32::MAX, "PingLimit")?;
    validate_range(
        settings.save_world_every_minutes,
        0,
        u32::MAX,
        "SaveWorldEveryMinutes",
    )?;
    validate_range(
        settings.hours_for_loot_respawn,
        0,
        u32::MAX,
        "HoursForLootRespawn",
    )?;
    validate_range(settings.backups_count, 1, 300, "BackupsCount")?;
    validate_range(settings.backups_period, 0, 1500, "BackupsPeriod")?;

    Ok(())
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

fn validate_range(value: u32, min: u32, max: u32, label: &str) -> Result<u32, String> {
    if (min..=max).contains(&value) {
        return Ok(value);
    }

    Err(format!(
        "{} {label} ({}-{}).",
        text("Enter a valid value for", "Informe um valor valido para"),
        min,
        max
    ))
}

fn read_ini_string(content: &str, key: &str, fallback: &str) -> String {
    read_ini_value(content, key).unwrap_or_else(|| fallback.to_string())
}

fn read_ini_u32(content: &str, key: &str, fallback: u32) -> u32 {
    read_ini_value(content, key)
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(fallback)
}

fn read_ini_bool(content: &str, key: &str, fallback: bool) -> bool {
    read_ini_value(content, key)
        .and_then(|value| match value.trim().to_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Some(true),
            "false" | "0" | "no" | "off" => Some(false),
            _ => None,
        })
        .unwrap_or(fallback)
}

fn bool_ini_value(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

fn read_server_lua_settings(content: &str) -> Vec<ServerLuaSetting> {
    let mut settings = Vec::new();
    let mut sections = Vec::new();
    let mut comments = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        if let Some(comment) = trimmed.strip_prefix("--") {
            comments.push(comment.trim().to_string());
            continue;
        }

        if trimmed.is_empty() {
            continue;
        }

        if is_lua_table_close(trimmed) {
            sections.pop();
            comments.clear();
            continue;
        }

        let Some((key, value)) = parse_lua_assignment(trimmed) else {
            comments.clear();
            continue;
        };

        if value == "{" {
            if !(sections.is_empty() && key == "SandboxVars") {
                sections.push(key.to_string());
            }
            comments.clear();
            continue;
        }

        let Some((value_kind, parsed_value)) = parse_lua_setting_value(value) else {
            comments.clear();
            continue;
        };
        let section = if sections.is_empty() {
            "SandboxVars".to_string()
        } else {
            sections.join(".")
        };
        let path = if sections.is_empty() {
            key.to_string()
        } else {
            format!("{}.{}", sections.join("."), key)
        };

        settings.push(ServerLuaSetting {
            path,
            key: key.to_string(),
            section,
            value: parsed_value,
            value_kind: value_kind.to_string(),
            default_value: extract_lua_default_value(&comments),
            options: extract_lua_setting_options(&comments),
        });
        comments.clear();
    }

    settings
}

fn extract_lua_setting_options(comments: &[String]) -> Vec<ServerLuaSettingOption> {
    comments
        .iter()
        .filter_map(|comment| {
            let (value, label) = comment.split_once('=')?;
            let value = value.trim();
            let label = label.trim();

            if value.is_empty()
                || label.is_empty()
                || !value
                    .chars()
                    .all(|char| char.is_ascii_digit() || char == '-' || char == '.')
                || value.parse::<f64>().is_err()
            {
                return None;
            }

            Some(ServerLuaSettingOption {
                value: value.to_string(),
                label: label.to_string(),
            })
        })
        .collect()
}

fn extract_lua_default_value(comments: &[String]) -> Option<String> {
    comments.iter().find_map(|comment| {
        let marker_start = comment.find("Padr")?;
        let marker = &comment[marker_start..];
        let (_, value) = marker.split_once('=')?;
        let value = value.trim();

        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    })
}

fn write_server_lua_settings(
    content: &str,
    settings: &[ServerLuaSetting],
) -> Result<String, String> {
    let values = settings
        .iter()
        .map(|setting| {
            let value = format_lua_setting_value(setting)?;
            Ok((setting.path.clone(), value))
        })
        .collect::<Result<HashMap<_, _>, String>>()?;
    let mut sections = Vec::new();
    let mut output = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        if is_lua_table_close(trimmed) {
            sections.pop();
            output.push(line.to_string());
            continue;
        }

        let Some((key, value)) = parse_lua_assignment(trimmed) else {
            output.push(line.to_string());
            continue;
        };

        if value == "{" {
            if !(sections.is_empty() && key == "SandboxVars") {
                sections.push(key.to_string());
            }
            output.push(line.to_string());
            continue;
        }

        let path = if sections.is_empty() {
            key.to_string()
        } else {
            format!("{}.{}", sections.join("."), key)
        };

        if let Some(next_value) = values.get(&path) {
            let indent = line
                .chars()
                .take_while(|char| char.is_whitespace())
                .collect::<String>();
            let comma = if trimmed.ends_with(',') { "," } else { "" };
            output.push(format!("{indent}{key} = {next_value}{comma}"));
        } else {
            output.push(line.to_string());
        }
    }

    let trailing_newline = if content.ends_with('\n') { "\n" } else { "" };
    Ok(format!("{}{}", output.join("\n"), trailing_newline))
}

fn parse_lua_assignment(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once('=')?;
    let key = key.trim();

    if key.is_empty()
        || !key
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || char == '_')
    {
        return None;
    }

    let value = value.trim().trim_end_matches(',').trim();
    Some((key, value))
}

fn parse_lua_setting_value(value: &str) -> Option<(&'static str, String)> {
    if value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("false") {
        return Some(("boolean", value.to_ascii_lowercase()));
    }

    if value.parse::<f64>().is_ok() {
        return Some(("number", value.to_string()));
    }

    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        return Some(("string", value[1..value.len() - 1].replace("\\\"", "\"")));
    }

    None
}

fn format_lua_setting_value(setting: &ServerLuaSetting) -> Result<String, String> {
    match setting.value_kind.as_str() {
        "boolean" => match setting.value.trim().to_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Ok("true".to_string()),
            "false" | "0" | "no" | "off" => Ok("false".to_string()),
            _ => Err(format!("Valor booleano invalido para {}.", setting.path)),
        },
        "number" => {
            let value = setting.value.trim();
            value
                .parse::<f64>()
                .map(|_| value.to_string())
                .map_err(|_| format!("Valor numerico invalido para {}.", setting.path))
        }
        "string" => Ok(format!("\"{}\"", setting.value.replace('\\', "\\\\").replace('"', "\\\""))),
        _ => Err(format!("Tipo de valor invalido para {}.", setting.path)),
    }
}

fn is_lua_table_close(line: &str) -> bool {
    line == "}" || line == "},"
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
    entries.sort_by_key(|entry| entry.0.to_lowercase());
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
    entries.sort_by_key(|entry| entry.0.to_lowercase());
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

        let mut settings = get_zomboid_server_settings_impl("SettingsServer")
            .expect("server settings should be readable");
        settings.public_name = "New Name".to_string();
        settings.max_players = 24;
        settings.default_port = "16271".to_string();
        settings.udp_port = "16272".to_string();
        settings.pvp = false;
        settings.backups_count = 7;

        let server = update_zomboid_server_settings_impl("SettingsServer", &settings)
            .expect("server settings should be updated");
        let content = read_text_lossy(&server_path).expect("server ini should be readable");

        assert_eq!(server.name, "New Name");
        assert_eq!(server.max_players, 24);
        assert_eq!(server.port, "16271");
        assert!(content.contains("PublicName=New Name"));
        assert!(content.contains("MaxPlayers=24"));
        assert!(content.contains("DefaultPort=16271"));
        assert!(content.contains("UDPPort=16272"));
        assert!(content.contains("PVP=false"));
        assert!(content.contains("BackupsCount=7"));

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
        fs::write(
            server_dir.join("SettingsServer.ini"),
            "PublicName=Old Name\n",
        )
        .expect("server ini should be written");

        let mut settings = read_server_ini_settings("PublicName=Old Name\n");
        settings.public_name = "New Name".to_string();
        settings.max_players = 0;

        let error = update_zomboid_server_settings_impl("SettingsServer", &settings)
            .expect_err("invalid player count should fail");

        assert!(error.contains("Max players") || error.contains("jogadores"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reads_server_settings_from_ini_values() {
        let content = concat!(
            "PublicName=Read Me\n",
            "PublicDescription=Visible server\n",
            "Password=hunter2\n",
            "MaxPlayers=12\n",
            "DefaultPort=16261\n",
            "UDPPort=16262\n",
            "Public=true\n",
            "Open=false\n",
            "PVP=false\n",
            "PauseEmpty=false\n",
            "BackupsCount=9\n",
        );

        let settings = read_server_ini_settings(content);

        assert_eq!(settings.public_name, "Read Me");
        assert_eq!(settings.public_description, "Visible server");
        assert_eq!(settings.password, "hunter2");
        assert_eq!(settings.max_players, 12);
        assert!(settings.is_public);
        assert!(!settings.is_open);
        assert!(!settings.pvp);
        assert!(!settings.pause_empty);
        assert_eq!(settings.backups_count, 9);
    }

    #[test]
    fn reads_nested_lua_settings_from_sandbox_vars() {
        let content = concat!(
            "SandboxVars = {\n",
            "    -- Padrao=Normal\n",
            "    -- 1 = Insano\n",
            "    -- 4 = Normal\n",
            "    Zombies = 4,\n",
            "    StarterKit = false,\n",
            "    WorldItemRemovalList = \"Base.Hat,Base.Glasses\",\n",
            "    Map = {\n",
            "        AllowWorldMap = true,\n",
            "    },\n",
            "    ZombieConfig = {\n",
            "        -- Minimo = 0,00 Maximo = 4,00 Padrao = 1,00\n",
            "        PopulationMultiplier = 1.5,\n",
            "    },\n",
            "}\n",
        );

        let settings = read_server_lua_settings(content);

        assert!(settings.iter().any(|setting| {
            setting.path == "Zombies"
                && setting.value == "4"
                && setting.value_kind == "number"
                && setting.section == "SandboxVars"
                && setting.default_value.as_deref() == Some("Normal")
                && setting.options
                    == vec![
                        ServerLuaSettingOption {
                            value: "1".to_string(),
                            label: "Insano".to_string(),
                        },
                        ServerLuaSettingOption {
                            value: "4".to_string(),
                            label: "Normal".to_string(),
                        },
                    ]
        }));
        assert!(settings.iter().any(|setting| {
            setting.path == "StarterKit"
                && setting.value == "false"
                && setting.value_kind == "boolean"
        }));
        assert!(settings.iter().any(|setting| {
            setting.path == "WorldItemRemovalList"
                && setting.value == "Base.Hat,Base.Glasses"
                && setting.value_kind == "string"
        }));
        assert!(settings.iter().any(|setting| {
            setting.path == "Map.AllowWorldMap"
                && setting.section == "Map"
                && setting.value == "true"
        }));
        assert!(settings.iter().any(|setting| {
            setting.path == "ZombieConfig.PopulationMultiplier"
                && setting.section == "ZombieConfig"
                && setting.value == "1.5"
                && setting.default_value.as_deref() == Some("1,00")
                && setting.options.is_empty()
        }));
    }

    #[test]
    fn reads_lua_options_and_defaults_for_nested_values() {
        let content = concat!(
            "SandboxVars = {\n",
            "    ZombieLore = {\n",
            "        -- Controla a movimentacao do zumbi. Padrao=Normal\n",
            "        -- 1 = Corredores (Sprinters)\n",
            "        -- 2 = Normal\n",
            "        -- 3 = Lento\n",
            "        Speed = 2,\n",
            "    },\n",
            "}\n",
        );

        let settings = read_server_lua_settings(content);
        let speed = settings
            .iter()
            .find(|setting| setting.path == "ZombieLore.Speed")
            .expect("nested setting should be parsed");

        assert_eq!(speed.default_value.as_deref(), Some("Normal"));
        assert_eq!(
            speed.options,
            vec![
                ServerLuaSettingOption {
                    value: "1".to_string(),
                    label: "Corredores (Sprinters)".to_string(),
                },
                ServerLuaSettingOption {
                    value: "2".to_string(),
                    label: "Normal".to_string(),
                },
                ServerLuaSettingOption {
                    value: "3".to_string(),
                    label: "Lento".to_string(),
                },
            ]
        );
    }

    #[test]
    fn writes_lua_settings_without_reordering_file() {
        let content = concat!(
            "SandboxVars = {\n",
            "    Zombies = 4,\n",
            "    StarterKit = false,\n",
            "    Map = {\n",
            "        AllowWorldMap = true,\n",
            "    },\n",
            "}\n",
        );
        let mut settings = read_server_lua_settings(content);

        for setting in &mut settings {
            match setting.path.as_str() {
                "Zombies" => setting.value = "5".to_string(),
                "StarterKit" => setting.value = "true".to_string(),
                "Map.AllowWorldMap" => setting.value = "false".to_string(),
                _ => {}
            }
        }

        let updated =
            write_server_lua_settings(content, &settings).expect("lua settings should be written");

        assert!(updated.contains("    Zombies = 5,"));
        assert!(updated.contains("    StarterKit = true,"));
        assert!(updated.contains("        AllowWorldMap = false,"));
        assert!(updated.ends_with('\n'));
    }
}
