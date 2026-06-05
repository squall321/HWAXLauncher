//! `config.json` — the agent's non-secret settings (v2 plan §26.4).
//! The device JWT and refresh token are **never** here — they live in the
//! Windows Credential Manager (see the `auth` module in the Tauri shell).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// HEAXHub base URL (set at pairing time).
    pub server: String,
    /// Paired agent id (set at pairing time). JWT plaintext is NOT stored here.
    pub agent_id: String,
    #[serde(default = "d_true")]
    pub auto_update: bool,
    #[serde(default)]
    pub start_on_boot: bool,
    #[serde(default = "d_log_level")]
    pub log_level: String,
    /// Download allow-list. Empty ⇒ `[server]` (see [`Self::effective_allowed_origins`]).
    #[serde(default)]
    pub allowed_origins: Vec<String>,
    #[serde(default = "d_keep")]
    pub keep_last_n_versions: u32,
    #[serde(default = "d_sync")]
    pub sync_interval_min: u32,
    #[serde(default = "d_channel")]
    pub channel: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy: Option<String>,
    #[serde(default)]
    pub telemetry_anonymous: bool,
}

fn d_true() -> bool {
    true
}
fn d_log_level() -> String {
    "info".into()
}
fn d_keep() -> u32 {
    3
}
fn d_sync() -> u32 {
    30
}
fn d_channel() -> String {
    "stable".into()
}

impl AgentConfig {
    /// Fresh config after pairing. `allowed_origins` defaults to the server
    /// origin only — the launcher never downloads from anywhere else.
    pub fn new(server: impl Into<String>, agent_id: impl Into<String>) -> Self {
        let server = server.into();
        Self {
            allowed_origins: vec![server.clone()],
            server,
            agent_id: agent_id.into(),
            auto_update: true,
            start_on_boot: false,
            log_level: d_log_level(),
            keep_last_n_versions: d_keep(),
            sync_interval_min: d_sync(),
            channel: d_channel(),
            proxy: None,
            telemetry_anonymous: false,
        }
    }

    /// The origins downloads are allowed from. Falls back to `[server]` when
    /// `allowed_origins` is empty so a hand-edited config can never widen the
    /// surface to "anywhere".
    pub fn effective_allowed_origins(&self) -> Vec<String> {
        if self.allowed_origins.is_empty() {
            vec![self.server.clone()]
        } else {
            self.allowed_origins.clone()
        }
    }
}
