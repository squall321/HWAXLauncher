//! All on-disk locations the agent uses, rooted at
//! `%LocalAppData%\HWAXAgent` (v2 plan §5). Nothing is ever written outside
//! this tree — that single-root invariant is what keeps EDR/AV calm (§15/§16)
//! and means the agent never needs admin rights.
//!
//! ```text
//! %LocalAppData%\HWAXAgent\
//!  ├─ modules\<id>\<ver>\ , <id>\current.json
//!  ├─ cache\manifest.json , cache\manifest.etag
//!  ├─ cache\downloads\<id>-<ver>.zip.partial
//!  ├─ logs\agent-YYYY-MM-DD.log , install-*.log , run-*.log
//!  └─ config.json
//! ```

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// The product folder name under `%LocalAppData%`.
pub const APP_DIR_NAME: &str = "HWAXAgent";

/// Resolved, process-wide set of agent directories. Cheap to clone (just
/// `PathBuf`s) and stored in [`crate::state::AppState`] so no command re-derives
/// paths from the environment.
#[derive(Debug, Clone)]
pub struct Paths {
    /// `%LocalAppData%\HWAXAgent`
    pub root: PathBuf,
    /// `…\modules`
    pub modules: PathBuf,
    /// `…\cache`
    pub cache: PathBuf,
    /// `…\cache\downloads`
    pub downloads: PathBuf,
    /// `…\logs`
    pub logs: PathBuf,
    /// `…\config.json`
    pub config_file: PathBuf,
}

impl Paths {
    /// Resolve the directory layout from the OS. Does **not** create anything;
    /// call [`Self::ensure_dirs`] once at startup.
    pub fn resolve() -> Result<Self> {
        let base = dirs::data_local_dir()
            .context("could not resolve %LocalAppData% (dirs::data_local_dir)")?;
        let root = base.join(APP_DIR_NAME);
        Ok(Self {
            modules: root.join("modules"),
            cache: root.join("cache"),
            downloads: root.join("cache").join("downloads"),
            logs: root.join("logs"),
            config_file: root.join("config.json"),
            root,
        })
    }

    /// Create the whole directory tree (idempotent). Run at startup before any
    /// download/log write so later code can assume the dirs exist.
    pub fn ensure_dirs(&self) -> Result<()> {
        for dir in [&self.modules, &self.downloads, &self.logs] {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("failed to create {}", dir.display()))?;
        }
        Ok(())
    }

    /// `…\modules\<id>` — the per-module directory `hwax_core::store` operates
    /// on. The id is a manifest slug (`^[a-z0-9][a-z0-9_-]*$`) so it is safe to
    /// use as a single path segment, but we still pass it through
    /// [`sanitize_id`] defensively.
    pub fn module_dir(&self, id: &str) -> PathBuf {
        self.modules.join(sanitize_id(id))
    }

    /// `…\cache\downloads\<id>-<ver>.zip.partial` — the streaming download
    /// target. Fixed location (v2 §15 ③): downloads never land anywhere else.
    pub fn partial_download(&self, id: &str, version: &str) -> PathBuf {
        self.downloads.join(format!(
            "{}-{}.zip.partial",
            sanitize_id(id),
            sanitize_id(version)
        ))
    }

    /// `…\cache\manifest.json` — last synced manifest snapshot.
    pub fn manifest_cache(&self) -> PathBuf {
        self.cache.join("manifest.json")
    }

    /// `…\cache\manifest.etag` — the ETag that goes back out as
    /// `If-None-Match` on the next manifest GET (v2 §13).
    pub fn manifest_etag(&self) -> PathBuf {
        self.cache.join("manifest.etag")
    }

    /// `…\logs\install-<id>-<ver>.log` — one file per install attempt (§19).
    pub fn install_log(&self, id: &str, version: &str) -> PathBuf {
        self.logs.join(format!(
            "install-{}-{}.log",
            sanitize_id(id),
            sanitize_id(version)
        ))
    }

    /// `…\logs\run-<id>-<ts>.log` — one file per run (§14/§19). `ts` is an
    /// already-sanitized timestamp string supplied by the caller.
    pub fn run_log(&self, id: &str, ts: &str) -> PathBuf {
        self.logs
            .join(format!("run-{}-{}.log", sanitize_id(id), ts))
    }
}

/// Reduce an id/version to a single safe path segment: strip anything that is
/// not `[A-Za-z0-9._-]`. Manifest ids already satisfy this; the guard exists so
/// a malformed value can never traverse directories.
pub fn sanitize_id(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Lexical containment check used by the run whitelist (v2 §14): is `child`
/// underneath `base`? Resolves `.`/`..` textually, no filesystem access.
pub fn is_within(base: &Path, child: &Path) -> bool {
    use std::path::Component;
    let norm = |p: &Path| -> PathBuf {
        let mut out = PathBuf::new();
        for c in p.components() {
            match c {
                Component::ParentDir => {
                    out.pop();
                }
                Component::CurDir => {}
                other => out.push(other.as_os_str()),
            }
        }
        out
    };
    norm(child).starts_with(norm(base))
}
