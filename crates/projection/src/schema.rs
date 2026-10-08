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

const EVENTS_COLUMNS: &[&str] = &[
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

const PROJECTION_META_COLUMNS: &[&str] = &["id", "schema_version", "built_at", "build_id"];

const DEVICE_CHECKPOINT_COLUMNS: &[&str] = &[
    "device_id",
    "last_device_seq",
    "last_content_hash",
    "last_sealed_index",
    "open_verified_prefix_len",
];

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

    require_table(conn, "projection_meta")?;
    require_table(conn, "events")?;
    require_table(conn, "device_checkpoint")?;
    require_exact_columns(conn, "projection_meta", PROJECTION_META_COLUMNS)?;
    require_exact_columns(conn, "events", EVENTS_COLUMNS)?;
    require_exact_columns(conn, "device_checkpoint", DEVICE_CHECKPOINT_COLUMNS)?;
    require_fts_events(conn)?;
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
    let row: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT type, sql FROM sqlite_master WHERE name='events_fts'",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .optional_sql()?;
    let Some((kind, sql)) = row else {
        return Err(ProjectionError::SchemaInconsistent {
            detail: "events_fts virtual table is missing".into(),
        });
    };
    if kind != "table" {
        return Err(ProjectionError::SchemaInconsistent {
            detail: format!("events_fts has unexpected sqlite_master type '{kind}'"),
        });
    }
    let Some(sql) = sql else {
        return Err(ProjectionError::SchemaInconsistent {
            detail: "events_fts has no CREATE sql in sqlite_master".into(),
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
    // Text-only FTS: the user-defined column list must be exactly `text`.
    let after_fts5 =
        lowered
            .split("using fts5")
            .nth(1)
            .ok_or_else(|| ProjectionError::SchemaInconsistent {
                detail: "events_fts CREATE sql missing fts5 body".into(),
            })?;
    let cols_start = after_fts5
        .find('(')
        .ok_or_else(|| ProjectionError::SchemaInconsistent {
            detail: "events_fts CREATE sql missing column list".into(),
        })?;
    let cols_end =
        after_fts5[cols_start..]
            .find(')')
            .ok_or_else(|| ProjectionError::SchemaInconsistent {
                detail: "events_fts CREATE sql has unclosed column list".into(),
            })?
            + cols_start;
    let cols_body = after_fts5[cols_start + 1..cols_end].trim();
    let user_cols: Vec<&str> = cols_body
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty() && !part.contains('='))
        .collect();
    if user_cols != ["text"] {
        return Err(ProjectionError::SchemaInconsistent {
            detail: format!("events_fts must expose only text column, found {user_cols:?}"),
        });
    }
    Ok(())
}

fn require_exact_columns(
    conn: &Connection,
    table: &str,
    expected: &[&str],
) -> ProjectionResult<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let found: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    for col in expected {
        if !found.iter().any(|c| c == col) {
            return Err(ProjectionError::SchemaInconsistent {
                detail: format!("{table} table missing required column '{col}'"),
            });
        }
    }
    for col in &found {
        if !expected.iter().any(|e| e == col) {
            return Err(ProjectionError::SchemaInconsistent {
                detail: format!("{table} table has unexpected column '{col}'"),
            });
        }
    }
    if found.len() != expected.len() {
        return Err(ProjectionError::SchemaInconsistent {
            detail: format!(
                "{table} table column count mismatch: found {}, expected {}",
                found.len(),
                expected.len()
            ),
        });
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ProjectionDb;

    #[test]
    fn rejects_unexpected_events_column() {
        let db = ProjectionDb::open_in_memory().expect("memory");
        db.connection()
            .execute("ALTER TABLE events ADD COLUMN entity_id TEXT", [])
            .unwrap();
        let err = validate_schema_v1(db.connection()).expect_err("must refuse");
        match err {
            ProjectionError::SchemaInconsistent { detail } => {
                assert!(detail.contains("unexpected column 'entity_id'"), "{detail}");
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn rejects_missing_required_events_table() {
        let db = ProjectionDb::open_in_memory().expect("memory");
        db.connection()
            .execute_batch("DROP TABLE events; DROP TABLE IF EXISTS events_fts;")
            .unwrap();
        let err = validate_schema_v1(db.connection()).expect_err("must refuse");
        match err {
            ProjectionError::SchemaInconsistent { detail } => {
                assert!(detail.contains("events"), "{detail}");
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn rejects_missing_device_checkpoint_table() {
        let db = ProjectionDb::open_in_memory().expect("memory");
        db.connection()
            .execute("DROP TABLE device_checkpoint", [])
            .unwrap();
        let err = validate_schema_v1(db.connection()).expect_err("must refuse");
        match err {
            ProjectionError::SchemaInconsistent { detail } => {
                assert!(
                    detail.contains("device_checkpoint") && detail.contains("missing"),
                    "{detail}"
                );
            }
            other => panic!("unexpected error: {other}"),
        }
    }
}
