use super::*;
use crate::bootstrap_journal::{BootstrapJournalStore, BootstrapOperation};
use crate::incoming_publication::{publish_incoming_with_instance, recover_live_with_instance};
use crate::instance::{InstanceLock, try_acquire_instance};
use crate::knowledge_materialization as km;
use crate::retirement_notice as handoff;
use cryptex_events::BootstrapApprovalRole;
use std::fs;

const BRAIN: &str = "11111111-1111-4111-8111-111111111111";
struct Peer {
    root: PathBuf,
    device: DeviceId,
    transport: String,
    key: ed25519_dalek::SigningKey,
}
impl Peer {
    fn new(name: &str, transport: &str, store: &crate::FakeSecureStore) -> Self {
        let root = crate::test_temp::exclusive_temp_dir(name);
        let (device, _) = crate::trusted_import_probe::tests::local(&root, BRAIN, store);
        let key =
            crate::event_signing_key::load_existing_event_signing_key(store, ENV, &root, &device)
                .unwrap()
                .signing_key;
        crate::managed_profile::platform::ensure_exchange_tree(&root).unwrap();
        Self {
            root,
            device,
            transport: transport.into(),
            key,
        }
    }
    fn reopen(&self, store: &crate::FakeSecureStore) -> InstanceLock {
        let mut instance = try_acquire_instance(&self.root).unwrap();
        recover_live_with_instance(&self.root, &mut instance, store, ENV).unwrap();
        instance
    }
    fn tick(&self, owner: &InstanceLock, store: &crate::FakeSecureStore) -> Result<Report, Error> {
        let admission = owner.admission();
        let permit = admission
            .exclusive(&self.root, Instant::now() + Duration::from_secs(10), false)
            .unwrap();
        let guard = permit.transport_guard(&self.root).unwrap();
        permit.finish(None);
        Exporter::default().tick(
            &self.root,
            &admission,
            &guard,
            &self.transport,
            (store, ENV),
            &AtomicBool::new(false),
        )
    }
    fn cutoff(&self, author: &DeviceId, store: &crate::FakeSecureStore) -> u64 {
        let resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
            &self.root, BRAIN, store, ENV,
        )
        .unwrap();
        let snapshot = km::scan_verified_canonical_from_local_anchor_until(
            &self.root,
            BRAIN,
            &resolver.local_trust_anchor().unwrap(),
            None,
        )
        .unwrap();
        snapshot
            .verified_trust_context()
            .unwrap()
            .1
            .effective_cutoffs[author]
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn pair(a: &Peer, b: &Peer, ordinal: u8, store: &crate::FakeSecureStore) {
    let tx = format!("{ordinal:064x}");
    let offer = cryptex_events::create_bootstrap_offer(
        BRAIN,
        &tx,
        &a.device,
        &a.transport,
        &[ordinal; 32],
        &a.key,
    )
    .unwrap();
    let proof = cryptex_events::create_bootstrap_proof(
        &offer,
        &b.device,
        &b.transport,
        &[ordinal + 1; 32],
        &b.key,
    )
    .unwrap();
    let ar = cryptex_events::verify_bootstrap_approval_request(
        &[&offer, &proof],
        BootstrapApprovalRole::Initiator,
    )
    .unwrap();
    let first = cryptex_events::sign_bootstrap_approval(&ar, &a.key).unwrap();
    let br = cryptex_events::verify_bootstrap_approval_request(
        &[&offer, &proof, &first],
        BootstrapApprovalRole::Responder,
    )
    .unwrap();
    let last = cryptex_events::sign_bootstrap_approval(&br, &b.key).unwrap();
    let messages = vec![offer, proof, first, last];
    for (peer, request, prefix) in [(a, ar, 2), (b, br, 3)] {
        let journal = BootstrapJournalStore::new(&peer.root).unwrap();
        let id = format!("33333333-3333-4333-8333-{ordinal:010x}{prefix:02x}");
        journal
            .begin_verified_approval_prefix(&request, &messages[..prefix], &id)
            .unwrap();
        journal
            .record_local_approval_signature(&request, &messages[prefix])
            .unwrap();
        journal
            .record_verified_complete_transcript(&tx, &messages)
            .unwrap();
        km::append_local_peer_approval_anchor(&peer.root, BRAIN, store, ENV, &tx, &messages)
            .unwrap();
    }
}

fn deliver_exchange(sender: &Peer, receiver: &Peer, path: &Path) {
    let src = sender.root.join("transport/incoming").join(path);
    let dst = receiver.root.join("transport/incoming").join(path);
    fs::create_dir_all(dst.parent().unwrap()).unwrap();
    fs::copy(src, dst).unwrap();
}

#[test]
fn interrupted_self_retirement_reaches_third_profile_after_first_owner_goes_offline() {
    let store = crate::FakeSecureStore::new();
    let a = Peer::new("retire-a", LOCAL, &store);
    let b = Peer::new("retire-b", REMOTE, &store);
    let c = Peer::new("retire-c", &["CCCCCCC"; 8].join("-"), &store);
    pair(&a, &b, 1, &store);
    pair(&a, &c, 2, &store);
    pair(&b, &c, 3, &store);
    let mut owner_a = a.reopen(&store);
    let mut owner_b = b.reopen(&store);
    let mut owner_c = c.reopen(&store);
    a.tick(&owner_a, &store).unwrap();
    let prefix = PathBuf::from(a.device.as_filename_stem()).join(format!("{:020}.seg", 0));
    for (peer, owner) in [(&b, &mut owner_b), (&c, &mut owner_c)] {
        deliver_exchange(&a, peer, &prefix);
        publish_incoming_with_instance(
            &peer.root,
            std::slice::from_ref(&prefix),
            owner,
            &store,
            ENV,
        )
        .unwrap();
    }
    drop(owner_b);
    drop(owner_c);
    let transaction = "af".repeat(32);
    let snapshot = km::build_owner_security_operation_snapshot(
        &a.root,
        BRAIN,
        &store,
        ENV,
        km::OwnerSecuritySnapshotRequest {
            transaction_id: transaction.clone(),
            operation: BootstrapOperation::SelfRetirement,
            target_device_id: None,
            reason: None,
            cutoff_seq: None,
        },
    )
    .unwrap();
    let cutoff: u64 = snapshot
        .last_authorized_seq
        .as_ref()
        .unwrap()
        .parse()
        .unwrap();
    let journal = BootstrapJournalStore::new(&a.root).unwrap();
    journal
        .begin_owner_operation(snapshot.clone(), "44444444-4444-4444-8444-444444444444")
        .unwrap();
    journal
        .confirm_owner_operation(&transaction, &snapshot.digest().unwrap())
        .unwrap();
    crate::canonical_writer::fail_after_append_before_tip();
    assert!(
        km::append_confirmed_owner_security_operation(&a.root, BRAIN, &store, ENV, &transaction)
            .is_err()
    );
    drop(owner_a);
    km::reconcile_peer_approval_anchors_before_startup(&a.root, &store, ENV).unwrap();
    owner_a = a.reopen(&store);
    let portable = handoff::exchange(None, |request| {
        handoff::reply(request, &a.root, &mut owner_a, &store, ENV).map(|(bytes, _)| bytes)
    })
    .unwrap()
    .unwrap();
    let bytes = serde_json::to_vec(&portable).unwrap();
    assert_eq!(
        bytes,
        serde_json::to_vec(&handoff::export(&a.root, &owner_a.admission(), &store, ENV).unwrap())
            .unwrap()
    );
    let (_, index, original) = portable.frame().unwrap();
    let path = PathBuf::from(a.device.as_filename_stem()).join(format!("{index:020}.seg"));
    assert_eq!(
        fs::read(a.root.join("canonical/events").join(&path)).unwrap(),
        original
    );
    assert!(a.tick(&owner_a, &store).is_err());
    drop(owner_a);
    let mut owner_b = b.reopen(&store);
    crate::incoming_publication::STOP_AFTER_PREPARE.with(|f| f.set(true));
    let import = |owner: &mut InstanceLock| {
        handoff::exchange(Some(handoff::Notice::decode(&bytes).unwrap()), |request| {
            handoff::reply(request, &b.root, owner, &store, ENV).map(|(bytes, _)| bytes)
        })
    };
    assert!(import(&mut owner_b).is_err());
    drop(owner_b);
    let mut owner_b = b.reopen(&store);
    assert!(import(&mut owner_b).unwrap().is_none());
    assert!(import(&mut owner_b).unwrap().is_none());
    // B exports from verified canonical state; owner import never seeds the exchange tree.
    assert!(!b.root.join("transport/incoming").join(&path).exists());
    b.tick(&owner_b, &store).unwrap();
    assert_eq!(
        fs::read(b.root.join("transport/incoming").join(&path)).unwrap(),
        original
    );
    let mut owner_c = c.reopen(&store);
    deliver_exchange(&b, &c, &path);
    let report = publish_incoming_with_instance(
        &c.root,
        std::slice::from_ref(&path),
        &mut owner_c,
        &store,
        ENV,
    )
    .unwrap();
    assert_eq!(report.published_events, 1);
    drop(owner_c);
    drop(owner_b);
    for peer in [&b, &c] {
        let _offline = peer.reopen(&store);
        assert_eq!(peer.cutoff(&a.device, &store), cutoff);
        assert_eq!(
            fs::read(peer.root.join("canonical/events").join(&path)).unwrap(),
            original
        );
    }
}
