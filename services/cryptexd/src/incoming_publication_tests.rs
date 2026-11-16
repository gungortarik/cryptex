use super::*;
use cryptex_core::DeviceId;
use cryptex_events::envelope::Classification;
use std::fs;

#[test]
#[cfg(windows)]
fn live_recovery_initializes_newly_approved_peer_stream_before_rebuild() {
    let fixture = Fixture::new();
    let peer = DeviceId::new(fixture.report.receiver_device.clone()).unwrap();
    let peer_path = cryptex_storage::canonical_events_device_dir(&fixture.source, &peer);
    assert!(
        !peer_path.exists(),
        "fixture must not pre-create the approved peer slot"
    );
    let before = event_hashes(&fixture.source);
    let mut instance = crate::instance::try_acquire_instance(&fixture.source).unwrap();
    recover_live_with_instance(&fixture.source, &mut instance, &fixture.store, ENV).unwrap();
    assert!(peer_path.is_dir());
    assert_eq!(event_hashes(&fixture.source), before);
    assert!(instance.admission().enter(&fixture.source).is_ok());
    // Repeating recovery must not append peer data or rewrite existing events.
    recover_live_with_instance(&fixture.source, &mut instance, &fixture.store, ENV).unwrap();
    assert_eq!(event_hashes(&fixture.source), before);
}

fn append_offline_note(fixture: &Fixture, root: &Path, device: &str, text: &str) -> PathBuf {
    let device = DeviceId::new(device).unwrap();
    let appended = crate::canonical_writer::append_memory_note(
        &fixture.store,
        ENV,
        crate::canonical_writer::AppendMemoryNoteArgs {
            runtime_root: root,
            brain_id: &fixture.report.brain_id,
            device_id: &device,
            classification: Classification::Public,
            text,
            event_id: None,
            config: &crate::canonical_writer::CanonicalWriterConfig {
                seal_event_threshold: 1,
            },
        },
    )
    .unwrap();
    assert!(appended.sealed);
    PathBuf::from(device.as_filename_stem()).join(format!("{:020}.seg", appended.segment_index))
}

fn verified_note_texts(fixture: &Fixture, root: &Path) -> std::collections::BTreeSet<String> {
    let db = cryptex_projection::ProjectionDb::open_existing_read_only(root).unwrap();
    let mut resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
        root,
        &fixture.report.brain_id,
        &fixture.store,
        ENV,
    )
    .unwrap();
    let safety = crate::knowledge_materialization::evaluate_retrieval_safety(
        root,
        &fixture.report.brain_id,
        &[],
        &mut resolver,
        &db,
    );
    assert!(safety.allows_content(), "{:?}", safety.hold_reason());
    cryptex_projection::search::search_events(db.connection(), &Default::default())
        .unwrap()
        .into_iter()
        .map(|hit| hit.text)
        .collect()
}

#[test]
fn offline_both_peers_write_then_reconcile_without_a_master_or_duplicate_segments() {
    let fixture = Fixture::new();
    // No transport process: each daemon writes its own stream independently.
    let source_tail = append_offline_note(
        &fixture,
        &fixture.source,
        &fixture.report.source_device,
        "Synthetic offline source note.",
    );
    let receiver_tail = append_offline_note(
        &fixture,
        &fixture.receiver,
        &fixture.report.receiver_device,
        "Synthetic offline receiver note.",
    );
    // prepare() initializes the receiving side only. Initialize the other
    // installation's derived state from its own signed history as well.
    {
        let _slots = crate::canonical_writer::acquire_canonical_write_lock_set(
            &fixture.source,
            &[
                DeviceId::new(fixture.report.source_device.clone()).unwrap(),
                DeviceId::new(fixture.report.receiver_device.clone()).unwrap(),
            ],
        )
        .unwrap();
    }
    for root in [&fixture.source, &fixture.receiver] {
        let mut instance = crate::instance::try_acquire_instance(root).unwrap();
        recover_live_with_instance(root, &mut instance, &fixture.store, ENV).unwrap();
    }
    let source_before = event_hashes(&fixture.source);
    let receiver_before = event_hashes(&fixture.receiver);
    copy_incoming(&fixture.receiver, &fixture.source, &receiver_tail);
    copy_incoming(&fixture.source, &fixture.receiver, &source_tail);
    // Source receives the receiver's complete sealed prefix first. Receiver
    // receives its peer's segments in reverse hint order in one bounded batch.
    let a = publish_incoming(
        &fixture.source,
        std::slice::from_ref(&receiver_tail),
        &fixture.store,
        ENV,
    )
    .unwrap();
    let mut paths = vec![source_tail];
    paths.extend(fixture.paths.clone());
    let b = publish_incoming(&fixture.receiver, &paths, &fixture.store, ENV).unwrap();
    assert_eq!(a.published_segments, 1);
    assert_eq!(b.published_segments, 2);
    let union_a = event_hashes(&fixture.source);
    let union_b = event_hashes(&fixture.receiver);
    assert_eq!(union_a, union_b);
    let notes = verified_note_texts(&fixture, &fixture.source);
    assert_eq!(notes, verified_note_texts(&fixture, &fixture.receiver));
    assert!(notes.contains("Synthetic offline source note."));
    assert!(notes.contains("Synthetic offline receiver note."));
    for prior in source_before.iter().chain(&receiver_before) {
        assert!(
            union_a.contains(prior),
            "pre-existing immutable segment changed"
        );
    }
    assert_eq!(
        publish_incoming(&fixture.source, &[receiver_tail], &fixture.store, ENV)
            .unwrap()
            .published_segments,
        0
    );
    assert_eq!(
        publish_incoming(&fixture.receiver, &paths, &fixture.store, ENV)
            .unwrap()
            .published_segments,
        0
    );
    assert_eq!(event_hashes(&fixture.source), union_a);
    assert_eq!(event_hashes(&fixture.receiver), union_b);
}

#[test]
fn offline_restricted_peer_tail_is_refused_after_owner_confirmed_cutoff() {
    use crate::bootstrap_journal::{BootstrapJournalStore, RestrictionReason};
    for reason in [RestrictionReason::Retired, RestrictionReason::Compromised] {
        let fixture = Fixture::new();
        fixture.publish().unwrap();
        let issuer = DeviceId::new(fixture.report.receiver_device.clone()).unwrap();
        let target = DeviceId::new(fixture.report.source_device.clone()).unwrap();
        let key = crate::event_signing_key::load_existing_event_signing_key(
            &fixture.store,
            ENV,
            &fixture.receiver,
            &issuer,
        )
        .unwrap();
        let policy = crate::knowledge_materialization::scan_verified_trust_policy(
            &fixture.receiver,
            &fixture.report.brain_id,
            &issuer,
            &cryptex_events::SigningKeyId::parse(&key.public.signing_key_id).unwrap(),
            &key.signing_key.verifying_key().to_bytes(),
            &[issuer.clone(), target.clone()],
        )
        .unwrap();
        let transaction = "ac".repeat(32);
        let snapshot = policy
            .device_restriction_approval_snapshot(transaction.clone(), &target, reason, 3)
            .unwrap();
        drop(policy);
        let journal = BootstrapJournalStore::new(&fixture.receiver).unwrap();
        journal
            .begin_owner_operation(snapshot.clone(), "99999999-9999-4999-8999-999999999997")
            .unwrap();
        journal
            .confirm_owner_operation(&transaction, &snapshot.digest().unwrap())
            .unwrap();
        crate::knowledge_materialization::append_confirmed_owner_security_operation(
            &fixture.receiver,
            &fixture.report.brain_id,
            &fixture.store,
            ENV,
            &transaction,
        )
        .unwrap();
        // Source has not received the restriction and can still create local
        // bytes while offline. Receiver must not mistake its signature for authority.
        let tail = append_offline_note(
            &fixture,
            &fixture.source,
            &fixture.report.source_device,
            "Synthetic stale restricted tail.",
        );
        copy_incoming(&fixture.source, &fixture.receiver, &tail);
        let before = event_hashes(&fixture.receiver);
        let incoming = fixture.receiver.join("transport/incoming").join(&tail);
        let bytes = fs::read(&incoming).unwrap();
        let result = publish_incoming(
            &fixture.receiver,
            std::slice::from_ref(&tail),
            &fixture.store,
            ENV,
        );
        assert!(
            matches!(result, Err(IncomingPublicationError::Rejected(_))),
            "{reason:?}: {result:?}"
        );
        assert_eq!(event_hashes(&fixture.receiver), before);
        assert_eq!(fs::read(&incoming).unwrap(), bytes);
        assert!(
            !fixture
                .receiver
                .join("canonical/events")
                .join(&tail)
                .exists()
        );
        let notes = verified_note_texts(&fixture, &fixture.receiver);
        assert!(notes.contains(crate::trusted_import_probe::NOTE));
        assert!(!notes.contains("Synthetic stale restricted tail."));
        fixture.assert_clean();
    }
}

#[test]
fn local_stream_replays_are_idempotent_but_new_signed_tails_require_restore() {
    fn clone_tree(source: &Path, target: &Path) {
        fs::create_dir_all(target).unwrap();
        for entry in fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                clone_tree(&entry.path(), &target.join(entry.file_name()));
            } else {
                assert!(entry.file_type().unwrap().is_file());
                fs::copy(entry.path(), target.join(entry.file_name())).unwrap();
            }
        }
    }
    let fixture = Fixture::new();
    let local = DeviceId::new(fixture.report.receiver_device.clone()).unwrap();
    let config = crate::canonical_writer::CanonicalWriterConfig {
        seal_event_threshold: 1,
    };
    let append = |root: &Path| {
        crate::canonical_writer::append_memory_note(
            &fixture.store,
            ENV,
            crate::canonical_writer::AppendMemoryNoteArgs {
                runtime_root: root,
                brain_id: &fixture.report.brain_id,
                device_id: &local,
                classification: Classification::Public,
                text: "Synthetic local restore tail.",
                event_id: None,
                config: &config,
            },
        )
        .unwrap()
    };
    append(&fixture.receiver);
    let mut instance = crate::instance::try_acquire_instance(&fixture.receiver).unwrap();
    recover_live_with_instance(&fixture.receiver, &mut instance, &fixture.store, ENV).unwrap();
    let replay = PathBuf::from(local.as_filename_stem()).join("00000000000000000000.seg");
    copy_incoming(&fixture.receiver, &fixture.receiver, &replay);
    let report = publish_incoming_with_instance(
        &fixture.receiver,
        &[replay],
        &mut instance,
        &fixture.store,
        ENV,
    )
    .unwrap();
    assert_eq!(
        (report.published_segments, report.identical_segments),
        (0, 1)
    );
    drop(instance);
    let restored = crate::test_temp::exclusive_temp_dir("incoming-local-restore-copy");
    clone_tree(&fixture.receiver, &restored);
    append(&restored);
    let tail = PathBuf::from(local.as_filename_stem()).join("00000000000000000001.seg");
    copy_incoming(&restored, &fixture.receiver, &tail);
    let before = event_hashes(&fixture.receiver);
    let sequence =
        crate::sequence_runtime::last_issued_device_sequence(&fixture.receiver, &local).unwrap();
    let result = publish_incoming(&fixture.receiver, &[tail], &fixture.store, ENV);
    assert!(
        matches!(&result, Err(IncomingPublicationError::Rejected(ImportStagingError::Invalid(message)))
        if message.contains("local stream")),
        "{result:?}"
    );
    assert_eq!(event_hashes(&fixture.receiver), before);
    assert_eq!(
        crate::sequence_runtime::last_issued_device_sequence(&fixture.receiver, &local).unwrap(),
        sequence
    );
    fs::remove_dir_all(restored).unwrap();
}

#[test]
fn rejects_signed_wrong_target_key_restriction_before_publication() {
    use cryptex_events::{ContentHashHex, DeviceSeq, SigningKeyId};
    use std::io::Write;

    let mut fixture = Fixture::new();
    let next = fixture.append_next();
    let prior_bytes = fs::read(fixture.source.join("canonical/events").join(&next)).unwrap();
    let prior: serde_json::Value =
        serde_json::from_slice(&prior_bytes[cryptex_events::FRAME_HEADER_BYTES..]).unwrap();
    let previous = ContentHashHex::parse(prior["content_hash"].as_str().unwrap()).unwrap();
    let sequence: u64 = prior["device_seq"].as_str().unwrap().parse().unwrap();
    let issuer = DeviceId::new(fixture.report.source_device.clone()).unwrap();
    let target = DeviceId::new(fixture.report.receiver_device.clone()).unwrap();
    let issuer_key = crate::event_signing_key::load_existing_event_signing_key(
        &fixture.store,
        ENV,
        &fixture.source,
        &issuer,
    )
    .unwrap();
    let unrelated_key = ed25519_dalek::SigningKey::from_bytes(&[0x52; 32]);
    let body = cryptex_events::security_v3::sign_device_restricted_event(
        "99999999-9999-4999-8999-999999999999",
        &fixture.report.brain_id,
        &issuer,
        DeviceSeq::from_u64(sequence + 1).unwrap(),
        "2026-11-08T00:00:03Z",
        &SigningKeyId::parse(&issuer_key.public.signing_key_id).unwrap(),
        &previous,
        &"af".repeat(32),
        &target,
        &cryptex_events::signing_key_id_from_verifying_key(&unrelated_key.verifying_key()).unwrap(),
        &unrelated_key.verifying_key().to_bytes(),
        cryptex_events::security_v3::RestrictionReasonV3::Retired,
        0,
        &ContentHashHex::zero(),
        &issuer_key.signing_key,
    )
    .unwrap();
    let path = PathBuf::from(issuer.as_filename_stem()).join("00000000000000000002.seg");
    let incoming = fixture.receiver.join("transport/incoming").join(&path);
    let mut file = fs::File::create(&incoming).unwrap();
    file.write_all(&cryptex_events::encode_frame_header(body.len() as u32).unwrap())
        .unwrap();
    file.write_all(&body).unwrap();
    drop(file);
    fixture.paths.extend([next, path]);
    let before = event_hashes(&fixture.receiver);
    let incoming_before = fs::read(&incoming).unwrap();
    assert!(matches!(
        fixture.publish(),
        Err(IncomingPublicationError::Rejected(_))
    ));
    assert_eq!(event_hashes(&fixture.receiver), before);
    assert_eq!(fs::read(incoming).unwrap(), incoming_before);
    fixture.assert_clean();
}

#[test]
fn import_policy_proofs_cannot_cross_runtime_boundaries() {
    fn clone_plain_tree(source: &Path, target: &Path) {
        fs::create_dir_all(target).unwrap();
        for entry in fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let destination = target.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                clone_plain_tree(&entry.path(), &destination);
            } else {
                assert!(entry.file_type().unwrap().is_file());
                fs::copy(entry.path(), destination).unwrap();
            }
        }
    }

    let fixture = Fixture::new();
    let other = crate::test_temp::exclusive_temp_dir("incoming-foreign-policy");
    clone_plain_tree(&fixture.receiver, &other);
    let before = event_hashes(&other);
    let mut foreign_session = ImportStagingSession::begin(&other).unwrap();
    foreign_session
        .stage_batch_from_transport_paths(&fixture.paths)
        .unwrap();
    foreign_session.release_staging_slot().unwrap();
    let devices = vec![DeviceId::new(fixture.report.source_device.clone()).unwrap()];
    let db = cryptex_projection::ProjectionDb::open_existing_read_only(&fixture.receiver).unwrap();
    let safety = || {
        let mut resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
            &fixture.receiver,
            &fixture.report.brain_id,
            &fixture.store,
            ENV,
        )
        .unwrap();
        crate::knowledge_materialization::evaluate_import_retrieval_safety(
            &fixture.receiver,
            &fixture.report.brain_id,
            &devices,
            &mut resolver,
            &db,
        )
    };
    let first = safety();
    assert!(first.allows_content());
    assert!(
        first
            .verify_import_batch(&foreign_session, &fixture.report.brain_id)
            .is_err(),
        "a source runtime's locks must not authorize another runtime's import"
    );

    // Obtain a legitimate source-runtime proof, then try to reuse its exact
    // verified bytes at the final publication boundary in the other runtime.
    let mut source_session = ImportStagingSession::begin(&fixture.receiver).unwrap();
    source_session
        .stage_batch_from_transport_paths(&fixture.paths)
        .unwrap();
    source_session.release_staging_slot().unwrap();
    let policy = safety()
        .verify_import_batch(&source_session, &fixture.report.brain_id)
        .unwrap();
    foreign_session
        .prepare_verified_manifest(policy.verified())
        .unwrap();
    let destination_lock =
        crate::canonical_writer::acquire_canonical_inventory_lock(&other).unwrap();
    assert!(
        policy
            .validate_publication_preconditions(&foreign_session)
            .is_err()
    );
    assert!(foreign_session.publish_verified_batch(&policy).is_err());
    assert_eq!(event_hashes(&other), before);
    assert!(
        fs::read_dir(other.join("canonical/import-journal"))
            .unwrap()
            .all(|entry| {
                !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".committed.json")
            })
    );
    drop(destination_lock);
    drop(policy);
    drop(db);
    source_session.discard().unwrap();
    drop(foreign_session);
    fs::remove_dir_all(other).unwrap();
}

const ENV: SecureStoreEnvironment = SecureStoreEnvironment::Dev;
const PROBE: &str = "44444444-4444-4444-8444-444444444444";

#[cfg(target_os = "macos")]
#[test]
fn original_mac_policy_and_lock_capabilities_refuse_replaced_canonical_subtrees() {
    fn copy_directory(source: &Path, target: &Path) {
        fs::create_dir(target).unwrap();
        for entry in fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let destination = target.join(entry.file_name());
            if entry.path().is_dir() {
                copy_directory(&entry.path(), &destination);
            } else {
                fs::copy(entry.path(), destination).unwrap();
            }
        }
    }
    for replace_device_only in [false, true] {
        let fixture = Fixture::new();
        let mut session = ImportStagingSession::begin(&fixture.receiver).unwrap();
        let devices = session
            .stage_batch_from_transport_paths(&fixture.paths)
            .unwrap()
            .iter()
            .map(|c| c.device_id.clone())
            .collect::<Vec<_>>();
        session.release_staging_slot().unwrap();
        let db =
            cryptex_projection::ProjectionDb::open_existing_read_only(&fixture.receiver).unwrap();
        let mut resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
            &fixture.receiver,
            &fixture.report.brain_id,
            &fixture.store,
            ENV,
        )
        .unwrap();
        let safety = crate::knowledge_materialization::evaluate_import_retrieval_safety(
            &fixture.receiver,
            &fixture.report.brain_id,
            &devices,
            &mut resolver,
            &db,
        );
        let proof = safety
            .verify_import_batch(&session, &fixture.report.brain_id)
            .unwrap();
        session.prepare_verified_manifest(proof.verified()).unwrap();
        let local = DeviceId::new(fixture.report.receiver_device.clone()).unwrap();
        let selected = if replace_device_only {
            cryptex_storage::layout::canonical_events_device_dir(&fixture.receiver, &local)
        } else {
            fixture.receiver.join("canonical/events")
        };
        let moved = fixture.receiver.join("original-locked-subtree");
        let before = event_hashes(&fixture.receiver);
        fs::rename(&selected, &moved).unwrap();
        // Valid canonical layout/bytes, fresh object identities. A generic
        // malformed-inventory error cannot establish this regression.
        copy_directory(&moved, &selected);
        let original_entries = fs::read_dir(&moved).unwrap().count();
        assert!(
            matches!(session.publish_verified_batch(&proof), Err(ImportStagingError::RecoveryConflict(message)) if message.contains("namespace"))
        );
        assert_eq!(event_hashes(&fixture.receiver), before);
        assert_eq!(fs::read_dir(&moved).unwrap().count(), original_entries);
        assert_eq!(
            fs::read_dir(fixture.receiver.join("canonical/import-staging"))
                .unwrap()
                .count(),
            1
        );
        drop(proof);
        drop(session);
    }
}

#[cfg(target_os = "macos")]
#[test]
fn mac_prepared_recovery_and_committed_cleanup_keep_the_original_namespace() {
    use crate::import_staging::namespace_fault::{self, Point};
    use std::process::{Command, Stdio};
    use std::time::Duration;
    fn copy_directory(source: &Path, target: &Path) {
        fs::create_dir(target).unwrap();
        for entry in fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let destination = target.join(entry.file_name());
            if entry.path().is_dir() {
                copy_directory(&entry.path(), &destination);
            } else {
                fs::copy(entry.path(), destination).unwrap();
            }
        }
    }
    fn evidence(root: &Path) -> std::collections::BTreeMap<std::ffi::OsString, Vec<u8>> {
        fs::read_dir(root.join("canonical/import-journal"))
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                (entry.file_name(), fs::read(entry.path()).unwrap())
            })
            .collect()
    }
    for (boundary, point) in [
        ("m8_after_prepared_manifest_sync", Point::PreparedAbort),
        ("m8_after_canonical_segment_move_0", Point::RecoveryPolicy),
        ("m8_cleanup_receipt_final_synced", Point::CleanupValidated),
    ] {
        let mut fixture = Fixture::new();
        let next = fixture.append_next();
        fixture.paths.push(next);
        let seed = fixture
            .store
            .get(
                crate::ServiceLabel::device_credentials(ENV).as_str(),
                crate::AccountLabel::device_event_signing_seed(&fixture.report.receiver_device)
                    .as_str(),
            )
            .unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "incoming_publication::tests::publication_interrupt_worker",
                "--nocapture",
            ])
            .env("CRYPTEX_TEST_PUBLICATION_ROOT", &fixture.receiver)
            .env(crate::test_interrupt::INTERRUPT_ENV, boundary)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        serde_json::to_writer(
            child.stdin.take().unwrap(),
            &(&fixture.report.receiver_device, seed, &fixture.paths),
        )
        .unwrap();
        let status =
            crate::test_interrupt::wait_child_bounded(&mut child, Duration::from_secs(30)).unwrap();
        assert_eq!(
            status.code(),
            Some(crate::test_interrupt::INTERRUPT_EXIT_CODE)
        );
        let before = event_hashes(&fixture.receiver);
        let original_evidence = evidence(&fixture.receiver);
        let selected = fixture.receiver.join("canonical/events");
        let moved = fixture.receiver.join("original-recovery-events");
        let _hook = namespace_fault::install(point, move || {
            fs::rename(&selected, &moved).unwrap();
            copy_directory(&moved, &selected);
        });
        let mut instance = crate::instance::try_acquire_instance(&fixture.receiver).unwrap();
        assert!(
            matches!(recover_live_with_instance(&fixture.receiver, &mut instance, &fixture.store, ENV),
            Err(IncomingPublicationError::RecoveryRequired(ImportStagingError::RecoveryConflict(message))) if message.contains("namespace"))
        );
        assert!(instance.admission().enter(&fixture.receiver).is_err());
        assert_eq!(event_hashes(&fixture.receiver), before);
        assert_eq!(evidence(&fixture.receiver), original_evidence);
        assert_eq!(
            fs::read_dir(fixture.receiver.join("canonical/import-staging"))
                .unwrap()
                .count(),
            1
        );
    }
}

struct Fixture {
    source: PathBuf,
    receiver: PathBuf,
    negative: PathBuf,
    store: crate::FakeSecureStore,
    report: crate::trusted_import_probe::PreparedTrustedProbe,
    paths: Vec<PathBuf>,
}

impl Fixture {
    fn new() -> Self {
        let (source, receiver, negative, store) = crate::trusted_import_probe::tests::pair();
        let report =
            crate::trusted_import_probe::prepare(&source, &receiver, &negative, PROBE, &store)
                .unwrap();
        let paths = report
            .segments
            .iter()
            .map(|s| PathBuf::from(&s.relative_path))
            .collect::<Vec<_>>();
        for path in &paths {
            copy_incoming(&source, &receiver, path);
        }
        Self {
            source,
            receiver,
            negative,
            store,
            report,
            paths,
        }
    }

    fn publish(&self) -> Result<IncomingPublicationReport, IncomingPublicationError> {
        publish_incoming(&self.receiver, &self.paths, &self.store, ENV)
    }

    fn append_next(&self) -> PathBuf {
        let device = DeviceId::new(self.report.source_device.clone()).unwrap();
        crate::canonical_writer::append_memory_note(
            &self.store,
            ENV,
            crate::canonical_writer::AppendMemoryNoteArgs {
                runtime_root: &self.source,
                brain_id: &self.report.brain_id,
                device_id: &device,
                classification: Classification::Public,
                text: "Synthetic second persistent import note.",
                event_id: None,
                config: &crate::canonical_writer::CanonicalWriterConfig {
                    seal_event_threshold: 1,
                },
            },
        )
        .unwrap();
        let path = PathBuf::from(device.as_filename_stem()).join("00000000000000000001.seg");
        copy_incoming(&self.source, &self.receiver, &path);
        path
    }

    fn assert_clean(&self) {
        assert_eq!(
            fs::read_dir(self.receiver.join("canonical/import-staging"))
                .unwrap()
                .count(),
            0
        );
        assert_eq!(
            fs::read_dir(self.receiver.join("canonical/import-journal"))
                .unwrap()
                .count(),
            1
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for root in [&self.source, &self.receiver, &self.negative] {
            fs::remove_dir_all(root).unwrap();
        }
    }
}

fn copy_incoming(source: &Path, receiver: &Path, relative: &Path) {
    let target = receiver.join("transport/incoming").join(relative);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::copy(source.join("canonical/events").join(relative), target).unwrap();
}

fn event_hashes(root: &Path) -> Vec<(PathBuf, String)> {
    use sha2::{Digest, Sha256};
    let mut hashes = Vec::new();
    for device in fs::read_dir(root.join("canonical/events")).unwrap() {
        let device = device.unwrap();
        if !device.file_type().unwrap().is_dir() {
            continue;
        }
        for file in fs::read_dir(device.path()).unwrap() {
            let path = file.unwrap().path();
            if matches!(
                path.extension().and_then(|s| s.to_str()),
                Some("seg" | "open")
            ) {
                hashes.push((
                    path.strip_prefix(root).unwrap().to_owned(),
                    format!("{:x}", Sha256::digest(fs::read(path).unwrap())),
                ));
            }
        }
    }
    hashes.sort();
    hashes
}

#[test]
fn publishes_peer_batch_persists_reopens_and_replays_without_rewriting() {
    let fixture = Fixture::new();
    let source_before = event_hashes(&fixture.source);
    let incoming_before = fs::read(
        fixture
            .receiver
            .join("transport/incoming")
            .join(&fixture.paths[0]),
    )
    .unwrap();
    let report = fixture.publish().unwrap();
    assert_eq!(
        (
            report.published_segments,
            report.published_events,
            report.identical_segments
        ),
        (1, 3, 0)
    );
    assert_eq!(
        fs::read(
            fixture
                .receiver
                .join("canonical/events")
                .join(&fixture.paths[0])
        )
        .unwrap(),
        incoming_before
    );
    fixture.assert_clean();
    // Opening a new context proves the appended canonical bytes are admitted
    // by the local anchor and that derived M3/M4 was refreshed for that union.
    let db = cryptex_projection::ProjectionDb::open_existing_read_only(&fixture.receiver).unwrap();
    let mut resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
        &fixture.receiver,
        &fixture.report.brain_id,
        &fixture.store,
        ENV,
    )
    .unwrap();
    let safety = crate::knowledge_materialization::evaluate_retrieval_safety(
        &fixture.receiver,
        &fixture.report.brain_id,
        &[],
        &mut resolver,
        &db,
    );
    assert!(safety.allows_content(), "{:?}", safety.hold_reason());
    drop(safety);
    drop(db);
    let published_before = event_hashes(&fixture.receiver);
    let replay = fixture.publish().unwrap();
    assert_eq!(
        (
            replay.published_segments,
            replay.published_events,
            replay.identical_segments
        ),
        (0, 0, 1)
    );
    assert_eq!(event_hashes(&fixture.receiver), published_before);
    assert_eq!(event_hashes(&fixture.source), source_before);
    assert_eq!(
        fs::read(
            fixture
                .receiver
                .join("transport/incoming")
                .join(&fixture.paths[0])
        )
        .unwrap(),
        incoming_before
    );
    fixture.assert_clean();
}

#[test]
fn mixed_replay_and_new_tail_publishes_only_new_segment() {
    let mut fixture = Fixture::new();
    fixture.publish().unwrap();
    let prior = fs::read(
        fixture
            .receiver
            .join("canonical/events")
            .join(&fixture.paths[0]),
    )
    .unwrap();
    let next = fixture.append_next();
    fixture.paths.push(next.clone());
    fixture.paths.reverse();
    let result = fixture.publish().unwrap();
    assert_eq!(
        (
            result.published_segments,
            result.published_events,
            result.identical_segments
        ),
        (1, 1, 1)
    );
    let source_path = PathBuf::from(&fixture.report.segments[0].relative_path);
    assert_eq!(
        fs::read(fixture.receiver.join("canonical/events").join(source_path)).unwrap(),
        prior
    );
    assert_eq!(
        fs::read(fixture.receiver.join("canonical/events").join(&next)).unwrap(),
        fs::read(fixture.source.join("canonical/events").join(next)).unwrap()
    );
    fixture.assert_clean();
}

#[test]
fn rejects_unapproved_corrupt_stale_and_divergent_inputs_without_canonical_changes() {
    for case in [
        "unapproved",
        "corrupt",
        "stale",
        "divergent",
        "mixed_corrupt",
    ] {
        let mut fixture = Fixture::new();
        if case == "divergent" {
            fixture.publish().unwrap();
        }
        let before = event_hashes(&fixture.receiver);
        match case {
            "unapproved" => {
                let path = PathBuf::from(&fixture.report.unapproved_segment.relative_path);
                copy_incoming(&fixture.negative, &fixture.receiver, &path);
                fixture.paths.push(path);
            }
            "corrupt" | "divergent" => {
                let path = fixture
                    .receiver
                    .join("transport/incoming")
                    .join(&fixture.paths[0]);
                let mut bytes = fs::read(&path).unwrap();
                let end = bytes.len() - 1;
                bytes[end] ^= 1;
                fs::write(path, bytes).unwrap();
            }
            "mixed_corrupt" => {
                let path = fixture.append_next();
                fs::write(
                    fixture.receiver.join("transport/incoming").join(&path),
                    b"invalid",
                )
                .unwrap();
                fixture.paths.push(path);
            }
            "stale" => {
                let db =
                    cryptex_projection::ProjectionDb::open_or_create(&fixture.receiver).unwrap();
                db.connection().execute("UPDATE knowledge_materialization_state SET integrity_content_hash = 'invalid' WHERE integrity_content_hash IS NOT NULL", []).unwrap();
            }
            _ => unreachable!(),
        }
        assert!(
            matches!(
                fixture.publish(),
                Err(IncomingPublicationError::Rejected(_))
            ),
            "{case}"
        );
        assert_eq!(event_hashes(&fixture.receiver), before, "{case}");
        fixture.assert_clean();
    }
}

#[test]
fn refuses_owned_runtime_before_staging_or_publication() {
    let fixture = Fixture::new();
    let before = event_hashes(&fixture.receiver);
    let instance = crate::instance::try_acquire_instance(&fixture.receiver).unwrap();
    assert!(matches!(
        fixture.publish(),
        Err(IncomingPublicationError::Rejected(_))
    ));
    assert_eq!(event_hashes(&fixture.receiver), before);
    drop(instance);
}

#[test]
fn hardlinked_runtime_lock_cannot_modify_existing_canonical_bytes() {
    let fixture = Fixture::new();
    let before = event_hashes(&fixture.receiver);
    let target = fixture.receiver.join(&before[0].0);
    fs::hard_link(target, crate::paths::lock_path(&fixture.receiver)).unwrap();
    assert!(matches!(
        fixture.publish(),
        Err(IncomingPublicationError::Rejected(_))
    ));
    assert_eq!(event_hashes(&fixture.receiver), before);
    assert!(!fixture.receiver.join("canonical/import-staging").exists());
}

#[test]
fn reuses_matching_instance_and_refuses_a_foreign_instance_without_staging() {
    let fixture = Fixture::new();
    let foreign = crate::test_temp::exclusive_temp_dir("incoming-foreign-owner");
    let mut foreign_instance = crate::instance::try_acquire_instance(&foreign).unwrap();
    let before = event_hashes(&fixture.receiver);
    let result = publish_incoming_with_instance(
        &fixture.receiver,
        &fixture.paths,
        &mut foreign_instance,
        &fixture.store,
        ENV,
    );
    assert!(matches!(result, Err(IncomingPublicationError::Rejected(_))));
    assert_eq!(event_hashes(&fixture.receiver), before);
    assert!(!fixture.receiver.join("canonical/import-staging").exists());
    drop(foreign_instance);
    fs::remove_dir_all(foreign).unwrap();
    let mut instance = crate::instance::try_acquire_instance(&fixture.receiver).unwrap();
    let report = publish_incoming_with_instance(
        &fixture.receiver,
        &fixture.paths,
        &mut instance,
        &fixture.store,
        ENV,
    )
    .unwrap();
    assert_eq!(report.published_segments, 1);
    assert!(
        crate::instance::try_acquire_instance(&fixture.receiver).is_err(),
        "caller instance guard remains held"
    );
    drop(instance);
    fixture.assert_clean();
}

struct FailAfterCommit<'a> {
    inner: &'a crate::FakeSecureStore,
    destination: PathBuf,
}
impl SecureStore for FailAfterCommit<'_> {
    fn put(
        &self,
        service: &str,
        account: &str,
        secret: &[u8],
    ) -> crate::secure_store::SecureStoreResult<()> {
        self.inner.put(service, account, secret)
    }
    fn get(&self, service: &str, account: &str) -> crate::secure_store::SecureStoreResult<Vec<u8>> {
        if self.destination.exists() {
            return Err(crate::SecureStoreError::Platform {
                op: "test_refresh",
                code: 1,
            });
        }
        self.inner.get(service, account)
    }
    fn delete(&self, service: &str, account: &str) -> crate::secure_store::SecureStoreResult<()> {
        self.inner.delete(service, account)
    }
}

#[test]
fn live_commit_rebuild_failure_holds_all_clones_until_verified_repair() {
    let fixture = Fixture::new();
    let mut instance = crate::instance::try_acquire_instance(&fixture.receiver).unwrap();
    let gate = instance.admission();
    let failing = FailAfterCommit {
        inner: &fixture.store,
        destination: fixture
            .receiver
            .join("canonical/events")
            .join(&fixture.paths[0]),
    };
    assert!(matches!(
        publish_incoming_with_instance(
            &fixture.receiver,
            &fixture.paths,
            &mut instance,
            &failing,
            ENV
        ),
        Err(IncomingPublicationError::CommittedNeedsRebuild(_))
    ));
    let committed = event_hashes(&fixture.receiver);
    assert!(matches!(
        gate.enter(&fixture.receiver),
        Err(AdmissionError::Held(RuntimeHold::DerivedRebuildRequired))
    ));
    assert!(matches!(
        publish_incoming_with_instance(
            &fixture.receiver,
            &fixture.paths,
            &mut instance,
            &fixture.store,
            ENV
        ),
        Err(IncomingPublicationError::Admission(AdmissionError::Held(_)))
    ));
    assert!(recover_live_with_instance(&fixture.receiver, &mut instance, &failing, ENV).is_err());
    assert!(gate.enter(&fixture.receiver).is_err());
    recover_live_with_instance(&fixture.receiver, &mut instance, &fixture.store, ENV).unwrap();
    drop(gate.enter(&fixture.receiver).unwrap());
    assert_eq!(event_hashes(&fixture.receiver), committed);
    let replay = publish_incoming_with_instance(
        &fixture.receiver,
        &fixture.paths,
        &mut instance,
        &fixture.store,
        ENV,
    )
    .unwrap();
    assert_eq!(
        (replay.published_segments, replay.identical_segments),
        (0, 1)
    );
    drop(instance);
    fixture.assert_clean();
}

#[test]
fn live_timeout_is_nonmutating_and_recovery_failure_latches_before_new_operations() {
    let fixture = Fixture::new();
    let mut instance = crate::instance::try_acquire_instance(&fixture.receiver).unwrap();
    let gate = instance.admission();
    let ordinary = gate.enter(&fixture.receiver).unwrap();
    let before = event_hashes(&fixture.receiver);
    assert!(matches!(
        publish_incoming_until(
            &fixture.receiver,
            &fixture.paths,
            &mut instance,
            &fixture.store,
            ENV,
            std::time::Instant::now()
        ),
        Err(IncomingPublicationError::Admission(AdmissionError::Busy))
    ));
    assert_eq!(event_hashes(&fixture.receiver), before);
    assert!(!fixture.receiver.join("canonical/import-staging").exists());
    drop(ordinary);
    let journal = fixture.receiver.join("canonical/import-journal");
    fs::create_dir_all(&journal).unwrap();
    let corrupt = journal.join("unknown.json");
    fs::write(&corrupt, b"synthetic invalid recovery evidence").unwrap();
    assert!(matches!(
        publish_incoming_with_instance(
            &fixture.receiver,
            &fixture.paths,
            &mut instance,
            &fixture.store,
            ENV
        ),
        Err(IncomingPublicationError::RecoveryRequired(_))
    ));
    assert!(matches!(
        gate.enter(&fixture.receiver),
        Err(AdmissionError::Held(RuntimeHold::RecoveryRequired))
    ));
    assert!(
        recover_live_with_instance(&fixture.receiver, &mut instance, &fixture.store, ENV).is_err()
    );
    assert!(gate.enter(&fixture.receiver).is_err());
    assert_eq!(event_hashes(&fixture.receiver), before);
    // Simulate operator repair of only the injected synthetic evidence.
    fs::remove_file(corrupt).unwrap();
    recover_live_with_instance(&fixture.receiver, &mut instance, &fixture.store, ENV).unwrap();
    drop(gate.enter(&fixture.receiver).unwrap());
    assert_eq!(event_hashes(&fixture.receiver), before);
    assert!(matches!(
        publish_incoming_with_instance(
            &fixture.receiver,
            &[PathBuf::from("../invalid.seg")],
            &mut instance,
            &fixture.store,
            ENV
        ),
        Err(IncomingPublicationError::Rejected(_))
    ));
    drop(gate.enter(&fixture.receiver).unwrap());
    assert_eq!(event_hashes(&fixture.receiver), before);
    drop(instance);
}

#[test]
fn post_commit_refresh_failure_never_reports_a_clean_rejection() {
    let fixture = Fixture::new();
    let destination = fixture
        .receiver
        .join("canonical/events")
        .join(&fixture.paths[0]);
    let failing = FailAfterCommit {
        inner: &fixture.store,
        destination: destination.clone(),
    };
    let result = publish_incoming(&fixture.receiver, &fixture.paths, &failing, ENV);
    assert!(
        matches!(
            result,
            Err(IncomingPublicationError::CommittedNeedsRebuild(_))
        ),
        "{result:?}"
    );
    assert_eq!(
        fs::read(destination).unwrap(),
        fs::read(
            fixture
                .source
                .join("canonical/events")
                .join(&fixture.paths[0])
        )
        .unwrap()
    );
    fixture.assert_clean();
    assert!(
        matches!(
            fixture.publish(),
            Err(IncomingPublicationError::Rejected(_))
        ),
        "stale derived state must continue holding after a committed refresh failure"
    );
}

#[test]
fn publication_interrupt_worker() {
    let Ok(root) = std::env::var("CRYPTEX_TEST_PUBLICATION_ROOT") else {
        return;
    };
    let (device, seed, paths): (String, Vec<u8>, Vec<PathBuf>) =
        serde_json::from_reader(std::io::stdin()).unwrap();
    let store = crate::FakeSecureStore::new();
    store
        .put(
            crate::ServiceLabel::device_credentials(ENV).as_str(),
            crate::AccountLabel::device_event_signing_seed(&device).as_str(),
            &seed,
        )
        .unwrap();
    let _ = publish_incoming(Path::new(&root), &paths, &store, ENV);
    panic!("worker did not reach the expected interrupt");
}

/// A genuine admitted peer restricts the local stream to genesis. Its own
/// admission depends on that stream's grant, so the complete union must hold.
fn add_policy_conflict(fixture: &mut Fixture) -> PathBuf {
    add_policy_conflict_kind(fixture, "dependency")
}

fn add_policy_conflict_kind(fixture: &mut Fixture, kind: &str) -> PathBuf {
    add_policy_conflict_at(fixture, kind, None)
}

fn add_policy_conflict_at(
    fixture: &mut Fixture,
    kind: &str,
    cutoff: Option<(u64, cryptex_events::ContentHashHex)>,
) -> PathBuf {
    use cryptex_events::{ContentHashHex, DeviceSeq, SigningKeyId};
    use std::io::Write;
    let next = fixture.append_next();
    let bytes = fs::read(fixture.source.join("canonical/events").join(&next)).unwrap();
    let prior: serde_json::Value =
        serde_json::from_slice(&bytes[cryptex_events::FRAME_HEADER_BYTES..]).unwrap();
    let mut previous = ContentHashHex::parse(prior["content_hash"].as_str().unwrap()).unwrap();
    let mut sequence: u64 = prior["device_seq"].as_str().unwrap().parse().unwrap();
    let issuer = DeviceId::new(fixture.report.source_device.clone()).unwrap();
    let target = DeviceId::new(if kind == "cycle" {
        fixture.report.source_device.clone()
    } else {
        fixture.report.receiver_device.clone()
    })
    .unwrap();
    let issuer_key = crate::event_signing_key::load_existing_event_signing_key(
        &fixture.store,
        ENV,
        &fixture.source,
        &issuer,
    )
    .unwrap();
    let target_key = crate::event_signing_key::load_existing_event_signing_key(
        &fixture.store,
        ENV,
        if kind == "cycle" {
            &fixture.source
        } else {
            &fixture.receiver
        },
        &target,
    )
    .unwrap();
    let mut bodies = Vec::new();
    let (cutoff_seq, cutoff_hash) = cutoff.unwrap_or((0, ContentHashHex::zero()));
    if kind == "terminal" {
        let retirement = cryptex_events::security_v3::sign_device_self_retired_event(
            "88888888-8888-4888-8888-888888888888",
            &fixture.report.brain_id,
            &issuer,
            DeviceSeq::from_u64(sequence + 1).unwrap(),
            "2026-11-14T00:00:02Z",
            &SigningKeyId::parse(&issuer_key.public.signing_key_id).unwrap(),
            &previous,
            &"ae".repeat(32),
            sequence,
            &previous,
            &issuer_key.signing_key,
        )
        .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&retirement).unwrap();
        previous = ContentHashHex::parse(value["content_hash"].as_str().unwrap()).unwrap();
        sequence += 1;
        bodies.push(retirement);
    }
    let body = cryptex_events::security_v3::sign_device_restricted_event(
        "99999999-9999-4999-8999-999999999999",
        &fixture.report.brain_id,
        &issuer,
        DeviceSeq::from_u64(sequence + 1).unwrap(),
        "2026-11-14T00:00:03Z",
        &SigningKeyId::parse(&issuer_key.public.signing_key_id).unwrap(),
        &previous,
        &"af".repeat(32),
        &target,
        &SigningKeyId::parse(&target_key.public.signing_key_id).unwrap(),
        &target_key.signing_key.verifying_key().to_bytes(),
        cryptex_events::security_v3::RestrictionReasonV3::Retired,
        cutoff_seq,
        &cutoff_hash,
        &issuer_key.signing_key,
    )
    .unwrap();
    let relative = PathBuf::from(issuer.as_filename_stem()).join("00000000000000000002.seg");
    let mut file =
        fs::File::create(fixture.receiver.join("transport/incoming").join(&relative)).unwrap();
    bodies.push(body);
    for body in bodies {
        file.write_all(&cryptex_events::encode_frame_header(body.len() as u32).unwrap())
            .unwrap();
        file.write_all(&body).unwrap();
    }
    drop(file);
    fixture.paths.extend([next, relative.clone()]);
    relative
}

fn retained_transaction(root: &Path) -> PathBuf {
    fs::read_dir(root.join("canonical/import-staging"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path()
}

#[test]
fn ready_union_removing_a_canonical_peers_grant_retains_evidence_and_holds_after_restart() {
    use crate::bootstrap_journal::BootstrapJournalStore;
    use cryptex_events::{AuthenticatedSecurityRecord, BootstrapApprovalRole, SecurityPayloadV3};
    let mut fixture = Fixture::new();
    fixture.publish().unwrap();
    let local = DeviceId::new(fixture.report.receiver_device.clone()).unwrap();
    let a = DeviceId::new(fixture.report.source_device.clone()).unwrap();
    let c = DeviceId::new(fixture.report.unapproved_device.clone()).unwrap();
    let local_key = crate::event_signing_key::load_existing_event_signing_key(
        &fixture.store,
        ENV,
        &fixture.receiver,
        &local,
    )
    .unwrap();
    let c_key = crate::event_signing_key::load_existing_event_signing_key(
        &fixture.store,
        ENV,
        &fixture.negative,
        &c,
    )
    .unwrap();
    let resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
        &fixture.receiver,
        &fixture.report.brain_id,
        &fixture.store,
        ENV,
    )
    .unwrap();
    let anchor = resolver.local_trust_anchor().unwrap();
    let snapshot = crate::knowledge_materialization::scan_verified_trust_policy(
        &fixture.receiver,
        &fixture.report.brain_id,
        &local,
        anchor.signing_key_id(),
        anchor.public_key(),
        &[local.clone(), a.clone()],
    )
    .unwrap();
    let first_grant = snapshot
        .history
        .records()
        .iter()
        .find_map(|record| match record {
            AuthenticatedSecurityRecord::SecurityV3(event) if event.device_id() == &local => {
                Some(event)
            }
            _ => None,
        })
        .unwrap();
    let boundary = (
        first_grant.device_seq().to_u64().unwrap(),
        first_grant.content_hash().clone(),
    );
    assert_eq!(boundary.0, 2);
    let SecurityPayloadV3::PeerApproved { transcript, .. } = first_grant.payload() else {
        panic!("expected A admission");
    };
    let old_messages = transcript
        .as_array()
        .unwrap()
        .iter()
        .map(|v| serde_json::to_vec(v).unwrap())
        .collect::<Vec<_>>();
    let old = cryptex_events::verify_bootstrap_transcript(
        &old_messages.iter().map(Vec::as_slice).collect::<Vec<_>>(),
    )
    .unwrap();
    let transport = if old.initiator().device_id == local {
        old.initiator_transport_device_id()
    } else {
        old.responder_transport_device_id()
    }
    .to_owned();
    drop(snapshot);

    // The real daemon journal/writer creates C's only grant after A's grant.
    let tx = "bf".repeat(32);
    let offer = cryptex_events::create_bootstrap_offer(
        &fixture.report.brain_id,
        &tx,
        &local,
        &transport,
        &[0x61; 32],
        &local_key.signing_key,
    )
    .unwrap();
    let proof = cryptex_events::create_bootstrap_proof(
        &offer,
        &c,
        &["CCCCCCC"; 8].join("-"),
        &[0x62; 32],
        &c_key.signing_key,
    )
    .unwrap();
    let request = cryptex_events::verify_bootstrap_approval_request(
        &[&offer, &proof],
        BootstrapApprovalRole::Initiator,
    )
    .unwrap();
    let local_approval =
        cryptex_events::sign_bootstrap_approval(&request, &local_key.signing_key).unwrap();
    let peer_request = cryptex_events::verify_bootstrap_approval_request(
        &[&offer, &proof, &local_approval],
        BootstrapApprovalRole::Responder,
    )
    .unwrap();
    let peer_approval =
        cryptex_events::sign_bootstrap_approval(&peer_request, &c_key.signing_key).unwrap();
    let messages = vec![offer, proof, local_approval, peer_approval];
    let journal = BootstrapJournalStore::new(&fixture.receiver).unwrap();
    journal
        .begin_verified_approval_prefix(
            &request,
            &messages[..2],
            "77777777-7777-4777-8777-777777777777",
        )
        .unwrap();
    journal
        .record_local_approval_signature(&request, &messages[2])
        .unwrap();
    journal
        .record_verified_complete_transcript(&tx, &messages)
        .unwrap();
    crate::knowledge_materialization::append_local_peer_approval_anchor(
        &fixture.receiver,
        &fixture.report.brain_id,
        &fixture.store,
        ENV,
        &tx,
        &messages,
    )
    .unwrap();
    drop(
        crate::canonical_writer::acquire_canonical_write_lock_set(
            &fixture.receiver,
            &[local.clone(), a, c.clone()],
        )
        .unwrap(),
    );
    let mut instance = crate::instance::try_acquire_instance(&fixture.receiver).unwrap();
    recover_live_with_instance(&fixture.receiver, &mut instance, &fixture.store, ENV).unwrap();
    let c_first = PathBuf::from(&fixture.report.unapproved_segment.relative_path);
    let c_tail = append_offline_note(
        &fixture,
        &fixture.negative,
        c.as_str(),
        "Synthetic C admission-removal witness.",
    );
    for path in [&c_first, &c_tail] {
        copy_incoming(&fixture.negative, &fixture.receiver, path);
    }
    publish_incoming_with_instance(
        &fixture.receiver,
        &[c_first, c_tail],
        &mut instance,
        &fixture.store,
        ENV,
    )
    .unwrap();
    assert!(
        verified_note_texts(&fixture, &fixture.receiver)
            .contains("Synthetic C admission-removal witness.")
    );
    let before = event_hashes(&fixture.receiver);
    add_policy_conflict_at(&mut fixture, "removal", Some(boundary));
    let gate = instance.admission();
    assert!(matches!(
        publish_incoming_with_instance(
            &fixture.receiver,
            &fixture.paths,
            &mut instance,
            &fixture.store,
            ENV
        ),
        Err(IncomingPublicationError::PolicyHeld(
            crate::import_staging::AuthenticatedPolicyConflict::AdmissionRemoved
        ))
    ));
    assert!(gate.enter(&fixture.receiver).is_err());
    assert!(gate.clone().enter(&fixture.receiver).is_err());
    assert_eq!(event_hashes(&fixture.receiver), before);
    drop(instance);
    let mut restarted = crate::instance::try_acquire_instance(&fixture.receiver).unwrap();
    assert!(matches!(
        recover_live_with_instance(&fixture.receiver, &mut restarted, &fixture.store, ENV),
        Err(IncomingPublicationError::RecoveryRequired(
            ImportStagingError::PolicyConflict(
                crate::import_staging::AuthenticatedPolicyConflict::AdmissionRemoved
            )
        ))
    ));
    assert!(restarted.admission().enter(&fixture.receiver).is_err());
    assert!(retained_transaction(&fixture.receiver).exists());
    assert_eq!(event_hashes(&fixture.receiver), before);
}

#[test]
fn authenticated_cycle_and_terminal_tail_are_retained_and_rechecked_after_restart() {
    use crate::import_staging::AuthenticatedPolicyConflict;
    for (kind, expected) in [
        ("cycle", AuthenticatedPolicyConflict::RestrictionCycle),
        (
            "terminal",
            AuthenticatedPolicyConflict::TerminalRetirementTail,
        ),
    ] {
        let mut fixture = Fixture::new();
        add_policy_conflict_kind(&mut fixture, kind);
        let before = event_hashes(&fixture.receiver);
        assert!(matches!(
            fixture.publish(),
            Err(IncomingPublicationError::PolicyHeld(reason)) if reason == expected
        ));
        let mut instance = crate::instance::try_acquire_instance(&fixture.receiver).unwrap();
        assert!(matches!(
            recover_live_with_instance(&fixture.receiver, &mut instance, &fixture.store, ENV),
            Err(IncomingPublicationError::RecoveryRequired(
                ImportStagingError::PolicyConflict(reason)
            )) if reason == expected
        ));
        assert!(instance.admission().enter(&fixture.receiver).is_err());
        assert_eq!(event_hashes(&fixture.receiver), before);
    }
}

#[test]
fn authenticated_policy_conflict_holds_all_clones_and_survives_restart_without_sqlite() {
    let mut fixture = Fixture::new();
    add_policy_conflict(&mut fixture);
    let before = event_hashes(&fixture.receiver);
    let mut instance = crate::instance::try_acquire_instance(&fixture.receiver).unwrap();
    let first = instance.admission();
    let clone = first.clone();
    assert!(matches!(
        publish_incoming_with_instance(
            &fixture.receiver,
            &fixture.paths,
            &mut instance,
            &fixture.store,
            ENV
        ),
        Err(IncomingPublicationError::PolicyHeld(
            crate::import_staging::AuthenticatedPolicyConflict::AdmissionDependency
        ))
    ));
    for gate in [first, clone] {
        assert!(matches!(
            gate.enter(&fixture.receiver),
            Err(AdmissionError::Held(RuntimeHold::IncomingPolicyConflict))
        ));
    }
    assert_eq!(event_hashes(&fixture.receiver), before);
    let transaction = retained_transaction(&fixture.receiver);
    let evidence = fs::read(transaction.join("policy-review.json")).unwrap();
    assert!(crate::import_staging::recover_import_staging(&fixture.receiver).is_err());
    assert!(
        recover_live_with_instance(&fixture.receiver, &mut instance, &fixture.store, ENV).is_err()
    );
    drop(instance);
    // Corrupt derived state cannot mask or erase the authenticated policy hold.
    fs::write(
        cryptex_projection::projection_sqlite_path(&fixture.receiver),
        b"not a SQLite database",
    )
    .unwrap();
    let mut restarted = crate::instance::try_acquire_instance(&fixture.receiver).unwrap();
    assert!(matches!(
        recover_live_with_instance(&fixture.receiver, &mut restarted, &fixture.store, ENV),
        Err(IncomingPublicationError::RecoveryRequired(
            ImportStagingError::PolicyConflict(_)
        ))
    ));
    assert!(restarted.admission().enter(&fixture.receiver).is_err());
    assert_eq!(
        fs::read(transaction.join("policy-review.json")).unwrap(),
        evidence
    );
    assert_eq!(event_hashes(&fixture.receiver), before);
}

#[test]
fn mixed_malformed_conflict_is_clean_rejection_and_does_not_latch_authority() {
    use std::io::Write;
    let mut fixture = Fixture::new();
    let conflict = add_policy_conflict(&mut fixture);
    let path = fixture.receiver.join("transport/incoming").join(conflict);
    fs::OpenOptions::new()
        .append(true)
        .open(path)
        .unwrap()
        .write_all(b"incomplete frame")
        .unwrap();
    let before = event_hashes(&fixture.receiver);
    let mut instance = crate::instance::try_acquire_instance(&fixture.receiver).unwrap();
    assert!(matches!(
        publish_incoming_with_instance(
            &fixture.receiver,
            &fixture.paths,
            &mut instance,
            &fixture.store,
            ENV
        ),
        Err(IncomingPublicationError::Rejected(_))
    ));
    assert!(instance.admission().enter(&fixture.receiver).is_ok());
    assert_eq!(event_hashes(&fixture.receiver), before);
    fixture.assert_clean();
}

#[test]
fn missing_corrupt_or_changed_policy_evidence_never_reopens_old_content() {
    for fault in [
        "missing-decision",
        "missing-intent",
        "corrupt-decision",
        "corrupt-stage",
        "extra-stage",
        "missing-local-key",
        "wrong-local-key",
        "unsupported-version",
    ] {
        let mut fixture = Fixture::new();
        add_policy_conflict(&mut fixture);
        assert!(matches!(
            fixture.publish(),
            Err(IncomingPublicationError::PolicyHeld(_))
        ));
        let before = event_hashes(&fixture.receiver);
        let stage = retained_transaction(&fixture.receiver);
        let id = stage.file_name().unwrap().to_str().unwrap();
        let journal = fixture.receiver.join("canonical/import-journal");
        let held = journal.join(format!("{id}.policy-held.json"));
        let intent = journal.join(format!("{id}.policy-review.json"));
        match fault {
            "missing-decision" => fs::remove_file(&held).unwrap(),
            "missing-intent" => fs::remove_file(&intent).unwrap(),
            "corrupt-decision" => fs::write(&held, b"{}").unwrap(),
            "corrupt-stage" => {
                fs::write(stage.join(&fixture.paths[0]), b"changed evidence").unwrap()
            }
            "extra-stage" => fs::write(stage.join("unlisted.seg"), b"extra evidence").unwrap(),
            "unsupported-version" => {
                for path in [intent, stage.join("policy-review.json")] {
                    let mut value: serde_json::Value =
                        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                    value["version"] = 2.into();
                    fs::write(path, serde_json_canonicalizer::to_vec(&value).unwrap()).unwrap();
                }
            }
            "wrong-local-key" | "missing-local-key" => {}
            _ => unreachable!(),
        }
        let mut instance = crate::instance::try_acquire_instance(&fixture.receiver).unwrap();
        let empty = crate::FakeSecureStore::new();
        if fault == "wrong-local-key" {
            empty
                .put(
                    crate::ServiceLabel::device_credentials(ENV).as_str(),
                    crate::AccountLabel::device_event_signing_seed(&fixture.report.receiver_device)
                        .as_str(),
                    &[0x64; 32],
                )
                .unwrap();
        }
        let store = if matches!(fault, "wrong-local-key" | "missing-local-key") {
            &empty
        } else {
            &fixture.store
        };
        assert!(
            recover_live_with_instance(&fixture.receiver, &mut instance, store, ENV).is_err(),
            "{fault}"
        );
        assert!(
            instance.admission().enter(&fixture.receiver).is_err(),
            "{fault}"
        );
        assert!(stage.exists(), "{fault}");
        assert_eq!(event_hashes(&fixture.receiver), before, "{fault}");
    }
}

#[test]
fn process_interruptions_of_policy_review_preserve_or_reconstruct_the_hold() {
    use std::process::{Command, Stdio};
    use std::time::Duration;
    for boundary in [
        "m8_policy_intent_synced",
        "m8_policy_held_latch_synced",
        "m8_policy_held_decision_synced",
    ] {
        let mut fixture = Fixture::new();
        add_policy_conflict(&mut fixture);
        let before = event_hashes(&fixture.receiver);
        let seed = fixture
            .store
            .get(
                crate::ServiceLabel::device_credentials(ENV).as_str(),
                crate::AccountLabel::device_event_signing_seed(&fixture.report.receiver_device)
                    .as_str(),
            )
            .unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "incoming_publication::tests::publication_interrupt_worker",
                "--nocapture",
            ])
            .env("CRYPTEX_TEST_PUBLICATION_ROOT", &fixture.receiver)
            .env(crate::test_interrupt::INTERRUPT_ENV, boundary)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        serde_json::to_writer(
            child.stdin.take().unwrap(),
            &(&fixture.report.receiver_device, seed, &fixture.paths),
        )
        .unwrap();
        let status =
            crate::test_interrupt::wait_child_bounded(&mut child, Duration::from_secs(30)).unwrap();
        assert_eq!(
            status.code(),
            Some(crate::test_interrupt::INTERRUPT_EXIT_CODE),
            "{boundary}"
        );
        let mut instance = crate::instance::try_acquire_instance(&fixture.receiver).unwrap();
        assert!(
            recover_live_with_instance(&fixture.receiver, &mut instance, &fixture.store, ENV)
                .is_err(),
            "{boundary}"
        );
        assert!(
            instance.admission().enter(&fixture.receiver).is_err(),
            "{boundary}"
        );
        assert!(retained_transaction(&fixture.receiver).exists());
        assert_eq!(event_hashes(&fixture.receiver), before);
    }
}

#[test]
fn interrupted_unclassified_ready_or_mixed_bad_batch_aborts_on_first_restart() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    use std::time::Duration;
    for malformed in [false, true] {
        let mut fixture = Fixture::new();
        if malformed {
            let conflict = add_policy_conflict(&mut fixture);
            fs::OpenOptions::new()
                .append(true)
                .open(fixture.receiver.join("transport/incoming").join(conflict))
                .unwrap()
                .write_all(b"incomplete frame")
                .unwrap();
        }
        let before = event_hashes(&fixture.receiver);
        let seed = fixture
            .store
            .get(
                crate::ServiceLabel::device_credentials(ENV).as_str(),
                crate::AccountLabel::device_event_signing_seed(&fixture.report.receiver_device)
                    .as_str(),
            )
            .unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "incoming_publication::tests::publication_interrupt_worker",
                "--nocapture",
            ])
            .env("CRYPTEX_TEST_PUBLICATION_ROOT", &fixture.receiver)
            .env(
                crate::test_interrupt::INTERRUPT_ENV,
                "m8_policy_intent_synced",
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        serde_json::to_writer(
            child.stdin.take().unwrap(),
            &(&fixture.report.receiver_device, seed, &fixture.paths),
        )
        .unwrap();
        let status =
            crate::test_interrupt::wait_child_bounded(&mut child, Duration::from_secs(30)).unwrap();
        assert_eq!(
            status.code(),
            Some(crate::test_interrupt::INTERRUPT_EXIT_CODE)
        );
        let mut instance = crate::instance::try_acquire_instance(&fixture.receiver).unwrap();
        recover_live_with_instance(&fixture.receiver, &mut instance, &fixture.store, ENV).unwrap();
        assert!(instance.admission().enter(&fixture.receiver).is_ok());
        assert_eq!(event_hashes(&fixture.receiver), before);
        fixture.assert_clean();
    }
}

#[test]
fn process_interruptions_preserve_evidence_and_recover_exact_canonical_bytes() {
    use std::process::{Command, Stdio};
    use std::time::Duration;
    for boundary in [
        "m8_after_prepared_temp_sync",
        "m8_after_prepared_manifest_sync",
        "m8_after_canonical_segment_move_0",
        "m8_after_canonical_segment_move_1",
        "m8_after_committed_marker_temp_sync",
        "m8_after_committed_marker_final_sync",
    ] {
        let mut fixture = Fixture::new();
        let next = fixture.append_next();
        fixture.paths.push(next);
        let before = event_hashes(&fixture.receiver);
        let seed = fixture
            .store
            .get(
                crate::ServiceLabel::device_credentials(ENV).as_str(),
                crate::AccountLabel::device_event_signing_seed(&fixture.report.receiver_device)
                    .as_str(),
            )
            .unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "incoming_publication::tests::publication_interrupt_worker",
                "--nocapture",
            ])
            .env("CRYPTEX_TEST_PUBLICATION_ROOT", &fixture.receiver)
            .env(crate::test_interrupt::INTERRUPT_ENV, boundary)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        serde_json::to_writer(
            child.stdin.take().unwrap(),
            &(&fixture.report.receiver_device, seed, &fixture.paths),
        )
        .unwrap();
        let status =
            crate::test_interrupt::wait_child_bounded(&mut child, Duration::from_secs(30)).unwrap();
        assert_eq!(
            status.code(),
            Some(crate::test_interrupt::INTERRUPT_EXIT_CODE),
            "{boundary}"
        );
        assert_eq!(
            crate::test_interrupt::read_ready(&fixture.receiver).0,
            boundary
        );
        let instance = crate::instance::try_acquire_instance(&fixture.receiver).unwrap();
        for _ in 0..2 {
            crate::import_staging::recover_import_staging_with_local_state(
                &fixture.receiver,
                &instance,
                &fixture.store,
                ENV,
            )
            .unwrap();
        }
        drop(instance);
        fixture.assert_clean();
        if matches!(
            boundary,
            "m8_after_prepared_temp_sync" | "m8_after_prepared_manifest_sync"
        ) {
            assert_eq!(event_hashes(&fixture.receiver), before);
            assert_eq!(fixture.publish().unwrap().published_segments, 2);
        } else {
            assert_eq!(
                fs::read(
                    fixture
                        .receiver
                        .join("canonical/events")
                        .join(&fixture.paths[0])
                )
                .unwrap(),
                fs::read(
                    fixture
                        .source
                        .join("canonical/events")
                        .join(&fixture.paths[0])
                )
                .unwrap()
            );
            // Recovery never invents fresh derived watermarks. Until explicit
            // rebuild, a new admission must retain the existing stale hold.
            assert!(matches!(
                fixture.publish(),
                Err(IncomingPublicationError::Rejected(_))
            ));
            let resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
                &fixture.receiver,
                &fixture.report.brain_id,
                &fixture.store,
                ENV,
            )
            .unwrap();
            let mut db =
                cryptex_projection::ProjectionDb::open_or_create(&fixture.receiver).unwrap();
            crate::knowledge_materialization::rebuild_knowledge_from_local_approval_history(
                &fixture.receiver,
                &fixture.report.brain_id,
                &resolver.local_trust_anchor().unwrap(),
                &mut db,
            )
            .unwrap();
            drop(db);
            assert_eq!(fixture.publish().unwrap().identical_segments, 2);
        }
        for path in &fixture.paths {
            let source = fs::read(fixture.source.join("canonical/events").join(path)).unwrap();
            assert_eq!(
                fs::read(fixture.receiver.join("transport/incoming").join(path)).unwrap(),
                source
            );
            assert_eq!(
                fs::read(fixture.receiver.join("canonical/events").join(path)).unwrap(),
                source
            );
        }
    }
}
