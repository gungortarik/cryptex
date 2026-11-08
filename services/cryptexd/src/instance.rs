use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use fs2::FileExt;

use crate::error::{LifecycleError, LifecycleResult};
use crate::paths::lock_path;
use crate::status::{read_status_pid, read_status_recorded_pid};

/// Exclusive single-instance lock for one local runtime context.
pub struct InstanceLock {
    _file: Arc<File>,
    path: PathBuf,
    pid: u32,
    admission: crate::runtime_admission::RuntimeAdmission,
}

impl InstanceLock {
    /// Stop ordinary admission and wait for actual workers/response writes,
    /// rather than treating a request timeout as completion. A failed drain is
    /// an attention outcome; Drop still retains the OS lock in active leases.
    pub fn drain_runtime(&self, timeout: std::time::Duration) -> LifecycleResult<()> {
        let deadline = std::time::Instant::now()
            .checked_add(timeout)
            .ok_or_else(|| LifecycleError::Attention("invalid runtime drain deadline".into()))?;
        self.admission
            .stop_and_drain(deadline)
            .map_err(|error| LifecycleError::Attention(error.to_string()))
    }

    pub(crate) fn admission(&self) -> crate::runtime_admission::RuntimeAdmission {
        self.admission.clone()
    }
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }
}

impl Drop for InstanceLock {
    fn drop(&mut self) {
        self.admission.stop();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceObservation {
    pub lock_present: bool,
    pub lock_held: bool,
    /// PID associated with a currently held lock, when available.
    pub holder_pid: Option<u32>,
    /// PID retained in the lock/status metadata; this may be stale.
    pub recorded_pid: Option<u32>,
}

/// Attempt to become the sole daemon instance for `runtime_root`.
pub fn try_acquire_instance(runtime_root: &Path) -> LifecycleResult<InstanceLock> {
    fs::create_dir_all(runtime_root)?;
    let path = lock_path(runtime_root);
    let mut file = crate::platform::instance_file::open(&path, true)?;

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

    crate::platform::instance_file::validate(&file)?;
    let pid = std::process::id();
    file.set_len(0)?;
    writeln!(file, "{pid}")?;
    file.sync_all()?;

    let file = Arc::new(file);
    let admission =
        crate::runtime_admission::RuntimeAdmission::new(runtime_root, Arc::downgrade(&file));
    Ok(InstanceLock {
        _file: file,
        path,
        pid,
        admission,
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
            recorded_pid: None,
        });
    }

    let mut probe = crate::platform::instance_file::open(&path, false)?;
    match probe.try_lock_exclusive() {
        Ok(()) => {
            let holder_pid = read_lock_pid(&mut probe)?;
            let _ = probe.unlock();
            Ok(InstanceObservation {
                lock_present: true,
                lock_held: false,
                holder_pid: None,
                recorded_pid: holder_pid,
            })
        }
        Err(error) if is_lock_conflict(&error) => {
            let holder_pid = read_status_pid(runtime_root);
            let recorded_pid = read_status_recorded_pid(runtime_root).or(holder_pid);
            Ok(InstanceObservation {
                lock_present: true,
                lock_held: true,
                holder_pid,
                recorded_pid,
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

fn read_lock_pid(file: &mut File) -> LifecycleResult<Option<u32>> {
    crate::platform::instance_file::validate(file)?;
    let mut contents = String::new();
    file.take(129).read_to_string(&mut contents)?;
    if contents.len() > 128 {
        return Err(LifecycleError::UnsafeRuntimePath(
            "instance PID file exceeds its read bound".into(),
        ));
    }
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

    fn unique_root(label: &str) -> PathBuf {
        let root = crate::test_temp::exclusive_temp_dir(&format!("cryptexd-instance-{label}"));
        assert_outside_repository(&root).expect("temp runtime must be outside repository");
        root
    }

    #[test]
    fn hardlinked_instance_file_cannot_truncate_an_immutable_file() {
        let root = unique_root("hardlink-refusal");
        let canonical = root.join("synthetic-canonical.seg");
        let original = b"synthetic immutable canonical bytes\n";
        fs::write(&canonical, original).unwrap();
        fs::hard_link(&canonical, lock_path(&root)).unwrap();
        assert!(
            observe_instance(&root).is_err(),
            "observation must refuse linked lock files too"
        );
        let result = try_acquire_instance(&root);
        let rejected = result.is_err();
        drop(result);
        let observed = fs::read(&canonical).unwrap();
        fs::remove_dir_all(root).unwrap();
        assert!(
            rejected,
            "hardlinked instance file must be refused before PID writes"
        );
        assert_eq!(observed, original);
    }

    #[test]
    fn oversized_pid_observation_is_bounded_and_preserves_bytes() {
        let root = unique_root("oversized-pid");
        let original = vec![b'1'; 1024];
        fs::write(lock_path(&root), &original).unwrap();
        assert!(observe_instance(&root).is_err());
        assert_eq!(fs::read(lock_path(&root)).unwrap(), original);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_instance_file_is_refused_without_touching_target() {
        let root = unique_root("symlink-refusal");
        let target = root.join("synthetic-canonical.seg");
        fs::write(&target, b"synthetic immutable bytes").unwrap();
        std::os::unix::fs::symlink(&target, lock_path(&root)).unwrap();
        assert!(try_acquire_instance(&root).is_err());
        assert!(observe_instance(&root).is_err());
        assert_eq!(fs::read(target).unwrap(), b"synthetic immutable bytes");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[cfg(windows)]
    fn windows_live_instance_file_cannot_be_replaced() {
        let root = unique_root("pinned-lock-name");
        let guard = try_acquire_instance(&root).unwrap();
        let moved = root.join("renamed-daemon.lock");
        assert!(fs::rename(lock_path(&root), &moved).is_err());
        assert!(lock_path(&root).is_file());
        drop(guard);
        fs::rename(lock_path(&root), moved).unwrap();
        fs::remove_dir_all(root).unwrap();
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
        assert_eq!(obs.holder_pid, None, "stale PID is not a live holder");
        assert_eq!(obs.recorded_pid, Some(pid), "stale PID remains diagnosable");
        let report = LifecycleReport::from_observation(&root, &obs, None, "stale");
        assert_eq!(report.pid, None);
        assert_eq!(report.recorded_pid, Some(pid));

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
        assert_eq!(released.holder_pid, None);
        assert_eq!(released.recorded_pid, Some(first_pid));

        let restarted = try_acquire_instance(&root).expect("restart acquire");
        assert_eq!(restarted.pid(), std::process::id());
        let _ = first_pid;

        let held = observe_instance(&root).expect("observe restarted holder");
        // Status still has old pid until rewritten; lock probe is authoritative.
        assert!(held.lock_held);
        assert_eq!(held.holder_pid, Some(first_pid));

        write_status_file(
            &root,
            &LifecycleReport::active_holder(&root, restarted.pid(), "after restart"),
        )
        .expect("rewrite status");
        let held2 = observe_instance(&root).expect("observe with fresh status");
        assert_eq!(held2.holder_pid, Some(restarted.pid()));
        assert_eq!(held2.recorded_pid, Some(restarted.pid()));

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
        assert_eq!(obs.recorded_pid, Some(lock.pid()));

        drop(lock);
        let _ = fs::remove_dir_all(&root);
    }
}
