use super::*;
use crate::canonical_writer::{AppendMemoryNoteArgs, CanonicalWriterConfig, append_memory_note};
use cryptex_core::DeviceId;
use cryptex_events::envelope::Classification;
use std::path::PathBuf;

const LOCAL: &str = "U2SYIGU-YWZX757-2WPZGCR-DHD7MLH-ZKV7HLJ-EYO6CXE-K3DU3DU-SDFXYAE";
const REMOTE: &str = "XAAPCAD-GA4SF4J-BNQL3FS-T456WKV-FGLXUKO-CDWHVDW-EYWNRIG-SM5GUAZ";
const ENV: SecureStoreEnvironment = SecureStoreEnvironment::Dev;

struct Fixture {
    roots: [PathBuf; 3],
    store: crate::FakeSecureStore,
    instance: crate::instance::InstanceLock,
    guard: TransportGuard,
    device: DeviceId,
}
impl Fixture {
    fn new() -> Self {
        let (a, b, c, store) =
            crate::trusted_import_probe::tests::pair_with_transport(LOCAL, REMOTE);
        let brain = crate::identity_runtime::load_brain_identity(&a).unwrap();
        let resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
            &a,
            &brain.brain_id,
            &store,
            ENV,
        )
        .unwrap();
        let device = resolver.local_trust_anchor().unwrap().device_id().clone();
        super::super::platform::ensure_exchange_tree(&a).unwrap();
        let instance = crate::instance::try_acquire_instance(&a).unwrap();
        let permit = instance
            .admission()
            .exclusive(&a, Instant::now() + Duration::from_secs(5), false)
            .unwrap();
        let guard = permit.transport_guard(&a).unwrap();
        permit.finish(None);
        Self {
            roots: [a, b, c],
            store,
            instance,
            guard,
            device,
        }
    }
    fn run(&self, exporter: &mut Exporter) -> Result<Report, Error> {
        exporter.tick(
            &self.roots[0],
            &self.instance.admission(),
            &self.guard,
            LOCAL,
            (&self.store, ENV),
            &AtomicBool::new(false),
        )
    }
    fn canonical(&self, index: u64, extension: &str) -> PathBuf {
        self.roots[0]
            .join("canonical/events")
            .join(self.device.as_filename_stem())
            .join(format!("{index:020}.{extension}"))
    }
    fn exchange(&self, index: u64) -> PathBuf {
        self.roots[0]
            .join("transport/incoming")
            .join(self.device.as_filename_stem())
            .join(format!("{index:020}.seg"))
    }
    fn append(&self, seal: bool) {
        let brain = crate::identity_runtime::load_brain_identity(&self.roots[0]).unwrap();
        append_memory_note(
            &self.store,
            ENV,
            AppendMemoryNoteArgs {
                runtime_root: &self.roots[0],
                brain_id: &brain.brain_id,
                device_id: &self.device,
                classification: Classification::Public,
                text: "export checkpoint test",
                event_id: None,
                config: &CanonicalWriterConfig {
                    seal_event_threshold: if seal { 1 } else { 1000 },
                },
            },
        )
        .unwrap();
    }
    fn cleanup(self) {
        let Self {
            roots,
            instance,
            guard,
            ..
        } = self;
        drop(guard);
        drop(instance);
        for root in roots {
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}

#[test]
fn seals_local_tail_copies_exact_bytes_replays_and_next_append_keeps_sequence() {
    let fixture = Fixture::new();
    let original = std::fs::read(fixture.canonical(0, "open")).unwrap();
    let mut exporter = Exporter::default();
    let first = fixture.run(&mut exporter).unwrap();
    assert_eq!((first.sealed, first.copied), (1, 1));
    assert!(!fixture.canonical(0, "open").exists());
    assert_eq!(
        std::fs::read(fixture.canonical(0, "seg")).unwrap(),
        original
    );
    assert_eq!(std::fs::read(fixture.exchange(0)).unwrap(), original);
    let again = fixture.run(&mut Exporter::default()).unwrap();
    assert_eq!((again.sealed, again.copied, again.present), (0, 0, 1));
    fixture.append(false);
    assert!(fixture.canonical(1, "open").exists());
    assert_eq!(fixture.run(&mut exporter).unwrap().copied, 1);
    assert_eq!(
        std::fs::read(fixture.canonical(0, "seg")).unwrap(),
        original
    );
    let brain = crate::identity_runtime::load_brain_identity(&fixture.roots[0]).unwrap();
    let resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
        &fixture.roots[0],
        &brain.brain_id,
        &fixture.store,
        ENV,
    )
    .unwrap();
    let snapshot =
        crate::knowledge_materialization::scan_verified_canonical_from_local_anchor_until(
            &fixture.roots[0],
            &brain.brain_id,
            &resolver.local_trust_anchor().unwrap(),
            None,
        )
        .unwrap();
    assert_eq!(snapshot.export_packs(|| true).unwrap().len(), 2);
    drop(snapshot);
    fixture.cleanup();
}

#[test]
fn interruption_at_seal_partial_copy_flush_and_promotion_reopens_idempotently() {
    for point in 0..4 {
        let fixture = Fixture::new();
        let original = std::fs::read(fixture.canonical(0, "open")).unwrap();
        FAULT.with(|f| f.set(Some(point)));
        assert_eq!(
            fixture.run(&mut Exporter::default()).unwrap_err(),
            Error::Io(std::io::ErrorKind::StorageFull, None)
        );
        fixture.run(&mut Exporter::default()).unwrap();
        assert_eq!(std::fs::read(fixture.exchange(0)).unwrap(), original);
        assert_eq!(
            std::fs::read(fixture.canonical(0, "seg")).unwrap(),
            original
        );
        assert!(
            !fixture.roots[0]
                .join("transport/export-staging/segment.tmp")
                .exists()
        );
        fixture.cleanup();
    }
}

#[test]
fn divergent_exchange_bytes_are_never_replaced_and_canonical_is_unchanged() {
    let fixture = Fixture::new();
    fixture.run(&mut Exporter::default()).unwrap();
    let original = std::fs::read(fixture.canonical(0, "seg")).unwrap();
    std::fs::write(fixture.exchange(0), b"conflicting transport bytes").unwrap();
    assert_eq!(
        fixture.run(&mut Exporter::default()).unwrap_err(),
        Error::DestinationConflict
    );
    assert_eq!(
        std::fs::read(fixture.exchange(0)).unwrap(),
        b"conflicting transport bytes"
    );
    assert_eq!(
        std::fs::read(fixture.canonical(0, "seg")).unwrap(),
        original
    );
    fixture.cleanup();
}

#[test]
fn deleted_exchange_copy_is_repopulated_and_hardlink_alias_is_refused() {
    let fixture = Fixture::new();
    fixture.run(&mut Exporter::default()).unwrap();
    std::fs::remove_file(fixture.exchange(0)).unwrap();
    assert_eq!(fixture.run(&mut Exporter::default()).unwrap().copied, 1);
    std::fs::remove_file(fixture.exchange(0)).unwrap();
    std::fs::hard_link(fixture.canonical(0, "seg"), fixture.exchange(0)).unwrap();
    assert!(fixture.run(&mut Exporter::default()).is_err());
    fixture.cleanup();
}

#[test]
fn bounded_cursor_advances_beyond_first_sixteen_present_packs_and_revocation_stops_export() {
    let fixture = Fixture::new();
    for _ in 0..20 {
        fixture.append(true);
    }
    let mut exporter = Exporter::default();
    assert_eq!(fixture.run(&mut exporter).unwrap().copied, 16);
    assert_eq!(fixture.run(&mut exporter).unwrap().copied, 4);
    assert_eq!(fixture.run(&mut exporter).unwrap().present, 16);
    let permit = fixture
        .instance
        .admission()
        .exclusive(
            &fixture.roots[0],
            Instant::now() + Duration::from_secs(5),
            false,
        )
        .unwrap();
    permit.finish(None);
    assert_eq!(fixture.run(&mut exporter).unwrap_err(), Error::Deferred);
    fixture.cleanup();
}
