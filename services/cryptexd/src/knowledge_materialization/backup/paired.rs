//! Preserve the original ciphertext and separately encrypt the new stream.
//! The last file published is the independently stored checkpoint for both.
use super::*;
use crate::import_staging::export_fs::Directory;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::io::Seek;

pub(crate) struct PriorFiles<'a> {
    pub backup: &'a Path,
    pub checkpoint: &'a Path,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ParentAuthority {
    pub(super) approval: recovered_search::Approval,
    pub(super) signing_key: [u8; 32],
}

#[derive(Serialize, Deserialize)]
enum Format {
    #[serde(rename = "cryptex.recovered-backup-pair.v1")]
    SeparateFiles,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Pair {
    format: Format,
    pub(super) archived: RecoverySelection,
    pub(super) replacement: RecoverySelection,
    pub(super) replacement_sha256: [u8; 32],
    pub(super) parent: ParentAuthority,
}

pub(super) fn validate_lineage(
    request: &recovered_search::Request,
    old: &Checkpoint<'_>,
    new: &LocalTrustAnchor,
    identity: &[u8; 32],
) -> Result<(), Error> {
    if request.operation != "cryptex.m11.trial.authorize-first-device.v2"
        || request.brain_identity_sha256.as_ref() != Some(identity)
        || old.brain_identity_sha256 != *identity
        || old.brain_id != request.brain_id
        || request.archive_history_sha256 != old.history_sha256
        || &request.archive_device != old.writer.device_id()
        || &request.archive_key != old.writer.public_key()
        || &request.new_device != new.device_id()
        || &request.new_key != new.public_key()
        || old.writer.device_id() == new.device_id()
        || old.writer.public_key() == new.public_key()
    {
        return Err(Error::RecoveryHistory);
    }
    Ok(())
}

impl Pair {
    pub(super) fn validate(&self) -> Result<(), Error> {
        let old_key = self.archived.writer()?;
        let old = self.archived.checkpoint(&old_key)?;
        let new_key = self.replacement.writer()?;
        let new = self.replacement.checkpoint(&new_key)?;
        if new.brain_id != old.brain_id {
            return Err(Error::RecoveryHistory);
        }
        let approval = &self.parent.approval;
        validate_lineage(
            &approval.request,
            &old,
            &new_key,
            &new.brain_identity_sha256,
        )?;
        let bytes =
            serde_json_canonicalizer::to_vec(&approval.request).map_err(|_| Error::Checkpoint)?;
        VerifyingKey::from_bytes(&self.parent.signing_key)
            .map_err(|_| Error::Credentials)?
            .verify_strict(
                &bytes,
                &Signature::from_slice(&approval.signature).map_err(|_| Error::Signature)?,
            )
            .map_err(|_| Error::Signature)
    }
}

pub(super) fn copy_verified_cipher(path: &Path, expected: [u8; 32]) -> Result<fs::File, Error> {
    let (frozen, hash) = freeze_cipher(path)?;
    if hash != expected {
        return Err(Error::RecoveryHistory);
    }
    Ok(frozen)
}

pub(super) fn freeze_cipher(path: &Path) -> Result<(fs::File, [u8; 32]), Error> {
    let (mut source, before) = open_recovery_source(path).map_err(|_| Error::RecoveryHistory)?;
    if before.length == 0 {
        return Err(Error::RecoveryHistory);
    }
    let mut frozen = tempfile::tempfile().map_err(|_| Error::Archive)?;
    let mut measured = (&mut source).take(before.length);
    if io::copy(&mut measured, &mut frozen).map_err(|_| Error::Archive)? != before.length
        || source_snapshot(&source).map_err(|_| Error::Archive)? != before
    {
        return Err(Error::RecoveryHistory);
    }
    frozen.rewind().map_err(|_| Error::Archive)?;
    let mut hash = Sha256::new();
    io::copy(&mut frozen, &mut hash).map_err(|_| Error::Archive)?;
    frozen.rewind().map_err(|_| Error::Archive)?;
    Ok((frozen, hash.finalize().into()))
}

pub(crate) struct Destination<'a> {
    pub codec: &'a Path,
    pub recipient: &'a [u8],
    pub target: &'a Path,
    pub checkpoints: &'a Path,
}

pub(crate) fn export_recovered(
    root: &Path,
    store: &dyn SecureStore,
    env: SecureStoreEnvironment,
    prior: PriorFiles<'_>,
    destination: Destination<'_>,
) -> Result<(String, String, String), Error> {
    let Destination {
        codec,
        recipient,
        target,
        checkpoints,
    } = destination;
    independent_folders(target, checkpoints)?;
    independent_folders(
        prior.backup.parent().ok_or(Error::Checkpoint)?,
        prior.checkpoint.parent().ok_or(Error::Checkpoint)?,
    )?;
    let archived = serde_json::from_slice(&super::restore::read_selected(prior.checkpoint, 8192)?)
        .map_err(|_| Error::Checkpoint)?;
    let (prepared, parent) = prepare_selected(root, store, env, 1, Some(&archived))?;
    let parent = parent.ok_or(Error::RecoveryHistory)?;
    let old = copy_verified_cipher(prior.backup, parent.approval.request.backup_sha256)?;
    let cipher = prepared.encrypt(codec, recipient)?;
    let mut hash = Sha256::new();
    io::copy(&mut cipher.reopen().map_err(|_| Error::Archive)?, &mut hash)
        .map_err(|_| Error::Archive)?;
    let pair = Pair {
        format: Format::SeparateFiles,
        archived,
        replacement: prepared.selection,
        replacement_sha256: hash.finalize().into(),
        parent,
    };
    pair.validate()?;
    let dir = Directory::open(target).map_err(|_| Error::ExportDestination)?;
    let recovery = Directory::open(checkpoints).map_err(|_| Error::ExportDestination)?;
    let json = serde_json::to_vec(&pair).map_err(|_| Error::Checkpoint)?;
    let id = manual::export_id()?;
    let old = manual::publish_file(&dir, id, "prior.pgp", old)?;
    let new = manual::publish_file(
        &dir,
        id,
        "pgp",
        cipher.reopen().map_err(|_| Error::Archive)?,
    )?;
    let checkpoint = manual::publish_file(&recovery, id, "json", json.as_slice())?;
    Ok((old, new, checkpoint))
}
