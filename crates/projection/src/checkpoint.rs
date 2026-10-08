//! Per-device projection checkpoints (M3 Slice 5).
//!
//! Checkpoints are disposable derived state. They must be written in the same
//! SQLite transaction as the projected rows they describe.

use rusqlite::Connection;

use crate::{ProjectionError, ProjectionResult};

/// Last successfully projected tip for one device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceCheckpoint {
    pub device_id: String,
    pub last_device_seq: Option<String>,
    pub last_content_hash: Option<String>,
    pub last_sealed_index: Option<i64>,
    pub open_verified_prefix_len: Option<i64>,
}

/// Read the checkpoint row for `device_id`, if present.
pub fn read_device_checkpoint(
    conn: &Connection,
    device_id: &str,
) -> ProjectionResult<Option<DeviceCheckpoint>> {
    let mut stmt = conn.prepare(
        "SELECT device_id, last_device_seq, last_content_hash, last_sealed_index,
                open_verified_prefix_len
         FROM device_checkpoint WHERE device_id = ?1",
    )?;
    let mut rows = stmt.query([device_id])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };
    Ok(Some(DeviceCheckpoint {
        device_id: row.get(0)?,
        last_device_seq: row.get(1)?,
        last_content_hash: row.get(2)?,
        last_sealed_index: row.get(3)?,
        open_verified_prefix_len: row.get(4)?,
    }))
}

/// Insert or replace the checkpoint row for a device.
pub fn upsert_device_checkpoint(
    conn: &Connection,
    checkpoint: &DeviceCheckpoint,
) -> ProjectionResult<()> {
    let changed = conn.execute(
        "INSERT INTO device_checkpoint (
            device_id, last_device_seq, last_content_hash,
            last_sealed_index, open_verified_prefix_len
         ) VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(device_id) DO UPDATE SET
            last_device_seq = excluded.last_device_seq,
            last_content_hash = excluded.last_content_hash,
            last_sealed_index = excluded.last_sealed_index,
            open_verified_prefix_len = excluded.open_verified_prefix_len",
        (
            checkpoint.device_id.as_str(),
            checkpoint.last_device_seq.as_deref(),
            checkpoint.last_content_hash.as_deref(),
            checkpoint.last_sealed_index,
            checkpoint.open_verified_prefix_len,
        ),
    )?;
    if changed == 0 {
        return Err(ProjectionError::SchemaInconsistent {
            detail: "device_checkpoint upsert changed 0 rows".into(),
        });
    }
    Ok(())
}

/// Remove projected rows and FTS entries for one device (keeps other devices).
pub fn clear_device_projection(conn: &Connection, device_id: &str) -> ProjectionResult<()> {
    let mut stmt = conn.prepare("SELECT rowid, text FROM events WHERE device_id = ?1")?;
    let rows = stmt
        .query_map([device_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);
    for (rowid, text) in rows {
        conn.execute(
            "INSERT INTO events_fts(events_fts, rowid, text) VALUES('delete', ?1, ?2)",
            (rowid, text),
        )?;
    }
    conn.execute("DELETE FROM events WHERE device_id = ?1", [device_id])?;
    conn.execute(
        "DELETE FROM device_checkpoint WHERE device_id = ?1",
        [device_id],
    )?;
    Ok(())
}

/// Remove all projected rows/checkpoints (keeps `projection_meta`).
pub fn clear_all_projection_rows(conn: &Connection) -> ProjectionResult<()> {
    conn.execute_batch(
        "DELETE FROM events_fts;
         DELETE FROM events;
         DELETE FROM device_checkpoint;",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProjectionDb;
    use crate::ingest::project_memory_note;
    use cryptex_events::device_seq::DeviceSeq;
    use cryptex_events::envelope::{
        AcceptedEvent, Classification, EVENT_SCHEMA_VERSION, EventPayload, EventType,
        MemoryNotePayload,
    };
    use cryptex_events::hex::{ContentHashHex, SignatureHex, SigningKeyId};

    fn sample_note(event_id: &str, seq: u64, text: &str, content_hash: &str) -> AcceptedEvent {
        AcceptedEvent {
            schema_version: EVENT_SCHEMA_VERSION,
            event_id: event_id.into(),
            brain_id: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".into(),
            device_id: "ckpt-device".into(),
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
    fn upsert_and_read_checkpoint_round_trip() {
        let db = ProjectionDb::open_in_memory().unwrap();
        let cp = DeviceCheckpoint {
            device_id: "ckpt-device".into(),
            last_device_seq: Some("3".into()),
            last_content_hash: Some("ab".repeat(32)),
            last_sealed_index: Some(1),
            open_verified_prefix_len: Some(42),
        };
        upsert_device_checkpoint(db.connection(), &cp).unwrap();
        let loaded = read_device_checkpoint(db.connection(), "ckpt-device")
            .unwrap()
            .expect("row");
        assert_eq!(loaded, cp);
    }

    #[test]
    fn clear_device_projection_removes_events_fts_and_checkpoint() {
        let db = ProjectionDb::open_in_memory().unwrap();
        let note = sample_note(
            "11111111-1111-4111-8111-111111111111",
            1,
            "hello",
            &"cd".repeat(32),
        );
        project_memory_note(db.connection(), &note).unwrap();
        upsert_device_checkpoint(
            db.connection(),
            &DeviceCheckpoint {
                device_id: "ckpt-device".into(),
                last_device_seq: Some("1".into()),
                last_content_hash: Some("cd".repeat(32)),
                last_sealed_index: None,
                open_verified_prefix_len: None,
            },
        )
        .unwrap();
        clear_device_projection(db.connection(), "ckpt-device").unwrap();
        let events: i64 = db
            .connection()
            .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))
            .unwrap();
        let fts: i64 = db
            .connection()
            .query_row("SELECT COUNT(*) FROM events_fts", [], |r| r.get(0))
            .unwrap();
        let cps: i64 = db
            .connection()
            .query_row("SELECT COUNT(*) FROM device_checkpoint", [], |r| r.get(0))
            .unwrap();
        assert_eq!(events, 0);
        assert_eq!(fts, 0);
        assert_eq!(cps, 0);
    }
}
