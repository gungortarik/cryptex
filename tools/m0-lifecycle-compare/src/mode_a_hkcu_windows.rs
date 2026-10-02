//! Windows Mode A HKCU Run login-start compare path (test-only).
//!
//! Does **not** claim OS-level crash restart. Does not select ADR-0012.

use std::fs;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use cryptexd::compare::{EvidenceClass, SupervisorMode, mode_runtime_root, unix_now_ms};
use cryptexd::error::LifecycleError;
use cryptexd::instance::observe_instance;
use cryptexd::paths::ensure_runtime_root;
use cryptexd::platform::windows::{
    HkcuRunSpec, register_hkcu_run, status_hkcu_run, unregister_hkcu_run,
};

use crate::{RunOpts, seed_measurement, wait_until, write_measurement};

pub fn run_mode_a_hkcu(opts: RunOpts, leave_registered: bool) -> Result<(), LifecycleError> {
    let mode = SupervisorMode::NativeHkcuRun;
    let mode_root = mode_runtime_root(&opts.base_runtime, mode);
    ensure_runtime_root(&mode_root)?;
    let mut measurement = seed_measurement(&opts, mode)?;
    let spec = HkcuRunSpec::for_runtime(&mode_root, &opts.cryptexd_bin)?;

    let result: Result<(), LifecycleError> = (|| {
        let startup = Instant::now();
        measurement.startup_unix_ms = Some(unix_now_ms());
        register_hkcu_run(&spec)?;

        let st = status_hkcu_run(&spec)?;
        if !st.registered {
            return Err(LifecycleError::Attention(
                "HKCU Run value not present after register".into(),
            ));
        }

        // On-demand launch proves the registered command line shape; it is NOT a login proof.
        let _ = fs::remove_file(&spec.ready_file);
        let _ = fs::remove_file(&spec.stop_file);
        let mut child = Command::new(&opts.cryptexd_bin)
            .args([
                "lifecycle",
                "hold",
                "--runtime-root",
                mode_root
                    .to_str()
                    .ok_or_else(|| LifecycleError::Io("runtime root is not valid UTF-8".into()))?,
                "--ready-file",
                spec.ready_file
                    .to_str()
                    .ok_or_else(|| LifecycleError::Io("ready path is not valid UTF-8".into()))?,
                "--stop-file",
                spec.stop_file
                    .to_str()
                    .ok_or_else(|| LifecycleError::Io("stop path is not valid UTF-8".into()))?,
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        let pid = wait_until_ready_file(&spec.ready_file, Duration::from_secs(10))?;
        measurement.daemon_pid = Some(pid);
        measurement.time_to_daemon_ready_ms = Some(startup.elapsed().as_millis());
        measurement.supervisor_pid = None;

        let dup = Command::new(&opts.cryptexd_bin)
            .args([
                "lifecycle",
                "probe",
                "--runtime-root",
                mode_root.to_str().unwrap(),
            ])
            .output()?;
        measurement.duplicate_launch_result = if dup.status.code() == Some(1) {
            format!("{}: rejected exit=1", EvidenceClass::Automated.as_str())
        } else {
            format!("FAILED: expected exit 1, got {:?}", dup.status.code())
        };

        measurement.main_window_close_result = format!(
            "{}: Mode A HKCU Run has no Desktop window supervisor; N/A",
            EvidenceClass::Automated.as_str()
        );
        measurement.daemon_alive_after_window_close = format!(
            "{}: N/A (HKCU Run login registration; no window supervisor)",
            EvidenceClass::Automated.as_str()
        );

        // Explicit: OS-level crash restart is unsupported for HKCU Run.
        measurement.crash_detected = format!(
            "{}: unsupported for HKCU Run candidate (no OS KeepAlive/RestartOnFailure)",
            EvidenceClass::NotYetVerified.as_str()
        );
        measurement.restart_attempted = format!(
            "{}: not attempted — HKCU Run does not supervise crash restart",
            EvidenceClass::NotYetVerified.as_str()
        );
        measurement.restart_success = format!(
            "{}: unsupported for this candidate",
            EvidenceClass::NotYetVerified.as_str()
        );
        measurement.stale_lock_recovery = format!(
            "{}: N/A for HKCU Run OS supervision (exclusive lock still applies on next start)",
            EvidenceClass::Automated.as_str()
        );

        let status_out = Command::new(&opts.cryptexd_bin)
            .args([
                "lifecycle",
                "status",
                "--runtime-root",
                mode_root.to_str().unwrap(),
            ])
            .output()?;
        if status_out.status.success()
            && String::from_utf8_lossy(&status_out.stdout).contains("health=active")
        {
            measurement.desktop_reconnect_result = format!(
                "{}: status observe while held succeeded (full Desktop IPC reconnect still NOT_YET_VERIFIED; login start still PHYSICAL_MANUAL)",
                EvidenceClass::Automated.as_str()
            );
        }

        fs::write(&spec.stop_file, "stop\n")?;
        let stopped = wait_until(Duration::from_secs(5), || {
            observe_instance(&mode_root)
                .map(|o| !o.lock_held)
                .unwrap_or(false)
        });
        let _ = child.wait();
        measurement.clean_shutdown_result = if stopped {
            format!(
                "{}: stop-file clean shutdown after on-demand hold",
                EvidenceClass::Automated.as_str()
            )
        } else {
            "FAILED: lock still held after stop-file".into()
        };

        if leave_registered {
            measurement.runtime_cleanup_result = format!(
                "{}: HKCU value left registered for PHYSICAL_MANUAL login proof (value={})",
                EvidenceClass::Automated.as_str(),
                spec.value_name
            );
            measurement.login_startup_result = format!(
                "{}: registry registration present; interactive sign-out/sign-in not performed by harness",
                EvidenceClass::NotYetVerified.as_str()
            );
        } else {
            unregister_hkcu_run(&spec)?;
            let after = status_hkcu_run(&spec)?;
            measurement.runtime_cleanup_result = if !after.registered {
                format!(
                    "{}: test-only HKCU Run value removed",
                    EvidenceClass::Automated.as_str()
                )
            } else {
                "FAILED: HKCU Run value still present after unregister".into()
            };
            measurement.login_startup_result = format!(
                "{}: interactive login/reboot not performed (register+on-demand only)",
                EvidenceClass::NotYetVerified.as_str()
            );
        }

        measurement.evidence_class_notes = format!(
            "HKCU Run candidate: OS crash restart unsupported; interactive login {}; full Desktop reconnect {}; ADR-0012 unresolved",
            EvidenceClass::NotYetVerified.as_str(),
            EvidenceClass::NotYetVerified.as_str()
        );
        Ok(())
    })();

    if let Err(error) = &result {
        measurement.errors = error.to_string();
        if !leave_registered {
            let _ = unregister_hkcu_run(&spec);
        }
        let _ = fs::write(&spec.stop_file, "stop\n");
    }
    write_measurement(&opts, mode, &measurement)?;
    result
}

fn wait_until_ready_file(
    ready: &std::path::Path,
    timeout: Duration,
) -> Result<u32, LifecycleError> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if ready.exists() {
            let contents = fs::read_to_string(ready)?;
            if let Ok(pid) = contents.trim().parse::<u32>() {
                return Ok(pid);
            }
        }
        thread::sleep(Duration::from_millis(40));
    }
    Err(LifecycleError::Attention(
        "HKCU probe daemon did not become ready before timeout".into(),
    ))
}

pub fn register_only(opts: &RunOpts) -> Result<(), LifecycleError> {
    let mode_root = mode_runtime_root(&opts.base_runtime, SupervisorMode::NativeHkcuRun);
    ensure_runtime_root(&mode_root)?;
    let spec = HkcuRunSpec::for_runtime(&mode_root, &opts.cryptexd_bin)?;
    register_hkcu_run(&spec)?;
    let st = status_hkcu_run(&spec)?;
    println!("hkcu_run_value={}", st.value_name);
    println!("registered={}", st.registered);
    println!(
        "command_line={}",
        st.command_line.unwrap_or_else(|| "none".into())
    );
    println!("runtime_root={}", mode_root.display());
    println!("note=interactive login/reboot not performed; OS crash restart unsupported");
    Ok(())
}

pub fn status_only(opts: &RunOpts) -> Result<(), LifecycleError> {
    let mode_root = mode_runtime_root(&opts.base_runtime, SupervisorMode::NativeHkcuRun);
    let spec = HkcuRunSpec::for_runtime(&mode_root, &opts.cryptexd_bin)?;
    let st = status_hkcu_run(&spec)?;
    println!("hkcu_run_value={}", st.value_name);
    println!("registered={}", st.registered);
    println!(
        "command_line={}",
        st.command_line.unwrap_or_else(|| "none".into())
    );
    println!("daemon_lock_held={}", st.daemon_lock_held);
    println!(
        "daemon_pid={}",
        st.daemon_pid
            .map(|p| p.to_string())
            .unwrap_or_else(|| "none".into())
    );
    println!("ready_present={}", st.ready_present);
    println!("runtime_root={}", mode_root.display());
    Ok(())
}

pub fn remove_only(opts: &RunOpts) -> Result<(), LifecycleError> {
    let mode_root = mode_runtime_root(&opts.base_runtime, SupervisorMode::NativeHkcuRun);
    let spec = HkcuRunSpec::for_runtime(&mode_root, &opts.cryptexd_bin)?;
    unregister_hkcu_run(&spec)?;
    let st = status_hkcu_run(&spec)?;
    println!("hkcu_run_value={}", st.value_name);
    println!("registered={}", st.registered);
    println!("removed={}", !st.registered);
    Ok(())
}
