//! RFC3339 time formatting. The contract date-time fields are strings; we keep
//! the wire structs as `String` and format here so the output matches the
//! examples exactly (`2026-06-05T10:00:00Z` — seconds precision, `Z` suffix).

use chrono::{DateTime, SecondsFormat, Utc};

/// Format a UTC instant as `YYYY-MM-DDTHH:MM:SSZ`.
pub fn rfc3339(dt: DateTime<Utc>) -> String {
    dt.to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// `rfc3339(Utc::now())` — production callers use this; tests pass explicit
/// instants so payloads are deterministic.
pub fn now_rfc3339() -> String {
    rfc3339(Utc::now())
}
