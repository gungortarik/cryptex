//! The signed note/link inventory selects the digest. Transported filenames
//! and sender-supplied bytes confer no canonical authority.
use super::{Directory, Error};
use crate::attachment_blobs::{self, BlobError, read_transfer_bytes};
use crate::knowledge_materialization::VerifiedCanonicalSnapshot;
use crate::managed_profile::exchange_policy::ExchangePeers;
use std::collections::BTreeSet;
use std::io::{ErrorKind, Seek, Write};

fn blob_error(error: BlobError) -> Error {
    match error {
        BlobError::IntegrityMismatch | BlobError::Missing => Error::SourceChanged,
        BlobError::UnsafeFilesystem => Error::UnsafeFilesystem,
        BlobError::Io(ErrorKind::WouldBlock) => Error::Deferred,
        BlobError::Io(kind) => Error::Io(kind, None),
    }
}

// Only the local transport's disposable offer may be replaced. Stage the
// authenticated copy first; after a crash, a missing offer will be published again.
fn offer(
    own: &Directory,
    transport: &Directory,
    name: &str,
    digest: &[u8; 32],
    bytes: &[u8],
    check: &impl Fn() -> Result<(), Error>,
) -> Result<(), Error> {
    let damaged = if own.exists(name)? {
        let file = own.open_file(name, false)?;
        match read_transfer_bytes(&file, digest) {
            Ok(_) => return Ok(()),
            Err(BlobError::IntegrityMismatch) => Some(file),
            Err(err) => return Err(blob_error(err)),
        }
    } else {
        None
    };
    let stage = transport.child("m9-blob-export", true)?;
    if stage.exists("segment.tmp")? {
        stage.discard_scratch(stage.open_file("segment.tmp", false)?)?;
    }
    let mut temp = stage.open_file("segment.tmp", true)?;
    temp.write_all(bytes)?;
    temp.rewind()?;
    read_transfer_bytes(&temp, digest).map_err(blob_error)?;
    check()?;
    if let Some(file) = damaged {
        own.remove_blob(digest, file)?;
        eprintln!("component=attachment_sync status=damaged_offer_replaced");
    }
    match stage.promote("segment.tmp", &temp, own, name) {
        Err(crate::import_staging::export_fs::Error::Io(ErrorKind::AlreadyExists, _)) => {
            stage.discard_scratch(temp)?;
            // Another publication won; next tick verifies the winning file.
            Err(Error::Deferred)
        }
        result => result.map_err(Into::into),
    }
}

fn receive(
    peers: &ExchangePeers<'_>,
    exchange: &Directory,
    name: &str,
    digest: &[u8; 32],
    check: &impl Fn() -> Result<(), Error>,
) -> Result<bool, Error> {
    for peer in peers.bindings() {
        check()?;
        if !exchange.exists(&peer.transport_device_id)? {
            continue;
        }
        let sender = exchange.child(&peer.transport_device_id, false)?;
        if !sender.exists(name)? {
            continue;
        }
        let bytes = match read_transfer_bytes(sender.open_file(name, false)?, digest) {
            Ok(bytes) => bytes,
            Err(BlobError::IntegrityMismatch) => {
                eprintln!("component=attachment_sync status=peer_digest_rejected");
                continue;
            }
            Err(err) => return Err(blob_error(err)),
        };
        check()?;
        attachment_blobs::restore(peers.runtime_root(), *digest, &bytes).map_err(blob_error)?;
        return Ok(true);
    }
    Ok(false)
}

pub(super) fn reconcile(
    snapshot: &VerifiedCanonicalSnapshot,
    peers: &ExchangePeers<'_>,
    incoming: &Directory,
    transport: &Directory,
    after: &mut Option<[u8; 32]>,
    check: &impl Fn() -> Result<(), Error>,
) -> Result<(), Error> {
    use std::ops::Bound::{Excluded, Unbounded};
    let digests: BTreeSet<_> = snapshot.admitted_blob_digests().collect();
    let mut pending = digests
        .range((after.map_or(Unbounded, Excluded), Unbounded))
        .peekable();
    if pending.peek().is_none() {
        *after = None;
        return Ok(());
    }
    check()?;
    let exchange = incoming.child(crate::import_staging::ATTACHMENT_FOLDER, true)?;
    for digest in pending
        .by_ref()
        .take(crate::import_limits::MAX_IMPORT_FILES_PER_BATCH)
    {
        check()?;
        let name = format!("{:x}", sha2::digest::Output::<sha2::Sha256>::from(*digest));
        match attachment_blobs::local_transfer_bytes(peers.runtime_root(), digest) {
            Ok(bytes) => {
                let own = exchange.child(peers.local_transport(), true)?;
                offer(&own, transport, &name, digest, &bytes, check)?;
            }
            Err(error @ (BlobError::Missing | BlobError::IntegrityMismatch)) => {
                if !receive(peers, &exchange, &name, digest, check)?
                    && error == BlobError::IntegrityMismatch
                {
                    eprintln!("component=attachment_sync status=damaged_awaiting_peer");
                }
            }
            Err(err) => return Err(blob_error(err)),
        }
        *after = Some(*digest);
    }
    if pending.peek().is_none() {
        *after = None;
    }
    Ok(())
}
