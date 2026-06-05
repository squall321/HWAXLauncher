//! The complete Tauri `#[tauri::command]` surface (v2 plan §10, IPC catalog).
//! Every command is thin: it pulls `AppState`, delegates to the focused modules
//! (`installer`, `sync`, `auth`, `http`, `telemetry`), and returns an IPC
//! view-model from [`crate::models`]. Where a piece needs more wiring it carries
//! a clear `TODO` but still compiles and behaves sensibly.

use crate::models::*;
use crate::state::{AppState, AGENT_VERSION};
use crate::{auth, config_store, installer, sync, telemetry};
use hwax_core::audit::{AuditEvent, AuditKind, Severity};
use hwax_core::config::AgentConfig;
use hwax_core::state::ModuleState;
use hwax_core::time;
use hwax_core::{install, store};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

/// Registry of currently-running module child processes, keyed by module id, so
/// `stop_module` can terminate what `run_module` spawned (v2 §14).
#[derive(Default)]
pub struct RunRegistry {
    children: Mutex<HashMap<String, tokio::process::Child>>,
}

impl RunRegistry {
    pub fn new() -> Self {
        Self::default()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// status / pairing
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn agent_status(state: State<'_, AppState>) -> AgentStatus {
    let cfg = state.config_snapshot();
    let paired = state.is_paired();
    AgentStatus {
        agent_id: (!cfg.agent_id.is_empty()).then(|| cfg.agent_id.clone()),
        server: (!cfg.server.is_empty()).then(|| cfg.server.clone()),
        paired,
        last_sync: state.last_sync(),
        module_count: sync::module_count(&state),
        error_count: state.error_count() as u32,
        status_color: state.status_color().as_str().to_string(),
    }
}

/// Begin pairing: open the HEAXHub enrollment page and return the URL + a code
/// the user reads off the web UI. The real device-JWT exchange happens in
/// [`complete_pairing`] with the operator-issued enrollment token.
#[tauri::command]
pub fn start_pairing(state: State<'_, AppState>) -> Result<PairingInfo, String> {
    let cfg = state.config_snapshot();
    // Before first pairing `server` may be empty; fall back to the on-prem
    // default. The user never types this URL — it is fixed by deployment.
    let server = if cfg.server.is_empty() {
        "https://heaxhub.internal".to_string()
    } else {
        cfg.server.clone()
    };
    // A short human-facing code shown alongside the web approval (display only;
    // the security boundary is the single-use enrollment_token).
    let code = gen_pairing_code();
    let url = format!(
        "{}/devices/pair?code={}",
        server.trim_end_matches('/'),
        code
    );
    Ok(PairingInfo { url, code })
}

/// Complete pairing by exchanging an operator-issued enrollment token for a
/// device JWT pair (stored in the credential manager) and recording `agent_id`
/// in `config.json` (v2 §12). `server` is fixed by deployment; we keep whatever
/// the config already has, or the on-prem default for a first pairing.
#[tauri::command]
pub async fn complete_pairing(
    app: AppHandle,
    enrollment_token: String,
) -> Result<AgentStatus, String> {
    let state = app.state::<AppState>();
    let existing = state.config_snapshot();
    let server = if existing.server.is_empty() {
        "https://heaxhub.internal".to_string()
    } else {
        existing.server.clone()
    };
    // Re-pairing: wipe any stale device JWT / refresh token so the new
    // enrollment starts from a clean credential-store state (v2 §12).
    let _ = auth::clear_tokens();

    let hostname = telemetry::hostname();
    let result = auth::enroll(
        &state.http,
        &server,
        &enrollment_token,
        Some(&hostname),
        AGENT_VERSION,
    )
    .await
    .map_err(|e| e.to_string())?;

    // Persist agent_id + server (NOT the JWT — that is in the credential store).
    // Preserve ALL user-tuned settings across (re-)pairing; only `server` and
    // `agent_id` are (re)set by enrollment. `allowed_origins` is left at the fresh
    // [server] default (same server ⇒ correct, and never silently widened).
    let mut cfg = AgentConfig::new(server.clone(), result.agent_id.clone());
    cfg.auto_update = existing.auto_update;
    cfg.start_on_boot = existing.start_on_boot;
    cfg.log_level = existing.log_level;
    cfg.keep_last_n_versions = existing.keep_last_n_versions;
    cfg.sync_interval_min = existing.sync_interval_min;
    cfg.channel = existing.channel;
    cfg.proxy = existing.proxy;
    cfg.telemetry_anonymous = existing.telemetry_anonymous;
    config_store::save(&state.paths, &cfg).map_err(|e| e.to_string())?;
    *state.config.write().expect("config lock") = cfg;

    // Audit the enrollment (e2e §8.1).
    let ev = AuditEvent::new(
        &result.agent_id,
        AuditKind::Enrollment,
        chrono::Utc::now(),
        Severity::Info,
    )
    .client_meta(telemetry::client_meta());
    telemetry::record(&app, ev);

    // Kick an immediate sync so the module list is populated post-pairing.
    let _ = sync::sync_now(&app).await;
    crate::tray::refresh(&app);

    Ok(agent_status(state))
}

fn gen_pairing_code() -> String {
    // A 6-digit display code derived from the process id + a time nonce. This is
    // not a secret (the enrollment_token is); it only helps the user match the
    // agent to the right web approval.
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!("{:06}", (nonce ^ std::process::id()) % 1_000_000)
}

// ─────────────────────────────────────────────────────────────────────────────
// manifest / modules
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn sync_manifest(app: AppHandle) -> Result<SyncResult, String> {
    let result = sync::sync_now(&app).await.map_err(|e| e.to_string())?;
    crate::tray::refresh(&app);
    Ok(result)
}

#[tauri::command]
pub fn list_modules(state: State<'_, AppState>) -> Vec<ModuleView> {
    sync::build_module_views(&state)
}

#[tauri::command]
pub fn module_detail(state: State<'_, AppState>, id: String) -> Result<ModuleDetail, String> {
    let manifest = sync::read_cached_manifest(&state);
    let program = manifest.as_ref().and_then(|m| m.program(&id).cloned());

    let module_dir = state.paths.module_dir(&id);
    let current = store::try_read_current(&module_dir);
    let current_version = current.as_ref().map(|c| c.version.clone());
    let latest_version = program.as_ref().map(|p| p.version.clone());

    let module_state =
        decide_state_or_default(current_version.as_deref(), latest_version.as_deref());

    // History = on-disk version directories with their recorded install time.
    let mut history = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&module_dir) {
        for entry in rd.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let ver = entry.file_name().to_string_lossy().to_string();
                if semver::Version::parse(&ver).is_err() {
                    continue; // skip *.staging and non-version dirs
                }
                let installed_at = store::read_install_meta(&entry.path())
                    .map(|m| m.installed_at)
                    .unwrap_or_default();
                history.push(HistoryEntry {
                    version: ver,
                    installed_at,
                });
            }
        }
    }
    history.sort_by(|a, b| b.version.cmp(&a.version));

    let requires_admin = program
        .as_ref()
        .and_then(|p| p.requirements.as_ref())
        .map(|r| r.requires_admin)
        .unwrap_or(false);

    Ok(ModuleDetail {
        id: id.clone(),
        name: program.as_ref().map(|p| p.name.clone()).unwrap_or(id),
        description: program.as_ref().and_then(|p| p.description.clone()),
        category: program.as_ref().and_then(|p| p.category.clone()),
        current_version,
        latest_version,
        state: module_state,
        history,
        requires_admin,
    })
}

fn decide_state_or_default(local: Option<&str>, latest: Option<&str>) -> ModuleState {
    match latest {
        Some(latest) => {
            hwax_core::state::decide_state(local, latest).unwrap_or(ModuleState::Installed)
        }
        None => {
            if local.is_some() {
                ModuleState::Installed
            } else {
                ModuleState::NotInstalled
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// install / cancel / rollback / uninstall
// ─────────────────────────────────────────────────────────────────────────────

/// Install or update a module to `version`. The package URL/sha are taken from
/// the cached manifest — NEVER from the UI — so the security gates apply to a
/// server-signed descriptor. Progress streams via the `install:progress` event.
#[tauri::command]
pub async fn install_module(app: AppHandle, id: String, version: String) -> Result<(), String> {
    let program = {
        let state = app.state::<AppState>();
        let manifest = sync::read_cached_manifest(&state)
            .ok_or_else(|| "no manifest cached; sync first".to_string())?;
        let program = manifest
            .program(&id)
            .ok_or_else(|| format!("module '{id}' not in manifest"))?
            .clone();
        if program.version != version {
            // The catalog only ever publishes one installable version; refuse a
            // mismatch rather than fetching an unlisted version.
            return Err(format!(
                "requested version {version} != manifest version {} for '{id}'",
                program.version
            ));
        }
        program
    };
    installer::install(&app, &program)
        .await
        .map_err(|e| e.to_string())?;
    crate::tray::refresh(&app);
    Ok(())
}

#[tauri::command]
pub async fn cancel_install(app: AppHandle, id: String) -> Result<(), String> {
    let paths = app.state::<AppState>().paths.clone();
    installer::cancel(&paths, &id)
        .await
        .map_err(|e| e.to_string())
}

/// Roll back the active version by rewriting `current.json` (v2 §9). Pure logic
/// is `hwax_core::install::rollback`; we add the report + audit emission.
#[tauri::command]
pub async fn rollback_module(
    app: AppHandle,
    id: String,
    target: Option<String>,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let cfg = state.config_snapshot();
    let module_dir = state.paths.module_dir(&id);
    let started = chrono::Utc::now();
    let installed_at = time::now_rfc3339();

    let new_current = install::rollback(&module_dir, target.as_deref(), &installed_at)
        .map_err(|e| e.to_string())?;

    // install-report status=rolled_back (e2e §7): `version` is the version we
    // rolled FROM, `previous_version` is the version we rolled TO. The core's
    // `new_current` has `.version` = rolled-to and `.rolled_back_from` = rolled-from.
    let rolled_from = new_current
        .rolled_back_from
        .clone()
        .unwrap_or_else(|| new_current.version.clone());
    let report = hwax_core::report::InstallReport::new(
        &cfg.agent_id,
        &id,
        &rolled_from,
        hwax_core::report::InstallStatus::RolledBack,
        started,
        chrono::Utc::now(),
    )
    .sha256_verified(true)
    .previous_version(&new_current.version);
    let _ = telemetry::send_install_report(&app, &report).await;

    // audit kind=rollback (urgent → immediate send), e2e §8.4 shape.
    let ev = AuditEvent::new(
        &cfg.agent_id,
        AuditKind::Rollback,
        chrono::Utc::now(),
        Severity::Warn,
    )
    .app(&id, &new_current.version)
    .payload(json!({
        "rolled_back_from": new_current.rolled_back_from,
        "rolled_back_to": new_current.version,
        "trigger": "user_click",
        "reason": "user"
    }))
    .client_meta(telemetry::client_meta());
    telemetry::record(&app, ev);

    let _ = app.emit(
        "state:changed",
        json!({ "id": id, "state": ModuleState::RolledBack }),
    );
    crate::tray::refresh(&app);
    Ok(())
}

/// Remove a module entirely: stop it if running, delete its module directory
/// (all versions + current.json), and audit it.
#[tauri::command]
pub async fn uninstall_module(app: AppHandle, id: String) -> Result<(), String> {
    let _ = stop_module(app.clone(), id.clone()).await; // best-effort stop first
    let state = app.state::<AppState>();
    let cfg = state.config_snapshot();
    let module_dir = state.paths.module_dir(&id);
    tokio::fs::remove_dir_all(&module_dir)
        .await
        .map_err(|e| format!("removing {}: {e}", module_dir.display()))?;

    let ev = AuditEvent::new(
        &cfg.agent_id,
        AuditKind::Uninstall,
        chrono::Utc::now(),
        Severity::Info,
    )
    .app_id(&id)
    .client_meta(telemetry::client_meta());
    telemetry::record(&app, ev);

    let _ = app.emit(
        "state:changed",
        json!({ "id": id, "state": ModuleState::NotInstalled }),
    );
    crate::tray::refresh(&app);
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// run / stop
// ─────────────────────────────────────────────────────────────────────────────

/// Run a module's whitelisted entry executable (v2 §14). ONLY
/// `manifest.entry.executable`, resolved inside the active version directory,
/// may be spawned; args come from `args_template` only — never the UI.
#[tauri::command]
pub async fn run_module(app: AppHandle, id: String) -> Result<RunHandle, String> {
    let state = app.state::<AppState>();
    let cfg = state.config_snapshot();
    let module_dir = state.paths.module_dir(&id);
    let current = store::read_current(&module_dir).map_err(|e| e.to_string())?;
    let version_dir = store::version_dir(&module_dir, &current.version);

    let manifest =
        sync::read_cached_manifest(&state).ok_or_else(|| "no manifest cached".to_string())?;
    let program = manifest
        .program(&id)
        .ok_or_else(|| format!("module '{id}' not in manifest"))?;
    let entry = &program.entry;
    if entry.executable.is_empty() {
        return Err(format!("module '{id}' has no runnable entry"));
    }

    let exe = version_dir.join(&entry.executable);
    // Whitelist guard: the resolved exe must exist and stay inside the version
    // directory (no `..` escape, no absolute override).
    if !crate::paths::is_within(&version_dir, &exe) || !exe.exists() {
        return Err(format!(
            "entry executable not allowed: {}",
            entry.executable
        ));
    }

    // args_template only — placeholders left as-is for Phase 1 (no user args).
    let args: Vec<String> = entry.args_template.clone().unwrap_or_default();

    let ts = chrono::Utc::now().format("%Y-%m-%dT%H-%M-%S").to_string();
    let log_path = state.paths.run_log(&id, &ts);
    let log_file = std::fs::File::create(&log_path).map_err(|e| e.to_string())?;
    let log_clone = log_file.try_clone().map_err(|e| e.to_string())?;

    let working_dir = entry
        .working_dir
        .as_ref()
        .filter(|w| !w.is_empty())
        .map(|w| version_dir.join(w))
        .unwrap_or_else(|| version_dir.clone());

    let mut cmd = tokio::process::Command::new(&exe);
    cmd.args(&args)
        .current_dir(&working_dir)
        .stdout(std::process::Stdio::from(log_file))
        .stderr(std::process::Stdio::from(log_clone));

    let child = cmd.spawn().map_err(|e| e.to_string())?;
    let pid = child.id().unwrap_or(0);

    app.state::<RunRegistry>()
        .children
        .lock()
        .expect("run registry poisoned")
        .insert(id.clone(), child);

    let ev = AuditEvent::new(
        &cfg.agent_id,
        AuditKind::Run,
        chrono::Utc::now(),
        Severity::Info,
    )
    .app(&id, &current.version)
    .payload(json!({ "pid": pid }))
    .client_meta(telemetry::client_meta());
    telemetry::record(&app, ev);

    let _ = app.emit(
        "state:changed",
        json!({ "id": id, "state": ModuleState::Running }),
    );
    Ok(RunHandle { pid, id })
}

#[tauri::command]
pub async fn stop_module(app: AppHandle, id: String) -> Result<(), String> {
    let child = app
        .state::<RunRegistry>()
        .children
        .lock()
        .expect("run registry poisoned")
        .remove(&id);
    if let Some(mut child) = child {
        let _ = child.start_kill();
        let _ = child.wait().await;
        let cfg = app.state::<AppState>().config_snapshot();
        let ev = AuditEvent::new(
            &cfg.agent_id,
            AuditKind::Stop,
            chrono::Utc::now(),
            Severity::Info,
        )
        .app_id(&id)
        .client_meta(telemetry::client_meta());
        telemetry::record(&app, ev);
        let _ = app.emit(
            "state:changed",
            json!({ "id": id, "state": ModuleState::Stopped }),
        );
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// logs
// ─────────────────────────────────────────────────────────────────────────────

/// Open the logs folder (or reveal a specific module's newest log) in Explorer
/// via the `opener` plugin — NOT `shell.open` (forbidden by §15). The opener
/// capability is scoped to `$LOCALDATA/HWAXAgent/**`.
#[tauri::command]
pub fn open_log(app: AppHandle, id: Option<String>) -> Result<(), String> {
    let _ = id; // Phase 1: always reveal the logs folder; per-file in Phase 2.
    open_logs_folder(&app).map_err(|e| e.to_string())
}

/// Tail the last `lines` lines of the agent log (or a module's newest log).
#[tauri::command]
pub fn tail_log(
    state: State<'_, AppState>,
    id: Option<String>,
    lines: usize,
) -> Result<String, String> {
    let dir = &state.paths.logs;
    // Pick the newest matching log file.
    let prefix = match &id {
        Some(id) => format!("install-{}", crate::paths::sanitize_id(id)),
        None => "agent-".to_string(),
    };
    let mut newest: Option<(std::time::SystemTime, std::path::PathBuf)> = None;
    if let Ok(rd) = std::fs::read_dir(dir) {
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&prefix) {
                if let Ok(meta) = entry.metadata() {
                    let modified = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
                    if newest.as_ref().map(|(t, _)| modified > *t).unwrap_or(true) {
                        newest = Some((modified, entry.path()));
                    }
                }
            }
        }
    }
    let Some((_, path)) = newest else {
        return Ok(String::new());
    };
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let tail: Vec<&str> = content.lines().rev().take(lines).collect();
    Ok(tail.into_iter().rev().collect::<Vec<_>>().join("\n"))
}

// ─────────────────────────────────────────────────────────────────────────────
// config
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> AgentConfig {
    state.config_snapshot()
}

/// Apply a partial config patch (only the user-tunable fields — `server`/
/// `agent_id` change only via re-pairing). Persists atomically and returns the
/// merged config. `start_on_boot` is reflected through the autostart plugin.
#[tauri::command]
pub fn update_config(app: AppHandle, patch: ConfigPatch) -> Result<AgentConfig, String> {
    let state = app.state::<AppState>();
    let mut cfg = state.config_snapshot();
    if let Some(v) = patch.auto_update {
        cfg.auto_update = v;
    }
    if let Some(v) = patch.start_on_boot {
        cfg.start_on_boot = v;
        apply_autostart(&app, v);
    }
    if let Some(v) = patch.log_level {
        cfg.log_level = v;
    }
    if let Some(v) = patch.keep_last_n_versions {
        cfg.keep_last_n_versions = v;
    }
    if let Some(v) = patch.sync_interval_min {
        cfg.sync_interval_min = v;
    }
    if let Some(v) = patch.channel {
        cfg.channel = v;
    }
    if let Some(v) = patch.proxy {
        cfg.proxy = v;
    }
    if let Some(v) = patch.telemetry_anonymous {
        cfg.telemetry_anonymous = v;
    }
    config_store::save(&state.paths, &cfg).map_err(|e| e.to_string())?;
    *state.config.write().expect("config lock") = cfg.clone();
    Ok(cfg)
}

/// Toggle the OS autostart entry via the plugin. Never auto-enabled (§11.2).
fn apply_autostart(app: &AppHandle, enable: bool) {
    use tauri_plugin_autostart::ManagerExt;
    let mgr = app.autolaunch();
    let result = if enable { mgr.enable() } else { mgr.disable() };
    if let Err(e) = result {
        tracing::warn!(error = %e, enable, "autostart toggle failed");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// health / diagnostics / lifecycle
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn health_check(app: AppHandle) -> Result<HealthReport, String> {
    let state = app.state::<AppState>();
    let cfg = state.config_snapshot();
    let server_reachable = crate::http::server_reachable(&state.http, &cfg.server).await;
    state
        .server_reachable
        .store(server_reachable, std::sync::atomic::Ordering::Relaxed);

    // write_ok: prove we can write under the app root.
    let probe = state.paths.cache.join(".write_probe");
    let write_ok = std::fs::write(&probe, b"ok").is_ok();
    let _ = std::fs::remove_file(&probe);

    let disk_free_bytes = free_space(&state.paths.root);

    Ok(HealthReport {
        server_reachable,
        disk_free_bytes,
        write_ok,
    })
}

/// Available bytes on the volume backing `path` (for `health_check`). `fs2` is a
/// thin `GetDiskFreeSpaceExW` wrapper on Windows. Returns 0 if the query fails —
/// the hub/health UI treats 0 as "unknown".
fn free_space(path: &std::path::Path) -> u64 {
    fs2::available_space(path).unwrap_or(0)
}

/// Build a diagnostic zip under `%LocalAppData%\HWAXAgent\diagnostics\` (v2 §19)
/// and reveal it in Explorer. Bundles an anonymized `system.json`, `config.json`
/// (which never holds a token — the device_jwt lives only in the Credential
/// Manager), the last cached manifest, and the last 7 days of logs. Because the
/// dump lands under the app-data root it is inside the `opener` capability scope,
/// so we can reveal it (unlike a `%TEMP%` path).
#[tauri::command]
pub fn make_dump(app: AppHandle) -> Result<String, String> {
    use std::io::Write;
    let state = app.state::<AppState>();
    let cfg = state.config_snapshot();
    let paths = state.paths.clone();

    // Anonymized summary (built while `state` is in scope).
    let system = json!({
        "agent_version": AGENT_VERSION,
        "server": cfg.server,
        "agent_id": cfg.agent_id,
        "log_level": cfg.log_level,
        "channel": cfg.channel,
        "last_sync": state.last_sync(),
        "module_count": sync::module_count(&state),
        "error_count": state.error_count(),
        "os": std::env::consts::OS,
        "generated_at": time::now_rfc3339(),
    });

    let ts = chrono::Utc::now().format("%Y%m%dT%H%M%S").to_string();
    let diag_dir = paths.root.join("diagnostics");
    std::fs::create_dir_all(&diag_dir).map_err(|e| e.to_string())?;
    let out = diag_dir.join(format!("hwax-dump-{ts}.zip"));

    let file = std::fs::File::create(&out).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("system.json", opts)
        .map_err(|e| e.to_string())?;
    zip.write_all(&serde_json::to_vec_pretty(&system).unwrap_or_default())
        .map_err(|e| e.to_string())?;

    // config.json (no secrets) + last cached manifest — best-effort.
    for (name, path) in [
        ("config.json", paths.config_file.clone()),
        ("manifest.json", paths.manifest_cache()),
    ] {
        if let Ok(bytes) = std::fs::read(&path) {
            if zip.start_file(name, opts).is_ok() {
                let _ = zip.write_all(&bytes);
            }
        }
    }

    // Logs from the last 7 days.
    let cutoff = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(7 * 24 * 3600))
        .unwrap_or(std::time::UNIX_EPOCH);
    if let Ok(rd) = std::fs::read_dir(&paths.logs) {
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.ends_with(".log") {
                continue;
            }
            let recent = entry
                .metadata()
                .and_then(|m| m.modified())
                .map(|t| t >= cutoff)
                .unwrap_or(true);
            if recent {
                if let Ok(bytes) = std::fs::read(entry.path()) {
                    if zip.start_file(format!("logs/{name}"), opts).is_ok() {
                        let _ = zip.write_all(&bytes);
                    }
                }
            }
        }
    }

    zip.finish().map_err(|e| e.to_string())?;
    // diag_dir is under %LocalAppData%\HWAXAgent → within the opener scope.
    let _ = reveal(&app, &diag_dir);
    Ok(out.to_string_lossy().to_string())
}

#[tauri::command]
pub fn clear_cache(state: State<'_, AppState>) -> Result<(), String> {
    // Delete only the transient downloads — never module versions or config.
    if let Ok(rd) = std::fs::read_dir(&state.paths.downloads) {
        for entry in rd.flatten() {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    Ok(())
}

#[tauri::command]
pub fn quit(app: AppHandle) {
    app.exit(0);
}

// ─────────────────────────────────────────────────────────────────────────────
// helpers shared with the tray
// ─────────────────────────────────────────────────────────────────────────────

/// Reveal the logs folder in Explorer via the opener plugin (scoped to the app
/// data dir in `capabilities/default.json`).
pub fn open_logs_folder(app: &AppHandle) -> anyhow::Result<()> {
    let logs = app.state::<AppState>().paths.logs.clone();
    reveal(app, &logs)
}

/// Open a path with the system handler via the opener plugin. For a directory
/// this opens Explorer at that folder; for a file it reveals it.
fn reveal(app: &AppHandle, path: &std::path::Path) -> anyhow::Result<()> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(path.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| anyhow::anyhow!("opener failed: {e}"))
}

/// Dispatch a tray module submenu action (`run` / `update` / `detail`).
pub async fn dispatch_tray_module_action(app: &AppHandle, action: &str, id: &str) {
    match action {
        "run" => {
            if let Err(e) = run_module(app.clone(), id.to_string()).await {
                tracing::warn!(id, error = %e, "tray run failed");
            }
        }
        "update" => {
            // Look up the latest version and install it.
            let version = {
                let state = app.state::<AppState>();
                sync::read_cached_manifest(&state)
                    .and_then(|m| m.program(id).map(|p| p.version.clone()))
            };
            if let Some(version) = version {
                if let Err(e) = install_module(app.clone(), id.to_string(), version).await {
                    tracing::warn!(id, error = %e, "tray update failed");
                }
            }
        }
        "detail" => {
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.show();
                let _ = win.set_focus();
                let _ = win.emit("nav:detail", json!({ "id": id }));
            }
        }
        _ => {}
    }
}
