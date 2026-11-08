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
    let mut session = ImportStagingSession::begin(root)?;
    // Keep instance ownership through staging cleanup as well as validation.
    let instance = crate::instance::try_acquire_instance(root);
    let result = (|| {
        // This command runs while the Desktop-owned daemon is stopped.
        let _instance = instance
            .as_ref()
            .map_err(|error| ImportStagingError::Invalid(error.to_string()))?;
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
        session.release_staging_slot()?;
        let safety = crate::knowledge_materialization::evaluate_import_retrieval_safety(
            root,
            &brain.brain_id,
            &devices,
            &mut resolver,
            &db,
        );
        let verified = safety.verify_import_batch(&session, &brain.brain_id)?;
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
        fs::write(&unknown, b"synthetic unknown state").unwrap();
        assert!(validate_incoming(&root, &[path], &store, SecureStoreEnvironment::Dev).is_err());
        assert_eq!(fs::read(unknown).unwrap(), b"synthetic unknown state");
        fs::remove_dir_all(root).unwrap();
    }
}
