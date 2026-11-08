//! Pilot-only reuse of existing locally approved peers; never creates approval.
use std::fs;
use std::path::Path;

use cryptex_core::DeviceId;
use cryptex_events::{Classification, EventPayload, TrustedDaemonKeyResolver, VersionedRecord};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::capture::{CaptureOutcome, CaptureRequest, capture_memory_note};
use crate::local_device_identity::{LocalDeviceKeyResolver, load_local_device_identity};
use crate::secure_store::{SecureStore, SecureStoreEnvironment};
use crate::signed_transfer_probe::ProbeError;

pub const NOTE: &str = "Synthetic trusted import probe. No personal data.";
const TIMELINE_NOTE: &str =
    "Synthetic M5 timeline fixture. Safe to discard with this isolated pilot runtime.";
const ENV: SecureStoreEnvironment = SecureStoreEnvironment::Dev;

#[derive(Debug, Clone, Serialize)]
pub struct ProbeSegment {
    pub relative_path: String,
    pub bytes: u64,
    pub sha256: String,
}
#[derive(Debug, Serialize)]
pub struct PreparedTrustedProbe {
    pub schema: u32,
    pub scope: &'static str,
    pub probe_id: String,
    pub brain_id: String,
    pub source_device: String,
    pub receiver_device: String,
    pub reciprocal_approval_verified: bool,
    pub segments: Vec<ProbeSegment>,
    pub unapproved_segment: ProbeSegment,
    pub unapproved_device: String,
    pub unapproved_descriptor_sha256: String,
    pub published: bool,
}

fn context<S: SecureStore>(
    root: &Path,
    store: &S,
) -> Result<(String, DeviceId, LocalDeviceKeyResolver), ProbeError> {
    let brain =
        crate::identity_runtime::load_brain_identity(root).map_err(|_| ProbeError::Setup)?;
    let local =
        load_local_device_identity(root, &brain.brain_id).map_err(|_| ProbeError::Enrollment)?;
    let device = DeviceId::new(local.device_id).map_err(|_| ProbeError::Binding)?;
    let resolver = LocalDeviceKeyResolver::load(root, &brain.brain_id, store, ENV)
        .map_err(|_| ProbeError::Key)?;
    Ok((brain.brain_id, device, resolver))
}

fn check_approval<S: SecureStore>(
    root: &Path,
    peer_root: &Path,
    store: &S,
    synthetic_only: bool,
) -> Result<(), ProbeError> {
    let (brain, local_device, resolver) = context(root, store)?;
    let (peer_brain, peer_device, peer_resolver) = context(peer_root, store)?;
    if brain != peer_brain {
        return Err(ProbeError::Binding);
    }
    let local_anchor = resolver.local_trust_anchor().ok_or(ProbeError::Key)?;
    let peer_anchor = peer_resolver.local_trust_anchor().ok_or(ProbeError::Key)?;
    let snapshot =
        crate::knowledge_materialization::scan_verified_canonical_from_local_anchor_until(
            root,
            &brain,
            &local_anchor,
            None,
        )
        .map_err(|_| ProbeError::Integrity)?;
    let (history, trust) = snapshot
        .verified_trust_context()
        .ok_or(ProbeError::Binding)?;
    let binding = history
        .approved_peer_binding(&peer_device)
        .ok_or(ProbeError::NotApproved)?;
    if !trust.trusted_devices.contains(&peer_device)
        || trust.effective_cutoffs.get(&peer_device) != Some(&u64::MAX)
        || trust.effective_cutoffs.get(&local_device) != Some(&u64::MAX)
        || binding.signing_key_id != *peer_anchor.signing_key_id()
        || binding.public_key != *peer_anchor.public_key()
    {
        return Err(ProbeError::NotApproved);
    }
    if synthetic_only {
        for record in snapshot.verified_records() {
            match record {
                VersionedRecord::VerifiedV1(event)
                    if event.classification == Classification::Public
                        && matches!(&event.payload, EventPayload::MemoryNote(note) if note.text == NOTE || note.text == TIMELINE_NOTE) =>
                    {}
                VersionedRecord::VerifiedSecurityV3(event)
                    if matches!(
                        event.payload(),
                        cryptex_events::SecurityPayloadV3::PeerApproved { .. }
                    ) => {}
                _ => return Err(ProbeError::NonSynthetic),
            }
        }
    }
    Ok(())
}

fn describe_file(root: &Path, device: &DeviceId, name: &str) -> Result<ProbeSegment, ProbeError> {
    let relative = format!("{}/{}", device.as_filename_stem(), name);
    let path = root.join("canonical/events").join(&relative);
    let metadata = fs::symlink_metadata(&path).map_err(|_| ProbeError::Io)?;
    if !metadata.is_file() || metadata.len() > 8 * 1024 * 1024 {
        return Err(ProbeError::Limit);
    }
    let bytes = fs::read(path).map_err(|_| ProbeError::Io)?;
    Ok(ProbeSegment {
        relative_path: relative,
        bytes: bytes.len() as u64,
        sha256: format!("{:X}", Sha256::digest(&bytes)),
    })
}

/// Caller holds source, receiver and fresh-negative runtime guards after recovery.
/// Both approval bindings are reconstructed from existing signed local history.
pub fn prepare<S: SecureStore>(
    source: &Path,
    receiver: &Path,
    negative: &Path,
    probe_id: &str,
    store: &S,
) -> Result<PreparedTrustedProbe, ProbeError> {
    cryptex_core::validate::parse_uuid_str(probe_id).map_err(|_| ProbeError::Binding)?;
    check_approval(source, receiver, store, true)?;
    check_approval(receiver, source, store, false)?;
    let (brain, source_device, _) = context(source, store)?;
    let (_, receiver_device, receiver_resolver) = context(receiver, store)?;
    match capture_memory_note(
        store,
        ENV,
        source,
        CaptureRequest {
            capture_id: probe_id.into(),
            text: NOTE.into(),
            classification: Classification::Public,
        },
        &crate::canonical_writer::CanonicalWriterConfig {
            seal_event_threshold: 1,
        },
    ) {
        CaptureOutcome::Committed(_) => {}
        _ => return Err(ProbeError::Append),
    }
    check_approval(source, receiver, store, true)?;
    // Derived state only. Never insert a peer key/roster row to make validation pass.
    // Establish the already-approved peer's empty writer slot through the normal
    // canonical lock API, then rebuild from that actual inventory. Strict M4
    // watermark equality remains unchanged; no peer event is installed here.
    {
        let _slots = crate::canonical_writer::acquire_canonical_write_lock_set(
            receiver,
            &[receiver_device.clone(), source_device.clone()],
        )
        .map_err(|_| ProbeError::Setup)?;
    }
    let mut db = cryptex_projection::ProjectionDb::open_or_create(receiver)
        .map_err(|_| ProbeError::Setup)?;
    crate::knowledge_materialization::rebuild_knowledge_from_local_approval_history(
        receiver,
        &brain,
        &receiver_resolver
            .local_trust_anchor()
            .ok_or(ProbeError::Key)?,
        &mut db,
    )
    .map_err(|_| ProbeError::Setup)?;
    drop(db);
    let mut negative_event_id = probe_id.to_owned();
    let replacement = if negative_event_id.ends_with('0') {
        "1"
    } else {
        "0"
    };
    negative_event_id.replace_range(35..36, replacement);
    crate::signed_transfer_probe::create(negative, store, &brain, &negative_event_id)?;
    let negative_identity =
        load_local_device_identity(negative, &brain).map_err(|_| ProbeError::Setup)?;
    let negative_device =
        DeviceId::new(negative_identity.device_id).map_err(|_| ProbeError::Binding)?;
    let mut segments = Vec::new();
    let directory = cryptex_storage::canonical_events_device_dir(source, &source_device);
    for entry in fs::read_dir(directory).map_err(|_| ProbeError::Io)? {
        let entry = entry.map_err(|_| ProbeError::Io)?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| ProbeError::Io)?;
        if name.ends_with(".seg") {
            if name.len() != 24 || !name[..20].bytes().all(|b| b.is_ascii_digit()) {
                return Err(ProbeError::Binding);
            }
            segments.push(describe_file(source, &source_device, &name)?);
            if segments.len() > 16 {
                return Err(ProbeError::Limit);
            }
        }
    }
    segments.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    if segments.is_empty() {
        return Err(ProbeError::Frame);
    }
    let unapproved_segment = describe_file(negative, &negative_device, "00000000000000000000.seg")?;
    Ok(PreparedTrustedProbe {
        schema: 1,
        scope: "trusted_import_validation_only",
        probe_id: probe_id.into(),
        brain_id: brain,
        source_device: source_device.to_string(),
        receiver_device: receiver_device.to_string(),
        reciprocal_approval_verified: true,
        segments,
        unapproved_segment,
        unapproved_device: negative_device.to_string(),
        unapproved_descriptor_sha256: format!(
            "{:X}",
            Sha256::digest(
                fs::read(negative.join("signed-probe.json")).map_err(|_| ProbeError::Io)?
            )
        ),
        published: false,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::canonical_writer::{
        AppendMemoryNoteArgs, CanonicalWriterConfig, append_memory_note,
    };
    use crate::event_signing_key::load_existing_event_signing_key;
    use cryptex_events::{BootstrapApprovalRole, DeviceSeq, SigningKeyId};
    use std::io::Write;

    fn local(
        root: &Path,
        brain: &str,
        store: &crate::FakeSecureStore,
    ) -> (DeviceId, cryptex_events::AcceptedEvent) {
        crate::identity_runtime::write_brain_identity(
            root,
            &cryptex_core::BrainIdentity::new(brain, "2026-11-08T00:00:00Z").unwrap(),
        )
        .unwrap();
        let local = crate::local_device_identity::enroll_local_device(root, store, ENV).unwrap();
        let device = DeviceId::new(local.device_id).unwrap();
        let event = append_memory_note(
            store,
            ENV,
            AppendMemoryNoteArgs {
                runtime_root: root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Public,
                text: TIMELINE_NOTE,
                event_id: None,
                config: &CanonicalWriterConfig::default(),
            },
        )
        .unwrap()
        .accepted;
        (device, event)
    }
    pub(crate) fn pair() -> (
        std::path::PathBuf,
        std::path::PathBuf,
        std::path::PathBuf,
        crate::FakeSecureStore,
    ) {
        let source = crate::test_temp::exclusive_temp_dir("trusted-source");
        let receiver = crate::test_temp::exclusive_temp_dir("trusted-receiver");
        let negative = crate::test_temp::exclusive_temp_dir("trusted-negative");
        let store = crate::FakeSecureStore::new();
        let brain = "11111111-1111-4111-8111-111111111111";
        let (a, first_a) = local(&source, brain, &store);
        let (b, first_b) = local(&receiver, brain, &store);
        let key_a = load_existing_event_signing_key(&store, ENV, &source, &a).unwrap();
        let key_b = load_existing_event_signing_key(&store, ENV, &receiver, &b).unwrap();
        let transport_a = "XAAPCAD-GA4SF4J-BNQL3FS-T456WKV-FGLXUKO-CDWHVDW-EYWNRIG-SM5GUAZ";
        let transport_b = "U2SYIGU-YWZX757-2WPZGCR-DHD7MLH-ZKV7HLJ-EYO6CXE-K3DU3DU-SDFXYAE";
        let offer = cryptex_events::create_bootstrap_offer(
            brain,
            &"a".repeat(64),
            &a,
            transport_a,
            &[1; 32],
            &key_a.signing_key,
        )
        .unwrap();
        let proof = cryptex_events::create_bootstrap_proof(
            &offer,
            &b,
            transport_b,
            &[2; 32],
            &key_b.signing_key,
        )
        .unwrap();
        let request = cryptex_events::verify_bootstrap_approval_request(
            &[&offer, &proof],
            BootstrapApprovalRole::Initiator,
        )
        .unwrap();
        let approval_a =
            cryptex_events::sign_bootstrap_approval(&request, &key_a.signing_key).unwrap();
        let request = cryptex_events::verify_bootstrap_approval_request(
            &[&offer, &proof, &approval_a],
            BootstrapApprovalRole::Responder,
        )
        .unwrap();
        let approval_b =
            cryptex_events::sign_bootstrap_approval(&request, &key_b.signing_key).unwrap();
        let transcript = cryptex_events::verify_bootstrap_transcript(&[
            &offer,
            &proof,
            &approval_a,
            &approval_b,
        ])
        .unwrap();
        // Test-only canonical fixtures: genuinely signed local approval records,
        // not injected trust rows or fake policy flags. Product code only reads them.
        for (root, device, first, key, event_id) in [
            (
                &source,
                &a,
                first_a,
                &key_a,
                "22222222-2222-4222-8222-222222222222",
            ),
            (
                &receiver,
                &b,
                first_b,
                &key_b,
                "33333333-3333-4333-8333-333333333333",
            ),
        ] {
            assert_eq!(
                crate::sequence_runtime::allocate_device_sequence(root, device).unwrap(),
                2
            );
            let body = cryptex_events::sign_peer_approved_event(
                &transcript,
                event_id,
                DeviceSeq::from_u64(2).unwrap(),
                "2026-11-08T00:00:01Z",
                &SigningKeyId::parse(&key.public.signing_key_id).unwrap(),
                &first.content_hash,
                &key.signing_key,
            )
            .unwrap();
            let mut file = fs::OpenOptions::new()
                .append(true)
                .open(cryptex_storage::active_segment_path(root, device, 0))
                .unwrap();
            file.write_all(&cryptex_events::encode_frame_header(body.len() as u32).unwrap())
                .unwrap();
            file.write_all(&body).unwrap();
        }
        (source, receiver, negative, store)
    }
    #[test]
    fn trusted_peer_stream_passes_full_import_policy_without_publication() {
        let (source, receiver, negative, store) = pair();
        let report = prepare(
            &source,
            &receiver,
            &negative,
            "44444444-4444-4444-8444-444444444444",
            &store,
        )
        .unwrap();
        let mut paths = Vec::new();
        for segment in &report.segments {
            let path = receiver
                .join("transport/incoming")
                .join(&segment.relative_path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::copy(
                source.join("canonical/events").join(&segment.relative_path),
                &path,
            )
            .unwrap();
            paths.push(std::path::PathBuf::from(&segment.relative_path));
        }
        let result =
            crate::incoming_validation::validate_incoming(&receiver, &paths, &store, ENV).unwrap();
        assert!(!result.published);
        assert_eq!(result.outcome, "validated_only");
        assert_eq!(result.event_count, 3);
        assert!(!receiver.join("canonical/events").join(&paths[0]).exists());
        let again =
            crate::incoming_validation::validate_incoming(&receiver, &paths, &store, ENV).unwrap();
        assert_eq!(again.event_count, 3);
        assert!(!receiver.join("canonical/events").join(&paths[0]).exists());
        let db = cryptex_projection::ProjectionDb::open_or_create(&receiver).unwrap();
        db.connection().execute("UPDATE knowledge_materialization_state SET integrity_content_hash = 'invalid' WHERE integrity_content_hash IS NOT NULL",[]).unwrap();
        drop(db);
        assert!(
            crate::incoming_validation::validate_incoming(&receiver, &paths, &store, ENV).is_err(),
            "stale existing watermarks must still hold"
        );
        for root in [&source, &receiver, &negative] {
            fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn unapproved_same_brain_signer_and_corruption_reject() {
        let (source, receiver, negative, store) = pair();
        let report = prepare(
            &source,
            &receiver,
            &negative,
            "44444444-4444-4444-8444-444444444444",
            &store,
        )
        .unwrap();
        crate::signed_transfer_probe::verify_files(
            &negative
                .join("canonical/events")
                .join(&report.unapproved_segment.relative_path),
            &negative.join("signed-probe.json"),
        )
        .expect("negative signer has a genuine valid signature in the same Brain");
        let relative = std::path::PathBuf::from(&report.unapproved_segment.relative_path);
        let target = receiver.join("transport/incoming").join(&relative);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(negative.join("canonical/events").join(&relative), &target).unwrap();
        let rejected =
            crate::incoming_validation::validate_incoming(&receiver, &[relative], &store, ENV)
                .unwrap_err();
        assert!(
            rejected
                .to_string()
                .contains("retrieval safety gate is held: Some(CanonicalVerificationUnavailable)"),
            "{rejected}"
        );
        let segment = &report.segments[0];
        let target = receiver
            .join("transport/incoming")
            .join(&segment.relative_path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        let mut bytes =
            fs::read(source.join("canonical/events").join(&segment.relative_path)).unwrap();
        let offset = bytes
            .windows(TIMELINE_NOTE.len())
            .position(|w| w == TIMELINE_NOTE.as_bytes())
            .unwrap();
        bytes[offset] ^= 1;
        fs::write(&target, bytes).unwrap();
        let rejected = crate::incoming_validation::validate_incoming(
            &receiver,
            &[segment.relative_path.clone().into()],
            &store,
            ENV,
        )
        .unwrap_err();
        assert!(
            rejected.to_string().contains("integrity_failure"),
            "{rejected}"
        );
        for root in [&source, &receiver, &negative] {
            fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn missing_approval_and_non_synthetic_source_are_refused_before_capture() {
        let store = crate::FakeSecureStore::new();
        let source = crate::test_temp::exclusive_temp_dir("unpaired-source");
        let receiver = crate::test_temp::exclusive_temp_dir("unpaired-receiver");
        let negative = crate::test_temp::exclusive_temp_dir("unpaired-negative");
        let brain = "11111111-1111-4111-8111-111111111111";
        local(&source, brain, &store);
        local(&receiver, brain, &store);
        assert!(
            prepare(
                &source,
                &receiver,
                &negative,
                "44444444-4444-4444-8444-444444444444",
                &store
            )
            .is_err()
        );
        assert!(fs::read_dir(&negative).unwrap().next().is_none());
        for root in [&source, &receiver, &negative] {
            fs::remove_dir_all(root).unwrap();
        }
        let (source, receiver, negative, store) = pair();
        let (_, device, _) = context(&source, &store).unwrap();
        append_memory_note(
            &store,
            ENV,
            AppendMemoryNoteArgs {
                runtime_root: &source,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Public,
                text: "PRIVATE CONTENT MUST NOT BE EXPORTED",
                event_id: None,
                config: &CanonicalWriterConfig::default(),
            },
        )
        .unwrap();
        let issued =
            crate::sequence_runtime::last_issued_device_sequence(&source, &device).unwrap();
        assert!(
            prepare(
                &source,
                &receiver,
                &negative,
                "44444444-4444-4444-8444-444444444444",
                &store
            )
            .is_err()
        );
        assert_eq!(
            issued,
            crate::sequence_runtime::last_issued_device_sequence(&source, &device).unwrap()
        );
        for root in [&source, &receiver, &negative] {
            fs::remove_dir_all(root).unwrap();
        }
    }
}
