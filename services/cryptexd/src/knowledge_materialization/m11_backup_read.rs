use super::*;
use crate::attachment_blobs::{BlobError, LocalLinkError};
use crate::ipc::attachment::SelectedFile;
use cryptex_events::attachment_link::LinkError;
use std::io::Read;
use std::process::Command;

#[path = "m11_first_device.rs"]
mod first_device;

#[test]
#[ignore = "requires the M11 Node/Python fixture installed by native CI"]
fn restored_note_and_attachment_pass_guarded_reads() {
    if let Some(work) = std::env::var_os("CRYPTEX_M11_REOPEN_FIXTURE") {
        first_device::restarted_search::child(Path::new(&work));
        return;
    }
    let work = tempfile::Builder::new()
        .prefix("cryptex-m11-recovery-")
        .tempdir()
        .unwrap();
    let mut seed = [0u8; 32];
    getrandom::getrandom(&mut seed).unwrap();
    let offline = SigningKey::from_bytes(&seed);
    seed.fill(0);
    let pinned_recovery_key = offline.verifying_key();
    let profiles = PairProfiles::new();
    profiles.pair(0, [true, true], false);
    profiles.import_peer_fixture(0);
    let source = &profiles.profiles[0];
    drop(crate::instance::try_acquire_instance(source).unwrap());
    crate::instance::migration::migrate_profile(source).unwrap();
    let instance = crate::instance::try_acquire_instance(source).unwrap();
    let bytes = b"M11 synthetic attachment";
    let policy = profiles.policy();
    let note_for = |device: &DeviceId| {
        policy
            .history
            .records()
            .iter()
            .find_map(|record| match record {
                AuthenticatedSecurityRecord::V1(note)
                    if record.device_id() == device.as_str() && record.device_seq() == 1 =>
                {
                    Some(note.clone())
                }
                _ => None,
            })
            .unwrap()
    };
    let note = note_for(&profiles.devices[0]);
    let other = note_for(&profiles.devices[1]);
    drop(policy);
    crate::canonical_writer::isolated_attachment::append(
        source,
        &profiles.store,
        SecureStoreEnvironment::Dev,
        &SelectedFile {
            note_id: note.event_id.clone(),
            note_hash: note.content_hash.as_str().to_owned(),
            bytes: bytes.to_vec(),
        },
    )
    .unwrap();
    let policy = profiles.policy();
    let link = policy
        .history
        .records()
        .iter()
        .find_map(|record| match record {
            AuthenticatedSecurityRecord::Attachment(link, fields)
                if fields.note_event_id() == note.event_id =>
            {
                Some(link.event_id.clone())
            }
            _ => None,
        })
        .unwrap();
    drop(policy);
    // Public historical identity captured when preparing the trusted kit.
    let anchor = resolver(&profiles).local_trust_anchor().unwrap();
    let snapshot =
        scan_verified_canonical_from_local_anchor_until(source, BRAIN, &anchor, None).unwrap();
    let history_digest = first_device::restarted_search::history_digest(&snapshot);
    let restored = work.path().join("rehearsal");
    let kit = work.path().join("kit");
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/m11-backup-spike/daemon_roundtrip.py");
    let run = |action: &str, first: &Path, second: &Path| {
        assert!(
            Command::new("python")
                .arg("-B")
                .arg(&script)
                .arg(action)
                .arg(first)
                .arg(second)
                .status()
                .unwrap()
                .success()
        );
    };
    run("prepare", source, &kit);
    drop((snapshot, instance));
    let old_profiles = profiles.profiles.clone();
    drop(profiles); // Drops both original private keys and the old secure store.
    assert!(old_profiles.iter().all(|root| !root.exists()));
    run("recover", &kit, &restored);
    let mut keys = first_device::rehearse(
        work.path(),
        &kit.join("backup.pgp"),
        &anchor,
        offline,
        pinned_recovery_key,
        history_digest,
    );
    let anchor = keys.local_trust_anchor().unwrap();
    assert_eq!(fs::read_dir(&restored).unwrap().count(), 1);
    let (mut db, _) = crate::projection::open_projection_db(&restored).unwrap();
    rebuild_knowledge_from_local_approval_history(&restored, BRAIN, &anchor, &mut db).unwrap();
    let detail = event_detail_with_retrieval_safety_deadline(
        &restored,
        BRAIN,
        &[],
        &mut keys,
        &db,
        &note.event_id,
        None,
    )
    .unwrap()
    .unwrap();
    assert_eq!(detail.text, "synthetic original prefix");
    assert_eq!(detail.content_hash, note.content_hash.as_str());
    let state = evaluate_retrieval_safety(&restored, BRAIN, &[], &mut keys, &db);
    let mut content = Vec::new();
    state
        .open_local_attachment(&note.event_id, &link)
        .unwrap()
        .read_to_end(&mut content)
        .unwrap();
    assert_eq!(content, bytes);
    assert_eq!(
        state.verify_local_attachment(&other.event_id, &link),
        Err(LocalLinkError::Reference(LinkError::MissingLink))
    );
    drop(state);

    drop(db);
    first_device::restarted_search::check(work.path(), &note.event_id, "ready");
    let (mut db, _) = crate::projection::open_projection_db(&restored).unwrap();

    // Exercise blob checks independently from envelope and event verification.
    let blobs = restored.join("canonical/blobs");
    let entries = fs::read_dir(blobs)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(entries.len(), 1);
    let path = entries[0].path();
    fs::remove_file(&path).unwrap();
    for expected in [BlobError::Missing, BlobError::IntegrityMismatch] {
        if expected == BlobError::IntegrityMismatch {
            fs::write(&path, vec![0; bytes.len()]).unwrap();
        }
        let state = evaluate_retrieval_safety(&restored, BRAIN, &[], &mut keys, &db);
        assert_eq!(
            state.verify_local_attachment(&note.event_id, &link),
            Err(LocalLinkError::Blob(expected))
        );
    }
    fs::write(path, bytes).unwrap();
    // Keep the JSON, frame lengths, signed content and derived database intact.
    // Change one hex digit in the original note signature.
    let signature = note.signature.as_str().as_bytes();
    let mut changed = 0;
    for (path, mut pack) in canonical_bytes(&restored) {
        if let Some(index) = pack.windows(signature.len()).position(|v| v == signature) {
            pack[index] = if pack[index] == b'0' { b'1' } else { b'0' };
            fs::write(path, pack).unwrap();
            changed += 1;
        }
    }
    assert_eq!(changed, 1);
    first_device::restarted_search::check(work.path(), &note.event_id, "history-held");
    assert!(
        rebuild_knowledge_from_local_approval_history(&restored, BRAIN, &anchor, &mut db).is_err()
    );
    assert!(
        event_detail_with_retrieval_safety_deadline(
            &restored,
            BRAIN,
            &[],
            &mut keys,
            &db,
            &note.event_id,
            None
        )
        .is_err()
    );
    let state = evaluate_retrieval_safety(&restored, BRAIN, &[], &mut keys, &db);
    assert!(matches!(
        state.verify_local_attachment(&note.event_id, &link),
        Err(LocalLinkError::Held(_))
    ));
    println!(
        "m11_guarded_restore=passed signed_inventory=verified wrong_note=denied missing_blob=denied damaged_blob=denied forged_note=held"
    );
    println!(
        "m11_first_device=passed old_devices=lost kit_authorized=true replacement_note=verified historical_attachment=verified"
    );
    println!(
        "m11_restarted_search=passed single_fts=true stale_index_held=both damaged_receipt=held"
    );
}
