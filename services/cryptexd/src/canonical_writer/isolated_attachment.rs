//! Compiled only into the test daemon. The caller owns an isolated M9 instance.
use super::*;
use crate::ipc::attachment::{AttachError, SelectedFile};
use crate::local_device_identity::LocalDeviceKeyResolver;
use cryptex_events::{TrustedDaemonKeyResolver, attachment_link::LinkPayload};

pub(crate) fn append(
    root: &Path,
    store: &dyn SecureStore,
    env: SecureStoreEnvironment,
    file: &SelectedFile,
) -> Result<(), AttachError> {
    use AttachError::{Storage, TargetUnavailable, VerificationFailed};
    let _protected = crate::import_staging::open_existing_profile_lock(
        &root.join("daemon.lock"),
        "reader-m9.pid",
    )
    .map_err(|_| AttachError::Disabled)?;
    let brain =
        crate::identity_runtime::load_brain_identity(root).map_err(|_| VerificationFailed)?;
    let resolver = LocalDeviceKeyResolver::load(root, &brain.brain_id, store, env)
        .map_err(|_| VerificationFailed)?;
    let anchor = resolver.local_trust_anchor().ok_or(VerificationFailed)?;
    let snapshot =
        crate::knowledge_materialization::scan_verified_canonical_from_local_anchor_until(
            root,
            &brain.brain_id,
            &anchor,
            Some(std::time::Instant::now() + std::time::Duration::from_secs(20)),
        )
        .map_err(|_| VerificationFailed)?;
    let (history, trust) = snapshot
        .verified_trust_context()
        .ok_or(VerificationFailed)?;
    let device = &history.local_device;
    if !snapshot.locks_runtime(root)
        || !snapshot.locks_devices(std::slice::from_ref(device))
        || history.restricted(trust, device)
        || trust.effective_cutoffs.get(device) != Some(&u64::MAX)
    {
        return Err(VerificationFailed);
    }
    // The guarded content view already applies the target author's cutoff.
    let note = snapshot
        .verified_records()
        .iter()
        .find_map(|record| match record {
            VersionedRecord::VerifiedV1(note)
                if note.event_id == file.note_id
                    && note.content_hash.as_str() == file.note_hash =>
            {
                Some(note)
            }
            _ => None,
        })
        .ok_or(TargetUnavailable)?;
    let key = crate::event_signing_key::load_existing_event_signing_key(store, env, root, device)
        .map_err(|_| VerificationFailed)?;
    if key.signing_key.verifying_key().to_bytes() != history.local_public_key {
        return Err(VerificationFailed);
    }
    let digest = crate::attachment_blobs::store(root, &file.bytes[..]).map_err(|_| Storage)?;
    if snapshot.already_links(&note.event_id, &digest) {
        return Ok(());
    }
    append_note_control(
        root,
        device,
        note,
        &key,
        cryptex_events::attachment_link::EVENT_TYPE,
        serde_json::json!(LinkPayload::for_note(note, &digest).map_err(|_| VerificationFailed)?),
    )
}

/// The caller retains the verified snapshot's inventory and device locks.
pub(crate) fn append_note_control(
    root: &Path,
    device: &DeviceId,
    note: &AcceptedEvent,
    key: &crate::event_signing_key::ReadyEventSigningKey,
    event_type: &str,
    payload: serde_json::Value,
) -> Result<(), AttachError> {
    use AttachError::{Storage, Uncertain, VerificationFailed};
    let validator = VerifyingBodyValidator {
        verifying_key: key.signing_key.verifying_key(),
        expected_device_id: device.clone(),
    };
    let state = rebuild_chain_state(root, device, &validator).map_err(|_| VerificationFailed)?;
    ensure_sequence_ready(root, device).map_err(|_| Storage)?;
    let seq = allocate_device_sequence(root, device).map_err(|_| Storage)?;
    if state.last_device_seq.is_some_and(|tip| seq <= tip) {
        return Err(VerificationFailed);
    }
    let id = new_event_id().map_err(|_| Storage)?;
    let body = crate::test_temp::sign_v2_fixture(
        serde_json::json!({
            "schema_version": 2, "event_id": id, "brain_id": note.brain_id, "device_id": device.as_str(),
            "device_seq": DeviceSeq::from_u64(seq).map_err(|_| VerificationFailed)?,
            "created_at": now_rfc3339().map_err(|_| Storage)?, "event_type": event_type,
            "payload_schema_version": 1, "classification": note.classification, "signing_key_id": key.public.signing_key_id,
            "prev_content_hash": state.last_content_hash, "payload": payload
        }),
        &key.signing_key,
    );
    validator
        .validate_frame_body(&body)
        .map_err(|_| VerificationFailed)?;
    append_frame(root, device, state.active_segment_index, &body).map_err(|_| Uncertain)?;
    if state.events_in_active + 1 >= DEFAULT_SEAL_EVENT_THRESHOLD_HINT {
        seal_active_segment(root, device, state.active_segment_index).map_err(|_| Uncertain)?;
    }
    let updated = rebuild_chain_state(root, device, &validator).map_err(|_| Uncertain)?;
    if updated.last_device_seq != Some(seq) {
        return Err(Uncertain);
    }
    persist_tip_from_state(root, device, &updated, &key.public.signing_key_id)
        .map_err(|_| Uncertain)
}
