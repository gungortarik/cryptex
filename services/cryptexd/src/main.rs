use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;
use std::thread;
use std::time::Duration;

use cryptex_core::BrainIdentity;
use cryptex_events::envelope::Classification;
use cryptexd::canonical_writer::CanonicalWriterConfig;
use cryptexd::capture::{CaptureOutcome, CaptureRequest, capture_memory_note};

use cryptexd::compat::{
    CompatDecision, ComponentVersions, DAEMON_PROTOCOL_VERSION, evaluate_compat,
    read_synthetic_marker, simulate_failed_update, simulate_rollback, write_synthetic_marker,
};
use cryptexd::error::LifecycleError;
use cryptexd::identity_runtime::write_brain_identity;
use cryptexd::install_runtime::prepare_m1_runtime_foundation;
use cryptexd::instance::{observe_instance, try_acquire_instance};
use cryptexd::local_device_identity::enroll_local_device;
use cryptexd::paths::{assert_outside_repository, default_p0_2_runtime_root, ensure_runtime_root};
use cryptexd::secure_store::{SecureStore, SecureStoreEnvironment};
use cryptexd::status::{LifecycleReport, write_status_file};
use cryptexd::supervision::selected_supervision;
use cryptexd::update::run_update_rollback_proof;

fn main() {
    match run(env::args().skip(1).collect()) {
        Ok(()) => process::exit(0),
        Err(error) => {
            eprintln!("cryptexd: {error}");
            let code = match error {
                LifecycleError::AlreadyRunning { .. } => 1,
                LifecycleError::Incompatible(_) => 3,
                LifecycleError::UnsafeRuntimePath(_) => 4,
                LifecycleError::Usage(_) => 2,
                _ => 2,
            };
            process::exit(code);
        }
    }
}

fn run(args: Vec<String>) -> Result<(), LifecycleError> {
    if args.is_empty() {
        println!("cryptexd M0 foundation");
        return Ok(());
    }

    match args[0].as_str() {
        "lifecycle" => run_lifecycle(&args[1..]),
        "identity" => run_identity(&args[1..]),
        "pilot-fixture" => run_pilot_fixture(&args[1..]),
        "help" | "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        other => Err(LifecycleError::Usage(format!(
            "unknown command '{other}'\n{}",
            usage_text()
        ))),
    }
}

const PILOT_RUNTIME_MARKER: &str = "m0-native-desktop-runtime-v1\n";
const PILOT_FIXTURE_MARKER: &str = "m5-timeline-fixture-v1\n";
const PILOT_TIMELINE_NOTE: &str =
    "Synthetic M5 timeline fixture. Safe to discard with this isolated pilot runtime.";

fn run_pilot_fixture(args: &[String]) -> Result<(), LifecycleError> {
    if args.first().map(String::as_str) != Some("timeline-v1") {
        return Err(LifecycleError::Usage(format!(
            "pilot-fixture requires the explicit 'timeline-v1' fixture\n{}",
            usage_text()
        )));
    }
    let mut runtime_root = None;
    let mut confirmed = false;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--runtime-root" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    LifecycleError::Usage("--runtime-root requires a path".into())
                })?;
                runtime_root = Some(PathBuf::from(value));
                index += 2;
            }
            "--confirm-isolated-pilot" => {
                if confirmed {
                    return Err(LifecycleError::Usage(
                        "--confirm-isolated-pilot must appear once".into(),
                    ));
                }
                confirmed = true;
                index += 1;
            }
            other => {
                return Err(LifecycleError::Usage(format!(
                    "unexpected pilot-fixture argument '{other}'"
                )));
            }
        }
    }
    let runtime_root = runtime_root.ok_or_else(|| {
        LifecycleError::Usage(
            "pilot-fixture timeline-v1 requires an explicit --runtime-root".into(),
        )
    })?;
    if !confirmed {
        return Err(LifecycleError::Usage(
            "pilot-fixture requires --confirm-isolated-pilot".into(),
        ));
    }
    validate_pilot_fixture_root(&runtime_root)?;
    let _instance = try_acquire_instance(&runtime_root)?;
    prepare_m1_runtime_foundation(&runtime_root)
        .map_err(|error| LifecycleError::Attention(error.to_string()))?;
    let (brain_id, event_id) = seed_timeline_fixture_with_platform_store(&runtime_root)?;
    let mut marker = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(runtime_root.join("pilot-timeline-fixture.marker"))?;
    marker.write_all(PILOT_FIXTURE_MARKER.as_bytes())?;
    marker.sync_all()?;
    println!(
        "pilot_fixture=timeline-v1\nbrain_id={brain_id}\nevent_id={event_id}\nclassification=public"
    );
    Ok(())
}

fn validate_pilot_fixture_root(runtime_root: &Path) -> Result<(), LifecycleError> {
    if !runtime_root.is_absolute()
        || runtime_root
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err(LifecycleError::UnsafeRuntimePath(
            "pilot fixture requires an absolute runtime path without parent traversal".into(),
        ));
    }
    assert_outside_repository(runtime_root)?;
    let root_meta = fs::symlink_metadata(runtime_root)?;
    if root_meta.file_type().is_symlink() || !root_meta.is_dir() {
        return Err(LifecycleError::UnsafeRuntimePath(
            "pilot fixture runtime must be an ordinary directory".into(),
        ));
    }
    let marker = runtime_root.join("pilot-runtime.marker");
    let marker_meta = fs::symlink_metadata(&marker)?;
    if marker_meta.file_type().is_symlink() || !marker_meta.is_file() {
        return Err(LifecycleError::UnsafeRuntimePath(
            "pilot fixture requires an ordinary pilot-runtime.marker file".into(),
        ));
    }
    if fs::read(&marker)? != PILOT_RUNTIME_MARKER.as_bytes() {
        return Err(LifecycleError::UnsafeRuntimePath(
            "pilot fixture runtime marker does not match".into(),
        ));
    }
    let mut entries = fs::read_dir(runtime_root)?;
    let only_marker = entries
        .next()
        .transpose()?
        .is_some_and(|entry| entry.file_name() == "pilot-runtime.marker");
    if !only_marker || entries.next().transpose()?.is_some() {
        return Err(LifecycleError::UnsafeRuntimePath(
            "pilot fixture only accepts a fresh runtime containing its marker".into(),
        ));
    }
    Ok(())
}

fn seed_timeline_fixture_with_store<S: SecureStore>(
    runtime_root: &Path,
    store: &S,
) -> Result<(String, String), LifecycleError> {
    let brain_id = random_uuid_v4()?;
    let brain = BrainIdentity::new(brain_id.clone(), "2026-10-11T00:00:00Z")
        .map_err(|error| LifecycleError::Attention(error.to_string()))?;
    write_brain_identity(runtime_root, &brain)
        .map_err(|error| LifecycleError::Attention(error.to_string()))?;
    enroll_local_device(runtime_root, store, SecureStoreEnvironment::Dev)
        .map_err(|error| LifecycleError::Attention(error.to_string()))?;

    let capture_id = random_uuid_v4()?;
    let outcome = capture_memory_note(
        store,
        SecureStoreEnvironment::Dev,
        runtime_root,
        CaptureRequest {
            capture_id,
            text: PILOT_TIMELINE_NOTE.into(),
            classification: Classification::Public,
        },
        &CanonicalWriterConfig::default(),
    );
    match outcome {
        CaptureOutcome::Committed(commit) => Ok((commit.brain_id, commit.event_id)),
        CaptureOutcome::Rejected { code, detail } => Err(LifecycleError::Attention(format!(
            "pilot fixture capture rejected ({code:?}): {detail}"
        ))),
        CaptureOutcome::Uncertain { reason, detail } => Err(LifecycleError::Attention(format!(
            "pilot fixture capture uncertain ({reason:?}): {detail}"
        ))),
    }
}

fn random_uuid_v4() -> Result<String, LifecycleError> {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).map_err(|error| {
        LifecycleError::Attention(format!("OS randomness unavailable: {error}"))
    })?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Ok(format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    ))
}

#[cfg(windows)]
fn seed_timeline_fixture_with_platform_store(
    runtime_root: &Path,
) -> Result<(String, String), LifecycleError> {
    seed_timeline_fixture_with_store(runtime_root, &cryptexd::WindowsCredentialStore::new())
}

#[cfg(target_os = "macos")]
fn seed_timeline_fixture_with_platform_store(
    runtime_root: &Path,
) -> Result<(String, String), LifecycleError> {
    seed_timeline_fixture_with_store(runtime_root, &cryptexd::MacosKeychainStore::new())
}

#[cfg(not(any(windows, target_os = "macos")))]
fn seed_timeline_fixture_with_platform_store(
    _runtime_root: &Path,
) -> Result<(String, String), LifecycleError> {
    Err(LifecycleError::Usage(
        "pilot timeline fixtures are supported only on Windows and macOS".into(),
    ))
}

fn run_identity(args: &[String]) -> Result<(), LifecycleError> {
    if args.first().map(String::as_str) != Some("enroll-local") {
        return Err(LifecycleError::Usage(format!(
            "identity requires the explicit 'enroll-local' operation\n{}",
            usage_text()
        )));
    }
    let mut runtime_root = None;
    let mut environment = None;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--runtime-root" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    LifecycleError::Usage("--runtime-root requires a path".into())
                })?;
                runtime_root = Some(PathBuf::from(value));
                index += 2;
            }
            "--environment" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    LifecycleError::Usage("--environment requires dev or prod".into())
                })?;
                environment = Some(match value.as_str() {
                    "dev" => SecureStoreEnvironment::Dev,
                    "prod" => SecureStoreEnvironment::Prod,
                    _ => {
                        return Err(LifecycleError::Usage(
                            "--environment must be dev or prod".into(),
                        ));
                    }
                });
                index += 2;
            }
            other => {
                return Err(LifecycleError::Usage(format!(
                    "unexpected identity argument '{other}'"
                )));
            }
        }
    }
    let runtime_root = runtime_root.ok_or_else(|| {
        LifecycleError::Usage(
            "identity enroll-local requires --runtime-root; choose the app-data runtime explicitly"
                .into(),
        )
    })?;
    let environment = environment.ok_or_else(|| {
        LifecycleError::Usage(
            "identity enroll-local requires --environment dev|prod explicitly".into(),
        )
    })?;
    assert_outside_repository(&runtime_root)?;
    ensure_runtime_root(&runtime_root)?;
    enroll_local_with_platform_store(&runtime_root, environment)
}

fn enroll_local_with_store<S: SecureStore>(
    runtime_root: &Path,
    store: &S,
    environment: SecureStoreEnvironment,
) -> Result<(), LifecycleError> {
    let identity = enroll_local_device(runtime_root, store, environment)
        .map_err(|error| LifecycleError::Attention(error.to_string()))?;
    println!("brain_id={}", identity.brain_id);
    println!("device_id={}", identity.device_id);
    println!("enrollment=ready");
    Ok(())
}

#[cfg(windows)]
fn enroll_local_with_platform_store(
    runtime_root: &Path,
    environment: SecureStoreEnvironment,
) -> Result<(), LifecycleError> {
    enroll_local_with_store(
        runtime_root,
        &cryptexd::WindowsCredentialStore::new(),
        environment,
    )
}

#[cfg(target_os = "macos")]
fn enroll_local_with_platform_store(
    runtime_root: &Path,
    environment: SecureStoreEnvironment,
) -> Result<(), LifecycleError> {
    enroll_local_with_store(
        runtime_root,
        &cryptexd::MacosKeychainStore::new(),
        environment,
    )
}

#[cfg(not(any(windows, target_os = "macos")))]
fn enroll_local_with_platform_store(
    _runtime_root: &Path,
    _environment: SecureStoreEnvironment,
) -> Result<(), LifecycleError> {
    Err(LifecycleError::Usage(
        "local identity enrollment is supported only on Windows and macOS".into(),
    ))
}

fn run_lifecycle(args: &[String]) -> Result<(), LifecycleError> {
    let command = args
        .first()
        .ok_or_else(|| LifecycleError::Usage(usage_text()))?;

    match command.as_str() {
        "status" => {
            let runtime_root = parse_runtime_root(&args[1..])?;
            ensure_runtime_root(&runtime_root)?;
            let observation = observe_instance(&runtime_root)?;
            let report = LifecycleReport::from_observation(
                &runtime_root,
                &observation,
                None,
                "observed without acquiring instance lock",
            );
            write_status_file(&runtime_root, &report)?;
            print!("{}", report.render_kv());
            Ok(())
        }
        "probe" => {
            let runtime_root = parse_runtime_root(&args[1..])?;
            ensure_runtime_root(&runtime_root)?;
            let lock = try_acquire_instance(&runtime_root)?;
            let report = LifecycleReport::active_holder(
                &runtime_root,
                lock.pid(),
                "probe acquired instance lock and released on exit",
            );
            write_status_file(&runtime_root, &report)?;
            print!("{}", report.render_kv());
            drop(lock);
            let observation = observe_instance(&runtime_root)?;
            let released_report = LifecycleReport::from_observation(
                &runtime_root,
                &observation,
                None,
                "probe released instance lock",
            );
            write_status_file(&runtime_root, &released_report)?;
            Ok(())
        }
        "hold" | "login-hold" => {
            let opts = if command == "login-hold" {
                // Compact test-only startup command: avoid repeating long app-data
                // paths in a Windows Run value. Uses the same hold lifecycle.
                let runtime_root = parse_runtime_root(&args[1..])?;
                HoldOpts {
                    ready_file: Some(runtime_root.join("ready")),
                    mode: HoldMode::UntilStopFile(runtime_root.join("stop")),
                    runtime_root,
                }
            } else {
                parse_hold_opts(&args[1..])?
            };
            ensure_runtime_root(&opts.runtime_root)?;
            // Acquire the single-instance lock before any durable identity writes.
            let lock = try_acquire_instance(&opts.runtime_root)?;
            let install = prepare_m1_runtime_foundation(&opts.runtime_root)
                .map_err(|e| e.into_lifecycle())?;
            spawn_platform_ipc_server(&opts.runtime_root)?;
            let report = LifecycleReport::active_holder(
                &opts.runtime_root,
                lock.pid(),
                format!(
                    "holding instance lock for harness; install_id={}",
                    install.install_id
                ),
            );
            write_status_file(&opts.runtime_root, &report)?;
            print!("{}", report.render_kv());
            if let Some(ready) = &opts.ready_file {
                if let Some(parent) = ready.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(ready, format!("{}\n", lock.pid()))?;
            }

            match opts.mode {
                HoldMode::UntilStopFile(stop) => wait_for_stop_file(&stop),
                HoldMode::Duration(duration) => thread::sleep(duration),
                HoldMode::CrashAfterReady => {
                    // Intentional abnormal exit for crash-detection harnesses.
                    process::abort();
                }
            }

            drop(lock);
            let observation = observe_instance(&opts.runtime_root)?;
            let release_report = LifecycleReport::from_observation(
                &opts.runtime_root,
                &observation,
                None,
                "clean shutdown released instance lock",
            );
            write_status_file(&opts.runtime_root, &release_report)?;
            Ok(())
        }
        "compat" => {
            let (desktop, daemon, runtime_root) = parse_compat_args(&args[1..])?;
            let decision = evaluate_compat(ComponentVersions {
                desktop_protocol: desktop,
                daemon_protocol: daemon,
            })
            .map_err(|error| LifecycleError::Incompatible(error.message))?;

            println!("desktop_protocol={desktop}");
            println!("daemon_protocol={daemon}");
            println!("builtin_daemon_protocol={DAEMON_PROTOCOL_VERSION}");
            println!("supervision={}", selected_supervision().as_str());
            match decision {
                CompatDecision::Compatible => {
                    println!("compat=compatible");
                    if let Some(root) = runtime_root {
                        ensure_runtime_root(&root)?;
                        write_synthetic_marker(&root, "canonical-v1")?;
                        simulate_failed_update(&root)?;
                        println!(
                            "marker_after_failed_update={}",
                            read_synthetic_marker(&root)?
                        );
                        simulate_rollback(&root, "canonical-v1")?;
                        println!("marker_after_rollback={}", read_synthetic_marker(&root)?);
                        println!("canonical_marker_safe=true");
                    }
                    Ok(())
                }
                CompatDecision::Incompatible => Err(LifecycleError::Incompatible(format!(
                    "desktop protocol {desktop} is incompatible with daemon protocol {daemon}"
                ))),
            }
        }
        "update-proof" => {
            let opts = parse_update_proof_opts(&args[1..])?;
            ensure_runtime_root(&opts.runtime_root)?;
            let report =
                run_update_rollback_proof(&opts.runtime_root, &opts.base_bin, &opts.experiment_id)?;
            print!("{}", report.render_kv());
            Ok(())
        }
        other => Err(LifecycleError::Usage(format!(
            "unknown lifecycle command '{other}'\n{}",
            usage_text()
        ))),
    }
}

#[cfg(windows)]
fn spawn_platform_ipc_server(runtime_root: &Path) -> std::io::Result<()> {
    cryptexd::ipc::spawn_daemon_ipc_server(
        runtime_root,
        std::sync::Arc::new(cryptexd::WindowsCredentialStore::new()),
        SecureStoreEnvironment::Dev,
    )
}

#[cfg(target_os = "macos")]
fn spawn_platform_ipc_server(runtime_root: &Path) -> std::io::Result<()> {
    cryptexd::ipc::spawn_daemon_ipc_server(
        runtime_root,
        std::sync::Arc::new(cryptexd::MacosKeychainStore::new()),
        SecureStoreEnvironment::Dev,
    )
}

#[cfg(not(any(windows, target_os = "macos")))]
fn spawn_platform_ipc_server(_runtime_root: &Path) -> std::io::Result<()> {
    cryptexd::ipc::spawn_health_server()
}

#[derive(Debug)]
enum HoldMode {
    UntilStopFile(PathBuf),
    Duration(Duration),
    CrashAfterReady,
}

#[derive(Debug)]
struct HoldOpts {
    runtime_root: PathBuf,
    ready_file: Option<PathBuf>,
    mode: HoldMode,
}

fn parse_runtime_root(args: &[String]) -> Result<PathBuf, LifecycleError> {
    let mut runtime_root = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--runtime-root" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    LifecycleError::Usage("--runtime-root requires a path".into())
                })?;
                runtime_root = Some(PathBuf::from(value));
                index += 2;
            }
            other => {
                return Err(LifecycleError::Usage(format!(
                    "unexpected argument '{other}'"
                )));
            }
        }
    }

    let root = match runtime_root {
        Some(path) => path,
        None => default_p0_2_runtime_root()?,
    };
    assert_outside_repository(&root)?;
    Ok(root)
}

fn parse_hold_opts(args: &[String]) -> Result<HoldOpts, LifecycleError> {
    let mut runtime_root = None;
    let mut ready_file = None;
    let mut stop_file = None;
    let mut hold_ms = None;
    let mut crash_after_ready = false;

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--runtime-root" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    LifecycleError::Usage("--runtime-root requires a path".into())
                })?;
                runtime_root = Some(PathBuf::from(value));
                index += 2;
            }
            "--ready-file" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| LifecycleError::Usage("--ready-file requires a path".into()))?;
                ready_file = Some(PathBuf::from(value));
                index += 2;
            }
            "--stop-file" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| LifecycleError::Usage("--stop-file requires a path".into()))?;
                stop_file = Some(PathBuf::from(value));
                index += 2;
            }
            "--hold-ms" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| LifecycleError::Usage("--hold-ms requires an integer".into()))?;
                let parsed = value.parse::<u64>().map_err(|_| {
                    LifecycleError::Usage(format!("invalid --hold-ms value '{value}'"))
                })?;
                hold_ms = Some(parsed);
                index += 2;
            }
            "--crash-after-ready" => {
                crash_after_ready = true;
                index += 1;
            }
            other => {
                return Err(LifecycleError::Usage(format!(
                    "unexpected argument '{other}'"
                )));
            }
        }
    }

    let runtime_root = match runtime_root {
        Some(path) => path,
        None => default_p0_2_runtime_root()?,
    };
    assert_outside_repository(&runtime_root)?;

    let mode = if crash_after_ready {
        HoldMode::CrashAfterReady
    } else if let Some(stop) = stop_file {
        HoldMode::UntilStopFile(stop)
    } else if let Some(ms) = hold_ms {
        HoldMode::Duration(Duration::from_millis(ms))
    } else {
        return Err(LifecycleError::Usage(
            "lifecycle hold requires --stop-file, --hold-ms, or --crash-after-ready".into(),
        ));
    };

    Ok(HoldOpts {
        runtime_root,
        ready_file,
        mode,
    })
}

fn parse_compat_args(args: &[String]) -> Result<(u32, u32, Option<PathBuf>), LifecycleError> {
    let mut desktop = None;
    let mut daemon = None;
    let mut runtime_root = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--desktop-protocol" => {
                desktop = Some(parse_u32(args.get(index + 1), "--desktop-protocol")?);
                index += 2;
            }
            "--daemon-protocol" => {
                daemon = Some(parse_u32(args.get(index + 1), "--daemon-protocol")?);
                index += 2;
            }
            "--runtime-root" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    LifecycleError::Usage("--runtime-root requires a path".into())
                })?;
                runtime_root = Some(PathBuf::from(value));
                index += 2;
            }
            other => {
                return Err(LifecycleError::Usage(format!(
                    "unexpected argument '{other}'"
                )));
            }
        }
    }

    let desktop = desktop.ok_or_else(|| {
        LifecycleError::Usage("lifecycle compat requires --desktop-protocol".into())
    })?;
    let daemon = daemon.unwrap_or(DAEMON_PROTOCOL_VERSION);
    if let Some(root) = &runtime_root {
        assert_outside_repository(root)?;
    }
    Ok((desktop, daemon, runtime_root))
}

fn parse_u32(value: Option<&String>, flag: &str) -> Result<u32, LifecycleError> {
    let value =
        value.ok_or_else(|| LifecycleError::Usage(format!("{flag} requires an integer")))?;
    value
        .parse::<u32>()
        .map_err(|_| LifecycleError::Usage(format!("invalid {flag} value '{value}'")))
}

fn wait_for_stop_file(stop: &Path) {
    loop {
        if stop.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }
}

fn print_usage() {
    println!("{}", usage_text());
}

#[derive(Debug)]
struct UpdateProofOpts {
    runtime_root: PathBuf,
    base_bin: PathBuf,
    experiment_id: String,
}

fn parse_update_proof_opts(args: &[String]) -> Result<UpdateProofOpts, LifecycleError> {
    let mut runtime_root = None;
    let mut base_bin = None;
    let mut experiment_id = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--runtime-root" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    LifecycleError::Usage("--runtime-root requires a path".into())
                })?;
                runtime_root = Some(PathBuf::from(value));
                index += 2;
            }
            "--base-bin" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| LifecycleError::Usage("--base-bin requires a path".into()))?;
                base_bin = Some(PathBuf::from(value));
                index += 2;
            }
            "--experiment-id" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    LifecycleError::Usage("--experiment-id requires a value".into())
                })?;
                experiment_id = Some(value.clone());
                index += 2;
            }
            other => {
                return Err(LifecycleError::Usage(format!(
                    "unknown update-proof argument '{other}'"
                )));
            }
        }
    }

    let runtime_root = match runtime_root {
        Some(path) => {
            assert_outside_repository(&path)?;
            path
        }
        None => default_p0_2_runtime_root()?.join("update-rollback"),
    };
    let base_bin = base_bin.ok_or_else(|| {
        LifecycleError::Usage("lifecycle update-proof requires --base-bin".into())
    })?;
    let experiment_id = experiment_id.unwrap_or_else(|| {
        format!(
            "update-rollback-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        )
    });
    Ok(UpdateProofOpts {
        runtime_root,
        base_bin,
        experiment_id,
    })
}

fn usage_text() -> String {
    [
        "usage:",
        "  cryptexd",
        "  cryptexd identity enroll-local --runtime-root <app-data-path> --environment <dev|prod>",
        "  cryptexd pilot-fixture timeline-v1 --runtime-root <fresh-pilot-path> --confirm-isolated-pilot",
        "  cryptexd lifecycle status [--runtime-root <path>]",
        "  cryptexd lifecycle probe [--runtime-root <path>]",
        "  cryptexd lifecycle hold --runtime-root <path> (--stop-file <path> | --hold-ms <ms> | --crash-after-ready) [--ready-file <path>]",
        "  cryptexd lifecycle login-hold --runtime-root <path> (test-only; ready/stop under runtime root)",
        "  cryptexd lifecycle compat --desktop-protocol <n> [--daemon-protocol <n>] [--runtime-root <path>]",
        "  cryptexd lifecycle update-proof --base-bin <path> [--runtime-root <path>] [--experiment-id <id>]",
        "",
        "P0-2 notes:",
        "  - supervision strategy remains unresolved (ADR-0012 open)",
        "  - default runtime root is Cryptex-Dev/p0-2 (never inside the repository)",
        "  - platform registration adapters are placeholders only",
        "  - update-proof is a synthetic fixture harness, not a production updater",
        "  - pilot-fixture writes one public synthetic v1 note only to a fresh marked pilot runtime",
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fresh_pilot_root() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("cryptex-pilot-fixture-{}-{stamp}", process::id()));
        fs::create_dir(&root).expect("pilot root");
        fs::write(root.join("pilot-runtime.marker"), PILOT_RUNTIME_MARKER).expect("marker");
        root
    }

    #[test]
    fn local_enrollment_requires_explicit_runtime_and_secure_store_namespace() {
        let missing_both = run(vec!["identity".into(), "enroll-local".into()])
            .expect_err("must not choose a runtime root or keychain namespace implicitly");
        assert!(matches!(missing_both, LifecycleError::Usage(_)));

        let missing_environment = run(vec![
            "identity".into(),
            "enroll-local".into(),
            "--runtime-root".into(),
            "C:\\synthetic-runtime".into(),
        ])
        .expect_err("must require explicit SecureStore namespace");
        assert!(matches!(missing_environment, LifecycleError::Usage(_)));
    }

    #[test]
    fn local_enrollment_rejects_unknown_environment_without_touching_paths() {
        let error = run(vec![
            "identity".into(),
            "enroll-local".into(),
            "--runtime-root".into(),
            "C:\\synthetic-runtime".into(),
            "--environment".into(),
            "other".into(),
        ])
        .expect_err("unknown environment");
        assert!(matches!(error, LifecycleError::Usage(_)));
    }

    #[test]
    fn timeline_fixture_requires_a_fresh_marked_pilot_runtime() {
        let root = fresh_pilot_root();
        validate_pilot_fixture_root(&root).expect("fresh marked pilot");

        fs::write(root.join("unrelated.txt"), "preserve").expect("extra file");
        let error = validate_pilot_fixture_root(&root).expect_err("reused runtime");
        assert!(matches!(error, LifecycleError::UnsafeRuntimePath(_)));

        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn timeline_fixture_requires_explicit_confirmation_before_path_access() {
        let error = run(vec![
            "pilot-fixture".into(),
            "timeline-v1".into(),
            "--runtime-root".into(),
            "C:\\unused-pilot-runtime".into(),
        ])
        .expect_err("must require explicit fixture confirmation");
        assert!(matches!(error, LifecycleError::Usage(_)));
    }

    #[test]
    fn timeline_fixture_creates_an_accepted_v1_event_through_capture_service() {
        let root = fresh_pilot_root();
        prepare_m1_runtime_foundation(&root).expect("install identity");
        let store = cryptexd::FakeSecureStore::new();
        let (_, event_id) = seed_timeline_fixture_with_store(&root, &store).expect("seed fixture");

        assert_eq!(event_id.len(), 36);
        assert!(cryptexd::identity_runtime::brain_identity_path(&root).is_file());
        let canonical_events = root.join("canonical").join("events");
        let device_dir = fs::read_dir(&canonical_events)
            .expect("canonical event directory")
            .next()
            .expect("device directory")
            .expect("read device directory")
            .path();
        assert!(
            fs::read_dir(device_dir)
                .expect("device events")
                .any(|entry| entry.as_ref().is_ok_and(|entry| entry
                    .path()
                    .extension()
                    .is_some_and(|ext| ext == "open" || ext == "seg"))),
            "capture service must append a canonical segment"
        );

        drop(store);
        fs::remove_dir_all(root).expect("remove fixture");
    }
}
