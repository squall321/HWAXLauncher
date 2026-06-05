//! Atomic file writes: write a sibling temp file, then rename over the target.
//! On Windows a rename within the same volume is atomic and replaces the
//! destination — so a reader never observes a half-written `current.json`, and
//! a power loss leaves either the old file or the new one, never a torn one.

use crate::error::{CoreError, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQ: AtomicU64 = AtomicU64::new(0);

/// Atomically write `bytes` to `path`.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| CoreError::InvalidPath(path.display().to_string()))?;
    std::fs::create_dir_all(parent)?;
    let tmp = tmp_sibling(path);
    std::fs::write(&tmp, bytes)?;
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e.into())
        }
    }
}

/// Atomically write `value` as pretty JSON.
pub fn write_atomic_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    write_atomic(path, &bytes)
}

/// A unique hidden temp file in the same directory (same volume) as `path`,
/// so the subsequent rename is atomic. Unique per (process, sequence) to avoid
/// collisions even if two writes race.
fn tmp_sibling(path: &Path) -> PathBuf {
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let file = path.file_name().and_then(|f| f.to_str()).unwrap_or("hwax");
    path.with_file_name(format!(".{file}.{pid}.{seq}.tmp"))
}
