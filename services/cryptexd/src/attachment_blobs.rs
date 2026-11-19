//! Daemon storage for immutable attachment bytes. A stored blob does not by
//! itself create a reference, authorize retrieval, or make content visible.
use crate::import_staging::export_fs::{self, Directory};
use sha2::{Digest, Sha256};
use std::io::{self, Read, Seek, Write};
use std::path::Path;

#[derive(Debug, PartialEq, Eq)]
pub enum BlobError {
    UnsafeFilesystem,
    Io(io::ErrorKind),
    IntegrityMismatch,
}

impl From<export_fs::Error> for BlobError {
    fn from(error: export_fs::Error) -> Self {
        match error {
            export_fs::Error::Unsafe => Self::UnsafeFilesystem,
            export_fs::Error::Io(kind, _) => Self::Io(kind),
        }
    }
}

impl From<io::Error> for BlobError {
    fn from(error: io::Error) -> Self {
        Self::Io(error.kind())
    }
}

/// Store one binary stream in an existing owner-controlled runtime root.
/// Returns the SHA-256 of the verified bytes. A separate attachment writer
/// lock permits event reads and writes while long attachments are streaming.
pub fn store(runtime_root: &Path, mut source: impl Read) -> Result<[u8; 32], BlobError> {
    crate::paths::assert_outside_repository(runtime_root)
        .map_err(|_| BlobError::UnsafeFilesystem)?;
    let root = Directory::open(runtime_root)?;
    let staging = root.child("blob-staging", true)?;
    let _writer = staging.lock_exclusive("writer.lock")?;
    let canonical = root.child("canonical", true)?;
    root.sync()?;
    let blobs = canonical.child("blobs", true)?;
    canonical.sync()?;
    if staging.exists("segment.tmp")? {
        staging.discard_scratch(staging.open_file("segment.tmp", false)?)?;
    }
    let mut staged = staging.open_file("segment.tmp", true)?;
    let digest = match copy_and_hash(&mut source, &mut staged) {
        Ok(digest) => digest,
        Err(error) => {
            staging.discard_scratch(staged)?;
            return Err(error.into());
        }
    };
    let name = format!("{digest:x}");
    if blobs.exists(&name)? {
        let mut existing = blobs.open_file(&name, false)?;
        let result = verify(&mut existing, &digest);
        staging.discard_scratch(staged)?;
        result?;
        // Also complete a previous publication whose flush reply was lost.
        export_fs::flush(&existing)?;
        blobs.sync()?;
    } else {
        staged.rewind()?;
        verify(&mut staged, &digest)?;
        staging.promote("segment.tmp", &staged, &blobs, &name)?;
    }
    Ok(digest.into())
}

fn copy_and_hash(
    source: &mut impl Read,
    dest: &mut impl Write,
) -> io::Result<sha2::digest::Output<Sha256>> {
    let mut hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = match source.read(&mut buffer) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if count == 0 {
            return Ok(hash.finalize());
        }
        dest.write_all(&buffer[..count])?;
        hash.update(&buffer[..count]);
    }
}

fn verify(
    file: &mut std::fs::File,
    expected: &sha2::digest::Output<Sha256>,
) -> Result<(), BlobError> {
    if copy_and_hash(file, &mut io::sink())? != *expected {
        return Err(BlobError::IntegrityMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
