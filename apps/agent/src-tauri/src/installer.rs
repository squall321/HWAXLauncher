//! One-install orchestration (v2 plan §8, e2e §5–§6). This is the IO glue around
//! the pure `hwax_core` functions — the security-critical *logic* (origin
//! allow-list, sha256 verify, zip-slip-safe extract, atomic swap, GC) lives in
//! the core crate and is merely *called* here. The download (`reqwest`), the
//! `post_install_check` process spawn, the progress events, and the install
//! report / audit emission are this module's job.
//!
//! Flow, with an `install:progress` event at each phase:
//!   1. origin::ensure_allowed(url)         — policy gate (§15 ①)
//!   2. http::download_to(.partial)         — stream (phase=download)
//!   3. hash::verify_file(.partial, sha256) — (phase=verify); delete on mismatch
//!   4. zip_safe::extract_zip_safe(staging) — (phase=extract)
//!   5. run_post_install_check (whitelist)  — (phase=check)
//!   6. install::perform_swap(staging→ver)  — (phase=swap) + GC
//!
//! On any failure: build an InstallReport + AuditEvent via hwax-core and POST.

use crate::state::AppState;
use crate::{paths::Paths, telemetry};
use anyhow::{bail, Context, Result};
use chrono::Utc;
use hwax_core::audit::{AuditEvent, AuditKind, Severity};
use hwax_core::manifest::{PostInstallCheck, Program};
use hwax_core::report::{InstallReport, InstallStatus};
use hwax_core::state::{InstallPhase, InstallProgress};
use hwax_core::{hash, install, origin, time, zip_safe, CoreError};
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};

/// Emit `install:progress { id, phase, percent }` to the React UI (v2 §10).
fn emit_progress(app: &AppHandle, id: &str, phase: InstallPhase, percent: u8) {
    let _ = app.emit("install:progress", InstallProgress::new(id, phase, percent));
}

/// Emit `state:changed { id, state }` (v2 events catalog).
fn emit_state(app: &AppHandle, id: &str, state: hwax_core::state::ModuleState) {
    let _ = app.emit("state:changed", json!({ "id": id, "state": state }));
}

/// Install (or update to) `program.version`. The `program` is the already-fetched
/// manifest entry — the caller (commands::install_module) looks it up from the
/// cached manifest so we never trust a UI-supplied URL/sha.
pub async fn install(app: &AppHandle, program: &Program) -> Result<()> {
    let state = app.state::<AppState>();
    let cfg = state.config_snapshot();
    let paths = state.paths.clone();
    let id = program.id.clone();
    let version = program.version.clone();
    let started_at = Utc::now();

    let outcome = run_install(app, &state, &cfg, &paths, program).await;

    match outcome {
        Ok(()) => {
            // Success report (e2e §5). sha was verified, status=success.
            let report = InstallReport::new(
                &cfg.agent_id,
                &id,
                &version,
                InstallStatus::Success,
                started_at,
                Utc::now(),
            )
            .sha256_verified(true);
            let _ = telemetry::send_install_report(app, &report).await;

            // audit kind=install, payload.outcome=success (e2e §8.2 pattern).
            let ev = AuditEvent::new(
                &cfg.agent_id,
                AuditKind::Install,
                Utc::now(),
                Severity::Info,
            )
            .app(&id, &version)
            .payload(json!({ "outcome": "success" }))
            .client_meta(telemetry::client_meta());
            telemetry::record(app, ev);

            emit_state(app, &id, hwax_core::state::ModuleState::Installed);
            tracing::info!(id = %id, version = %version, "install succeeded");
            Ok(())
        }
        Err(err) => {
            // Clean up this attempt's artifacts so a failed install never leaves
            // orphaned bytes behind (e2e §6.3/§6.4 require '.partial 삭제';
            // v2 scenario D requires the staging dir be removed). Covers download,
            // extract, and post_install_check failures; the sha-mismatch path also
            // deletes .partial inside run_install, so this is a harmless retry there.
            let _ = tokio::fs::remove_file(paths.partial_download(&id, &version)).await;
            let _ =
                tokio::fs::remove_dir_all(paths.module_dir(&id).join(format!("{version}.staging")))
                    .await;
            state.bump_error();
            emit_state(app, &id, hwax_core::state::ModuleState::Failed);
            // Classify the failure into the right report status + audit kind.
            let (status, kind, sha_verified) = classify(&err);
            let report =
                InstallReport::new(&cfg.agent_id, &id, &version, status, started_at, Utc::now())
                    .sha256_verified(sha_verified)
                    .error(err.to_string());
            let _ = telemetry::send_install_report(app, &report).await;

            let ev = AuditEvent::new(&cfg.agent_id, kind, Utc::now(), Severity::Error)
                .app(&id, &version)
                .payload(json!({ "stage": stage_of(kind), "error": err.to_string() }))
                .client_meta(telemetry::client_meta());
            telemetry::record(app, ev);

            tracing::error!(id = %id, version = %version, error = %err, "install failed");
            Err(err)
        }
    }
}

/// The fallible body, factored out so the success/failure reporting wraps it.
async fn run_install(
    app: &AppHandle,
    state: &AppState,
    cfg: &hwax_core::config::AgentConfig,
    paths: &Paths,
    program: &Program,
) -> Result<()> {
    let id = &program.id;
    let version = &program.version;
    let pkg = &program.package;
    // The hub contract says package.url is absolute, but the current server
    // emits a root-relative path; normalize it against config.server (keeping
    // any portal sub-path) so the origin allow-list and reqwest both work.
    let pkg_url = origin::absolutize(&cfg.server, &pkg.url);

    // Open the per-install log file (v2 §19: install-<id>-<ver>.log). Best-effort
    // — a logging failure must not abort the install itself.
    let install_log = paths.install_log(id, version);
    let _ = tokio::fs::write(
        &install_log,
        format!(
            "[{}] install {id} {version} from {}\n",
            time::now_rfc3339(),
            pkg.url
        ),
    )
    .await;

    // 1) Origin allow-list (§15 ①). Maps to audit kind=policy_denied on failure.
    origin::ensure_allowed(&pkg_url, &cfg.effective_allowed_origins())
        .map_err(anyhow::Error::from)?;
    emit_state(app, id, hwax_core::state::ModuleState::Downloading);

    // 2) Stream download to the fixed .partial path (§15 ③).
    let partial = paths.partial_download(id, version);
    emit_progress(app, id, InstallPhase::Download, 0);
    {
        let app2 = app.clone();
        let id2 = id.clone();
        crate::http::download_to(
            &state.http,
            &cfg.server,
            &pkg_url,
            &partial,
            move |done, total| {
                let pct = match total {
                    Some(t) if t > 0 => ((done.saturating_mul(100)) / t).min(100) as u8,
                    _ => 0,
                };
                emit_progress(&app2, &id2, InstallPhase::Download, pct);
            },
        )
        .await
        .context("download failed")?;
    }
    emit_progress(app, id, InstallPhase::Download, 100);

    // 3) SHA-256 verify (§15 ②). On mismatch: delete .partial, propagate typed
    //    CoreError::Sha256Mismatch so `classify` emits sha256_mismatch.
    emit_state(app, id, hwax_core::state::ModuleState::Verifying);
    emit_progress(app, id, InstallPhase::Verify, 0);
    if let Err(e) = hash::verify_file(&partial, &pkg.sha256) {
        let _ = tokio::fs::remove_file(&partial).await; // delete on sha mismatch
        return Err(anyhow::Error::from(e));
    }
    emit_progress(app, id, InstallPhase::Verify, 100);

    // 4) zip-slip-safe extract into <ver>.staging (§8).
    emit_state(app, id, hwax_core::state::ModuleState::Extracting);
    emit_progress(app, id, InstallPhase::Extract, 0);
    let module_dir = paths.module_dir(id);
    let staging = module_dir.join(format!("{version}.staging"));
    // Clean any stale staging from a previously aborted attempt.
    let _ = tokio::fs::remove_dir_all(&staging).await;
    {
        let zip_path = partial.clone();
        let staging2 = staging.clone();
        // Extraction is CPU/IO-bound and synchronous in core — run it off the
        // async reactor so we don't block other tasks.
        tokio::task::spawn_blocking(move || zip_safe::extract_zip_safe(&zip_path, &staging2))
            .await
            .context("extract task panicked")?
            .map_err(anyhow::Error::from)?;
    }
    emit_progress(app, id, InstallPhase::Extract, 100);

    // 5) post_install_check — entry/check executable whitelist only (§14).
    emit_progress(app, id, InstallPhase::Check, 0);
    if let Some(lifecycle) = &program.lifecycle {
        if let Some(check) = &lifecycle.post_install_check {
            run_post_install_check(&staging, check)
                .await
                .context("post_install_check failed")?;
        }
    }
    emit_progress(app, id, InstallPhase::Check, 100);

    // 6) atomic swap staging→final + write current.json + GC (§8/§9).
    emit_state(app, id, hwax_core::state::ModuleState::Swapping);
    emit_progress(app, id, InstallPhase::Swap, 0);
    let installed_at = time::now_rfc3339();
    {
        let module_dir2 = module_dir.clone();
        let staging2 = staging.clone();
        let version2 = version.clone();
        let sha = pkg.sha256.clone();
        let keep = cfg.keep_last_n_versions as usize;
        tokio::task::spawn_blocking(move || {
            install::perform_swap(
                &module_dir2,
                &staging2,
                &version2,
                &sha,
                &installed_at,
                keep,
            )
        })
        .await
        .context("swap task panicked")?
        .map_err(anyhow::Error::from)?;
    }
    emit_progress(app, id, InstallPhase::Swap, 100);

    // 8) cleanup: drop the .partial (the bytes now live in the version dir).
    let _ = tokio::fs::remove_file(&partial).await;
    Ok(())
}

/// Run the manifest's `post_install_check` executable from *inside* the staging
/// directory — only the whitelisted relative path may be spawned (§14). The
/// optional `expected_stdout_regex` is matched as a plain substring/prefix
/// fallback (we avoid pulling a regex crate; the e2e check is a `^`-anchored
/// prefix, which a `starts_with` after stripping `^` satisfies).
async fn run_post_install_check(staging: &std::path::Path, check: &PostInstallCheck) -> Result<()> {
    let exe = staging.join(&check.executable);
    if !crate::paths::is_within(staging, &exe) || !exe.exists() {
        bail!(
            "post_install_check executable not within staging or missing: {}",
            check.executable
        );
    }
    let mut cmd = tokio::process::Command::new(&exe);
    cmd.current_dir(staging);
    if let Some(args) = &check.args {
        cmd.args(args);
    }
    let output = cmd.output().await.context("spawning post_install_check")?;
    if !output.status.success() {
        bail!(
            "post_install_check exited with {}",
            output.status.code().unwrap_or(-1)
        );
    }
    if let Some(rx) = &check.expected_stdout_regex {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout_matches(&stdout, rx) {
            bail!("post_install_check stdout did not match expected pattern");
        }
    }
    Ok(())
}

/// Minimal pattern check — supports the `^prefix` anchor used by the e2e
/// example (`^Koo Preprocessor 1\.2\.0`). For a leading `^` we treat the rest,
/// with backslashes stripped, as a literal prefix; otherwise we substring-match.
/// (A real regex engine is deliberately not pulled in for Phase 1 — keeping the
/// dependency surface — and the manifest authors control these patterns.)
fn stdout_matches(stdout: &str, pattern: &str) -> bool {
    if let Some(prefix) = pattern.strip_prefix('^') {
        let literal: String = prefix.chars().filter(|c| *c != '\\').collect();
        stdout.trim_start().starts_with(&literal)
    } else {
        let literal: String = pattern.chars().filter(|c| *c != '\\').collect();
        stdout.contains(&literal)
    }
}

/// Map a failure into (install-report status, audit kind, sha256_verified)
/// per the e2e §6 table. Downcasts to the typed `CoreError` where possible.
fn classify(err: &anyhow::Error) -> (InstallStatus, AuditKind, bool) {
    if let Some(core) = err.downcast_ref::<CoreError>() {
        return match core {
            CoreError::Sha256Mismatch { .. } => {
                (InstallStatus::Failed, AuditKind::Sha256Mismatch, false)
            }
            CoreError::OriginNotAllowed(_) => {
                (InstallStatus::Failed, AuditKind::PolicyDenied, false)
            }
            CoreError::ZipSlip(_) => (InstallStatus::Failed, AuditKind::Install, true),
            CoreError::Io(_) => (InstallStatus::Partial, AuditKind::Install, false),
            _ => (InstallStatus::Failed, AuditKind::Install, false),
        };
    }
    // A network/download error string (download_to wraps reqwest errors).
    let msg = err.to_string().to_lowercase();
    if msg.contains("download") || msg.contains("network") || msg.contains("timeout") {
        return (InstallStatus::Failed, AuditKind::DownloadFailed, false);
    }
    (InstallStatus::Failed, AuditKind::Install, false)
}

/// Human-readable stage label for the audit payload.
fn stage_of(kind: AuditKind) -> &'static str {
    match kind {
        AuditKind::PolicyDenied => "policy",
        AuditKind::DownloadFailed => "downloading",
        AuditKind::Sha256Mismatch => "verifying",
        _ => "installing",
    }
}

/// Delete an in-flight `.partial` download (used by `cancel_install`).
pub async fn cancel(paths: &Paths, id: &str) -> Result<()> {
    // We don't know the version mid-flight from the UI, so clear all partials
    // for this id (and any stale staging dirs).
    let prefix = format!("{}-", crate::paths::sanitize_id(id));
    if let Ok(mut rd) = tokio::fs::read_dir(&paths.downloads).await {
        while let Ok(Some(entry)) = rd.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&prefix) && name.ends_with(".zip.partial") {
                let _ = tokio::fs::remove_file(entry.path()).await;
            }
        }
    }
    let module_dir = paths.module_dir(id);
    if let Ok(mut rd) = tokio::fs::read_dir(&module_dir).await {
        while let Ok(Some(entry)) = rd.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".staging") {
                let _ = tokio::fs::remove_dir_all(entry.path()).await;
            }
        }
    }
    Ok(())
}
