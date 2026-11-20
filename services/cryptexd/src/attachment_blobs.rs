//! Daemon storage for immutable attachment bytes. A stored blob does not by
//! itself create a reference, authorize retrieval, or make content visible.
use crate::import_staging::export_fs::{self, Directory};
use cryptex_events::attachment_link::LinkError;
use sha2::{Digest, Sha256};
use std::io::{self, Read, Seek, Write};
use std::path::Path;

#[derive(Debug, PartialEq, Eq)]
pub enum BlobError {
    UnsafeFilesystem,
    Io(io::ErrorKind),
    Missing,
    IntegrityMismatch,
}

impl From<export_fs::Error> for BlobError {
    fn from(error: export_fs::Error) -> Self {
        match error {
            // The Mac capability adapter rejects unsafe metadata with a
            // synthetic PermissionDenied. Preserve native OS errors separately.
            export_fs::Error::Unsafe
            | export_fs::Error::Io(io::ErrorKind::PermissionDenied, None) => Self::UnsafeFilesystem,
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
pub fn store(runtime_root: &Path, source: impl Read) -> Result<[u8; 32], BlobError> {
    store_stream(runtime_root, source, StoreIntent::Capture)
}

enum StoreIntent {
    Capture,
    Restore([u8; 32]),
}

/// The trial executable admits only the two explicitly enrolled disposable roots.
pub(crate) fn transfer_enabled(_root: &Path) -> bool {
    #[cfg(test)]
    {
        crate::test_temp::attachment_trial(_root)
    }
    #[cfg(not(test))]
    {
        false
    }
}

/// Caller retains the locked snapshot authorizing the expected digest.
pub(crate) fn restore(
    runtime_root: &Path,
    expected: [u8; 32],
    bytes: &[u8],
) -> Result<[u8; 32], BlobError> {
    if !transfer_enabled(runtime_root) {
        return Err(BlobError::UnsafeFilesystem);
    }
    if bytes.len() > crate::ipc::attachment::MAX_BYTES {
        return Err(BlobError::IntegrityMismatch);
    }
    store_stream(runtime_root, bytes, StoreIntent::Restore(expected))
}

fn store_stream(
    runtime_root: &Path,
    mut source: impl Read,
    intent: StoreIntent,
) -> Result<[u8; 32], BlobError> {
    crate::paths::assert_outside_repository(runtime_root)
        .map_err(|_| BlobError::UnsafeFilesystem)?;
    let root = Directory::open(runtime_root)?;
    let staging = root.child("blob-staging", true)?;
    let _writer = match intent {
        StoreIntent::Capture => staging.lock_exclusive("writer.lock"),
        StoreIntent::Restore(_) => staging.try_lock_exclusive("writer.lock"),
    }?;
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
    if matches!(intent, StoreIntent::Restore(expected) if expected.as_slice() != digest.as_slice())
    {
        staging.discard_scratch(staged)?;
        return Err(BlobError::IntegrityMismatch);
    }
    if blobs.exists(&name)? {
        let mut existing = blobs.open_file(&name, false)?;
        let result = if existing.metadata()?.len() == staged.metadata()?.len() {
            verify(&mut existing, &digest)
        } else {
            Err(BlobError::IntegrityMismatch)
        };
        if result == Err(BlobError::IntegrityMismatch) && matches!(intent, StoreIntent::Restore(_))
        {
            // Authenticate the replacement on disk before moving the original.
            staged.rewind()?;
            verify(&mut staged, &digest)?;
            export_fs::flush(&staged)?;
            let evidence = staging.child("quarantine", true)?;
            staging.sync()?;
            let nonce = crate::import_staging::random_hex_16()
                .map_err(|_| BlobError::Io(io::ErrorKind::Other))?;
            blobs.promote(&name, &existing, &evidence, &format!("{name}-{nonce}"))?;
            #[cfg(test)]
            if INTERRUPT_AFTER_QUARANTINE.with(|pending| pending.replace(false)) {
                return Err(BlobError::Io(io::ErrorKind::Interrupted));
            }
            staging.promote("segment.tmp", &staged, &blobs, &name)?;
            return Ok(digest.into());
        }
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

#[derive(Debug, PartialEq, Eq)]
pub enum LocalLinkError {
    Reference(LinkError),
    Blob(BlobError),
    Held(crate::knowledge_materialization::RetrievalSafetyHold),
}

/// Check a reference against the supplied authenticated snapshot and rehash
/// its local bytes. Callers must separately enforce device and access policy.
#[cfg(test)]
fn verify_local_link<'a>(
    runtime_root: &Path,
    history: &'a cryptex_events::VerifiedV2History,
    link_event_id: &str,
) -> Result<cryptex_events::attachment_link::ResolvedAttachmentLink<'a>, LocalLinkError> {
    let link = cryptex_events::attachment_link::resolve(history, link_event_id)
        .map_err(LocalLinkError::Reference)?;
    verify_stored(runtime_root, &link.blob_sha256).map_err(LocalLinkError::Blob)?;
    Ok(link)
}

/// Recheck a stored blob before a daemon operation refers to its bytes.
/// A missing object is distinct from an existing object with the wrong hash.
#[cfg(test)]
fn verify_stored(runtime_root: &Path, digest: &[u8; 32]) -> Result<(), BlobError> {
    open_verified(runtime_root, digest).map(drop)
}

/// Return the same verified, rewound handle to the vault copier. The copier
/// also checks the staged file against the signed digest before publication.
pub(crate) fn open_verified(
    runtime_root: &Path,
    digest: &[u8; 32],
) -> Result<impl Read + use<>, BlobError> {
    let (blobs, mut file) = open_stored(runtime_root, digest)?;
    verify(&mut file, &(*digest).into())?;
    blobs.validate()?;
    file.rewind()?;
    Ok(file)
}

pub(crate) fn read_transfer_bytes(
    source: impl Read,
    expected: &[u8; 32],
) -> Result<Vec<u8>, BlobError> {
    let limit = crate::ipc::attachment::MAX_BYTES;
    let mut bytes = Vec::new();
    source.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit || <[u8; 32]>::from(Sha256::digest(&bytes)) != *expected {
        return Err(BlobError::IntegrityMismatch);
    }
    Ok(bytes)
}

pub(crate) fn local_transfer_bytes(root: &Path, expected: &[u8; 32]) -> Result<Vec<u8>, BlobError> {
    let (blobs, file) = open_stored(root, expected)?;
    let bytes = read_transfer_bytes(file, expected)?;
    blobs.validate()?;
    Ok(bytes)
}

/// The caller holds a verified purge snapshot and the blob writer lock.
#[cfg(test)]
pub(crate) fn remove_verified_copy(
    dir: &Directory,
    digest: &[u8; 32],
) -> Result<(), crate::ipc::purge_review::ReviewError> {
    use crate::ipc::purge_review::ReviewError;
    let hash = sha2::digest::Output::<Sha256>::from(*digest);
    let name = format!("{hash:x}");
    if dir.exists(&name)? {
        let mut file = dir.open_file(&name, false)?;
        verify(&mut file, &hash).map_err(|err| match err {
            BlobError::IntegrityMismatch => ReviewError::PackIntegrity,
            _ => ReviewError::CleanupPending,
        })?;
        dir.remove_blob(digest, file)?;
    }
    // Complete a previous unlink even if its directory flush was interrupted.
    dir.sync()?;
    Ok(())
}

fn open_stored(
    runtime_root: &Path,
    digest: &[u8; 32],
) -> Result<(Directory, std::fs::File), BlobError> {
    crate::paths::assert_outside_repository(runtime_root)
        .map_err(|_| BlobError::UnsafeFilesystem)?;
    let root = Directory::open(runtime_root)?;
    if !root.exists("canonical")? {
        return Err(BlobError::Missing);
    }
    let canonical = root.child("canonical", false)?;
    if !canonical.exists("blobs")? {
        return Err(BlobError::Missing);
    }
    let blobs = canonical.child("blobs", false)?;
    let expected = sha2::digest::Output::<Sha256>::from(*digest);
    let name = format!("{expected:x}");
    if !blobs.exists(&name)? {
        return Err(BlobError::Missing);
    }
    let file = blobs.open_file(&name, false)?;
    Ok((blobs, file))
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
thread_local! {
    pub(crate) static INTERRUPT_AFTER_QUARANTINE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
mod tests;
