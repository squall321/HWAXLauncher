//! Unit coverage for the origin allow-list, SHA-256, version-state decisions,
//! and config defaults.

use hwax_core::config::AgentConfig;
use hwax_core::error::CoreError;
use hwax_core::hash::{digests_match, sha256_hex, verify_file};
use hwax_core::origin::{ensure_allowed, is_allowed};
use hwax_core::state::{decide_state, is_newer, ModuleState};

// ── origin allow-list ─────────────────────────────────────────────────────

#[test]
fn origin_allow_list_is_exact() {
    let allowed = vec!["https://heaxhub.internal".to_string()];

    assert!(is_allowed(
        "https://heaxhub.internal/api/v1/installers/7c1f0a30/download",
        &allowed
    ));
    // default port is equivalent
    assert!(is_allowed("https://heaxhub.internal:443/x", &allowed));

    // different host / scheme / port / suffix are all rejected
    assert!(!is_allowed("https://evil.example/x", &allowed));
    assert!(!is_allowed("http://heaxhub.internal/x", &allowed));
    assert!(!is_allowed("https://heaxhub.internal:8443/x", &allowed));
    assert!(!is_allowed("https://heaxhub.internal.evil.com/x", &allowed));

    // unparseable / non-http schemes are rejected
    assert!(!is_allowed("not a url", &allowed));
    assert!(!is_allowed("file:///etc/passwd", &allowed));
}

#[test]
fn origin_explicit_port_must_match() {
    let allowed = vec!["https://hub:8443".to_string()];
    assert!(is_allowed("https://hub:8443/p", &allowed));
    assert!(!is_allowed("https://hub/p", &allowed));
}

#[test]
fn origin_normalizes_userinfo_and_case() {
    let allowed = vec!["https://heaxhub.internal".to_string()];
    // userinfo is stripped; host compare is case-insensitive
    assert!(is_allowed("https://user:pass@heaxhub.internal/x", &allowed));
    assert!(is_allowed("https://HEAXHUB.INTERNAL/x", &allowed));
    // a look-alike suffix must NOT match
    assert!(!is_allowed("https://heaxhub.internal.evil.com/x", &allowed));
}

#[test]
fn ensure_allowed_returns_typed_error() {
    let allowed = vec!["https://heaxhub.internal".to_string()];
    assert!(ensure_allowed("https://heaxhub.internal/x", &allowed).is_ok());
    assert!(matches!(
        ensure_allowed("https://evil/x", &allowed),
        Err(CoreError::OriginNotAllowed(_))
    ));
}

// ── SHA-256 ───────────────────────────────────────────────────────────────

#[test]
fn sha256_known_vector() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn digest_compare_is_case_insensitive_and_length_checked() {
    assert!(digests_match("ABCDEF1234", "abcdef1234"));
    assert!(!digests_match("aa", "aaa"));
    assert!(!digests_match("dead", "beef"));
}

#[test]
fn verify_file_ok_and_mismatch() {
    let tmp = tempfile::tempdir().unwrap();
    let f = tmp.path().join("x.bin");
    std::fs::write(&f, b"abc").unwrap();

    assert!(verify_file(
        &f,
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    )
    .is_ok());

    let err = verify_file(&f, &"0".repeat(64)).unwrap_err();
    match err {
        CoreError::Sha256Mismatch { expected, actual } => {
            assert_eq!(expected, "0".repeat(64));
            assert_eq!(
                actual,
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
            );
        }
        other => panic!("expected Sha256Mismatch, got {other:?}"),
    }
}

// ── version → state ───────────────────────────────────────────────────────

#[test]
fn version_state_decisions() {
    assert_eq!(
        decide_state(None, "1.0.0").unwrap(),
        ModuleState::NotInstalled
    );
    assert_eq!(
        decide_state(Some("1.0.0"), "1.0.0").unwrap(),
        ModuleState::Installed
    );
    assert_eq!(
        decide_state(Some("1.0.0"), "1.2.0").unwrap(),
        ModuleState::Outdated
    );
    // local newer than server (e.g. after a downgrade-server) is not "outdated"
    assert_eq!(
        decide_state(Some("2.0.0"), "1.2.0").unwrap(),
        ModuleState::Installed
    );

    assert!(is_newer("1.2.0", "1.1.0").unwrap());
    assert!(!is_newer("1.0.0", "1.0.0").unwrap());
    assert!(matches!(
        is_newer("not-semver", "1.0.0"),
        Err(CoreError::Semver(_, _))
    ));

    // SemVer pre-release ordering: a released 1.2.0 is newer than 1.2.0-beta.1,
    // so a locally-installed beta is "outdated" once the stable ships.
    assert!(is_newer("1.2.0", "1.2.0-beta.1").unwrap());
    assert_eq!(
        decide_state(Some("1.2.0-beta.1"), "1.2.0").unwrap(),
        ModuleState::Outdated
    );
}

// ── config defaults ───────────────────────────────────────────────────────

#[test]
fn config_defaults_and_origin_fallback() {
    let c: AgentConfig =
        serde_json::from_str(r#"{"server":"https://h","agent_id":"ag_1"}"#).unwrap();
    assert!(c.auto_update);
    assert!(!c.start_on_boot);
    assert_eq!(c.log_level, "info");
    assert_eq!(c.keep_last_n_versions, 3);
    assert_eq!(c.sync_interval_min, 30);
    assert_eq!(c.channel, "stable");
    // empty allowed_origins falls back to [server] — never "anywhere"
    assert_eq!(c.effective_allowed_origins(), vec!["https://h".to_string()]);

    let c2 = AgentConfig::new("https://hub.internal", "ag_2");
    assert_eq!(c2.allowed_origins, vec!["https://hub.internal".to_string()]);
}
