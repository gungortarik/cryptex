//! Process-level update / rollback proof for M0 P0-2.
//!
//! Uses synthetic staged binaries and a marker fixture only. Does not select
//! ADR-0012 and does not touch P0-1 Syncthing runtime paths.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use cryptexd::instance::observe_instance;
use cryptexd::paths::assert_outside_repository;
use cryptexd::update::{UpdateLayout, marker_digest, run_update_rollback_proof};

fn unique_root(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!("cryptexd-update-{label}-{stamp}"));
    assert_outside_repository(&root).expect("outside repo");
    fs::create_dir_all(&root).expect("mkdir");
    root
}

fn cryptexd_bin() -> PathBuf {
    env!("CARGO_BIN_EXE_cryptexd").into()
}

#[test]
fn update_rollback_proof_preserves_marker_and_single_instance() {
    let root = unique_root("proof");
    let report =
        run_update_rollback_proof(&root, &cryptexd_bin(), "test-update-rollback").expect("proof");

    assert_eq!(report.initial_identity, "v1");
    assert_eq!(report.compatible_identity, "v2");
    assert!(report.failed_detected);
    assert_eq!(report.rollback_identity, "v2");
    assert_eq!(
        report.initial_marker_digest,
        report.compatible_marker_digest
    );
    assert_eq!(report.initial_marker_digest, report.failed_marker_digest);
    assert_eq!(report.initial_marker_digest, report.rollback_marker_digest);
    assert_ne!(report.initial_pid, 0);
    assert_ne!(report.compatible_pid, 0);
    assert_ne!(report.rollback_pid, 0);
    assert_eq!(report.final_instance_count, 0);
    assert!(!observe_instance(&root).expect("obs").lock_held);

    let layout = UpdateLayout::new(&root).expect("layout");
    assert_eq!(layout.active_identity().expect("id"), "v2");
    assert_eq!(
        marker_digest(&root).expect("digest"),
        report.initial_marker_digest
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn update_proof_cli_runs_outside_repository() {
    let root = unique_root("cli");
    let output = Command::new(cryptexd_bin())
        .args([
            "lifecycle",
            "update-proof",
            "--base-bin",
            cryptexd_bin().to_str().unwrap(),
            "--runtime-root",
            root.to_str().unwrap(),
            "--experiment-id",
            "cli-update-proof",
        ])
        .output()
        .expect("cli");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("compatible_identity=v2"));
    assert!(stdout.contains("failed_detected=true"));
    assert!(stdout.contains("rollback_identity=v2"));
    assert!(stdout.contains("compatible_marker_unchanged=true"));
    assert!(stdout.contains("failed_marker_unchanged=true"));
    assert!(stdout.contains("rollback_marker_unchanged=true"));
    let _ = fs::remove_dir_all(&root);
}
