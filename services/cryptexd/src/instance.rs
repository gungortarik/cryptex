use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use fs2::FileExt;

use crate::error::{LifecycleError, LifecycleResult};
use crate::paths::lock_path;
use crate::status::read_status_pid;

/// Exclusive single-instance lock for one local runtime context.
pub struct InstanceLock {
    _file: File,
    path: PathBuf,
    pid: u32,
}

impl InstanceLock {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceObservation {
    pub lock_present: bool,
    pub lock_held: bool,
    pub holder_pid: Option<u32>,
}

/// Attempt to become the sole daemon instance for `runtime_root`.
pub fn try_acquire_instance(runtime_root: &Path) -> LifecycleResult<InstanceLock> {
    fs::create_dir_all(runtime_root)?;
    let path = lock_path(runtime_root);
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&path)?;

    match file.try_lock_exclusive() {
        Ok(()) => {}
        Err(error) if is_lock_conflict(&error) => {
            // Do not read locked daemon.lock bytes (Windows ERROR_LOCK_VIOLATION).
            // Prefer the lifecycle.status PID side-channel while the live lock is held.
            let holder_pid = read_status_pid(runtime_root);
            return Err(LifecycleError::AlreadyRunning { holder_pid });
        }
        Err(error) => {
            return Err(LifecycleError::Io(format!(
                "failed to acquire instance lock {}: {error}",
                path.display()
            )));
        }
    }

    let pid = std::process::id();
    file.set_len(0)?;
    writeln!(file, "{pid}")?;
    file.sync_all()?;

    Ok(InstanceLock {
        _file: file,
        path,
        pid,
    })
}

/// Observe lock file state without becoming the instance holder.
///
/// Probe the exclusive lock first. While another process holds the lock, do not
/// read `daemon.lock` bytes — on Windows that can fail with ERROR_LOCK_VIOLATION
/// (os error 33). Holder PID comes from `lifecycle.status` in that case.
/// When the lock is free, reading stale PID bytes from `daemon.lock` remains safe
/// and deterministic.
pub fn observe_instance(runtime_root: &Path) -> LifecycleResult<InstanceObservation> {
    let path = lock_path(runtime_root);
    if !path.exists() {
        return Ok(InstanceObservation {
            lock_present: false,
            lock_held: false,
            holder_pid: None,
        });
    }

    let probe = OpenOptions::new().read(true).write(true).open(&path)?;
    match probe.try_lock_exclusive() {
        Ok(()) => {
            let _ = probe.unlock();
            let holder_pid = read_lock_pid(&path)?;
            Ok(InstanceObservation {
                lock_present: true,
                lock_held: false,
                holder_pid,
            })
        }
        Err(error) if is_lock_conflict(&error) => {
            let holder_pid = read_status_pid(runtime_root);
            Ok(InstanceObservation {
                lock_present: true,
                lock_held: true,
                holder_pid,
            })
        }
        Err(error) => Err(LifecycleError::Io(format!(
            "failed to observe instance lock {}: {error}",
            path.display()
        ))),
    }
}

fn is_lock_conflict(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    ) || matches!(error.raw_os_error(), Some(11) | Some(35) | Some(33))
}

fn read_lock_pid(path: &Path) -> LifecycleResult<Option<u32>> {
    let mut contents = String::new();
    let mut file = File::open(path)?;
    file.read_to_string(&mut contents)?;
    let trimmed = contents.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    Ok(trimmed.parse::<u32>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::assert_outside_repository;
    use crate::status::{LifecycleReport, write_status_file};
    use std::io;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_root(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("cryptexd-instance-{label}-{stamp}"));
        assert_outside_repository(&root).expect("temp runtime must be outside repository");
        fs::create_dir_all(&root).expect("create runtime");
        root
    }

    #[test]
    fn live_holder_is_observable_without_reading_locked_bytes() {
        let root = unique_root("live-holder");
        let lock = try_acquire_instance(&root).expect("acquire");
        let report = LifecycleReport::active_holder(&root, lock.pid(), "unit live holder");
        write_status_file(&root, &report).expect("status");

        let obs = observe_instance(&root).expect("observe must not fail while lock held");
        assert!(obs.lock_present);
        assert!(obs.lock_held);
        assert_eq!(obs.holder_pid, Some(lock.pid()));

        drop(lock);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn duplicate_instance_is_rejected_with_status_pid() {
        let root = unique_root("dup");
        let lock = try_acquire_instance(&root).expect("acquire");
        let report = LifecycleReport::active_holder(&root, lock.pid(), "unit duplicate");
        write_status_file(&root, &report).expect("status");

        match try_acquire_instance(&root) {
            Err(LifecycleError::AlreadyRunning { holder_pid }) => {
                assert_eq!(holder_pid, Some(lock.pid()));
            }
            Ok(_) => panic!("second acquire must fail"),
            Err(other) => panic!("unexpected error: {other}"),
        }

        drop(lock);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn released_and_stale_lock_are_observed_deterministically() {
        let root = unique_root("stale");
        let pid = {
            let lock = try_acquire_instance(&root).expect("acquire");
            let pid = lock.pid();
            let report = LifecycleReport::active_holder(&root, pid, "unit stale");
            write_status_file(&root, &report).expect("status");
            drop(lock);
            pid
        };

        let obs = observe_instance(&root).expect("observe released lock");
        assert!(obs.lock_present);
        assert!(!obs.lock_held);
        assert_eq!(obs.holder_pid, Some(pid), "stale PID bytes remain readable");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn crash_restart_path_proceeds_after_lock_release() {
        let root = unique_root("crash-restart");
        let first_pid = {
            let lock = try_acquire_instance(&root).expect("first");
            let pid = lock.pid();
            write_status_file(
                &root,
                &LifecycleReport::active_holder(&root, pid, "before crash"),
            )
            .expect("status");
            // Dropping the lock simulates process exit / crash releasing the OS lock.
            drop(lock);
            pid
        };

        let released = observe_instance(&root).expect("observe after release");
        assert!(!released.lock_held);

        let restarted = try_acquire_instance(&root).expect("restart acquire");
        assert_eq!(restarted.pid(), std::process::id());
        let _ = first_pid;

        let held = observe_instance(&root).expect("observe restarted holder");
        // Status still has old pid until rewritten; lock probe is authoritative.
        assert!(held.lock_held);

        write_status_file(
            &root,
            &LifecycleReport::active_holder(&root, restarted.pid(), "after restart"),
        )
        .expect("rewrite status");
        let held2 = observe_instance(&root).expect("observe with fresh status");
        assert_eq!(held2.holder_pid, Some(restarted.pid()));

        drop(restarted);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn only_recognized_lock_conflicts_mean_held() {
        assert!(is_lock_conflict(&io::Error::from(
            io::ErrorKind::WouldBlock
        )));
        assert!(is_lock_conflict(&io::Error::from_raw_os_error(33)));
        assert!(is_lock_conflict(&io::Error::from_raw_os_error(11)));
        assert!(is_lock_conflict(&io::Error::from_raw_os_error(35)));
        assert!(!is_lock_conflict(&io::Error::from(
            io::ErrorKind::PermissionDenied
        )));
        assert!(!is_lock_conflict(&io::Error::from(io::ErrorKind::NotFound)));
    }

    #[cfg(windows)]
    #[test]
    fn windows_locked_byte_read_is_error_33_but_observe_succeeds() {
        let root = unique_root("win-err33");
        let lock = try_acquire_instance(&root).expect("acquire");
        write_status_file(
            &root,
            &LifecycleReport::active_holder(&root, lock.pid(), "win err33"),
        )
        .expect("status");

        // Direct byte read of the locked region fails with ERROR_LOCK_VIOLATION.
        let lock_path = lock_path(&root);
        let read_err = File::open(&lock_path)
            .and_then(|mut f| {
                let mut buf = String::new();
                f.read_to_string(&mut buf)?;
                Ok(buf)
            })
            .expect_err("reading locked daemon.lock bytes must fail on Windows");
        assert_eq!(
            read_err.raw_os_error(),
            Some(33),
            "expected ERROR_LOCK_VIOLATION (33), got: {read_err}"
        );

        // Observation must succeed via lock probe + status side-channel.
        let obs = observe_instance(&root).expect("observe must tolerate Windows lock bytes");
        assert!(obs.lock_held);
        assert_eq!(obs.holder_pid, Some(lock.pid()));

        drop(lock);
        let _ = fs::remove_dir_all(&root);
    }
}
