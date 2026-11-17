use super::*;
use crate::managed_profile::worker::ManagedTransportWorker;
use std::fs;

const LOCAL: &str = "U2SYIGU-YWZX757-2WPZGCR-DHD7MLH-ZKV7HLJ-EYO6CXE-K3DU3DU-SDFXYAE";
const ENV: SecureStoreEnvironment = SecureStoreEnvironment::Dev;

struct Fixture {
    source: PathBuf,
    receiver: PathBuf,
    negative: PathBuf,
    store: Arc<crate::FakeSecureStore>,
    report: crate::trusted_import_probe::PreparedTrustedProbe,
    tail: PathBuf,
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
        let device = cryptex_core::DeviceId::new(&report.source_device).unwrap();
        let note = crate::canonical_writer::append_memory_note(
            &store,
            ENV,
            crate::canonical_writer::AppendMemoryNoteArgs {
                runtime_root: &source,
                brain_id: &report.brain_id,
                device_id: &device,
                classification: cryptex_events::Classification::Public,
                text: "test 3",
                event_id: None,
                config: &crate::canonical_writer::CanonicalWriterConfig {
                    seal_event_threshold: 1,
                },
            },
        )
        .unwrap();
        assert!(note.sealed);
        let tail = PathBuf::from(device.as_filename_stem())
            .join(format!("{:020}.seg", note.segment_index));
        Self {
            source,
            receiver,
            negative,
            store: Arc::new(store),
            report,
            tail,
        }
    }
    fn copy(&self, root: &Path, relative: &Path) {
        let target = self.receiver.join("transport/incoming").join(relative);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(root.join("canonical/events").join(relative), target).unwrap();
    }
    fn deliver(&self) {
        for segment in &self.report.segments {
            self.copy(&self.source, Path::new(&segment.relative_path));
        }
        self.copy(&self.source, &self.tail);
    }
    fn controller(&self) -> Controller {
        let instance = crate::instance::try_acquire_instance(&self.receiver).unwrap();
        let admission = instance.admission();
        let worker = ManagedTransportWorker::synthetic_for_incoming_tests(LOCAL, true);
        let reader = worker.status_reader();
        Controller {
            root: self.receiver.clone(),
            admission,
            store: self.store.clone(),
            environment: ENV,
            location: None,
            registry: crate::approval_session::ApprovalSessionRegistry::default(),
            recovery_owner: Some(Arc::new(Mutex::new(instance))),
            settings: None,
            preferences: Preferences::default(),
            worker: Some(worker),
            guard: None,
            reader: Arc::new(Mutex::new(Some(reader))),
            cleanup_failed: false,
            exporter: crate::managed_profile::sealed_export::Exporter::default(),
            next_export: Instant::now() + Duration::from_secs(3600),
            incoming: None,
            next_incoming: Instant::now(),
            identity_only: false,
            stop: Arc::new(AtomicBool::new(false)),
            view: View {
                revision: "0".into(),
                paused: false,
                listen_address: None,
                local_transport_port: None,
                devices: Vec::new(),
                state: State::ReadyLocally,
                error: None,
            },
        }
    }
    fn contains_note(&self) -> bool {
        let db = cryptex_projection::ProjectionDb::open_existing_read_only(&self.receiver).unwrap();
        let mut resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
            &self.receiver,
            &self.report.brain_id,
            self.store.as_ref(),
            ENV,
        )
        .unwrap();
        let safety = crate::knowledge_materialization::evaluate_retrieval_safety(
            &self.receiver,
            &self.report.brain_id,
            &[],
            &mut resolver,
            &db,
        );
        assert!(safety.allows_content(), "{:?}", safety.hold_reason());
        cryptex_projection::search::search_events(db.connection(), &Default::default())
            .unwrap()
            .iter()
            .any(|hit| hit.text == "test 3")
    }
    fn canonical_bytes(&self) -> Vec<(PathBuf, Vec<u8>)> {
        let mut files = Vec::new();
        for dir in fs::read_dir(self.receiver.join("canonical/events")).unwrap() {
            let dir = dir.unwrap();
            if !dir.file_type().unwrap().is_dir() {
                continue; // Inventory coordination files are not device streams.
            }
            for file in fs::read_dir(dir.path()).unwrap() {
                let path = file.unwrap().path();
                if path
                    .extension()
                    .is_some_and(|ext| ext == "seg" || ext == "open")
                {
                    files.push((path.clone(), fs::read(path).unwrap()));
                }
            }
        }
        files.sort();
        files
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for root in [&self.source, &self.receiver, &self.negative] {
            fs::remove_dir_all(root).unwrap();
        }
    }
}
fn tick(controller: &mut Controller) {
    controller.next_incoming = Instant::now();
    controller.check_generation(); // Same routine called by normal Owner's timer and Read route.
}

#[test]
fn normal_tick_discovers_peer_note_and_restart_preserves_search_without_duplicate_append() {
    let fixture = Fixture::new();
    let before = fixture.canonical_bytes();
    assert!(!fixture.contains_note());
    let mut controller = fixture.controller();
    fixture.deliver(); // Simulates transport bytes only; no enqueue/publish shortcut.
    tick(&mut controller);
    assert_eq!(controller.view.error, None);
    assert!(
        fixture.contains_note(),
        "normal Windows timer must import and rebuild search"
    );
    for (path, bytes) in before {
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
    let committed = fixture.canonical_bytes();
    assert_eq!(
        fs::read(fixture.source.join("canonical/events").join(&fixture.tail)).unwrap(),
        fs::read(
            fixture
                .receiver
                .join("canonical/events")
                .join(&fixture.tail)
        )
        .unwrap()
    );
    tick(&mut controller);
    assert_eq!(fixture.canonical_bytes(), committed);
    controller.close().unwrap();
    drop(controller);
    let mut restarted = fixture.controller();
    restarted.view.state = State::Paused;
    tick(&mut restarted);
    assert!(fixture.contains_note());
    restarted.view.state = State::ReadyLocally;
    tick(&mut restarted); // Rediscovered copies must be verified identical replay.
    assert_eq!(restarted.view.error, None);
    assert_eq!(fixture.canonical_bytes(), committed);
    assert!(fixture.contains_note());
    restarted.close().unwrap();
}

#[test]
fn normal_tick_defers_during_pause_exclusive_work_and_shutdown() {
    let fixture = Fixture::new();
    let mut controller = fixture.controller();
    fixture.deliver();
    let before = fixture.canonical_bytes();
    controller.view.state = State::Paused;
    tick(&mut controller);
    assert!(controller.incoming.is_none());
    controller.view.state = State::ReadyLocally;
    let exclusive = controller
        .admission
        .exclusive(
            &fixture.receiver,
            Instant::now() + Duration::from_secs(2),
            false,
        )
        .unwrap();
    tick(&mut controller);
    assert!(controller.incoming.is_none());
    exclusive.finish(None);
    controller.stop.store(true, Ordering::Release);
    tick(&mut controller);
    assert!(controller.incoming.is_none());
    assert_eq!(fixture.canonical_bytes(), before);
    controller.stop.store(false, Ordering::Release);
    tick(&mut controller);
    assert!(fixture.contains_note());
    controller.close().unwrap();
}

#[test]
fn normal_tick_preserves_rejected_bytes_and_accepts_later_trusted_delivery() {
    let fixture = Fixture::new();
    let mut controller = fixture.controller();
    let bad = Path::new(&fixture.report.unapproved_segment.relative_path);
    fixture.copy(&fixture.negative, bad);
    let before = fixture.canonical_bytes();
    let untrusted = fixture.receiver.join("transport/incoming").join(bad);
    let original = fs::read(&untrusted).unwrap();
    tick(&mut controller);
    assert_eq!(controller.view.error, None);
    assert_eq!(fixture.canonical_bytes(), before);
    assert_eq!(fs::read(&untrusted).unwrap(), original);
    tick(&mut controller);
    assert_eq!(fixture.canonical_bytes(), before);
    fixture.deliver();
    tick(&mut controller);
    assert!(fixture.contains_note());
    assert!(!fixture.receiver.join("canonical/events").join(bad).exists());
    controller.close().unwrap();
}

#[test]
fn normal_tick_cannot_clear_a_policy_hold_or_import_during_it() {
    let fixture = Fixture::new();
    let mut controller = fixture.controller();
    fixture.deliver();
    let before = fixture.canonical_bytes();
    controller
        .admission
        .exclusive(
            &fixture.receiver,
            Instant::now() + Duration::from_secs(2),
            false,
        )
        .unwrap()
        .finish(Some(
            crate::runtime_admission::RuntimeHold::IncomingPolicyConflict,
        ));
    tick(&mut controller);
    assert_eq!(controller.view.error, Some(Error::Held));
    assert!(controller.incoming.is_none());
    assert_eq!(fixture.canonical_bytes(), before);
    assert!(matches!(
        controller.admission.enter(&fixture.receiver),
        Err(AdmissionError::Held(
            crate::runtime_admission::RuntimeHold::IncomingPolicyConflict
        ))
    ));
    controller.close().unwrap();
}
