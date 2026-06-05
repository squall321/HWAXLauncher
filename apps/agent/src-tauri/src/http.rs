//! Authenticated HTTP client helpers (openapi `/api/v1/launcher-agents/*` and
//! `/api/v1/installers/{id}/download`). Every request carries the device JWT;
//! on a 401 we transparently `/refresh` once and retry (v2 §12).
//!
//! All URLs are built from `config.server` + a fixed route — NEVER a user-typed
//! URL. The only place a remote-supplied URL is used is the installer download,
//! and that URL is origin-allow-listed by the caller in [`crate::installer`]
//! before it ever reaches here (v2 §15 ①).

use crate::auth;
use crate::state::AGENT_VERSION;
use anyhow::{anyhow, bail, Context, Result};
use futures_util::StreamExt;
use hwax_core::manifest::Manifest;
use hwax_core::origin::origin_of;
use serde::Serialize;
use std::path::Path;
use tokio::io::AsyncWriteExt;

/// Outcome of a manifest GET (v2 §13: 200 → fresh body+etag, 304 → use cache).
pub enum ManifestFetch {
    /// 200: a new manifest plus its ETag (store both in the cache).
    Modified {
        manifest: Manifest,
        etag: Option<String>,
    },
    /// 304: the cached manifest is still current.
    NotModified,
}

/// Build a route URL under the launcher-agents prefix.
fn route(server: &str, path: &str) -> String {
    format!("{}{}", server.trim_end_matches('/'), path)
}

/// Attach the bearer token, run the request builder via `make`, and on a 401
/// refresh the token once and retry. `make` is a closure so we can rebuild a
/// fresh `RequestBuilder` (they are not `Clone` once a body is set).
async fn authed<F>(http: &reqwest::Client, server: &str, make: F) -> Result<reqwest::Response>
where
    F: Fn(&reqwest::Client, &str) -> reqwest::RequestBuilder,
{
    let token = auth::access_token()?
        .ok_or_else(|| anyhow!("not paired: no device_jwt in credential store"))?;
    let resp = make(http, &token)
        .send()
        .await
        .context("request send failed")?;
    if resp.status() != reqwest::StatusCode::UNAUTHORIZED {
        return Ok(resp);
    }
    // 401 → refresh once, then retry with the new token.
    let new_token = auth::refresh(http, server).await?;
    let resp = make(http, &new_token)
        .send()
        .await
        .context("request send failed after token refresh")?;
    Ok(resp)
}

/// `GET /api/v1/launcher-agents/manifest` with conditional `If-None-Match`.
pub async fn fetch_manifest(
    http: &reqwest::Client,
    server: &str,
    prev_etag: Option<&str>,
) -> Result<ManifestFetch> {
    let url = route(server, "/api/v1/launcher-agents/manifest?os=windows-x64");
    let etag_owned = prev_etag.map(|s| s.to_string());
    let resp = authed(http, server, |c, token| {
        let mut rb = c.get(&url).bearer_auth(token);
        if let Some(etag) = &etag_owned {
            rb = rb.header(reqwest::header::IF_NONE_MATCH, etag);
        }
        rb
    })
    .await?;

    if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(ManifestFetch::NotModified);
    }
    let resp = resp.error_for_status().context("manifest GET failed")?;
    let etag = resp
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let manifest: Manifest = resp.json().await.context("decoding manifest body")?;
    Ok(ManifestFetch::Modified { manifest, etag })
}

/// Stream an installer download to `dest` (a `*.partial` path), reporting
/// per-chunk progress through `on_progress(downloaded, total)`.
///
/// `url` MUST already be origin-allow-listed by the caller. Token handling is
/// explicit (e2e §9 #6): the device JWT is attached ONLY when `url` shares the
/// server's origin (the installer endpoint, which 302s to a presigned
/// object-storage URL). reqwest's default redirect policy then strips
/// `Authorization` on the cross-origin hop, so the token never reaches the
/// storage host — and because we never bearer a non-server origin in the first
/// place, a later change to the redirect policy cannot leak it either.
pub async fn download_to<P: FnMut(u64, Option<u64>)>(
    http: &reqwest::Client,
    server: &str,
    url: &str,
    dest: &Path,
    mut on_progress: P,
) -> Result<u64> {
    let url_owned = url.to_string();
    let same_origin_as_server = match (origin_of(server), origin_of(url)) {
        (Some(s), Some(u)) => s == u,
        _ => false,
    };
    let resp = if same_origin_as_server {
        authed(http, server, |c, token| {
            c.get(&url_owned).bearer_auth(token)
        })
        .await?
    } else {
        // Allow-listed, but not our server's origin (e.g. a manifest pointing
        // straight at a presigned URL) → fetch WITHOUT the device JWT.
        http.get(&url_owned)
            .send()
            .await
            .context("installer download failed")?
    }
    .error_for_status()
    .context("installer download failed")?;

    let total = resp.content_length();
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await.ok();
    }
    let mut file = tokio::fs::File::create(dest)
        .await
        .with_context(|| format!("creating {}", dest.display()))?;

    let mut downloaded: u64 = 0;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("network error mid-download")?;
        file.write_all(&chunk)
            .await
            .with_context(|| format!("writing {} (disk full?)", dest.display()))?;
        downloaded += chunk.len() as u64;
        on_progress(downloaded, total);
    }
    file.flush().await.ok();
    Ok(downloaded)
}

/// `POST /api/v1/launcher-agents/installs` — report one install outcome.
/// Accepts any `Serialize` body so callers pass `hwax_core::report::InstallReport`.
pub async fn post_install_report<T: Serialize>(
    http: &reqwest::Client,
    server: &str,
    report: &T,
) -> Result<()> {
    post_json(http, server, "/api/v1/launcher-agents/installs", report).await
}

/// `POST /api/v1/launcher-agents/audit` — submit a single audit event.
pub async fn post_audit<T: Serialize>(
    http: &reqwest::Client,
    server: &str,
    event: &T,
) -> Result<()> {
    post_json(http, server, "/api/v1/launcher-agents/audit", event).await
}

/// `POST /api/v1/launcher-agents/heartbeat` — 30-min alive ping (204, no body).
pub async fn post_heartbeat(
    http: &reqwest::Client,
    server: &str,
    hostname: Option<&str>,
    modules: &[(String, String)],
) -> Result<()> {
    #[derive(Serialize)]
    struct Mod<'a> {
        id: &'a str,
        version: &'a str,
    }
    #[derive(Serialize)]
    struct Beat<'a> {
        agent_version: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        hostname: Option<&'a str>,
        modules: Vec<Mod<'a>>,
    }
    let body = Beat {
        agent_version: AGENT_VERSION,
        hostname,
        modules: modules
            .iter()
            .map(|(id, version)| Mod { id, version })
            .collect(),
    };
    post_json(http, server, "/api/v1/launcher-agents/heartbeat", &body).await
}

/// Shared POST-JSON-with-auth helper. Treats any 2xx as success (the launcher
/// endpoints return 202/204).
async fn post_json<T: Serialize>(
    http: &reqwest::Client,
    server: &str,
    path: &str,
    body: &T,
) -> Result<()> {
    let url = route(server, path);
    let bytes = serde_json::to_vec(body).context("serializing request body")?;
    let resp = authed(http, server, |c, token| {
        c.post(&url)
            .bearer_auth(token)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(bytes.clone())
    })
    .await?;
    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        bail!("POST {path} → {status}: {text}");
    }
    Ok(())
}

/// A lightweight reachability probe for `health_check` — a HEAD/GET to the
/// server root that does not require auth to *succeed*, only to *connect*.
pub async fn server_reachable(http: &reqwest::Client, server: &str) -> bool {
    if server.is_empty() {
        return false;
    }
    let url = route(server, "/api/v1/launcher-agents/manifest");
    // We only care that the TLS handshake + HTTP response happened; a 401 is a
    // perfectly good signal the server is up.
    http.get(&url).send().await.is_ok()
}
