//! IPC view-models — the exact shapes that cross the Tauri `invoke` boundary
//! (the command catalog in the workstream brief). Field names are snake_case so
//! they reach the TypeScript side unchanged. These are *view* types assembled
//! from `hwax_core` models + on-disk state; they are never the wire format.

use hwax_core::state::ModuleState;
use serde::{Deserialize, Serialize};

/// `agent_status()` result.
#[derive(Debug, Clone, Serialize)]
pub struct AgentStatus {
    pub agent_id: Option<String>,
    pub server: Option<String>,
    pub paired: bool,
    pub last_sync: Option<String>,
    pub module_count: u32,
    pub error_count: u32,
    /// 'green' | 'yellow' | 'red'
    pub status_color: String,
}

/// `start_pairing()` result.
#[derive(Debug, Clone, Serialize)]
pub struct PairingInfo {
    pub url: String,
    pub code: String,
}

/// One row in `list_modules()` / `sync_manifest().modules`.
#[derive(Debug, Clone, Serialize)]
pub struct ModuleView {
    pub id: String,
    pub name: String,
    pub current_version: Option<String>,
    pub latest_version: Option<String>,
    pub state: ModuleState,
    pub show_in_tray: bool,
    pub color_accent: Option<String>,
    pub category: Option<String>,
}

/// `sync_manifest()` result.
#[derive(Debug, Clone, Serialize)]
pub struct SyncResult {
    pub changed: bool,
    pub modules: Vec<ModuleView>,
}

/// One entry in `ModuleDetail.history`.
#[derive(Debug, Clone, Serialize)]
pub struct HistoryEntry {
    pub version: String,
    pub installed_at: String,
}

/// `module_detail()` result.
#[derive(Debug, Clone, Serialize)]
pub struct ModuleDetail {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub current_version: Option<String>,
    pub latest_version: Option<String>,
    pub state: ModuleState,
    pub history: Vec<HistoryEntry>,
    pub requires_admin: bool,
}

/// `run_module()` result.
#[derive(Debug, Clone, Serialize)]
pub struct RunHandle {
    pub pid: u32,
    pub id: String,
}

/// `health_check()` result.
#[derive(Debug, Clone, Serialize)]
pub struct HealthReport {
    pub server_reachable: bool,
    pub disk_free_bytes: u64,
    pub write_ok: bool,
}

/// `update_config({ patch })` — a partial of `AgentConfig`. Only the
/// user-tunable, non-secret fields are patchable; `server` and `agent_id` are
/// intentionally NOT here (they change only via re-pairing — v2 §4.4/§15 ⑧).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ConfigPatch {
    pub auto_update: Option<bool>,
    pub start_on_boot: Option<bool>,
    pub log_level: Option<String>,
    pub keep_last_n_versions: Option<u32>,
    pub sync_interval_min: Option<u32>,
    pub channel: Option<String>,
    pub proxy: Option<Option<String>>,
    pub telemetry_anonymous: Option<bool>,
}
