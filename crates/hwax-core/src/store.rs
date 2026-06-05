//! On-disk module state: `current.json` (which version is active) and
//! `.install_meta.json` (per-version provenance). `current.json` is the single
//! source of truth for "which version is active"; rollback only ever rewrites
//! it (see [`crate::install`]).
//!
//! All functions take an explicit `module_dir` (= `<modules_root>/<id>`) so the
//! crate stays pure and testable — the Tauri shell supplies the real
//! `%LocalAppData%\HWAXAgent\modules` root.

use crate::atomic::write_atomic_json;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const CURRENT_JSON: &str = "current.json";
pub const INSTALL_META: &str = ".install_meta.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurrentJson {
    pub version: String,
    pub installed_at: String,
    pub sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_version: Option<String>,
    /// Set only when this `current.json` is the result of a rollback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rolled_back_from: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallMeta {
    pub sha256: String,
    pub installed_at: String,
}

pub fn version_dir(module_dir: &Path, version: &str) -> PathBuf {
    module_dir.join(version)
}

pub fn current_path(module_dir: &Path) -> PathBuf {
    module_dir.join(CURRENT_JSON)
}

pub fn install_meta_path(version_dir: &Path) -> PathBuf {
    version_dir.join(INSTALL_META)
}

pub fn read_current(module_dir: &Path) -> Result<CurrentJson> {
    let bytes = std::fs::read(current_path(module_dir))?;
    Ok(serde_json::from_slice(&bytes)?)
}

/// `read_current` but `None` on any error (not-yet-installed / unreadable).
pub fn try_read_current(module_dir: &Path) -> Option<CurrentJson> {
    read_current(module_dir).ok()
}

pub fn write_current(module_dir: &Path, current: &CurrentJson) -> Result<()> {
    write_atomic_json(&current_path(module_dir), current)
}

pub fn read_install_meta(version_dir: &Path) -> Result<InstallMeta> {
    let bytes = std::fs::read(install_meta_path(version_dir))?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn write_install_meta(version_dir: &Path, meta: &InstallMeta) -> Result<()> {
    write_atomic_json(&install_meta_path(version_dir), meta)
}
