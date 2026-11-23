//! M16 signed-pack fixture and offline projection recovery campaign.
use super::*;
use crate::event_signing_key::ensure_event_signing_key;
use crate::secure_store::FakeSecureStore;
use cryptex_events::{
    Classification, ContentHashHex, DeviceSeq, EVENT_SCHEMA_VERSION, EventPayload, EventType,
    MemoryNotePayload, SignableEvent, SigningKeyId, accept_event, encode_frame_header,
};
use cryptex_projection::{
    ProjectionReplaceReason, SearchQuery, projection_sqlite_path, search_events,
};
use cryptex_storage::sealed_segment_path;
use sha2::{Digest, Sha256};
use std::io::{BufWriter, Write};
use std::time::Instant;

const PACK_SIZE: u64 = 1000;
const BRAIN: &str = "6651823e-f82f-4c0e-817c-51edb9428556";
const ENV: SecureStoreEnvironment = SecureStoreEnvironment::Dev;

fn pack_digests(root: &Path, device: &DeviceId) -> Vec<(std::ffi::OsString, [u8; 32])> {
    let mut packs = Vec::new();
    for entry in fs::read_dir(canonical_events_device_dir(root, device)).unwrap() {
        let path = entry.unwrap().path();
        if path
            .extension()
            .is_some_and(|ext| ext == "seg" || ext == "open")
        {
            packs.push((
                path.file_name().unwrap().to_owned(),
                Sha256::digest(fs::read(path).unwrap()).into(),
            ));
        }
    }
    packs.sort_unstable();
    packs
}

fn synthetic_id(seq: u64) -> String {
    format!("8286fd36-4ca3-44d6-91b4-{seq:012x}")
}

/// Fixture setup uses the production envelope and frame formats. Timings start
/// after setup, so they measure the actual daemon reader and SQLite rebuild.
fn write_signed_fixture(
    root: &Path,
    device: &DeviceId,
    store: &FakeSecureStore,
    count: u64,
) -> ContentHashHex {
    let key = ensure_event_signing_key(store, ENV, root, device, &[], false).unwrap();
    let signing_key_id = SigningKeyId::parse(&key.public.signing_key_id).unwrap();
    fs::create_dir_all(canonical_events_device_dir(root, device)).unwrap();
    let mut previous = ContentHashHex::zero();
    for pack in 0..count.div_ceil(PACK_SIZE) {
        let mut file =
            BufWriter::new(fs::File::create_new(sealed_segment_path(root, device, pack)).unwrap());
        for seq in pack * PACK_SIZE + 1..=count.min((pack + 1) * PACK_SIZE) {
            let edge = if seq.is_multiple_of(PACK_SIZE) {
                " m16edge"
            } else {
                ""
            };
            let event = accept_event(
                SignableEvent {
                    schema_version: EVENT_SCHEMA_VERSION,
                    event_id: synthetic_id(seq),
                    brain_id: BRAIN.to_owned(),
                    device_id: device.as_str().to_owned(),
                    device_seq: DeviceSeq::from_u64(seq).unwrap(),
                    created_at: "2026-01-01T00:00:00Z".to_owned(),
                    event_type: EventType::MemoryNote,
                    classification: Classification::Internal,
                    signing_key_id: signing_key_id.clone(),
                    payload: EventPayload::MemoryNote(
                        MemoryNotePayload::new(format!("Synthetic archive note {seq}{edge}"))
                            .unwrap(),
                    ),
                    prev_content_hash: previous,
                },
                &key.signing_key,
            )
            .unwrap();
            let bytes = serde_json_canonicalizer::to_vec(&event).unwrap();
            file.write_all(&encode_frame_header(bytes.len().try_into().unwrap()).unwrap())
                .unwrap();
            file.write_all(&bytes).unwrap();
            previous = event.content_hash;
        }
        file.flush().unwrap();
    }
    previous
}

#[test]
#[ignore = "native M16 scale campaign; run in release mode"]
fn signed_pack_projection_recovers_at_scale() {
    let count = match std::env::var("CRYPTEX_M16_EVENTS").as_deref() {
        Ok("500000") => 500_000,
        Ok("100000") => 100_000,
        Ok("10000") | Err(_) => 10_000,
        Ok(_) => panic!("unsupported M16 scale workload"),
    };
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let device = DeviceId::new("m16-synthetic-projection").unwrap();
    let store = FakeSecureStore::new();
    let tip = write_signed_fixture(root, &device, &store, count);
    let packs = pack_digests(root, &device);
    assert_eq!(packs.len() as u64, count / PACK_SIZE);
    for phase in ["missing", "corrupt"] {
        if phase == "corrupt" {
            fs::write(
                projection_sqlite_path(root),
                b"synthetic damaged derived database",
            )
            .unwrap();
        }
        let start = Instant::now();
        let (db, opened) = open_projection_db(root).unwrap();
        match phase {
            "missing" => assert_eq!(opened, ProjectionOpenKind::Created),
            _ => assert!(matches!(
                opened,
                ProjectionOpenKind::Replaced {
                    reason: ProjectionReplaceReason::Corrupt { .. }
                }
            )),
        }
        let rebuilt = catch_up_device_projection(&store, ENV, root, &device, &db).unwrap();
        let rebuild_ms = start.elapsed().as_millis();
        assert!(rebuilt.rebuilt);
        assert_eq!(rebuilt.projected_events, count);
        drop(db);
        let (db, opened) = open_projection_db(root).unwrap();
        assert_eq!(opened, ProjectionOpenKind::OpenedExisting);
        let total: i64 = db
            .connection()
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .unwrap();
        assert_eq!(total, i64::try_from(count).unwrap());
        let checkpoint = read_device_checkpoint(db.connection(), device.as_str())
            .unwrap()
            .unwrap();
        assert_eq!(checkpoint.last_content_hash.as_deref(), Some(tip.as_str()));
        assert_eq!(checkpoint.last_device_seq.unwrap(), count.to_string());
        let start = Instant::now();
        let hits = search_events(
            db.connection(),
            &SearchQuery {
                text: Some("m16edge".into()),
                brain_id: Some(BRAIN.into()),
                ..Default::default()
            },
        )
        .unwrap();
        let search_ms = start.elapsed().as_millis();
        let expected: Vec<_> = (1..=count / PACK_SIZE)
            .map(|index| synthetic_id(index * PACK_SIZE))
            .collect();
        assert_eq!(
            hits.into_iter().map(|hit| hit.event_id).collect::<Vec<_>>(),
            expected
        );
        assert_eq!(pack_digests(root, &device), packs);
        println!(
            "m16_projection phase={phase} events={count} packs={} rebuild_ms={rebuild_ms} search_ms={search_ms}",
            packs.len()
        );
    }
}
