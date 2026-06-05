//! Mirror of `contracts/hwax-agent/manifest.schema.json` (the
//! `GET /api/v1/launcher-agents/manifest` response).
//!
//! Received from the server → **unknown fields are ignored** (no
//! `deny_unknown_fields`) so a newer server that adds an optional field does
//! not crash an older agent (PR-protocol §3 graceful degradation). When the
//! agent serializes one of these (e.g. into a cache file) it never emits an
//! `Option::None`, so it cannot introduce a key the schema forbids.

use serde::{Deserialize, Serialize};

/// Top-level manifest. `schema_version` is `const: 1` in the schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub schema_version: u32,
    pub generated_at: String,
    pub programs: Vec<Program>,
}

impl Manifest {
    pub const SCHEMA_VERSION: u32 = 1;

    /// Find a program by its slug id.
    pub fn program(&self, id: &str) -> Option<&Program> {
        self.programs.iter().find(|p| p.id == id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Program {
    /// Stable App slug. Pattern `^[a-z0-9][a-z0-9_-]*$`. Used as the on-disk
    /// module directory name — never interpolate it into a path without that
    /// guarantee (see [`crate::store`]).
    pub id: String,
    pub name: String,
    /// SemVer of the currently published installer.
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub released_at: Option<String>,
    pub package: Package,
    pub entry: Entry,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requirements: Option<Requirements>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<Lifecycle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui: Option<UiHints>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visibility: Option<Visibility>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    /// Wire key is `type`.
    #[serde(rename = "type")]
    pub kind: PackageType,
    /// Absolute URL the agent GETs. Must pass [`crate::origin`] allow-listing.
    pub url: String,
    /// Lowercase hex SHA-256 the agent MUST verify before executing.
    pub sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageType {
    Zip,
    Exe,
    Msi,
    Msix,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    /// Relative path inside the installed program root, e.g. `bin/Tool.exe`.
    /// May be empty for plugin-type modules (no runnable entry).
    pub executable: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args_template: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Requirements {
    #[serde(default)]
    pub requires_admin: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_windows: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lifecycle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_install_check: Option<PostInstallCheck>,
    #[serde(default = "default_true")]
    pub rollback_on_failure: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostInstallCheck {
    pub executable: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_stdout_regex: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiHints {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    /// `#RRGGBB`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_accent: Option<String>,
    #[serde(default)]
    pub show_in_tray: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Private,
    Team,
    Department,
    Company,
}
