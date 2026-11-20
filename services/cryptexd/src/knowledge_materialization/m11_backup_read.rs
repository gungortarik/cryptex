use super::*;
use crate::attachment_blobs::{BlobError, LocalLinkError};
use crate::ipc::attachment::SelectedFile;
use cryptex_events::attachment_link::LinkError;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::process::Command;

#[path = "m11_first_device.rs"]
mod first_device;

fn run_archive(action: &str, paths: &[&Path]) {
    archive_operation(action, paths).unwrap();
}

fn archive_operation(action: &str, paths: &[&Path]) -> std::io::Result<()> {
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/m11-backup-spike/daemon_roundtrip.py");
    let status = Command::new("python")
        .arg("-B")
        .arg(script)
        .arg(action)
        .args(paths)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "M11 archive {action}: {status}"
        )))
    }
}

#[test]
#[ignore = "requires the M11 Node/Python fixture installed by native CI"]
fn next_backup_restores_the_offline_capture_after_reconnection() {
    use crate::backup_scheduler::journal::DeviceJobs;
    use crate::backup_scheduler::{Outcome, PendingBackup};
    use crate::capture::{CaptureOutcome, CaptureRequest, capture_memory_note};
    use crate::secure_store::FileBackedSecureStore;
    use std::sync::mpsc;
    use std::time::Duration;

    let env = SecureStoreEnvironment::Dev;
    if let Some(profile) = std::env::var_os("CRYPTEX_M11_JOB_CHILD") {
        let root = Path::new(&profile);
        let _instance = crate::instance::try_acquire_instance(root).unwrap();
        let store = FileBackedSecureStore::open(root.join("qa-credentials")).unwrap();
        let reader = LocalDeviceKeyResolver::load(root, BRAIN, &store, env).unwrap();
        let enrolled = reader.local_trust_anchor().unwrap();
        let jobs = DeviceJobs::new(&store, env, &enrolled);
        let (_tx, stop) = mpsc::channel();
        let outcome = jobs
            .resume(
                &enrolled,
                &stop,
                || {
                    crate::test_interrupt::abrupt_exit_at_boundary("m11-waiting", root, "pending");
                },
                |_| panic!("replay attempted a second publication"),
            )
            .unwrap();
        assert_eq!(outcome, Some(Outcome::Published));
        return;
    }
    let work = tempfile::tempdir().unwrap();
    let writer = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let store = FileBackedSecureStore::open(source.path().join("qa-credentials")).unwrap();
    let (_, signing_key) = PairProfiles::enroll(source.path(), &store);
    drop(signing_key);
    let mut reader = LocalDeviceKeyResolver::load(source.path(), BRAIN, &store, env).unwrap();
    let anchor = reader.local_trust_anchor().unwrap();
    let kit = work.path().join("recovery-kit");
    let volume = work.path().join("backup-volume");
    fs::create_dir(&volume).unwrap();
    let first = volume.join("backup-00000000000000000001.pgp");
    let next = volume.join("backup-00000000000000000002.pgp");
    run_archive("prepare", &[source.path(), &kit, &first, writer.path()]);
    let first_digest = Sha256::digest(fs::read(&first).unwrap());
    let jobs = DeviceJobs::new(&store, env, &anchor);
    jobs.enqueue(&first, first_digest.into(), 2).unwrap();
    let job = PendingBackup {
        designated_writer: &anchor,
        previous_generation: &first,
        previous_sha256: first_digest.into(),
    };
    // Both devices see the same target and assignment. Only the enrolled
    // identity matching the assignment may enter the publication path.
    let peer = tempfile::tempdir().unwrap();
    let peer_store = FakeSecureStore::new();
    PairProfiles::enroll(peer.path(), &peer_store);
    let peer_reader = LocalDeviceKeyResolver::load(peer.path(), BRAIN, &peer_store, env).unwrap();
    let peer_anchor = peer_reader.local_trust_anchor().unwrap();
    let (_keep_running, shutdown) = mpsc::channel();
    assert_eq!(
        job.run(
            &peer_anchor,
            &shutdown,
            || panic!("peer waited for target"),
            || panic!("peer started backup")
        )
        .unwrap(),
        Outcome::NotDesignated
    );

    let detached = work.path().join("detached-volume");
    fs::rename(&volume, &detached).unwrap();
    fs::create_dir(&volume).unwrap();
    fs::write(&first, b"different backup target").unwrap();
    assert_eq!(
        job.run(
            &anchor,
            &shutdown,
            || panic!("unexpected wait"),
            || panic!("wrong target")
        )
        .unwrap_err()
        .kind(),
        std::io::ErrorKind::InvalidData
    );
    fs::remove_file(&first).unwrap();
    fs::remove_dir(&volume).unwrap();
    assert_eq!(
        fs::File::open(&first).unwrap_err().kind(),
        std::io::ErrorKind::NotFound
    );
    let launch_saved_job = || {
        Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "knowledge_materialization::pairing_recovery_tests::m11_backup_read::next_backup_restores_the_offline_capture_after_reconnection", "--ignored", "--nocapture"])
        .env("CRYPTEX_M11_JOB_CHILD", source.path()).status().unwrap()
    };
    assert_eq!(
        launch_saved_job().code(),
        Some(crate::test_interrupt::INTERRUPT_EXIT_CODE)
    );
    let text = "M11 reconnectcapture saved offline";
    let receipt = std::thread::scope(|scope| {
        let (stop, shutdown) = mpsc::channel();
        let (waiting, observed) = mpsc::channel();
        let (result, finished) = mpsc::channel();
        let publish = |staged: &Path| {
            archive_operation("write-generation", &[source.path(), writer.path(), staged])
        };
        let assigned = &anchor;
        scope.spawn(move || {
            let outcome = jobs.resume(assigned, &shutdown, || waiting.send(()).unwrap(), publish);
            let _ = result.send(outcome);
        });
        observed.recv_timeout(Duration::from_secs(10)).unwrap();
        let outcome = capture_memory_note(
            &store,
            env,
            source.path(),
            CaptureRequest {
                capture_id: "44f9399a-688d-4c43-b688-cabea438c9ad".into(),
                classification: Classification::Public,
                text: text.into(),
            },
            &CanonicalWriterConfig::default(),
        );
        let CaptureOutcome::Committed(receipt) = outcome else {
            panic!("expected committed offline capture");
        };
        assert!(!volume.try_exists().unwrap());
        assert!(
            !detached
                .join(next.file_name().unwrap())
                .try_exists()
                .unwrap()
        );
        fs::rename(detached, &volume).unwrap();
        assert_eq!(
            finished
                .recv_timeout(Duration::from_secs(120))
                .unwrap()
                .unwrap(),
            Some(Outcome::Published)
        );
        drop(stop);
        receipt
    });
    assert!(launch_saved_job().success());
    println!("m11_backup_job_restart=passed offline_exit=recovered published_replay=idempotent");
    assert_eq!(fs::read_dir(&volume).unwrap().count(), 3); // two archives and writer lock
    assert_eq!(Sha256::digest(fs::read(&first).unwrap()), first_digest);
    let manual = volume.join("manual-export.pgp");
    run_archive("write-generation", &[source.path(), writer.path(), &manual]);
    let [first_publication, next_publication] = DeviceJobs::new(&store, env, &anchor)
        .published_pair()
        .unwrap();

    // Only public event verification material and the independent kit survive.
    drop(store);
    source.close().unwrap();
    writer.close().unwrap();
    let query = SearchQuery {
        text: Some("prefix OR reconnectcapture".into()),
        ..Default::default()
    };
    let mut restore_hits = |file: &Path, name: &str| {
        let sandbox = work.path().join(name);
        run_archive("recover", &[&kit, &sandbox, file]);
        assert_eq!(fs::read_dir(&sandbox).unwrap().count(), 2);
        assert_eq!(
            crate::identity_runtime::load_brain_identity(&sandbox)
                .unwrap()
                .brain_id,
            BRAIN
        );
        let (mut db, _) = crate::projection::open_projection_db(&sandbox).unwrap();
        rebuild_knowledge_from_local_approval_history(&sandbox, BRAIN, &anchor, &mut db).unwrap();
        search_events_with_retrieval_safety(&sandbox, BRAIN, &[], &mut reader, &db, &query).unwrap()
    };
    let receipt_store = FileBackedSecureStore::open(work.path().join("restore-receipts")).unwrap();
    let catalog = crate::backup_scheduler::retention::VerifiedBackups {
        store: &receipt_store,
        environment: env,
        designated: &anchor,
        target: &volume,
    };
    let before = catalog
        .verify(&anchor, &first_publication, |file| {
            let hits = restore_hits(file, "restore-first");
            assert_eq!(hits.len(), 1);
            assert_eq!(hits[0].text, "synthetic original prefix");
            assert_ne!(hits[0].event_id, receipt.event_id);
            Ok(hits)
        })
        .unwrap();
    assert!(
        catalog
            .verify(&anchor, &next_publication, |_| Err::<(), _>(
                std::io::Error::other("reconstruction failed")
            ))
            .is_err()
    );
    let after =
        catalog
            .verify(&anchor, &next_publication, |file| {
                let hits = restore_hits(file, "restore-next");
                assert!(hits.iter().any(|hit| hit.event_id == receipt.event_id
                    && hit.content_hash == receipt.content_hash));
                assert!(hits.contains(&before[0]));
                Ok(hits)
            })
            .unwrap();
    assert_eq!(after.len(), 2);
    assert!(after.contains(&before[0]));
    let saved = after
        .iter()
        .find(|hit| hit.event_id == receipt.event_id)
        .unwrap();
    assert_eq!(saved.content_hash, receipt.content_hash);
    assert_eq!(saved.device_id, receipt.device_id);
    assert_eq!(saved.text, text);
    let keep = std::num::NonZeroUsize::new(1);
    let unknown = volume.join("backup-00000000000000000003.pgp");
    fs::write(&unknown, b"unverified fixture").unwrap();
    assert_eq!(
        catalog
            .set_retention(&peer_anchor, keep)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::PermissionDenied
    );
    assert_eq!(catalog.prune(&anchor).unwrap(), 0);
    catalog.set_retention(&anchor, keep).unwrap();
    let reopened = FileBackedSecureStore::open(receipt_store.root()).unwrap();
    let catalog = crate::backup_scheduler::retention::VerifiedBackups {
        store: &reopened,
        ..catalog
    };
    assert_eq!(
        catalog.prune(&peer_anchor).unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied
    );
    let good = fs::read(&next).unwrap();
    fs::write(&next, b"corrupt fixture").unwrap();
    assert_eq!(
        catalog.prune(&anchor).unwrap_err().kind(),
        std::io::ErrorKind::InvalidData
    );
    assert!(first.exists());
    fs::write(&next, good).unwrap();
    catalog
        .verify(&anchor, &next_publication, |file| {
            assert_eq!(restore_hits(file, "recheck-after-restart"), after);
            Ok(())
        })
        .unwrap();
    assert!(!first.try_exists().unwrap());
    assert_eq!(catalog.prune(&anchor).unwrap(), 0);
    catalog.set_retention(&anchor, None).unwrap();
    assert!(next.is_file());
    assert_eq!(fs::read(unknown).unwrap(), b"unverified fixture");
    assert_eq!(restore_hits(&manual, "restore-manual"), after);
    println!(
        "m11_retention=passed default=keep_all retained=1 manual=restored unverified=preserved damaged=blocked"
    );
    println!(
        "m11_scheduled_reconnect=passed designated=published peer=skipped offline_capture=restored"
    );
    println!(
        "m11_reconnected_backup=passed generations=2 original=preserved offline_capture=restored receipt=matched"
    );
}

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
    let history_digest = super::super::recovered_search::history_digest(&snapshot);
    let restored = work.path().join("replacement/recovery/archive");
    let kit = work.path().join("kit");
    let volume = work.path().join("backup-volume");
    fs::create_dir(&volume).unwrap();
    let backup = volume.join("backup.pgp");
    let original_identity = fs::read(crate::identity_runtime::brain_identity_path(source)).unwrap();
    run_archive("prepare", &[source, &kit, &backup]);
    drop((snapshot, instance));
    let old_profiles = profiles.profiles.clone();
    drop(profiles); // Drops both original private keys and the old secure store.
    assert!(old_profiles.iter().all(|root| !root.exists()));
    fs::create_dir_all(restored.parent().unwrap()).unwrap();
    run_archive("recover", &[&kit, &restored, &backup]);
    let backup_sha256: [u8; 32] = Sha256::digest(fs::read(&backup).unwrap()).into();
    // Model an unmounted filesystem target after decrypting the archive. Only
    // the test parent knows where the volume went. Local credentials stay put.
    let detached = tempfile::tempdir().unwrap();
    let parked = detached.path().join("volume");
    fs::rename(&volume, &parked).unwrap();
    assert_eq!(
        fs::File::open(&backup).unwrap_err().kind(),
        std::io::ErrorKind::NotFound
    );
    let mut keys = first_device::rehearse(
        work.path(),
        backup_sha256,
        &anchor,
        offline,
        pinned_recovery_key,
        history_digest,
    );
    let anchor = keys.local_trust_anchor().unwrap();
    assert_eq!(fs::read_dir(&restored).unwrap().count(), 2);
    for root in [&restored, &work.path().join("replacement")] {
        assert_eq!(
            fs::read(crate::identity_runtime::brain_identity_path(root)).unwrap(),
            original_identity
        );
    }
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
    let identity_path = crate::identity_runtime::brain_identity_path(&restored);
    let changed = BrainIdentity::new(BRAIN, "2026-02-01T00:00:00Z")
        .unwrap()
        .to_json_lf()
        .unwrap();
    fs::write(&identity_path, changed).unwrap();
    first_device::restarted_search::check(work.path(), &note.event_id, "checkpoint-held");
    fs::write(&identity_path, &original_identity).unwrap();
    assert!(!volume.try_exists().unwrap());
    fs::rename(&parked, &volume).unwrap();
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(fs::read(&backup).unwrap())),
        backup_sha256
    );
    println!("m11_offline_backup_target=passed capture=committed search=reopened backup=unchanged");
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
