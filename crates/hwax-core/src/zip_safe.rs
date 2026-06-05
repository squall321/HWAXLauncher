//! Zip-slip-safe extraction (v2 plan §8, e2e §9 #4). Every entry name is
//! resolved manually — splitting on both `/` and `\`, rejecting `..`, absolute
//! roots, and drive/ADS (`:`) segments — and the joined path is then checked to
//! remain lexically under `dst`. A crafted archive cannot write outside the
//! staging directory.

use crate::error::{CoreError, Result};
use std::io;
use std::path::{Component, Path, PathBuf};

/// Extract `zip_path` into `dst`, rejecting any entry that would escape `dst`.
pub fn extract_zip_safe(zip_path: &Path, dst: &Path) -> Result<()> {
    let file = std::fs::File::open(zip_path)?;
    let mut zip = zip::ZipArchive::new(file)?;
    std::fs::create_dir_all(dst)?;

    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let name = entry.name().to_string();
        let out_path = match resolve_entry(dst, &name)? {
            Some(p) => p,
            None => continue, // empty / "." / root-only entry — nothing to write
        };
        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out = std::fs::File::create(&out_path)?;
            io::copy(&mut entry, &mut out)?;
        }
    }
    Ok(())
}

/// Audit a single entry name without extracting: `true` if it would be
/// rejected as escaping `dst`. Exposed for tests and pre-flight checks.
pub fn entry_escapes(dst: &Path, name: &str) -> bool {
    resolve_entry(dst, name).is_err()
}

/// Resolve a zip entry name to a concrete path under `dst`.
/// - `Ok(Some(path))` — safe, write here.
/// - `Ok(None)` — benign empty entry, skip.
/// - `Err(ZipSlip)` — the entry tried to escape; abort the whole extraction.
fn resolve_entry(dst: &Path, raw: &str) -> Result<Option<PathBuf>> {
    let mut out = dst.to_path_buf();
    let mut pushed = false;
    for seg in raw.split(['/', '\\']) {
        match seg {
            "" | "." => continue,
            ".." => return Err(CoreError::ZipSlip(raw.to_string())),
            s if s.contains(':') => return Err(CoreError::ZipSlip(raw.to_string())),
            s => {
                out.push(s);
                pushed = true;
            }
        }
    }
    if !pushed {
        return Ok(None);
    }
    if !lexically_within(dst, &out) {
        return Err(CoreError::ZipSlip(raw.to_string()));
    }
    Ok(Some(out))
}

/// Lexical containment check — resolves `.`/`..` textually (no filesystem
/// access, no symlink following) and verifies `candidate` is under `base`.
fn lexically_within(base: &Path, candidate: &Path) -> bool {
    candidate
        .starts_with(lexical_normalize(base).as_path()) // fast path
        || lexical_normalize(candidate).starts_with(lexical_normalize(base))
}

fn lexical_normalize(p: &Path) -> PathBuf {
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
}
