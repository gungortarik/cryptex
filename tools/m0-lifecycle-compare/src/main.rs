//! M0 P0-2 supervision comparison CLI (Mode A vs Mode B).
//!
//! Does not select ADR-0012. Does not touch P0-1 Syncthing runtimes.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{self, Command};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use cryptexd::compare::{
    ComparisonMeasurement, EvidenceClass, SupervisorMode, measurement_path, mode_runtime_root,
    unix_now_ms,
};
use cryptexd::error::LifecycleError;
use cryptexd::instance::observe_instance;
use cryptexd::paths::{assert_outside_repository, default_p0_2_runtime_root, ensure_runtime_root};
use cryptexd::supervisor::DesktopSupervisor;
use cryptexd::update::run_update_rollback_proof;

#[cfg(target_os = "windows")]
mod mode_a_windows;

fn main() {
    match run(env::args().skip(1).collect()) {
        Ok(()) => process::exit(0),
        Err(error) => {
            eprintln!("m0-lifecycle-compare: {error}");
            process::exit(2);
        }
    }
}

fn run(args: Vec<String>) -> Result<(), LifecycleError> {
    let command = args.first().ok_or_else(|| LifecycleError::Usage(usage()))?;
    match command.as_str() {
        "help" | "--help" | "-h" => {
            println!("{}", usage());
            Ok(())
        }
        "run-mode-a" => {
            let opts = parse_run_opts(&args[1..])?;
            run_mode_a(opts)
        }
        "run-mode-b" => {
            let opts = parse_run_opts(&args[1..])?;
            run_mode_b(opts)
        }
        "status" => {
            let opts = parse_run_opts(&args[1..])?;
            print_status(&opts)
        }
        "cleanup" => {
            let opts = parse_run_opts(&args[1..])?;
            cleanup(&opts)
        }
        "run-update-rollback" => {
            let opts = parse_run_opts(&args[1..])?;
            run_update_rollback(opts)
        }
        other => Err(LifecycleError::Usage(format!(
            "unknown command '{other}'\n{}",
            usage()
        ))),
    }
}

#[derive(Debug)]
pub(crate) struct RunOpts {
    experiment_id: String,
    base_runtime: PathBuf,
    cryptexd_bin: PathBuf,
    branch: String,
    commit: String,
}

fn parse_run_opts(args: &[String]) -> Result<RunOpts, LifecycleError> {
    let mut experiment_id = None;
    let mut runtime_root = None;
    let mut cryptexd_bin = None;
    let mut branch = String::new();
    let mut commit = String::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--experiment-id" => {
                experiment_id = Some(require_value(args, index, "--experiment-id")?);
                index += 2;
            }
            "--runtime-root" => {
                runtime_root = Some(PathBuf::from(require_value(args, index, "--runtime-root")?));
                index += 2;
            }
            "--cryptexd" => {
                cryptexd_bin = Some(PathBuf::from(require_value(args, index, "--cryptexd")?));
                index += 2;
            }
            "--branch" => {
                branch = require_value(args, index, "--branch")?;
                index += 2;
            }
            "--commit" => {
                commit = require_value(args, index, "--commit")?;
                index += 2;
            }
            other => {
                return Err(LifecycleError::Usage(format!(
                    "unexpected argument '{other}'"
                )));
            }
        }
    }

    let experiment_id = experiment_id.unwrap_or_else(|| {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        format!("exp-{stamp}")
    });
    let base_runtime = match runtime_root {
        Some(path) => path,
        None => default_p0_2_runtime_root()?.join("compare"),
    };
    assert_outside_repository(&base_runtime)?;
    let cryptexd_bin = match cryptexd_bin {
        Some(path) => path,
        None => default_cryptexd_bin()?,
    };
    if !cryptexd_bin.exists() {
        return Err(LifecycleError::Io(format!(
            "cryptexd binary not found: {}",
            cryptexd_bin.display()
        )));
    }

    Ok(RunOpts {
        experiment_id,
        base_runtime,
        cryptexd_bin,
        branch,
        commit,
    })
}

fn require_value(args: &[String], index: usize, flag: &str) -> Result<String, LifecycleError> {
    args.get(index + 1)
        .cloned()
        .ok_or_else(|| LifecycleError::Usage(format!("{flag} requires a value")))
}

fn default_cryptexd_bin() -> Result<PathBuf, LifecycleError> {
    if let Ok(path) = env::var("CRYPTEXD_BIN") {
        return Ok(PathBuf::from(path));
    }
    // Prefer cargo-built debug binary adjacent to this tool when running under cargo test/run.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidates = [
        manifest.join("../../target/debug/cryptexd"),
        manifest.join("../../target/release/cryptexd"),
    ];
    for candidate in candidates {
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(LifecycleError::Io(
        "cryptexd binary not found; pass --cryptexd or set CRYPTEXD_BIN".into(),
    ))
}

pub(crate) fn seed_measurement(
    opts: &RunOpts,
    mode: SupervisorMode,
) -> Result<ComparisonMeasurement, LifecycleError> {
    let mode_root = mode_runtime_root(&opts.base_runtime, mode);
    ensure_runtime_root(&mode_root)?;
    let mut measurement = ComparisonMeasurement::blank(&opts.experiment_id, mode, &mode_root);
    measurement.branch = opts.branch.clone();
    measurement.commit = opts.commit.clone();
    measurement.macos_version = read_macos_version();
    measurement.architecture = env::consts::ARCH.into();
    measurement.login_startup_result = format!(
        "{}: interactive login/reboot not performed by this harness (register + on-demand run only)",
        EvidenceClass::NotYetVerified.as_str()
    );
    measurement.desktop_reconnect_result = format!(
        "{}: full Desktop IPC reconnect not exercised in this harness slice",
        EvidenceClass::NotYetVerified.as_str()
    );
    Ok(measurement)
}

pub(crate) fn write_measurement(
    opts: &RunOpts,
    mode: SupervisorMode,
    measurement: &ComparisonMeasurement,
) -> Result<PathBuf, LifecycleError> {
    let mode_root = mode_runtime_root(&opts.base_runtime, mode);
    let path = measurement_path(&mode_root, &opts.experiment_id);
    measurement.write(&path)?;
    println!("measurement_path={}", path.display());
    print!("{}", measurement.render_kv());
    Ok(path)
}

#[cfg(target_os = "macos")]
fn run_mode_a(opts: RunOpts) -> Result<(), LifecycleError> {
    use cryptexd::platform::macos::{
        LaunchAgentSpec, bootout, bootstrap, install, remove, request_clean_stop, status,
        wait_until_ready,
    };

    let mode = SupervisorMode::NativeLaunchAgent;
    let mode_root = mode_runtime_root(&opts.base_runtime, mode);
    ensure_runtime_root(&mode_root)?;
    let mut measurement = seed_measurement(&opts, mode)?;
    let spec = LaunchAgentSpec::for_runtime(&mode_root, &opts.cryptexd_bin)?;

    // Always attempt cleanup on failure paths.
    let result: Result<(), LifecycleError> = (|| {
        let startup = Instant::now();
        measurement.startup_unix_ms = Some(unix_now_ms());
        install(&spec)?;
        bootstrap(&spec)?;
        let pid = wait_until_ready(&spec, Duration::from_secs(10))?;
        measurement.daemon_pid = Some(pid);
        measurement.time_to_daemon_ready_ms = Some(startup.elapsed().as_millis());
        measurement.supervisor_pid = None; // launchd is OS supervisor; PID not captured here

        let st = status(&spec)?;
        if !st.loaded || !st.daemon_lock_held {
            return Err(LifecycleError::Attention(format!(
                "LaunchAgent not healthy after bootstrap: loaded={} lock_held={}",
                st.loaded, st.daemon_lock_held
            )));
        }

        // Duplicate launch via cryptexd probe.
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

        // Mode A is independent of Desktop window lifecycle.
        measurement.main_window_close_result = format!(
            "{}: Mode A has no Desktop window supervisor; N/A for window-close ownership",
            EvidenceClass::Automated.as_str()
        );
        measurement.daemon_alive_after_window_close = format!(
            "{}: N/A (native supervision; Desktop not required for daemon lifetime)",
            EvidenceClass::Automated.as_str()
        );

        // Forced crash + KeepAlive restart.
        let pre_crash = observe_instance(&mode_root)?;
        let crash_pid = pre_crash
            .holder_pid
            .ok_or_else(|| LifecycleError::Attention("no daemon pid before crash".into()))?;
        measurement.forced_crash_unix_ms = Some(unix_now_ms());
        let kill = Command::new("kill")
            .args(["-9", &crash_pid.to_string()])
            .output()?;
        if !kill.status.success() {
            return Err(LifecycleError::Attention(format!(
                "kill -9 failed for pid {crash_pid}"
            )));
        }

        // Detect lock release then KeepAlive restart.
        let detected = wait_until(Duration::from_secs(3), || {
            observe_instance(&mode_root)
                .map(|o| !o.lock_held)
                .unwrap_or(false)
        });
        measurement.crash_detected = if detected {
            format!(
                "{}: lock released after SIGKILL",
                EvidenceClass::Automated.as_str()
            )
        } else {
            "FAILED: lock still held after SIGKILL".into()
        };

        measurement.restart_attempted = format!(
            "{}: launchd KeepAlive.SuccessfulExit=false",
            EvidenceClass::Automated.as_str()
        );
        let restart_started = Instant::now();
        let restarted = wait_until(Duration::from_secs(10), || {
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
            "FAILED: KeepAlive did not restart daemon".into()
        };

        // Stale lock observation after clean stop path later; for now after crash recovery
        // a held lock is healthy, not stale.
        measurement.stale_lock_recovery = format!(
            "{}: after crash, KeepAlive reacquired live lock (not stale)",
            EvidenceClass::Automated.as_str()
        );

        // Client observe-after-restart feasibility (not full Desktop reconnect).
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

        // Clean shutdown: stop-file then bootout/remove.
        request_clean_stop(&spec)?;
        let stopped = wait_until(Duration::from_secs(5), || {
            observe_instance(&mode_root)
                .map(|o| !o.lock_held)
                .unwrap_or(false)
        });
        bootout(&spec)?;
        remove(&spec)?;
        measurement.clean_shutdown_result = if stopped {
            format!(
                "{}: stop-file + bootout + remove succeeded",
                EvidenceClass::Automated.as_str()
            )
        } else {
            format!(
                "{}: bootout/remove forced after stop-file; lock_held may linger briefly",
                EvidenceClass::Automated.as_str()
            )
        };

        let leftover_plist = spec.plist_path.exists();
        let leftover_loaded = status(&spec).map(|s| s.loaded).unwrap_or(false);
        measurement.runtime_cleanup_result = if !leftover_plist && !leftover_loaded {
            format!(
                "{}: plist removed and launchd label unloaded",
                EvidenceClass::Automated.as_str()
            )
        } else {
            format!("PARTIAL: plist_present={leftover_plist} loaded={leftover_loaded}")
        };

        measurement.evidence_class_notes = format!(
            "login/reboot {}; full Desktop reconnect {}",
            EvidenceClass::NotYetVerified.as_str(),
            EvidenceClass::NotYetVerified.as_str()
        );
        Ok(())
    })();

    if let Err(error) = &result {
        measurement.errors = error.to_string();
        let _ = bootout(&spec);
        let _ = remove(&spec);
    }
    write_measurement(&opts, mode, &measurement)?;
    result
}

#[cfg(target_os = "windows")]
fn run_mode_a(opts: RunOpts) -> Result<(), LifecycleError> {
    mode_a_windows::run_mode_a(opts)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn run_mode_a(_opts: RunOpts) -> Result<(), LifecycleError> {
    Err(LifecycleError::Attention(
        "Mode A native compare harness is supported on macOS and Windows only".into(),
    ))
}

fn run_mode_b(opts: RunOpts) -> Result<(), LifecycleError> {
    let mode = SupervisorMode::DesktopSupervisor;
    let mode_root = mode_runtime_root(&opts.base_runtime, mode);
    ensure_runtime_root(&mode_root)?;
    let mut measurement = seed_measurement(&opts, mode)?;
    let mut supervisor = DesktopSupervisor::new(&mode_root, &opts.cryptexd_bin)?;
    measurement.supervisor_pid = Some(process::id());

    let result: Result<(), LifecycleError> = (|| {
        let startup = Instant::now();
        measurement.startup_unix_ms = Some(unix_now_ms());
        let pid = supervisor.start_daemon()?;
        measurement.daemon_pid = Some(pid);
        measurement.time_to_daemon_ready_ms = Some(startup.elapsed().as_millis());

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

        // Process-level window-close simulation (supervisor stays alive).
        supervisor.simulate_window_close_keep_running()?;
        let alive = observe_instance(&mode_root)?.lock_held;
        measurement.main_window_close_result = format!(
            "{}: process-level window-close event recorded (Tauri GUI close is separate PHYSICAL_MANUAL)",
            EvidenceClass::Automated.as_str()
        );
        measurement.daemon_alive_after_window_close = if alive {
            format!("{}: true", EvidenceClass::Automated.as_str())
        } else {
            "FAILED: daemon not alive after simulated window close".into()
        };

        // Crash detection + supervised restart.
        let old_pid = measurement.daemon_pid;
        measurement.forced_crash_unix_ms = Some(unix_now_ms());
        supervisor.force_kill_daemon()?;
        let detected = wait_until(Duration::from_secs(2), || {
            observe_instance(&mode_root)
                .map(|o| !o.lock_held)
                .unwrap_or(false)
        });
        measurement.crash_detected = if detected {
            format!(
                "{}: supervisor observed child/lock release",
                EvidenceClass::Automated.as_str()
            )
        } else {
            "FAILED: crash not observable".into()
        };
        measurement.restart_attempted = format!(
            "{}: DesktopSupervisor::restart_daemon",
            EvidenceClass::Automated.as_str()
        );
        let (new_pid, latency) = supervisor.restart_daemon()?;
        measurement.restart_latency_ms = Some(latency);
        measurement.restart_success = if Some(new_pid) != old_pid {
            measurement.daemon_pid = Some(new_pid);
            format!(
                "{}: restarted pid={new_pid}",
                EvidenceClass::Automated.as_str()
            )
        } else {
            "FAILED: restart pid unchanged/unexpected".into()
        };
        measurement.stale_lock_recovery = format!(
            "{}: waited for lock release before respawn",
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
        if status_out.status.success() {
            measurement.desktop_reconnect_result = format!(
                "{}: status observe after supervised restart succeeded (full Desktop reconnect still NOT_YET_VERIFIED)",
                EvidenceClass::Automated.as_str()
            );
        }

        supervisor.clean_stop()?;
        let stopped = !observe_instance(&mode_root)?.lock_held;
        measurement.clean_shutdown_result = if stopped {
            format!(
                "{}: stop-file clean shutdown",
                EvidenceClass::Automated.as_str()
            )
        } else {
            "FAILED: lock still held after clean stop".into()
        };

        // Idempotent cleanup of mode-b control files (keep measurement).
        let _ = fs::remove_file(mode_root.join("ready"));
        let _ = fs::remove_file(mode_root.join("stop"));
        measurement.runtime_cleanup_result = format!(
            "{}: control files removed; measurement retained under measurements/",
            EvidenceClass::Automated.as_str()
        );
        measurement.evidence_class_notes = format!(
            "Tauri physical window-close {}; login/reboot {}; full Desktop reconnect {}",
            EvidenceClass::NotYetVerified.as_str(),
            EvidenceClass::NotYetVerified.as_str(),
            EvidenceClass::NotYetVerified.as_str()
        );
        Ok(())
    })();

    if let Err(error) = &result {
        measurement.errors = error.to_string();
        let _ = supervisor.force_kill_daemon();
    }
    write_measurement(&opts, mode, &measurement)?;
    result
}

fn run_update_rollback(opts: RunOpts) -> Result<(), LifecycleError> {
    let mode_root = opts.base_runtime.join("update-rollback");
    ensure_runtime_root(&mode_root)?;
    assert_outside_repository(&mode_root)?;
    let report = run_update_rollback_proof(&mode_root, &opts.cryptexd_bin, &opts.experiment_id)?;
    let out = mode_root.join("measurements");
    fs::create_dir_all(&out)?;
    let path = out.join(format!("{}.kv", opts.experiment_id));
    fs::write(&path, report.render_kv())?;
    print!("{}", report.render_kv());
    println!("measurement_path={}", path.display());
    Ok(())
}

fn print_status(opts: &RunOpts) -> Result<(), LifecycleError> {
    for mode in [
        SupervisorMode::NativeLaunchAgent,
        SupervisorMode::DesktopSupervisor,
    ] {
        let mode_root = mode_runtime_root(&opts.base_runtime, mode);
        let obs = if mode_root.exists() {
            observe_instance(&mode_root)?
        } else {
            cryptexd::instance::InstanceObservation {
                lock_present: false,
                lock_held: false,
                holder_pid: None,
            }
        };
        println!(
            "mode={} runtime_root={} lock_held={} holder_pid={:?}",
            mode.as_str(),
            mode_root.display(),
            obs.lock_held,
            obs.holder_pid
        );
    }
    Ok(())
}

fn cleanup(opts: &RunOpts) -> Result<(), LifecycleError> {
    #[cfg(target_os = "macos")]
    {
        use cryptexd::platform::macos::{LaunchAgentSpec, remove};
        let mode_root = mode_runtime_root(&opts.base_runtime, SupervisorMode::NativeLaunchAgent);
        if mode_root.exists() {
            let spec = LaunchAgentSpec::for_runtime(&mode_root, &opts.cryptexd_bin)?;
            remove(&spec)?;
        }
    }
    #[cfg(target_os = "windows")]
    {
        use cryptexd::platform::windows::{LogonTaskSpec, remove};
        let mode_root = mode_runtime_root(&opts.base_runtime, SupervisorMode::NativeLaunchAgent);
        if mode_root.exists() {
            let spec = LogonTaskSpec::for_runtime(&mode_root, &opts.cryptexd_bin)?;
            remove(&spec)?;
        }
    }
    for mode in [
        SupervisorMode::NativeLaunchAgent,
        SupervisorMode::DesktopSupervisor,
    ] {
        let mode_root = mode_runtime_root(&opts.base_runtime, mode);
        if mode_root.exists() {
            // Keep measurements/; remove live control files only.
            for name in [
                "ready",
                "stop",
                "daemon.lock",
                "lifecycle.status",
                "supervisor.state",
                "window-close.event",
                "LaunchAgent.plist",
                "LogonTask.xml",
                "launchagent.stdout.log",
                "launchagent.stderr.log",
            ] {
                let _ = fs::remove_file(mode_root.join(name));
            }
        }
    }
    println!("cleanup=ok runtime_root={}", opts.base_runtime.display());
    Ok(())
}

pub(crate) fn wait_until(timeout: Duration, mut predicate: impl FnMut() -> bool) -> bool {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if predicate() {
            return true;
        }
        thread::sleep(Duration::from_millis(40));
    }
    false
}

fn read_macos_version() -> String {
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("sw_vers").arg("-productVersion").output();
        if let Ok(output) = output
            && output.status.success()
        {
            return String::from_utf8_lossy(&output.stdout).trim().to_string();
        }
        "unknown".into()
    }
    #[cfg(not(target_os = "macos"))]
    {
        "n/a".into()
    }
}

fn usage() -> String {
    [
        "usage:",
        "  m0-lifecycle-compare run-mode-a [--experiment-id ID] [--runtime-root PATH] [--cryptexd PATH] [--branch B] [--commit C]",
        "  m0-lifecycle-compare run-mode-b [--experiment-id ID] [--runtime-root PATH] [--cryptexd PATH] [--branch B] [--commit C]",
        "  m0-lifecycle-compare run-update-rollback [--experiment-id ID] [--runtime-root PATH] [--cryptexd PATH] [--branch B] [--commit C]",
        "  m0-lifecycle-compare status [--runtime-root PATH]",
        "  m0-lifecycle-compare cleanup [--runtime-root PATH] [--cryptexd PATH]",
        "",
        "Notes:",
        "  - ADR-0012 remains unresolved; this tool compares candidates only",
        "  - default runtime: Cryptex-Dev/p0-2/compare",
        "  - update-rollback uses synthetic staged binaries only (not a production updater)",
        "  - does not reboot/logout and must not touch syncthing-p0-1*",
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_mentions_both_modes() {
        let text = usage();
        assert!(text.contains("run-mode-a"));
        assert!(text.contains("run-mode-b"));
        assert!(text.contains("run-update-rollback"));
        assert!(text.contains("ADR-0012"));
    }
}
