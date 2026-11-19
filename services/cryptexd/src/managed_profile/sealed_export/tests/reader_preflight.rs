use super::*;
use crate::import_staging::READER_CONTROL_FOLDER;
use crate::instance::{InstanceLock, migration::migrate_profile, try_acquire_instance};
use crate::local_device_identity::load_local_device_identity;
use cryptex_events::attachment_link::LinkPayload;
use std::fs;

mod recovery;
pub(in crate::managed_profile) mod transfer;

const BRAIN: &str = "11111111-1111-4111-8111-111111111111";

struct Side {
    root: PathBuf,
    transport: String,
    instance: InstanceLock,
    guard: TransportGuard,
    exporter: Exporter,
}
impl Side {
    fn new(root: PathBuf, transport: &str) -> Self {
        drop(try_acquire_instance(&root).unwrap());
        migrate_profile(&root).unwrap();
        crate::managed_profile::platform::ensure_exchange_tree(&root).unwrap();
        let instance = try_acquire_instance(&root).unwrap();
        let guard = Self::transport_guard(&instance, &root);
        Self {
            root,
            transport: transport.to_owned(),
            instance,
            guard,
            exporter: Exporter::default(),
        }
    }
    fn transport_guard(instance: &InstanceLock, root: &Path) -> TransportGuard {
        let permit = instance
            .admission()
            .exclusive(root, Instant::now() + Duration::from_secs(5), false)
            .unwrap();
        let guard = permit.transport_guard(root).unwrap();
        permit.finish(None);
        guard
    }
    fn tick(&mut self, store: &crate::FakeSecureStore) -> Report {
        self.exporter
            .tick(
                &self.root,
                &self.instance.admission(),
                &self.guard,
                &self.transport,
                (store, ENV),
                &AtomicBool::new(false),
            )
            .unwrap()
    }
    fn controls(&self) -> PathBuf {
        self.root
            .join("transport/incoming")
            .join(READER_CONTROL_FOLDER)
    }
}

/// Add an authentic test-only v2 link to the first signed note. Production
/// has no call path to this writer. Return the untouched active pack path.
fn seed_link(root: &Path, store: &crate::FakeSecureStore) -> PathBuf {
    let id = load_local_device_identity(root, BRAIN).unwrap();
    let device = DeviceId::new(id.device_id).unwrap();
    let path = cryptex_storage::active_segment_path(root, &device, 0);
    let bytes = fs::read(&path).unwrap();
    let (frames, _) = cryptex_storage::scan_complete_frames(&bytes).unwrap();
    let note: cryptex_events::AcceptedEvent =
        serde_json::from_slice(&frames[0][cryptex_events::FRAME_HEADER_BYTES..]).unwrap();
    let tip: serde_json::Value =
        serde_json::from_slice(&frames[1][cryptex_events::FRAME_HEADER_BYTES..]).unwrap();
    let key = crate::event_signing_key::load_existing_event_signing_key(store, ENV, root, &device)
        .unwrap();
    let seq = crate::sequence_runtime::allocate_device_sequence(root, &device).unwrap();
    assert_eq!(seq, 3);
    let link = crate::test_temp::sign_v2_fixture(
        serde_json::json!({
            "schema_version":2, "event_id":"aaaaaaaa-aaaa-4aaa-8111-aaaaaaaaaaaa", "brain_id":BRAIN,
            "device_id":device.as_str(), "device_seq":"3", "created_at":"2026-11-18T00:00:00Z",
            "event_type":"attachment.link", "payload_schema_version":1, "classification":note.classification,
            "signing_key_id":key.public.signing_key_id, "prev_content_hash":tip["content_hash"],
            "payload":LinkPayload::for_note(&note, &Sha256::digest(b"abc").into()).unwrap()
        }),
        &key.signing_key,
    );
    cryptex_storage::append_frame(root, &device, 0, &link).unwrap();
    path
}

#[test]
fn paired_readers_exchange_fresh_proofs_before_attachment_export() {
    use reader::Status;
    let (a, b, negative, store) =
        crate::trusted_import_probe::tests::pair_with_transport(LOCAL, REMOTE);
    let roots = [a.clone(), b.clone(), negative];
    let mut a = Side::new(a, LOCAL);
    let mut b = Side::new(b, REMOTE);
    let pack = seed_link(&a.root, &store);
    let original = fs::read(&pack).unwrap();
    assert_eq!(a.tick(&store).attachment_waiting_for_reader, 1);
    assert_eq!(a.exporter.last_reader_status, Some(Status::Waiting));
    let attempt = std::env::var("CRYPTEX_M9_READER_ATTEMPT").ok();
    let receipt_path = |side: &Side, attempt: &str| {
        side.root
            .join("transport/m9-control-staging")
            .join(format!("{attempt}.receipt"))
    };
    if let Some(id) = attempt.as_deref() {
        assert!(!receipt_path(&a, id).exists());
    }
    assert_eq!(b.tick(&store).copied, 1); // ordinary v1/security traffic continues
    let slot = |side: &Side| {
        fs::read_dir(side.controls())
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .file_name()
    };
    let from_a = slot(&a);
    let from_b = slot(&b);
    assert_ne!(from_a, from_b);
    let send = |src: &Side, dst: &Side, file: &std::ffi::OsStr| {
        fs::copy(src.controls().join(file), dst.controls().join(file)).unwrap();
    };
    send(&a, &b, &from_a);
    b.tick(&store);
    send(&b, &a, &from_b);
    let result = a.tick(&store);
    assert_eq!(
        (
            result.attachment_transfer_disabled,
            result.copied,
            result.sealed
        ),
        (1, 0, 0)
    );
    assert!(
        matches!(&a.exporter.last_reader_status, Some(Status::Verified(builds)) if builds.len()==1)
    );
    send(&a, &b, &from_a);
    b.tick(&store);
    assert!(
        matches!(&b.exporter.last_reader_status, Some(Status::Verified(builds)) if builds.len()==1)
    );
    if let Some(id) = attempt.as_deref() {
        let receipt = |side: &Side| {
            serde_json::from_slice::<serde_json::Value>(&fs::read(receipt_path(side, id)).unwrap())
                .unwrap()
        };
        let (left, right) = (receipt(&a), receipt(&b));
        assert_eq!(left["attempt"], id);
        assert_eq!(right["attempt"], id);
        for (local, peer) in [
            ("local", "peer"),
            ("local_nonce", "peer_nonce"),
            ("local_build", "peer_build"),
        ] {
            assert!(left[local].is_string() && right[local].is_string());
            assert_eq!(left[local], right[peer]);
            assert_eq!(right[local], left[peer]);
        }
    }
    for side in [&a, &b] {
        assert!(
            !side
                .root
                .join("transport/incoming")
                .join(crate::import_staging::ATTACHMENT_FOLDER)
                .exists()
        );
    }
    a.exporter.end_reader_session();
    assert_eq!(a.tick(&store).attachment_waiting_for_reader, 1); // old response still on disk
    let peer_slot = a.controls().join(&from_b);
    let signed = fs::read(&peer_slot).unwrap();
    let mut tampered: serde_json::Value = serde_json::from_slice(&signed).unwrap();
    tampered["body"]["daemon_sha256"] = serde_json::json!("0".repeat(64));
    fs::write(
        &peer_slot,
        serde_json_canonicalizer::to_vec(&tampered).unwrap(),
    )
    .unwrap();
    assert_eq!(a.tick(&store).attachment_waiting_for_reader, 1);
    assert_eq!(a.exporter.last_reader_status, Some(Status::Rejected));
    fs::copy(a.controls().join(&from_a), &peer_slot).unwrap(); // reflected valid local message
    a.tick(&store);
    assert_eq!(a.exporter.last_reader_status, Some(Status::Rejected));
    assert_eq!(original, fs::read(pack).unwrap());
    assert!(!a.root.join("canonical/blobs").exists());
    drop((a, b));
    for root in roots {
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn every_paired_reader_must_answer_the_current_challenge() {
    use super::retirement_relay::{Peer, pair};
    use reader::Status;
    const THIRD: &str = "CCCCCCC-CCCCCCC-CCCCCCC-CCCCCCC-CCCCCCC-CCCCCCC-CCCCCCC-CCCCCCC";
    let store = crate::FakeSecureStore::new();
    let hub = Peer::new("reader-hub", LOCAL, &store);
    let first = Peer::new("reader-first", REMOTE, &store);
    let last = Peer::new("reader-last", THIRD, &store);
    pair(&hub, &first, 1, &store);
    pair(&hub, &last, 2, &store);
    let mut hub = Side::new(hub.root.clone(), LOCAL);
    let mut first = Side::new(first.root.clone(), REMOTE);
    let mut last = Side::new(last.root.clone(), THIRD);
    hub.tick(&store);
    for peer in [&mut first, &mut last] {
        peer.tick(&store);
        for entry in fs::read_dir(hub.controls()).unwrap() {
            let entry = entry.unwrap();
            fs::copy(entry.path(), peer.controls().join(entry.file_name())).unwrap();
        }
        peer.tick(&store);
        for entry in fs::read_dir(peer.controls()).unwrap() {
            let entry = entry.unwrap();
            if !hub.controls().join(entry.file_name()).exists() {
                fs::copy(entry.path(), hub.controls().join(entry.file_name())).unwrap();
            }
        }
        hub.tick(&store);
        if peer.transport == REMOTE {
            assert_eq!(hub.exporter.last_reader_status, Some(Status::Waiting));
        }
    }
    assert!(
        matches!(hub.exporter.last_reader_status, Some(Status::Verified(ref builds)) if builds.len()==2)
    );
}
