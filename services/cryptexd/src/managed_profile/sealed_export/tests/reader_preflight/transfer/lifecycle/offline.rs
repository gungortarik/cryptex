use super::super::automatic::wait_for;
use super::*;
#[cfg(target_os = "macos")]
use crate::import_staging::macos_inbox::Inbox;
#[cfg(windows)]
use crate::import_staging::windows_inbox::Inbox;
use crate::incoming_publication::{rebuild_imported_state, recover_live_with_instance};

pub(super) fn exercise(
    wire: &Path,
    sender: &mut Side,
    receiver: &mut Side,
    store: &dyn crate::secure_store::SecureStore,
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
    // The exclusive import closes the previous transport admission epoch.
    sender.guard = Side::transport_guard(&sender.instance, &sender.root);
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
    let mut inbox = Inbox::new(&receiver.root).unwrap();
    let forget = &batch[0];
    let restore = &batch[1];
    wait_for(
        || remote_inbox.join(restore).is_file(),
        "restore delivered before forget",
    );
    assert!(!remote_inbox.join(forget).try_exists().unwrap());
    let early_bytes = fs::read(remote_inbox.join(restore)).unwrap();
    assert_eq!(
        early_bytes,
        fs::read(sender.root.join("canonical/events").join(restore)).unwrap()
    );
    let existing = inbox.discover().unwrap();
    assert!(!existing.contains(restore));
    if !existing.is_empty() {
        assert_eq!(
            publish(receiver, &existing, store).identical_segments,
            existing.len()
        );
        inbox.consumed().unwrap();
    }
    assert!(inbox.discover().unwrap().is_empty());
    // Even an explicit early hint must fail at the independent canonical gate.
    let early = publish_incoming_with_instance(
        &receiver.root,
        std::slice::from_ref(restore),
        &mut receiver.instance,
        store,
        ENV,
    );
    assert!(
        matches!(early, Err(crate::incoming_publication::IncomingPublicationError::Rejected(
        crate::import_staging::ImportStagingError::Invalid(ref reason))) if reason.contains("is not contiguous")),
        "{early:?}"
    );
    assert!(!remote_canonical.join(restore).try_exists().unwrap());
    assert!(!remote_canonical.join(forget).try_exists().unwrap());
    assert_eq!(search(receiver, store), baseline);
    report_pending(&baseline, &early_bytes);
}

// The parent compares these observations across process exit without providing
// the new process with either the staged segment or the expected note identities.
fn report_pending(notes: &[cryptex_projection::SearchHit], restore: &[u8]) {
    let rows: Vec<_> = notes
        .iter()
        .map(|row| (&row.event_id, &row.content_hash))
        .collect();
    println!(
        "pending_restore_evidence={:x}:{:x}",
        Sha256::digest(restore),
        Sha256::digest(serde_json::to_vec(&rows).unwrap())
    );
}

pub(in super::super) fn resume(
    wire: &Path,
    sender: &Side,
    receiver: &mut Side,
    store: &dyn crate::secure_store::SecureStore,
) {
    // The receiver transport is still stopped. Only the original on-disk inbox
    // can supply this restore after all instance locks and discovery caches died.
    recover_live_with_instance(&receiver.root, &mut receiver.instance, store, ENV).unwrap();
    let author = resolver(sender, store)
        .local_trust_anchor()
        .unwrap()
        .device_id()
        .clone();
    let batch = [3, 4]
        .map(|index| PathBuf::from(author.as_filename_stem()).join(format!("{index:020}.seg")));
    let [forget, restore] = &batch;
    let remote_inbox = receiver.root.join("transport/incoming");
    let remote_canonical = receiver.root.join("canonical/events");
    let early_bytes = fs::read(remote_inbox.join(restore)).unwrap();
    assert_eq!(
        early_bytes,
        fs::read(sender.root.join("canonical/events").join(restore)).unwrap()
    );
    assert!(!remote_inbox.join(forget).try_exists().unwrap());
    for path in &batch {
        assert!(!remote_canonical.join(path).try_exists().unwrap());
    }
    let baseline = search(sender, store);
    assert_eq!(baseline.len(), 2);
    assert_eq!(search(receiver, store), baseline);
    let mut inbox = Inbox::new(&receiver.root).unwrap();
    let existing = inbox.discover().unwrap();
    assert!(!existing.contains(restore));
    assert!(!existing.is_empty());
    assert_eq!(
        publish(receiver, &existing, store).identical_segments,
        existing.len()
    );
    inbox.consumed().unwrap();
    assert!(inbox.discover().unwrap().is_empty());
    report_pending(&baseline, &early_bytes);
    let source_forget = sender.root.join("canonical/events").join(forget);
    let valid = fs::read(&source_forget).unwrap();
    let mut damaged = valid.clone();
    let signature = b"\"signature\":\"";
    let digit = damaged
        .windows(signature.len())
        .position(|s| s == signature)
        .unwrap()
        + signature.len();
    damaged[digit] = if damaged[digit] == b'0' { b'1' } else { b'0' };
    let outgoing = sender.root.join("transport/incoming").join(forget);
    let replace_transport_copy = |bytes: &[u8]| {
        let temp = wire.join("replacement-forget.tmp");
        fs::write(&temp, bytes).unwrap();
        fs::rename(temp, &outgoing).unwrap();
    };
    assert_eq!(fs::read(&outgoing).unwrap(), valid);
    replace_transport_copy(&damaged);
    fs::write(wire.join("release-forget-segment"), []).unwrap();
    wait_for(
        || fs::read(remote_inbox.join(forget)).is_ok_and(|bytes| bytes == damaged),
        "damaged forget delivery",
    );
    let rejected = inbox.discover().unwrap();
    assert_eq!(rejected, batch);
    let result = publish_incoming_with_instance(
        &receiver.root,
        &rejected,
        &mut receiver.instance,
        store,
        ENV,
    );
    assert!(
        matches!(&result, Err(crate::incoming_publication::IncomingPublicationError::Rejected(
        crate::import_staging::ImportStagingError::Invalid(reason))) if reason.contains("v2 signature verification failed")),
        "{result:?}"
    );
    inbox.rejected().unwrap();
    assert!(inbox.discover().unwrap().is_empty());
    for path in &batch {
        assert!(!remote_canonical.join(path).try_exists().unwrap());
    }
    assert_eq!(fs::read(remote_inbox.join(restore)).unwrap(), early_bytes);
    assert_eq!(search(receiver, store), baseline);
    assert_eq!(fs::read(source_forget).unwrap(), valid);

    replace_transport_copy(&valid);
    wait_for(
        || fs::read(remote_inbox.join(forget)).is_ok_and(|bytes| bytes == valid),
        "corrected forget delivery",
    );
    let discovered = inbox.discover().unwrap();
    assert_eq!(discovered, batch);
    assert_eq!(fs::read(remote_inbox.join(restore)).unwrap(), early_bytes);
    assert_eq!(publish(receiver, &discovered, store).published_events, 2);
    inbox.consumed().unwrap();
    assert!(inbox.discover().unwrap().is_empty());
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
    println!("reordered_note_lifecycle=verified first=restore deferred=1 converged=true");
    println!(
        "corrupt_forget_recovery=verified signature=rejected published_on_rejection=0 corrected_imports=2 replay_added=0"
    );
}
