//! A note-purge confirmation is never authority to remove a whole generation.
use super::*;
use crate::knowledge_materialization::backup::{
    preserves_after_purge, retention_state, role::DeviceRole,
};
use std::io::{Seek, Write};
use std::time::{Duration, Instant};

pub(crate) struct Review {
    target: std::path::PathBuf,
    generation: u64,
    digest: [u8; 32],
    replacement: u64,
    replacement_digest: [u8; 32],
    head: [u8; 32],
    purges: [u8; 32],
    expires: Instant,
}

pub(crate) enum Decision {
    #[cfg(test)]
    Cancel,
    Confirm(std::path::PathBuf),
}

impl Review {
    pub(crate) fn details(&self) -> io::Result<(crate::ipc::backup_removal::Selection, Instant)> {
        let display = |p: std::path::PathBuf| {
            p.into_os_string().into_string().map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "backup path cannot be displayed",
                )
            })
        };
        Ok((
            crate::ipc::backup_removal::Selection {
                old_file: display(self.selected_file())?,
                old_digest: format!("{:x}", sha2::digest::Output::<Sha256>::from(self.digest)),
                replacement_file: display(self.target.join(generation_name(self.replacement)))?,
                replacement_digest: format!(
                    "{:x}",
                    sha2::digest::Output::<Sha256>::from(self.replacement_digest)
                ),
            },
            self.expires,
        ))
    }

    pub(crate) fn selected_file(&self) -> std::path::PathBuf {
        self.target.join(generation_name(self.generation))
    }
}

fn denied() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "backup removal requires current owner review",
    )
}

impl VerifiedBackups<'_> {
    fn receipt(&self, number: u64) -> io::Result<Receipt> {
        let service = ServiceLabel::device_credentials(self.environment);
        let bytes = self
            .store
            .get(service.as_str(), self.account(Some(number))?.as_str())
            .map_err(io::Error::other)?;
        serde_json::from_slice(&bytes).map_err(|_| denied())
    }

    /// The daemon supplies its authenticated Kit reader. Both private copies are
    /// restored again for review, including their signed inventory and blobs.
    pub(crate) fn review_removal(
        &self,
        enrolled: &LocalTrustAnchor,
        old: u64,
        new: u64,
        mut restore: impl FnMut(u64, &Path) -> io::Result<tempfile::TempDir>,
    ) -> io::Result<Review> {
        self.require_writer(enrolled)?;
        if new <= old {
            return Err(denied());
        }
        let dir = Directory::open(self.target).map_err(directory_error)?;
        let role = DeviceRole {
            store: self.store,
            env: self.environment,
            local: enrolled,
        };
        let head = role
            .assigned_head(self.target)
            .map_err(super::super::role_error)?;
        let mut reconstruct = |number, receipt: &Receipt| -> io::Result<_> {
            let mut source = checked_generation(&dir, number, receipt.digest)?;
            source.rewind()?;
            let scratch = tempfile::tempdir()?;
            let mut copy = tempfile::NamedTempFile::new_in(scratch.path())?;
            io::copy(&mut source, &mut copy)?;
            copy.flush()?;
            let path = copy.into_temp_path();
            if super::super::generation_digest(&path)? != receipt.digest {
                return Err(denied());
            }
            restore(number, &path)
        };
        let prior = self.receipt(old)?;
        let next = self.receipt(new)?;
        let previous = reconstruct(old, &prior)?;
        let replacement = reconstruct(new, &next)?;
        let (a, first_purges) = retention_state(previous.path(), enrolled).map_err(|_| denied())?;
        let (b, next_purges) =
            retention_state(replacement.path(), enrolled).map_err(|_| denied())?;
        let (_current, current_purges) =
            retention_state(self.writer_profile, enrolled).map_err(|_| denied())?;
        if prior.purges != first_purges
            || current_purges != next_purges
            || next.purges != next_purges
            || !preserves_after_purge(&a, &b).map_err(|_| denied())?
        {
            return Err(denied());
        }
        Ok(Review {
            target: self.target.to_owned(),
            generation: old,
            digest: prior.digest,
            replacement: new,
            replacement_digest: next.digest,
            head,
            purges: current_purges,
            expires: Instant::now() + Duration::from_secs(120),
        })
    }

    /// A native owner session keeps the review private. Consuming it once also
    /// consumes a cancellation. A restarted session must prepare a new review.
    pub(crate) fn decide_removal(
        &self,
        enrolled: &LocalTrustAnchor,
        review: Review,
        decision: Decision,
    ) -> io::Result<bool> {
        match decision {
            #[cfg(test)]
            Decision::Cancel => return Ok(false),
            Decision::Confirm(selected)
                if self.target == review.target && selected == review.selected_file() => {}
            Decision::Confirm(_) => return Err(denied()),
        }
        self.require_writer(enrolled)?;
        let dir = Directory::open(self.target).map_err(directory_error)?;
        let _lock = dir
            .try_lock_exclusive("backup-writer.lock")
            .map_err(directory_error)?;
        let role = DeviceRole {
            store: self.store,
            env: self.environment,
            local: enrolled,
        };
        let (_guard, head) = role
            .guard_writer(self.target, &dir)
            .map_err(super::super::role_error)?;
        let (_snapshot, current_purges) =
            retention_state(self.writer_profile, enrolled).map_err(|_| denied())?;
        if head != review.head || current_purges != review.purges {
            return Err(denied());
        }
        let _replacement = checked_generation(&dir, review.replacement, review.replacement_digest)?;
        let old = checked_generation(&dir, review.generation, review.digest)?;
        if Instant::now() >= review.expires {
            return Err(denied());
        }
        dir.remove_backup_generation(review.generation, old)
            .map_err(directory_error)?;
        Ok(true)
    }
}
