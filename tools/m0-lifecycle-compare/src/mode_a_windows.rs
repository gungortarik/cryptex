//! Windows Mode A compare path (test-only per-user logon task).

use std::process::Command;
use std::time::{Duration, Instant};

use cryptexd::compare::{EvidenceClass, SupervisorMode, mode_runtime_root, unix_now_ms};
use cryptexd::error::LifecycleError;
use cryptexd::instance::observe_instance;
use cryptexd::paths::ensure_runtime_root;
use cryptexd::platform::windows::{
    LogonTaskSpec, install, register, remove, request_clean_stop, run_now, status, unregister,
    wait_until_ready,
};

use crate::{RunOpts, seed_measurement, wait_until, write_measurement};

pub fn run_mode_a(opts: RunOpts) -> Result<(), LifecycleError> {
    let mode = SupervisorMode::NativeLaunchAgent;
    let mode_root = mode_runtime_root(&opts.base_runtime, mode);
    ensure_runtime_root(&mode_root)?;
    let mut measurement = seed_measurement(&opts, mode)?;
    let spec = LogonTaskSpec::for_runtime(&mode_root, &opts.cryptexd_bin)?;

    let result: Result<(), LifecycleError> = (|| {
        let startup = Instant::now();
        measurement.startup_unix_ms = Some(unix_now_ms());
        install(&spec)?;
        register(&spec)?;
        run_now(&spec)?;
        let pid = wait_until_ready(&spec, Duration::from_secs(15))?;
        measurement.daemon_pid = Some(pid);
        measurement.time_to_daemon_ready_ms = Some(startup.elapsed().as_millis());
        measurement.supervisor_pid = None;

        let st = status(&spec)?;
        if !st.registered || !st.daemon_lock_held {
            return Err(LifecycleError::Attention(format!(
                "logon task not healthy after register/run: registered={} lock_held={}",
                st.registered, st.daemon_lock_held
            )));
        }

        let dup = Command::new(&opts.cryptexd_bin)
            .args([
                "lifecycle",
                "probe",
                "--runtime-root",
                mode_root.to_str().unwrap(),
            ])
            .output()?;
        measurement.duplicate_launch_result = if dup.status.code() == Some(1) {
            format!(
                "{}: rejected exit=1 stderr={}",
                EvidenceClass::Automated.as_str(),
                String::from_utf8_lossy(&dup.stderr)
                    .trim()
                    .replace('\n', " ")
            )
        } else {
            format!("FAILED: expected exit 1, got {:?}", dup.status.code())
        };

        measurement.main_window_close_result = format!(
            "{}: Mode A has no Desktop window supervisor; N/A for window-close ownership",
            EvidenceClass::Automated.as_str()
        );
        measurement.daemon_alive_after_window_close = format!(
            "{}: N/A (native supervision; Desktop not required for daemon lifetime)",
            EvidenceClass::Automated.as_str()
        );

        let pre_crash = observe_instance(&mode_root)?;
        let crash_pid = pre_crash
            .holder_pid
            .ok_or_else(|| LifecycleError::Attention("no daemon pid before crash".into()))?;
        measurement.forced_crash_unix_ms = Some(unix_now_ms());
        let kill = Command::new("taskkill")
            .args(["/F", "/PID", &crash_pid.to_string()])
            .output()?;
        if !kill.status.success() {
            return Err(LifecycleError::Attention(format!(
                "taskkill /F failed for pid {crash_pid}: {}",
                String::from_utf8_lossy(&kill.stderr).trim()
            )));
        }

        let detected = wait_until(Duration::from_secs(5), || {
            observe_instance(&mode_root)
                .map(|o| !o.lock_held)
                .unwrap_or(false)
        });
        measurement.crash_detected = if detected {
            format!(
                "{}: lock released after taskkill /F",
                EvidenceClass::Automated.as_str()
            )
        } else {
            "FAILED: lock still held after taskkill /F".into()
        };

        measurement.restart_attempted = format!(
            "{}: Task Scheduler RestartOnFailure (PT1M)",
            EvidenceClass::Automated.as_str()
        );
        let restart_started = Instant::now();
        let restarted = wait_until(Duration::from_secs(90), || {
            observe_instance(&mode_root)
                .map(|o| o.lock_held && o.holder_pid.is_some_and(|p| p != crash_pid))
                .unwrap_or(false)
        });
        measurement.restart_success = if restarted {
            let obs = observe_instance(&mode_root)?;
            measurement.daemon_pid = obs.holder_pid;
            measurement.restart_latency_ms = Some(restart_started.elapsed().as_millis());
            format!(
                "{}: new pid {:?}",
                EvidenceClass::Automated.as_str(),
                obs.holder_pid
            )
        } else {
            "FAILED: RestartOnFailure did not restart daemon".into()
        };

        measurement.stale_lock_recovery = format!(
            "{}: after crash, logon task reacquired live lock (not stale)",
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
                "{}: status observe after restart succeeded (IPC Desktop reconnect still NOT_YET_VERIFIED)",
                EvidenceClass::Automated.as_str()
            );
        }

        request_clean_stop(&spec)?;
        let stopped = wait_until(Duration::from_secs(5), || {
            observe_instance(&mode_root)
                .map(|o| !o.lock_held)
                .unwrap_or(false)
        });
        unregister(&spec)?;
        remove(&spec)?;
        measurement.clean_shutdown_result = if stopped {
            format!(
                "{}: stop-file + unregister + remove succeeded",
                EvidenceClass::Automated.as_str()
            )
        } else {
            format!(
                "{}: unregister/remove forced after stop-file; lock_held may linger briefly",
                EvidenceClass::Automated.as_str()
            )
        };

        let leftover_xml = spec.xml_path.exists();
        let leftover_registered = status(&spec).map(|s| s.registered).unwrap_or(false);
        measurement.runtime_cleanup_result = if !leftover_xml && !leftover_registered {
            format!(
                "{}: task XML removed and schtasks entry deleted",
                EvidenceClass::Automated.as_str()
            )
        } else {
            format!("PARTIAL: xml_present={leftover_xml} registered={leftover_registered}")
        };

        measurement.evidence_class_notes = format!(
            "interactive login/reboot {}; full Desktop reconnect {}",
            EvidenceClass::NotYetVerified.as_str(),
            EvidenceClass::NotYetVerified.as_str()
        );
        Ok(())
    })();

    if let Err(error) = &result {
        measurement.errors = error.to_string();
        let _ = unregister(&spec);
        let _ = remove(&spec);
    }
    write_measurement(&opts, mode, &measurement)?;
    result
}
