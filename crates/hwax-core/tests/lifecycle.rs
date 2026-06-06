//! End-to-end integration of the pure install pipeline: build a REAL zip (the
//! bytes the agent would have streamed to `.partial`), verify its SHA-256,
//! zip-slip-safe extract to `<ver>.staging`, atomic swap, GC, and rollback —
//! exercising `hash` + `zip_safe` + `install` + `store` together the way
//! `apps/agent/src-tauri/src/installer.rs` wires them, minus the network.

use hwax_core::error::CoreError;
use hwax_core::hash::{sha256_file, verify_file};
use hwax_core::install::rollback;
use hwax_core::store::{read_current, read_install_meta};
use hwax_core::zip_safe::extract_zip_safe;
use std::io::Write;
use std::path::Path;

fn make_zip(path: &Path, files: &[(&str, &[u8])]) {
    let f = std::fs::File::create(path).unwrap();
    let mut w = zip::ZipWriter::new(f);
    // Stored: no compression feature required; extract reads it the same way.
    let opts =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, bytes) in files {
        w.start_file(*name, opts).unwrap();
        w.write_all(bytes).unwrap();
    }
    w.finish().unwrap();
}

/// One full per-version install: "download" → verify → extract → atomic swap.
/// Returns the sha256 the manifest would have carried.
fn install_version(
    modroot: &Path,
    downloads: &Path,
    version: &str,
    exe_bytes: &[u8],
    keep: usize,
) -> String {
    let partial = downloads.join(format!("koo-{version}.zip.partial"));
    make_zip(
        &partial,
        &[
            ("bin/KooPreprocessor.exe", exe_bytes),
            ("resources/data.bin", b"x"),
        ],
    );

    let sha = sha256_file(&partial).unwrap();
    verify_file(&partial, &sha).unwrap(); // matches the manifest value

    let staging = modroot.join(format!("{version}.staging"));
    let _ = std::fs::remove_dir_all(&staging);
    extract_zip_safe(&partial, &staging).unwrap();
    assert!(staging.join("bin/KooPreprocessor.exe").exists());

    let current = hwax_core::install::perform_swap(
        modroot,
        &staging,
        version,
        &sha,
        "2026-06-05T10:00:00Z",
        keep,
    )
    .unwrap();
    let _ = std::fs::remove_file(&partial);
    assert_eq!(current.version, version);
    assert!(!staging.exists(), "staging renamed away by the swap");
    sha
}

#[test]
fn full_install_lifecycle() {
    let tmp = tempfile::tempdir().unwrap();
    let modroot = tmp.path().join("modules/koo");
    let downloads = tmp.path().join("cache/downloads");
    std::fs::create_dir_all(&downloads).unwrap();

    // First install.
    let sha10 = install_version(&modroot, &downloads, "1.0.0", b"v1.0.0-exe", 3);
    let c = read_current(&modroot).unwrap();
    assert_eq!(c.version, "1.0.0");
    assert!(c.previous_version.is_none());
    assert_eq!(
        read_install_meta(&modroot.join("1.0.0")).unwrap().sha256,
        sha10
    );
    assert_eq!(
        std::fs::read(modroot.join("1.0.0/bin/KooPreprocessor.exe")).unwrap(),
        b"v1.0.0-exe"
    );

    // Three more updates; keep_last_n=3 must evict the oldest non-protected.
    install_version(&modroot, &downloads, "1.1.0", b"v1.1.0-exe", 3);
    install_version(&modroot, &downloads, "1.2.0", b"v1.2.0-exe", 3);
    install_version(&modroot, &downloads, "1.3.0", b"v1.3.0-exe", 3);
    assert!(
        !modroot.join("1.0.0").exists(),
        "1.0.0 GC'd (not current/prev, beyond keep_last_n)"
    );
    for v in ["1.1.0", "1.2.0", "1.3.0"] {
        assert!(modroot.join(v).exists());
    }
    let c = read_current(&modroot).unwrap();
    assert_eq!(c.version, "1.3.0");
    assert_eq!(c.previous_version.as_deref(), Some("1.2.0"));

    // Rollback to previous (1.2.0): only current.json changes; dirs survive.
    let r = rollback(&modroot, None, "2026-06-05T14:00:00Z").unwrap();
    assert_eq!(r.version, "1.2.0");
    assert_eq!(r.rolled_back_from.as_deref(), Some("1.3.0"));
    let active = read_current(&modroot).unwrap().version;
    assert_eq!(active, "1.2.0");
    assert_eq!(
        std::fs::read(modroot.join("1.2.0/bin/KooPreprocessor.exe")).unwrap(),
        b"v1.2.0-exe"
    );

    // Rollback to a GC'd version is refused.
    assert!(matches!(
        rollback(&modroot, Some("1.0.0"), "ts"),
        Err(CoreError::VersionMissing(v)) if v == "1.0.0"
    ));

    // Re-installing the active version over itself is idempotent (rm_rf + rename).
    install_version(&modroot, &downloads, "1.2.0", b"v1.2.0-exe-rebuilt", 3);
    assert_eq!(
        std::fs::read(modroot.join("1.2.0/bin/KooPreprocessor.exe")).unwrap(),
        b"v1.2.0-exe-rebuilt"
    );
}

#[test]
fn sha_mismatch_is_typed_before_extract() {
    let tmp = tempfile::tempdir().unwrap();
    let partial = tmp.path().join("pkg.zip.partial");
    make_zip(&partial, &[("a.txt", b"hello")]);
    // The agent verifies before extracting; a wrong digest yields the typed
    // error it maps to `sha256_verified=false` + audit kind=sha256_mismatch.
    let err = verify_file(&partial, &"0".repeat(64)).unwrap_err();
    assert!(matches!(err, CoreError::Sha256Mismatch { .. }));
}

#[test]
fn extract_into_staging_is_self_contained() {
    // A benign multi-dir archive extracts entirely under the staging root.
    let tmp = tempfile::tempdir().unwrap();
    let zip = tmp.path().join("p.zip");
    make_zip(
        &zip,
        &[
            ("bin/tool.exe", b"MZ"),
            ("bin/lib/dep.dll", b"dll"),
            ("readme.txt", b"hi"),
        ],
    );
    let dst = tmp.path().join("1.0.0.staging");
    extract_zip_safe(&zip, &dst).unwrap();
    assert!(dst.join("bin/tool.exe").exists());
    assert!(dst.join("bin/lib/dep.dll").exists());
    assert!(dst.join("readme.txt").exists());
    // Nothing escaped the staging directory.
    assert!(!tmp.path().join("tool.exe").exists());
}
