use super::*;
use cryptex_core::DeviceId;
use cryptex_events::envelope::Classification;
use std::fs;

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
