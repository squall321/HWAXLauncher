//! Validates that the payloads `hwax-core` builds serialize to JSON that
//! conforms to the **real** contract schemas in `contracts/hwax-agent/`.
//! This is the Rust mirror of HEAXHub's `scripts/validate-e2e-examples.py`
//! ("15 JSON blocks validated") and the e2e example doc — if a schema and our
//! types ever drift, this test fails.

use chrono::{DateTime, Utc};
use hwax_core::audit::{AuditEvent, AuditKind, ClientMeta, Severity};
use hwax_core::manifest::{
    Entry, Lifecycle, Manifest, Package, PackageType, PostInstallCheck, Program, Requirements,
    UiHints, Visibility,
};
use hwax_core::report::{InstallReport, InstallStatus};
use hwax_core::state::{InstallPhase, ModuleState};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const AGENT: &str = "018f5a3b-4c2d-7e1f-9a8b-1234567890ab";
const KOO_SHA: &str = "9f1a5c1b2c3d4e5f60718293a4b5c6d7e8f9001122334455667788991011aabb";

fn schema_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/hwax-agent")
        .join(name)
}

fn assert_valid(schema_name: &str, instance: &Value) {
    let schema: Value =
        serde_json::from_slice(&std::fs::read(schema_path(schema_name)).expect("read schema"))
            .expect("parse schema");
    let compiled = jsonschema::JSONSchema::compile(&schema).expect("compile schema");
    // Collect owned messages within the borrow scope so the error iterator
    // (which borrows `compiled`) is fully consumed before we panic.
    let msgs: Vec<String> = match compiled.validate(instance) {
        Ok(()) => Vec::new(),
        Err(errors) => errors
            .map(|e| format!("  - {e} (at `{}`)", e.instance_path))
            .collect(),
    };
    if !msgs.is_empty() {
        panic!(
            "{schema_name} validation FAILED for:\n{}\n{}",
            serde_json::to_string_pretty(instance).unwrap(),
            msgs.join("\n")
        );
    }
}

fn t(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
}

fn koo_manifest() -> Manifest {
    Manifest {
        schema_version: Manifest::SCHEMA_VERSION,
        generated_at: "2026-06-05T10:00:00Z".into(),
        programs: vec![Program {
            id: "koo_preprocessor".into(),
            name: "Koo Preprocessor".into(),
            version: "1.2.0".into(),
            description: Some("STEP/IGES 임포트, 메시 품질 평가, LS-DYNA keyword 변환.".into()),
            category: Some("preprocessor".into()),
            released_at: Some("2026-06-04T12:00:00Z".into()),
            package: Package {
                kind: PackageType::Zip,
                url: "https://heaxhub.internal/api/v1/installers/7c1f0a30-0e9d-4b8f-9c61-f1c2a0b3d401/download".into(),
                sha256: KOO_SHA.into(),
                size_bytes: Some(188_743_680),
            },
            entry: Entry {
                executable: "bin/KooPreprocessor.exe".into(),
                args_template: Some(vec!["--workspace".into(), "{workspace}".into()]),
                working_dir: Some(String::new()),
            },
            requirements: Some(Requirements {
                requires_admin: false,
                min_windows: Some("10.0.19045".into()),
                depends_on: None,
            }),
            lifecycle: Some(Lifecycle {
                post_install_check: Some(PostInstallCheck {
                    executable: "bin/KooPreprocessor.exe".into(),
                    args: Some(vec!["--selftest".into(), "--json".into()]),
                    expected_stdout_regex: Some(r"^Koo Preprocessor 1\.2\.0".into()),
                }),
                rollback_on_failure: true,
            }),
            ui: Some(UiHints {
                icon_url: None,
                color_accent: Some("#f59e0b".into()),
                show_in_tray: true,
            }),
            tags: Some(vec!["cae".into(), "windows".into(), "internal".into()]),
            visibility: Some(Visibility::Company),
        }],
    }
}

#[test]
fn manifest_matches_schema() {
    assert_valid(
        "manifest.schema.json",
        &serde_json::to_value(koo_manifest()).unwrap(),
    );
}

#[test]
fn manifest_ignores_unknown_fields() {
    // Forward-compat: a newer server adds an optional field; an older agent
    // must still parse the manifest (PR-protocol §3 graceful degradation).
    let mut v = serde_json::to_value(koo_manifest()).unwrap();
    v["programs"][0]["future_field"] = json!("server-added");
    v["programs"][0]["package"]["mirror_url"] = json!("https://elsewhere/x");
    let parsed: Manifest =
        serde_json::from_value(v).expect("unknown fields must not break parsing");
    assert_eq!(parsed.programs[0].id, "koo_preprocessor");
}

#[test]
fn install_report_sha256_mismatch() {
    let r = InstallReport::new(
        AGENT,
        "koo_preprocessor",
        "1.2.0",
        InstallStatus::Failed,
        t("2026-06-05T10:11:00Z"),
        t("2026-06-05T10:13:42Z"),
    )
    .sha256_verified(false)
    .error("sha256 mismatch")
    .log_excerpt("expected=9f1a5c1b... actual=11aabbcc...\nstream bytes=188743680");
    assert_valid(
        "install-report.schema.json",
        &serde_json::to_value(&r).unwrap(),
    );
}

#[test]
fn install_report_post_check_failed() {
    let r = InstallReport::new(
        AGENT,
        "koo_preprocessor",
        "1.2.0",
        InstallStatus::Failed,
        t("2026-06-05T11:02:00Z"),
        t("2026-06-05T11:04:18Z"),
    )
    .exit_code(1)
    .sha256_verified(true)
    .error("post_install_check failed: selftest exited 1 before swap")
    .log_excerpt("KooPreprocessor.exe --selftest --json\n[ERR] missing dep: vcruntime140.dll\nProcess exit=1");
    assert_valid(
        "install-report.schema.json",
        &serde_json::to_value(&r).unwrap(),
    );
}

#[test]
fn install_report_partial_enospc() {
    let r = InstallReport::new(
        AGENT,
        "koo_preprocessor",
        "1.2.0",
        InstallStatus::Partial,
        t("2026-06-05T13:21:00Z"),
        t("2026-06-05T13:24:11Z"),
    )
    .sha256_verified(false)
    .error("ENOSPC")
    .log_excerpt("write cache/downloads/koo_preprocessor-1.2.0.zip.partial\nOSError 28 No space left on device");
    assert_valid(
        "install-report.schema.json",
        &serde_json::to_value(&r).unwrap(),
    );
}

#[test]
fn install_report_rolled_back() {
    let r = InstallReport::new(
        AGENT,
        "koo_preprocessor",
        "1.2.0",
        InstallStatus::RolledBack,
        t("2026-06-05T14:30:00Z"),
        t("2026-06-05T14:30:02Z"),
    )
    .sha256_verified(true)
    .previous_version("1.1.0")
    .log_excerpt("user_action=rollback target=1.1.0\nswap current.json 1.2.0 -> 1.1.0\nrolled_back_from=1.2.0");
    assert_valid(
        "install-report.schema.json",
        &serde_json::to_value(&r).unwrap(),
    );
}

#[test]
fn install_report_caps_oversized_fields() {
    let r = InstallReport::new(
        AGENT,
        "x",
        "1.0.0",
        InstallStatus::Failed,
        t("2026-06-05T10:00:00Z"),
        t("2026-06-05T10:00:01Z"),
    )
    .error("e".repeat(5000))
    .log_excerpt("y".repeat(40000));
    let v = serde_json::to_value(&r).unwrap();
    assert!(v["error"].as_str().unwrap().chars().count() <= 2048);
    assert!(v["log_excerpt"].as_str().unwrap().chars().count() <= 16384);
    assert_valid("install-report.schema.json", &v);
}

fn win_meta() -> ClientMeta {
    ClientMeta::windows("10.0.22631", "1.0.0", "WS-CAE-014")
}

#[test]
fn audit_enrollment() {
    let a = AuditEvent::new(
        AGENT,
        AuditKind::Enrollment,
        t("2026-06-05T09:00:00Z"),
        Severity::Info,
    )
    .payload(json!({ "hostname": "WS-CAE-014", "enrolled_by_user": "alice@heax.example.com" }))
    .client_meta(win_meta());
    assert_valid(
        "audit-event.schema.json",
        &serde_json::to_value(&a).unwrap(),
    );
}

#[test]
fn audit_install_success() {
    let a = AuditEvent::new(
        AGENT,
        AuditKind::Install,
        t("2026-06-05T10:09:55Z"),
        Severity::Info,
    )
    .app("koo_preprocessor", "1.2.0")
    .payload(json!({
        "outcome": "success", "duration_ms": 132480,
        "package_size_bytes": 188743680, "previous_version": "1.1.0"
    }))
    .client_meta(win_meta());
    assert_valid(
        "audit-event.schema.json",
        &serde_json::to_value(&a).unwrap(),
    );
}

#[test]
fn audit_sha256_mismatch() {
    let a = AuditEvent::new(AGENT, AuditKind::Sha256Mismatch, t("2026-06-05T10:13:42Z"), Severity::Error)
        .app("koo_preprocessor", "1.2.0")
        .payload(json!({
            "expected": KOO_SHA,
            "actual": "11aabbccddeeff00112233445566778899aabbccddeeff00112233445566ffff",
            "size_bytes": 188743680,
            "source_url": "https://heaxhub.internal/api/v1/installers/7c1f0a30-0e9d-4b8f-9c61-f1c2a0b3d401/download"
        }))
        .client_meta(win_meta());
    assert_valid(
        "audit-event.schema.json",
        &serde_json::to_value(&a).unwrap(),
    );
}

#[test]
fn audit_download_failed() {
    let a = AuditEvent::new(AGENT, AuditKind::DownloadFailed, t("2026-06-05T12:05:30Z"), Severity::Warn)
        .app("koo_preprocessor", "1.2.0")
        .payload(json!({ "reason": "read_timeout", "bytes_read": 24117248, "size_bytes": 188743680, "elapsed_sec": 330 }))
        .client_meta(win_meta());
    assert_valid(
        "audit-event.schema.json",
        &serde_json::to_value(&a).unwrap(),
    );
}

#[test]
fn audit_rollback() {
    let a = AuditEvent::new(
        AGENT,
        AuditKind::Rollback,
        t("2026-06-05T14:30:02Z"),
        Severity::Warn,
    )
    .app("koo_preprocessor", "1.1.0")
    .payload(json!({
        "rolled_back_from": "1.2.0", "rolled_back_to": "1.1.0",
        "trigger": "user_click", "reason": "av suspected 1.2.0 dll"
    }))
    .client_meta(win_meta());
    assert_valid(
        "audit-event.schema.json",
        &serde_json::to_value(&a).unwrap(),
    );
}

#[test]
fn audit_av_block() {
    let a = AuditEvent::new(AGENT, AuditKind::AvBlock, t("2026-06-05T14:25:11Z"), Severity::Error)
        .app("koo_preprocessor", "1.2.0")
        .payload(json!({
            "av_product": "Kaspersky Endpoint Security",
            "detection_name": "HEUR:Trojan.Win32.Generic",
            "quarantined_path": "C:/Users/koo/AppData/Local/HWAXAgent/modules/koo_preprocessor/1.2.0/bin/KooPreprocessor.exe",
            "action": "quarantine"
        }))
        .client_meta(win_meta());
    assert_valid(
        "audit-event.schema.json",
        &serde_json::to_value(&a).unwrap(),
    );
}

#[test]
fn audit_payload_non_object_is_wrapped() {
    // A careless caller passing a bare array must still produce an object payload.
    let a = AuditEvent::new(
        AGENT,
        AuditKind::Heartbeat,
        t("2026-06-05T10:00:00Z"),
        Severity::Info,
    )
    .payload(json!([1, 2, 3]));
    let v = serde_json::to_value(&a).unwrap();
    assert!(v["payload"].is_object(), "payload must be an object");
    assert_valid("audit-event.schema.json", &v);
}

#[test]
fn enum_wire_strings_are_exact() {
    // Guards the serde rename_all rules against the contract enum strings.
    assert_eq!(
        serde_json::to_value(InstallStatus::RolledBack).unwrap(),
        json!("rolled_back")
    );
    assert_eq!(
        serde_json::to_value(InstallStatus::Partial).unwrap(),
        json!("partial")
    );
    assert_eq!(
        serde_json::to_value(AuditKind::Sha256Mismatch).unwrap(),
        json!("sha256_mismatch")
    );
    assert_eq!(
        serde_json::to_value(AuditKind::AvBlock).unwrap(),
        json!("av_block")
    );
    assert_eq!(
        serde_json::to_value(AuditKind::DownloadFailed).unwrap(),
        json!("download_failed")
    );
    assert_eq!(
        serde_json::to_value(AuditKind::PolicyDenied).unwrap(),
        json!("policy_denied")
    );
    assert_eq!(
        serde_json::to_value(PackageType::Zip).unwrap(),
        json!("zip")
    );
    assert_eq!(
        serde_json::to_value(PackageType::Msix).unwrap(),
        json!("msix")
    );
    assert_eq!(
        serde_json::to_value(Visibility::Company).unwrap(),
        json!("company")
    );
    assert_eq!(serde_json::to_value(Severity::Warn).unwrap(), json!("warn"));
    assert_eq!(
        serde_json::to_value(ModuleState::RolledBack).unwrap(),
        json!("rolled_back")
    );
    assert_eq!(
        serde_json::to_value(ModuleState::NotInstalled).unwrap(),
        json!("not_installed")
    );
    assert_eq!(
        serde_json::to_value(InstallPhase::Verify).unwrap(),
        json!("verify")
    );
}
