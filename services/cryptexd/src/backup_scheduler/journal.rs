//! Device-local scheduler state. The daemon serializes changes under its profile
//! instance lock. Neither the removable target nor an imported backup supplies this record.
use super::{Outcome, PendingBackup, generation_digest};
use crate::secure_store::{
    AccountLabel, SecureStore, SecureStoreEnvironment, SecureStoreError, ServiceLabel,
};
use cryptex_events::LocalTrustAnchor;
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::mpsc::Receiver,
};

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
enum Phase {
    Queued,
    Prepared([u8; 32]),
    Published([u8; 32]),
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Job {
    version: u8,
    writer_key: [u8; 32],
    prior: PathBuf,
    prior_hash: [u8; 32],
    generation: u64,
    phase: Phase,
}

pub(crate) struct DeviceJobs<'a> {
    store: &'a dyn SecureStore,
    service: ServiceLabel,
    account: AccountLabel,
    owner: &'a LocalTrustAnchor,
}

impl<'a> DeviceJobs<'a> {
    pub fn new(
        store: &'a dyn SecureStore,
        env: SecureStoreEnvironment,
        owner: &'a LocalTrustAnchor,
    ) -> Self {
        Self {
            store,
            service: ServiceLabel::device_credentials(env),
            account: AccountLabel::backup_job(owner.device_id().as_str()),
            owner,
        }
    }

    fn save(&self, job: &Job) -> io::Result<()> {
        self.store
            .put(
                self.service.as_str(),
                self.account.as_str(),
                &serde_json::to_vec(job)?,
            )
            .map_err(io::Error::other)
    }

    fn load(&self) -> io::Result<Option<Job>> {
        let bytes = match self.store.get(self.service.as_str(), self.account.as_str()) {
            Ok(bytes) => bytes,
            Err(SecureStoreError::Missing) => return Ok(None),
            Err(error) => return Err(io::Error::other(error)),
        };
        let job: Job = serde_json::from_slice(&bytes)?;
        if job.version != 1
            || &job.writer_key != self.owner.public_key()
            || !job.prior.is_absolute()
            || job.prior.file_name().is_none()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid local backup assignment",
            ));
        }
        Ok(Some(job))
    }

    pub fn enqueue(&self, prior: &Path, prior_hash: [u8; 32], generation: u64) -> io::Result<()> {
        if !prior.is_absolute() || prior.file_name().is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid prior generation",
            ));
        }
        if let Some(job) = self.load()? {
            if job.prior == prior && job.prior_hash == prior_hash && job.generation == generation {
                return Ok(());
            }
            if job.phase != Phase::Published(prior_hash)
                || job.output() != prior
                || job.generation.checked_add(1) != Some(generation)
            {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "different backup assignment",
                ));
            }
        }
        self.save(&Job {
            version: 1,
            writer_key: *self.owner.public_key(),
            prior: prior.to_owned(),
            prior_hash,
            generation,
            phase: Phase::Queued,
        })
    }

    pub fn resume(
        &self,
        enrolled: &LocalTrustAnchor,
        stop: &Receiver<()>,
        waiting: impl FnOnce(),
        encode: impl FnOnce(&Path) -> io::Result<()>,
    ) -> io::Result<Option<Outcome>> {
        let Some(mut job) = self.load()? else {
            return Ok(None);
        };
        if enrolled != self.owner {
            return Ok(Some(Outcome::NotDesignated));
        }
        if let Phase::Published(expected) = job.phase {
            // Retention may have removed the previous generation after this
            // ciphertext was durably published. Recheck the surviving artifact.
            if generation_digest(&job.output())? == expected {
                return Ok(Some(Outcome::Published));
            }
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "published backup changed",
            ));
        }
        let prior = job.prior.clone();
        let pending = PendingBackup {
            designated_writer: self.owner,
            previous_generation: &prior,
            previous_sha256: job.prior_hash,
        };
        pending
            .run(enrolled, stop, waiting, || {
                let output = job.output();
                let target =
                    crate::import_staging::export_fs::Directory::open(output.parent().unwrap())
                        .map_err(super::retention::directory_error)?;
                let _lock = target
                    .try_lock_exclusive("backup-writer.lock")
                    .map_err(super::retention::directory_error)?;
                if generation_digest(&prior)? != job.prior_hash {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "prior backup changed",
                    ));
                }
                match generation_digest(&output) {
                    Ok(actual) if job.phase == Phase::Prepared(actual) => {
                        cryptex_storage::durability::sync_parent_directory(&output)
                            .map_err(io::Error::other)?;
                        job.phase = Phase::Published(actual);
                        return self.save(&job);
                    }
                    Ok(_) => {
                        return Err(io::Error::new(
                            io::ErrorKind::AlreadyExists,
                            "backup generation collision",
                        ));
                    }
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error),
                }
                let directory = tempfile::Builder::new()
                    .prefix(".cryptex-backup-")
                    .tempdir_in(output.parent().unwrap())?;
                let staged = directory.path().join("encrypted.pgp");
                encode(&staged)?;
                fs::OpenOptions::new()
                    .write(true)
                    .open(&staged)?
                    .sync_all()?;
                let digest = generation_digest(&staged)?;
                job.phase = Phase::Prepared(digest);
                self.save(&job)?;
                // Create the final name exclusively. tempfile uses the platform's
                // no-replace primitive. Stage and destination are on the same volume.
                tempfile::TempPath::try_from_path(staged)?
                    .persist_noclobber(&output)
                    .map_err(|e| e.error)?;
                cryptex_storage::durability::sync_parent_directory(&output)
                    .map_err(io::Error::other)?;
                job.phase = Phase::Published(digest);
                self.save(&job)
            })
            .map(Some)
    }
}

impl Job {
    fn output(&self) -> PathBuf {
        self.prior
            .with_file_name(super::retention::generation_name(self.generation))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secure_store::FakeSecureStore;
    use cryptex_core::DeviceId;
    use cryptex_events::signing_key_id_from_verifying_key;
    use ed25519_dalek::SigningKey;
    use std::sync::mpsc;

    #[test]
    fn publication_gap_requires_the_durable_ciphertext_digest() {
        let root = tempfile::tempdir().unwrap();
        let prior = root.path().join("first.pgp");
        fs::write(&prior, b"prior fixture").unwrap();
        let key = SigningKey::from_bytes(&[13; 32]).verifying_key();
        let owner = LocalTrustAnchor::new(
            DeviceId::new("7ad3ac64-7aa8-4be4-99d0-b1c53210a6da").unwrap(),
            signing_key_id_from_verifying_key(&key).unwrap(),
            key.to_bytes(),
        );
        let store = FakeSecureStore::new();
        let queue = DeviceJobs::new(&store, SecureStoreEnvironment::Dev, &owner);
        let (_keep, stop) = mpsc::channel();
        let resume = || {
            queue.resume(
                &owner,
                &stop,
                || panic!("target present"),
                |_| panic!("unexpected rewrite"),
            )
        };
        let original = generation_digest(&prior).unwrap();
        queue.enqueue(&prior, original, 2).unwrap();
        queue.enqueue(&prior, original, 2).unwrap();
        assert!(queue.enqueue(&prior, original, 3).is_err());
        let mut job = queue.load().unwrap().unwrap();
        let output = job.output();
        fs::write(&output, b"foreign generation").unwrap();
        assert_eq!(resume().unwrap_err().kind(), io::ErrorKind::AlreadyExists);
        fs::write(&output, b"published before shutdown").unwrap();
        let expected = generation_digest(&output).unwrap();
        job.phase = Phase::Prepared(expected);
        queue.save(&job).unwrap();
        assert_eq!(resume().unwrap(), Some(Outcome::Published));
        assert_eq!(
            queue.load().unwrap().unwrap().phase,
            Phase::Published(expected)
        );
        fs::remove_file(&prior).unwrap();
        assert_eq!(resume().unwrap(), Some(Outcome::Published));
        fs::write(&output, b"modified after publication").unwrap();
        assert_eq!(resume().unwrap_err().kind(), io::ErrorKind::AlreadyExists);
        fs::remove_file(&output).unwrap();
        assert_eq!(resume().unwrap_err().kind(), io::ErrorKind::NotFound);
    }
}
