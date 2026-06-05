//! Mirror of `contracts/hwax-agent/audit-event.schema.json`
//! (`POST /api/v1/launcher-agents/audit`).
//!
//! `kind` classifies the event and is disjoint from
//! [`crate::report::InstallStatus`]. `payload` is free-form, kind-specific
//! context the hub stores as JSONB without interpreting.

use crate::time::rfc3339;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditKind {
    Enrollment,
    /// "an install was attempted/observed"; the granular outcome lives in
    /// `install-report.status`. Use `payload.outcome` for success|failed|partial.
    Install,
    Uninstall,
    Run,
    Stop,
    Rollback,
    AvBlock,
    Sha256Mismatch,
    DownloadFailed,
    PolicyDenied,
    Heartbeat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClientMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
}

impl ClientMeta {
    /// The common Windows shape used throughout the e2e example.
    pub fn windows(
        os_version: impl Into<String>,
        agent_version: impl Into<String>,
        hostname: impl Into<String>,
    ) -> Self {
        Self {
            os: Some("windows".into()),
            os_version: Some(os_version.into()),
            agent_version: Some(agent_version.into()),
            hostname: Some(hostname.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub agent_id: String,
    pub kind: AuditKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub occurred_at: String,
    pub severity: Severity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_meta: Option<ClientMeta>,
}

impl AuditEvent {
    pub fn new(
        agent_id: impl Into<String>,
        kind: AuditKind,
        occurred_at: DateTime<Utc>,
        severity: Severity,
    ) -> Self {
        Self {
            agent_id: agent_id.into(),
            kind,
            app_id: None,
            version: None,
            occurred_at: rfc3339(occurred_at),
            severity,
            payload: None,
            client_meta: None,
        }
    }

    pub fn app(mut self, app_id: impl Into<String>, version: impl Into<String>) -> Self {
        self.app_id = Some(app_id.into());
        self.version = Some(version.into());
        self
    }

    pub fn app_id(mut self, app_id: impl Into<String>) -> Self {
        self.app_id = Some(app_id.into());
        self
    }

    /// Set the free-form payload. The schema requires `payload` to be an
    /// object; non-object values are wrapped under a `value` key so a careless
    /// caller cannot produce a schema-invalid event.
    pub fn payload(mut self, payload: Value) -> Self {
        self.payload = Some(match payload {
            Value::Object(_) => payload,
            other => {
                let mut m = Map::new();
                m.insert("value".to_string(), other);
                Value::Object(m)
            }
        });
        self
    }

    pub fn client_meta(mut self, meta: ClientMeta) -> Self {
        self.client_meta = Some(meta);
        self
    }
}
