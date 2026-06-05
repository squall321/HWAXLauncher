//! Process-wide application state, `manage()`d by Tauri so every
//! `#[tauri::command]` can pull it from `State<'_, AppState>`.
//!
//! It holds the live [`AgentConfig`] behind an `RwLock` (config is read on every
//! sync/download and written by `update_config`), one shared `reqwest::Client`
//! (connection pooling + a single TLS context), the resolved [`Paths`], and the
//! small counters the tray status dot is derived from.

use crate::paths::Paths;
use hwax_core::config::AgentConfig;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;

/// Agent version baked in at build time — reported in heartbeats, audit
/// `client_meta`, and the install/run logs.
pub const AGENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Tray status color, derived from `error_count` and reachability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusColor {
    Green,
    Yellow,
    Red,
}

impl StatusColor {
    /// The string the `AgentStatus.status_color` IPC field carries.
    pub fn as_str(self) -> &'static str {
        match self {
            StatusColor::Green => "green",
            StatusColor::Yellow => "yellow",
            StatusColor::Red => "red",
        }
    }
}

/// Shared, thread-safe agent state. Cheap fields are atomics; the config is an
/// `RwLock` because it is read far more often than written.
pub struct AppState {
    /// Live config (mirrors `config.json`). Writers must also persist via
    /// [`crate::config_store::save`].
    pub config: RwLock<AgentConfig>,
    /// One pooled HTTPS client for the whole process.
    pub http: reqwest::Client,
    /// Resolved on-disk layout.
    pub paths: Paths,
    /// `last_sync` as an RFC3339 string, or empty if never synced.
    pub last_sync: RwLock<Option<String>>,
    /// Count of errors since start (drives the yellow/red dot).
    pub error_count: AtomicU64,
    /// Consecutive manifest-sync failures (5 ⇒ yellow, per §13).
    pub consecutive_sync_failures: AtomicU64,
    /// Last known server reachability (set by sync/health_check).
    pub server_reachable: std::sync::atomic::AtomicBool,
}

impl AppState {
    pub fn new(config: AgentConfig, http: reqwest::Client, paths: Paths) -> Self {
        Self {
            config: RwLock::new(config),
            http,
            paths,
            last_sync: RwLock::new(None),
            error_count: AtomicU64::new(0),
            consecutive_sync_failures: AtomicU64::new(0),
            server_reachable: std::sync::atomic::AtomicBool::new(true),
        }
    }

    /// Snapshot the config under a read lock. Cloned because callers do async IO
    /// while holding the value; never hold the lock across `.await`.
    pub fn config_snapshot(&self) -> AgentConfig {
        self.config.read().expect("config lock poisoned").clone()
    }

    /// Whether pairing has happened: a non-empty `agent_id` in the config plus a
    /// device JWT in the credential store (the latter is checked by `auth`).
    pub fn is_paired(&self) -> bool {
        !self
            .config
            .read()
            .expect("config lock poisoned")
            .agent_id
            .is_empty()
    }

    pub fn bump_error(&self) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn error_count(&self) -> u64 {
        self.error_count.load(Ordering::Relaxed)
    }

    /// Compute the tray dot color from current counters/reachability (§4.1/§13).
    pub fn status_color(&self) -> StatusColor {
        if self.error_count() > 0 {
            return StatusColor::Red;
        }
        let unreachable = !self.server_reachable.load(Ordering::Relaxed);
        let many_fails = self.consecutive_sync_failures.load(Ordering::Relaxed) >= 5;
        if unreachable || many_fails {
            StatusColor::Yellow
        } else {
            StatusColor::Green
        }
    }

    pub fn set_last_sync(&self, ts: String) {
        *self.last_sync.write().expect("last_sync lock poisoned") = Some(ts);
    }

    pub fn last_sync(&self) -> Option<String> {
        self.last_sync
            .read()
            .expect("last_sync lock poisoned")
            .clone()
    }
}
