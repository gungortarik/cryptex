//! Continue on the genuinely paired profiles while the two native workers share
//! the live exchange directories.
use super::*;
use crate::incoming_publication::{IncomingPublicationReport, publish_incoming_with_instance};
use cryptex_events::{AcceptedEvent, ContentHashHex};
use serde_json::json;

pub(super) mod conflict;
mod offline;
mod purge;
pub(super) use offline::resume;
pub(super) use purge::unconfirmed_purge;

fn publish(
    side: &mut Side,
    segments: &[PathBuf],
    store: &dyn crate::secure_store::SecureStore,
) -> IncomingPublicationReport {
    publish_incoming_with_instance(&side.root, segments, &mut side.instance, store, ENV).unwrap()
}

fn search(
    side: &Side,
    store: &dyn crate::secure_store::SecureStore,
) -> Vec<cryptex_projection::SearchHit> {
    let db = cryptex_projection::ProjectionDb::open_existing_read_only(&side.root).unwrap();
    crate::knowledge_materialization::search_events_with_retrieval_safety(
        &side.root,
        BRAIN,
        &[],
        &mut resolver(side, store),
        &db,
        &cryptex_projection::SearchQuery::default(),
    )
    .unwrap()
}

fn signed_control(
    note: &AcceptedEvent,
    store: &dyn crate::secure_store::SecureStore,
    sender: &Side,
    event_id: &str,
    kind: &str,
    prior: &ContentHashHex,
) -> (Vec<u8>, ContentHashHex) {
    let device = resolver(sender, store)
        .local_trust_anchor()
        .unwrap()
        .device_id()
        .clone();
    let history = snapshot(sender, store);
    let tip = history
        .reader_device_chain(device.as_str())
        .last()
        .unwrap()
        .content_hash()
        .clone();
    drop(history);
    let key = crate::event_signing_key::load_existing_event_signing_key(
        store,
        ENV,
        &sender.root,
        &device,
    )
    .unwrap();
    let seq = crate::sequence_runtime::allocate_device_sequence(&sender.root, &device).unwrap();
    let raw = crate::test_temp::sign_v2_fixture(
        json!({
            "schema_version": 2, "event_type": kind, "payload_schema_version": 1,
            "brain_id": note.brain_id, "device_id": device.as_str(), "device_seq": seq.to_string(),
            "event_id": event_id, "created_at": "2026-11-19T00:00:00Z",
            "signing_key_id": key.public.signing_key_id, "classification": note.classification,
            "prev_content_hash": tip, "payload": { "note_event_id": note.event_id,
                "note_content_hash": note.content_hash, "prior_state_hash": prior }
        }),
        &key.signing_key,
    );
    let signed: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    let hash = serde_json::from_value(signed["content_hash"].clone()).unwrap();
    (raw, hash)
}

pub(super) fn exercise(
    wire: &Path,
    sender: &mut Side,
    receiver: &mut Side,
    store: &dyn crate::secure_store::SecureStore,
) {
    crate::test_temp::LIFECYCLE_PAIR.with(|pair| {
        pair.set([sender.root.clone(), receiver.root.clone()])
            .unwrap()
    });
    let author = resolver(sender, store)
        .local_trust_anchor()
        .unwrap()
        .device_id()
        .clone();
    let history = snapshot(sender, store);
    let target = history
        .reader_device_chain(author.as_str())
        .iter()
        .find_map(|record| match record {
            AuthenticatedSecurityRecord::V1(note) => Some(note.clone()),
            _ => None,
        })
        .unwrap();
    drop(history);
    let baseline = search(receiver, store);
    assert_eq!(baseline.len(), 2);
    let original = baseline
        .iter()
        .find(|hit| hit.event_id == target.event_id)
        .unwrap();
    assert_eq!(original.content_hash, target.content_hash.as_str());
    let mut hidden = baseline.clone();
    hidden.retain(|hit| hit.event_id != target.event_id);
    let mut prior = target.content_hash.clone();
    for (index, kind, id, before, after) in [
        (
            1,
            "memory.forget",
            "55555555-5555-4555-8555-555555555551",
            &baseline,
            &hidden,
        ),
        (
            2,
            "memory.restore",
            "55555555-5555-4555-8555-555555555552",
            &hidden,
            &baseline,
        ),
    ] {
        let (raw, hash) = signed_control(&target, store, sender, id, kind, &prior);
        prior = hash;
        cryptex_storage::append_frame(&sender.root, &author, index, &raw).unwrap();
        let relative = PathBuf::from(author.as_filename_stem()).join(format!("{index:020}.seg"));
        let remote_inbox = receiver.root.join("transport/incoming").join(&relative);
        let remote_pack = receiver.root.join("canonical/events").join(&relative);
        let source_pack = cryptex_storage::sealed_segment_path(&sender.root, &author, index);
        let deadline = Instant::now() + Duration::from_secs(75);
        while !remote_inbox.is_file() {
            sender.tick(store);
            assert!(
                Instant::now() < deadline,
                "native lifecycle delivery timed out: {kind}"
            );
            thread::sleep(Duration::from_millis(150));
        }
        assert_eq!(&search(receiver, store), before);
        assert!(!remote_pack.try_exists().unwrap());
        let batch = std::slice::from_ref(&relative);
        assert_eq!(publish(receiver, batch, store).published_events, 1);
        let bytes = fs::read(source_pack).unwrap();
        assert_eq!(fs::read(&remote_pack).unwrap(), bytes);
        let (frames, consumed) = cryptex_storage::scan_complete_frames(&bytes).unwrap();
        assert_eq!(consumed, bytes.len());
        assert_eq!(frames.len(), 1);
        assert_eq!(&frames[0][cryptex_events::FRAME_HEADER_BYTES..], &raw);
        assert_eq!(publish(receiver, batch, store).identical_segments, 1);
        assert_eq!(&search(receiver, store), after);
    }
    println!(
        "paired_note_lifecycle=verified receiver_search=visible-hidden-restored signed_packs=2 identical_replays=2"
    );
    offline::exercise(wire, sender, receiver, store, &target, prior);
}
