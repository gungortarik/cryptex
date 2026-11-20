//! One pending backup on an explicitly designated device. Currently wired
//! only to the disposable M11 daemon profile runner.
use cryptex_events::LocalTrustAnchor;
use sha2::{Digest, Sha256};
use std::fs::File;
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

impl PendingBackup<'_> {
    fn target_is_ready(&self) -> io::Result<bool> {
        let mut file = match File::open(self.previous_generation) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error),
        };
        let mut digest = Sha256::new();
        io::copy(&mut file, &mut digest)?;
        if <[u8; 32]>::from(digest.finalize()) != self.previous_sha256 {
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
