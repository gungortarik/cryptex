//! Construct a private backup payload from a locked, authenticated device
//! history. The ciphertext writer receives the resulting ZIP and signature.
use super::*;
use crate::backup_inventory::{Checkpoint, Error, RecoverySelection};
use crate::import_staging::{open_recovery_source, source_snapshot};
use crate::local_device_identity::LocalDeviceKeyResolver;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use tempfile::NamedTempFile;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;
mod codec;
pub(super) mod cortex;
pub(crate) mod first_device;
pub(crate) use codec::public_recipient;
mod manual;
pub(crate) use manual::independent_folders;
#[cfg(feature = "m11-offline-verifier")]
mod kit_review;
mod paired;
mod restore;
#[cfg(feature = "m11-offline-verifier")]
pub use kit_review::review_kit_backups;
pub mod role;
pub(crate) use paired::{
    ArchivedSource, Destination, PriorFiles, bind_original, export_recovered, scheduled_pair,
    verify_archived,
};
pub(crate) use restore::inspect_first_archive;
pub(crate) use restore::private_scratch;
pub(crate) use restore::reconstruct_files;
pub(crate) use restore::reconstruct_selected;
pub(crate) use restore::verify_manual;

pub(super) fn snapshot(
    root: &Path,
    brain: &str,
    anchor: &LocalTrustAnchor,
) -> Result<VerifiedCanonicalSnapshot, Error> {
    snapshot_and_approvals(root, brain, anchor).map(|(snapshot, _)| snapshot)
}

/// Compare the authenticated purge decisions on both sides of automatic pruning.
/// Keep the snapshot locks until the caller has finished its filesystem operation.
pub(crate) fn retention_state(
    root: &Path,
    anchor: &LocalTrustAnchor,
) -> Result<(VerifiedCanonicalSnapshot, [u8; 32]), Error> {
    let brain =
        crate::identity_runtime::load_brain_identity(root).map_err(|_| Error::SourceIdentity)?;
    let (snapshot, approvals) = snapshot_and_approvals(root, &brain.brain_id, anchor)?;
    let (history, trust) = snapshot
        .verified_trust_context()
        .ok_or(Error::Credentials)?;
    if !trust.trusted_devices.contains(anchor.device_id())
        || history.restricted(trust, anchor.device_id())
    {
        return Err(Error::Credentials);
    }
    let mut hash = Sha256::new();
    for proof in approvals.values() {
        hash.update(Sha256::digest(proof));
    }
    Ok((snapshot, hash.finalize().into()))
}

type Approvals = BTreeMap<String, Vec<u8>>;

fn snapshot_and_approvals(
    root: &Path,
    brain: &str,
    anchor: &LocalTrustAnchor,
) -> Result<(VerifiedCanonicalSnapshot, Approvals), Error> {
    use crate::import_staging::export_fs::Directory;
    use crate::ipc::purge_review::receipt;
    let snapshot = scan_anchored_history(
        root,
        brain,
        anchor,
        ImportScanScope {
            candidate_devices: &[],
            recovery_exclusions: &[],
            lifecycle: NoteLifecycleRead::LocalPurgeTrial,
        },
        None,
    )
    .map_err(|_| Error::Checkpoint)?;
    // This reader admits bridges only with the corresponding independently
    // signed owner receipts. Pending approvals still paired with an original
    // note stop the whole backup, even if some other notes were already purged.
    let canonical = Directory::open(&root.join("canonical")).map_err(|_| Error::Archive)?;
    let ledger = if canonical
        .exists(receipt::FOLDER)
        .map_err(|_| Error::PurgeHistory)?
    {
        Some(
            canonical
                .child(receipt::FOLDER, false)
                .map_err(|_| Error::PurgeHistory)?,
        )
    } else {
        None
    };
    let (history, _) = snapshot.verified_trust_context().ok_or(Error::Checkpoint)?;
    let mut keys = history.key_resolver();
    let mut approvals = BTreeMap::new();
    for record in snapshot.security_history.records() {
        if let AuthenticatedSecurityRecord::PurgeBridge(bridge, _, original, _) = record {
            let key = keys
                .trusted_daemon_key(&bridge.device_id, bridge.signing_key_id.as_str())
                .ok_or(Error::PurgeHistory)?;
            let bytes = receipt::verified_bytes(
                ledger.as_ref().ok_or(Error::PurgeHistory)?,
                brain,
                &bridge.device_id,
                &bridge.event_id,
                original.as_str(),
                key.as_bytes(),
            )
            .map_err(|_| Error::PurgeHistory)?;
            if approvals
                .insert(format!("{}.json", bridge.event_id), bytes)
                .is_some()
            {
                return Err(Error::PurgeHistory);
            }
        }
    }
    if let Some(ledger) = ledger {
        ledger.validate().map_err(|_| Error::PurgeHistory)?;
        let mut observed = 0;
        let entries = fs::read_dir(root.join("canonical").join(receipt::FOLDER))
            .map_err(|_| Error::PurgeHistory)?;
        for entry in entries {
            let name = entry.map_err(|_| Error::PurgeHistory)?.file_name();
            observed += 1;
            if !name.to_str().is_some_and(|n| approvals.contains_key(n)) {
                return Err(Error::PurgeHistory);
            }
        }
        ledger.validate().map_err(|_| Error::PurgeHistory)?;
        if observed != approvals.len() {
            return Err(Error::PurgeHistory);
        }
    }
    Ok((snapshot, approvals))
}

/// Forget is reversible. Retrieval hides these links, but the verified backup
/// must retain the bytes needed when the owner later restores the note.
pub(super) fn blob_digests(
    snapshot: &VerifiedCanonicalSnapshot,
) -> Result<BTreeSet<[u8; 32]>, Error> {
    if snapshot.forgotten.is_empty() {
        return Ok(snapshot.admitted_blob_digests().collect());
    }
    let (_, trust) = snapshot.verified_trust_context().ok_or(Error::Checkpoint)?;
    let cutoffs = trust
        .effective_cutoffs
        .iter()
        .map(|(device, &seq)| (device.as_str().to_owned(), seq))
        .collect();
    let all = admitted_attachment_links(&snapshot.security_history, &cutoffs, None)
        .map_err(|_| Error::Attachment)?;
    Ok(all
        .live
        .into_values()
        .flat_map(|links| links.into_values())
        .collect())
}

/// A newer verified archive may replace an affected generation only when the
/// approved bridge replaces that exact original and every unrelated record and
/// surviving attachment remains recoverable, including forgotten references.
pub(crate) fn preserves_after_purge(
    before: &VerifiedCanonicalSnapshot,
    after: &VerifiedCanonicalSnapshot,
) -> Result<bool, Error> {
    use AuthenticatedSecurityRecord::{PurgeBridge, V1};
    if before.security_history.expected_brain_id() != after.security_history.expected_brain_id() {
        return Ok(false);
    }
    let records: BTreeMap<_, _> = after
        .security_history
        .records()
        .iter()
        .map(|r| (r.event_id(), r))
        .collect();
    let mut purged_note = None;
    for old in before.security_history.records() {
        let Some(&new) = records.get(old.event_id()) else {
            return Ok(false);
        };
        if let (V1(original), PurgeBridge(bridge, _, hash, _)) = (old, new) {
            if original.device_id != bridge.device_id
                || &original.content_hash != hash
                || purged_note.replace(old.event_id()).is_some()
            {
                return Ok(false);
            }
        } else if match (old, new) {
            (PurgeBridge(a, ..), PurgeBridge(b, ..)) => a.content_hash != b.content_hash,
            (PurgeBridge(..), _) | (_, PurgeBridge(..)) => true,
            _ => old.content_hash() != new.content_hash(),
        } {
            return Ok(false);
        }
    }
    let Some(note) = purged_note else {
        return Ok(false);
    };
    let (_, trust) = before.verified_trust_context().ok_or(Error::Checkpoint)?;
    let cutoffs = trust
        .effective_cutoffs
        .iter()
        .map(|(d, &s)| (d.as_str().to_owned(), s))
        .collect();
    let old_links = admitted_attachment_links(&before.security_history, &cutoffs, None)
        .map_err(|_| Error::Attachment)?;
    let available = blob_digests(after)?;
    Ok(old_links
        .live
        .iter()
        .filter(|(id, _)| id.as_str() != note)
        .flat_map(|(_, links)| links.values())
        .all(|digest| available.contains(digest)))
}

pub struct Prepared {
    payload: NamedTempFile,
    inventory: Vec<u8>,
    pub selection: RecoverySelection,
}

impl Prepared {
    /// Input for the public-key encryptor. The device signing credential is
    /// absent from both members.
    pub fn encryption_input(&self) -> Result<NamedTempFile, Error> {
        let mut output = NamedTempFile::new().map_err(|_| Error::Archive)?;
        let mut zip = ZipWriter::new(output.as_file_mut());
        append(
            &mut zip,
            "content.zip",
            self.payload.reopen().map_err(|_| Error::Archive)?,
        )?;
        append(&mut zip, "inventory.json", self.inventory.as_slice())?;
        zip.finish()
            .map_err(|_| Error::Archive)?
            .flush()
            .map_err(|_| Error::Archive)?;
        Ok(output)
    }
}

fn append(
    zip: &mut ZipWriter<&mut fs::File>,
    name: &str,
    mut reader: impl Read,
) -> Result<u64, Error> {
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .large_file(true);
    zip.start_file(name, options).map_err(|_| Error::Archive)?;
    io::copy(&mut reader, zip).map_err(|_| Error::Archive)
}

pub fn prepare(
    root: &Path,
    store: &dyn SecureStore,
    env: SecureStoreEnvironment,
    generation: u64,
) -> Result<Prepared, Error> {
    prepare_selected(root, store, env, generation, None).map(|(prepared, _)| prepared)
}

fn prepare_selected(
    root: &Path,
    store: &dyn SecureStore,
    env: SecureStoreEnvironment,
    generation: u64,
    prior: Option<&RecoverySelection>,
) -> Result<(Prepared, Option<paired::ParentAuthority>), Error> {
    let (identity_file, _) =
        open_recovery_source(&crate::identity_runtime::brain_identity_path(root))
            .map_err(|_| Error::SourceIdentity)?;
    let mut bytes = Vec::new();
    identity_file
        .take(4097)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Archive)?;
    if bytes.len() > 4096 {
        return Err(Error::Archive);
    }
    let identity = cryptex_core::BrainIdentity::from_json(
        std::str::from_utf8(&bytes).map_err(|_| Error::Archive)?,
    )
    .map_err(|_| Error::Archive)?;
    let anchor = LocalDeviceKeyResolver::load(root, &identity.brain_id, store, env)
        .map_err(|_| Error::Credentials)?
        .local_trust_anchor()
        .ok_or(Error::Credentials)?;
    // A replacement device can have two independently authenticated histories.
    // A single-history backup would silently omit the archived notes.
    let ceremony =
        recovered_search::for_profile(root, &identity.brain_id, anchor.device_id(), store, env)
            .map_err(|_| Error::RecoveryHistory)?;
    let historical = match (ceremony, prior) {
        (None, None) => None,
        (Some(ceremony), Some(prior)) => {
            let approval = ceremony
                .checked_receipt()
                .map_err(|_| Error::RecoveryHistory)?;
            let old_key = prior.writer()?;
            let old_checkpoint = prior.checkpoint(&old_key)?;
            paired::validate_lineage(
                &approval.request,
                &old_checkpoint,
                &anchor,
                &Sha256::digest(&bytes).into(),
            )?;
            let archive = root.join(recovered_search::RECOVERY).join("archive");
            crate::backup_inventory::verify_reconstruction(&archive, &old_checkpoint)?;
            let old = snapshot(&archive, &identity.brain_id, &old_key)?;
            if recovered_search::history_digest(&old) != old_checkpoint.history_sha256 {
                return Err(Error::RecoveryHistory);
            }
            Some((
                paired::ParentAuthority {
                    approval,
                    signing_key: ceremony.trusted_root.to_bytes(),
                },
                old,
            ))
        }
        _ => return Err(Error::RecoveryHistory),
    };
    let (snapshot, approvals) = snapshot_and_approvals(root, &identity.brain_id, &anchor)?;
    let parent = if let Some((approval, old)) = historical {
        if !recovered_search::disjoint_histories(&old, &snapshot, anchor.device_id()) {
            return Err(Error::RecoveryHistory);
        }
        Some(approval)
    } else {
        None
    };
    let (history, state) = snapshot
        .verified_trust_context()
        .ok_or(Error::Credentials)?;
    if history.local_device != *anchor.device_id()
        || !state.trusted_devices.contains(anchor.device_id())
        || history.restricted(state, anchor.device_id())
    {
        return Err(Error::Credentials);
    }
    let mut payload = NamedTempFile::new().map_err(|_| Error::Archive)?;
    let mut archive = ZipWriter::new(payload.as_file_mut());
    append(
        &mut archive,
        "identity/brain_identity.json",
        bytes.as_slice(),
    )?;
    for device in &snapshot.devices {
        let directory = canonical_events_device_dir(root, device);
        if !directory.try_exists().map_err(|_| Error::Archive)? {
            continue;
        }
        let mut packs = BTreeSet::new();
        for entry in fs::read_dir(&directory).map_err(|_| Error::Archive)? {
            let name = entry
                .map_err(|_| Error::Archive)?
                .file_name()
                .into_string()
                .map_err(|_| Error::Archive)?;
            if is_known_housekeeping_file(&name) {
                continue;
            }
            if parse_pack_index(&name, "seg").is_none() && parse_pack_index(&name, "open").is_none()
            {
                return Err(Error::Archive);
            }
            packs.insert(name);
        }
        for name in packs {
            let (mut file, before) =
                open_recovery_source(&directory.join(&name)).map_err(|_| Error::SourcePack)?;
            let member = format!("canonical/events/{}/{name}", device.as_filename_stem());
            let copied = append(&mut archive, &member, &mut file)?;
            if copied != before.length
                || source_snapshot(&file).map_err(|_| Error::Archive)? != before
            {
                return Err(Error::Archive);
            }
        }
    }
    for (name, bytes) in approvals {
        append(
            &mut archive,
            &format!("canonical/purge-approvals/{name}"),
            bytes.as_slice(),
        )?;
    }
    let evidence = cortex::verified_members(root, &identity.brain_id, &snapshot)?;
    let cortex_decision_sha256 = evidence
        .get(crate::cortex_evidence::DECISION_FILE)
        .map(|bytes| Sha256::digest(bytes).into());
    for (name, bytes) in evidence {
        append(&mut archive, &name, bytes.as_slice())?;
    }
    for digest in blob_digests(&snapshot)? {
        let hash = sha2::digest::Output::<Sha256>::from(digest);
        let file =
            crate::attachment_blobs::open_verified(root, &digest).map_err(|_| Error::Attachment)?;
        append(&mut archive, &format!("canonical/blobs/{hash:x}"), file)?;
    }
    archive
        .finish()
        .map_err(|_| Error::Archive)?
        .flush()
        .map_err(|_| Error::Archive)?;
    let checkpoint = Checkpoint {
        brain_id: &identity.brain_id,
        writer: &anchor,
        generation,
        history_sha256: recovered_search::history_digest(&snapshot),
        brain_identity_sha256: Sha256::digest(bytes).into(),
        cortex_decision_sha256,
    };
    let inventory = crate::backup_inventory::sign(root, store, env, payload.path(), &checkpoint)?;
    Ok((
        Prepared {
            payload,
            inventory,
            selection: RecoverySelection::capture(&checkpoint),
        },
        parent,
    ))
}
