//! Retention only recognizes numbered automatic generations with a local
//! reconstruction receipt bound to this writer, directory, and ciphertext.
use crate::import_staging::export_fs::{Directory, Error as DirectoryError};
use crate::secure_store::{
    AccountLabel, SecureStore, SecureStoreEnvironment, SecureStoreError, ServiceLabel,
};
use cryptex_events::LocalTrustAnchor;
use sha2::{Digest, Sha256};
use std::{fs, io, num::NonZeroUsize, path::Path};

pub(super) fn directory_error(error: DirectoryError) -> io::Error {
    match error {
        DirectoryError::Unsafe => {
            io::Error::new(io::ErrorKind::InvalidData, "unsafe backup directory")
        }
        DirectoryError::Io(kind, _) => io::Error::new(kind, "backup filesystem operation failed"),
    }
}

pub(super) fn generation_name(number: u64) -> String {
    format!("backup-{number:020}.pgp")
}

fn generation_number(name: &str) -> Option<u64> {
    let number = name
        .strip_prefix("backup-")?
        .strip_suffix(".pgp")?
        .parse()
        .ok()?;
    (generation_name(number) == name).then_some(number)
}

pub(crate) struct VerifiedBackups<'a> {
    pub store: &'a dyn SecureStore,
    pub environment: SecureStoreEnvironment,
    pub designated: &'a LocalTrustAnchor,
    pub target: &'a Path,
}

impl VerifiedBackups<'_> {
    fn account(&self, number: u64) -> io::Result<AccountLabel> {
        let binding = serde_json::to_vec(&(
            self.target,
            self.designated.device_id().as_str(),
            self.designated.public_key(),
            number,
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
        number: u64,
        reconstruct: impl FnOnce(&Path) -> io::Result<T>,
    ) -> io::Result<T> {
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
        let verified = reconstruct(&path)?;
        let service = ServiceLabel::device_credentials(self.environment);
        let account = self.account(number)?;
        match self.store.get(service.as_str(), account.as_str()) {
            Ok(old) if old.as_slice() == digest => return Ok(verified),
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "verified generation changed",
                ));
            }
            Err(SecureStoreError::Missing) => (),
            Err(error) => return Err(io::Error::other(error)),
        }
        self.store
            .put(service.as_str(), account.as_str(), &digest)
            .map_err(io::Error::other)?;
        Ok(verified)
    }

    pub fn prune(
        &self,
        enrolled: &LocalTrustAnchor,
        keep: Option<NonZeroUsize>,
    ) -> io::Result<usize> {
        if enrolled != self.designated {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "backup writer required",
            ));
        }
        let Some(keep) = keep else { return Ok(0) };
        let dir = Directory::open(self.target).map_err(directory_error)?;
        let _lock = dir
            .try_lock_exclusive("backup-writer.lock")
            .map_err(directory_error)?;
        let service = ServiceLabel::device_credentials(self.environment);
        let mut verified = Vec::new();
        for entry in fs::read_dir(self.target)? {
            let name = entry?.file_name();
            let Some(number) = name.to_str().and_then(generation_number) else {
                continue;
            };
            let digest = match self
                .store
                .get(service.as_str(), self.account(number)?.as_str())
            {
                Ok(bytes) => <[u8; 32]>::try_from(bytes).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "invalid verification receipt")
                })?,
                Err(SecureStoreError::Missing) => continue,
                Err(error) => return Err(io::Error::other(error)),
            };
            // Validate every survivor before removing a candidate.
            let file = checked_generation(&dir, number, digest)?;
            drop(file);
            verified.push((number, digest));
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
