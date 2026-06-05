//! `hwax-core` — pure, Tauri-independent core logic for the HWAX Agent.
//!
//! Everything here builds and tests **without** Tauri or WebView2, so the
//! correctness-critical paths (SHA-256 verification, zip-slip-safe extraction,
//! atomic version swap, rollback, GC, the install state machine, and the
//! contract payload builders) can be exercised headlessly via
//! `cargo test -p hwax-core`.
//!
//! The Tauri shell (`apps/agent/src-tauri`) supplies the IO that does *not*
//! belong here — the async HTTP download (`reqwest`), the credential store
//! (`keyring`), tray/IPC, and spawning the `post_install_check` process — and
//! calls into these pure functions for the rest.
//!
//! ## Contract fidelity
//! The model types are faithful mirrors of `contracts/hwax-agent/*.schema.json`
//! (contract v0.2.0). Received types (manifest) ignore unknown fields for
//! forward-compatibility (PR-protocol §3 graceful degradation). Sent types
//! (install-report, audit-event) only ever serialize keys that exist in the
//! schema, because every object there is `additionalProperties: false`.

pub mod atomic;
pub mod audit;
pub mod config;
pub mod error;
pub mod hash;
pub mod install;
pub mod manifest;
pub mod origin;
pub mod report;
pub mod state;
pub mod store;
pub mod time;
pub mod zip_safe;

pub use error::{CoreError, Result};

/// Convenience re-exports for callers (the Tauri shell).
pub mod prelude {
    pub use crate::audit::{AuditEvent, AuditKind, ClientMeta, Severity};
    pub use crate::config::AgentConfig;
    pub use crate::error::{CoreError, Result};
    pub use crate::manifest::{
        Entry, Lifecycle, Manifest, Package, PackageType, PostInstallCheck, Program, Requirements,
        UiHints, Visibility,
    };
    pub use crate::report::{InstallReport, InstallStatus};
    pub use crate::state::{InstallPhase, ModuleState};
    pub use crate::store::{CurrentJson, InstallMeta};
}
