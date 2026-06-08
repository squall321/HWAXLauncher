//! Device identity & token lifecycle (v2 plan §12, openapi `enroll`/`refresh`).
//!
//! The access token (3600 s) and refresh token (30 d, rotated on every refresh)
//! live ONLY in the Windows Credential Manager via the `keyring` crate —
//! NEVER in a plaintext file (v2 §5/§12, e2e §9 #6). `config.json` carries just
//! the non-secret `agent_id` and `server`.
//!
//! Credential Manager keys (service = "HWAXAgent"):
//!   - "device_jwt"     → current access_token
//!   - "refresh_token"  → current refresh_token
//!
//! Pairing: the operator approves the device in the HEAXHub web UI and hands the
//! user a single-use `enrollment_token`; the agent POSTs it to
//! `/api/v1/launcher-agents/enroll` and stores the returned pair.

use anyhow::{anyhow, Context, Result};
use keyring::Entry;
use serde::{Deserialize, Serialize};

/// Credential Manager service name for all agent secrets.
const KEYRING_SERVICE: &str = "HWAXAgent";
const KEY_ACCESS: &str = "device_jwt";
const KEY_REFRESH: &str = "refresh_token";

/// `POST /api/v1/launcher-agents/enroll` request body (openapi).
#[derive(Debug, Serialize)]
struct EnrollRequest<'a> {
    enrollment_token: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    hostname: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_version: Option<&'a str>,
}

/// `EnrollmentResult` (openapi components/schemas).
#[derive(Debug, Deserialize)]
pub struct EnrollmentResult {
    pub agent_id: String,
    pub access_token: String,
    pub refresh_token: String,
    #[allow(dead_code)]
    pub expires_in: i64,
}

/// `POST /api/v1/launcher-agents/refresh` request body.
#[derive(Debug, Serialize)]
struct RefreshRequest<'a> {
    refresh_token: &'a str,
}

/// `RefreshResult` (openapi). `refresh_token` is present whenever rotation
/// occurred (always, in Phase 1).
#[derive(Debug, Deserialize)]
pub struct RefreshResult {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[allow(dead_code)]
    pub expires_in: i64,
}

fn entry(key: &str) -> Result<Entry> {
    Entry::new(KEYRING_SERVICE, key)
        .with_context(|| format!("opening credential store entry {KEYRING_SERVICE}/{key}"))
}

/// Read the current access token from the credential store, if paired.
pub fn access_token() -> Result<Option<String>> {
    read_secret(KEY_ACCESS)
}

/// Read the current refresh token, if any.
pub fn refresh_token() -> Result<Option<String>> {
    read_secret(KEY_REFRESH)
}

fn read_secret(key: &str) -> Result<Option<String>> {
    match entry(key)?.get_password() {
        Ok(v) => Ok(Some(v)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(anyhow!("reading credential {key}: {e}")),
    }
}

fn store_secret(key: &str, value: &str) -> Result<()> {
    entry(key)?
        .set_password(value)
        .with_context(|| format!("writing credential {key}"))
}

fn delete_secret(key: &str) -> Result<()> {
    match entry(key)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(anyhow!("deleting credential {key}: {e}")),
    }
}

/// Persist a freshly issued access/refresh pair.
pub fn store_tokens(access: &str, refresh: &str) -> Result<()> {
    store_secret(KEY_ACCESS, access)?;
    store_secret(KEY_REFRESH, refresh)?;
    Ok(())
}

/// Replace just the access token (and, if rotation occurred, the refresh token)
/// after a successful `/refresh`.
pub fn store_refreshed(access: &str, refresh: Option<&str>) -> Result<()> {
    store_secret(KEY_ACCESS, access)?;
    if let Some(r) = refresh {
        store_secret(KEY_REFRESH, r)?;
    }
    Ok(())
}

/// Wipe both secrets — used by "re-pair" so a new enrollment starts clean.
pub fn clear_tokens() -> Result<()> {
    delete_secret(KEY_ACCESS)?;
    delete_secret(KEY_REFRESH)?;
    Ok(())
}

/// Exchange a single-use enrollment token for a device JWT pair and persist it.
/// Returns the `agent_id` the caller writes into `config.json`.
///
/// `server` is the base URL (e.g. `https://heaxhub.internal`); the route is
/// always `/api/v1/launcher-agents/enroll` (never a user-typed URL).
pub async fn enroll(
    http: &reqwest::Client,
    server: &str,
    enrollment_token: &str,
    hostname: Option<&str>,
    agent_version: &str,
) -> Result<EnrollmentResult> {
    let url = format!(
        "{}/api/v1/launcher-agents/enroll",
        server.trim_end_matches('/')
    );
    let body = EnrollRequest {
        enrollment_token,
        hostname,
        agent_version: Some(agent_version),
    };
    let resp = http
        .post(&url)
        .json(&body)
        .send()
        .await
        .context("enroll request failed")?;
    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(anyhow!(
            "enrollment_token unknown, expired, or already redeemed"
        ));
    }
    if resp.status() == reqwest::StatusCode::FORBIDDEN {
        // The server returns 403 when the registered agent is device_kind=service
        // rather than launcher — give the operator an actionable message instead
        // of a generic HTTP error.
        return Err(anyhow!(
            "this device is registered as a service agent, not a launcher — \
             re-register it in HEAXHub as a launcher device (device_kind=launcher)"
        ));
    }
    let resp = resp
        .error_for_status()
        .context("enroll returned an error status")?;
    let result: EnrollmentResult = resp.json().await.context("decoding EnrollmentResult")?;
    store_tokens(&result.access_token, &result.refresh_token)?;
    Ok(result)
}

/// Rotate the refresh token and obtain a fresh access token. Called on a 401
/// from any authed endpoint (see [`crate::http`]). On success the credential
/// store is updated and the new access token is returned.
pub async fn refresh(http: &reqwest::Client, server: &str) -> Result<String> {
    let refresh = refresh_token()?
        .ok_or_else(|| anyhow!("no refresh_token in credential store; re-pairing required"))?;
    let url = format!(
        "{}/api/v1/launcher-agents/refresh",
        server.trim_end_matches('/')
    );
    let resp = http
        .post(&url)
        .json(&RefreshRequest {
            refresh_token: &refresh,
        })
        .send()
        .await
        .context("refresh request failed")?;
    // A 401 means the submitted refresh token was rejected. Refresh tokens are
    // single-use and rotating, so a replayed/expired one makes the server revoke
    // the WHOLE chain — both stored secrets are now permanently dead. Wipe them
    // so we never replay a revoked token, and force a clean re-pairing.
    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        let _ = clear_tokens();
        return Err(anyhow!(
            "refresh token revoked or expired — re-pairing required"
        ));
    }
    let resp = resp.error_for_status().context("refresh rejected")?;
    let result: RefreshResult = resp.json().await.context("decoding RefreshResult")?;
    // Phase 1 ALWAYS rotates (openapi: "Present when rotation occurred (always,
    // in Phase 1)"). After a 200 the token we just sent is dead, so a response
    // without a replacement would strand the agent on the next refresh — treat a
    // missing rotated token as a protocol error rather than silently keeping the
    // now-revoked one.
    let new_refresh = result.refresh_token.ok_or_else(|| {
        anyhow!("/refresh succeeded but did not rotate the refresh_token")
    })?;
    store_refreshed(&result.access_token, Some(&new_refresh))?;
    Ok(result.access_token)
}
