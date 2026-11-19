use super::{ABC, Profile};
use crate::attachment_blobs::{BlobError, LocalLinkError, store, verify_local_link};
use crate::test_temp::sign_v2_fixture;
use cryptex_core::DeviceId;
use cryptex_events::attachment_link::{EVENT_TYPE, LinkError, LinkPayload};
use cryptex_events::{
    AcceptedEvent, Classification, ContentHashHex, DeviceSeq, EventPayload, EventType,
    MemoryNotePayload, SignableEvent, TrustedDaemonKeyResolver, VersionedRecord, accept_event,
    signing_key_id_from_verifying_key, verify_v2_history,
};
use ed25519_dalek::{SigningKey, VerifyingKey};
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

mod migration;

#[test]
fn incomplete_m9_layout_is_never_repaired_by_open_or_probe() {
    use crate::instance::{create_m9_profile_root, observe_instance, try_acquire_instance};
    let temporary = Profile::new();
    let root = temporary.0.join("incomplete");
    fs::create_dir_all(root.join("daemon.lock")).unwrap();
    assert!(try_acquire_instance(&root).is_err());
    assert!(observe_instance(&root).is_err());
    assert!(create_m9_profile_root(&temporary.0, "incomplete").is_err());
    assert_eq!(fs::read_dir(root.join("daemon.lock")).unwrap().count(), 0);
}

#[test]
#[ignore = "CI supplies an independently built, pinned pre-M9 daemon"]
fn audit_pre_m9_profile_admission() {
    let old_daemon = std::env::var_os("CRYPTEX_M9_PREVIOUS_DAEMON").expect("pinned M8 daemon");
    use crate::instance::{create_m9_profile_root, observe_instance, try_acquire_instance};
    use std::process::Command;

    let temporary = Profile::new();
    let legacy = temporary.0.join("legacy");
    fs::create_dir(&legacy).unwrap();
    let old_run = |root: &std::path::Path, command: &str, extra: &[&str]| {
        Command::new(&old_daemon)
            .args(["lifecycle", command, "--runtime-root"])
            .arg(root)
            .args(extra)
            .output()
            .expect("execute independently built M8")
    };
    let probe = old_run(&legacy, "probe", &[]);
    assert!(probe.status.success());
    assert!(String::from_utf8_lossy(&probe.stdout).contains("probe acquired instance lock"));
    assert!(create_m9_profile_root(&temporary.0, "legacy").is_err());
    assert!(legacy.join("daemon.lock").is_file());

    let root = create_m9_profile_root(&temporary.0, "protected").unwrap();
    let key = SigningKey::from_bytes(&[21; 32]);
    let (note, unsigned) = note_and_link(&key);
    let note = serde_json_canonicalizer::to_vec(&note).unwrap();
    let link = sign_v2_fixture(unsigned, &key);
    let history =
        verify_v2_history(&[&note, &link], TrustedTestDevice(key.verifying_key())).unwrap();
    cryptex_events::attachment_link::resolve(&history, LINK).unwrap();
    let device = DeviceId::new(DEVICE).unwrap();
    {
        let _writer = try_acquire_instance(&root).unwrap();
        assert!(observe_instance(&root).unwrap().lock_held);
        assert!(matches!(
            try_acquire_instance(&root),
            Err(crate::error::LifecycleError::AlreadyRunning { .. })
        ));
        for record in [note, link] {
            cryptex_storage::append_frame(&root, &device, 0, &record).unwrap();
        }
    }
    let pack = cryptex_storage::active_segment_path(&root, &device, 0);
    let before = fs::read(&pack).unwrap();
    assert!(!observe_instance(&root).unwrap().lock_held);
    for (command, args) in [("probe", &[][..]), ("hold", &["--hold-ms", "1"][..])] {
        let result = old_run(&root, command, args);
        // The old executable reports the native error opening daemon.lock as
        // a file. A busy lock, bad command or unrelated crash cannot pass.
        let stderr = String::from_utf8(result.stderr).unwrap();
        let expected_os_code = if cfg!(windows) { 5 } else { 21 };
        assert_eq!(result.status.code(), Some(2), "{command}: {stderr}");
        assert!(
            stderr.trim_start().starts_with("cryptexd: io error: "),
            "{command}: {stderr}"
        );
        assert!(
            stderr
                .trim_end()
                .ends_with(&format!("(os error {expected_os_code})")),
            "{command}: {stderr}"
        );
        assert!(!root.join("lifecycle.status").exists());
        assert_eq!(fs::read(&pack).unwrap(), before);
    }
    // Reopen after the rejected downgrade and check the signed store again.
    let _reader = try_acquire_instance(&root).unwrap();
    assert_eq!(fs::read(&pack).unwrap(), before);
    assert!(observe_instance(&root).unwrap().lock_held);
    migration::exercise_existing_profile(&legacy, &old_daemon);
    println!(
        "historical_profile_lock=denied legacy_probe=allowed protected_hold=denied current_reopen=allowed pack_unchanged=true production_v2_activation=blocked"
    );
}

#[test]
fn signed_note_and_link_reopen_with_verified_bytes_and_replay_once() {
    let profile = Profile::new();
    let key = SigningKey::from_bytes(&[21; 32]);
    let (note, unsigned) = note_and_link(&key);
    let original = serde_json_canonicalizer::to_vec(&note).unwrap();
    let signed = sign_v2_fixture(unsigned, &key);
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
        let signed = sign_v2_fixture(changed, &key);
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
    let signed = sign_v2_fixture(unsigned.clone(), &key);
    let mut altered: Value = serde_json::from_slice(&signed).unwrap();
    altered["payload"]["blob_sha256"] = json!("00".repeat(32));
    let tampered = serde_json_canonicalizer::to_vec(&altered).unwrap();
    let wrong_signature = sign_v2_fixture(unsigned, &SigningKey::from_bytes(&[22; 32]));
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
