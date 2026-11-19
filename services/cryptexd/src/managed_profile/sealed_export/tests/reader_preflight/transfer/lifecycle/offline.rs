use super::super::automatic::wait_for;
use super::*;
use crate::incoming_publication::{rebuild_imported_state, recover_live_with_instance};

pub(super) fn exercise(
    wire: &Path,
    sender: &mut Side,
    receiver: &mut Side,
    store: &crate::FakeSecureStore,
    target: &AcceptedEvent,
    mut prior: ContentHashHex,
) {
    // The preceding one-way trial imported A's history on B. Import B's
    // original pack on A so both guarded searches start from the same notes.
    let peer = resolver(receiver, store)
        .local_trust_anchor()
        .unwrap()
        .device_id()
        .clone();
    let peer_pack = PathBuf::from(peer.as_filename_stem()).join("00000000000000000000.seg");
    wait_for(
        || {
            sender
                .root
                .join("transport/incoming")
                .join(&peer_pack)
                .is_file()
        },
        "initial peer pack",
    );
    recover_live_with_instance(&sender.root, &mut sender.instance, store, ENV).unwrap();
    publish(sender, &[peer_pack], store);
    let baseline = search(receiver, store);
    assert_eq!(search(sender, store), baseline);
    let mut hidden = baseline.clone();
    hidden.retain(|note| note.event_id != target.event_id);
    assert_eq!(hidden.len() + 1, baseline.len());

    fs::write(wire.join("stop-lifecycle-receiver"), []).unwrap();
    wait_for(
        || wire.join("lifecycle-receiver-offline").is_file(),
        "receiver process exit",
    );
    let author = DeviceId::new(target.device_id.clone()).unwrap();
    let source_inbox = sender.root.join("transport/incoming");
    let remote_inbox = receiver.root.join("transport/incoming");
    let remote_canonical = receiver.root.join("canonical/events");
    let mut batch = Vec::new();
    for (index, kind, id, expected) in [
        (
            3,
            "memory.forget",
            "55555555-5555-4555-8555-555555555553",
            &hidden,
        ),
        (
            4,
            "memory.restore",
            "55555555-5555-4555-8555-555555555554",
            &baseline,
        ),
    ] {
        let (raw, hash) = signed_control(target, store, sender, id, kind, &prior);
        prior = hash;
        cryptex_storage::append_frame(&sender.root, &author, index, &raw).unwrap();
        let relative = PathBuf::from(author.as_filename_stem()).join(format!("{index:020}.seg"));
        wait_for(
            || {
                sender.tick(store);
                source_inbox.join(&relative).is_file()
            },
            "sealed record queued offline",
        );
        rebuild_imported_state(&sender.root, BRAIN, store, ENV).unwrap();
        assert_eq!(&search(sender, store), expected);
        assert_eq!(search(receiver, store), baseline);
        batch.push(relative);
        for path in &batch {
            assert!(!remote_inbox.join(path).try_exists().unwrap());
            assert!(!remote_canonical.join(path).try_exists().unwrap());
        }
    }

    fs::write(wire.join("resume-lifecycle-receiver"), []).unwrap();
    wait_for(
        || batch.iter().all(|path| remote_inbox.join(path).is_file()),
        "queued lifecycle delivery",
    );
    assert_eq!(publish(receiver, &batch, store).published_events, 2);
    for path in &batch {
        let signed = fs::read(sender.root.join("canonical/events").join(path)).unwrap();
        assert_eq!(fs::read(remote_canonical.join(path)).unwrap(), signed);
    }
    let replay = publish(receiver, &batch, store);
    assert_eq!(replay.identical_segments, 2);
    assert_eq!(replay.published_events, 0);
    assert_eq!(search(receiver, store), baseline);
    assert_eq!(search(sender, store), search(receiver, store));
    println!("offline_note_lifecycle=verified queued=2 imported=2 identical_replays=2");
}
