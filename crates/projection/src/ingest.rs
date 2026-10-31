//! Project verified M2 `memory.note` events into disposable SQLite rows (M3 Slice 4).
//!
//! Callers must supply events that already passed cryptexd verified-reader checks.
//! This module never opens `.open`/`.seg` packs.

use cryptex_events::{AcceptedEvent, Classification, EventPayload, EventType};
use rusqlite::{Connection, ErrorCode};

use crate::{ProjectionError, ProjectionResult};

/// Insert one verified `memory.note` into `events` and keep text-only FTS in sync.
///
/// Identical `event_id` replay is idempotent. A stored row with the same
/// `event_id` but different M2-derived fields returns
/// [`ProjectionError::EventConflict`]. Does not advance device checkpoints.
pub fn project_memory_note(conn: &Connection, event: &AcceptedEvent) -> ProjectionResult<()> {
    let text = match (&event.event_type, &event.payload) {
        (EventType::MemoryNote, EventPayload::MemoryNote(payload)) => payload.text.as_str(),
    };

    let insert = conn.execute(
        "INSERT INTO events (
            event_id, brain_id, device_id, device_seq, created_at,
            event_type, classification, content_hash, signing_key_id, text
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        (
            event.event_id.as_str(),
            event.brain_id.as_str(),
            event.device_id.as_str(),
            event.device_seq.as_str(),
            event.created_at.as_str(),
            event.event_type.as_str(),
            classification_as_str(event.classification),
            event.content_hash.as_str(),
            event.signing_key_id.as_str(),
            text,
        ),
    );

    match insert {
        Ok(1) => {
            let rowid: i64 = conn.query_row(
                "SELECT rowid FROM events WHERE event_id = ?1",
                [event.event_id.as_str()],
                |row| row.get(0),
            )?;
            conn.execute(
                "INSERT INTO events_fts(rowid, text) VALUES (?1, ?2)",
                (rowid, text),
            )?;
            Ok(())
        }
        Ok(changed) => Err(ProjectionError::SchemaInconsistent {
            detail: format!("unexpected INSERT row count {changed} for events"),
        }),
        Err(err) if is_events_event_id_conflict(&err) => {
            reconcile_existing_event(conn, event, text)
        }
        Err(err) => Err(err.into()),
    }
}

/// Project a stream of already-verified events inside one SQLite transaction.
///
/// On any insert error the transaction rolls back. Canonical verification errors
/// must be raised by the caller before/during the iterator and will prevent commit
/// if this function returns Err.
pub fn project_memory_notes<'a, I>(conn: &Connection, events: I) -> ProjectionResult<u64>
where
    I: IntoIterator<Item = &'a AcceptedEvent>,
{
    let tx = conn.unchecked_transaction()?;
    let mut count = 0u64;
    for event in events {
        project_memory_note(&tx, event)?;
        count = count.saturating_add(1);
    }
    tx.commit()?;
    Ok(count)
}

fn is_events_event_id_conflict(err: &rusqlite::Error) -> bool {
    match err {
        rusqlite::Error::SqliteFailure(efi, Some(message))
            if efi.code == ErrorCode::ConstraintViolation =>
        {
            let message = message.to_ascii_lowercase();
            message.contains("events.event_id")
                || (message.contains("unique") && message.contains("event_id"))
        }
        _ => false,
    }
}

fn reconcile_existing_event(
    conn: &Connection,
    event: &AcceptedEvent,
    text: &str,
) -> ProjectionResult<()> {
    let row: (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
    ) = conn.query_row(
        "SELECT brain_id, device_id, device_seq, created_at, event_type,
                classification, content_hash, signing_key_id, text
         FROM events WHERE event_id = ?1",
        [event.event_id.as_str()],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
            ))
        },
    )?;

    let expected = [
        ("brain_id", row.0.as_str(), event.brain_id.as_str()),
        ("device_id", row.1.as_str(), event.device_id.as_str()),
        ("device_seq", row.2.as_str(), event.device_seq.as_str()),
        ("created_at", row.3.as_str(), event.created_at.as_str()),
        ("event_type", row.4.as_str(), event.event_type.as_str()),
        (
            "classification",
            row.5.as_str(),
            classification_as_str(event.classification),
        ),
        ("content_hash", row.6.as_str(), event.content_hash.as_str()),
        (
            "signing_key_id",
            row.7.as_str(),
            event.signing_key_id.as_str(),
        ),
        ("text", row.8.as_str(), text),
    ];

    let mut differing_fields = Vec::new();
    for (name, stored, incoming) in expected {
        if stored != incoming {
            differing_fields.push(name);
        }
    }
    if differing_fields.is_empty() {
        return Ok(());
    }
    Err(ProjectionError::EventConflict {
        event_id: event.event_id.clone(),
        differing_fields,
    })
}

fn classification_as_str(value: Classification) -> &'static str {
    match value {
        Classification::Public => "public",
        Classification::Internal => "internal",
        Classification::Private => "private",
        Classification::Restricted => "restricted",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ProjectionDb;
    use cryptex_events::device_seq::DeviceSeq;
    use cryptex_events::hex::{ContentHashHex, SignatureHex, SigningKeyId};
    use cryptex_events::{EVENT_SCHEMA_VERSION, MemoryNotePayload};

    fn sample_note(event_id: &str, seq: u64, text: &str, content_hash: &str) -> AcceptedEvent {
        AcceptedEvent {
            schema_version: EVENT_SCHEMA_VERSION,
            event_id: event_id.into(),
            brain_id: "22222222-2222-4222-8222-222222222222".into(),
            device_id: "device-a".into(),
            device_seq: DeviceSeq::from_u64(seq).unwrap(),
            created_at: "2026-10-08T00:00:00Z".into(),
            event_type: EventType::MemoryNote,
            classification: Classification::Internal,
            signing_key_id: SigningKeyId::parse(&"ab".repeat(16)).unwrap(),
            payload: EventPayload::MemoryNote(MemoryNotePayload::new(text).unwrap()),
            prev_content_hash: ContentHashHex::zero(),
            content_hash: ContentHashHex::parse(content_hash).unwrap(),
            signature: SignatureHex::parse(&"ef".repeat(64)).unwrap(),
        }
    }

    #[test]
    fn projects_memory_note_into_events_and_fts() {
        let db = ProjectionDb::open_in_memory().expect("db");
        let note = sample_note(
            "11111111-1111-4111-8111-111111111111",
            1,
            "hello projection",
            &"cd".repeat(32),
        );
        project_memory_note(db.connection(), &note).expect("project");
        let (count, text): (i64, String) = db
            .connection()
            .query_row(
                "SELECT COUNT(*), text FROM events WHERE event_id = ?1",
                [note.event_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(text, "hello projection");
        let fts: i64 = db
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM events_fts WHERE events_fts MATCH 'projection'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(fts, 1);
    }

    #[test]
    fn identical_replay_is_idempotent_single_event_and_fts_row() {
        let db = ProjectionDb::open_in_memory().expect("db");
        let note = sample_note(
            "11111111-1111-4111-8111-111111111111",
            1,
            "replay-me",
            &"cd".repeat(32),
        );
        project_memory_note(db.connection(), &note).expect("first");
        project_memory_note(db.connection(), &note).expect("identical replay");
        let events: i64 = db
            .connection()
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .unwrap();
        let fts: i64 = db
            .connection()
            .query_row("SELECT COUNT(*) FROM events_fts", [], |row| row.get(0))
            .unwrap();
        assert_eq!(events, 1);
        assert_eq!(fts, 1);
    }

    #[test]
    fn conflicting_replay_returns_typed_error_and_leaves_row_unchanged() {
        let db = ProjectionDb::open_in_memory().expect("db");
        let original = sample_note(
            "11111111-1111-4111-8111-111111111111",
            1,
            "keep-me",
            &"cd".repeat(32),
        );
        project_memory_note(db.connection(), &original).expect("first");

        let mut conflict = original.clone();
        conflict.content_hash = ContentHashHex::parse(&"ab".repeat(32)).unwrap();
        let err = project_memory_note(db.connection(), &conflict).expect_err("must conflict");
        match &err {
            ProjectionError::EventConflict {
                event_id,
                differing_fields,
            } => {
                assert_eq!(event_id, original.event_id.as_str());
                assert_eq!(differing_fields.as_slice(), ["content_hash"]);
                let rendered = err.to_string();
                assert!(!rendered.contains("keep-me"));
                assert!(!rendered.contains(&"ab".repeat(32)));
            }
            other => panic!("unexpected error: {other}"),
        }

        let (hash, text): (String, String) = db
            .connection()
            .query_row(
                "SELECT content_hash, text FROM events WHERE event_id = ?1",
                [original.event_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(hash, "cd".repeat(32));
        assert_eq!(text, "keep-me");
        let fts: i64 = db
            .connection()
            .query_row("SELECT COUNT(*) FROM events_fts", [], |row| row.get(0))
            .unwrap();
        assert_eq!(fts, 1);
    }

    #[test]
    fn project_memory_notes_is_transactional_and_idempotent() {
        let db = ProjectionDb::open_in_memory().expect("db");
        let a = sample_note(
            "11111111-1111-4111-8111-111111111111",
            1,
            "one",
            &"cd".repeat(32),
        );
        let b = sample_note(
            "11111111-1111-4111-8111-111111111112",
            2,
            "two",
            &"ce".repeat(32),
        );
        assert_eq!(project_memory_notes(db.connection(), [&a, &b]).unwrap(), 2);
        assert_eq!(project_memory_notes(db.connection(), [&a]).unwrap(), 1);
        let count: i64 = db
            .connection()
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }
}
