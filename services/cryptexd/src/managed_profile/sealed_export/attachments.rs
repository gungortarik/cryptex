//! Reconcile only digests admitted by both the note and link cutoffs. The
//! transport can supply bytes, but cannot select a canonical name or note.
use super::{Directory, Error};
use crate::attachment_blobs::{self, BlobError};
use crate::knowledge_materialization::VerifiedCanonicalSnapshot;
use crate::managed_profile::exchange_policy::ExchangePeers;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::io::{Read, Seek, Write};

fn verified_bytes(source: impl Read, expected: &[u8; 32]) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    let limit = crate::ipc::attachment::MAX_BYTES;
    source.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit || <[u8; 32]>::from(Sha256::digest(&bytes)) != *expected {
        return Err(Error::DestinationConflict);
    }
    Ok(bytes)
}

fn blob_error(error: BlobError) -> Error {
    match error {
        BlobError::IntegrityMismatch | BlobError::Missing => Error::SourceChanged,
        BlobError::UnsafeFilesystem => Error::UnsafeFilesystem,
        BlobError::Io(kind) => Error::Io(kind, None),
    }
}

pub(super) fn reconcile(
    snapshot: &VerifiedCanonicalSnapshot,
    peers: &ExchangePeers<'_>,
    incoming: &Directory,
    transport: &Directory,
    after: &mut Option<[u8; 32]>,
    check: &impl Fn() -> Result<(), Error>,
) -> Result<(), Error> {
    let digests: BTreeSet<_> = snapshot.admitted_blob_digests().collect();
    let mut pending = digests
        .iter()
        .filter(|d| after.is_none_or(|last| **d > last));
    let batch: Vec<_> = pending
        .by_ref()
        .take(crate::import_limits::MAX_IMPORT_FILES_PER_BATCH)
        .collect();
    let more = pending.next().is_some();
    if !batch.is_empty() {
        check()?;
        let exchange = incoming.child(crate::import_staging::ATTACHMENT_FOLDER, true)?;
        for digest in batch {
            check()?;
            let name = format!("{:x}", sha2::digest::Output::<Sha256>::from(*digest));
            match attachment_blobs::open_verified(peers.runtime_root(), digest) {
                Ok(source) => {
                    let own = exchange.child(peers.local_transport(), true)?;
                    if own.exists(&name)? {
                        verified_bytes(own.open_file(&name, false)?, digest)?;
                    } else {
                        let bytes = verified_bytes(source, digest)?;
                        let stage = transport.child("m9-blob-export", true)?;
                        if stage.exists("segment.tmp")? {
                            stage.discard_scratch(stage.open_file("segment.tmp", false)?)?;
                        }
                        let mut temp = stage.open_file("segment.tmp", true)?;
                        temp.write_all(&bytes)?;
                        temp.rewind()?;
                        verified_bytes(&temp, digest)?;
                        check()?;
                        match stage.promote("segment.tmp", &temp, &own, &name) {
                            Err(crate::import_staging::export_fs::Error::Io(
                                std::io::ErrorKind::AlreadyExists,
                                _,
                            )) => {
                                stage.discard_scratch(temp)?;
                                verified_bytes(own.open_file(&name, false)?, digest)?;
                            }
                            result => result?,
                        }
                    }
                }
                Err(error @ (BlobError::Missing | BlobError::IntegrityMismatch)) => {
                    let mut restored = false;
                    for peer in peers.bindings() {
                        check()?;
                        if !exchange.exists(&peer.transport_device_id)? {
                            continue;
                        }
                        let sender = exchange.child(&peer.transport_device_id, false)?;
                        if !sender.exists(&name)? {
                            continue;
                        }
                        let bytes = match verified_bytes(sender.open_file(&name, false)?, digest) {
                            Err(Error::DestinationConflict) => {
                                eprintln!("component=attachment_sync status=peer_digest_rejected");
                                continue;
                            }
                            result => result?,
                        };
                        check()?;
                        attachment_blobs::restore(peers.runtime_root(), *digest, &bytes)
                            .map_err(blob_error)?;
                        restored = true;
                        break;
                    }
                    if !restored && error == BlobError::IntegrityMismatch {
                        eprintln!("component=attachment_sync status=damaged_awaiting_peer");
                    }
                }
                Err(err) => return Err(blob_error(err)),
            }
            *after = Some(*digest);
        }
    }
    if !more {
        *after = None;
    }
    Ok(())
}
