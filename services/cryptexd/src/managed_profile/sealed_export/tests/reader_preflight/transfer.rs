//! Two enrolled M9 profiles, two real Syncthing workers and the normal sealed importer.
use super::*;
use crate::attachment_blobs::{BlobError, LocalLinkError};
use crate::local_device_identity::LocalDeviceKeyResolver;
use cryptex_events::AuthenticatedSecurityRecord;
use std::thread;

struct Wire {
    sender: PathBuf,
    receiver: PathBuf,
}

impl Wire {
    fn new(base: &Path, sender: &str, receiver: &str) -> Self {
        Self {
            sender: base.join(sender).join("out"),
            receiver: base.join(receiver).join("in"),
        }
    }

    // Only Syncthing can populate the other folder. Unique leaves distinguish
    // successive versions of the same reader slot and retries of a blob.
    fn deliver(&self, name: &str, bytes: &[u8]) -> Vec<u8> {
        let destination = self.receiver.join(name);
        assert!(!destination.exists());
        fs::write(self.sender.join(name), bytes).unwrap();
        let deadline = Instant::now() + Duration::from_secs(90);
        loop {
            match fs::read(&destination) {
                Ok(received) => return received,
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => (),
                Err(err) => panic!("attachment transport read: {err}"),
            }
            assert!(
                Instant::now() < deadline,
                "native delivery timed out: {name}"
            );
            thread::sleep(Duration::from_millis(150));
        }
    }

    fn control(&self, round: &str, slot: &std::ffi::OsStr, sender: &Side, receiver: &Side) {
        let bytes = fs::read(sender.controls().join(slot)).unwrap();
        fs::write(receiver.controls().join(slot), self.deliver(round, &bytes)).unwrap();
    }
}

fn resolver(side: &Side, store: &crate::FakeSecureStore) -> LocalDeviceKeyResolver {
    LocalDeviceKeyResolver::load(&side.root, BRAIN, store, ENV).unwrap()
}

fn snapshot(
    side: &Side,
    store: &crate::FakeSecureStore,
) -> crate::knowledge_materialization::VerifiedCanonicalSnapshot {
    let anchor = resolver(side, store).local_trust_anchor().unwrap();
    crate::knowledge_materialization::scan_verified_canonical_from_local_anchor_until(
        &side.root, BRAIN, &anchor, None,
    )
    .unwrap()
}

fn guarded(
    side: &Side,
    store: &crate::FakeSecureStore,
) -> crate::knowledge_materialization::RetrievalSafetyState {
    let (db, _) = crate::projection::open_projection_db(&side.root).unwrap();
    let gate = crate::knowledge_materialization::evaluate_retrieval_safety(
        &side.root,
        BRAIN,
        &[],
        &mut resolver(side, store),
        &db,
    );
    assert!(
        gate.allows_content(),
        "receiver retrieval hold: {:?}",
        gate.hold_reason()
    );
    gate
}

pub(in crate::managed_profile) fn run() {
    let transport = PathBuf::from(std::env::var_os("CRYPTEX_TEST_WIRE").unwrap());
    let a_id = std::env::var("CRYPTEX_TEST_DEVICE_A").unwrap();
    let b_id = std::env::var("CRYPTEX_TEST_DEVICE_B").unwrap();
    let (left, right, scratch, store) =
        crate::trusted_import_probe::tests::pair_with_transport(&a_id, &b_id);
    let mut a = Side::new(left.clone(), &a_id);
    let mut b = Side::new(right.clone(), &b_id);
    let sender = resolver(&a, &store)
        .local_trust_anchor()
        .unwrap()
        .device_id()
        .clone();
    let history = snapshot(&a, &store);
    let note = history
        .reader_device_chain(sender.as_str())
        .iter()
        .find_map(|record| match record {
            AuthenticatedSecurityRecord::V1(note) => Some(note),
            _ => None,
        })
        .unwrap();
    let bytes: Vec<_> = (0..=255).cycle().take(8193).collect();
    let selection = crate::ipc::attachment::SelectedFile {
        note_id: note.event_id.clone(),
        note_hash: note.content_hash.as_str().into(),
        bytes,
    };
    drop(history);
    crate::canonical_writer::isolated_attachment::append(&a.root, &store, ENV, &selection).unwrap();
    let history = snapshot(&a, &store);
    let link = history
        .reader_device_chain(sender.as_str())
        .iter()
        .find_map(|record| match record {
            AuthenticatedSecurityRecord::Attachment(link, _) => Some(link.event_id.clone()),
            _ => None,
        })
        .unwrap();
    drop(history);

    crate::test_temp::ATTACHMENT_PAIR.with(|pair| pair.set([left.clone(), right.clone()]).unwrap());
    let ab = Wire::new(&transport, "A", "B");
    let ba = Wire::new(&transport, "B", "A");
    let from_a = super::recovery::first_challenge(&mut a, &store);
    let from_b = super::recovery::first_challenge(&mut b, &store);
    let relative = PathBuf::from(sender.as_filename_stem()).join("00000000000000000000.seg");
    let published = a.root.join("transport/incoming").join(&relative);
    assert!(!published.exists(), "no export before reader verification");
    ab.control("challenge", &from_a, &a, &b);
    b.tick(&store);
    ba.control("response", &from_b, &b, &a);
    assert_eq!(a.tick(&store).copied, 1);
    ab.control("confirmation", &from_a, &a, &b);
    b.tick(&store);
    for side in [&a, &b] {
        assert!(
            matches!(&side.exporter.last_reader_status, Some(reader::Status::Verified(peers)) if peers.len()==1)
        );
    }

    crate::incoming_publication::recover_live_with_instance(&right, &mut b.instance, &store, ENV)
        .unwrap();
    let input = b.root.join("transport/incoming").join(&relative);
    fs::create_dir_all(input.parent().unwrap()).unwrap();
    fs::write(
        &input,
        ab.deliver("signed-pack", &fs::read(published).unwrap()),
    )
    .unwrap();
    let publish = |side: &mut Side| {
        crate::incoming_publication::publish_incoming_with_instance(
            &side.root,
            std::slice::from_ref(&relative),
            &mut side.instance,
            &store,
            ENV,
        )
        .unwrap()
    };
    assert_eq!(publish(&mut b).published_events, 3);
    assert_eq!(publish(&mut b).identical_segments, 1);

    let state = guarded(&b, &store);
    let note = &selection.note_id;
    assert_eq!(
        state.verify_local_attachment(note, &link),
        Err(LocalLinkError::Blob(BlobError::Missing))
    );
    let digest = Sha256::digest(&selection.bytes);
    let name = format!("{digest:x}");
    let source = fs::read(left.join("canonical/blobs").join(&name)).unwrap();
    let mut damaged = source.clone();
    damaged[0] ^= 1;
    let damaged = ab.deliver("damaged-file", &damaged);
    assert_eq!(
        state.receive_trial_attachment(note, &link, &damaged),
        Err(LocalLinkError::Blob(BlobError::IntegrityMismatch))
    );
    assert!(!right.join("canonical/blobs").join(&name).exists());
    let received = ab.deliver("selected-file", &source);
    assert!(
        state
            .receive_trial_attachment(&link, &link, &received)
            .is_err()
    );
    assert!(!right.join("canonical/blobs").exists());
    for _ in 0..2 {
        state
            .receive_trial_attachment(note, &link, &received)
            .unwrap();
    }
    drop(state);
    drop((a, b));
    let b = Side::new(right.clone(), &b_id);
    let state = guarded(&b, &store);
    state.verify_local_attachment(note, &link).unwrap();
    let blobs = right.join("canonical/blobs");
    assert_eq!(fs::read_dir(&blobs).unwrap().count(), 1);
    let local_blob = blobs.join(&name);
    assert_eq!(fs::read(&local_blob).unwrap(), selection.bytes);
    println!(
        "paired_attachment_transfer=verified bytes={} sha256={digest:x} receiver_reopened=true canonical_blobs=1",
        selection.bytes.len()
    );
    // Corrupt only this disposable receiver. No signed record changes during repair.
    let peer_pack = right.join("canonical/events").join(&relative);
    let original_pack = fs::read(&peer_pack).unwrap();
    let evidence = right.join("blob-staging/quarantine");
    fs::write(&local_blob, &damaged).unwrap();
    assert_eq!(
        state.verify_local_attachment(note, &link),
        Err(LocalLinkError::Blob(BlobError::IntegrityMismatch))
    );
    assert!(
        state
            .receive_trial_attachment(note, &link, &damaged)
            .is_err()
    );
    assert_eq!(fs::read(&local_blob).unwrap(), damaged);
    assert!(!evidence.exists());

    let replacement = ab.deliver("repair-file", &source);
    assert!(
        state
            .receive_trial_attachment(&link, &link, &replacement)
            .is_err()
    );
    assert!(!evidence.exists());
    let alias = right.join("shared-damaged-file");
    fs::hard_link(&local_blob, &alias).unwrap();
    assert_eq!(
        state.receive_trial_attachment(note, &link, &replacement),
        Err(LocalLinkError::Blob(BlobError::UnsafeFilesystem))
    );
    assert_eq!(fs::read(&alias).unwrap(), damaged);
    assert!(!evidence.exists());
    fs::remove_file(alias).unwrap();
    state
        .receive_trial_attachment(note, &link, &replacement)
        .unwrap();
    state.verify_local_attachment(note, &link).unwrap();
    assert_eq!(fs::read_dir(&evidence).unwrap().count(), 1);

    // A second incident interrupts the daemon after it preserves the damaged file.
    fs::write(&local_blob, &damaged).unwrap();
    crate::attachment_blobs::INTERRUPT_AFTER_QUARANTINE.with(|pending| pending.set(true));
    assert_eq!(
        state.receive_trial_attachment(note, &link, &replacement),
        Err(LocalLinkError::Blob(BlobError::Io(
            std::io::ErrorKind::Interrupted
        )))
    );
    assert_eq!(
        state.verify_local_attachment(note, &link),
        Err(LocalLinkError::Blob(BlobError::Missing))
    );
    drop(state);
    drop(b);
    let b = Side::new(right.clone(), &b_id);
    let state = guarded(&b, &store);
    let retry = ab.deliver("repair-retry", &source);
    state.receive_trial_attachment(note, &link, &retry).unwrap();
    state.receive_trial_attachment(note, &link, &retry).unwrap();
    drop(state);
    drop(b);
    let b = Side::new(right.clone(), &b_id);
    guarded(&b, &store)
        .verify_local_attachment(note, &link)
        .unwrap();
    assert_eq!(fs::read(local_blob).unwrap(), selection.bytes);
    assert_eq!(fs::read_dir(blobs).unwrap().count(), 1);
    let retained: Vec<_> = fs::read_dir(evidence)
        .unwrap()
        .map(|entry| fs::read(entry.unwrap().path()).unwrap())
        .collect();
    assert_eq!(retained, vec![damaged.clone(), damaged]);
    assert_eq!(fs::read(peer_pack).unwrap(), original_pack);
    println!(
        "paired_attachment_repair=verified receiver_reopened=true interrupted_retry=verified canonical_blobs=1 quarantined=2"
    );
    drop(b);
    for root in [left, right, scratch] {
        fs::remove_dir_all(root).unwrap();
    }
}
