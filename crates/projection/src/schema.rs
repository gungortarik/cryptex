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

#[derive(Debug, Clone, Copy)]
struct ColumnSpec {
    name: &'static str,
    /// Declared affinity/type text from the v1 DDL (compared case-insensitively).
    declared_type: &'static str,
    not_null: bool,
    /// True when this column is part of the table's PRIMARY KEY (`pk > 0`).
    primary_key: bool,
}

const EVENTS_COLUMNS: &[ColumnSpec] = &[
    ColumnSpec {
        name: "event_id",
        declared_type: "TEXT",
        not_null: true,
        primary_key: true,
    },
    ColumnSpec {
        name: "brain_id",
        declared_type: "TEXT",
        not_null: true,
        primary_key: false,
    },
    ColumnSpec {
        name: "device_id",
        declared_type: "TEXT",
        not_null: true,
        primary_key: false,
    },
    ColumnSpec {
        name: "device_seq",
        declared_type: "TEXT",
        not_null: true,
        primary_key: false,
    },
    ColumnSpec {
        name: "created_at",
        declared_type: "TEXT",
        not_null: true,
        primary_key: false,
    },
    ColumnSpec {
        name: "event_type",
        declared_type: "TEXT",
        not_null: true,
        primary_key: false,
    },
    ColumnSpec {
        name: "classification",
        declared_type: "TEXT",
        not_null: true,
        primary_key: false,
    },
    ColumnSpec {
        name: "content_hash",
        declared_type: "TEXT",
        not_null: true,
        primary_key: false,
    },
    ColumnSpec {
        name: "signing_key_id",
        declared_type: "TEXT",
        not_null: true,
        primary_key: false,
    },
    ColumnSpec {
        name: "text",
        declared_type: "TEXT",
        not_null: true,
        primary_key: false,
    },
];

const PROJECTION_META_COLUMNS: &[ColumnSpec] = &[
    ColumnSpec {
        name: "id",
        declared_type: "INTEGER",
        not_null: true,
        primary_key: true,
    },
    ColumnSpec {
        name: "schema_version",
        declared_type: "INTEGER",
        not_null: true,
        primary_key: false,
    },
    ColumnSpec {
        name: "built_at",
        declared_type: "TEXT",
        not_null: true,
        primary_key: false,
    },
    ColumnSpec {
        name: "build_id",
        declared_type: "TEXT",
        not_null: true,
        primary_key: false,
    },
];

const DEVICE_CHECKPOINT_COLUMNS: &[ColumnSpec] = &[
    ColumnSpec {
        name: "device_id",
        declared_type: "TEXT",
        not_null: true,
        primary_key: true,
    },
    ColumnSpec {
        name: "last_device_seq",
        declared_type: "TEXT",
        not_null: false,
        primary_key: false,
    },
    ColumnSpec {
        name: "last_content_hash",
        declared_type: "TEXT",
        not_null: false,
        primary_key: false,
    },
    ColumnSpec {
        name: "last_sealed_index",
        declared_type: "INTEGER",
        not_null: false,
        primary_key: false,
    },
    ColumnSpec {
        name: "open_verified_prefix_len",
        declared_type: "INTEGER",
        not_null: false,
        primary_key: false,
    },
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
    require_table_ddl_constraints(
        conn,
        "events",
        &["primary key", "event_id"],
        "events.event_id must be PRIMARY KEY",
    )?;
    require_table_ddl_constraints(
        conn,
        "device_checkpoint",
        &["primary key", "device_id"],
        "device_checkpoint.device_id must be PRIMARY KEY",
    )?;
    require_table_ddl_constraints(
        conn,
        "projection_meta",
        &["primary key", "check (id = 1)"],
        "projection_meta must keep PRIMARY KEY id with CHECK (id = 1)",
    )?;
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
    expected: &[ColumnSpec],
) -> ProjectionResult<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let found: Vec<(String, String, i64, i64)> = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?, // name
                row.get::<_, String>(2)?, // type
                row.get::<_, i64>(3)?,    // notnull
                row.get::<_, i64>(5)?,    // pk
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    for spec in expected {
        let Some((_, declared_type, notnull, pk)) =
            found.iter().find(|(n, _, _, _)| n == spec.name)
        else {
            return Err(ProjectionError::SchemaInconsistent {
                detail: format!("{table} table missing required column '{}'", spec.name),
            });
        };
        if !declared_type.eq_ignore_ascii_case(spec.declared_type) {
            return Err(ProjectionError::SchemaInconsistent {
                detail: format!(
                    "{table}.{} declared type mismatch: found '{declared_type}', expected '{}'",
                    spec.name, spec.declared_type
                ),
            });
        }
        let is_not_null = *notnull != 0;
        if is_not_null != spec.not_null {
            return Err(ProjectionError::SchemaInconsistent {
                detail: format!(
                    "{table}.{} NOT NULL mismatch: found {is_not_null}, expected {}",
                    spec.name, spec.not_null
                ),
            });
        }
        let is_pk = *pk > 0;
        if is_pk != spec.primary_key {
            return Err(ProjectionError::SchemaInconsistent {
                detail: format!(
                    "{table}.{} PRIMARY KEY mismatch: found {is_pk}, expected {}",
                    spec.name, spec.primary_key
                ),
            });
        }
    }
    for (name, _, _, _) in &found {
        if !expected.iter().any(|e| e.name == name) {
            return Err(ProjectionError::SchemaInconsistent {
                detail: format!("{table} table has unexpected column '{name}'"),
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

fn require_table_ddl_constraints(
    conn: &Connection,
    table: &str,
    needles: &[&str],
    detail: &str,
) -> ProjectionResult<()> {
    let sql: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name=?1",
            [table],
            |row| row.get(0),
        )
        .optional_sql()?;
    let Some(sql) = sql else {
        return Err(ProjectionError::SchemaInconsistent {
            detail: format!("{table} has no CREATE sql in sqlite_master"),
        });
    };
    let lowered = sql.to_ascii_lowercase();
    for needle in needles {
        if !lowered.contains(needle) {
            return Err(ProjectionError::SchemaInconsistent {
                detail: detail.into(),
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

    #[test]
    fn rejects_events_event_id_wrong_declared_type() {
        let db = ProjectionDb::open_in_memory().expect("memory");
        db.connection()
            .execute_batch(
                "
                DROP TABLE events;
                DROP TABLE IF EXISTS events_fts;
                CREATE TABLE events (
                    event_id INTEGER PRIMARY KEY NOT NULL,
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
                ",
            )
            .unwrap();
        let err = validate_schema_v1(db.connection()).expect_err("must refuse");
        match err {
            ProjectionError::SchemaInconsistent { detail } => {
                assert!(
                    detail.contains("event_id") && detail.to_ascii_lowercase().contains("type"),
                    "{detail}"
                );
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn rejects_events_without_primary_key_on_event_id() {
        let db = ProjectionDb::open_in_memory().expect("memory");
        db.connection()
            .execute_batch(
                "
                DROP TABLE events;
                DROP TABLE IF EXISTS events_fts;
                CREATE TABLE events (
                    event_id TEXT NOT NULL,
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
                ",
            )
            .unwrap();
        let err = validate_schema_v1(db.connection()).expect_err("must refuse");
        match err {
            ProjectionError::SchemaInconsistent { detail } => {
                assert!(
                    detail.contains("PRIMARY KEY") || detail.contains("event_id"),
                    "{detail}"
                );
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn rejects_projection_meta_missing_id_check_constraint() {
        let db = ProjectionDb::open_in_memory().expect("memory");
        db.connection()
            .execute_batch(
                "
                DROP TABLE projection_meta;
                CREATE TABLE projection_meta (
                    id INTEGER PRIMARY KEY NOT NULL,
                    schema_version INTEGER NOT NULL,
                    built_at TEXT NOT NULL,
                    build_id TEXT NOT NULL
                );
                INSERT INTO projection_meta (id, schema_version, built_at, build_id)
                VALUES (1, 1, '2026-10-08T00:00:00Z', 'cryptex-projection-schema-v1');
                ",
            )
            .unwrap();
        let err = validate_schema_v1(db.connection()).expect_err("must refuse");
        match err {
            ProjectionError::SchemaInconsistent { detail } => {
                assert!(
                    detail.to_ascii_lowercase().contains("check")
                        || detail.contains("projection_meta"),
                    "{detail}"
                );
            }
            other => panic!("unexpected error: {other}"),
        }
    }
}
