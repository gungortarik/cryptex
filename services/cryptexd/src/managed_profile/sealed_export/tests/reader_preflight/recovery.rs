use super::*;
use std::ffi::{OsStr, OsString};

/// Deliver only the sender's own slot, as if the control channel were online.
fn deliver(sender: &Side, receiver: &Side, slot: &OsStr) {
    fs::copy(sender.controls().join(slot), receiver.controls().join(slot)).unwrap();
}

fn first_challenge(side: &mut Side, store: &crate::FakeSecureStore) -> OsString {
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
        assert_eq!(
            side.exporter.last_reader_status,
            Some(reader::Status::Waiting)
        );
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

// Only used on a stopped synthetic profile. Copy regular files exclusively
// into an empty protected destination, including stale control and receipt files.
fn copy_entry(source: &Path, dest: &Path) {
    let metadata = fs::symlink_metadata(source).unwrap();
    if metadata.is_dir() {
        fs::create_dir(dest).unwrap();
        for entry in fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            copy_entry(&entry.path(), &dest.join(entry.file_name()));
        }
    } else {
        assert!(metadata.is_file());
        let mut output = File::create_new(dest).unwrap();
        std::io::copy(&mut File::open(source).unwrap(), &mut output).unwrap();
        fs::set_permissions(dest, metadata.permissions()).unwrap();
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
    drop(a);

    let restored = crate::instance::create_m9_profile_root(&scratch, "restored").unwrap();
    for entry in fs::read_dir(&live).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name() != "daemon.lock" {
            copy_entry(&entry.path(), &restored.join(entry.file_name()));
        }
    }
    let mut a = Side::new(live.clone(), LOCAL);
    let device =
        DeviceId::new(load_local_device_identity(&peer, BRAIN).unwrap().device_id).unwrap();
    let path = PathBuf::from(device.as_filename_stem()).join(format!("{:020}.seg", 0));
    let expected = fs::read(peer.join("canonical/events").join(&path)).unwrap();
    import_peer_pack(&mut a, &b, &path, &store);
    drop(a);
    let live_pack = live.join("canonical/events").join(&path);
    let restored_pack = restored.join("canonical/events").join(&path);
    assert_eq!(fs::read(&live_pack).unwrap(), expected);
    assert!(!restored_pack.exists());

    // The previous run's signed exchange survived the backup. Verify it
    // cannot satisfy a newly opened reader, even with the same secure key.
    let mut a = Side::new(restored.clone(), LOCAL);
    assert_eq!(fs::read(a.controls().join(&from_b)).unwrap(), stale_reply);
    a.tick(&store);
    assert_eq!(a.exporter.last_reader_status, Some(reader::Status::Waiting));
    import_peer_pack(&mut a, &b, &path, &store);
    assert_eq!(fs::read(&restored_pack).unwrap(), expected);
    drop(a);
    let mut a = Side::new(restored, LOCAL);
    a.tick(&store);
    assert_eq!(a.exporter.last_reader_status, Some(reader::Status::Waiting));
    b.exporter.end_reader_session();
    b.tick(&store);
    assert_eq!(b.exporter.last_reader_status, Some(reader::Status::Waiting));
    exchange(&mut a, &mut b, &from_a, &from_b, &store);
    assert_ne!(fs::read(a.controls().join(&from_b)).unwrap(), stale_reply);
    assert_eq!(fs::read(restored_pack).unwrap(), expected);
    assert_eq!(fs::read(live_pack).unwrap(), expected);
    drop((a, b));
    for root in [live, peer, scratch] {
        fs::remove_dir_all(root).unwrap();
    }
}
