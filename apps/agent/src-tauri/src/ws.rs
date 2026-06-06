//! Optional WebSocket push subscriber (v2 plan §13). When the server pushes a
//! "new version published" event, the agent resyncs immediately instead of
//! waiting for the next 30-minute poll. The poll ([`crate::sync::spawn_loop`])
//! always runs too, so a dropped or blocked WS degrades gracefully — this is a
//! *latency* optimization, never a correctness dependency.
//!
//! Endpoint (v2 §13): `<ws|wss>://<server>/ws/agent/{agent_id}?token=<device_jwt>`.
//! The push payload is server-defined and intentionally NOT parsed — any inbound
//! message means "the catalog may have changed, go check", so we just resync.
//!
//! Because the server-side WS is a later phase, connection failures are logged
//! at `debug` (not `warn`): a missing endpoint must not spam the agent log, and
//! the poll keeps everything correct regardless.

use crate::state::AppState;
use futures_util::StreamExt;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tokio_tungstenite::tungstenite::Message;

/// Reconnect backoff, capped at 5 minutes so an absent endpoint stays quiet.
const BACKOFF_STEPS_SECS: [u64; 6] = [2, 5, 15, 30, 60, 300];

/// Spawn the WS push loop. Idle until paired; otherwise reconnects forever with
/// capped backoff. Safe to run even if the server has no WS endpoint yet.
pub fn spawn_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        // Let pairing / boot settle before the first attempt.
        tokio::time::sleep(Duration::from_secs(8)).await;
        let mut attempt: usize = 0;
        loop {
            let Some(url) = ws_url(&app) else {
                // Not paired / no token yet — the poll covers sync; check later.
                tokio::time::sleep(Duration::from_secs(30)).await;
                continue;
            };

            match connect_and_listen(&app, &url).await {
                Ok(()) => {
                    attempt = 0; // clean close → reconnect promptly
                    tracing::debug!("agent WS closed; reconnecting");
                }
                Err(e) => {
                    attempt = attempt.saturating_add(1);
                    tracing::debug!(error = %e, "agent WS unavailable; will retry");
                }
            }
            let idx = attempt.min(BACKOFF_STEPS_SECS.len() - 1);
            tokio::time::sleep(Duration::from_secs(BACKOFF_STEPS_SECS[idx])).await;
        }
    });
}

/// Build `<ws|wss>://host/ws/agent/{id}?token=...` from config + the stored JWT,
/// or `None` when not paired / no token is available.
fn ws_url(app: &AppHandle) -> Option<String> {
    let state = app.state::<AppState>();
    if !state.is_paired() {
        return None;
    }
    let cfg = state.config_snapshot();
    if cfg.server.is_empty() || cfg.agent_id.is_empty() {
        return None;
    }
    let token = crate::auth::access_token().ok().flatten()?;
    let base = cfg
        .server
        .trim_end_matches('/')
        .replacen("https://", "wss://", 1)
        .replacen("http://", "ws://", 1);
    Some(format!("{base}/ws/agent/{}?token={}", cfg.agent_id, token))
}

/// Connect, then resync on every inbound message until the socket closes.
async fn connect_and_listen(app: &AppHandle, url: &str) -> anyhow::Result<()> {
    let (mut ws, _resp) = tokio_tungstenite::connect_async(url).await?;
    tracing::info!("agent WS connected");
    while let Some(msg) = ws.next().await {
        match msg? {
            // Any push ⇒ the catalog may have changed ⇒ resync immediately.
            Message::Text(_) | Message::Binary(_) => {
                if let Err(e) = crate::sync::sync_now(app).await {
                    tracing::warn!(error = %e, "WS-triggered sync failed");
                } else {
                    crate::tray::refresh(app);
                }
            }
            Message::Close(_) => break,
            // Ping/Pong are answered by tungstenite automatically.
            _ => {}
        }
    }
    Ok(())
}
