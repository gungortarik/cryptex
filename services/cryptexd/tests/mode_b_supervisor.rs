//! Integration coverage for Mode B desktop-supervisor harness.
//! Mode A LaunchAgent physical/OS proofs run via m0-lifecycle-compare on macOS.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use cryptexd::instance::observe_instance;
use cryptexd::paths::assert_outside_repository;
use cryptexd::supervisor::DesktopSupervisor;

// Both real daemon fixtures bind the same per-user IPC endpoint. Keep their
// lifetimes separate while preserving each duplicate/restart assertion.
static PROCESS_HARNESS_LOCK: Mutex<()> = Mutex::new(());

fn unique_root(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!("cryptexd-mode-b-{label}-{stamp}"));
    assert_outside_repository(&root).expect("outside repo");
    fs::create_dir_all(&root).expect("mkdir");
    root
}

fn cryptexd_bin() -> PathBuf {
    env!("CARGO_BIN_EXE_cryptexd").into()
}

#[test]
fn mode_b_rejects_duplicate_and_restarts_after_crash() {
    let _guard = PROCESS_HARNESS_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let root = unique_root("supervise");
    let mut supervisor = DesktopSupervisor::new(&root, &cryptexd_bin()).expect("supervisor");
    let pid = supervisor.start_daemon().expect("start");
    assert!(pid > 0);

    let dup = Command::new(cryptexd_bin())
        .args([
            "lifecycle",
            "probe",
            "--runtime-root",
            root.to_str().unwrap(),
        ])
        .output()
        .expect("dup");
    assert_eq!(dup.status.code(), Some(1));

    supervisor
        .simulate_window_close_keep_running()
        .expect("window close");
    assert!(observe_instance(&root).expect("obs").lock_held);

    supervisor.force_kill_daemon().expect("kill");
    let (new_pid, latency) = supervisor.restart_daemon().expect("restart");
    assert_ne!(new_pid, pid);
    assert!(latency < 10_000);

    supervisor.clean_stop().expect("stop");
    assert!(!observe_instance(&root).expect("obs2").lock_held);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn supervisor_detects_unexpected_exit_and_restarts_but_respects_clean_stop() {
    let _guard = PROCESS_HARNESS_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let root = unique_root("automatic-restart");
    let mut supervisor = DesktopSupervisor::new(&root, &cryptexd_bin()).expect("supervisor");
    let first_pid = supervisor.start_daemon().expect("start");

    terminate_process(first_pid);
    let mut restarted = None;
    for _ in 0..80 {
        if let Some(result) = supervisor
            .restart_if_child_exited()
            .expect("restart after child exit")
        {
            restarted = Some(result);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    let (second_pid, _) = restarted.expect("replacement daemon");
    assert_ne!(first_pid, second_pid);
    assert!(observe_instance(&root).expect("live replacement").lock_held);

    supervisor.clean_stop().expect("clean stop");
    assert!(
        supervisor
            .restart_if_child_exited()
            .expect("clean exit must not restart")
            .is_none()
    );
    assert!(!observe_instance(&root).expect("stopped instance").lock_held);
    let _ = fs::remove_dir_all(&root);
}

#[cfg(windows)]
fn terminate_process(pid: u32) {
    let status = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .status()
        .expect("taskkill");
    assert!(status.success(), "taskkill failed for daemon PID {pid}");
}

#[cfg(unix)]
fn terminate_process(pid: u32) {
    let status = Command::new("kill")
        .args(["-KILL", &pid.to_string()])
        .status()
        .expect("kill");
    assert!(status.success(), "kill failed for daemon PID {pid}");
}

#[test]
fn cleanup_control_files_are_idempotent() {
    let root = unique_root("cleanup");
    fs::write(root.join("ready"), "1\n").unwrap();
    fs::write(root.join("stop"), "stop\n").unwrap();
    for name in ["ready", "stop", "missing-file"] {
        let _ = fs::remove_file(root.join(name));
        let _ = fs::remove_file(root.join(name));
    }
    assert!(!root.join("ready").exists());
    let _ = fs::remove_dir_all(&root);
}
