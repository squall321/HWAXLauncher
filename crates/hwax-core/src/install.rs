//! Filesystem orchestration for install finalize, rollback, and GC. The
//! network download (`reqwest`) and the `post_install_check` process spawn live
//! in the Tauri shell; everything that touches the module directory tree lives
//! here so it is unit-testable with a `tempfile` root.
//!
//! Invariants (v2 plan §8/§9):
//! - `staging → final` is a same-volume `rename` (atomic). The directory
//!   `current.json` points at is therefore always a complete version.
//! - rollback only rewrites `current.json`; version directories are never
//!   deleted by rollback, only by GC — and GC always protects current+previous.

use crate::error::{CoreError, Result};
use crate::store::{
    read_current, read_install_meta, try_read_current, version_dir, write_current,
    write_install_meta, CurrentJson, InstallMeta,
};
use std::collections::HashSet;
use std::path::Path;

/// Promote a verified, extracted staging directory to the active version.
///
/// 1. `rm_rf(final)` if a same-version dir already exists, then
///    `rename(staging → final)` (atomic).
/// 2. write `.install_meta.json`.
/// 3. atomically swap `current.json`, recording the prior version as
///    `previous_version`.
/// 4. GC old versions, always protecting the new and previous versions.
///
/// Returns the freshly written `current.json`.
pub fn perform_swap(
    module_dir: &Path,
    staging_dir: &Path,
    version: &str,
    sha256: &str,
    installed_at: &str,
    keep_last_n: usize,
) -> Result<CurrentJson> {
    std::fs::create_dir_all(module_dir)?;
    let final_dir = version_dir(module_dir, version);
    if final_dir.exists() {
        rm_rf(&final_dir)?;
    }
    std::fs::rename(staging_dir, &final_dir)?;

    write_install_meta(
        &final_dir,
        &InstallMeta {
            sha256: sha256.to_string(),
            installed_at: installed_at.to_string(),
        },
    )?;

    let previous_version = try_read_current(module_dir).map(|c| c.version);
    let current = CurrentJson {
        version: version.to_string(),
        installed_at: installed_at.to_string(),
        sha256: sha256.to_string(),
        previous_version: previous_version.clone(),
        rolled_back_from: None,
    };
    write_current(module_dir, &current)?;

    let mut protect: Vec<&str> = vec![version];
    if let Some(p) = previous_version.as_deref() {
        protect.push(p);
    }
    gc_old_versions(module_dir, keep_last_n, &protect)?;

    Ok(current)
}

/// Roll back the active version by rewriting `current.json` only. `target` of
/// `None` uses the recorded `previous_version`. Errors if the target directory
/// was already GC'd.
pub fn rollback(
    module_dir: &Path,
    target: Option<&str>,
    installed_at: &str,
) -> Result<CurrentJson> {
    let current = read_current(module_dir)?;
    let target_version = match target {
        Some(t) => t.to_string(),
        None => current
            .previous_version
            .clone()
            .ok_or_else(|| CoreError::NoPreviousVersion(module_name(module_dir)))?,
    };

    let target_dir = version_dir(module_dir, &target_version);
    if !target_dir.exists() {
        return Err(CoreError::VersionMissing(target_version));
    }

    // Best-effort: carry the target's recorded sha256 forward.
    let sha256 = read_install_meta(&target_dir)
        .map(|m| m.sha256)
        .unwrap_or_default();

    let new_current = CurrentJson {
        version: target_version,
        installed_at: installed_at.to_string(),
        sha256,
        // previous_version now points back the way we came, so the user can
        // re-apply the version they rolled away from.
        previous_version: Some(current.version.clone()),
        rolled_back_from: Some(current.version),
    };
    write_current(module_dir, &new_current)?;
    Ok(new_current)
}

/// Keep the newest `keep_last_n` semver directories (plus everything in
/// `protect`); delete the rest. Non-semver directories and loose files are left
/// untouched. Returns the versions removed.
pub fn gc_old_versions(
    module_dir: &Path,
    keep_last_n: usize,
    protect: &[&str],
) -> Result<Vec<String>> {
    let mut versions: Vec<(semver::Version, String)> = Vec::new();
    for entry in std::fs::read_dir(module_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if let Ok(v) = semver::Version::parse(&name) {
            versions.push((v, name));
        }
    }
    // Newest first.
    versions.sort_by(|a, b| b.0.cmp(&a.0));

    let mut keep: HashSet<String> = protect.iter().map(|s| s.to_string()).collect();
    for (_, name) in versions.iter().take(keep_last_n) {
        keep.insert(name.clone());
    }

    let mut removed = Vec::new();
    for (_, name) in &versions {
        if !keep.contains(name) {
            rm_rf(&module_dir.join(name))?;
            removed.push(name.clone());
        }
    }
    Ok(removed)
}

fn rm_rf(path: &Path) -> Result<()> {
    match std::fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

fn module_name(p: &Path) -> String {
    p.file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default()
}
