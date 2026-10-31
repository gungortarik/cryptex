//! Test-only abrupt interruption helpers for cross-process restart proofs.
//!
//! These helpers exit the current process at a named durability boundary after
//! writing a handshake file. They are not power-loss proofs and must not be
//! compiled into non-test builds.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

/// Environment variable naming the capture durability boundary at which to exit.
pub const INTERRUPT_ENV: &str = "CRYPTEX_TEST_CAPTURE_INTERRUPT";

/// Handshake file written under the runtime root before abrupt exit.
pub const READY_NAME: &str = "capture-interrupt-ready.txt";

/// Exit code used after a successful interrupt handshake.
pub const INTERRUPT_EXIT_CODE: i32 = 97;

/// If `CRYPTEX_TEST_CAPTURE_INTERRUPT` matches `boundary`, sync a ready marker
/// under `runtime_root` and exit without returning through normal cleanup.
pub fn abrupt_exit_at_boundary(boundary: &str, runtime_root: &Path, detail: &str) -> ! {
    let ready = runtime_root.join(READY_NAME);
    let payload = format!("{boundary}\n{detail}\n");
    {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&ready)
            .unwrap_or_else(|e| panic!("interrupt ready open failed: {e}"));
        file.write_all(payload.as_bytes())
            .unwrap_or_else(|e| panic!("interrupt ready write failed: {e}"));
        file.sync_all()
            .unwrap_or_else(|e| panic!("interrupt ready sync failed: {e}"));
    }
    if let Some(parent) = ready.parent()
        && let Ok(dir) = OpenOptions::new().read(true).open(parent)
    {
        let _ = dir.sync_all();
    }
    std::process::exit(INTERRUPT_EXIT_CODE);
}

/// Exit at `boundary` when the interrupt env is armed for it.
pub fn maybe_abrupt_exit(boundary: &str, runtime_root: &Path, detail: &str) {
    if std::env::var(INTERRUPT_ENV).ok().as_deref() == Some(boundary) {
        abrupt_exit_at_boundary(boundary, runtime_root, detail);
    }
}

/// Wait until `path` exists or `timeout` elapses.
pub fn wait_for_path(path: &Path, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if path.exists() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    path.exists()
}

/// Wait for a child with a wall-clock bound; kill it if the bound is exceeded.
pub fn wait_child_bounded(
    child: &mut std::process::Child,
    timeout: Duration,
) -> std::io::Result<std::process::ExitStatus> {
    let start = Instant::now();
    loop {
        match child.try_wait()? {
            Some(status) => return Ok(status),
            None if start.elapsed() >= timeout => {
                let _ = child.kill();
                return child.wait();
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    }
}

/// Read interrupt handshake payload (boundary + detail).
pub fn read_ready(runtime_root: &Path) -> (String, String) {
    let text = fs::read_to_string(runtime_root.join(READY_NAME)).expect("read interrupt ready");
    let mut lines = text.lines();
    let boundary = lines.next().unwrap_or("").to_string();
    let detail = lines.next().unwrap_or("").to_string();
    (boundary, detail)
}
