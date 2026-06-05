//! Module lifecycle state machine (v2 plan §6) and install progress phases.
//! The states are serialized to the UI as snake_case strings.

use crate::error::{CoreError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleState {
    Idle,
    Checking,
    Installed,
    Outdated,
    NotInstalled,
    Downloading,
    Verifying,
    Extracting,
    Swapping,
    Running,
    Stopped,
    Failed,
    RollingBack,
    RolledBack,
}

/// Phases reported through the `install:progress` Tauri event (v2 §10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallPhase {
    Download,
    Verify,
    Extract,
    Check,
    Swap,
}

/// Payload of the `install:progress` event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallProgress {
    pub id: String,
    pub phase: InstallPhase,
    /// 0..=100
    pub percent: u8,
}

impl InstallProgress {
    pub fn new(id: impl Into<String>, phase: InstallPhase, percent: u8) -> Self {
        Self {
            id: id.into(),
            phase,
            percent: percent.min(100),
        }
    }
}

/// Decide a module's resting state by comparing the locally active version
/// (from `current.json`, if any) with the manifest's latest version.
pub fn decide_state(local: Option<&str>, latest: &str) -> Result<ModuleState> {
    match local {
        None => Ok(ModuleState::NotInstalled),
        Some(cur) => {
            if is_newer(latest, cur)? {
                Ok(ModuleState::Outdated)
            } else {
                Ok(ModuleState::Installed)
            }
        }
    }
}

/// `true` if `candidate` is a strictly newer SemVer than `baseline`.
pub fn is_newer(candidate: &str, baseline: &str) -> Result<bool> {
    Ok(parse(candidate)? > parse(baseline)?)
}

fn parse(v: &str) -> Result<semver::Version> {
    semver::Version::parse(v).map_err(|e| CoreError::Semver(v.to_string(), e.to_string()))
}
