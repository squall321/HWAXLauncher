//! Filesystem-level guarantees: zip-slip rejection, atomic swap, rollback
//! (current.json-only), and GC (always protecting current + previous).

use hwax_core::error::CoreError;
use hwax_core::install::{gc_old_versions, perform_swap, rollback};
use hwax_core::store::{read_current, read_install_meta};
use hwax_core::zip_safe::{entry_escapes, extract_zip_safe};
use std::io::Write;
use std::path::Path;

// ── zip-slip ──────────────────────────────────────────────────────────────

#[test]
fn zip_slip_entries_are_rejected() {
    let dst = Path::new("C:/staging/koo/1.2.0"); // path need not exist for the lexical check
    assert!(entry_escapes(dst, "../evil.txt"));
    assert!(entry_escapes(dst, "a/../../evil"));
    assert!(entry_escapes(dst, "..\\..\\evil.dll"));
    assert!(entry_escapes(dst, "C:\\Windows\\System32\\evil.dll")); // drive segment
    assert!(entry_escapes(dst, "sub/dir/../../../escape"));
}

#[test]
fn benign_entries_are_allowed() {
    let dst = Path::new("C:/staging/koo/1.2.0");
    assert!(!entry_escapes(dst, "bin/KooPreprocessor.exe"));
    assert!(!entry_escapes(dst, "./resources/data.bin"));
    assert!(!entry_escapes(dst, "readme.txt"));
    // A leading slash is contained (treated as relative), not an escape.
    assert!(!entry_escapes(dst, "/already/contained"));
}

#[test]
fn extract_benign_zip_lands_under_dst() {
    let tmp = tempfile::tempdir().unwrap();
    let zip_path = tmp.path().join("pkg.zip");
    {
        let f = std::fs::File::create(&zip_path).unwrap();
        let mut w = zip::ZipWriter::new(f);
        // Stored: no compression feature needed; exercises the read path either way.
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        w.add_directory("bin/", opts).unwrap();
        w.start_file("bin/Tool.exe", opts).unwrap();
        w.write_all(b"MZfake").unwrap();
        w.start_file("readme.txt", opts).unwrap();
        w.write_all(b"hello").unwrap();
        w.finish().unwrap();
    }
    let dst = tmp.path().join("out");
    extract_zip_safe(&zip_path, &dst).unwrap();
    assert_eq!(std::fs::read(dst.join("bin/Tool.exe")).unwrap(), b"MZfake");
    assert_eq!(std::fs::read(dst.join("readme.txt")).unwrap(), b"hello");
}

// ── atomic swap + rollback ────────────────────────────────────────────────

fn install(modroot: &Path, version: &str, sha: &str, ts: &str) {
    let staging = modroot.join(format!("{version}.staging"));
    std::fs::create_dir_all(&staging).unwrap();
    std::fs::write(staging.join("Tool.exe"), version.as_bytes()).unwrap();
    perform_swap(modroot, &staging, version, sha, ts, 3).unwrap();
    assert!(!staging.exists(), "staging must be renamed away");
}

#[test]
fn swap_is_atomic_and_records_previous() {
    let tmp = tempfile::tempdir().unwrap();
    let modroot = tmp.path().join("modules/koo");

    install(&modroot, "1.0.0", "aa", "2026-06-05T10:00:00Z");
    let c1 = read_current(&modroot).unwrap();
    assert_eq!(c1.version, "1.0.0");
    assert!(c1.previous_version.is_none());
    assert_eq!(
        read_install_meta(&modroot.join("1.0.0")).unwrap().sha256,
        "aa"
    );
    assert_eq!(
        std::fs::read(modroot.join("1.0.0/Tool.exe")).unwrap(),
        b"1.0.0"
    );

    install(&modroot, "1.1.0", "bb", "2026-06-05T11:00:00Z");
    let c2 = read_current(&modroot).unwrap();
    assert_eq!(c2.version, "1.1.0");
    assert_eq!(c2.previous_version.as_deref(), Some("1.0.0"));
}

#[test]
fn rollback_rewrites_current_only_and_keeps_dirs() {
    let tmp = tempfile::tempdir().unwrap();
    let modroot = tmp.path().join("modules/koo");
    install(&modroot, "1.0.0", "aa", "2026-06-05T10:00:00Z");
    install(&modroot, "1.1.0", "bb", "2026-06-05T11:00:00Z");
    install(&modroot, "1.2.0", "cc", "2026-06-05T12:00:00Z");

    let rolled = rollback(&modroot, None, "2026-06-05T14:30:02Z").unwrap();
    assert_eq!(rolled.version, "1.1.0");
    assert_eq!(rolled.previous_version.as_deref(), Some("1.2.0"));
    assert_eq!(rolled.rolled_back_from.as_deref(), Some("1.2.0"));
    assert_eq!(rolled.sha256, "bb"); // carried from 1.1.0's install_meta

    // No directory is deleted by a rollback.
    for v in ["1.0.0", "1.1.0", "1.2.0"] {
        assert!(modroot.join(v).exists(), "{v} dir must survive rollback");
    }
}

#[test]
fn rollback_to_missing_version_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let modroot = tmp.path().join("modules/koo");
    install(&modroot, "1.0.0", "aa", "2026-06-05T10:00:00Z");
    let err = rollback(&modroot, Some("9.9.9"), "2026-06-05T14:30:02Z").unwrap_err();
    assert!(matches!(err, CoreError::VersionMissing(v) if v == "9.9.9"));
}

#[test]
fn rollback_with_no_previous_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let modroot = tmp.path().join("modules/koo");
    install(&modroot, "1.0.0", "aa", "2026-06-05T10:00:00Z");
    let err = rollback(&modroot, None, "ts").unwrap_err();
    assert!(matches!(err, CoreError::NoPreviousVersion(_)));
}

// ── GC ────────────────────────────────────────────────────────────────────

fn mkver(modroot: &Path, v: &str) {
    std::fs::create_dir_all(modroot.join(v)).unwrap();
}

#[test]
fn gc_keeps_newest_n() {
    let tmp = tempfile::tempdir().unwrap();
    let modroot = tmp.path().join("modules/koo");
    for v in ["1.0.0", "1.1.0", "1.2.0", "1.3.0", "1.4.0"] {
        mkver(&modroot, v);
    }
    let mut removed = gc_old_versions(&modroot, 3, &["1.4.0", "1.3.0"]).unwrap();
    removed.sort();
    assert_eq!(removed, vec!["1.0.0".to_string(), "1.1.0".to_string()]);
    assert!(modroot.join("1.2.0").exists());
    assert!(!modroot.join("1.1.0").exists());
}

#[test]
fn gc_always_protects_current_and_previous_even_if_oldest() {
    let tmp = tempfile::tempdir().unwrap();
    let modroot = tmp.path().join("modules/koo");
    for v in ["1.0.0", "1.1.0", "1.2.0", "1.3.0", "1.4.0"] {
        mkver(&modroot, v);
    }
    // keep_last_n=1 (newest=1.4.0), but protect the oldest 1.0.0 (pretend it's previous).
    let removed = gc_old_versions(&modroot, 1, &["1.4.0", "1.0.0"]).unwrap();
    assert!(
        modroot.join("1.0.0").exists(),
        "protected version must survive GC"
    );
    assert!(modroot.join("1.4.0").exists());
    assert!(!removed.contains(&"1.0.0".to_string()));
    assert!(removed.contains(&"1.2.0".to_string()));
}

#[test]
fn gc_ignores_non_semver_dirs() {
    let tmp = tempfile::tempdir().unwrap();
    let modroot = tmp.path().join("modules/koo");
    mkver(&modroot, "1.0.0");
    std::fs::create_dir_all(modroot.join("not-a-version")).unwrap();
    std::fs::write(modroot.join("current.json"), b"{}").unwrap();
    let removed = gc_old_versions(&modroot, 0, &[]).unwrap();
    assert_eq!(removed, vec!["1.0.0".to_string()]);
    assert!(
        modroot.join("not-a-version").exists(),
        "non-version dirs are left alone"
    );
    assert!(modroot.join("current.json").exists());
}
