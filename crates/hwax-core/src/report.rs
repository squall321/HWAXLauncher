//! Mirror of `contracts/hwax-agent/install-report.schema.json`
//! (`POST /api/v1/launcher-agents/installs`).
//!
//! `status` is intentionally **disjoint** from [`crate::audit::AuditKind`]:
//! status is the terminal outcome of one install attempt; kind classifies an
//! event. The builder enforces the schema `maxLength` caps so the agent can
//! never emit an over-length payload.

use crate::time::rfc3339;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

const ERROR_MAX: usize = 2048;
const LOG_EXCERPT_MAX: usize = 16384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallStatus {
    Success,
    Failed,
    /// User-driven revert to a prior version (only after a swap completed).
    RolledBack,
    /// Installer wrote partial bytes (e.g. ENOSPC mid-download).
    Partial,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallReport {
    /// `WindowsAgent.id` (UUID) — must match the `sub` of the access token.
    pub agent_id: String,
    pub app_id: String,
    pub version: String,
    pub status: InstallStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    pub started_at: String,
    pub finished_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256_verified: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_excerpt: Option<String>,
    /// Version rolled back TO (only meaningful when `status=rolled_back`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_version: Option<String>,
}

impl InstallReport {
    pub fn new(
        agent_id: impl Into<String>,
        app_id: impl Into<String>,
        version: impl Into<String>,
        status: InstallStatus,
        started_at: DateTime<Utc>,
        finished_at: DateTime<Utc>,
    ) -> Self {
        Self {
            agent_id: agent_id.into(),
            app_id: app_id.into(),
            version: version.into(),
            status,
            exit_code: None,
            started_at: rfc3339(started_at),
            finished_at: rfc3339(finished_at),
            sha256_verified: None,
            error: None,
            log_excerpt: None,
            previous_version: None,
        }
    }

    pub fn exit_code(mut self, code: i32) -> Self {
        self.exit_code = Some(code);
        self
    }

    pub fn sha256_verified(mut self, v: bool) -> Self {
        self.sha256_verified = Some(v);
        self
    }

    pub fn error(mut self, e: impl Into<String>) -> Self {
        self.error = Some(truncate_chars(e.into(), ERROR_MAX));
        self
    }

    pub fn log_excerpt(mut self, l: impl Into<String>) -> Self {
        // The hub truncates above 16 KiB; we truncate first so the payload is
        // schema-valid before it ever leaves the machine. Keep the *tail*,
        // which is where the useful failure context lives.
        self.log_excerpt = Some(keep_tail_chars(l.into(), LOG_EXCERPT_MAX));
        self
    }

    pub fn previous_version(mut self, v: impl Into<String>) -> Self {
        self.previous_version = Some(v.into());
        self
    }
}

fn truncate_chars(s: String, max: usize) -> String {
    if s.chars().count() <= max {
        s
    } else {
        s.chars().take(max).collect()
    }
}

fn keep_tail_chars(s: String, max: usize) -> String {
    let count = s.chars().count();
    if count <= max {
        s
    } else {
        s.chars().skip(count - max).collect()
    }
}
