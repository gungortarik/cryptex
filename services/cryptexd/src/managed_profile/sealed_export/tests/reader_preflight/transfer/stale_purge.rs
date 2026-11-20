//! A stale, genuinely signed pack crosses the wire back to its author after purge.
use super::automatic::wait_for;
use super::*;
use crate::incoming_publication::{publish_incoming_with_instance, recover_live_with_instance};
use crate::ipc::purge_review::{physical, receipt};
use crate::secure_store::{FileBackedSecureStore, SecureStore};
use std::sync::Arc;

fn relative(device: &DeviceId) -> PathBuf {
    PathBuf::from(device.as_filename_stem()).join("00000000000000000000.seg")
}

fn publish(
    side: &mut Side,
    path: &Path,
    bytes: &[u8],
    store: &dyn SecureStore,
) -> Result<
    crate::incoming_publication::IncomingPublicationReport,
    crate::incoming_publication::IncomingPublicationError,
> {
    let incoming = side.root.join("transport/incoming").join(path);
    fs::create_dir_all(incoming.parent().unwrap()).unwrap();
    fs::write(&incoming, bytes).unwrap();
    publish_incoming_with_instance(
        &side.root,
        &[path.to_owned()],
        &mut side.instance,
        store,
        ENV,
    )
}

pub(super) fn exercise(wire: &Path, owner_transport: &str, peer_transport: &str) {
    let left = wire.join("A/purge-profile");
    let right = wire.join("B/purge-profile");
    for root in [&left, &right] {
        fs::create_dir(root).unwrap();
    }
    let store = Arc::new(FileBackedSecureStore::open(wire.join("purge-credentials")).unwrap());
    let keys = store.as_ref();
    crate::trusted_import_probe::tests::enroll_transport_pair(
        &left,
        &right,
        owner_transport,
        peer_transport,
        keys,
    );
    let mut owner = Side::new(left.clone(), owner_transport);
    let mut peer = Side::new(right.clone(), peer_transport);
    let author = resolver(&owner, keys).local_trust_anchor().unwrap();
    let remote = resolver(&peer, keys).local_trust_anchor().unwrap();
    let note = append_memory_note(
        keys,
        ENV,
        AppendMemoryNoteArgs {
            runtime_root: &left,
            brain_id: BRAIN,
            device_id: author.device_id(),
            classification: Classification::Public,
            text: "isolatedpurgeoffline",
            event_id: None,
            config: &CanonicalWriterConfig::default(),
        },
    )
    .unwrap()
    .accepted;
    for (side, id) in [
        (&mut owner, author.device_id()),
        (&mut peer, remote.device_id()),
    ] {
        cryptex_storage::seal_active_segment(&side.root, id, 0).unwrap();
        recover_live_with_instance(&side.root, &mut side.instance, keys, ENV).unwrap();
    }
    let a_path = relative(author.device_id());
    let b_path = relative(remote.device_id());
    let ab = Wire::new(wire, "A", "B");
    let ba = Wire::new(wire, "B", "A");
    let owner_pack = left.join("canonical/events").join(&a_path);
    let peer_copy = right.join("canonical/events").join(&a_path);
    let baseline = fs::read(&owner_pack).unwrap();
    let other = fs::read(right.join("canonical/events").join(&b_path)).unwrap();
    assert_eq!(
        publish(
            &mut peer,
            &a_path,
            &ab.deliver("purge-initial-owner", &baseline),
            keys
        )
        .unwrap()
        .published_events,
        3
    );
    assert_eq!(
        publish(
            &mut owner,
            &b_path,
            &ba.deliver("purge-initial-peer", &other),
            keys
        )
        .unwrap()
        .published_events,
        2
    );
    let visible = lifecycle::search(&peer, keys);
    assert_eq!(visible.len(), 3);
    assert_eq!(lifecycle::search(&owner, keys), visible);
    let remaining: Vec<_> = visible
        .iter()
        .filter(|hit| hit.event_id != note.event_id)
        .cloned()
        .collect();
    assert_eq!(remaining.len(), 2);
    // Import takes an exclusive admission epoch. Restart each transport session
    // with its current guard before exercising the normal export tick.
    owner.guard = Side::transport_guard(&owner.instance, &owner.root);
    peer.guard = Side::transport_guard(&peer.instance, &peer.root);
    physical::register_pair(wire);
    fs::write(wire.join("partition-purge"), []).unwrap();
    wait_for(
        || wire.join("purge-offline").exists(),
        "both native workers stopped",
    );
    physical::confirm_fixture(
        &left,
        &owner.instance,
        store.clone(),
        &note.event_id,
        note.content_hash.as_str(),
    );
    owner.tick(keys);
    let control = format!("{}.purge", note.event_id);
    assert!(ab.sender.join(&control).is_file());
    assert!(!ab.receiver.join(&control).exists());
    assert_eq!(lifecycle::search(&peer, keys), visible);
    assert_eq!(lifecycle::search(&owner, keys), remaining);
    let compacted = fs::read(&owner_pack).unwrap();
    assert_ne!(compacted, baseline);
    let stale = fs::read(&peer_copy).unwrap();
    assert_eq!(stale, baseline);
    fs::write(ba.sender.join("purge-stale-replay"), stale).unwrap();
    fs::write(wire.join("reconnect-purge"), []).unwrap();
    wait_for(
        || ba.receiver.join("purge-stale-replay").is_file() && ab.receiver.join(&control).is_file(),
        "reconnected peer deliveries",
    );
    let replay = fs::read(ba.receiver.join("purge-stale-replay")).unwrap();
    assert_eq!(replay, baseline);
    // Exercise the normal publication entry point before peer notice consumption.
    let rejected = publish(&mut owner, &a_path, &replay, keys).unwrap_err();
    assert!(
        matches!(
            rejected,
            crate::incoming_publication::IncomingPublicationError::Rejected(_)
        ),
        "{rejected:?}"
    );
    assert_eq!(fs::read(&owner_pack).unwrap(), compacted);
    assert_eq!(lifecycle::search(&owner, keys), remaining);
    let notice: serde_json::Value =
        serde_json::from_slice(&fs::read(ab.receiver.join(&control)).unwrap()).unwrap();
    let approval = notice["approval"].as_str().unwrap().as_bytes();
    let mut forged: serde_json::Value = serde_json::from_slice(approval).unwrap();
    forged["signature"] = serde_json::json!("00".repeat(64));
    let forged = serde_json_canonicalizer::to_vec(&forged).unwrap();
    assert!(matches!(
        receipt::receive(
            &right,
            BRAIN,
            &remote,
            notice["bridge"].as_str().unwrap().as_bytes(),
            &forged
        ),
        Err(crate::ipc::purge_review::ReviewError::ReceiptIntegrity)
    ));
    assert!(receipt::receive(&right, BRAIN, &remote, b"{}", approval).is_err());
    assert!(
        receipt::receive(
            &right,
            BRAIN,
            &remote,
            notice["bridge"].as_str().unwrap().as_bytes(),
            b"{}"
        )
        .is_err()
    );
    assert_eq!(lifecycle::search(&peer, keys), visible);
    peer.tick(keys);
    assert_eq!(lifecycle::search(&peer, keys), remaining);
    let ledger = right
        .join("canonical")
        .join(receipt::FOLDER)
        .join(format!("{}.json", note.event_id));
    let durable = fs::read(&ledger).unwrap();
    peer.tick(keys);
    assert_eq!(fs::read(&ledger).unwrap(), durable);
    assert_eq!(
        publish(&mut peer, &a_path, &replay, keys)
            .unwrap()
            .published_events,
        0
    );
    drop((owner, peer));
    for (root, transport) in [(left, owner_transport), (right, peer_transport)] {
        let side = Side::new(root, transport);
        crate::incoming_publication::rebuild_imported_state(&side.root, BRAIN, keys, ENV).unwrap();
        assert_eq!(lifecycle::search(&side, keys), remaining);
        let db = crate::projection::open_projection_db(&side.root).unwrap().0;
        let mut key_resolver = resolver(&side, keys);
        let detail = |key_resolver: &mut LocalDeviceKeyResolver, id: &str| {
            crate::knowledge_materialization::event_detail_with_attachments(
                &side.root,
                BRAIN,
                &[],
                key_resolver,
                &db,
                id,
                None,
            )
            .unwrap()
        };
        assert!(detail(&mut key_resolver, &note.event_id).is_none());
        for keep in &remaining {
            assert_eq!(
                detail(&mut key_resolver, &keep.event_id)
                    .unwrap()
                    .0
                    .content_hash,
                keep.content_hash
            );
        }
        assert_eq!(
            fs::read(side.root.join("canonical/events").join(&b_path)).unwrap(),
            other
        );
    }
    println!("stale_purge_reconnect=verified stale_replay=refused survivors=2 reopened=2");
}
