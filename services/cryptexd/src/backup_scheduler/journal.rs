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
    #[serde(default)]
    role_head: Option<[u8; 32]>,
    writer_key: [u8; 32],
    prior: PathBuf,
    prior_hash: [u8; 32],
    generation: u64,
    phase: Phase,
}

/// Only the local scheduler's journal can identify an automatic publication.
pub(crate) struct PublishedGeneration {
    pub(super) writer: LocalTrustAnchor,
    pub(super) path: PathBuf,
    pub(super) number: u64,
    pub(super) digest: [u8; 32],
}

pub(crate) struct DeviceJobs<'a> {
    env: SecureStoreEnvironment,
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
            env,
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
        let role = self
            .role()
            .assigned_head(prior.parent().unwrap())
            .map_err(super::role_error)?;
        if let Some(job) = self.load()? {
            if job.role_head == Some(role) {
                if job.prior == prior
                    && job.prior_hash == prior_hash
                    && job.generation == generation
                {
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
            } else if let Some(prior_head) = job.role_head
                && !matches!(job.phase, Phase::Published(_))
            {
                match self.role().assigned_head(job.prior.parent().unwrap()) {
                    Ok(current) if current == prior_head => {
                        return Err(io::Error::new(
                            io::ErrorKind::AlreadyExists,
                            "another target has a pending backup",
                        ));
                    }
                    Ok(_)
                    | Err(crate::knowledge_materialization::backup::role::Error::Authority) => {}
                    Err(error) => return Err(super::role_error(error)),
                }
            }
        }
        self.save(&Job {
            version: 1,
            role_head: Some(role),
            writer_key: *self.owner.public_key(),
            prior: prior.to_owned(),
            prior_hash,
            generation,
            phase: Phase::Queued,
        })
    }

    fn role(&self) -> crate::knowledge_materialization::backup::role::DeviceRole<'_> {
        crate::knowledge_materialization::backup::role::DeviceRole {
            store: self.store,
            env: self.env,
            local: self.owner,
        }
    }

    pub(crate) fn published_pair(&self) -> io::Result<[PublishedGeneration; 2]> {
        let job = self
            .load()?
            .ok_or_else(|| io::Error::other("missing scheduled job"))?;
        let Phase::Published(digest) = job.phase else {
            return Err(io::Error::other("scheduled backup is pending"));
        };
        let previous_number = job
            .generation
            .checked_sub(1)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid generation"))?;
        if job.prior.file_name()
            != Some(super::retention::generation_name(previous_number).as_ref())
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "prior backup is not a scheduled generation",
            ));
        }
        Ok([
            PublishedGeneration {
                writer: self.owner.clone(),
                path: job.prior.clone(),
                number: previous_number,
                digest: job.prior_hash,
            },
            PublishedGeneration {
                writer: self.owner.clone(),
                path: job.output(),
                number: job.generation,
                digest,
            },
        ])
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
        let role = self.role();
        let prior = job.prior.clone();
        let target_path = prior.parent().unwrap();
        let assigned = role.assigned_head(target_path).map_err(super::role_error)?;
        if job.role_head != Some(assigned) {
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
                let (_role_guard, current) = role
                    .guard_writer(target_path, &target)
                    .map_err(super::role_error)?;
                if current != assigned {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "backup role has changed",
                    ));
                }
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
    use cryptex_events::TrustedDaemonKeyResolver;
    use std::sync::mpsc;

    #[test]
    fn publication_gap_requires_the_durable_ciphertext_digest() {
        let root = tempfile::tempdir().unwrap();
        use crate::canonical_writer::{
            AppendMemoryNoteArgs, CanonicalWriterConfig, append_memory_note,
        };
        use crate::local_device_identity::{LocalDeviceKeyResolver, enroll_local_device};
        let brain = "7ad3ac64-7aa8-4be4-99d0-b1c53210a6da";
        let profile = root.path().join("profile");
        let env = SecureStoreEnvironment::Dev;
        let store = FakeSecureStore::new();
        let identity = cryptex_core::BrainIdentity::new(brain, "2026-01-01T00:00:00Z").unwrap();
        crate::identity_runtime::write_brain_identity(&profile, &identity).unwrap();
        let local = enroll_local_device(&profile, &store, env).unwrap();
        append_memory_note(
            &store,
            env,
            AppendMemoryNoteArgs {
                runtime_root: &profile,
                brain_id: brain,
                device_id: &DeviceId::new(local.device_id).unwrap(),
                classification: cryptex_events::Classification::Public,
                text: "synthetic scheduled generation",
                event_id: None,
                config: &CanonicalWriterConfig::default(),
            },
        )
        .unwrap();
        let owner = LocalDeviceKeyResolver::load(&profile, brain, &store, env)
            .unwrap()
            .local_trust_anchor()
            .unwrap();
        let role = crate::knowledge_materialization::backup::role::DeviceRole {
            store: &store,
            env: SecureStoreEnvironment::Dev,
            local: &owner,
        };
        role.create(&profile, root.path(), "target").unwrap();
        let prior = root.path().join("target/first.pgp");
        fs::write(&prior, b"prior fixture").unwrap();
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
        role.create(&profile, root.path(), "another").unwrap();
        let elsewhere = root.path().join("another/first.pgp");
        assert_eq!(
            queue.enqueue(&elsewhere, original, 2).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
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
