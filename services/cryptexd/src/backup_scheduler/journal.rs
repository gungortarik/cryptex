//! Device-local scheduler state. The daemon serializes changes under its profile
//! instance lock. Neither the removable target nor an imported backup supplies this record.
use super::{Outcome, PendingBackup, generation_digest};
use crate::knowledge_materialization::backup::ArchivedSource;
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
    #[serde(default)]
    schedule: Option<Schedule>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Schedule {
    checkpoints: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    original: Option<ArchivedSource>,
    enabled: bool,
    next_due: u64,
}

pub(crate) struct Due {
    pub target: PathBuf,
    pub checkpoints: PathBuf,
    pub original: Option<ArchivedSource>,
    pub generation: u64,
}

/// Only the local scheduler's journal can identify an automatic publication.
pub(crate) struct PublishedGeneration {
    pub(crate) archived_digest: Option<[u8; 32]>,
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
            schedule: None,
        })
    }

    /// Persist the selected checkpoint directory before enabling autonomous publication.
    pub fn configure(&self, target: &Path, checkpoints: &Path) -> io::Result<()> {
        self.configure_history(target, checkpoints, None)
    }

    pub fn bound_original(
        &self,
        target: &Path,
        checkpoints: &Path,
    ) -> io::Result<Option<ArchivedSource>> {
        let head = self
            .role()
            .assigned_head(target)
            .map_err(super::role_error)?;
        Ok(self
            .load()?
            .filter(|job| job.role_head == Some(head) && job.prior.parent() == Some(target))
            .and_then(|job| job.schedule)
            .filter(|schedule| schedule.checkpoints == checkpoints)
            .and_then(|schedule| schedule.original))
    }

    pub fn configure_history(
        &self,
        target: &Path,
        checkpoints: &Path,
        original: Option<ArchivedSource>,
    ) -> io::Result<()> {
        let head = self
            .role()
            .assigned_head(target)
            .map_err(super::role_error)?;
        let mut job = match self.load()? {
            Some(job) if job.role_head == Some(head) && job.prior.parent() == Some(target) => job,
            _ => {
                let marker = target.join("backup-target.id");
                let assignment = self.role().reopen(target).map_err(super::role_error)?;
                let number = assignment
                    .reserved_through()
                    .checked_add(1)
                    .ok_or_else(|| io::Error::other("generation overflow"))?;
                self.enqueue(&marker, generation_digest(&marker)?, number)?;
                self.load()?
                    .ok_or_else(|| io::Error::other("missing scheduled job"))?
            }
        };
        if let Some(schedule) = &mut job.schedule {
            if schedule.checkpoints != checkpoints || schedule.original != original {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "different checkpoint folder or original backup",
                ));
            }
            schedule.enabled = true;
        } else {
            job.schedule = Some(Schedule {
                checkpoints: checkpoints.to_owned(),
                original,
                enabled: true,
                next_due: 0,
            });
        }
        self.save(&job)
    }

    pub fn enabled(&self) -> io::Result<bool> {
        Ok(self
            .load()?
            .and_then(|job| job.schedule)
            .is_some_and(|s| s.enabled))
    }

    pub fn pause(&self) -> io::Result<()> {
        if let Some(mut job) = self.load()?
            && let Some(schedule) = &mut job.schedule
        {
            schedule.enabled = false;
            self.save(&job)?;
        }
        Ok(())
    }

    pub fn due(&self, now: u64) -> io::Result<Option<Due>> {
        let Some(mut job) = self.load()? else {
            return Ok(None);
        };
        let Some(schedule) = job.schedule.clone().filter(|s| s.enabled) else {
            return Ok(None);
        };
        if !schedule.checkpoints.is_absolute() {
            return Err(io::Error::other("invalid checkpoint folder"));
        }
        if let Phase::Published(digest) = job.phase {
            if now < schedule.next_due {
                return Ok(None);
            }
            let prior = job.output();
            let number = job
                .generation
                .checked_add(1)
                .ok_or_else(|| io::Error::other("generation overflow"))?;
            // Keep the schedule and generation rollover in a single durable update.
            job.phase = Phase::Queued;
            job.prior = prior;
            job.prior_hash = digest;
            job.generation = number;
            self.save(&job)?;
        }
        Ok(Some(Due {
            target: job.prior.parent().unwrap().to_owned(),
            checkpoints: schedule.checkpoints,
            original: schedule.original,
            generation: job.generation,
        }))
    }

    fn publication_account(&self, path: &Path, head: [u8; 32]) -> io::Result<AccountLabel> {
        use sha2::{Digest, Sha256};
        let binding =
            serde_json::to_vec(&(self.owner.device_id(), self.owner.public_key(), path, head))?;
        Ok(AccountLabel::backup_publication(&format!(
            "{:x}",
            Sha256::digest(binding)
        )))
    }

    fn finish_publication(
        &self,
        job: &mut Job,
        digest: [u8; 32],
        head: [u8; 32],
    ) -> io::Result<()> {
        // Only a completed journal-owned generation can receive an automatic-retention receipt.
        let account = self.publication_account(&job.output(), head)?;
        let mut receipt = digest.to_vec();
        if let Some(original) = job.schedule.as_ref().and_then(|s| s.original.as_ref()) {
            receipt.extend_from_slice(&original.digest);
        }
        match self.store.get(self.service.as_str(), account.as_str()) {
            Ok(prior) if prior == receipt => (),
            Ok(_) => {
                return Err(io::Error::other(
                    "conflicting scheduled publication receipt",
                ));
            }
            Err(SecureStoreError::Missing) => self
                .store
                .put(self.service.as_str(), account.as_str(), &receipt)
                .map_err(io::Error::other)?,
            Err(error) => return Err(io::Error::other(error)),
        }
        job.record_publication(digest)?;
        self.save(job)
    }

    pub fn publication(&self, path: &Path) -> io::Result<PublishedGeneration> {
        let number = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(super::retention::generation_number)
            .ok_or_else(|| io::Error::other("select a numbered automatic generation"))?;
        let target = path
            .parent()
            .ok_or_else(|| io::Error::other("missing backup target"))?;
        let head = self
            .role()
            .assigned_head(target)
            .map_err(super::role_error)?;
        let account = self.publication_account(path, head)?;
        let receipt = self
            .store
            .get(self.service.as_str(), account.as_str())
            .map_err(io::Error::other)?;
        let (digest, archived_digest) = match receipt.as_slice() {
            digest if digest.len() == 32 => (digest.try_into().unwrap(), None),
            both if both.len() == 64 => (
                both[..32].try_into().unwrap(),
                Some(both[32..].try_into().unwrap()),
            ),
            _ => return Err(io::Error::other("invalid publication receipt")),
        };
        Ok(PublishedGeneration {
            archived_digest,
            writer: self.owner.clone(),
            path: path.to_owned(),
            number,
            digest,
        })
    }

    fn role(&self) -> crate::knowledge_materialization::backup::role::DeviceRole<'_> {
        crate::knowledge_materialization::backup::role::DeviceRole {
            store: self.store,
            env: self.env,
            local: self.owner,
        }
    }

    #[cfg(test)]
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
        if job.prior.file_name() != Some(super::generation_name(previous_number).as_ref()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "prior backup is not a scheduled generation",
            ));
        }
        Ok([
            PublishedGeneration {
                archived_digest: job
                    .schedule
                    .as_ref()
                    .and_then(|s| s.original.as_ref())
                    .map(|a| a.digest),
                writer: self.owner.clone(),
                path: job.prior.clone(),
                number: previous_number,
                digest: job.prior_hash,
            },
            PublishedGeneration {
                archived_digest: job
                    .schedule
                    .as_ref()
                    .and_then(|s| s.original.as_ref())
                    .map(|a| a.digest),
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
                        .map_err(super::directory_error)?;
                let _lock = target
                    .try_lock_exclusive("backup-writer.lock")
                    .map_err(super::directory_error)?;
                let (_role_guard, current) = role
                    .guard_writer(target_path, &target)
                    .map_err(super::role_error)?;
                if current != assigned {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "backup role has changed",
                    ));
                }
                if let Some(original) = job.schedule.as_ref().and_then(|s| s.original.as_ref()) {
                    crate::knowledge_materialization::backup::verify_archived(
                        &target,
                        original.digest,
                    )
                    .map_err(|_| io::Error::other("original scheduled ciphertext unavailable"))?;
                }
                if generation_digest(&prior)? != job.prior_hash {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "prior backup changed",
                    ));
                }
                role.reserve_generation(&target, job.generation)
                    .map_err(super::role_error)?;
                match generation_digest(&output) {
                    Ok(actual) if job.phase == Phase::Prepared(actual) => {
                        cryptex_storage::durability::sync_parent_directory(&output)
                            .map_err(io::Error::other)?;
                        return self.finish_publication(&mut job, actual, assigned);
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
                self.finish_publication(&mut job, digest, assigned)
            })
            .map(Some)
    }
}

impl Job {
    fn record_publication(&mut self, digest: [u8; 32]) -> io::Result<()> {
        if let Some(schedule) = &mut self.schedule {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(io::Error::other)?;
            schedule.next_due = now.as_secs().saturating_add(24 * 60 * 60);
        }
        self.phase = Phase::Published(digest);
        Ok(())
    }

    fn output(&self) -> PathBuf {
        self.prior
            .with_file_name(super::generation_name(self.generation))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secure_store::FakeSecureStore;
    use cryptex_core::DeviceId;
    use cryptex_events::TrustedDaemonKeyResolver;
    use std::sync::mpsc;

    fn fixture() -> (
        tempfile::TempDir,
        PathBuf,
        FakeSecureStore,
        LocalTrustAnchor,
    ) {
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
        (root, profile, store, owner)
    }

    #[test]
    fn a_daily_schedule_survives_pause_reconnection_and_the_next_due_date() {
        let (root, profile, store, owner) = fixture();
        let env = SecureStoreEnvironment::Dev;
        let role = crate::knowledge_materialization::backup::role::DeviceRole {
            store: &store,
            env,
            local: &owner,
        };
        let (target, _) = role.create(&profile, root.path()).unwrap();
        let checkpoints = root.path().join("checkpoints");
        fs::create_dir(&checkpoints).unwrap();
        let reopen = || DeviceJobs::new(&store, env, &owner);
        let offline = root.path().join("detached");
        reopen().configure(&target, &checkpoints).unwrap();
        reopen().pause().unwrap();
        assert!(reopen().due(u64::MAX).unwrap().is_none());
        reopen().configure(&target, &checkpoints).unwrap();
        let first = reopen().due(0).unwrap().unwrap();
        assert_eq!(
            (first.target, first.checkpoints, first.generation),
            (target.clone(), checkpoints.clone(), 1)
        );
        fs::rename(&target, &offline).unwrap();
        let (tx, stop) = mpsc::channel();
        drop(tx);
        let waiting = reopen()
            .resume(&owner, &stop, || {}, |_| panic!("offline write"))
            .unwrap();
        assert_eq!(waiting, Some(Outcome::Stopped));
        fs::rename(offline, &target).unwrap();
        assert_eq!(
            reopen()
                .resume(
                    &owner,
                    &stop,
                    || panic!("disk available"),
                    |p| fs::write(p, b"synthetic cipher")
                )
                .unwrap(),
            Some(Outcome::Published)
        );
        let published = reopen()
            .publication(&target.join("backup-00000000000000000001.pgp"))
            .unwrap();
        assert_eq!(published.number, 1);
        assert_eq!(
            published.digest,
            generation_digest(&published.path).unwrap()
        );
        let foreign = target.join("backup-00000000000000000002.pgp");
        fs::copy(&published.path, &foreign).unwrap();
        assert!(reopen().publication(&foreign).is_err());
        let scheduled = reopen().load().unwrap().unwrap().schedule.unwrap();
        assert!(scheduled.next_due > 86400);
        assert!(reopen().due(scheduled.next_due - 1).unwrap().is_none());
        let next = reopen().due(scheduled.next_due).unwrap().unwrap();
        assert_eq!(next.generation, 2);
        assert_eq!(
            reopen()
                .due(scheduled.next_due + 1)
                .unwrap()
                .unwrap()
                .generation,
            2
        );
        assert_eq!(
            reopen().load().unwrap().unwrap().prior.file_name().unwrap(),
            "backup-00000000000000000001.pgp"
        );
    }

    #[test]
    fn publication_gap_requires_the_durable_ciphertext_digest() {
        let (root, profile, store, owner) = fixture();
        let role = crate::knowledge_materialization::backup::role::DeviceRole {
            store: &store,
            env: SecureStoreEnvironment::Dev,
            local: &owner,
        };
        let (first_target, _) = role.create(&profile, root.path()).unwrap();
        let prior = first_target.join("first.pgp");
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
        let (second_target, _) = role.create(&profile, root.path()).unwrap();
        let elsewhere = second_target.join("first.pgp");
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
