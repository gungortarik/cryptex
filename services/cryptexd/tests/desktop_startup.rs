//! Shared production Desktop launch/readiness path with real child processes.
//! This does not establish macOS owner-operation authorization.

use cryptexd::daemon_startup::{StartupError, await_ready};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct Runtime(PathBuf);
impl Runtime {
    fn new(label: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "cryptex-startup-{label}-{}-{stamp}",
            std::process::id()
        ));
        cryptexd::paths::assert_outside_repository(&root).unwrap();
        fs::create_dir(&root).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        }
        Self(root)
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn worker(root: &Runtime, mode: &str) -> Child {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "startup_worker", "--nocapture"])
        .env("CRYPTEX_STARTUP_FIXTURE_ROOT", &root.0)
        .env("CRYPTEX_STARTUP_FIXTURE_MODE", mode)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    command.spawn().unwrap()
}

fn wait_file(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !path.exists() {
        assert!(
            Instant::now() < deadline,
            "fixture synchronization deadline"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn startup_worker() {
    let Some(root) = std::env::var_os("CRYPTEX_STARTUP_FIXTURE_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    let mode = std::env::var("CRYPTEX_STARTUP_FIXTURE_MODE").unwrap();
    if mode == "exit" {
        return;
    }
    if mode == "partial" {
        fs::write(root.join("ready"), b"").unwrap();
        fs::write(root.join("empty-created"), b"ready").unwrap();
        wait_file(&root.join("release-empty"));
        fs::write(root.join("ready"), std::process::id().to_string()).unwrap();
        fs::write(root.join("partial-created"), b"ready").unwrap();
        wait_file(&root.join("release-partial"));
        fs::write(root.join("ready"), format!("{}\n", std::process::id())).unwrap();
    } else if mode == "wrong" {
        fs::write(root.join("ready"), b"1\n").unwrap();
    } else if mode == "oversized" {
        fs::write(root.join("ready"), [b'x'; 65]).unwrap();
    } else if mode == "directory" {
        fs::create_dir(root.join("ready")).unwrap();
    } else if mode == "empty" || mode == "fixed-partial" {
        let bytes = if mode == "empty" {
            String::new()
        } else {
            std::process::id().to_string()
        };
        fs::write(root.join("ready"), bytes).unwrap();
        fs::write(root.join("fixture-prepared"), b"ready").unwrap();
    }
    // Refusal tests must kill/reap this still-running process.
    thread::sleep(Duration::from_secs(15));
    panic!("fixture survived its supervisor");
}

#[test]
fn empty_and_partial_pid_wait_for_complete_supervised_child_readiness() {
    let root = Runtime::new("partial");
    let child = worker(&root, "partial");
    let pid = child.id();
    wait_file(&root.0.join("empty-created"));
    let ready = root.0.join("ready");
    let (tx, rx) = std::sync::mpsc::channel();
    let waiter = thread::spawn(move || {
        tx.send(await_ready(child, &ready, Duration::from_secs(5)))
            .unwrap();
    });
    assert!(
        rx.recv_timeout(Duration::from_millis(80)).is_err(),
        "empty ready must not finish startup"
    );
    fs::write(root.0.join("release-empty"), b"go").unwrap();
    wait_file(&root.0.join("partial-created"));
    assert!(
        rx.recv_timeout(Duration::from_millis(80)).is_err(),
        "partial PID must not finish startup"
    );
    fs::write(root.0.join("release-partial"), b"go").unwrap();
    let child = OwnedChild(rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap());
    assert_eq!(child.0.id(), pid);
    waiter.join().unwrap();
}

#[test]
fn held_incomplete_readiness_reaches_deadline_instead_of_refusal_or_success() {
    for mode in ["empty", "fixed-partial"] {
        let root = Runtime::new(mode);
        let child = worker(&root, mode);
        wait_file(&root.0.join("fixture-prepared"));
        // No scheduler timing assumption: the fixture never completes its line.
        assert_eq!(
            await_ready(child, &root.0.join("ready"), Duration::ZERO).unwrap_err(),
            StartupError::Deadline
        );
    }
}

#[test]
fn mismatched_complete_pid_is_refused() {
    let root = Runtime::new("wrong");
    assert_eq!(
        await_ready(
            worker(&root, "wrong"),
            &root.0.join("ready"),
            Duration::from_secs(5)
        )
        .unwrap_err(),
        StartupError::ReadinessMismatch
    );
}

#[test]
fn no_ready_file_is_not_success() {
    let root = Runtime::new("missing");
    assert_eq!(
        await_ready(
            worker(&root, "missing"),
            &root.0.join("ready"),
            Duration::from_millis(150)
        )
        .unwrap_err(),
        StartupError::Deadline
    );
}

#[test]
fn exited_child_is_not_a_successful_start() {
    let root = Runtime::new("exit");
    assert_eq!(
        await_ready(
            worker(&root, "exit"),
            &root.0.join("ready"),
            Duration::from_secs(5)
        )
        .unwrap_err(),
        StartupError::ExitedBeforeReady
    );
}

#[test]
fn oversized_ready_is_refused() {
    let root = Runtime::new("oversized");
    assert_eq!(
        await_ready(
            worker(&root, "oversized"),
            &root.0.join("ready"),
            Duration::from_secs(5)
        )
        .unwrap_err(),
        StartupError::InvalidReadiness
    );
}

#[test]
fn nonregular_ready_is_refused() {
    let root = Runtime::new("directory");
    let error = await_ready(
        worker(&root, "directory"),
        &root.0.join("ready"),
        Duration::from_secs(5),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        StartupError::InvalidReadiness | StartupError::ReadinessIo
    ));
}

#[cfg(unix)]
#[test]
fn fifo_ready_does_not_block_and_symlink_is_refused() {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    for symlink in [false, true] {
        let root = Runtime::new("special");
        let ready = root.0.join("ready");
        if symlink {
            fs::write(root.0.join("target"), b"1\n").unwrap();
            std::os::unix::fs::symlink(root.0.join("target"), &ready).unwrap();
        } else {
            let path = CString::new(ready.as_os_str().as_bytes()).unwrap();
            assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
        }
        let error =
            await_ready(worker(&root, "missing"), &ready, Duration::from_secs(5)).unwrap_err();
        assert!(matches!(
            error,
            StartupError::InvalidReadiness | StartupError::ReadinessIo
        ));
    }
}

#[test]
#[ignore = "binds the per-user product endpoint; requires explicitly disposable account"]
fn actual_daemon_crash_restart_requires_fresh_readiness_and_connection() {
    // A failed health request cannot prove this endpoint is unused. Never let
    // ordinary developer test runs overwrite a slow owner's daemon socket.
    assert_eq!(
        std::env::var("CRYPTEX_STARTUP_DISPOSABLE_ACCOUNT").as_deref(),
        Ok("1")
    );
    assert_eq!(std::env::var("GITHUB_ACTIONS").as_deref(), Ok("true"));
    assert_eq!(
        std::env::var("RUNNER_ENVIRONMENT").as_deref(),
        Ok("github-hosted")
    );
    let root = Runtime::new("daemon");
    assert!(
        cryptexd::ipc::desktop_health_ping(Duration::from_millis(50)).is_err(),
        "a daemon already uses this test account's endpoint; leave it untouched"
    );
    let start = || {
        let ready = root.0.join("ready");
        match fs::remove_file(&ready) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!("ready cleanup: {error}"),
        }
        let mut command = Command::new(env!("CARGO_BIN_EXE_cryptexd"));
        command
            .args(["lifecycle", "hold", "--runtime-root"])
            .arg(&root.0)
            .arg("--ready-file")
            .arg(&ready)
            .arg("--stop-file")
            .arg(root.0.join("stop"))
            .stdout(Stdio::null())
            .stderr(Stdio::inherit());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        let child = command.spawn().unwrap();
        OwnedChild(await_ready(child, &ready, Duration::from_secs(5)).unwrap())
    };
    let mut first = start();
    let check_connection_contract = |expected_pid| {
        for role in ["desktop", "cli", "mcp"] {
            let ping = cryptexd::ipc::health_ping(role, Duration::from_secs(2)).unwrap();
            assert_eq!(ping.daemon_pid, expected_pid);
        }
        // The public health client rejects an unknown role before opening a
        // stream. Keep that local guard distinct from a server-side refusal.
        let rejected = cryptexd::ipc::health_ping("unknown", Duration::from_secs(2)).unwrap_err();
        assert_eq!(rejected.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(rejected.to_string(), "unsupported local client role");
        // This real wire request claims the Desktop role but the harness is
        // not the packaged Desktop. A successful health reconnect must never
        // turn it into an authenticated content client.
        let denied = cryptexd::ipc::desktop_search(
            cryptexd::ipc::DesktopSearchQuery::default(),
            Duration::from_secs(2),
        )
        .unwrap();
        assert!(!denied.accepted);
        assert_eq!(denied.daemon_pid, expected_pid);
        assert_eq!(denied.error.as_deref(), Some("unauthorized_client_process"));
        assert!(denied.hits.is_none());
    };
    check_connection_contract(first.0.id());
    #[cfg(unix)]
    let mut old = {
        use interprocess::local_socket::{GenericFilePath, Stream, prelude::*};
        let name = PathBuf::from(format!("/tmp/cpx-cryptex-{}/s", unsafe { libc::getuid() }))
            .to_fs_name::<GenericFilePath>()
            .unwrap();
        Stream::connect(name).unwrap()
    };
    first.0.kill().unwrap();
    first.0.wait().unwrap();
    let second = start();
    assert_ne!(first.0.id(), second.0.id());
    check_connection_contract(second.0.id());
    #[cfg(unix)]
    {
        use interprocess::local_socket::prelude::*;
        use std::io::{Read, Write};
        old.set_nonblocking(true).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut byte = [0];
        loop {
            match old.read(&mut byte) {
                Ok(0) => break,
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && Instant::now() < deadline =>
                {
                    thread::sleep(Duration::from_millis(10))
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::ConnectionReset
                            | std::io::ErrorKind::BrokenPipe
                            | std::io::ErrorKind::NotConnected
                    ) =>
                {
                    break;
                }
                result => panic!("old actual connection did not close: {result:?}"),
            }
        }
        assert!(old.write_all(b"{}\n").is_err());
    }
    // Both runs operate with no Brain identity and write no canonical events.
    assert!(!root.0.join("canonical/events").exists());
}
