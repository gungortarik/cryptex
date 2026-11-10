//! Supervised-child readiness. A ready file is a progress signal, not peer
//! authentication: callers still use the platform IPC authentication policy.

use std::fs;
use std::io;
use std::io::Read;
use std::path::Path;
use std::process::Child;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupError {
    ReadinessMismatch,
    InvalidReadiness,
    ExitedBeforeReady,
    Deadline,
    ReadinessIo,
    ChildObservation,
    ChildCleanup,
}

impl std::fmt::Display for StartupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "daemon startup: {self:?}")
    }
}

impl std::error::Error for StartupError {}

fn read_progress(ready: &Path) -> Result<Option<Vec<u8>>, StartupError> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = match options.open(ready) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(StartupError::ReadinessIo),
    };
    let metadata = file.metadata().map_err(|_| StartupError::ReadinessIo)?;
    if !metadata.is_file() || metadata.len() > 64 {
        return Err(StartupError::InvalidReadiness);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(StartupError::InvalidReadiness);
        }
    }
    let mut bytes = Vec::with_capacity(65);
    file.take(65)
        .read_to_end(&mut bytes)
        .map_err(|_| StartupError::ReadinessIo)?;
    if bytes.len() > 64 {
        return Err(StartupError::InvalidReadiness);
    }
    Ok(Some(bytes))
}

/// Take ownership so every refusal terminates and reaps this exact child.
/// An empty/partial ready file can be observed while the daemon writes its PID;
/// only a newline-terminated value is treated as a complete readiness claim.
pub fn await_ready(
    mut child: Child,
    ready: &Path,
    timeout: Duration,
) -> Result<Child, StartupError> {
    let deadline = Instant::now() + timeout;
    let result = loop {
        // Observe exit before trusting a leftover ready file.
        match child.try_wait() {
            Ok(Some(_)) => break Err(StartupError::ExitedBeforeReady),
            Err(_) => break Err(StartupError::ChildObservation),
            Ok(None) => {}
        }
        match read_progress(ready) {
            Ok(Some(bytes)) if bytes.ends_with(b"\n") => {
                let pid = std::str::from_utf8(&bytes)
                    .ok()
                    .and_then(|text| text.trim().parse::<u32>().ok());
                if pid != Some(child.id()) {
                    break Err(StartupError::ReadinessMismatch);
                }
                // Close the exit-versus-read observation window as far as a
                // readiness check can. Later exit is handled by supervision.
                match child.try_wait() {
                    Ok(None) => break Ok(()),
                    Ok(Some(_)) => break Err(StartupError::ExitedBeforeReady),
                    Err(_) => break Err(StartupError::ChildObservation),
                }
            }
            Ok(_) => {}
            Err(error) => break Err(error),
        }
        if Instant::now() >= deadline {
            break Err(StartupError::Deadline);
        }
        thread::sleep(Duration::from_millis(10));
    };
    match result {
        Ok(()) => Ok(child),
        Err(error) => {
            // kill may lose a race to normal exit; wait is authoritative and
            // also reaps an already observed exit. Never kill a PID by number.
            let _ = child.kill();
            child.wait().map_err(|_| StartupError::ChildCleanup)?;
            Err(error)
        }
    }
}
