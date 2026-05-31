use serde::Serialize;

#[derive(Serialize)]
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
}

#[derive(Serialize)]
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
    pub(crate) badges: Vec<String>,
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
    pub(crate) line: Option<String>,
    pub(crate) result: Option<ServerTestResult>,
    pub(crate) error: Option<String>,
}
