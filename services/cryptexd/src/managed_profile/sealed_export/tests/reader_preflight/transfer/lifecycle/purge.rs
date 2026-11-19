//! Negative wire fixture only. This does not define a purge ledger format.
use super::super::automatic::wait_for;
use super::*;
use crate::incoming_publication::{IncomingPublicationError, recover_live_with_instance};
use crate::test_temp::event_hashes;

pub(in super::super) fn unconfirmed_purge(
    source: &Side,
    receiver: &mut Side,
    store: &dyn crate::secure_store::SecureStore,
) {
    let author = resolver(source, store)
        .local_trust_anchor()
        .unwrap()
        .device_id()
        .clone();
    let snapshot = snapshot(source, store);
    let chain = snapshot.reader_device_chain(author.as_str());
    let target = chain
        .iter()
        .find_map(|record| match record {
            AuthenticatedSecurityRecord::V1(note) => Some(note.clone()),
            _ => None,
        })
        .unwrap();
    let tip = chain.last().unwrap().content_hash().clone();
    drop(snapshot);
    let visible = search(receiver, store);
    assert!(!visible.is_empty());
    assert!(visible.iter().all(|hit| hit.event_id != target.event_id));
    let original = event_hashes(&receiver.root);
    let source_original = event_hashes(&source.root);
    assert!(!original.is_empty() && !source_original.is_empty());
    let blobs = receiver.root.join("canonical/blobs");
    let blob_files = || {
        fs::read_dir(&blobs)
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                (entry.file_name(), fs::read(entry.path()).unwrap())
            })
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    let attachments = blob_files();
    assert!(!attachments.is_empty());
    // Even an authentic signature from the note's original device, naming
    // the correct forgotten note and its current state, gives no owner consent.
    let (body, _) = signed_control(
        &target,
        store,
        source,
        "77777777-7777-4777-8777-777777777778",
        "memory.purge",
        &tip,
    );
    let mut pack = cryptex_events::encode_frame_header(body.len().try_into().unwrap())
        .unwrap()
        .to_vec();
    pack.extend_from_slice(&body);
    let path = PathBuf::from(author.as_filename_stem()).join("00000000000000000008.seg");
    let outgoing = source.root.join("transport/incoming").join(&path);
    let incoming = receiver.root.join("transport/incoming").join(&path);
    assert!(!outgoing.exists() && !incoming.exists());
    let temporary = source.root.join("unconfirmed-purge.tmp");
    fs::write(&temporary, &pack).unwrap();
    fs::rename(temporary, outgoing).unwrap();
    wait_for(
        || fs::read(&incoming).is_ok_and(|bytes| bytes == pack),
        "unconfirmed purge delivery",
    );
    for _ in 0..2 {
        let result = publish_incoming_with_instance(
            &receiver.root,
            std::slice::from_ref(&path),
            &mut receiver.instance,
            store,
            ENV,
        );
        assert!(
            matches!(result, Err(IncomingPublicationError::Rejected(crate::import_staging::ImportStagingError::Invalid(ref reason))) if reason == "event payload schema is not supported for import"),
            "{result:?}"
        );
        recover_live_with_instance(&receiver.root, &mut receiver.instance, store, ENV).unwrap();
        assert_eq!(event_hashes(&receiver.root), original);
        assert_eq!(event_hashes(&source.root), source_original);
        assert_eq!(blob_files(), attachments);
        assert_eq!(search(receiver, store), visible);
    }
    println!(
        "unconfirmed_peer_purge=refused deliveries=2 history_unchanged=true blobs_unchanged=true"
    );
}
