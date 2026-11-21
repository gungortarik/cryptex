//! Exchange public files with the offline first-device signer.
use super::super::recovered_search::{self, Approval, Ceremony, Denial, RECEIPT, RECOVERY};
use super::*;
use crate::import_staging::export_fs::Directory;
use std::io::Write;
use std::path::PathBuf;

mod preparation;
pub(crate) use preparation::prepare;

fn device_profile(root: &Path) -> Result<PathBuf, Error> {
    let live = Directory::open(root).map_err(|_| Error::SourceIdentity)?;
    if live.exists("identity").map_err(|_| Error::SourceIdentity)? {
        return Ok(root.to_owned());
    }
    let pending = live
        .child(RECOVERY, false)
        .map_err(|_| Error::RecoveryHistory)?;
    pending
        .child("identity", false)
        .map_err(|_| Error::SourceIdentity)?;
    preparation::check_blank(root, true)?;
    Ok(root.join(RECOVERY))
}

pub(crate) fn bound(
    root: &Path,
    store: &dyn SecureStore,
    env: SecureStoreEnvironment,
) -> Result<Ceremony, Error> {
    let local_root = device_profile(root)?;
    let brain = crate::identity_runtime::load_brain_identity(&local_root)
        .map_err(|_| Error::SourceIdentity)?;
    let local = LocalDeviceKeyResolver::load(&local_root, &brain.brain_id, store, env)
        .map_err(|_| Error::Credentials)?
        .local_trust_anchor()
        .ok_or(Error::Credentials)?;
    let ceremony =
        recovered_search::for_profile(root, &brain.brain_id, local.device_id(), store, env)
            .map_err(|_| Error::RecoveryHistory)?
            .ok_or(Error::RecoveryHistory)?;
    let req = &ceremony.expected;
    if req.operation != "cryptex.m11.trial.authorize-first-device.v2"
        || req.kit_id != format!("{:x}", Sha256::digest(ceremony.trusted_root.as_bytes()))
        || &req.new_key != local.public_key()
        || req.archive_key == req.new_key
        || req.new_device == req.archive_device
    {
        return Err(Error::Credentials);
    }
    Ok(ceremony)
}

impl Ceremony {
    /// Publishing requires the local credential's exact challenge and pinned Kit key.
    /// An identical retry verifies the existing on-disk receipt.
    pub(in super::super) fn admit(&self, approval: &Approval) -> Result<(), Denial> {
        self.verify(approval)?;
        let dir = Directory::open(self.receipt.parent().ok_or(Denial::Receipt)?)
            .map_err(|_| Denial::Receipt)?;
        let _lock = dir
            .lock_exclusive("first-device.lock")
            .map_err(|_| Denial::Receipt)?;
        if dir.exists(RECEIPT).map_err(|_| Denial::Receipt)? {
            return self.reopen().map(|_| ()).map_err(|e| {
                if e == Denial::Context {
                    Denial::AlreadyAuthorized
                } else {
                    e
                }
            });
        }
        let id = manual::export_id().map_err(|_| Denial::Receipt)?;
        let partial = format!("manual-{id:032x}.partial");
        let mut stage = dir.open_file(&partial, true).map_err(|_| Denial::Receipt)?;
        if serde_json::to_writer(&mut stage, approval).is_err() || stage.flush().is_err() {
            dir.discard_partial_manual_export(id, stage)
                .map_err(|_| Denial::Receipt)?;
            return Err(Denial::Receipt);
        }
        dir.promote(&partial, &stage, &dir, RECEIPT)
            .map_err(|_| Denial::Receipt)
    }

    pub(crate) fn export_challenge(
        &self,
        dir: &Path,
    ) -> Result<crate::ipc::first_device::PublicChallenge, Error> {
        let output = Directory::open(dir).map_err(|_| Error::ExportDestination)?;
        let request =
            serde_json_canonicalizer::to_vec(&self.expected).map_err(|_| Error::Checkpoint)?;
        let file = manual::publish_file(
            &output,
            manual::export_id()?,
            "challenge.json",
            request.as_slice(),
        )?;
        Ok(crate::ipc::first_device::PublicChallenge {
            file,
            device: self.expected.new_device.as_str().into(),
            fingerprint: format!("{:x}", Sha256::digest(self.expected.new_key)),
            brain: self.expected.brain_id.clone(),
            kit: self.expected.kit_id.clone(),
            backup_sha256: format!(
                "{:x}",
                sha2::digest::Output::<Sha256>::from(self.expected.backup_sha256)
            ),
        })
    }

    pub(crate) fn accept_owner_file(&self, root: &Path, path: &Path) -> Result<(), Error> {
        let approval = serde_json::from_slice::<Approval>(&restore::read_selected(path, 16384)?)
            .map_err(|_| Error::Signature)?;
        self.verify(&approval).map_err(|_| Error::Signature)?;
        let archived = Self::archive_reader(&approval)
            .map_err(|_| Error::Credentials)?
            .local_trust_anchor()
            .ok_or(Error::Credentials)?;
        let expected = &self.expected;
        let identity = expected.brain_identity_sha256.ok_or(Error::Checkpoint)?;
        let root_archive = root.join(RECOVERY).join("archive");
        let local_root = device_profile(root)?;
        for profile in [local_root.as_path(), root_archive.as_path()] {
            let bytes = restore::read_selected(
                &crate::identity_runtime::brain_identity_path(profile),
                4096,
            )?;
            if <[u8; 32]>::from(Sha256::digest(bytes)) != identity {
                return Err(Error::Checkpoint);
            }
        }
        let snapshot = super::snapshot(&root_archive, &expected.brain_id, &archived)?;
        if recovered_search::history_digest(&snapshot) != expected.archive_history_sha256 {
            return Err(Error::Checkpoint);
        }
        for digest in super::blob_digests(&snapshot)? {
            crate::attachment_blobs::open_verified(&root_archive, &digest)
                .map_err(|_| Error::Attachment)?;
        }
        self.admit(&approval).map_err(|_| Error::RecoveryHistory)?;
        if local_root != root {
            preparation::check_blank(root, true)?;
            let live = Directory::open(root).map_err(|_| Error::SourceIdentity)?;
            let pending = live
                .child(RECOVERY, false)
                .map_err(|_| Error::RecoveryHistory)?;
            pending
                .publish_child("identity", &live, "identity")
                .map_err(|_| Error::SourceIdentity)?;
        }
        Ok(())
    }
}
