//! Audit / install-report telemetry (v2 plan §19, e2e §6–§8).
//!
//! Two channels, both posting `hwax_core`-built, schema-faithful payloads:
//!   - **install reports** → `POST …/installs` (one per install attempt).
//!   - **audit events**    → `POST …/audit` (one per event). Batched every
//!     5 minutes, with *immediate* send for failures / rollback / AV / sha
//!     mismatch / policy-denied (the security-relevant kinds).
//!
//! A bounded in-memory queue buffers events so a transient server outage never
//! blocks an install; the background flusher drains it. Loss on crash is
//! acceptable for audit (the install report is the durable record).

use crate::state::AppState;
use anyhow::Result;
use hwax_core::audit::{AuditEvent, AuditKind, ClientMeta};
use hwax_core::report::InstallReport;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Manager};

/// How often the background audit flusher runs (v2 §19: 5-minute batch).
pub const BATCH_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// Process-wide audit queue. Small and lock-guarded; audit volume is low.
#[derive(Default)]
pub struct AuditQueue {
    pending: Mutex<Vec<AuditEvent>>,
}

impl AuditQueue {
    pub fn new() -> Self {
        Self::default()
    }

    fn push(&self, event: AuditEvent) {
        let mut q = self.pending.lock().expect("audit queue poisoned");
        // Cap the buffer so a long outage cannot grow it unboundedly.
        if q.len() >= 512 {
            q.remove(0);
        }
        q.push(event);
    }

    fn drain(&self) -> Vec<AuditEvent> {
        std::mem::take(&mut *self.pending.lock().expect("audit queue poisoned"))
    }
}

/// `client_meta` for outgoing payloads. Best-effort hostname/os-version.
pub fn client_meta() -> ClientMeta {
    let hostname = hostname();
    ClientMeta::windows(os_version(), crate::state::AGENT_VERSION, hostname)
}

/// Best-effort machine hostname (used in heartbeat + audit client_meta).
pub fn hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown".into())
}

/// Best-effort Windows version string. We avoid pulling a winapi just for this;
/// the hub only logs it.
fn os_version() -> String {
    std::env::var("OS").unwrap_or_else(|_| "windows".into())
}

/// `true` for the security-relevant kinds that must be sent immediately rather
/// than batched (v2 §19: "immediate send for failures/rollback/av").
fn is_urgent(kind: AuditKind) -> bool {
    matches!(
        kind,
        AuditKind::Sha256Mismatch
            | AuditKind::DownloadFailed
            | AuditKind::PolicyDenied
            | AuditKind::AvBlock
            | AuditKind::Rollback
    )
}

/// Enqueue an audit event. Urgent kinds are also flushed right away on a
/// detached task; routine kinds wait for the 5-minute batch.
pub fn record(app: &AppHandle, event: AuditEvent) {
    let urgent = is_urgent(event.kind);
    if let Some(queue) = app.try_state::<AuditQueue>() {
        queue.push(event);
    }
    if urgent {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = flush_audit(&app).await {
                tracing::warn!(error = %e, "urgent audit flush failed (will retry in batch)");
            }
        });
    }
}

/// Drain and POST all queued audit events. Re-enqueues nothing on failure in
/// this Phase 1 implementation (best-effort); the durable record is the install
/// report. Called by the background loop and by urgent `record`.
pub async fn flush_audit(app: &AppHandle) -> Result<()> {
    let state = app.state::<AppState>();
    let server = state.config_snapshot().server;
    if server.is_empty() {
        return Ok(());
    }
    let events = match app.try_state::<AuditQueue>() {
        Some(q) => q.drain(),
        None => return Ok(()),
    };
    for ev in events {
        if let Err(e) = crate::http::post_audit(&state.http, &server, &ev).await {
            tracing::warn!(kind = ?ev.kind, error = %e, "audit POST failed");
            // Best-effort: drop. (A production Phase 2 would persist & retry.)
        }
    }
    Ok(())
}

/// POST a single install report immediately (these are never batched — one per
/// attempt, and they are the durable record the hub persists).
pub async fn send_install_report(app: &AppHandle, report: &InstallReport) -> Result<()> {
    let state = app.state::<AppState>();
    let server = state.config_snapshot().server;
    if server.is_empty() {
        return Ok(());
    }
    crate::http::post_install_report(&state.http, &server, report).await
}

/// Spawn the periodic audit-batch flusher (v2 §19). Runs for the life of the app.
pub fn spawn_batch_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(BATCH_INTERVAL);
        // Skip the immediate first tick so we don't flush an empty queue at boot.
        ticker.tick().await;
        loop {
            ticker.tick().await;
            if let Err(e) = flush_audit(&app).await {
                tracing::warn!(error = %e, "batch audit flush failed");
            }
        }
    });
}
