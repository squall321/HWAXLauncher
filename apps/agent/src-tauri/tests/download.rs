//! Integration test for the network layer (`src/http.rs::download_to`) — the one
//! surface that the headless `hwax-core` tests can't reach. Drives the real
//! reqwest streaming path against a mock HEAXHub (`tiny_http`):
//!   - streams bytes to the `.partial` dest, reports progress + content-length,
//!   - follows a 302 redirect (the installer endpoint → presigned URL pattern),
//!   - surfaces a 404 as an error (`error_for_status`),
//!   - and — critically — sends NO `Authorization` header to a cross-origin host
//!     (the §15/§9 hardening: the device JWT must never reach object storage).

use hwax_agent_lib::http::download_to;
use std::sync::mpsc;
use std::thread;

/// Mock HEAXHub on a random port. The returned receiver yields `true` for every
/// request that arrived WITH an `Authorization` header.
fn spawn_mock(body: Vec<u8>) -> (String, mpsc::Receiver<bool>) {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let port = server.server_addr().to_ip().unwrap().port();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for req in server.incoming_requests() {
            let had_auth = req
                .headers()
                .iter()
                .any(|h| h.field.to_string().eq_ignore_ascii_case("authorization"));
            let _ = tx.send(had_auth);
            let resp = match req.url() {
                "/pkg.zip" => tiny_http::Response::from_data(body.clone()).boxed(),
                "/redirect" => tiny_http::Response::empty(302)
                    .with_header(
                        tiny_http::Header::from_bytes(
                            &b"Location"[..],
                            format!("http://127.0.0.1:{port}/pkg.zip").into_bytes(),
                        )
                        .unwrap(),
                    )
                    .boxed(),
                _ => tiny_http::Response::empty(404).boxed(),
            };
            let _ = req.respond(resp);
        }
    });
    (format!("http://127.0.0.1:{port}"), rx)
}

fn client() -> reqwest::Client {
    reqwest::Client::builder().build().unwrap()
}

#[tokio::test]
async fn download_streams_and_omits_auth_cross_origin() {
    let body = vec![0xABu8; 4096];
    let (base, rx) = spawn_mock(body.clone());
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join("pkg.zip.partial");

    let mut calls = 0u32;
    let mut last_total: Option<u64> = None;
    // server origin (https://heaxhub.internal) != url origin (the mock) ⇒
    // download_to must do a plain GET with NO bearer.
    let n = download_to(
        &client(),
        "https://heaxhub.internal",
        &format!("{base}/pkg.zip"),
        &dest,
        |_done, total| {
            calls += 1;
            last_total = total;
        },
    )
    .await
    .expect("download should succeed");

    assert_eq!(n as usize, body.len());
    assert_eq!(std::fs::read(&dest).unwrap(), body);
    assert!(calls >= 1, "progress callback should fire");
    assert_eq!(
        last_total,
        Some(body.len() as u64),
        "content-length surfaced"
    );

    let had_auth = rx.recv().unwrap();
    assert!(
        !had_auth,
        "device JWT must NOT be sent to a cross-origin host"
    );
}

#[tokio::test]
async fn download_follows_redirect() {
    let body = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
    let (base, _rx) = spawn_mock(body.clone());
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join("p.partial");

    let n = download_to(
        &client(),
        "https://other",
        &format!("{base}/redirect"),
        &dest,
        |_, _| {},
    )
    .await
    .expect("302 should be followed to the payload");
    assert_eq!(n as usize, body.len());
    assert_eq!(std::fs::read(&dest).unwrap(), body);
}

#[tokio::test]
async fn download_404_is_error() {
    let (base, _rx) = spawn_mock(vec![0u8; 8]);
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join("p.partial");

    let r = download_to(
        &client(),
        "https://other",
        &format!("{base}/missing"),
        &dest,
        |_, _| {},
    )
    .await;
    assert!(
        r.is_err(),
        "404 must surface as an error (error_for_status)"
    );
}
