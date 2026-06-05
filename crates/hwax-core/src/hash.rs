//! SHA-256 over the downloaded installer bytes. The agent MUST verify this
//! against `manifest.programs[].package.sha256` before extracting or executing
//! anything (v2 plan §15 ②, e2e §9 #3).

use crate::error::{CoreError, Result};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

/// Lowercase hex SHA-256 of an in-memory slice.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

/// Lowercase hex SHA-256 of a file, streamed in 64 KiB chunks (installers are
/// hundreds of MB — never read them whole).
pub fn sha256_file(path: &Path) -> Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

/// Length-checked, branch-flat comparison of two hex digests. Inputs are
/// lowercased first (the schema mandates lowercase, but a defensive normalize
/// costs nothing).
pub fn digests_match(expected: &str, actual: &str) -> bool {
    let e = expected.trim().to_ascii_lowercase();
    let a = actual.trim().to_ascii_lowercase();
    let (e, a) = (e.as_bytes(), a.as_bytes());
    if e.len() != a.len() {
        return false;
    }
    let mut diff = 0u8;
    for i in 0..e.len() {
        diff |= e[i] ^ a[i];
    }
    diff == 0
}

/// Verify a file's digest against the manifest's expected value, returning a
/// typed [`CoreError::Sha256Mismatch`] on failure (carries both digests for the
/// audit `payload`).
pub fn verify_file(path: &Path, expected: &str) -> Result<()> {
    let actual = sha256_file(path)?;
    if digests_match(expected, &actual) {
        Ok(())
    } else {
        Err(CoreError::Sha256Mismatch {
            expected: expected.trim().to_ascii_lowercase(),
            actual,
        })
    }
}
