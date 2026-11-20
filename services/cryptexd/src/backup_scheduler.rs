//! One pending backup on an explicitly designated device. Currently wired
//! only to the disposable M11 daemon profile runner.
pub(crate) mod journal;
pub(crate) mod retention;
use cryptex_events::LocalTrustAnchor;
use sha2::{Digest, Sha256};
use std::fs::OpenOptions;
use std::io;
use std::path::Path;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

pub(crate) struct PendingBackup<'a> {
    pub designated_writer: &'a LocalTrustAnchor,
    pub previous_generation: &'a Path,
    pub previous_sha256: [u8; 32],
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    NotDesignated,
    Published,
    Stopped,
}

fn generation_digest(path: &Path) -> io::Result<[u8; 32]> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let mut file = options.open(path)?;
    let metadata = file.metadata()?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes()
            & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
            != 0
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "linked backup generation",
            ));
        }
    }
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid backup generation",
        ));
    }
    let mut digest = Sha256::new();
    io::copy(&mut file, &mut digest)?;
    Ok(digest.finalize().into())
}

impl PendingBackup<'_> {
    fn target_is_ready(&self) -> io::Result<bool> {
        let digest = match generation_digest(self.previous_generation) {
            Ok(digest) => digest,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error),
        };
        if digest != self.previous_sha256 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "previous backup differs from the selected generation",
            ));
        }
        Ok(true)
    }

    /// Run on a background worker. Dropping the sender cancels a waiting job.
    pub fn run(
        &self,
        enrolled: &LocalTrustAnchor,
        stop: &Receiver<()>,
        on_waiting: impl FnOnce(),
        publish: impl FnOnce() -> io::Result<()>,
    ) -> io::Result<Outcome> {
        if enrolled != self.designated_writer {
            return Ok(Outcome::NotDesignated);
        }
        if !self.target_is_ready()? {
            on_waiting();
            loop {
                match stop.recv_timeout(Duration::from_millis(250)) {
                    Err(RecvTimeoutError::Timeout) if self.target_is_ready()? => break,
                    Err(RecvTimeoutError::Timeout) => continue,
                    Ok(()) | Err(RecvTimeoutError::Disconnected) => return Ok(Outcome::Stopped),
                }
            }
        }
        publish()?;
        Ok(Outcome::Published)
    }
}
