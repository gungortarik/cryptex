//! Prepare a replacement only in a blank profile. Publish its archive and
//! pending identity together. Only a verified grant can activate that identity.
use super::*;
use crate::backup_inventory::verify_reconstruction;
use crate::knowledge_materialization::recovered_search::{Binding, Request};
use crate::secure_store::{AccountLabel, SecureStoreError, ServiceLabel};
use cryptex_core::{RecoveryKitPublicMetadata, RecoveryPurpose};

pub(super) fn check_blank(root: &Path, recovering: bool) -> Result<(), Error> {
    for entry in std::fs::read_dir(root).map_err(|_| Error::SourceIdentity)? {
        let name = entry.map_err(|_| Error::SourceIdentity)?.file_name();
        if name == "lifecycle.status" {
            // The ordinary service records its own PID here on startup.
            let status = restore::read_selected(&root.join(&name), 16 * 1024)?;
            if std::str::from_utf8(&status)
                .ok()
                .and_then(|text| text.lines().find_map(|line| line.strip_prefix("pid=")))
                .and_then(|pid| pid.trim().parse::<u32>().ok())
                == Some(std::process::id())
            {
                continue;
            }
        }
        if name != "daemon.lock" && !(recovering && name == RECOVERY) {
            return Err(Error::SourceIdentity);
        }
    }
    Ok(())
}

pub(crate) fn prepare(
    root: &Path,
    store: &dyn SecureStore,
    env: SecureStoreEnvironment,
    codec: &Path,
    files: &crate::ipc::backup_verify::Selection,
) -> Result<(), Error> {
    let live = Directory::open(root).map_err(|_| Error::SourceIdentity)?;
    // Restored history may contain v2 attachment records. An older daemon
    // must be fenced out even after the replacement is shut down.
    let _protected = crate::import_staging::open_existing_profile_lock(
        &crate::paths::lock_path(root),
        "reader-m9.pid",
    )
    .map_err(|_| Error::SourceIdentity)?;
    let parent = root.parent().ok_or(Error::SourceIdentity)?;
    let parent_dir = Directory::open(parent).map_err(|_| Error::SourceIdentity)?;
    let staging = restore::private_scratch(Some(parent))?;
    let verified = restore::first_archive(codec, files, staging.path())?;
    let writer = verified.selection.writer()?;
    let expected = verified.selection.checkpoint(&writer)?;
    let kit_id = format!("{:x}", Sha256::digest(verified.pin));
    let service = ServiceLabel::device_credentials(env);
    if !live.exists(RECOVERY).map_err(|_| Error::SourceIdentity)? {
        // The running daemon owns its instance lock. Every other existing
        // entry belongs to an existing profile and must be preserved.
        check_blank(root, false)?;
        let stage = Directory::open(staging.path()).map_err(|_| Error::Archive)?;
        let leaf = verified
            .profile
            .path()
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or(Error::Archive)?;
        stage
            .publish_child(leaf, &stage, "archive")
            .map_err(|_| Error::Archive)?;
        let archive = staging.path().join("archive");
        let brain = restore::read_selected(
            &crate::identity_runtime::brain_identity_path(&archive),
            4096,
        )?;
        let brain = std::str::from_utf8(&brain).map_err(|_| Error::SourceIdentity)?;
        crate::local_device_identity::join_existing_brain(
            staging.path(),
            store,
            env,
            brain,
            expected.brain_id,
        )
        .map_err(|_| Error::Credentials)?;
        let local = LocalDeviceKeyResolver::load(staging.path(), expected.brain_id, store, env)
            .map_err(|_| Error::Credentials)?
            .local_trust_anchor()
            .ok_or(Error::Credentials)?;
        if local.device_id() == writer.device_id() || local.public_key() == writer.public_key() {
            return Err(Error::Credentials);
        }
        let mut challenge = [0; 32];
        getrandom::getrandom(&mut challenge).map_err(|_| Error::Credentials)?;
        let binding = Binding {
            request: Request {
                operation: "cryptex.m11.trial.authorize-first-device.v2".into(),
                kit_id: kit_id.clone(),
                brain_id: expected.brain_id.into(),
                challenge,
                backup_sha256: verified.hash,
                new_device: local.device_id().clone(),
                new_key: *local.public_key(),
                archive_device: writer.device_id().clone(),
                archive_key: *writer.public_key(),
                archive_history_sha256: expected.history_sha256,
                brain_identity_sha256: Some(expected.brain_identity_sha256),
            },
            kit: RecoveryKitPublicMetadata::new(
                kit_id.clone(),
                expected.brain_id,
                vec![
                    RecoveryPurpose::TrustBootstrap,
                    RecoveryPurpose::BackupDecrypt,
                ],
            )
            .map_err(|_| Error::Credentials)?,
            pinned: verified.pin,
        };
        let account = AccountLabel::recovery_search(local.device_id().as_str());
        if !matches!(
            store.get(service.as_str(), account.as_str()),
            Err(SecureStoreError::Missing)
        ) {
            return Err(Error::Credentials);
        }
        let (mut db, _) =
            crate::projection::open_projection_db(&archive).map_err(|_| Error::RecoveryHistory)?;
        crate::knowledge_materialization::rebuild_knowledge_from_local_approval_history(
            &archive,
            expected.brain_id,
            &writer,
            &mut db,
        )
        .map_err(|_| Error::RecoveryHistory)?;
        drop(db);
        store
            .put(
                service.as_str(),
                account.as_str(),
                &serde_json::to_vec(&binding).map_err(|_| Error::Credentials)?,
            )
            .map_err(|_| Error::Credentials)?;
        drop(stage);
        let leaf = staging
            .path()
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or(Error::Archive)?;
        parent_dir
            .publish_child(leaf, &live, RECOVERY)
            .map_err(|_| Error::Archive)?;
    }
    // Reopening while waiting for the owner's signature must keep the same
    // pending key. An active key is accepted only for an already signed grant.
    let recovery = live
        .child(RECOVERY, false)
        .map_err(|_| Error::RecoveryHistory)?;
    let pending = recovery
        .exists("identity")
        .map_err(|_| Error::RecoveryHistory)?;
    let recovery_path = root.join(RECOVERY);
    let local_root = if pending {
        recovery_path.as_path()
    } else {
        root
    };
    let local = LocalDeviceKeyResolver::load(local_root, expected.brain_id, store, env)
        .map_err(|_| Error::Credentials)?
        .local_trust_anchor()
        .ok_or(Error::Credentials)?;
    let ceremony =
        recovered_search::for_profile(root, expected.brain_id, local.device_id(), store, env)
            .map_err(|_| Error::RecoveryHistory)?
            .ok_or(Error::RecoveryHistory)?;
    let request = &ceremony.expected;
    if ceremony.trusted_root.to_bytes() != verified.pin
        || request.kit_id != kit_id
        || request.operation != "cryptex.m11.trial.authorize-first-device.v2"
        || request.backup_sha256 != verified.hash
        || request.new_key != *local.public_key()
        || request.archive_device != *writer.device_id()
        || request.archive_key != *writer.public_key()
        || request.archive_history_sha256 != expected.history_sha256
        || request.brain_identity_sha256 != Some(expected.brain_identity_sha256)
    {
        return Err(Error::RecoveryHistory);
    }
    verify_reconstruction(&recovery_path.join("archive"), &expected)?;
    if pending {
        check_blank(root, true)?;
    } else {
        ceremony.reopen().map_err(|_| Error::RecoveryHistory)?;
    }
    bound(root, store, env)?;
    Ok(())
}
