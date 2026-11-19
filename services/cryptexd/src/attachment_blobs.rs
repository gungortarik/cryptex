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
    #[cfg(test)]
    Restore([u8; 32]),
}

/// Called only after the disposable receiver admits the signed link.
#[cfg(test)]
pub(crate) fn restore(
    runtime_root: &Path,
    expected: [u8; 32],
    bytes: &[u8],
) -> Result<[u8; 32], BlobError> {
    store_stream(runtime_root, bytes, StoreIntent::Restore(expected))
}

fn store_stream(
    runtime_root: &Path,
    mut source: impl Read,
    _intent: StoreIntent,
) -> Result<[u8; 32], BlobError> {
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
    #[cfg(test)]
    if matches!(_intent, StoreIntent::Restore(expected) if expected.as_slice() != digest.as_slice())
    {
        staging.discard_scratch(staged)?;
        return Err(BlobError::IntegrityMismatch);
    }
    if blobs.exists(&name)? {
        let mut existing = blobs.open_file(&name, false)?;
        let result = verify(&mut existing, &digest);
        #[cfg(test)]
        if result == Err(BlobError::IntegrityMismatch) && matches!(_intent, StoreIntent::Restore(_))
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
    let mut file = blobs.open_file(&name, false)?;
    verify(&mut file, &expected)?;
    blobs.validate()?;
    file.rewind()?;
    Ok(file)
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
