//! Process-level lifecycle harness for M0 P0-2.
//!
//! Uses temporary directories outside the repository. Does not touch P0-1
//! Syncthing runtime paths and does not select ADR-0012 supervision.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use cryptexd::instance::observe_instance;
use cryptexd::paths::assert_outside_repository;
use cryptexd::status::{HealthState, InstanceState, LifecycleReport};

fn unique_runtime(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!("cryptexd-p0-2-{label}-{stamp}"));
    assert_outside_repository(&root).expect("temp runtime must be outside repository");
    fs::create_dir_all(&root).expect("create runtime");
    root
}

fn cryptexd_bin() -> PathBuf {
    env!("CARGO_BIN_EXE_cryptexd").into()
}

fn wait_until(timeout: Duration, mut predicate: impl FnMut() -> bool) -> bool {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if predicate() {
            return true;
        }
        thread::sleep(Duration::from_millis(20));
    }
    false
}

fn parse_kv(output: &str, key: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let (k, v) = line.split_once('=')?;
        (k == key).then(|| v.to_string())
    })
}

#[test]
fn foundation_entrypoint_still_prints_banner() {
    let output = Command::new(cryptexd_bin()).output().expect("run cryptexd");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cryptexd M0 foundation"));
}

#[test]
fn second_daemon_instance_is_rejected_while_first_remains_healthy() {
    let runtime = unique_runtime("single-instance");
    let ready = runtime.join("ready");
    let stop = runtime.join("stop");

    let mut holder = Command::new(cryptexd_bin())
        .args([
            "lifecycle",
            "hold",
            "--runtime-root",
            runtime.to_str().unwrap(),
            "--ready-file",
            ready.to_str().unwrap(),
            "--stop-file",
            stop.to_str().unwrap(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn holder");

    assert!(
        wait_until(Duration::from_secs(5), || ready.exists()),
        "holder did not become ready"
    );

    let status = Command::new(cryptexd_bin())
        .args([
            "lifecycle",
            "status",
            "--runtime-root",
            runtime.to_str().unwrap(),
        ])
        .output()
        .expect("status");
    assert!(status.status.success());
    let status_out = String::from_utf8_lossy(&status.stdout);
    assert_eq!(parse_kv(&status_out, "instance").as_deref(), Some("held"));
    assert_eq!(parse_kv(&status_out, "health").as_deref(), Some("active"));
    assert_eq!(
        parse_kv(&status_out, "supervision").as_deref(),
        Some("unresolved")
    );

    let second = Command::new(cryptexd_bin())
        .args([
            "lifecycle",
            "probe",
            "--runtime-root",
            runtime.to_str().unwrap(),
        ])
        .output()
        .expect("second probe");
    assert_eq!(second.status.code(), Some(1), "second instance must exit 1");
    let stderr = String::from_utf8_lossy(&second.stderr);
    assert!(
        stderr.contains("already running"),
        "stderr should explain duplicate instance: {stderr}"
    );

    // First instance still healthy.
    let status_again = Command::new(cryptexd_bin())
        .args([
            "lifecycle",
            "status",
            "--runtime-root",
            runtime.to_str().unwrap(),
        ])
        .output()
        .expect("status again");
    let again = String::from_utf8_lossy(&status_again.stdout);
    assert_eq!(parse_kv(&again, "health").as_deref(), Some("active"));

    fs::write(&stop, "stop\n").expect("stop");
    let exit = holder.wait().expect("holder exit");
    assert!(exit.success(), "clean shutdown should succeed");

    let observation = observe_instance(&runtime).expect("observe after shutdown");
    assert!(!observation.lock_held);

    let _ = fs::remove_dir_all(&runtime);
}

#[test]
fn clean_shutdown_releases_instance_lock() {
    let runtime = unique_runtime("clean-shutdown");
    let output = Command::new(cryptexd_bin())
        .args([
            "lifecycle",
            "hold",
            "--runtime-root",
            runtime.to_str().unwrap(),
            "--hold-ms",
            "50",
        ])
        .output()
        .expect("hold");
    assert!(output.status.success());
    let observation = observe_instance(&runtime).expect("observe");
    assert!(!observation.lock_held);
    let report = LifecycleReport::from_observation(&runtime, &observation, None, "post-shutdown");
    assert_eq!(report.instance, InstanceState::Stale);
    assert_eq!(report.health, HealthState::Attention);
    let _ = fs::remove_dir_all(&runtime);
}

#[test]
fn compact_login_hold_publishes_ready_and_clean_stops_under_runtime_root() {
    let runtime = unique_runtime("login-hold");
    let mut holder = Command::new(cryptexd_bin())
        .args([
            "lifecycle",
            "login-hold",
            "--runtime-root",
            runtime.to_str().unwrap(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn login hold");
    let became_ready = wait_until(Duration::from_secs(5), || runtime.join("ready").exists());
    if !became_ready {
        let _ = holder.kill();
        let _ = holder.wait();
        panic!("login hold did not become ready");
    }
    let ready_pid = fs::read_to_string(runtime.join("ready")).expect("read ready");
    let observation = observe_instance(&runtime).expect("observe running login hold");
    fs::write(runtime.join("stop"), "stop\n").expect("clean stop");
    if !wait_until(Duration::from_secs(5), || {
        holder.try_wait().unwrap().is_some()
    }) {
        let _ = holder.kill();
        let _ = holder.wait();
        panic!("login hold did not react to runtime stop file");
    }
    let exit = holder.wait().expect("reap login hold");
    assert!(exit.success());
    assert_eq!(ready_pid.trim().parse::<u32>().unwrap(), holder.id());
    assert_eq!(observation.holder_pid, Some(holder.id()));
    assert!(!observe_instance(&runtime).unwrap().lock_held);
    assert!(
        fs::read_to_string(runtime.join("lifecycle.status"))
            .unwrap()
            .contains("clean shutdown released instance lock")
    );
    let _ = fs::remove_dir_all(&runtime);
}

#[test]
fn forced_crash_is_detectable_and_restart_path_works() {
    let runtime = unique_runtime("crash-restart");
    let ready = runtime.join("ready");

    let mut crash = Command::new(cryptexd_bin())
        .args([
            "lifecycle",
            "hold",
            "--runtime-root",
            runtime.to_str().unwrap(),
            "--ready-file",
            ready.to_str().unwrap(),
            "--crash-after-ready",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn crash fixture");

    assert!(
        wait_until(Duration::from_secs(5), || ready.exists()),
        "crash fixture never became ready"
    );

    let status = crash.wait().expect("wait crash");
    assert!(!status.success(), "abort must be non-success");

    assert!(
        wait_until(Duration::from_secs(2), || {
            observe_instance(&runtime)
                .map(|obs| !obs.lock_held)
                .unwrap_or(false)
        }),
        "lock should release after crash"
    );

    let observed = Command::new(cryptexd_bin())
        .args([
            "lifecycle",
            "status",
            "--runtime-root",
            runtime.to_str().unwrap(),
        ])
        .output()
        .expect("refresh status after crash");
    assert!(observed.status.success());
    let observed_out = String::from_utf8_lossy(&observed.stdout);
    assert_eq!(
        parse_kv(&observed_out, "instance").as_deref(),
        Some("stale")
    );
    assert_eq!(
        parse_kv(&observed_out, "health").as_deref(),
        Some("attention")
    );
    assert_eq!(parse_kv(&observed_out, "pid").as_deref(), Some("none"));
    let crashed_pid = parse_kv(&observed_out, "recorded_pid").expect("recorded PID");
    let persisted = fs::read_to_string(runtime.join("lifecycle.status")).expect("status file");
    assert_eq!(parse_kv(&persisted, "health").as_deref(), Some("attention"));
    assert_eq!(parse_kv(&persisted, "pid").as_deref(), Some("none"));
    assert_eq!(parse_kv(&persisted, "recorded_pid"), Some(crashed_pid));

    let restart = Command::new(cryptexd_bin())
        .args([
            "lifecycle",
            "probe",
            "--runtime-root",
            runtime.to_str().unwrap(),
        ])
        .output()
        .expect("restart probe");
    assert!(
        restart.status.success(),
        "restart after crash must acquire lock: {}",
        String::from_utf8_lossy(&restart.stderr)
    );
    let out = String::from_utf8_lossy(&restart.stdout);
    assert_eq!(parse_kv(&out, "instance").as_deref(), Some("held"));

    let _ = fs::remove_dir_all(&runtime);
}

#[test]
fn lifecycle_state_reflects_runtime_and_avoids_repo_files() {
    let runtime = unique_runtime("state-reflect");
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let repo_root = fs::canonicalize(&repo_root).expect("repo root");

    let probe = Command::new(cryptexd_bin())
        .args([
            "lifecycle",
            "probe",
            "--runtime-root",
            runtime.to_str().unwrap(),
        ])
        .output()
        .expect("probe");
    assert!(probe.status.success());

    assert!(runtime.join("daemon.lock").exists());
    assert!(runtime.join("lifecycle.status").exists());
    assert!(
        !path_has_runtime_residue(&repo_root),
        "no lifecycle runtime residue may appear inside the repository"
    );

    let persisted = fs::read_to_string(runtime.join("lifecycle.status")).expect("status file");
    assert_eq!(parse_kv(&persisted, "instance").as_deref(), Some("stale"));
    assert_eq!(parse_kv(&persisted, "health").as_deref(), Some("attention"));

    let _ = fs::remove_dir_all(&runtime);
}

#[test]
fn compat_fixture_rejects_mismatch_and_keeps_marker_safe() {
    let runtime = unique_runtime("compat");

    let bad = Command::new(cryptexd_bin())
        .args([
            "lifecycle",
            "compat",
            "--desktop-protocol",
            "2",
            "--daemon-protocol",
            "1",
        ])
        .output()
        .expect("incompatible");
    assert_eq!(bad.status.code(), Some(3));

    let good = Command::new(cryptexd_bin())
        .args([
            "lifecycle",
            "compat",
            "--desktop-protocol",
            "1",
            "--daemon-protocol",
            "1",
            "--runtime-root",
            runtime.to_str().unwrap(),
        ])
        .output()
        .expect("compatible");
    assert!(good.status.success());
    let out = String::from_utf8_lossy(&good.stdout);
    assert!(out.contains("canonical_marker_safe=true"));
    assert_eq!(
        fs::read_to_string(runtime.join("synthetic-canonical.marker"))
            .expect("marker")
            .trim(),
        "canonical-v1"
    );

    let _ = fs::remove_dir_all(&runtime);
}

fn path_has_runtime_residue(repo_root: &Path) -> bool {
    let candidates = [
        repo_root.join("Cryptex-Dev"),
        repo_root.join("p0-2"),
        repo_root.join("daemon.lock"),
        repo_root.join("lifecycle.status"),
    ];
    candidates.iter().any(|path| path.exists())
}
