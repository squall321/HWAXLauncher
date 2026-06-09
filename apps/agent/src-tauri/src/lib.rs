//! HWAX Agent — Tauri 2 application wiring (the "shell"). This file is the only
//! place that knows about Tauri's `Builder`: it registers the three minimal
//! plugins, `manage`s shared state, exposes the full IPC command surface, and on
//! `setup` initializes tracing, builds the tray, and spawns the background sync
//! and audit-batch loops. All real work lives in the focused modules.
//!
//! Security posture (v2 plan §15/§16/§17) is enforced structurally:
//!   - downloads only from `config.allowed_origins` (hwax_core::origin),
//!   - every package sha256-verified before extract/execute (hwax_core::hash),
//!   - zip-slip-safe extraction (hwax_core::zip_safe),
//!   - atomic staging→final + current.json (hwax_core::install/atomic),
//!   - only `manifest.entry.executable` is spawned (commands::run_module),
//!   - secrets in the Windows Credential Manager only (auth + keyring),
//!   - writes confined to %LocalAppData%\HWAXAgent (paths),
//!   - asInvoker, tight CSP, minimal capabilities (tauri.conf.json / capabilities).

mod auth;
mod commands;
mod config_store;
// `pub` so the integration test (tests/download.rs) can drive `download_to`
// against a mock HEAXHub. Nothing else outside the crate depends on it.
pub mod http;
mod installer;
mod models;
mod paths;
mod state;
mod sync;
mod telemetry;
mod tray;
mod ws;

use crate::paths::Paths;
use crate::state::AppState;
use std::time::Duration;
use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;
use tracing_subscriber::prelude::*;

/// Application entry point invoked by `main.rs`. Kept here so integration tests
/// can build the same `Builder` if needed.
pub fn run() {
    // Resolve + create the on-disk layout before anything else needs it.
    let paths = Paths::resolve().expect("resolve %LocalAppData%\\HWAXAgent paths");
    paths.ensure_dirs().expect("create app directories");

    // Load config (or an empty, unpaired placeholder) using the core type.
    let config = config_store::load_or_default(&paths).expect("load config.json");

    // Initialize file + stdout tracing into …\logs\agent-YYYY-MM-DD.log (§19).
    let _log_guard = init_tracing(&paths, &config.log_level);

    // One pooled HTTPS client (rustls), shared process-wide.
    let http = build_http_client(&config).expect("build reqwest client");

    let app_state = AppState::new(config, http, paths);

    tauri::Builder::default()
        // Single-instance guard (v2 §5) — MUST be the first plugin. A 2nd launch
        // hands its args to the already-running instance (we just focus the panel)
        // and then exits, so there is never a duplicate tray/agent.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        // Agent self-update (v2 §18). Config (endpoint/pubkey) is in tauri.conf.json.
        .plugin(tauri_plugin_updater::Builder::new().build())
        // Opt-in "start on boot" — registered ONLY when the user toggles it on
        // (commands::update_config); never auto-enabled here (§11.2).
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        // Reveal the logs folder in Explorer (replaces the forbidden shell.open).
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .manage(commands::RunRegistry::new())
        .manage(telemetry::AuditQueue::new())
        .invoke_handler(tauri::generate_handler![
            // status / pairing
            commands::agent_status,
            commands::start_pairing,
            commands::complete_pairing,
            // manifest / modules
            commands::sync_manifest,
            commands::list_modules,
            commands::module_detail,
            // install lifecycle
            commands::install_module,
            commands::cancel_install,
            commands::run_module,
            commands::stop_module,
            commands::rollback_module,
            commands::uninstall_module,
            // logs
            commands::open_log,
            commands::tail_log,
            // config
            commands::get_config,
            commands::update_config,
            // health / diagnostics / lifecycle
            commands::health_check,
            commands::make_dump,
            commands::clear_cache,
            commands::quit,
            // agent self-update (v2 §18)
            commands::check_update,
            commands::install_update,
        ])
        .setup(|app| {
            let handle = app.handle().clone();

            // Build the system tray (v2 §4.1/§11.1).
            if let Err(e) = tray::build(&handle) {
                tracing::error!(error = %e, "failed to build tray");
            }

            // Background loops: periodic manifest sync (§13) + audit batch (§19).
            sync::spawn_loop(handle.clone());
            telemetry::spawn_batch_loop(handle.clone());

            // Optional WebSocket push (§13): resync instantly on a server push;
            // the poll above remains the correctness fallback.
            ws::spawn_loop(handle.clone());

            // Heartbeat loop (openapi /heartbeat; v2 §13: 30-min alive ping).
            spawn_heartbeat_loop(handle.clone());

            // Refresh tray state once startup state is ready.
            tray::refresh(&handle);

            tracing::info!(version = state::AGENT_VERSION, "HWAX Agent started");
            Ok(())
        })
        // Closing the panel window hides it to the tray rather than quitting —
        // the agent is tray-resident (v2 §4). Quit is explicit (tray → 종료).
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running HWAX Agent");
}

/// Initialize `tracing` with a daily-rolling file appender in `…\logs` plus a
/// stdout layer (visible during `tauri dev`). Returns the appender guard, which
/// must be kept alive for the process lifetime or buffered logs are dropped.
fn init_tracing(paths: &Paths, log_level: &str) -> tracing_appender::non_blocking::WorkerGuard {
    let file_appender = tracing_appender::rolling::daily(&paths.logs, "agent.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // log_level from config (info default); honor RUST_LOG if set.
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        tracing_subscriber::EnvFilter::new(format!(
            "hwax_agent_lib={log_level},hwax_core={log_level},warn"
        ))
    });

    let file_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_writer(non_blocking)
        .with_ansi(false);
    let stdout_layer = tracing_subscriber::fmt::layer().with_ansi(true);

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(stdout_layer)
        .try_init();

    guard
}

/// Build the shared `reqwest` client. rustls-only TLS (no native OpenSSL), a
/// sane timeout, and the optional config proxy applied.
fn build_http_client(config: &hwax_core::config::AgentConfig) -> reqwest::Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .user_agent(format!("HWAXAgent/{}", state::AGENT_VERSION))
        .connect_timeout(Duration::from_secs(15))
        // No global request timeout: installer downloads can be hundreds of MB.
        .pool_idle_timeout(Duration::from_secs(90));
    if let Some(proxy) = &config.proxy {
        if !proxy.is_empty() {
            if let Ok(p) = reqwest::Proxy::all(proxy) {
                builder = builder.proxy(p);
            }
        }
    }
    builder.build()
}

/// 30-minute heartbeat loop (openapi `POST …/heartbeat` → 204). Sends the agent
/// version + currently installed module versions so the hub can track fleet
/// state. Best-effort; failures only warn.
fn spawn_heartbeat_loop(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            {
                let state = app.state::<AppState>();
                if state.is_paired() {
                    let cfg = state.config_snapshot();
                    let hostname = telemetry::hostname();
                    // Collect installed (id, version) pairs from the cached views.
                    let modules: Vec<(String, String)> = sync::build_module_views(&state)
                        .into_iter()
                        .filter_map(|m| m.current_version.map(|v| (m.id, v)))
                        .collect();
                    if let Err(e) = http::post_heartbeat(
                        &state.http,
                        &cfg.server,
                        Some(&hostname),
                        &modules,
                    )
                    .await
                    {
                        tracing::warn!(error = %e, "heartbeat failed");
                    }
                }
            }
            // ~30-min cadence + jitter so a fleet's beats don't all land at once.
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| u64::from(d.subsec_nanos()))
                .unwrap_or(0);
            let secs = hwax_core::backoff::next_delay_secs(30 * 60, 0, seed);
            tokio::time::sleep(Duration::from_secs(secs)).await;
        }
    });
}
