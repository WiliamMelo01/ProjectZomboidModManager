use serde::{Deserialize, Serialize};

pub(crate) const BUILD_41: &str = "b41";
pub(crate) const BUILD_42: &str = "b42";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ZomboidServer {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) file_name: String,
    pub(crate) path: String,
    pub(crate) port: String,
    pub(crate) max_players: u32,
    pub(crate) mods_count: usize,
    pub(crate) active_mod_ids: Vec<String>,
    pub(crate) status: String,
    pub(crate) game_build: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeleteServerResult {
    pub(crate) backup_path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerIniSettings {
    pub(crate) public_name: String,
    pub(crate) public_description: String,
    pub(crate) password: String,
    pub(crate) max_players: u32,
    pub(crate) default_port: String,
    pub(crate) udp_port: String,
    pub(crate) is_public: bool,
    pub(crate) is_open: bool,
    pub(crate) pvp: bool,
    pub(crate) pause_empty: bool,
    pub(crate) global_chat: bool,
    pub(crate) display_user_name: bool,
    pub(crate) safety_system: bool,
    pub(crate) voice_enable: bool,
    pub(crate) steam_vac: bool,
    pub(crate) upnp: bool,
    pub(crate) ping_limit: u32,
    pub(crate) save_world_every_minutes: u32,
    pub(crate) hours_for_loot_respawn: u32,
    pub(crate) player_safehouse: bool,
    pub(crate) admin_safehouse: bool,
    pub(crate) backups_count: u32,
    pub(crate) backups_on_start: bool,
    pub(crate) backups_period: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerLuaSetting {
    pub(crate) path: String,
    pub(crate) key: String,
    pub(crate) section: String,
    pub(crate) value: String,
    pub(crate) value_kind: String,
    pub(crate) default_value: Option<String>,
    pub(crate) options: Vec<ServerLuaSettingOption>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerLuaSettingOption {
    pub(crate) value: String,
    pub(crate) label: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerLuaSettings {
    pub(crate) file_name: String,
    pub(crate) settings: Vec<ServerLuaSetting>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ZomboidMod {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) author: String,
    pub(crate) version: String,
    pub(crate) workshop_id: String,
    pub(crate) description: String,
    pub(crate) size: String,
    pub(crate) is_installed: bool,
    pub(crate) source: String,
    pub(crate) path: String,
    pub(crate) image_url: Option<String>,
    pub(crate) dependencies: Vec<String>,
    pub(crate) map_names: Vec<String>,
    pub(crate) compatible_builds: Vec<String>,
    pub(crate) variants: Vec<ZomboidModVariant>,
    pub(crate) package_path: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ZomboidModVariant {
    pub(crate) game_build: String,
    pub(crate) id: String,
    pub(crate) path: String,
    pub(crate) dependencies: Vec<String>,
    pub(crate) map_names: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppSettings {
    pub(crate) steamcmd_path: String,
    pub(crate) resolved_steamcmd_path: Option<String>,
    pub(crate) is_steamcmd_configured: bool,
    pub(crate) game_executable_path: String,
    pub(crate) client_ram: String,
    pub(crate) server_ram: String,
    pub(crate) language_preference: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkshopDownloadResult {
    pub(crate) total_items: usize,
    pub(crate) downloaded_items: usize,
    pub(crate) failed_items: Vec<WorkshopDownloadFailedItem>,
    pub(crate) cancelled_items: usize,
    pub(crate) was_cancelled: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkshopDownloadFailedItem {
    pub(crate) workshop_id: String,
    pub(crate) name: String,
    pub(crate) error: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkshopDownloadEvent {
    pub(crate) workshop_id: String,
    pub(crate) name: String,
    pub(crate) status: String,
    pub(crate) error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModLocation {
    pub(crate) label: String,
    pub(crate) path: String,
    pub(crate) kind: String,
    pub(crate) exists: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ZomboidInstallationStatus {
    pub(crate) default_game_dir: String,
    pub(crate) detected_executable_path: Option<String>,
    pub(crate) is_game_dir_found: bool,
    pub(crate) is_executable_found: bool,
    pub(crate) is_client_config_found: bool,
    pub(crate) is_server_config_found: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerTestResult {
    pub(crate) status: String,
    pub(crate) summary: String,
    pub(crate) duration_seconds: u64,
    pub(crate) bat_path: String,
    pub(crate) command: String,
    pub(crate) warning_count: usize,
    pub(crate) critical_count: usize,
    pub(crate) log_lines: Vec<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerTestStarted {
    pub(crate) server_id: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PortUsage {
    pub(crate) port: u16,
    pub(crate) protocol: String,
    pub(crate) pid: u32,
    pub(crate) process_name: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerPortCheck {
    pub(crate) ports: Vec<u16>,
    pub(crate) usages: Vec<PortUsage>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerTestEvent {
    pub(crate) server_id: String,
    pub(crate) event: String,
    pub(crate) timeout_seconds: Option<u64>,
    pub(crate) line: Option<String>,
    pub(crate) result: Option<ServerTestResult>,
    pub(crate) error: Option<String>,
}
