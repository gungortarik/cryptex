use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use fs2::FileExt;

use crate::error::{LifecycleError, LifecycleResult};
use crate::paths::lock_path;

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
            let holder_pid = read_lock_pid(&path).ok().flatten();
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
pub fn observe_instance(runtime_root: &Path) -> LifecycleResult<InstanceObservation> {
    let path = lock_path(runtime_root);
    if !path.exists() {
        return Ok(InstanceObservation {
            lock_present: false,
            lock_held: false,
            holder_pid: None,
        });
    }

    let holder_pid = read_lock_pid(&path)?;
    let probe = OpenOptions::new().read(true).write(true).open(&path)?;
    match probe.try_lock_exclusive() {
        Ok(()) => {
            let _ = probe.unlock();
            Ok(InstanceObservation {
                lock_present: true,
                lock_held: false,
                holder_pid,
            })
        }
        Err(_) => Ok(InstanceObservation {
            lock_present: true,
            lock_held: true,
            holder_pid,
        }),
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
