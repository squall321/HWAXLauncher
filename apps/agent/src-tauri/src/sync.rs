//! Manifest synchronization (v2 plan §13). Periodic (every
//! `config.sync_interval_min`, default 30) and on-demand (tray "sync now" /
//! post-pairing / boot). Uses `If-None-Match` so an unchanged manifest costs a
//! 304, and the cached snapshot keeps the agent fully functional offline.
//!
//! Per-module resting state is decided by `hwax_core::state::decide_state`,
//! comparing each module's local `current.json` version with the manifest's
//! latest published version.

use crate::models::{ModuleView, SyncResult};
use crate::state::AppState;
use anyhow::{Context, Result};
use hwax_core::manifest::Manifest;
use hwax_core::state::{decide_state, ModuleState};
use hwax_core::store;
use hwax_core::time;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// Read the cached manifest snapshot (`cache/manifest.json`), if any.
pub fn read_cached_manifest(state: &AppState) -> Option<Manifest> {
    let bytes = std::fs::read(state.paths.manifest_cache()).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Read the stored ETag (`cache/manifest.etag`), if any.
fn read_cached_etag(state: &AppState) -> Option<String> {
    std::fs::read_to_string(state.paths.manifest_etag())
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Persist the manifest snapshot + ETag atomically.
fn write_cache(state: &AppState, manifest: &Manifest, etag: Option<&str>) -> Result<()> {
    hwax_core::atomic::write_atomic_json(&state.paths.manifest_cache(), manifest)
        .context("writing manifest cache")?;
    if let Some(etag) = etag {
        hwax_core::atomic::write_atomic(&state.paths.manifest_etag(), etag.as_bytes())
            .context("writing manifest etag")?;
    }
    Ok(())
}

/// Force a manifest sync. Returns the diff result (changed + per-module views).
/// On a 304 (or an unreachable server with a usable cache) `changed` is false.
pub async fn sync_now(app: &AppHandle) -> Result<SyncResult> {
    let state = app.state::<AppState>();
    let cfg = state.config_snapshot();
    if cfg.server.is_empty() {
        return Err(anyhow::anyhow!("not paired: no server configured"));
    }

    let prev_etag = read_cached_etag(&state);
    let result = crate::http::fetch_manifest(&state.http, &cfg.server, prev_etag.as_deref()).await;

    let changed = match result {
        Ok(crate::http::ManifestFetch::Modified { manifest, etag }) => {
            write_cache(&state, &manifest, etag.as_deref())?;
            state.consecutive_sync_failures.store(0, Ordering::Relaxed);
            state.server_reachable.store(true, Ordering::Relaxed);
            true
        }
        Ok(crate::http::ManifestFetch::NotModified) => {
            state.consecutive_sync_failures.store(0, Ordering::Relaxed);
            state.server_reachable.store(true, Ordering::Relaxed);
            false
        }
        Err(e) => {
            // Unreachable / error: keep the cache, bump the failure counter.
            let n = state
                .consecutive_sync_failures
                .fetch_add(1, Ordering::Relaxed)
                + 1;
            state.server_reachable.store(false, Ordering::Relaxed);
            tracing::warn!(error = %e, consecutive = n, "manifest sync failed; using cache");
            if read_cached_manifest(&state).is_none() {
                return Err(e); // no cache to fall back on → surface the error
            }
            false
        }
    };

    state.set_last_sync(time::now_rfc3339());
    let modules = build_module_views(&state);
    let _ = app.emit("sync:done", serde_json::json!({ "changed": changed }));
    Ok(SyncResult { changed, modules })
}

/// Build the full `ModuleView` list from the cached manifest + each module's
/// local `current.json`. Modules present in the manifest are listed with their
/// decided state; locally installed modules absent from the manifest are still
/// shown (so a removed-from-catalog module can be uninstalled).
pub fn build_module_views(state: &AppState) -> Vec<ModuleView> {
    let manifest = read_cached_manifest(state);
    let mut views: Vec<ModuleView> = Vec::new();

    if let Some(manifest) = &manifest {
        for p in &manifest.programs {
            let module_dir = state.paths.module_dir(&p.id);
            let local = store::try_read_current(&module_dir).map(|c| c.version);
            let module_state =
                decide_state(local.as_deref(), &p.version).unwrap_or(ModuleState::NotInstalled);
            let (color_accent, show_in_tray) =
                p.ui.as_ref()
                    .map(|u| (u.color_accent.clone(), u.show_in_tray))
                    .unwrap_or((None, false));
            views.push(ModuleView {
                id: p.id.clone(),
                name: p.name.clone(),
                current_version: local,
                latest_version: Some(p.version.clone()),
                state: module_state,
                show_in_tray,
                color_accent,
                category: p.category.clone(),
            });
        }
    }

    views
}

/// Count of modules currently known (for the tray status line).
pub fn module_count(state: &AppState) -> u32 {
    read_cached_manifest(state)
        .map(|m| m.programs.len() as u32)
        .unwrap_or(0)
}

/// Spawn the periodic sync loop (v2 §13). Re-reads the interval each cycle so a
/// config change takes effect without a restart. Fires once shortly after boot.
pub fn spawn_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        // Small startup delay so the UI/tray is up before the first sync.
        tokio::time::sleep(Duration::from_secs(5)).await;
        loop {
            // Only sync when paired; otherwise idle and re-check shortly.
            let paired = {
                let state = app.state::<AppState>();
                state.is_paired() && !state.config_snapshot().server.is_empty()
            };
            if paired {
                if let Err(e) = sync_now(&app).await {
                    tracing::warn!(error = %e, "periodic sync error");
                }
            }
            let interval_min = {
                let state = app.state::<AppState>();
                state.config_snapshot().sync_interval_min.max(1)
            };
            let sleep = if paired {
                Duration::from_secs(interval_min as u64 * 60)
            } else {
                Duration::from_secs(30) // poll for pairing more often
            };
            tokio::time::sleep(sleep).await;
        }
    });
}
