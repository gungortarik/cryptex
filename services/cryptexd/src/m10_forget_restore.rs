//! Disposable signed histories and an independent reference replay for the M10 note reader.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use cryptex_core::DeviceId;
use cryptex_events::{
    AcceptedEvent, Classification, ContentHashHex, DeviceSeq, EventPayload, EventType,
    MemoryNotePayload, SignableEvent, TrustedDaemonKeyResolver, VerifiedV2History, VersionedRecord,
    accept_event, signing_key_id_from_verifying_key, verify_v2_history,
};
use cryptex_projection::{ProjectionDb, SearchQuery, TimelineQuery};
use ed25519_dalek::{SigningKey, VerifyingKey};
use serde::Deserialize;
use serde_json::{Value, json};

const DEVICE: &str = "m10-fixture";
const BRAIN: &str = "aaaaaaaa-1111-4111-8111-000000000001";
const NOTE: &str = "aaaaaaaa-1111-4111-8111-000000000002";
const OTHER: &str = "aaaaaaaa-1111-4111-8111-000000000003";
const FORGET: &str = "aaaaaaaa-1111-4111-8111-000000000004";
const RESTORE: &str = "aaaaaaaa-1111-4111-8111-000000000005";
const TIME: &str = "2026-11-19T00:00:00Z";

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Operation {
    #[serde(rename = "memory.forget")]
    Forget,
    #[serde(rename = "memory.restore")]
    Restore,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Target {
    note_event_id: String,
    note_content_hash: ContentHashHex,
    prior_state_hash: ContentHashHex,
}

#[derive(Deserialize)]
struct Control {
    event_type: Operation,
    payload_schema_version: u32,
    brain_id: String,
    classification: Classification,
    payload: Target,
}

struct NoteState<'a> {
    original: &'a AcceptedEvent,
    latest: &'a ContentHashHex,
    forgotten: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum ReplayError {
    Unsupported,
    TargetMismatch,
    InvalidTransition,
}

fn visible_notes(history: &VerifiedV2History) -> Result<Vec<&AcceptedEvent>, ReplayError> {
    let mut notes = BTreeMap::new();
    for record in history.records() {
        match record {
            VersionedRecord::VerifiedV1(note) => {
                notes.insert(
                    note.event_id.as_str(),
                    NoteState {
                        original: note,
                        latest: &note.content_hash,
                        forgotten: false,
                    },
                );
            }
            VersionedRecord::VerifiedUnsupported(raw) => {
                let control: Control = serde_json::from_slice(&raw.raw_record)
                    .map_err(|_| ReplayError::Unsupported)?;
                if control.payload_schema_version != 1 {
                    return Err(ReplayError::Unsupported);
                }
                let state = notes
                    .get_mut(control.payload.note_event_id.as_str())
                    .ok_or(ReplayError::TargetMismatch)?;
                let note = state.original;
                if note.brain_id != control.brain_id
                    || note.device_id != raw.device_id
                    || note.signing_key_id != raw.signing_key_id
                    || note.classification != control.classification
                    || note.content_hash != control.payload.note_content_hash
                {
                    return Err(ReplayError::TargetMismatch);
                }
                let forget = matches!(control.event_type, Operation::Forget);
                if forget == state.forgotten || control.payload.prior_state_hash != *state.latest {
                    return Err(ReplayError::InvalidTransition);
                }
                state.forgotten = forget;
                state.latest = &raw.content_hash;
            }
            _ => return Err(ReplayError::Unsupported),
        }
    }
    Ok(notes
        .into_values()
        .filter(|s| !s.forgotten)
        .map(|s| s.original)
        .collect())
}

struct FixtureKey(VerifyingKey);

impl TrustedDaemonKeyResolver for FixtureKey {
    fn local_trust_anchor(&self) -> Option<cryptex_events::LocalTrustAnchor> {
        Some(cryptex_events::LocalTrustAnchor::new(
            DeviceId::new(DEVICE).unwrap(),
            signing_key_id_from_verifying_key(&self.0).unwrap(),
            self.0.to_bytes(),
        ))
    }

    fn trusted_daemon_key(&mut self, device: &str, id: &str) -> Option<VerifyingKey> {
        (device == DEVICE && signing_key_id_from_verifying_key(&self.0).unwrap().as_str() == id)
            .then_some(self.0)
    }
}

struct Profile(PathBuf);
impl Drop for Profile {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove disposable profile");
    }
}

fn note(
    key: &SigningKey,
    id: &str,
    seq: u64,
    previous: ContentHashHex,
    text: &str,
    classification: Classification,
) -> AcceptedEvent {
    accept_event(
        SignableEvent {
            schema_version: 1,
            event_id: id.into(),
            brain_id: BRAIN.into(),
            device_id: DEVICE.into(),
            device_seq: DeviceSeq::from_u64(seq).unwrap(),
            created_at: TIME.into(),
            event_type: EventType::MemoryNote,
            classification,
            signing_key_id: signing_key_id_from_verifying_key(&key.verifying_key()).unwrap(),
            payload: EventPayload::MemoryNote(MemoryNotePayload::new(text).unwrap()),
            prev_content_hash: previous,
        },
        key,
    )
    .unwrap()
}

fn control(
    note: &AcceptedEvent,
    id: &str,
    seq: u64,
    operation: &str,
    previous: &ContentHashHex,
    prior_state: &ContentHashHex,
) -> Value {
    json!({
        "schema_version":2, "event_id":id, "brain_id":BRAIN, "device_id":DEVICE,
        "device_seq":seq.to_string(), "created_at":TIME, "event_type":operation,
        "payload_schema_version":1, "classification":note.classification,
        "signing_key_id":note.signing_key_id, "prev_content_hash":previous,
        "payload":{"note_event_id":note.event_id, "note_content_hash":note.content_hash,
                   "prior_state_hash":prior_state}
    })
}

fn assert_reopened_views(
    profile: &Profile,
    key: VerifyingKey,
    original: &AcceptedEvent,
    expected_frames: &[&[u8]],
    visible: bool,
) {
    let device = DeviceId::new(DEVICE).unwrap();
    let bytes = fs::read(cryptex_storage::active_segment_path(&profile.0, &device, 0)).unwrap();
    let (frames, consumed) = cryptex_storage::scan_complete_frames(&bytes).unwrap();
    assert_eq!(consumed, bytes.len());
    let bodies: Vec<_> = frames
        .iter()
        .map(|f| &f[cryptex_events::FRAME_HEADER_BYTES..])
        .collect();
    assert_eq!(bodies, expected_frames);
    let history = verify_v2_history(&bodies, FixtureKey(key)).unwrap();
    let active = visible_notes(&history).unwrap();
    {
        let mut db = ProjectionDb::open_or_create(&profile.0).unwrap();
        let tx = db.connection_mut().transaction().unwrap();
        cryptex_projection::clear_all_projection_rows(&tx).unwrap();
        for note in active {
            cryptex_projection::project_memory_note(&tx, note).unwrap();
        }
        tx.commit().unwrap();
    }
    let db = ProjectionDb::open_existing_read_only(&profile.0).unwrap();
    let conn = db.connection();
    let query = SearchQuery {
        text: Some("orchid".into()),
        ..Default::default()
    };
    let hits = cryptex_projection::search_events(conn, &query).unwrap();
    let detail = cryptex_projection::event_detail(conn, BRAIN, NOTE).unwrap();
    if visible {
        let hit = detail.expect("target visible by ID");
        assert_eq!(hit.content_hash, original.content_hash.as_str());
        assert_eq!(hit.text, "orchid");
        assert_eq!(hit.classification, "restricted");
        assert_eq!(hits, [hit]);
    } else {
        assert!(detail.is_none() && hits.is_empty());
    }
    let timeline = cryptex_projection::timeline_events(
        conn,
        &TimelineQuery {
            limit: 5,
            ..Default::default()
        },
    )
    .unwrap();
    let ids: Vec<_> = timeline.hits.iter().map(|h| h.event_id.as_str()).collect();
    assert_eq!(ids.len(), 1 + usize::from(visible));
    assert_eq!(ids.contains(&NOTE), visible);
    assert!(ids.contains(&OTHER));
}

struct SignedFixture {
    key: SigningKey,
    target: AcceptedEvent,
    original: Vec<u8>,
    unaffected: Vec<u8>,
    forget: Vec<u8>,
    restore: Vec<u8>,
}

#[test]
fn vault_refresh_removes_and_restores_the_managed_note() {
    use crate::knowledge_materialization as reads;

    let profile = Profile(crate::test_temp::exclusive_temp_dir("cryptex-m10-profile"));
    let vault = Profile(crate::test_temp::exclusive_temp_dir("cryptex-m10-vault"));
    let signed = signed_fixture();
    let device = DeviceId::new(DEVICE).unwrap();
    let append = |bytes: &[u8]| {
        cryptex_storage::append_frame(&profile.0, &device, 0, bytes).unwrap();
    };
    append(&signed.original);
    append(&signed.unaffected);
    let key = signed.key.verifying_key();
    let anchor = FixtureKey(key).local_trust_anchor().unwrap();
    let refresh = || {
        let mut db = ProjectionDb::open_or_create(&profile.0).unwrap();
        reads::rebuild_knowledge_from_local_approval_history(&profile.0, BRAIN, &anchor, &mut db)
            .unwrap();
        reads::rebuild_vault_with_retrieval_safety(
            &profile.0,
            BRAIN,
            std::slice::from_ref(&device),
            &mut FixtureKey(key),
            &db,
            &vault.0,
        )
    };

    let personal = vault.0.join("Personal.md");
    fs::write(&personal, b"My own note").unwrap();
    let tree = vault.0.join("Cryptex");
    let note_path = tree.join("notes").join(format!("{NOTE}.md"));
    let other_path = tree.join("notes").join(format!("{OTHER}.md"));
    let manifest_path = tree.join(".cryptex-manifest.json");
    assert_eq!(refresh().unwrap().note_count, 2);
    let original = fs::read(&note_path).unwrap();
    assert_eq!(
        original,
        cryptex_projection::render_vault_note(&signed.target)
            .unwrap()
            .markdown
            .as_bytes()
    );
    let other = fs::read(&other_path).unwrap();
    let manifest_before = fs::read(&manifest_path).unwrap();

    append(&signed.forget);
    // A user's edit inside the managed tree must still block replacement.
    fs::write(&note_path, b"My pending edit").unwrap();
    assert!(matches!(
        refresh(),
        Err(reads::GuardedVaultError::Projection(_))
    ));
    assert_eq!(fs::read(&note_path).unwrap(), b"My pending edit");
    assert_eq!(fs::read(&manifest_path).unwrap(), manifest_before);
    fs::write(&note_path, &original).unwrap();

    for _ in 0..2 {
        let summary = refresh().unwrap();
        assert_eq!(summary.note_count, 1);
        assert_eq!(tree.join(summary.open_note_path.unwrap()), other_path);
        assert!(!note_path.try_exists().unwrap());
        let manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        let files = manifest["files"].as_object().unwrap();
        assert!(!files.contains_key(&format!("notes/{NOTE}.md")));
        assert!(files.contains_key(&format!("notes/{OTHER}.md")));
        assert_eq!(fs::read(&other_path).unwrap(), other);
        assert_eq!(fs::read(&personal).unwrap(), b"My own note");
    }

    append(&signed.restore);
    assert_eq!(refresh().unwrap().note_count, 2);
    assert_eq!(fs::read(note_path).unwrap(), original);
    assert_eq!(fs::read(manifest_path).unwrap(), manifest_before);
    assert_eq!(fs::read(other_path).unwrap(), other);
    assert_eq!(fs::read(personal).unwrap(), b"My own note");
}

fn signed_fixture() -> SignedFixture {
    let key = SigningKey::from_bytes(&[31; 32]);
    let target = note(
        &key,
        NOTE,
        1,
        ContentHashHex::zero(),
        "orchid",
        Classification::Restricted,
    );
    let other = note(
        &key,
        OTHER,
        2,
        target.content_hash.clone(),
        "fern",
        Classification::Public,
    );
    let original = serde_json_canonicalizer::to_vec(&target).unwrap();
    let unaffected = serde_json_canonicalizer::to_vec(&other).unwrap();
    let forget = crate::test_temp::sign_v2_fixture(
        control(
            &target,
            FORGET,
            3,
            "memory.forget",
            &other.content_hash,
            &target.content_hash,
        ),
        &key,
    );
    let signed: Value = serde_json::from_slice(&forget).unwrap();
    let forgotten_hash = serde_json::from_value(signed["content_hash"].clone()).unwrap();
    let restore = crate::test_temp::sign_v2_fixture(
        control(
            &target,
            RESTORE,
            4,
            "memory.restore",
            &forgotten_hash,
            &forgotten_hash,
        ),
        &key,
    );
    SignedFixture {
        key,
        target,
        original,
        unaffected,
        forget,
        restore,
    }
}

#[test]
fn forgotten_and_restored_views_survive_profile_reopen() {
    let profile = Profile(crate::test_temp::exclusive_temp_dir("cryptex-m10-forget"));
    let SignedFixture {
        key,
        target,
        original,
        unaffected,
        forget,
        restore,
    } = signed_fixture();
    let device = DeviceId::new(DEVICE).unwrap();
    let append = |body: &[u8]| {
        cryptex_storage::append_frame(&profile.0, &device, 0, body).unwrap();
    };
    let check = |records: &[&[u8]], visible| {
        assert_reopened_views(&profile, key.verifying_key(), &target, records, visible);
    };
    append(&original);
    append(&unaffected);
    check(&[&original, &unaffected], true);
    append(&forget);
    check(&[&original, &unaffected, &forget], false);
    cryptex_projection::remove_projection_files(&profile.0).unwrap();
    check(&[&original, &unaffected, &forget], false);
    append(&restore);
    check(&[&original, &unaffected, &forget, &restore], true);
    cryptex_projection::remove_projection_files(&profile.0).unwrap();
    check(&[&original, &unaffected, &forget, &restore], true);
}

#[test]
fn invalid_controls_cannot_change_visibility() {
    let key = SigningKey::from_bytes(&[31; 32]);
    let target = note(
        &key,
        NOTE,
        1,
        ContentHashHex::zero(),
        "orchid",
        Classification::Restricted,
    );
    let original = serde_json_canonicalizer::to_vec(&target).unwrap();
    let forget = control(
        &target,
        FORGET,
        2,
        "memory.forget",
        &target.content_hash,
        &target.content_hash,
    );
    for (field, value, error) in [
        (
            "/classification",
            json!("public"),
            ReplayError::TargetMismatch,
        ),
        (
            "/payload/note_event_id",
            json!(OTHER),
            ReplayError::TargetMismatch,
        ),
        (
            "/payload/note_content_hash",
            json!(ContentHashHex::zero()),
            ReplayError::TargetMismatch,
        ),
        (
            "/payload/prior_state_hash",
            json!(ContentHashHex::zero()),
            ReplayError::InvalidTransition,
        ),
        (
            "/event_type",
            json!("memory.restore"),
            ReplayError::InvalidTransition,
        ),
        (
            "/payload_schema_version",
            json!(2),
            ReplayError::Unsupported,
        ),
    ] {
        let mut changed = forget.clone();
        *changed.pointer_mut(field).unwrap() = value;
        let signed = crate::test_temp::sign_v2_fixture(changed, &key);
        let history =
            verify_v2_history(&[&original, &signed], FixtureKey(key.verifying_key())).unwrap();
        assert_eq!(visible_notes(&history).unwrap_err(), error, "{field}");
        use cryptex_events::security_history::{
            NoteLifecycleRead, verify_security_history_for_note_reads,
        };
        let cutoffs = BTreeMap::from([(DEVICE.to_owned(), u64::MAX)]);
        let product = verify_security_history_for_note_reads(
            &[&original, &signed],
            BRAIN,
            FixtureKey(key.verifying_key()),
            NoteLifecycleRead::Apply,
            || false,
        )
        .and_then(|history| history.forgotten_note_ids(&cutoffs, || false));
        assert!(product.is_err(), "product accepted {field}");
    }
    let signed = crate::test_temp::sign_v2_fixture(forget, &key);
    let mut forged: Value = serde_json::from_slice(&signed).unwrap();
    forged["event_type"] = json!("memory.restore");
    let forged = serde_json_canonicalizer::to_vec(&forged).unwrap();
    assert!(verify_v2_history(&[&original, &forged], FixtureKey(key.verifying_key())).is_err());
}

#[test]
fn normal_reads_enforce_signed_visibility_with_an_unchanged_index() {
    use crate::knowledge_materialization as reads;
    use std::time::{Duration, Instant};
    let profile = Profile(crate::test_temp::exclusive_temp_dir("cryptex-m10-guarded"));
    let SignedFixture {
        key,
        target,
        original,
        unaffected,
        forget,
        restore,
    } = signed_fixture();
    let device = DeviceId::new(DEVICE).unwrap();
    let devices = std::slice::from_ref(&device);
    let resolver = FixtureKey(key.verifying_key());
    let append = |bytes: &[u8]| {
        cryptex_storage::append_frame(&profile.0, &device, 0, bytes).unwrap();
    };
    append(&original);
    append(&unaffected);
    {
        let mut db = ProjectionDb::open_or_create(&profile.0).unwrap();
        reads::rebuild_knowledge_from_local_approval_history(
            &profile.0,
            BRAIN,
            &resolver.local_trust_anchor().unwrap(),
            &mut db,
        )
        .unwrap();
    }
    let query = SearchQuery {
        text: Some("orchid".into()),
        limit: Some(1),
        ..Default::default()
    };
    let check = |visible| {
        let db = ProjectionDb::open_existing_read_only(&profile.0).unwrap();
        let mut resolver = FixtureKey(key.verifying_key());
        let deadline = Some(Instant::now() + Duration::from_secs(10));
        let raw = cryptex_projection::search_events(db.connection(), &query).unwrap();
        assert_eq!(raw.len(), 1);
        assert_eq!(raw[0].content_hash, target.content_hash.as_str());
        let hits = reads::search_events_with_retrieval_safety_deadline(
            &profile.0,
            BRAIN,
            devices,
            &mut resolver,
            &db,
            &query,
            deadline,
        )
        .unwrap();
        let detail = reads::event_detail_with_retrieval_safety_deadline(
            &profile.0,
            BRAIN,
            devices,
            &mut resolver,
            &db,
            NOTE,
            deadline,
        )
        .unwrap();
        assert_eq!(detail.as_ref(), hits.first());
        if visible {
            assert_eq!(hits, raw);
        } else {
            assert!(hits.is_empty());
        }
        let timeline = reads::timeline_events_with_retrieval_safety_deadline(
            &profile.0,
            BRAIN,
            devices,
            &mut resolver,
            &db,
            &TimelineQuery {
                limit: 1,
                ..Default::default()
            },
            deadline,
        )
        .unwrap();
        assert_eq!(timeline.hits.len(), 1);
        assert_eq!(timeline.next_cursor.is_some(), visible);
        if !visible {
            assert_eq!(timeline.hits[0].event_id, OTHER);
        }
        let first = reads::search_events_with_retrieval_safety_deadline(
            &profile.0,
            BRAIN,
            devices,
            &mut resolver,
            &db,
            &SearchQuery {
                limit: Some(1),
                ..Default::default()
            },
            deadline,
        )
        .unwrap();
        assert_eq!(first[0].event_id, if visible { NOTE } else { OTHER });
    };
    check(true);
    append(&forget);
    check(false);
    // Consumers that do not apply lifecycle policy must still hold all reads.
    {
        let db = ProjectionDb::open_existing_read_only(&profile.0).unwrap();
        let safety = reads::evaluate_retrieval_safety(
            &profile.0,
            BRAIN,
            devices,
            &mut FixtureKey(key.verifying_key()),
            &db,
        );
        assert!(!safety.allows_content());
    }
    append(&restore);
    check(true);
    // A signed but stale restore cannot release any result from the old index.
    let raw: Value = serde_json::from_slice(&restore).unwrap();
    let tip = serde_json::from_value(raw["content_hash"].clone()).unwrap();
    append(&crate::test_temp::sign_v2_fixture(
        control(
            &target,
            "aaaaaaaa-1111-4111-8111-000000000006",
            5,
            "memory.restore",
            &tip,
            &target.content_hash,
        ),
        &key,
    ));
    let db = ProjectionDb::open_existing_read_only(&profile.0).unwrap();
    let result = reads::search_events_with_retrieval_safety(
        &profile.0,
        BRAIN,
        devices,
        &mut FixtureKey(key.verifying_key()),
        &db,
        &query,
    );
    assert!(matches!(
        result,
        Err(reads::GuardedSearchError::BrainWideHold(_))
    ));
}
