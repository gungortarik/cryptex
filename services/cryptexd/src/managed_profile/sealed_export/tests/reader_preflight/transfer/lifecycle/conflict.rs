//! One note, two offline issuers. Use the peer's own key for the inadmissible
//! restore and stage it only in transport, never in either canonical history.
use super::super::automatic::wait_for;
use super::*;
use crate::incoming_publication::{
    IncomingPublicationError, rebuild_imported_state, recover_live_with_instance,
};
use crate::secure_store::SecureStore;
use cryptex_events::security_history::NoteLifecycleRead;

fn segment(device: &DeviceId, index: u64) -> PathBuf {
    PathBuf::from(device.as_filename_stem()).join(format!("{index:020}.seg"))
}

fn queue_author_record(side: &Side, author: &DeviceId, index: u64, raw: &[u8]) -> PathBuf {
    cryptex_storage::append_frame(&side.root, author, index, raw).unwrap();
    cryptex_storage::seal_active_segment(&side.root, author, index).unwrap();
    let path = segment(author, index);
    let outgoing = side.root.join("transport/incoming").join(&path);
    let temp = side.root.join("queued-segment.tmp");
    fs::copy(side.root.join("canonical/events").join(&path), &temp).unwrap();
    assert!(!outgoing.exists());
    fs::rename(temp, outgoing).unwrap();
    path
}

// Try both signatures against the peer's *same* hidden-note snapshot and the
// same prior hash. This isolates the author rule from the later stale-parent
// check on the receiving author. Neither candidate is written here.
fn check_restore_author(
    peer: &Side,
    store: &dyn SecureStore,
    target: &AcceptedEvent,
    owner_restore: &[u8],
    peer_restore: &[u8],
) {
    let snapshot = snapshot(peer, store);
    let (anchor, _) = snapshot.verified_trust_context().unwrap();
    for (candidate, authorized) in [(owner_restore, true), (peer_restore, false)] {
        let mut records = snapshot.trial_security_frames();
        records.push(candidate);
        let (history, reduction, _) = crate::trust_reducer::verify_anchored_history_for_reads(
            &records,
            BRAIN,
            anchor.local_device.clone(),
            anchor.local_signing_key_id.clone(),
            anchor.local_public_key,
            NoteLifecycleRead::Apply,
            || false,
        )
        .unwrap();
        let crate::trust_reducer::TrustReduction::Ready(trust) = reduction else {
            panic!("paired authority held")
        };
        let cutoffs = trust
            .effective_cutoffs
            .iter()
            .map(|(id, cutoff)| (id.as_str().to_owned(), *cutoff))
            .collect();
        let result = history.forgotten_note_ids(&cutoffs, || false);
        if authorized {
            assert!(!result.unwrap().contains(&target.event_id));
        } else {
            assert!(matches!(result, Err(cryptex_events::security_history::CancellableSecurityHistoryError::InvalidRecord(_))));
        }
    }
}

pub(in super::super) fn exercise(
    wire: &Path,
    owner: &mut Side,
    peer: &mut Side,
    store: &dyn SecureStore,
) {
    recover_live_with_instance(&owner.root, &mut owner.instance, store, ENV).unwrap();
    let owner_id = resolver(owner, store)
        .local_trust_anchor()
        .unwrap()
        .device_id()
        .clone();
    let peer_id = resolver(peer, store)
        .local_trust_anchor()
        .unwrap()
        .device_id()
        .clone();
    assert_ne!(owner_id, peer_id);
    let snapshot_before = snapshot(owner, store);
    let chain = snapshot_before.reader_device_chain(owner_id.as_str());
    let note = chain
        .iter()
        .find_map(|r| match r {
            AuthenticatedSecurityRecord::V1(note) => Some(note.clone()),
            _ => None,
        })
        .unwrap();
    let prior = chain.last().unwrap().content_hash().clone();
    drop(snapshot_before);
    let visible = search(owner, store);
    let hidden: Vec<_> = visible
        .iter()
        .filter(|n| n.event_id != note.event_id)
        .cloned()
        .collect();
    assert_eq!(visible.len(), hidden.len() + 1);
    assert!(!hidden.is_empty());
    let (raw, shared_forget) = signed_control(
        &note,
        store,
        owner,
        "55555555-5555-4555-8555-555555555555",
        "memory.forget",
        &prior,
    );
    let shared = queue_author_record(owner, &owner_id, 5, &raw);
    let peer_inbox = peer.root.join("transport/incoming");
    wait_for(
        || peer_inbox.join(&shared).is_file(),
        "shared forget delivered",
    );
    assert_eq!(publish(peer, &[shared], store).published_events, 1);
    rebuild_imported_state(&owner.root, BRAIN, store, ENV).unwrap();
    assert_eq!(search(peer, store), hidden);
    assert_eq!(search(owner, store), hidden);

    fs::write(wire.join("partition-note-conflict"), []).unwrap();
    wait_for(
        || wire.join("note-conflict-offline").is_file(),
        "both conflict workers stopped",
    );
    let (restore, hash) = signed_control(
        &note,
        store,
        owner,
        "55555555-5555-4555-8555-555555555556",
        "memory.restore",
        &shared_forget,
    );
    let (competing, _) = signed_control(
        &note,
        store,
        peer,
        "66666666-6666-4666-8666-666666666661",
        "memory.restore",
        &shared_forget,
    );
    check_restore_author(peer, store, &note, &restore, &competing);
    let first = queue_author_record(owner, &owner_id, 6, &restore);
    rebuild_imported_state(&owner.root, BRAIN, store, ENV).unwrap();
    assert_eq!(search(owner, store), visible);
    assert_eq!(search(peer, store), hidden);
    let (forget, _) = signed_control(
        &note,
        store,
        owner,
        "55555555-5555-4555-8555-555555555557",
        "memory.forget",
        &hash,
    );
    let second = queue_author_record(owner, &owner_id, 7, &forget);
    rebuild_imported_state(&owner.root, BRAIN, store, ENV).unwrap();
    assert_eq!(search(owner, store), hidden);
    let valid = [first, second];
    let rejected = segment(&peer_id, 1);
    let owner_inbox = owner.root.join("transport/incoming");
    let mut frame = cryptex_events::encode_frame_header(competing.len().try_into().unwrap())
        .unwrap()
        .to_vec();
    frame.extend_from_slice(&competing);
    assert!(!peer_inbox.join(&rejected).exists());
    fs::write(peer_inbox.join(&rejected), &frame).unwrap();
    assert!(!owner_inbox.join(&rejected).exists());
    assert!(valid.iter().all(|p| !peer_inbox.join(p).exists()));
    fs::write(wire.join("reconnect-note-conflict"), []).unwrap();
    wait_for(
        || {
            fs::read(owner_inbox.join(&rejected)).is_ok_and(|b| b == frame)
                && valid.iter().all(|p| peer_inbox.join(p).is_file())
        },
        "opposing offline operations delivered",
    );
    let denied = publish_incoming_with_instance(
        &owner.root,
        std::slice::from_ref(&rejected),
        &mut owner.instance,
        store,
        ENV,
    );
    assert!(
        matches!(&denied, Err(IncomingPublicationError::Rejected(crate::import_staging::ImportStagingError::Invalid(reason))) if reason == "invalid signed note transition"),
        "{denied:?}"
    );
    for side in [&*peer, &*owner] {
        assert!(!side.root.join("canonical/events").join(&rejected).exists());
    }
    assert_eq!(publish(peer, &valid, store).published_events, 2);
    let replay = publish(peer, &valid, store);
    assert_eq!(replay.identical_segments, 2);
    assert_eq!(replay.published_events, 0);
    assert_eq!(search(owner, store), hidden);
    assert_eq!(search(peer, store), hidden);
    let a = snapshot(owner, store);
    let b = snapshot(peer, store);
    for id in [&owner_id, &peer_id] {
        assert_eq!(
            a.reader_device_chain(id.as_str()),
            b.reader_device_chain(id.as_str())
        );
    }
    println!(
        "offline_note_conflict=verified peer_restore=rejected author_forget=applied histories=equal target=hidden replay_added=0"
    );
}
