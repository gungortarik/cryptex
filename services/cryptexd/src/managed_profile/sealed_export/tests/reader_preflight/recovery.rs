use super::*;
use std::ffi::{OsStr, OsString};

/// Deliver only the sender's own slot, as if the control channel were online.
fn deliver(sender: &Side, receiver: &Side, slot: &OsStr) {
    fs::copy(sender.controls().join(slot), receiver.controls().join(slot)).unwrap();
}

pub(super) fn first_challenge(side: &mut Side, store: &crate::FakeSecureStore) -> OsString {
    side.tick(store);
    assert_eq!(
        side.exporter.last_reader_status,
        Some(reader::Status::Waiting)
    );
    let slots: Vec<_> = fs::read_dir(side.controls())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(slots.len(), 1);
    slots.into_iter().next().unwrap()
}

fn exchange(
    a: &mut Side,
    b: &mut Side,
    from_a: &OsStr,
    from_b: &OsStr,
    store: &crate::FakeSecureStore,
) -> Report {
    deliver(a, b, from_a);
    b.tick(store);
    deliver(b, a, from_b);
    let report = a.tick(store);
    deliver(a, b, from_a);
    b.tick(store);
    for side in [a, b] {
        assert!(
            matches!(&side.exporter.last_reader_status, Some(reader::Status::Verified(peers)) if peers.len() == 1)
        );
    }
    report
}

#[test]
fn offline_evidence_survives_reopen_and_separate_sessions_only_for_the_same_roster() {
    use super::super::retirement_relay::{Peer, pair};
    use reader::Status;
    const THIRD: &str = "CCCCCCC-CCCCCCC-CCCCCCC-CCCCCCC-CCCCCCC-CCCCCCC-CCCCCCC-CCCCCCC";
    let store = crate::FakeSecureStore::new();
    let hub = Peer::new("retained-reader", LOCAL, &store);
    let first = Peer::new("retained-peer", REMOTE, &store);
    let third = Peer::new("new-peer", THIRD, &store);
    pair(&hub, &first, 1, &store);
    let mut a = Side::new(hub.root.clone(), LOCAL);
    let mut b = Side::new(first.root.clone(), REMOTE);
    let from_a = first_challenge(&mut a, &store);
    let from_b = first_challenge(&mut b, &store);
    exchange(&mut a, &mut b, &from_a, &from_b, &store);
    fs::remove_file(a.controls().join(&from_b)).unwrap();
    drop(a);

    let mut a = Side::new(hub.root.clone(), LOCAL);
    let local_device = DeviceId::new(
        load_local_device_identity(&a.root, BRAIN)
            .unwrap()
            .device_id,
    )
    .unwrap();
    append_memory_note(
        &store,
        ENV,
        AppendMemoryNoteArgs {
            runtime_root: &a.root,
            brain_id: BRAIN,
            device_id: &local_device,
            classification: Classification::Public,
            text: "ordinary offline note",
            event_id: None,
            config: &CanonicalWriterConfig::default(),
        },
    )
    .unwrap();
    a.tick(&store);
    assert!(
        matches!(&a.exporter.last_reader_status, Some(Status::PreviouslyVerified(peers)) if peers.len() == 1)
    );
    let account = from_b.to_str().unwrap();
    let service = "Cryptex-Dev-M9ReaderObservations";
    let original = store.get(service, account).unwrap();
    let mut corrupt: serde_json::Value = serde_json::from_slice(&original).unwrap();
    corrupt["signature"] = serde_json::Value::from("00".repeat(64));
    store
        .put(service, account, &serde_json::to_vec(&corrupt).unwrap())
        .unwrap();
    a.tick(&store);
    assert_eq!(
        a.exporter.last_reader_status,
        Some(Status::Unavailable(reader::Error::Message))
    );
    store.put(service, account, &original).unwrap();
    drop(a);

    pair(&hub, &third, 2, &store);
    let mut a = Side::new(hub.root.clone(), LOCAL);
    let mut c = Side::new(third.root.clone(), THIRD);
    a.tick(&store);
    let to_c = fs::read_dir(a.controls())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .find(|name| *name != from_a)
        .unwrap();
    let from_c = first_challenge(&mut c, &store);
    deliver(&a, &c, &to_c);
    c.tick(&store);
    deliver(&c, &a, &from_c);
    a.tick(&store);
    // The new peer answered. The old proof predates its admission.
    assert_eq!(a.exporter.last_reader_status, Some(Status::Waiting));

    fs::remove_file(a.controls().join(&from_c)).unwrap();
    a.exporter.end_reader_session();
    a.tick(&store);
    deliver(&a, &b, &from_a);
    b.tick(&store);
    deliver(&b, &a, &from_b);
    a.tick(&store);
    // Each eligible peer answered under the new roster, in different sessions.
    assert!(
        matches!(&a.exporter.last_reader_status, Some(Status::PreviouslyVerified(peers)) if peers.len() == 2)
    );
}

#[test]
fn offline_round_expires_and_both_readers_reverify_after_reconnect() {
    let (left, right, scratch, store) =
        crate::trusted_import_probe::tests::pair_with_transport(LOCAL, REMOTE);
    let mut a = Side::new(left.clone(), LOCAL);
    let mut b = Side::new(right.clone(), REMOTE);
    let pack = seed_link(&left, &store);
    let original = fs::read(&pack).unwrap();
    let from_a = first_challenge(&mut a, &store);
    let from_b = first_challenge(&mut b, &store);
    assert_eq!(
        exchange(&mut a, &mut b, &from_a, &from_b, &store).attachment_transfer_disabled,
        1
    );
    let old_reply = fs::read(a.controls().join(&from_b)).unwrap();

    // No deliveries during this interval. Expire the actual round while the
    // controller is still running and the old valid signature is still on disk.
    for side in [&mut a, &mut b] {
        side.exporter.reader_check.expire_current_round();
        let report = side.tick(&store);
        assert!(matches!(&side.exporter.last_reader_status,
            Some(reader::Status::PreviouslyVerified(peers)) if peers.len() == 1));
        if side.transport == LOCAL {
            assert_eq!(
                (
                    report.attachment_waiting_for_reader,
                    report.sealed,
                    report.copied
                ),
                (1, 0, 0)
            );
        }
    }
    assert_eq!(fs::read(a.controls().join(&from_b)).unwrap(), old_reply);
    let report = exchange(&mut a, &mut b, &from_a, &from_b, &store);
    assert_eq!(
        (
            report.attachment_transfer_disabled,
            report.sealed,
            report.copied
        ),
        (1, 0, 0)
    );
    assert_ne!(fs::read(a.controls().join(&from_b)).unwrap(), old_reply);
    assert_eq!(fs::read(pack).unwrap(), original);
    drop((a, b));
    for root in [left, right, scratch] {
        fs::remove_dir_all(root).unwrap();
    }
}

fn import_peer_pack(receiver: &mut Side, peer: &Side, path: &Path, store: &crate::FakeSecureStore) {
    crate::incoming_publication::recover_live_with_instance(
        &receiver.root,
        &mut receiver.instance,
        store,
        ENV,
    )
    .unwrap();
    let incoming = receiver.root.join("transport/incoming").join(path);
    fs::create_dir_all(incoming.parent().unwrap()).unwrap();
    fs::copy(peer.root.join("transport/incoming").join(path), incoming).unwrap();
    let report = crate::incoming_publication::publish_incoming_with_instance(
        &receiver.root,
        &[path.to_owned()],
        &mut receiver.instance,
        store,
        ENV,
    )
    .unwrap();
    assert_eq!(report.published_events, 2);
}

#[test]
fn restored_profile_reconciles_a_missing_pack_and_requires_a_fresh_reader_reply() {
    let (live, peer, scratch, store) =
        crate::trusted_import_probe::tests::pair_with_transport(LOCAL, REMOTE);
    let mut a = Side::new(live.clone(), LOCAL);
    let mut b = Side::new(peer.clone(), REMOTE);
    let from_a = first_challenge(&mut a, &store);
    let from_b = first_challenge(&mut b, &store);
    exchange(&mut a, &mut b, &from_a, &from_b, &store);
    let stale_reply = fs::read(a.controls().join(&from_b)).unwrap();
    let service = "Cryptex-Dev-M9ReaderObservations";
    let saved = |slot: &OsStr| store.get(service, slot.to_str().unwrap()).unwrap();
    let receipt_name = std::env::var("CRYPTEX_M9_READER_ATTEMPT")
        .ok()
        .map(|attempt| format!("transport/m9-control-staging/{attempt}.receipt"));
    drop(a);

    let restored = crate::test_temp::stage_stopped_m9_profile(&live, &scratch, "restored");
    let mut a = Side::new(live.clone(), LOCAL);
    let device =
        DeviceId::new(load_local_device_identity(&peer, BRAIN).unwrap().device_id).unwrap();
    let path = PathBuf::from(device.as_filename_stem()).join(format!("{:020}.seg", 0));
    let expected = fs::read(peer.join("canonical/events").join(&path)).unwrap();
    import_peer_pack(&mut a, &b, &path, &store);
    drop(a);
    let mut a = Side::new(live.clone(), LOCAL);
    a.tick(&store);
    exchange(&mut a, &mut b, &from_a, &from_b, &store);
    let previous_a = saved(&from_b);
    let previous_b = saved(&from_a);
    drop(a);
    let live_pack = live.join("canonical/events").join(&path);
    let restored_pack = restored.join("canonical/events").join(&path);
    assert_eq!(fs::read(&live_pack).unwrap(), expected);
    assert!(!restored_pack.exists());

    // The previous run's signed exchange survived the backup. Verify it
    // cannot satisfy a newly opened reader, even with the same secure key.
    let mut a = Side::new(restored.clone(), LOCAL);
    if let Some(name) = &receipt_name {
        assert!(restored.join(name).is_file());
    }
    assert_eq!(fs::read(a.controls().join(&from_b)).unwrap(), stale_reply);
    let blocked = a.tick(&store);
    assert_eq!(
        a.exporter.last_reader_status,
        Some(reader::Status::WaitingForHistory)
    );
    assert_eq!((blocked.sealed, blocked.copied), (0, 0));
    assert!(blocked.withheld > 0);
    assert_eq!(saved(&from_b), previous_a);
    if let Some(name) = &receipt_name {
        assert!(!restored.join(name).exists());
    }
    import_peer_pack(&mut a, &b, &path, &store);
    assert_eq!(fs::read(&restored_pack).unwrap(), expected);
    drop(a);
    let mut a = Side::new(restored.clone(), LOCAL);
    a.tick(&store);
    assert_eq!(a.exporter.last_reader_status, Some(reader::Status::Waiting));
    // The surviving peer stays in the same session. Its proof still has
    // to be refreshed when the restored device answers with a new nonce.
    exchange(&mut a, &mut b, &from_a, &from_b, &store);
    assert_ne!(saved(&from_b), previous_a);
    assert_ne!(saved(&from_a), previous_b);
    if let Some(name) = &receipt_name {
        let receipt = |root: &Path| {
            serde_json::from_slice::<serde_json::Value>(&fs::read(root.join(name)).unwrap())
                .unwrap()
        };
        let new = receipt(&restored);
        assert_ne!(new["local_nonce"], receipt(&live)["local_nonce"]);
        assert_eq!(new["local_nonce"], receipt(&peer)["peer_nonce"]);
    }
    assert_ne!(fs::read(a.controls().join(&from_b)).unwrap(), stale_reply);
    assert_eq!(fs::read(restored_pack).unwrap(), expected);
    assert_eq!(fs::read(live_pack).unwrap(), expected);
    drop((a, b));
    for root in [live, peer, scratch] {
        fs::remove_dir_all(root).unwrap();
    }
}
