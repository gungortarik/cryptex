use super::*;
use crate::canonical_writer::{AppendMemoryNoteArgs, CanonicalWriterConfig, append_memory_note};
use cryptex_core::DeviceId;
use cryptex_events::envelope::Classification;
use std::path::PathBuf;

const LOCAL: &str = "U2SYIGU-YWZX757-2WPZGCR-DHD7MLH-ZKV7HLJ-EYO6CXE-K3DU3DU-SDFXYAE";
const REMOTE: &str = "XAAPCAD-GA4SF4J-BNQL3FS-T456WKV-FGLXUKO-CDWHVDW-EYWNRIG-SM5GUAZ";
const ENV: SecureStoreEnvironment = SecureStoreEnvironment::Dev;

thread_local! {
    static RACING_DELIVERY: std::cell::RefCell<Option<(PathBuf, Vec<u8>)>> = const { std::cell::RefCell::new(None) };
}

pub(super) fn publish_racing_delivery() {
    if let Some((destination, bytes)) = RACING_DELIVERY.with(|race| race.borrow_mut().take()) {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)
            .unwrap();
        file.write_all(&bytes).unwrap();
        file.sync_all().unwrap();
    }
}

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
    assert!(matches!(
        snapshot.export_packs(|| false),
        Err(crate::knowledge_materialization::KnowledgeMaterializationError::DeadlineExceeded)
    ));
    drop(snapshot);
    fixture.cleanup();
}

#[test]
fn signed_cutoff_withholds_whole_peer_pack_without_starving_another_eligible_peer() {
    use cryptex_events::{BootstrapApprovalRole, ContentHashHex, DeviceSeq, SigningKeyId};
    let fixture = Fixture::new();
    let root = &fixture.roots[0];
    let brain = crate::identity_runtime::load_brain_identity(root).unwrap();
    let third_root = &fixture.roots[2];
    crate::identity_runtime::write_brain_identity(third_root, &brain).unwrap();
    let third =
        crate::local_device_identity::enroll_local_device(third_root, &fixture.store, ENV).unwrap();
    let third = DeviceId::new(third.device_id).unwrap();
    let local_key = crate::event_signing_key::load_existing_event_signing_key(
        &fixture.store,
        ENV,
        root,
        &fixture.device,
    )
    .unwrap();
    let third_key = crate::event_signing_key::load_existing_event_signing_key(
        &fixture.store,
        ENV,
        third_root,
        &third,
    )
    .unwrap();
    let other_transport =
        "A".repeat(7) + "-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA";
    let offer = cryptex_events::create_bootstrap_offer(
        &brain.brain_id,
        &"b".repeat(64),
        &fixture.device,
        LOCAL,
        &[3; 32],
        &local_key.signing_key,
    )
    .unwrap();
    let proof = cryptex_events::create_bootstrap_proof(
        &offer,
        &third,
        &other_transport,
        &[4; 32],
        &third_key.signing_key,
    )
    .unwrap();
    let request = cryptex_events::verify_bootstrap_approval_request(
        &[&offer, &proof],
        BootstrapApprovalRole::Initiator,
    )
    .unwrap();
    let first = cryptex_events::sign_bootstrap_approval(&request, &local_key.signing_key).unwrap();
    let request = cryptex_events::verify_bootstrap_approval_request(
        &[&offer, &proof, &first],
        BootstrapApprovalRole::Responder,
    )
    .unwrap();
    let last = cryptex_events::sign_bootstrap_approval(&request, &third_key.signing_key).unwrap();
    let transcript =
        cryptex_events::verify_bootstrap_transcript(&[&offer, &proof, &first, &last]).unwrap();
    let original = std::fs::read(fixture.canonical(0, "open")).unwrap();
    let (frames, complete) = cryptex_storage::segment_io::scan_complete_frames(&original).unwrap();
    assert_eq!(complete, original.len());
    let tail: serde_json::Value = serde_json::from_slice(
        &frames.last().unwrap()[cryptex_events::framing::FRAME_HEADER_BYTES..],
    )
    .unwrap();
    let approval = cryptex_events::sign_peer_approved_event(
        &transcript,
        "99999999-9999-4999-8999-999999999991",
        DeviceSeq::from_u64(3).unwrap(),
        "2026-11-16T00:00:00Z",
        &SigningKeyId::parse(&local_key.public.signing_key_id).unwrap(),
        &ContentHashHex::parse(tail["content_hash"].as_str().unwrap()).unwrap(),
        &local_key.signing_key,
    )
    .unwrap();
    assert_eq!(
        crate::sequence_runtime::allocate_device_sequence(root, &fixture.device).unwrap(),
        3
    );
    cryptex_storage::append_frame(root, &fixture.device, 0, &approval).unwrap();

    let resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
        &fixture.roots[1],
        &brain.brain_id,
        &fixture.store,
        ENV,
    )
    .unwrap();
    let peer = resolver.local_trust_anchor().unwrap().device_id().clone();
    let peer_key = crate::event_signing_key::load_existing_event_signing_key(
        &fixture.store,
        ENV,
        &fixture.roots[1],
        &peer,
    )
    .unwrap();
    let peer_bytes = std::fs::read(cryptex_storage::active_segment_path(
        &fixture.roots[1],
        &peer,
        0,
    ))
    .unwrap();
    let (peer_frames, complete) =
        cryptex_storage::segment_io::scan_complete_frames(&peer_bytes).unwrap();
    assert_eq!(complete, peer_bytes.len());
    let boundary: serde_json::Value =
        serde_json::from_slice(&peer_frames[0][cryptex_events::framing::FRAME_HEADER_BYTES..])
            .unwrap();
    let tail: serde_json::Value = serde_json::from_slice(&approval).unwrap();
    let restriction = cryptex_events::security_v3::sign_device_restricted_event(
        "99999999-9999-4999-8999-999999999992",
        &brain.brain_id,
        &fixture.device,
        DeviceSeq::from_u64(4).unwrap(),
        "2026-11-16T00:00:01Z",
        &SigningKeyId::parse(&local_key.public.signing_key_id).unwrap(),
        &ContentHashHex::parse(tail["content_hash"].as_str().unwrap()).unwrap(),
        &"c".repeat(64),
        &peer,
        &SigningKeyId::parse(&peer_key.public.signing_key_id).unwrap(),
        &peer_key.signing_key.verifying_key().to_bytes(),
        cryptex_events::security_v3::RestrictionReasonV3::Compromised,
        1,
        &ContentHashHex::parse(boundary["content_hash"].as_str().unwrap()).unwrap(),
        &local_key.signing_key,
    )
    .unwrap();
    assert_eq!(
        crate::sequence_runtime::allocate_device_sequence(root, &fixture.device).unwrap(),
        4
    );
    cryptex_storage::append_frame(root, &fixture.device, 0, &restriction).unwrap();
    // Fixture-only complete signed canonical peer pack, deliberately crossing
    // the now effective cutoff. Product exporter never imports or fabricates it.
    let lock = crate::canonical_writer::acquire_device_canonical_lock(root, &peer).unwrap();
    std::fs::write(
        cryptex_storage::sealed_segment_path(root, &peer, 0),
        &peer_bytes,
    )
    .unwrap();
    drop(lock);
    let report = fixture.run(&mut Exporter::default()).unwrap();
    assert_eq!((report.copied, report.withheld), (1, 1));
    assert!(fixture.exchange(0).exists());
    assert!(
        !root
            .join("transport/incoming")
            .join(peer.as_filename_stem())
            .exists()
    );
    assert_eq!(
        std::fs::read(cryptex_storage::sealed_segment_path(root, &peer, 0)).unwrap(),
        peer_bytes
    );
    let again = fixture.run(&mut Exporter::default()).unwrap();
    assert_eq!((again.present, again.withheld), (1, 1));
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
fn racing_delivery_is_verified_without_replacing_the_winner() {
    for identical in [true, false] {
        let fixture = Fixture::new();
        let original = std::fs::read(fixture.canonical(0, "open")).unwrap();
        let winner = if identical {
            original.clone()
        } else {
            b"conflicting racing delivery".to_vec()
        };
        RACING_DELIVERY.with(|race| {
            *race.borrow_mut() = Some((fixture.exchange(0), winner.clone()));
        });
        let result = fixture.run(&mut Exporter::default());
        assert!(RACING_DELIVERY.with(|race| race.borrow().is_none()));
        if identical {
            let report = result.unwrap();
            assert_eq!((report.copied, report.present), (0, 1));
            assert!(
                !fixture.roots[0]
                    .join("transport/export-staging/segment.tmp")
                    .exists()
            );
        } else {
            assert_eq!(result.unwrap_err(), Error::DestinationConflict);
        }
        assert_eq!(std::fs::read(fixture.exchange(0)).unwrap(), winner);
        assert_eq!(
            std::fs::read(fixture.canonical(0, "seg")).unwrap(),
            original
        );
        fixture.cleanup();
    }
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
