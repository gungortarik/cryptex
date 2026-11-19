use super::{ABC, Profile};
use crate::attachment_blobs::{BlobError, LocalLinkError, store, verify_local_link};
use cryptex_core::DeviceId;
use cryptex_events::attachment_link::{EVENT_TYPE, LinkError, LinkPayload};
use cryptex_events::{
    AcceptedEvent, Classification, ContentHashHex, DeviceSeq, EventPayload, EventType,
    MemoryNotePayload, SignableEvent, SignatureHex, TrustedDaemonKeyResolver,
    V2_CONTENT_HASH_DOMAIN, V2_SIGNATURE_DOMAIN, VersionedRecord, accept_event,
    signing_key_id_from_verifying_key, verify_v2_history,
};
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;

const DEVICE: &str = "m9-test-device";
const NOTE: &str = "11111111-1111-4111-8111-111111111111";
const LINK: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const BRAIN: &str = "22222222-2222-4222-8222-222222222222";
const TIMESTAMP: &str = "2026-11-18T00:00:00Z";

struct TrustedTestDevice(VerifyingKey);
impl TrustedDaemonKeyResolver for TrustedTestDevice {
    fn trusted_daemon_key(&mut self, device: &str, key_id: &str) -> Option<VerifyingKey> {
        (device == DEVICE && signing_key_id_from_verifying_key(&self.0).unwrap().as_str() == key_id)
            .then_some(self.0)
    }
}

fn note_and_link(key: &SigningKey) -> (AcceptedEvent, Value) {
    let note = accept_event(
        SignableEvent {
            schema_version: 1,
            event_id: NOTE.into(),
            brain_id: BRAIN.into(),
            device_id: DEVICE.into(),
            device_seq: DeviceSeq::from_u64(1).unwrap(),
            created_at: TIMESTAMP.into(),
            event_type: EventType::MemoryNote,
            classification: Classification::Restricted,
            signing_key_id: signing_key_id_from_verifying_key(&key.verifying_key()).unwrap(),
            payload: EventPayload::MemoryNote(MemoryNotePayload::new("Synthetic note").unwrap()),
            prev_content_hash: ContentHashHex::zero(),
        },
        key,
    )
    .unwrap();
    let link = json!({
        "schema_version": 2, "event_id": LINK, "brain_id": note.brain_id,
        "device_id": DEVICE, "device_seq": "2", "created_at": TIMESTAMP,
        "event_type": EVENT_TYPE, "payload_schema_version": 1,
        "classification": note.classification, "signing_key_id": note.signing_key_id,
        "prev_content_hash": note.content_hash,
        "payload": LinkPayload::for_note(&note, &Sha256::digest(b"abc").into()).unwrap()
    });
    (note, link)
}

// Deliberately test-only until the ADR-0017 rollout gate permits v2 writes.
fn sign_link(mut body: Value, key: &SigningKey) -> Vec<u8> {
    let canonical = serde_json_canonicalizer::to_vec(&body).unwrap();
    let mut hasher = blake3::Hasher::new();
    hasher.update(V2_CONTENT_HASH_DOMAIN);
    hasher.update(&[0]);
    hasher.update(&canonical);
    let hash = hasher.finalize();
    let signature = key.sign(&[V2_SIGNATURE_DOMAIN, &[0], hash.as_bytes()].concat());
    body["content_hash"] = json!(ContentHashHex::from_bytes(hash.as_bytes()).unwrap());
    body["signature"] = json!(SignatureHex::from_bytes(&signature.to_bytes()).unwrap());
    serde_json_canonicalizer::to_vec(&body).unwrap()
}

#[test]
fn signed_note_and_link_reopen_with_verified_bytes_and_replay_once() {
    let profile = Profile::new();
    let key = SigningKey::from_bytes(&[21; 32]);
    let (note, unsigned) = note_and_link(&key);
    let original = serde_json_canonicalizer::to_vec(&note).unwrap();
    let signed = sign_link(unsigned, &key);
    let device = DeviceId::new(DEVICE).unwrap();
    cryptex_storage::append_frame(&profile.0, &device, 1, &original).unwrap();
    let history = verify_v2_history(
        &[&original, &signed],
        TrustedTestDevice(key.verifying_key()),
    )
    .unwrap();
    assert_eq!(
        verify_local_link(&profile.0, &history, LINK),
        Err(LocalLinkError::Blob(BlobError::Missing))
    );
    let digest = store(&profile.0, &b"abc"[..]).unwrap();
    verify_local_link(&profile.0, &history, LINK).unwrap();
    cryptex_storage::append_frame(&profile.0, &device, 1, &signed).unwrap();
    cryptex_storage::seal_active_segment(&profile.0, &device, 1).unwrap();
    let pack = fs::read(cryptex_storage::sealed_segment_path(&profile.0, &device, 1)).unwrap();
    let (frames, consumed) = cryptex_storage::scan_complete_frames(&pack).unwrap();
    assert_eq!(consumed, pack.len());
    let bodies: Vec<_> = frames
        .iter()
        .map(|frame| &frame[cryptex_events::FRAME_HEADER_BYTES..])
        .collect();
    assert_eq!(bodies, [original.as_slice(), signed.as_slice()]);
    let reopened = verify_v2_history(
        &[bodies[0], bodies[1], bodies[1]],
        TrustedTestDevice(key.verifying_key()),
    )
    .unwrap();
    assert_eq!(reopened.records().len(), 2);
    assert!(
        matches!(&reopened.records()[1], VersionedRecord::VerifiedUnsupported(r) if r.raw_record == bodies[1])
    );
    let link = verify_local_link(&profile.0, &reopened, LINK).unwrap();
    assert_eq!(link.note_event_id, NOTE);
    assert_eq!(link.blob_sha256, digest);
    assert_eq!(link.classification, Classification::Restricted);
    fs::write(profile.blobs().join(ABC), b"abd").unwrap();
    assert_eq!(
        verify_local_link(&profile.0, &reopened, LINK),
        Err(LocalLinkError::Blob(BlobError::IntegrityMismatch))
    );
}

#[test]
fn signed_but_invalid_references_are_rejected() {
    let key = SigningKey::from_bytes(&[21; 32]);
    let (note, unsigned) = note_and_link(&key);
    let original = serde_json_canonicalizer::to_vec(&note).unwrap();
    for (field, value, expected) in [
        (
            "/payload/note_event_id",
            json!(BRAIN),
            LinkError::MissingNote,
        ),
        (
            "/payload/note_event_id",
            json!("bad"),
            LinkError::InvalidPayload,
        ),
        (
            "/payload/note_content_hash",
            json!("00".repeat(32)),
            LinkError::NoteHashMismatch,
        ),
        ("/brain_id", json!(LINK), LinkError::BrainMismatch),
        (
            "/classification",
            json!("public"),
            LinkError::ClassificationMismatch,
        ),
        (
            "/payload/blob_sha256",
            json!(ABC.to_ascii_uppercase()),
            LinkError::InvalidPayload,
        ),
        (
            "/payload",
            json!({"note_event_id": NOTE, "note_content_hash": note.content_hash, "blob_sha256": ABC, "filename": "extra"}),
            LinkError::InvalidPayload,
        ),
        (
            "/payload_schema_version",
            json!(2),
            LinkError::UnsupportedTypeOrVersion,
        ),
    ] {
        let mut changed = unsigned.clone();
        *changed.pointer_mut(field).unwrap() = value;
        let signed = sign_link(changed, &key);
        let history = verify_v2_history(
            &[&original, &signed],
            TrustedTestDevice(key.verifying_key()),
        )
        .unwrap();
        assert_eq!(
            cryptex_events::attachment_link::resolve(&history, LINK),
            Err(expected),
            "{field}"
        );
    }
}

#[test]
fn changed_blob_reference_or_untrusted_signer_fails_history_verification() {
    let key = SigningKey::from_bytes(&[21; 32]);
    let (note, unsigned) = note_and_link(&key);
    let original = serde_json_canonicalizer::to_vec(&note).unwrap();
    let signed = sign_link(unsigned.clone(), &key);
    let mut altered: Value = serde_json::from_slice(&signed).unwrap();
    altered["payload"]["blob_sha256"] = json!("00".repeat(32));
    let tampered = serde_json_canonicalizer::to_vec(&altered).unwrap();
    let wrong_signature = sign_link(unsigned, &SigningKey::from_bytes(&[22; 32]));
    for invalid in [tampered, wrong_signature] {
        let err = verify_v2_history(
            &[&original, &invalid],
            TrustedTestDevice(key.verifying_key()),
        )
        .unwrap_err();
        assert_eq!(
            err.reason,
            cryptex_events::ValidationReasonCode::IntegrityFailure
        );
    }
}
