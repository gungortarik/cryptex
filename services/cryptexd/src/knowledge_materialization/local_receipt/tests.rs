use super::*;
use std::path::PathBuf;

const RUN: &str = "99999999-9999-4999-8999-999999999999";
const BRAIN: &str = "11111111-1111-4111-8111-111111111111";
const ENV: SecureStoreEnvironment = SecureStoreEnvironment::Dev;

struct Fixture {
    roots: [PathBuf; 3],
    devices: [String; 2],
    store: crate::FakeSecureStore,
}
impl Fixture {
    fn new() -> Self {
        let (a, b, c, store) = crate::trusted_import_probe::tests::pair();
        let roots = [a, b, c];
        let devices = [0, 1].map(|i| {
            load_local_device_identity(&roots[i], BRAIN)
                .unwrap()
                .device_id
        });
        // Verification unit fixture only: same signed records, different pack
        // state on each side. Not network delivery or publication evidence.
        for (source, dest) in [(0, 1), (1, 0)] {
            let device = DeviceId::new(&devices[source]).unwrap();
            let bytes = fs::read(active_segment_path(&roots[source], &device, 0)).unwrap();
            let _lock =
                crate::canonical_writer::acquire_device_canonical_lock(&roots[dest], &device)
                    .unwrap();
            fs::write(sealed_segment_path(&roots[dest], &device, 0), bytes).unwrap();
        }
        Self {
            roots,
            devices,
            store,
        }
    }
    fn request(&self, side: usize) -> Request<'_> {
        Request {
            run_id: RUN,
            brain_id: BRAIN,
            local_device_id: &self.devices[side],
            peer_device_id: &self.devices[1 - side],
        }
    }
    fn receipt(&self, side: usize) -> Result<Receipt, Error> {
        inspect(&self.roots[side], &self.store, ENV, &self.request(side))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for root in &self.roots {
            fs::remove_dir_all(root).unwrap();
        }
    }
}

#[test]
fn equal_signed_sets_ignore_pack_state_and_do_not_claim_remote_protection() {
    let f = Fixture::new();
    let a = f.receipt(0).unwrap();
    let b = f.receipt(1).unwrap();
    assert_eq!(a.canonical_set_sha256, b.canonical_set_sha256);
    assert_eq!(a.canonical_records, 4); // includes BOTH security approvals
    assert_eq!(a.canonical_devices, 2);
    assert_ne!(a.local_device_id, b.local_device_id);
    assert!(a.signatures_and_chains_verified);
    assert!(
        !a.network_delivery_verified
            && !a.remote_durability_verified
            && !a.derived_results_verified
    );
    let output = serde_json::to_string(&a).unwrap();
    assert!(!output.contains("Synthetic"));
    assert!(!output.contains("payload"));
    assert_eq!(
        a.canonical_set_sha256,
        f.receipt(0).unwrap().canonical_set_sha256
    );
    let local = DeviceId::new(&f.devices[0]).unwrap();
    assert!(
        active_segment_path(&f.roots[0], &local, 0).exists(),
        "verification never seals"
    );
}

#[test]
fn new_active_record_changes_digest_without_sealing_or_rebuilding_projection() {
    let f = Fixture::new();
    let before = f.receipt(0).unwrap();
    crate::canonical_writer::append_memory_note(
        &f.store,
        ENV,
        crate::canonical_writer::AppendMemoryNoteArgs {
            runtime_root: &f.roots[0],
            brain_id: BRAIN,
            device_id: &DeviceId::new(&f.devices[0]).unwrap(),
            classification: cryptex_events::Classification::Public,
            text: "new private test body",
            event_id: None,
            config: &crate::canonical_writer::CanonicalWriterConfig::default(),
        },
    )
    .unwrap();
    let after = f.receipt(0).unwrap();
    assert_eq!(after.canonical_records, before.canonical_records + 1);
    assert_ne!(after.canonical_set_sha256, before.canonical_set_sha256);
    assert_eq!(
        f.receipt(1).unwrap().canonical_set_sha256,
        before.canonical_set_sha256
    );
}

#[test]
fn live_daemon_exclusion_is_required() {
    let f = Fixture::new();
    let lock = crate::instance::try_acquire_instance(&f.roots[0]).unwrap();
    assert!(matches!(f.receipt(0), Err(Error::RunningOrUnavailable)));
    drop(lock);
    f.receipt(0).unwrap();
}

#[test]
fn empty_approved_peer_slot_is_not_received_history() {
    let f = Fixture::new();
    let peer = DeviceId::new(&f.devices[1]).unwrap();
    fs::remove_file(sealed_segment_path(&f.roots[0], &peer, 0)).unwrap();
    assert!(matches!(f.receipt(0), Err(Error::PeerUnavailable)));
}

#[test]
fn pending_import_or_policy_conflict_is_preserved_and_refused() {
    let f = Fixture::new();
    for leaf in ["import-staging", "import-journal"] {
        let dir = f.roots[0].join("canonical").join(leaf);
        fs::create_dir_all(&dir).unwrap();
        let marker = dir.join("unresolved.policy-held.json");
        fs::write(&marker, b"retained evidence").unwrap();
        assert!(matches!(f.receipt(0), Err(Error::RecoveryRequired)));
        assert_eq!(fs::read(&marker).unwrap(), b"retained evidence");
        fs::remove_file(marker).unwrap();
    }
    f.receipt(0).unwrap();
}

#[test]
fn wrong_run_identity_or_unapproved_peer_cannot_receive_success() {
    let f = Fixture::new();
    let mut request = f.request(0);
    request.run_id = "not-a-run";
    assert!(matches!(
        inspect(&f.roots[0], &f.store, ENV, &request),
        Err(Error::Arguments)
    ));
    request = f.request(1);
    assert!(matches!(
        inspect(&f.roots[0], &f.store, ENV, &request),
        Err(Error::IdentityMismatch)
    ));
    request = f.request(0);
    request.peer_device_id = RUN;
    assert!(matches!(
        inspect(&f.roots[0], &f.store, ENV, &request),
        Err(Error::PeerUnavailable)
    ));
}

#[test]
fn missing_os_key_is_not_replaced() {
    let f = Fixture::new();
    let empty = crate::FakeSecureStore::new();
    assert!(matches!(
        inspect(&f.roots[0], &empty, ENV, &f.request(0)),
        Err(Error::KeyUnavailable)
    ));
    f.receipt(0).unwrap();
}

#[test]
fn corrupted_canonical_bytes_are_not_repaired_or_reported_as_verified() {
    let f = Fixture::new();
    let path = active_segment_path(&f.roots[0], &DeviceId::new(&f.devices[0]).unwrap(), 0);
    let mut bytes = fs::read(&path).unwrap();
    let index = bytes
        .windows(9)
        .position(|value| value == b"Synthetic")
        .unwrap();
    bytes[index] ^= 1;
    fs::write(&path, &bytes).unwrap();
    assert!(matches!(f.receipt(0), Err(Error::HistoryUnavailable)));
    assert_eq!(fs::read(&path).unwrap(), bytes);
}
