use super::*;
use crate::managed_profile::worker::ManagedTransportWorker;

const LOCAL_TRANSPORT: &str = "U2SYIGU-YWZX757-2WPZGCR-DHD7MLH-ZKV7HLJ-EYO6CXE-K3DU3DU-SDFXYAE";
const OTHER_TRANSPORT: &str = "XAAPCAD-GA4SF4J-BNQL3FS-T456WKV-FGLXUKO-CDWHVDW-EYWNRIG-SM5GUAZ";
const ENV: SecureStoreEnvironment = SecureStoreEnvironment::Dev;

struct Fixture {
    source: PathBuf,
    receiver: PathBuf,
    negative: PathBuf,
    store: crate::FakeSecureStore,
    paths: Vec<PathBuf>,
    unapproved: PathBuf,
    report: crate::trusted_import_probe::PreparedTrustedProbe,
}
impl Fixture {
    fn new() -> Self {
        let (source, receiver, negative, store) = crate::trusted_import_probe::tests::pair();
        let report = crate::trusted_import_probe::prepare(
            &source,
            &receiver,
            &negative,
            "44444444-4444-4444-8444-444444444444",
            &store,
        )
        .unwrap();
        let paths: Vec<_> = report
            .segments
            .iter()
            .map(|s| PathBuf::from(&s.relative_path))
            .collect();
        for path in &paths {
            let dest = receiver.join("transport/incoming").join(path);
            std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
            std::fs::copy(source.join("canonical/events").join(path), dest).unwrap();
        }
        let unapproved = PathBuf::from(&report.unapproved_segment.relative_path);
        let destination = receiver.join("transport/incoming").join(&unapproved);
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        std::fs::copy(
            negative.join("canonical/events").join(&unapproved),
            destination,
        )
        .unwrap();
        Self {
            source,
            receiver,
            negative,
            store,
            paths,
            unapproved,
            report,
        }
    }
    fn controller(&self) -> IncomingController {
        let mut controller = IncomingController::new(self.receiver.clone(), ENV).unwrap();
        controller.enqueue(self.paths.clone()).unwrap();
        controller
    }
}

#[test]
fn last_peer_restriction_still_allows_identical_signed_inbox_replay() {
    use crate::bootstrap_journal::{BootstrapJournalStore, RestrictionReason};
    use cryptex_core::DeviceId;
    use cryptex_events::TrustedDaemonKeyResolver;

    for reason in [RestrictionReason::Retired, RestrictionReason::Compromised] {
        let fixture = Fixture::new();
        let mut instance = crate::instance::try_acquire_instance(&fixture.receiver).unwrap();
        let mut controller = fixture.controller();
        let mut ready = ManagedTransportWorker::synthetic_for_incoming_tests(LOCAL_TRANSPORT, true);
        assert!(matches!(
            ready
                .process_incoming(&mut controller, &mut instance, &fixture.store)
                .unwrap(),
            IncomingTick::Published(_)
        ));
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
        let transaction = "bc".repeat(32);
        let approval = policy
            .device_restriction_approval_snapshot(transaction.clone(), &target, reason, 3)
            .unwrap();
        drop(policy);
        let journal = BootstrapJournalStore::new(&fixture.receiver).unwrap();
        journal
            .begin_owner_operation(approval.clone(), "99999999-9999-4999-8999-999999999996")
            .unwrap();
        journal
            .confirm_owner_operation(&transaction, &approval.digest().unwrap())
            .unwrap();
        crate::knowledge_materialization::append_confirmed_owner_security_operation(
            &fixture.receiver,
            &fixture.report.brain_id,
            &fixture.store,
            ENV,
            &transaction,
        )
        .unwrap();
        {
            let resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
                &fixture.receiver,
                &fixture.report.brain_id,
                &fixture.store,
                ENV,
            )
            .unwrap();
            let anchor = resolver.local_trust_anchor().unwrap();
            let snapshot =
                crate::knowledge_materialization::scan_verified_canonical_from_local_anchor_until(
                    &fixture.receiver,
                    &fixture.report.brain_id,
                    &anchor,
                    None,
                )
                .unwrap();
            let active = super::super::exchange_policy::ExchangePeers::from_snapshot(
                &snapshot,
                &fixture.receiver,
                LOCAL_TRANSPORT,
            )
            .unwrap();
            assert!(active.bindings().is_empty());
        }
        controller.enqueue(fixture.paths.clone()).unwrap();
        let IncomingTick::Published(replay) = ready
            .process_incoming(&mut controller, &mut instance, &fixture.store)
            .unwrap()
        else {
            panic!("expected verified historical replay");
        };
        assert_eq!(replay.published_segments, 0);
        assert_eq!(replay.identical_segments, fixture.paths.len());
        ready.shutdown(std::time::Duration::from_secs(2)).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for root in [&self.source, &self.receiver, &self.negative] {
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}

#[test]
fn worker_bridge_waits_for_readiness_then_publishes_and_is_idle_without_new_hints() {
    let fixture = Fixture::new();
    let mut instance = crate::instance::try_acquire_instance(&fixture.receiver).unwrap();
    let mut controller = fixture.controller();
    let mut starting = ManagedTransportWorker::synthetic_for_incoming_tests(LOCAL_TRANSPORT, false);
    assert!(matches!(
        starting
            .process_incoming(&mut controller, &mut instance, &fixture.store)
            .unwrap(),
        IncomingTick::WaitingForTransport
    ));
    assert!(!fixture.receiver.join("canonical/import-staging").exists());
    starting
        .shutdown(std::time::Duration::from_secs(2))
        .unwrap();
    let mut ready = ManagedTransportWorker::synthetic_for_incoming_tests(LOCAL_TRANSPORT, true);
    let exclusive = instance
        .admission()
        .exclusive(
            &fixture.receiver,
            std::time::Instant::now() + std::time::Duration::from_secs(2),
            false,
        )
        .unwrap();
    assert!(matches!(
        ready
            .process_incoming(&mut controller, &mut instance, &fixture.store)
            .unwrap(),
        IncomingTick::Deferred
    ));
    assert!(!fixture.receiver.join("canonical/import-staging").exists());
    exclusive.finish(None);
    let result = ready
        .process_incoming(&mut controller, &mut instance, &fixture.store)
        .unwrap();
    let IncomingTick::Published(report) = result else {
        panic!("expected publication");
    };
    assert_eq!(report.published_segments, fixture.paths.len());
    assert!(matches!(
        ready
            .process_incoming(&mut controller, &mut instance, &fixture.store)
            .unwrap(),
        IncomingTick::Idle
    ));
    controller.enqueue(fixture.paths.clone()).unwrap();
    let IncomingTick::Published(replay) = ready
        .process_incoming(&mut controller, &mut instance, &fixture.store)
        .unwrap()
    else {
        panic!("expected replay");
    };
    assert_eq!(replay.published_segments, 0);
    assert_eq!(replay.identical_segments, fixture.paths.len());
    ready.shutdown(std::time::Duration::from_secs(2)).unwrap();
}

#[test]
fn worker_readiness_cannot_substitute_for_runtime_binding_or_approval() {
    let fixture = Fixture::new();
    let mut instance = crate::instance::try_acquire_instance(&fixture.receiver).unwrap();
    let mut controller = fixture.controller();
    let mut wrong = ManagedTransportWorker::synthetic_for_incoming_tests(OTHER_TRANSPORT, true);
    assert!(matches!(
        wrong.process_incoming(&mut controller, &mut instance, &fixture.store),
        Err(IncomingControllerError::BindingUnavailable)
    ));
    assert!(!fixture.receiver.join("canonical/import-staging").exists());
    wrong.shutdown(std::time::Duration::from_secs(2)).unwrap();
    assert!(controller.pending.is_none());
    let mut ready = ManagedTransportWorker::synthetic_for_incoming_tests(LOCAL_TRANSPORT, true);
    controller
        .enqueue(vec![fixture.unapproved.clone()])
        .unwrap();
    assert!(matches!(
        ready.process_incoming(&mut controller, &mut instance, &fixture.store),
        Err(IncomingControllerError::Publication(
            IncomingPublicationError::Rejected(_)
        ))
    ));
    assert!(
        !fixture
            .receiver
            .join("canonical/events")
            .join(&fixture.unapproved)
            .exists()
    );
    controller.enqueue(fixture.paths.clone()).unwrap();
    let foreign = crate::test_temp::exclusive_temp_dir("incoming-controller-foreign");
    let mut other = crate::instance::try_acquire_instance(&foreign).unwrap();
    assert!(matches!(
        ready.process_incoming(&mut controller, &mut other, &fixture.store),
        Err(IncomingControllerError::WrongRuntime)
    ));
    drop(other);
    std::fs::remove_dir_all(foreign).unwrap();
    // The prior rejected batch has already created the staging parent. Stop
    // must leave that parent empty, rather than requiring it never existed.
    let staging = fixture.receiver.join("canonical/import-staging");
    assert_eq!(std::fs::read_dir(&staging).unwrap().count(), 0);
    ready.request_stop().unwrap();
    assert!(matches!(
        ready
            .process_incoming(&mut controller, &mut instance, &fixture.store)
            .unwrap(),
        IncomingTick::WaitingForTransport
    ));
    assert_eq!(std::fs::read_dir(staging).unwrap().count(), 0);
    ready.shutdown(std::time::Duration::from_secs(2)).unwrap();
}

#[test]
fn pending_batch_survives_local_capture_until_verified_projection_catches_up() {
    let fixture = Fixture::new();
    let mut instance = crate::instance::try_acquire_instance(&fixture.receiver).unwrap();
    let mut controller = fixture.controller();
    let mut worker = ManagedTransportWorker::synthetic_for_incoming_tests(LOCAL_TRANSPORT, true);
    let admission = instance.admission();
    {
        let _permit = admission.enter(&fixture.receiver).unwrap();
        let outcome = crate::capture::capture_memory_note(
            &fixture.store,
            ENV,
            &fixture.receiver,
            crate::capture::CaptureRequest {
                capture_id: "1bfb4034-4f4c-4cd1-8dc9-cd34225dd910".into(),
                classification: cryptex_events::Classification::Public,
                text: "synthetic local acknowledgement".into(),
            },
            &crate::canonical_writer::CanonicalWriterConfig::default(),
        );
        assert!(
            matches!(outcome, crate::capture::CaptureOutcome::Committed(_)),
            "{outcome:?}"
        );
    }
    assert!(matches!(
        worker
            .process_incoming(&mut controller, &mut instance, &fixture.store)
            .unwrap(),
        IncomingTick::Deferred
    ));
    assert_eq!(controller.pending.as_ref(), Some(&fixture.paths));
    {
        let _permit = admission.enter(&fixture.receiver).unwrap();
        crate::incoming_publication::rebuild_imported_state(
            &fixture.receiver,
            &fixture.report.brain_id,
            &fixture.store,
            ENV,
        )
        .unwrap();
    }
    let IncomingTick::Published(report) = worker
        .process_incoming(&mut controller, &mut instance, &fixture.store)
        .unwrap()
    else {
        panic!("unchanged incoming batch was not reconsidered after catch-up");
    };
    assert_eq!(report.published_segments, fixture.paths.len());
    worker.shutdown(std::time::Duration::from_secs(2)).unwrap();
}

#[test]
fn rejected_untrusted_batch_is_not_reprocessed_on_every_worker_poll() {
    let fixture = Fixture::new();
    let mut controller = fixture.controller();
    let path = fixture
        .receiver
        .join("transport/incoming")
        .join(&fixture.paths[0]);
    let mut bytes = std::fs::read(&path).unwrap();
    *bytes.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &bytes).unwrap();
    let mut instance = crate::instance::try_acquire_instance(&fixture.receiver).unwrap();
    let mut worker = ManagedTransportWorker::synthetic_for_incoming_tests(LOCAL_TRANSPORT, true);
    assert!(matches!(
        worker.process_incoming(&mut controller, &mut instance, &fixture.store),
        Err(IncomingControllerError::Publication(
            IncomingPublicationError::Rejected(_)
        ))
    ));
    assert!(controller.pending.is_none());
    assert!(matches!(
        worker
            .process_incoming(&mut controller, &mut instance, &fixture.store)
            .unwrap(),
        IncomingTick::Idle
    ));
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert!(
        !fixture
            .receiver
            .join("canonical/events")
            .join(&fixture.paths[0])
            .exists()
    );
    worker.shutdown(std::time::Duration::from_secs(2)).unwrap();
}

#[test]
fn hints_are_bounded_and_a_runtime_hold_retains_the_batch_for_explicit_repair() {
    let fixture = Fixture::new();
    let mut controller = fixture.controller();
    assert!(controller.enqueue(fixture.paths.clone()).is_ok());
    assert!(matches!(
        controller.enqueue(vec![PathBuf::from("device/other.seg")]),
        Err(IncomingControllerError::QueueBusy)
    ));
    assert!(matches!(
        controller.enqueue(vec![PathBuf::from("../escape.seg")]),
        Err(IncomingControllerError::InvalidHint)
    ));
    assert!(matches!(
        controller.enqueue(vec![PathBuf::from("device/name.seg"); 17]),
        Err(IncomingControllerError::InvalidHint)
    ));
    let mut instance = crate::instance::try_acquire_instance(&fixture.receiver).unwrap();
    let gate = instance.admission();
    gate.exclusive(
        &fixture.receiver,
        std::time::Instant::now() + std::time::Duration::from_secs(1),
        false,
    )
    .unwrap()
    .finish(Some(
        crate::runtime_admission::RuntimeHold::RecoveryRequired,
    ));
    let mut worker = ManagedTransportWorker::synthetic_for_incoming_tests(LOCAL_TRANSPORT, true);
    assert!(matches!(
        worker.process_incoming(&mut controller, &mut instance, &fixture.store),
        Err(IncomingControllerError::Admission(AdmissionError::Held(_)))
    ));
    assert!(controller.pending.is_some());
    crate::incoming_publication::recover_live_with_instance(
        &fixture.receiver,
        &mut instance,
        &fixture.store,
        ENV,
    )
    .unwrap();
    assert!(matches!(
        worker
            .process_incoming(&mut controller, &mut instance, &fixture.store)
            .unwrap(),
        IncomingTick::Published(_)
    ));
    worker.shutdown(std::time::Duration::from_secs(2)).unwrap();
}
