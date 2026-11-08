//! Explicit validation of transport candidates through the daemon's import gates.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::import_staging::{ImportStagingError, ImportStagingSession};
use crate::secure_store::{SecureStore, SecureStoreEnvironment};

#[derive(Debug, Serialize)]
pub struct IncomingValidationReport {
    pub outcome: &'static str,
    pub segment_count: usize,
    pub event_count: u64,
    pub published: bool,
}

/// Snapshot and validate an explicit batch from transport/incoming, then
/// discard its staging files. Publication stays behind its separate gates.
pub fn validate_incoming<S: SecureStore + ?Sized>(
    root: &Path,
    paths: &[PathBuf],
    store: &S,
    environment: SecureStoreEnvironment,
) -> Result<IncomingValidationReport, ImportStagingError> {
    // Refuse unsafe roots before the instance API can create/write anything.
    let _runtime_pins = ImportStagingSession::pin_existing_runtime(root)?;
    // Own the runtime before inspecting or mutating import state, and retain
    // ownership through cleanup. In particular, never take the staging slot
    // from a running daemon whose startup recovery may need that same slot.
    let _instance = crate::instance::try_acquire_instance(root)
        .map_err(|error| ImportStagingError::Invalid(error.to_string()))?;
    let mut session = ImportStagingSession::begin(root)?;
    let result = (|| {
        let brain = crate::identity_runtime::load_brain_identity(root)
            .map_err(|error| ImportStagingError::Invalid(error.to_string()))?;
        let mut resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
            root,
            &brain.brain_id,
            store,
            environment,
        )
        .map_err(|error| ImportStagingError::Invalid(error.to_string()))?;
        let db = cryptex_projection::ProjectionDb::open_existing_read_only(root)
            .map_err(|error| ImportStagingError::Invalid(error.to_string()))?;
        let mut devices = session
            .stage_batch_from_transport_paths(paths)?
            .iter()
            .map(|candidate| candidate.device_id.clone())
            .collect::<Vec<_>>();
        devices.sort_by(|left, right| left.as_str().as_bytes().cmp(right.as_str().as_bytes()));
        devices.dedup();
        #[cfg(test)]
        crate::test_interrupt::maybe_abrupt_exit(
            "m8_validation_after_staging",
            root,
            "incoming snapshot complete; instance and staging locks held",
        );
        session.release_staging_slot()?;
        let safety = crate::knowledge_materialization::evaluate_import_retrieval_safety(
            root,
            &brain.brain_id,
            &devices,
            &mut resolver,
            &db,
        );
        let verified = safety.verify_import_batch(&session, &brain.brain_id)?;
        #[cfg(test)]
        crate::test_interrupt::maybe_abrupt_exit(
            "m8_validation_after_policy",
            root,
            "policy verified; instance and canonical locks held before discard",
        );
        Ok(IncomingValidationReport {
            outcome: "validated_only",
            segment_count: verified.verified().segments().len(),
            event_count: verified
                .verified()
                .segments()
                .iter()
                .map(|s| s.event_count)
                .sum(),
            published: false,
        })
    })();
    // Always surface cleanup failures, including when validation also failed.
    if let Err(cleanup) = session.discard() {
        return Err(ImportStagingError::RecoveryConflict(format!(
            "validation result {:?}; staging cleanup failed: {cleanup}",
            result.as_ref().err(),
        )));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use cryptex_core::DeviceId;
    use cryptex_events::{DeviceSeq, envelope::Classification};
    use std::fs;

    fn fixture() -> (
        PathBuf,
        PathBuf,
        crate::secure_store::FakeSecureStore,
        Vec<u8>,
    ) {
        let root = crate::test_temp::exclusive_temp_dir("cx-incoming");
        fixture_at(root)
    }

    fn fixture_at(
        root: PathBuf,
    ) -> (
        PathBuf,
        PathBuf,
        crate::secure_store::FakeSecureStore,
        Vec<u8>,
    ) {
        let store = crate::secure_store::FakeSecureStore::new();
        let env = SecureStoreEnvironment::Dev;
        let brain = "22222222-2222-4222-8222-222222222222";
        crate::identity_runtime::write_brain_identity(
            &root,
            &cryptex_core::BrainIdentity::new(brain, "2026-11-07T00:00:00Z").unwrap(),
        )
        .unwrap();
        let enrollment =
            crate::local_device_identity::enroll_local_device(&root, &store, env).unwrap();
        let device = DeviceId::new(enrollment.device_id).unwrap();
        let first = crate::canonical_writer::append_memory_note(
            &store,
            env,
            crate::canonical_writer::AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Public,
                text: "synthetic incoming validation",
                event_id: None,
                config: &crate::canonical_writer::CanonicalWriterConfig {
                    seal_event_threshold: 1,
                },
            },
        )
        .unwrap()
        .accepted;
        let ready = crate::event_signing_key::ensure_event_signing_key(
            &store,
            env,
            &root,
            &device,
            std::slice::from_ref(&first.signing_key_id),
            true,
        )
        .unwrap();
        let accepted = cryptex_events::accept_event(
            cryptex_events::SignableEvent {
                schema_version: 1,
                event_id: "22222222-2222-4222-8222-222222222224".into(),
                brain_id: brain.into(),
                device_id: device.as_str().into(),
                device_seq: DeviceSeq::from_u64(2).unwrap(),
                created_at: "2026-11-07T00:00:01Z".into(),
                event_type: cryptex_events::EventType::MemoryNote,
                classification: Classification::Public,
                signing_key_id: first.signing_key_id,
                payload: cryptex_events::EventPayload::MemoryNote(
                    cryptex_events::MemoryNotePayload::new("synthetic candidate").unwrap(),
                ),
                prev_content_hash: first.content_hash,
            },
            &ready.signing_key,
        )
        .unwrap();
        let body = serde_json::to_vec(&accepted).unwrap();
        let mut bytes = cryptex_events::encode_frame_header(body.len() as u32)
            .unwrap()
            .to_vec();
        bytes.extend_from_slice(&body);
        let relative = PathBuf::from(device.as_filename_stem()).join("00000000000000000001.seg");
        let source = root.join("transport/incoming").join(&relative);
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, &bytes).unwrap();
        let mut db = cryptex_projection::ProjectionDb::open_or_create(&root).unwrap();
        crate::projection::rebuild_device_projection(&store, env, &root, &device, &db).unwrap();
        let mut resolver =
            crate::local_device_identity::LocalDeviceKeyResolver::load(&root, brain, &store, env)
                .unwrap();
        crate::knowledge_materialization::rebuild_knowledge_from_canonical(
            &root,
            brain,
            &[device],
            &mut resolver,
            &mut db,
        )
        .unwrap();
        (root, relative, store, bytes)
    }

    #[test]
    fn accepts_two_segments_from_one_device_in_reverse_input_order() {
        let (root, path, store, source) = fixture();
        let first: cryptex_events::AcceptedEvent =
            serde_json::from_slice(&source[cryptex_events::FRAME_HEADER_BYTES..]).unwrap();
        let device = DeviceId::new(first.device_id.clone()).unwrap();
        let ready = crate::event_signing_key::load_existing_event_signing_key(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            &device,
        )
        .unwrap();
        let body = cryptex_events::security_v3::sign_device_self_retired_event(
            "22222222-2222-4222-8222-222222222225",
            &first.brain_id,
            &device,
            DeviceSeq::from_u64(3).unwrap(),
            "2026-11-07T00:00:02Z",
            &first.signing_key_id,
            &first.content_hash,
            &"a".repeat(64),
            2,
            &first.content_hash,
            &ready.signing_key,
        )
        .unwrap();
        let mut bytes = cryptex_events::encode_frame_header(body.len() as u32)
            .unwrap()
            .to_vec();
        bytes.extend_from_slice(&body);
        let second = path.with_file_name("00000000000000000002.seg");
        fs::write(root.join("transport/incoming").join(&second), bytes).unwrap();
        let report = validate_incoming(
            &root,
            &[second.clone(), path.clone()],
            &store,
            SecureStoreEnvironment::Dev,
        )
        .unwrap();
        assert_eq!((report.segment_count, report.event_count), (2, 2));
        assert!(!root.join("canonical/events").join(path).exists());
        assert!(!root.join("canonical/events").join(second).exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn validates_through_policy_without_publishing_and_cleans_staging() {
        let (root, path, store, source) = fixture();
        let report = validate_incoming(
            &root,
            std::slice::from_ref(&path),
            &store,
            SecureStoreEnvironment::Dev,
        )
        .unwrap();
        assert_eq!(report.outcome, "validated_only");
        assert!(!report.published);
        assert_eq!((report.segment_count, report.event_count), (1, 1));
        assert!(!root.join("canonical/events").join(&path).exists());
        assert_eq!(
            fs::read(root.join("transport/incoming").join(path)).unwrap(),
            source
        );
        assert_eq!(
            fs::read_dir(root.join("canonical/import-staging"))
                .unwrap()
                .count(),
            0
        );
        assert_eq!(
            fs::read_dir(root.join("canonical/import-journal"))
                .unwrap()
                .count(),
            1
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_signature_rejects_and_releases_slot_for_valid_retry() {
        let (root, path, store, source) = fixture();
        let source_path = root.join("transport/incoming").join(&path);
        let mut corrupt = source.clone();
        let signature = corrupt
            .windows(13)
            .position(|w| w == b"\"signature\":\"")
            .unwrap()
            + 13;
        corrupt[signature] = if corrupt[signature] == b'0' {
            b'1'
        } else {
            b'0'
        };
        fs::write(&source_path, corrupt).unwrap();
        assert!(
            validate_incoming(
                &root,
                std::slice::from_ref(&path),
                &store,
                SecureStoreEnvironment::Dev
            )
            .is_err()
        );
        assert!(!root.join("canonical/events").join(&path).exists());
        assert_eq!(
            fs::read_dir(root.join("canonical/import-staging"))
                .unwrap()
                .count(),
            0
        );
        fs::write(source_path, source).unwrap();
        assert!(validate_incoming(&root, &[path], &store, SecureStoreEnvironment::Dev).is_ok());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unapproved_candidate_cannot_create_a_canonical_device_directory() {
        let (root, _, store, source) = fixture();
        let peer = DeviceId::new("33333333-3333-4333-8333-333333333333").unwrap();
        let path = PathBuf::from(peer.as_filename_stem()).join("00000000000000000000.seg");
        let incoming = root.join("transport/incoming").join(&path);
        fs::create_dir_all(incoming.parent().unwrap()).unwrap();
        fs::write(incoming, source).unwrap();
        assert!(validate_incoming(&root, &[path], &store, SecureStoreEnvironment::Dev).is_err());
        assert!(
            !root
                .join("canonical/events")
                .join(peer.as_filename_stem())
                .exists()
        );
        assert_eq!(
            fs::read_dir(root.join("canonical/import-staging"))
                .unwrap()
                .count(),
            0
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn refuses_running_daemon_and_preserves_existing_prepared_state() {
        let (root, path, store, _) = fixture();
        let instance = crate::instance::try_acquire_instance(&root).unwrap();
        assert!(
            validate_incoming(
                &root,
                std::slice::from_ref(&path),
                &store,
                SecureStoreEnvironment::Dev
            )
            .is_err()
        );
        drop(instance);
        let unknown = root.join("canonical/import-journal/unknown.prepared.json");
        assert!(!unknown.parent().unwrap().exists());
        assert!(!root.join("canonical/import-staging").exists());
        fs::create_dir_all(unknown.parent().unwrap()).unwrap();
        fs::write(&unknown, b"synthetic unknown state").unwrap();
        assert!(validate_incoming(&root, &[path], &store, SecureStoreEnvironment::Dev).is_err());
        assert_eq!(fs::read(unknown).unwrap(), b"synthetic unknown state");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_missing_and_relative_roots_without_creating_them() {
        let parent = crate::test_temp::exclusive_temp_dir("cx-validation-root");
        let missing = parent.join("missing");
        let store = crate::secure_store::FakeSecureStore::new();
        assert!(validate_incoming(&missing, &[], &store, SecureStoreEnvironment::Dev).is_err());
        assert!(!missing.exists());
        assert!(
            validate_incoming(Path::new("."), &[], &store, SecureStoreEnvironment::Dev).is_err()
        );
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn running_instance_takes_precedence_over_import_slot_conflicts() {
        let (root, path, store, _) = fixture();
        let instance = crate::instance::try_acquire_instance(&root).unwrap();
        let session = ImportStagingSession::begin(&root).unwrap();
        let error =
            validate_incoming(&root, &[path], &store, SecureStoreEnvironment::Dev).unwrap_err();
        assert!(error.to_string().contains("already"), "{error}");
        session.discard().unwrap();
        drop(instance);
        fs::remove_dir_all(root).unwrap();
    }

    fn event_bytes(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        let mut records = Vec::new();
        for device in fs::read_dir(root.join("canonical/events")).unwrap() {
            let device = device.unwrap();
            if !device.file_type().unwrap().is_dir() {
                continue;
            }
            for entry in fs::read_dir(device.path()).unwrap() {
                let path = entry.unwrap().path();
                if path.extension().is_some_and(|ext| ext == "seg") {
                    records.push((path.clone(), fs::read(path).unwrap()));
                }
            }
        }
        records.sort_by(|a, b| a.0.cmp(&b.0));
        records
    }

    #[test]
    fn xproc_validation_interrupt_worker() {
        let Some(root) = std::env::var_os("CRYPTEX_TEST_VALIDATION_ROOT") else {
            return;
        };
        let (root, path, store, _) = fixture_at(PathBuf::from(root));
        fs::write(
            root.join("validation-before.json"),
            serde_json::to_vec(&event_bytes(&root)).unwrap(),
        )
        .unwrap();
        validate_incoming(&root, &[path], &store, SecureStoreEnvironment::Dev).unwrap();
        panic!("validation did not reach its expected interruption boundary");
    }

    #[test]
    fn interrupted_validation_recovers_without_publication_or_event_changes() {
        use std::process::Command;
        use std::time::Duration;

        for boundary in ["m8_validation_after_staging", "m8_validation_after_policy"] {
            let root = crate::test_temp::exclusive_temp_dir("cx-validation-crash");
            let mut child = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "incoming_validation::tests::xproc_validation_interrupt_worker",
                    "--nocapture",
                ])
                .env("CRYPTEX_TEST_VALIDATION_ROOT", &root)
                .env(crate::test_interrupt::INTERRUPT_ENV, boundary)
                .spawn()
                .unwrap();
            let status =
                crate::test_interrupt::wait_child_bounded(&mut child, Duration::from_secs(30))
                    .unwrap();
            assert_eq!(
                status.code(),
                Some(crate::test_interrupt::INTERRUPT_EXIT_CODE)
            );
            assert_eq!(crate::test_interrupt::read_ready(&root).0, boundary);
            let before: Vec<(PathBuf, Vec<u8>)> =
                serde_json::from_slice(&fs::read(root.join("validation-before.json")).unwrap())
                    .unwrap();
            assert!(!before.is_empty());
            assert_eq!(event_bytes(&root), before);
            let device = before[0].0.parent().unwrap().file_name().unwrap();
            let incoming = root
                .join("transport/incoming")
                .join(device)
                .join("00000000000000000001.seg");
            let incoming_bytes = fs::read(&incoming).unwrap();
            let transaction = fs::read_dir(root.join("canonical/import-staging"))
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            // A retry must preserve interrupted state for startup recovery.
            let store = crate::secure_store::FakeSecureStore::new();
            assert!(matches!(
                validate_incoming(&root, &[], &store, SecureStoreEnvironment::Dev),
                Err(ImportStagingError::RecoveryConflict(_))
            ));
            assert!(transaction.exists());
            let instance = crate::instance::try_acquire_instance(&root).unwrap();
            for _ in 0..2 {
                crate::import_staging::recover_import_staging(&root).unwrap();
                assert_eq!(event_bytes(&root), before);
                assert_eq!(fs::read(&incoming).unwrap(), incoming_bytes);
                assert_eq!(
                    fs::read_dir(root.join("canonical/import-staging"))
                        .unwrap()
                        .count(),
                    0
                );
            }
            assert_eq!(
                fs::read_dir(root.join("canonical/import-journal"))
                    .unwrap()
                    .count(),
                1
            );
            drop(instance);
            fs::remove_dir_all(root).unwrap();
        }
    }
}
