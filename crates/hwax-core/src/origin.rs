//! Download URL allow-listing (v2 plan §15 ①, e2e §9 #2). A package URL is
//! permitted only if its **origin** (scheme + host + port) exactly matches one
//! of `config.allowed_origins`. This is the core of the "no free-form URL"
//! posture that keeps EDR/AV from flagging the agent.
//!
//! We parse origins by hand rather than pulling a URL crate — the surface is
//! tiny and a hand-rolled, deny-by-default parser is easy to audit.

use crate::error::{CoreError, Result};

/// Extract a normalized `scheme://host[:port]` origin. Default ports (443/80)
/// are stripped so `https://h:443` and `https://h` compare equal. Returns
/// `None` for anything it cannot parse — callers treat `None` as "deny".
pub fn origin_of(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    if scheme.is_empty()
        || !scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    {
        return None;
    }
    let scheme = scheme.to_ascii_lowercase();

    // Authority ends at the first '/', '?' or '#'.
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..end];
    if authority.is_empty() {
        return None;
    }

    // Drop any userinfo ("user:pass@").
    let host_port = authority.rsplit_once('@').map_or(authority, |(_, hp)| hp);

    // Split host:port — only treat the suffix as a port if it is all digits.
    let (host, port) = match host_port.rsplit_once(':') {
        Some((h, p)) if !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()) => (h, Some(p)),
        _ => (host_port, None),
    };
    if host.is_empty() {
        return None;
    }
    let host = host.to_ascii_lowercase();

    let port = match port {
        Some(p) if matches!((scheme.as_str(), p), ("https", "443") | ("http", "80")) => None,
        other => other.map(|p| p.to_string()),
    };

    Some(match port {
        Some(p) => format!("{scheme}://{host}:{p}"),
        None => format!("{scheme}://{host}"),
    })
}

/// True iff `url`'s origin matches one of `allowed_origins`.
pub fn is_allowed(url: &str, allowed_origins: &[String]) -> bool {
    let target = match origin_of(url) {
        Some(o) => o,
        None => return false,
    };
    allowed_origins
        .iter()
        .any(|a| origin_of(a).is_some_and(|ao| ao == target))
}

/// `is_allowed` as a `Result`, with a typed [`CoreError::OriginNotAllowed`]
/// (maps to `audit.kind=policy_denied`).
pub fn ensure_allowed(url: &str, allowed_origins: &[String]) -> Result<()> {
    if is_allowed(url, allowed_origins) {
        Ok(())
    } else {
        Err(CoreError::OriginNotAllowed(url.to_string()))
    }
}
