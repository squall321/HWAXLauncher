//! Load/save of `config.json` using the `hwax_core::config::AgentConfig` type
//! (the single source of truth for the on-disk shape). Writes go through
//! `hwax_core::atomic` so a power loss never leaves a torn config file.
//!
//! Secrets are NEVER here — the device JWT and refresh token live in the Windows
//! Credential Manager (see [`crate::auth`]). `config.json` only holds
//! `agent_id`, `server`, and non-secret preferences (v2 plan §5/§26.4).

use crate::paths::Paths;
use anyhow::{Context, Result};
use hwax_core::atomic::write_atomic_json;
use hwax_core::config::AgentConfig;

/// Load `config.json`. Returns `Ok(None)` when the file does not exist yet
/// (fresh, unpaired install) so the caller can construct an empty placeholder.
pub fn load(paths: &Paths) -> Result<Option<AgentConfig>> {
    let path = &paths.config_file;
    if !path.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let cfg: AgentConfig =
        serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", path.display()))?;
    Ok(Some(cfg))
}

/// Load the config, or synthesize an empty, unpaired placeholder if absent.
/// The placeholder has empty `server`/`agent_id`; the UI treats that as
/// "needs pairing" (`AgentStatus.paired == false`).
pub fn load_or_default(paths: &Paths) -> Result<AgentConfig> {
    Ok(load(paths)?.unwrap_or_else(|| AgentConfig::new(String::new(), String::new())))
}

/// Atomically persist `config.json` (tempfile + same-volume rename).
pub fn save(paths: &Paths, config: &AgentConfig) -> Result<()> {
    write_atomic_json(&paths.config_file, config)
        .with_context(|| format!("writing {}", paths.config_file.display()))?;
    Ok(())
}
