//! Projection schema version 1 (M3 Slice 3).
//!
//! Columns are derived strictly from accepted M2 `AcceptedEvent` / `memory.note`
//! fields. This module creates empty disposable tables only; ingestion and
//! search belong to later slices.

use rusqlite::Connection;

use crate::fts::ensure_fts5_available;
use crate::{ProjectionError, ProjectionResult};

/// Derived projection schema version owned by this crate (not event schema_version).
pub const PROJECTION_SCHEMA_VERSION: u32 = 1;

/// Stable build marker written into `projection_meta` for empty v1 databases.
pub const PROJECTION_BUILD_ID_V1: &str = "cryptex-projection-schema-v1";

const DDL_V1: &str = r#"
CREATE TABLE projection_meta (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    schema_version INTEGER NOT NULL,
    built_at TEXT NOT NULL,
    build_id TEXT NOT NULL
);

CREATE TABLE events (
    event_id TEXT PRIMARY KEY NOT NULL,
    brain_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    device_seq TEXT NOT NULL,
    created_at TEXT NOT NULL,
    event_type TEXT NOT NULL,
    classification TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    signing_key_id TEXT NOT NULL,
    text TEXT NOT NULL
);

CREATE VIRTUAL TABLE events_fts USING fts5(
    text,
    content='events',
    content_rowid='rowid'
);

CREATE TABLE device_checkpoint (
    device_id TEXT PRIMARY KEY NOT NULL,
    last_device_seq TEXT,
    last_content_hash TEXT,
    last_sealed_index INTEGER,
    open_verified_prefix_len INTEGER
);
"#;

/// Apply the empty v1 schema and singleton metadata row.
pub fn apply_schema_v1(conn: &Connection, built_at: &str) -> ProjectionResult<()> {
    ensure_fts5_available(conn)?;
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(DDL_V1)?;
    tx.execute(
        "INSERT INTO projection_meta (id, schema_version, built_at, build_id) VALUES (1, ?1, ?2, ?3)",
        (PROJECTION_SCHEMA_VERSION as i64, built_at, PROJECTION_BUILD_ID_V1),
    )?;
    tx.commit()?;
    Ok(())
}

/// Read the singleton projection schema_version, if metadata exists.
pub fn read_schema_version(conn: &Connection) -> ProjectionResult<Option<u32>> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='projection_meta')",
        [],
        |row| row.get(0),
    )?;
    if !exists {
        return Ok(None);
    }
    let version: i64 = conn.query_row(
        "SELECT schema_version FROM projection_meta WHERE id = 1",
        [],
        |row| row.get(0),
    )?;
    if version < 0 || version > u32::MAX as i64 {
        return Err(ProjectionError::SchemaInconsistent {
            detail: format!("projection_meta.schema_version out of range: {version}"),
        });
    }
    Ok(Some(version as u32))
}

/// Fail closed unless schema_version matches this crate and required objects exist.
pub fn validate_schema_v1(conn: &Connection) -> ProjectionResult<()> {
    ensure_fts5_available(conn)?;
    let Some(found) = read_schema_version(conn)? else {
        return Err(ProjectionError::SchemaInconsistent {
            detail: "projection_meta table missing".into(),
        });
    };
    if found != PROJECTION_SCHEMA_VERSION {
        return Err(ProjectionError::UnsupportedSchema {
            found,
            expected: PROJECTION_SCHEMA_VERSION,
        });
    }

    require_table(conn, "events")?;
    require_table(conn, "device_checkpoint")?;
    require_fts_events(conn)?;
    require_events_columns(conn)?;
    Ok(())
}

fn require_table(conn: &Connection, name: &str) -> ProjectionResult<()> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [name],
        |row| row.get(0),
    )?;
    if !exists {
        return Err(ProjectionError::SchemaInconsistent {
            detail: format!("required table '{name}' is missing"),
        });
    }
    Ok(())
}

fn require_fts_events(conn: &Connection) -> ProjectionResult<()> {
    let sql: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE name='events_fts'",
            [],
            |row| row.get(0),
        )
        .optional_sql()?;
    let Some(sql) = sql else {
        return Err(ProjectionError::SchemaInconsistent {
            detail: "events_fts virtual table is missing".into(),
        });
    };
    let lowered = sql.to_ascii_lowercase();
    if !lowered.contains("using fts5") {
        return Err(ProjectionError::SchemaInconsistent {
            detail: "events_fts is not an fts5 virtual table".into(),
        });
    }
    if !lowered.contains("content='events'") && !lowered.contains("content=\"events\"") {
        return Err(ProjectionError::SchemaInconsistent {
            detail: "events_fts must use content='events'".into(),
        });
    }
    Ok(())
}

fn require_events_columns(conn: &Connection) -> ProjectionResult<()> {
    let expected = [
        "event_id",
        "brain_id",
        "device_id",
        "device_seq",
        "created_at",
        "event_type",
        "classification",
        "content_hash",
        "signing_key_id",
        "text",
    ];
    let mut stmt = conn.prepare("PRAGMA table_info(events)")?;
    let found: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    for col in expected {
        if !found.iter().any(|c| c == col) {
            return Err(ProjectionError::SchemaInconsistent {
                detail: format!("events table missing column '{col}'"),
            });
        }
    }
    // Reject opportunistic M4-ish columns if someone mutates the disposable DB.
    for forbidden in ["entity_id", "fact_id", "relation_id", "embedding", "vector"] {
        if found.iter().any(|c| c == forbidden) {
            return Err(ProjectionError::SchemaInconsistent {
                detail: format!("events table has unsupported semantic column '{forbidden}'"),
            });
        }
    }
    Ok(())
}

trait OptionalSql<T> {
    fn optional_sql(self) -> ProjectionResult<Option<T>>;
}

impl<T> OptionalSql<T> for Result<T, rusqlite::Error> {
    fn optional_sql(self) -> ProjectionResult<Option<T>> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }
}
