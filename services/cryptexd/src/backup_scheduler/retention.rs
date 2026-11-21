//! Retention only recognizes numbered automatic generations with a local
//! reconstruction receipt bound to this writer, directory, and ciphertext.
use super::{directory_error, generation_name};
use crate::import_staging::export_fs::Directory;
use crate::secure_store::{
    AccountLabel, SecureStore, SecureStoreEnvironment, SecureStoreError, ServiceLabel,
};
use cryptex_events::LocalTrustAnchor;
use sha2::{Digest, Sha256};
use std::{fs, io, num::NonZeroUsize, path::Path};

pub(crate) mod removal;

pub(crate) fn generation_number(name: &str) -> Option<u64> {
    let number = name
        .strip_prefix("backup-")?
        .strip_suffix(".pgp")?
        .parse()
        .ok()?;
    (generation_name(number) == name).then_some(number)
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Receipt {
    digest: [u8; 32],
    purges: [u8; 32],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    archive: Option<[u8; 32]>,
}

pub(crate) struct VerifiedBackups<'a> {
    pub store: &'a dyn SecureStore,
    pub environment: SecureStoreEnvironment,
    pub designated: &'a LocalTrustAnchor,
    pub target: &'a Path,
    pub writer_profile: &'a Path,
}

impl VerifiedBackups<'_> {
    // None addresses the retention preference. Some(n) addresses an
    // individual reconstruction receipt in this writer and target namespace.
    fn account(&self, generation: Option<u64>) -> io::Result<AccountLabel> {
        let binding = serde_json::to_vec(&(
            self.target,
            self.designated.device_id().as_str(),
            self.designated.public_key(),
            generation,
        ))?;
        Ok(AccountLabel::verified_backup(&format!(
            "{:x}",
            Sha256::digest(binding)
        )))
    }

    /// Reconstruct from the exact private ciphertext copy that the receipt
    /// identifies. The caller supplies the daemon's sandbox reconstruction.
    pub fn verify<T>(
        &self,
        enrolled: &LocalTrustAnchor,
        publication: &super::journal::PublishedGeneration,
        reconstruct: impl FnOnce(&Path) -> io::Result<(T, std::path::PathBuf, Option<u64>)>,
    ) -> io::Result<T> {
        self.require_writer(enrolled)?;
        let number = publication.number;
        if self.designated != &publication.writer
            || self.target.join(generation_name(number)) != publication.path
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "wrong scheduled backup target",
            ));
        }
        let dir = Directory::open(self.target).map_err(directory_error)?;
        let mut input = dir
            .open_file(&generation_name(number), false)
            .map_err(directory_error)?;
        let private = tempfile::tempdir()?;
        let path = private.path().join("snapshot.pgp");
        let mut copy = fs::File::create_new(&path)?;
        io::copy(&mut input, &mut copy)?;
        drop(copy);
        drop(input);
        let digest = super::generation_digest(&path)?;
        if digest != publication.digest {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "backup differs from publication",
            ));
        }
        let (verified, restored, protected) = reconstruct(&path)?;
        let (_, purges) =
            crate::knowledge_materialization::backup::retention_state(&restored, enrolled)
                .map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "unverified purge history")
                })?;
        let record = serde_json::to_vec(&Receipt {
            digest,
            purges,
            archive: publication.archived_digest,
        })?;
        let service = ServiceLabel::device_credentials(self.environment);
        let account = self.account(Some(number))?;
        match self.store.get(service.as_str(), account.as_str()) {
            Ok(old) if old == record => {}
            // An older receipt proves only the bytes. Upgrade only after this
            // reconstruction of the exact same ciphertext.
            Ok(old) if old.as_slice() == digest => self
                .store
                .put(service.as_str(), account.as_str(), &record)
                .map_err(io::Error::other)?,
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "verified generation changed",
                ));
            }
            Err(SecureStoreError::Missing) => self
                .store
                .put(service.as_str(), account.as_str(), &record)
                .map_err(io::Error::other)?,
            Err(error) => return Err(io::Error::other(error)),
        }
        self.prune(enrolled, protected)?;
        Ok(verified)
    }

    fn require_writer(&self, enrolled: &LocalTrustAnchor) -> io::Result<()> {
        if enrolled == self.designated {
            return Ok(());
        }
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "backup writer required",
        ))
    }

    pub fn set_retention(
        &self,
        enrolled: &LocalTrustAnchor,
        keep: Option<NonZeroUsize>,
    ) -> io::Result<()> {
        self.require_writer(enrolled)?;
        let service = ServiceLabel::device_credentials(self.environment);
        self.store
            .put(
                service.as_str(),
                self.account(None)?.as_str(),
                &serde_json::to_vec(&keep)?,
            )
            .map_err(io::Error::other)
    }

    pub fn prune(&self, enrolled: &LocalTrustAnchor, protected: Option<u64>) -> io::Result<usize> {
        self.require_writer(enrolled)?;
        let service = ServiceLabel::device_credentials(self.environment);
        let keep: Option<NonZeroUsize> = match self
            .store
            .get(service.as_str(), self.account(None)?.as_str())
        {
            Ok(bytes) => serde_json::from_slice(&bytes)?,
            Err(SecureStoreError::Missing) => None,
            Err(error) => return Err(io::Error::other(error)),
        };
        let Some(keep) = keep else { return Ok(0) };
        let dir = Directory::open(self.target).map_err(directory_error)?;
        let _lock = dir
            .try_lock_exclusive("backup-writer.lock")
            .map_err(directory_error)?;
        let role = crate::knowledge_materialization::backup::role::DeviceRole {
            store: self.store,
            env: self.environment,
            local: enrolled,
        };
        let _role_guard = role
            .guard_writer(self.target, &dir)
            .map_err(super::role_error)?;
        let (_current, purges) = crate::knowledge_materialization::backup::retention_state(
            self.writer_profile,
            enrolled,
        )
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "writer purge history unavailable",
            )
        })?;
        let mut verified = Vec::new();
        let mut checked_archives = std::collections::BTreeSet::new();
        for entry in fs::read_dir(self.target)? {
            let name = entry?.file_name();
            let Some(number) = name.to_str().and_then(generation_number) else {
                continue;
            };
            let digest = match self
                .store
                .get(service.as_str(), self.account(Some(number))?.as_str())
            {
                Ok(bytes) if bytes.len() == 32 => continue,
                Ok(bytes) => {
                    let receipt: Receipt = serde_json::from_slice(&bytes).map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidData, "invalid verification receipt")
                    })?;
                    if receipt.purges != purges {
                        // Never let routine retention consume an old generation
                        // across a purge, even after another backup is verified.
                        continue;
                    }
                    if let Some(digest) = receipt.archive
                        && checked_archives.insert(digest)
                    {
                        crate::knowledge_materialization::backup::verify_archived(&dir, digest)
                            .map_err(|_| {
                                io::Error::other("original scheduled backup unavailable")
                            })?;
                    }
                    receipt.digest
                }
                Err(SecureStoreError::Missing) => continue,
                Err(error) => return Err(io::Error::other(error)),
            };
            // Validate every survivor before removing a candidate.
            let file = checked_generation(&dir, number, digest)?;
            drop(file);
            // Keep the current Kit's first-device generation in addition to the configured count.
            if Some(number) != protected {
                verified.push((number, digest));
            }
        }
        verified.sort_unstable_by_key(|(number, _)| *number);
        let count = verified.len().saturating_sub(keep.get());
        for (number, digest) in verified.into_iter().take(count) {
            let file = checked_generation(&dir, number, digest)?;
            dir.remove_backup_generation(number, file)
                .map_err(directory_error)?;
        }
        Ok(count)
    }
}

fn checked_generation(dir: &Directory, number: u64, expected: [u8; 32]) -> io::Result<fs::File> {
    let mut file = dir
        .open_file(&generation_name(number), false)
        .map_err(directory_error)?;
    let mut digest = Sha256::new();
    io::copy(&mut file, &mut digest)?;
    if <[u8; 32]>::from(digest.finalize()) != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "backup differs from restore receipt",
        ));
    }
    Ok(file)
}
