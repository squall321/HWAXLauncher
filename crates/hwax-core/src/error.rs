//! Crate error type. Kept small and mapped 1:1 to the failure modes the
//! installer/store/verify paths actually produce, so callers can branch on
//! them and emit the right `audit-event.kind` / `install-report.status`.

use thiserror::Error;

pub type Result<T> = std::result::Result<T, CoreError>;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),

    /// SHA-256 of the downloaded bytes did not match the manifest.
    /// Maps to `install-report.sha256_verified=false` + `audit.kind=sha256_mismatch`.
    #[error("sha256 mismatch: expected {expected}, got {actual}")]
    Sha256Mismatch { expected: String, actual: String },

    /// A download URL whose origin is not in `config.allowed_origins`.
    /// Maps to `audit.kind=policy_denied`.
    #[error("origin not allowed: {0}")]
    OriginNotAllowed(String),

    /// A zip entry tried to escape the extraction root (zip-slip).
    #[error("zip entry escapes destination: {0}")]
    ZipSlip(String),

    #[error("invalid path: {0}")]
    InvalidPath(String),

    /// Rollback/run target version directory is no longer on disk (GC'd).
    #[error("version {0} is not on disk (garbage-collected?)")]
    VersionMissing(String),

    #[error("no previous_version recorded for {0}")]
    NoPreviousVersion(String),

    #[error("invalid semver '{0}': {1}")]
    Semver(String, String),

    #[error("{0}")]
    Other(String),
}

impl CoreError {
    pub fn other(msg: impl Into<String>) -> Self {
        CoreError::Other(msg.into())
    }
}
