use super::*;
use crate::attachment_blobs::{BlobError, LocalLinkError};
use crate::ipc::attachment::SelectedFile;
use cryptex_events::attachment_link::LinkError;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::process::Command;

#[path = "m11_first_device.rs"]
mod first_device;
#[path = "m11_installed_export.rs"]
mod installed_export;
#[path = "m11_separate_backups.rs"]
mod separate_backups;

#[test]
fn public_kit_allows_ordinary_backup_but_partial_recovery_holds() {
    use super::super::{backup, recovered_search};
    use crate::recovery_store::{
        recovery_dir, recovery_public_metadata_path, write_recovery_public_metadata,
    };
    use cryptex_core::{RecoveryKitPublicMetadata, RecoveryPurpose};
    let profiles = PairProfiles::new();
    let root = &profiles.profiles[0];
    let device = &profiles.devices[0];
    let env = SecureStoreEnvironment::Dev;
    let mode = || recovered_search::for_profile(root, BRAIN, device, &profiles.store, env);
    let prepare = || backup::prepare(root, &profiles.store, env, 1);
    let public =
        RecoveryKitPublicMetadata::new("offline-kit", BRAIN, vec![RecoveryPurpose::BackupDecrypt])
            .unwrap();
    write_recovery_public_metadata(root, &public).unwrap();
    assert!(matches!(mode(), Ok(None)));
    assert!(prepare().is_ok());
    let folder = recovery_dir(root);
    for name in ["first-device.json", "archive", "unknown"] {
        let extra = folder.join(name);
        fs::create_dir(&extra).unwrap();
        assert!(matches!(
            mode(),
            Err(recovered_search::SearchHold::Approval)
        ));
        assert!(matches!(
            prepare(),
            Err(crate::backup_inventory::Error::RecoveryHistory)
        ));
        fs::remove_dir(extra).unwrap();
    }
    let metadata_path = recovery_public_metadata_path(root);
    let original = fs::read(&metadata_path).unwrap();
    let mut other = public.clone();
    other.brain_id = "ba890321-c7ff-4679-bf0c-78288db36218".into();
    write_recovery_public_metadata(root, &other).unwrap();
    assert!(matches!(
        mode(),
        Err(recovered_search::SearchHold::Approval)
    ));
    fs::write(&metadata_path, b"broken").unwrap();
    assert!(matches!(
        mode(),
        Err(recovered_search::SearchHold::Approval)
    ));
    fs::write(&metadata_path, original).unwrap();
    assert!(matches!(mode(), Ok(None)));
}

#[test]
fn a_forgotten_notes_attachment_survives_backup_reconstruction() {
    use super::super::backup;
    let profiles = PairProfiles::new();
    profiles.pair(0, [true, true], false);
    profiles.import_peer_fixture(0);
    let root = &profiles.profiles[0];
    let env = SecureStoreEnvironment::Dev;
    drop(crate::instance::try_acquire_instance(root).unwrap());
    crate::instance::migration::migrate_profile(root).unwrap();
    let _instance = crate::instance::try_acquire_instance(root).unwrap();
    let owner = resolver(&profiles).local_trust_anchor().unwrap();
    let note = append_memory_note(
        &profiles.store,
        env,
        AppendMemoryNoteArgs {
            runtime_root: root,
            brain_id: BRAIN,
            device_id: owner.device_id(),
            classification: Classification::Public,
            text: "reversible note",
            event_id: None,
            config: &CanonicalWriterConfig::default(),
        },
    )
    .unwrap()
    .accepted;
    let bytes = b"file on a forgotten note";
    crate::canonical_writer::isolated_attachment::append(
        root,
        &profiles.store,
        env,
        &SelectedFile {
            note_id: note.event_id.clone(),
            note_hash: note.content_hash.as_str().into(),
            bytes: bytes.to_vec(),
        },
    )
    .unwrap();
    {
        let locked = backup::snapshot(root, BRAIN, &owner).unwrap();
        let original = locked
            .security_history
            .records()
            .iter()
            .find_map(|r| match r {
                AuthenticatedSecurityRecord::V1(n) if n.event_id == note.event_id => Some(n),
                _ => None,
            })
            .unwrap();
        let key =
            load_existing_event_signing_key(&profiles.store, env, root, owner.device_id()).unwrap();
        crate::canonical_writer::isolated_attachment::append_note_control(root, owner.device_id(), original, &key,
            "memory.forget", serde_json::json!({"note_event_id":note.event_id,"note_content_hash":note.content_hash,"prior_state_hash":note.content_hash})).unwrap();
    }
    let prepared = backup::prepare(root, &profiles.store, env, 1).unwrap();
    let checkpoint = prepared.selection.checkpoint(&owner).unwrap();
    let outer = prepared.encryption_input().unwrap();
    let mut archive = zip::ZipArchive::new(outer.reopen().unwrap()).unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let content = scratch.path().join("content.zip");
    std::io::copy(
        &mut archive.by_name("content.zip").unwrap(),
        &mut fs::File::create(&content).unwrap(),
    )
    .unwrap();
    let mut signature = Vec::new();
    archive
        .by_name("inventory.json")
        .unwrap()
        .read_to_end(&mut signature)
        .unwrap();
    let rebuilt =
        crate::backup_inventory::reconstruct_v2(&content, &signature, &checkpoint, scratch.path())
            .unwrap();
    let hidden = backup::snapshot(rebuilt.path(), BRAIN, &owner).unwrap();
    assert!(hidden.is_forgotten(&note.event_id));
    assert_eq!(hidden.admitted_blob_digests().count(), 0);
    drop(hidden);
    let blob = rebuilt
        .path()
        .join("canonical/blobs")
        .join(format!("{:x}", Sha256::digest(bytes)));
    assert_eq!(fs::read(&blob).unwrap(), bytes);
    fs::remove_file(blob).unwrap();
    assert_eq!(
        crate::backup_inventory::verify_reconstruction(rebuilt.path(), &checkpoint),
        Err(crate::backup_inventory::Error::Attachment)
    );
    let pending = root.join("canonical/purge-approvals");
    fs::create_dir_all(&pending).unwrap();
    fs::write(pending.join("unverified.json"), b"{}").unwrap();
    assert!(matches!(
        backup::prepare(root, &profiles.store, env, 2),
        Err(crate::backup_inventory::Error::PurgeHistory)
    ));
}

#[test]
fn scheduled_writer_relinquishes_before_a_peer_takes_the_same_target() {
    use crate::backup_scheduler::{journal::DeviceJobs, retention::VerifiedBackups};
    use crate::knowledge_materialization::backup::role::{Assignment, DeviceRole, Error};
    use crate::secure_store::{AccountLabel, FileBackedSecureStore, ServiceLabel};
    use std::num::NonZeroUsize;
    let pair = PairProfiles::new();
    pair.pair(0, [true, true], false);
    let env = SecureStoreEnvironment::Dev;
    let work = tempfile::tempdir().unwrap();
    let a_store = FileBackedSecureStore::open(work.path().join("a-state")).unwrap();
    let a = resolver(&pair).local_trust_anchor().unwrap();
    let b = LocalDeviceKeyResolver::load(&pair.profiles[1], BRAIN, &pair.store, env)
        .unwrap()
        .local_trust_anchor()
        .unwrap();
    // This disposable credential store belongs solely to the synthetic first device.
    let account = AccountLabel::device_event_signing_seed(a.device_id().as_str());
    a_store
        .put(
            ServiceLabel::device_credentials(env).as_str(),
            account.as_str(),
            pair.keys[0].as_bytes(),
        )
        .unwrap();
    let first = DeviceRole {
        store: &a_store,
        env,
        local: &a,
    };
    let second = DeviceRole {
        store: &pair.store,
        env,
        local: &b,
    };
    assert!(first.create(&pair.profiles[1], work.path()).is_err());
    let (target, _) = first.create(&pair.profiles[0], work.path()).unwrap();
    let original = first.reopen(&target).unwrap();
    first
        .verify_local_writer(&pair.profiles[0], &target)
        .unwrap();
    assert!(
        second
            .verify_local_writer(&pair.profiles[1], &target)
            .is_err()
    );
    let prior = target.join("backup-00000000000000000001.pgp");
    fs::write(&prior, b"prior synthetic backup").unwrap();
    let queue = DeviceJobs::new(&a_store, env, &a);
    queue
        .enqueue(&prior, Sha256::digest(fs::read(&prior).unwrap()).into(), 2)
        .unwrap();
    let catalog = VerifiedBackups {
        store: &a_store,
        environment: env,
        designated: &a,
        target: &target,
        writer_profile: &pair.profiles[0],
    };
    catalog.set_retention(&a, NonZeroUsize::new(1)).unwrap();
    assert!(second.create(&pair.profiles[1], &target).is_err());
    assert!(
        second
            .accept(&pair.profiles[1], &target, &original)
            .is_err()
    );
    let detached = work.path().join("detached-original");
    fs::rename(&target, &detached).unwrap();
    let (replacement, independent) = second.create(&pair.profiles[1], work.path()).unwrap();
    assert_ne!(target.file_name(), replacement.file_name());
    let separate_head = second.assigned_head(&replacement).unwrap();
    assert_ne!(first.assigned_head(&target).unwrap(), separate_head);
    let separate_prior = replacement.join("backup-00000000000000000001.pgp");
    fs::write(&separate_prior, b"independent target").unwrap();
    let successor_jobs = DeviceJobs::new(&pair.store, env, &b);
    successor_jobs
        .enqueue(
            &separate_prior,
            Sha256::digest(b"independent target").into(),
            2,
        )
        .unwrap();
    let (_keep, stop) = std::sync::mpsc::channel();
    assert_eq!(
        successor_jobs
            .resume(
                &b,
                &stop,
                || {},
                |out| fs::write(out, b"new target generation")
            )
            .unwrap(),
        Some(crate::backup_scheduler::Outcome::Published)
    );
    assert!(first.assigned_head(&replacement).is_err());
    fs::rename(&detached, &target).unwrap();
    assert_eq!(fs::read(&prior).unwrap(), b"prior synthetic backup");
    assert!(!target.join("backup-00000000000000000002.pgp").exists());
    let request = second
        .request(&pair.profiles[1], &target, &original)
        .unwrap();
    assert!(second.assigned_head(&target).is_err());
    let granted = first
        .release(&pair.profiles[0], &target, &original, &b, &request)
        .unwrap();
    let reopened = FileBackedSecureStore::open(a_store.root()).unwrap();
    let old = DeviceRole {
        store: &reopened,
        env,
        local: &a,
    };
    assert!(old.assigned_head(&target).is_err());
    assert_eq!(
        old.verify_local_writer(&pair.profiles[0], &target),
        Err(Error::Authority)
    );
    assert!(
        DeviceJobs::new(&reopened, env, &a)
            .resume(&a, &stop, || {}, |_| panic!("former writer published"))
            .is_err()
    );
    assert!(catalog.prune(&a).is_err());
    assert_eq!(fs::read(&prior).unwrap(), b"prior synthetic backup");
    let retry = old
        .release(
            &pair.profiles[0],
            &target,
            &old.reopen(&target).unwrap(),
            &b,
            &request,
        )
        .unwrap();
    assert_eq!(
        serde_json::to_vec(&granted).unwrap(),
        serde_json::to_vec(&retry).unwrap()
    );
    let mut damaged = serde_json::to_value(&granted).unwrap();
    let byte = damaged["grants"][0]["signature"][0].as_u64().unwrap();
    damaged["grants"][0]["signature"][0] = (byte ^ 1).into();
    let damaged: Assignment = serde_json::from_value(damaged).unwrap();
    assert_eq!(
        second.accept(&pair.profiles[1], &target, &damaged),
        Err(Error::Signature)
    );
    assert_eq!(
        second.accept(&pair.profiles[1], &replacement, &granted),
        Err(Error::Target)
    );
    assert!(
        second
            .accept(&pair.profiles[1], &target, &independent)
            .is_err()
    );
    second.accept(&pair.profiles[1], &target, &granted).unwrap();
    second.accept(&pair.profiles[1], &target, &retry).unwrap();
    second
        .verify_local_writer(&pair.profiles[1], &target)
        .unwrap();
    assert_ne!(second.assigned_head(&target).unwrap(), separate_head);
    successor_jobs
        .enqueue(&prior, Sha256::digest(fs::read(&prior).unwrap()).into(), 2)
        .unwrap();
    assert_eq!(
        successor_jobs
            .resume(&b, &stop, || {}, |out| fs::write(out, b"new writer"))
            .unwrap(),
        Some(crate::backup_scheduler::Outcome::Published)
    );
    assert_eq!(
        fs::read(target.join("backup-00000000000000000002.pgp")).unwrap(),
        b"new writer"
    );
    let granted = second.reopen(&target).unwrap();
    let return_request = old.request(&pair.profiles[0], &target, &granted).unwrap();
    let returned = second
        .release(&pair.profiles[1], &target, &granted, &a, &return_request)
        .unwrap();
    assert!(second.accept(&pair.profiles[1], &target, &granted).is_err());
    old.accept(&pair.profiles[0], &target, &returned).unwrap();
    assert_eq!(
        queue
            .resume(&a, &stop, || {}, |_| panic!("previous assignment reused"))
            .unwrap(),
        Some(crate::backup_scheduler::Outcome::NotDesignated)
    );
    assert_ne!(old.assigned_head(&target).unwrap(), separate_head);
    assert_eq!(second.assigned_head(&replacement).unwrap(), separate_head);
    let wiped = FakeSecureStore::new();
    assert!(
        DeviceRole {
            store: &wiped,
            env,
            local: &b
        }
        .accept(&pair.profiles[1], &target, &granted)
        .is_err()
    );
}

#[test]
#[ignore = "two hosted OS runners exchange their independently prepared files"]
fn exchange_native_backup_with_peer_os() {
    let work = PathBuf::from(std::env::var_os("CRYPTEX_M11_CROSS_ROOT").unwrap());
    let mode = std::env::var("CRYPTEX_M11_CROSS_ACTION").unwrap();
    let platform = std::env::var("CRYPTEX_M11_CROSS_PLATFORM").unwrap();
    assert!(matches!(platform.as_str(), "windows" | "macos"));
    let note_text = format!("portable M11 note from {platform}");
    let env = SecureStoreEnvironment::Dev;
    if mode == "prepare" {
        let source = tempfile::tempdir_in(work.join("private")).unwrap();
        let store = FakeSecureStore::new();
        let (device, _) = PairProfiles::enroll(source.path(), &store);
        append_memory_note(
            &store,
            env,
            AppendMemoryNoteArgs {
                runtime_root: source.path(),
                brain_id: BRAIN,
                device_id: &device,
                classification: Classification::Public,
                text: &note_text,
                event_id: None,
                config: &CanonicalWriterConfig::default(),
            },
        )
        .unwrap();
        let backup = super::super::backup::prepare(source.path(), &store, env, 1).unwrap();
        fs::write(
            work.join("public/selection.json"),
            serde_json::to_vec(&backup.selection).unwrap(),
        )
        .unwrap();
        fs::copy(
            backup.encryption_input().unwrap().path(),
            work.join("private/content.zip"),
        )
        .unwrap();
    } else {
        assert_eq!(mode, "restore");
        let selection: crate::backup_inventory::RecoverySelection =
            serde_json::from_slice(&fs::read(work.join("peer-public/selection.json")).unwrap())
                .unwrap();
        let anchor = selection.writer().unwrap();
        let checkpoint = selection.checkpoint(&anchor).unwrap();
        let archive = crate::backup_inventory::reconstruct_v2(
            &work.join("opened/content.zip"),
            &fs::read(work.join("opened/inventory.json")).unwrap(),
            &checkpoint,
            &work,
        )
        .unwrap();
        let snapshot =
            scan_verified_canonical_from_local_anchor_until(archive.path(), BRAIN, &anchor, None)
                .unwrap();
        let texts: Vec<_> = snapshot
            .verified_records()
            .iter()
            .filter_map(|record| {
                if let VersionedRecord::VerifiedV1(note) = record {
                    let cryptex_events::EventPayload::MemoryNote(payload) = &note.payload;
                    Some(payload.text.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(texts, ["synthetic original prefix", note_text.as_str()]);
        println!("m11_cross_inventory=passed source={platform} signed_notes=2");
    }
}

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
        let job_store =
            FileBackedSecureStore::open(std::env::var_os("CRYPTEX_M11_JOB_STORE").unwrap())
                .unwrap();
        let jobs = DeviceJobs::new(&job_store, env, &enrolled);
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
    let receipt_store = FileBackedSecureStore::open(work.path().join("restore-receipts")).unwrap();
    let (volume, _) = super::super::backup::role::DeviceRole {
        store: &receipt_store,
        env,
        local: &anchor,
    }
    .create(source.path(), work.path())
    .unwrap();
    let first = volume.join("backup-00000000000000000001.pgp");
    let next = volume.join("backup-00000000000000000002.pgp");
    run_archive("prepare", &[source.path(), &kit, &first, writer.path()]);
    let first_digest = Sha256::digest(fs::read(&first).unwrap());
    let jobs = DeviceJobs::new(&receipt_store, env, &anchor);
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
        .env("CRYPTEX_M11_JOB_CHILD", source.path()).env("CRYPTEX_M11_JOB_STORE", receipt_store.root()).status().unwrap()
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
    assert_eq!(fs::read_dir(&volume).unwrap().count(), 4); // two archives, target marker and writer lock
    assert_eq!(Sha256::digest(fs::read(&first).unwrap()), first_digest);
    let manual = volume.join("manual-export.pgp");
    run_archive("write-generation", &[source.path(), writer.path(), &manual]);
    let [first_publication, next_publication] = DeviceJobs::new(&receipt_store, env, &anchor)
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
    let catalog = crate::backup_scheduler::retention::VerifiedBackups {
        store: &receipt_store,
        environment: env,
        designated: &anchor,
        target: &volume,
        // The original profile was destroyed. This trial supplies the freshly
        // recovered complete history for the designated writer's pruning check.
        writer_profile: &work.path().join("restore-next"),
    };
    let before = catalog
        .verify(&anchor, &first_publication, |file| {
            let hits = restore_hits(file, "restore-first");
            assert_eq!(hits.len(), 1);
            assert_eq!(hits[0].text, "synthetic original prefix");
            assert_ne!(hits[0].event_id, receipt.event_id);
            Ok((hits, work.path().join("restore-first")))
        })
        .unwrap();
    assert!(
        catalog
            .verify(&anchor, &next_publication, |_| Err::<((), PathBuf), _>(
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
                Ok((hits, work.path().join("restore-next")))
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
            Ok(((), work.path().join("recheck-after-restart")))
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
    use ed25519_dalek::pkcs8::{DecodePrivateKey, EncodePrivateKey};
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
    let legacy = volume.join("legacy.pgp");
    let backup;
    let original_identity = fs::read(crate::identity_runtime::brain_identity_path(source)).unwrap();
    run_archive("prepare", &[source, &kit, &legacy]);
    drop(snapshot);
    let incomplete = source.join("recovery");
    fs::create_dir(&incomplete).unwrap();
    assert!(matches!(
        super::super::backup::prepare(source, &profiles.store, SecureStoreEnvironment::Dev, 1),
        Err(crate::backup_inventory::Error::RecoveryHistory)
    ));
    fs::remove_dir(incomplete).unwrap();
    {
        let prepared =
            super::super::backup::prepare(source, &profiles.store, SecureStoreEnvironment::Dev, 1)
                .unwrap();
        fs::write(
            kit.join("selected-backup.json"),
            serde_json::to_vec(&prepared.selection).unwrap(),
        )
        .unwrap();
        let recipient = kit.join("recovery.pub");
        if let Some(directory) = std::env::var_os("CRYPTEX_M11_TEST_INSTALLED_CODEC") {
            let directory = Path::new(&directory);
            let public = fs::read(recipient).unwrap();
            // Exercise substitution without altering the installed application.
            let scratch = tempfile::tempdir().unwrap();
            let node = if cfg!(windows) { "node.exe" } else { "node" };
            fs::copy(directory.join(node), scratch.path().join(node)).unwrap();
            fs::copy(
                directory.join("openpgp.min.cjs"),
                scratch.path().join("openpgp.min.cjs"),
            )
            .unwrap();
            fs::write(
                scratch.path().join("encrypt.cjs"),
                b"process.stdout.write('unverified');",
            )
            .unwrap();
            assert!(matches!(
                prepared.encrypt(scratch.path(), &public),
                Err(crate::backup_inventory::Error::CodecIntegrity)
            ));
            backup = prepared.export_manual(directory, &public, &volume).unwrap();
        } else {
            backup = volume.join("backup.pgp");
            let inner = prepared.encryption_input().unwrap();
            run_archive("seal-inventory", &[&recipient, inner.path(), &backup]);
        }
    }
    let sealed_kit = work.path().join("offline-kit.pgp");
    let unlock = tempfile::tempdir().unwrap();
    let password = unlock.path().join("owner-unlock");
    let mut random = [0u8; 32];
    getrandom::getrandom(&mut random).unwrap();
    fs::write(
        &password,
        random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
    )
    .unwrap();
    random.fill(0);
    fs::write(
        kit.join("bootstrap.pk8"),
        offline.to_pkcs8_der().unwrap().as_bytes(),
    )
    .unwrap();
    run_archive("seal-kit", &[&kit, &password, &sealed_kit]);
    drop(offline);
    fs::remove_dir_all(&kit).unwrap();
    drop(instance);
    drop(anchor);
    let old_profiles = profiles.profiles.clone();
    drop(profiles); // Drops both original private keys and the old secure store.
    assert!(old_profiles.iter().all(|root| !root.exists()));
    let wrong = unlock.path().join("wrong");
    fs::write(&wrong, "00".repeat(32)).unwrap();
    assert!(archive_operation("open-kit", &[&sealed_kit, &wrong, &kit]).is_err());
    assert!(!kit.try_exists().unwrap());
    let damaged_kit = work.path().join("damaged-kit.pgp");
    let mut damaged = fs::read(&sealed_kit).unwrap();
    *damaged.last_mut().unwrap() ^= 1;
    fs::write(&damaged_kit, damaged).unwrap();
    assert!(archive_operation("open-kit", &[&damaged_kit, &password, &kit]).is_err());
    assert!(!kit.try_exists().unwrap());
    run_archive("open-kit", &[&sealed_kit, &password, &kit]);
    if let Some(desktop) = std::env::var_os("CRYPTEX_M11_INSTALLED_DESKTOP") {
        run_archive(
            "kit-public-recipient",
            &[&sealed_kit, &password, &work.path().join("kit-public")],
        );
        installed_export::check(work.path(), Path::new(&desktop), &kit, &password);
        assert!(!work.path().join("clean-replacement").try_exists().unwrap());
        let check_volume = work.path().join("first-check");
        fs::create_dir(&check_volume).unwrap();
        let selected = check_volume.join("backup.pgp");
        fs::copy(&backup, &selected).unwrap();
        for mode in ["first-device-check", "first-device-check-damaged"] {
            if mode.ends_with("-damaged") {
                fs::write(&selected, b"truncated").unwrap();
            }
            separate_backups::launched(Path::new(&desktop), work.path(), mode, &password, "");
            let blank = work.path().join("clean-replacement");
            assert!(blank.join("daemon.lock/reader-m9.pid").is_file());
            for leaf in ["identity", "canonical", "recovery"] {
                assert!(!blank.join(leaf).try_exists().unwrap());
            }
        }
        println!(
            "m11_blank_recovery_check=passed authenticated_kit=true damaged=denied profile_writes=none"
        );
        first_device::blank_replacement(
            Path::new(&desktop),
            work.path(),
            first_device::OfflineInputs {
                sealed_kit: &sealed_kit,
                unlock: &password,
                backup: &backup,
            },
            &selected,
        );
    }

    let selection: crate::backup_inventory::RecoverySelection =
        serde_json::from_slice(&fs::read(kit.join("selected-backup.json")).unwrap()).unwrap();
    let anchor = selection.writer().unwrap();
    let checkpoint = selection.checkpoint(&anchor).unwrap();
    let offline =
        SigningKey::from_pkcs8_der(&fs::read(kit.join("bootstrap.pk8")).unwrap()).unwrap();
    assert_eq!(offline.verifying_key(), pinned_recovery_key);
    fs::create_dir_all(restored.parent().unwrap()).unwrap();
    {
        let staging = tempfile::tempdir().unwrap();
        let opened = staging.path().join("legacy");
        run_archive("open-payload", &[&kit, &legacy, &opened]);
        assert!(!opened.join("inventory.json").exists());
        let legacy_profile = staging.path().join("legacy-profile");
        run_archive(
            "unpack-checked",
            &[
                &opened.join("content.zip"),
                &legacy_profile,
                &kit.join("brain-identity.sha256"),
            ],
        );
        crate::backup_inventory::verify_reconstruction(&legacy_profile, &checkpoint).unwrap();
    }
    let staging = tempfile::tempdir_in(restored.parent().unwrap()).unwrap();
    let opened = staging.path().join("opened");
    run_archive("open-payload", &[&kit, &backup, &opened]);
    let payload = opened.join("content.zip");
    let signed = fs::read(opened.join("inventory.json")).unwrap();
    use crate::backup_inventory::{Checkpoint, verify};
    let damaged_payload = staging.path().join("altered-content.zip");
    fs::write(&damaged_payload, b"incomplete archive").unwrap();
    assert_eq!(
        verify(&damaged_payload, &signed, &checkpoint),
        Err(crate::backup_inventory::Error::Checkpoint)
    );
    assert!(
        verify(
            &payload,
            &signed,
            &Checkpoint {
                generation: 2,
                ..checkpoint
            }
        )
        .is_err()
    );
    let mut wrong_signature: serde_json::Value = serde_json::from_slice(&signed).unwrap();
    wrong_signature["signature"] = "00".repeat(64).into();
    assert!(
        verify(
            &payload,
            &serde_json_canonicalizer::to_vec(&wrong_signature).unwrap(),
            &checkpoint
        )
        .is_err()
    );
    assert!(!restored.exists());
    let sandbox =
        crate::backup_inventory::reconstruct_v2(&payload, &signed, &checkpoint, staging.path())
            .unwrap();
    let staged_profile = sandbox.path();
    let blob = staged_profile
        .join("canonical/blobs")
        .join(format!("{:x}", Sha256::digest(bytes)));
    let parked_blob = staging.path().join("held-attachment");
    fs::rename(&blob, &parked_blob).unwrap();
    assert_eq!(
        crate::backup_inventory::verify_reconstruction(staged_profile, &checkpoint),
        Err(crate::backup_inventory::Error::Attachment)
    );
    assert!(!restored.exists());
    fs::rename(&parked_blob, &blob).unwrap();
    crate::backup_inventory::verify_reconstruction(staged_profile, &checkpoint).unwrap();
    fs::rename(staged_profile, &restored).unwrap();
    println!(
        "m11_backup_inventory=passed daemon_signed=true public_only_encryption=true legacy=restored replay=matched"
    );
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
        first_device::OfflineInputs {
            sealed_kit: &sealed_kit,
            unlock: &password,
            backup: &parked.join(backup.file_name().unwrap()),
        },
    );
    let anchor = keys.local_trust_anchor().unwrap();
    println!(
        "m11_portable_kit=passed source=lost wrong_unlock=denied decrypt=restored bootstrap=authorized"
    );
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
    let pair = std::env::var_os("CRYPTEX_M11_INSTALLED_DESKTOP").map(|desktop| {
        separate_backups::export(
            work.path(),
            Path::new(&desktop),
            &backup,
            &sealed_kit,
            &password,
        )
    });
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
    drop(state);
    drop(db);
    if let Some(pair) = pair {
        pair.restore_after_source_loss(work.path(), &sealed_kit, &password, &note.event_id);
    }
    println!(
        "m11_restarted_search=passed single_fts=true stale_index_held=both damaged_receipt=held"
    );
}
