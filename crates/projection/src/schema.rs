//! Projection schema version 1 (M3 Slice 3).
//!
//! Columns are derived strictly from accepted M2 `AcceptedEvent` / `memory.note`
//! fields. This module creates empty disposable tables only; ingestion and
//! search belong to later slices.

#[cfg(test)]
use std::cell::RefCell;

use rusqlite::{Connection, ErrorCode};

use crate::fts::ensure_fts5_available;
use crate::{ProjectionError, ProjectionResult};

#[cfg(test)]
thread_local! {
    /// Injected in place of the real INSERT during meta id CHECK probes.
    static INJECT_META_ID_PROBE_ERROR: RefCell<Option<rusqlite::Error>> =
        const { RefCell::new(None) };
}

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
        &["primary key"],
        "projection_meta.id must be PRIMARY KEY",
    )?;
    require_projection_meta_id_singleton_constraint(conn)?;
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

/// Verify `projection_meta` enforces exactly `CHECK (id = 1)`.
///
/// Combines:
/// 1. Structural parse of CREATE sql (comments stripped) requiring the accepted
///    expression `id = 1` (not a permissive OR / extra disjunct).
/// 2. Semantic probes of several non-1 ids; only SQLite constraint rejection
///    counts as success for each probe. Busy/locked/I/O/readonly propagate as
///    non-replaceable [`ProjectionError::Sqlite`].
/// 3. Savepoint cleanup that never swallows `ROLLBACK TO` / `RELEASE` errors.
fn require_projection_meta_id_singleton_constraint(conn: &Connection) -> ProjectionResult<()> {
    require_projection_meta_check_expression_is_id_eq_1(conn)?;

    // Probe multiple non-1 values so CHECK (id = 1 OR id = 3) cannot pass.
    const PROBE_IDS: [i64; 5] = [0, 2, 3, -1, 100];
    for id in PROBE_IDS {
        probe_projection_meta_id_rejected(conn, id)?;
    }

    let id: i64 = conn.query_row("SELECT id FROM projection_meta WHERE id = 1", [], |row| {
        row.get(0)
    })?;
    if id != 1 {
        return Err(ProjectionError::SchemaInconsistent {
            detail: "projection_meta singleton id must remain 1 after CHECK probes".into(),
        });
    }
    let extras: i64 = conn.query_row(
        "SELECT COUNT(*) FROM projection_meta WHERE id != 1",
        [],
        |row| row.get(0),
    )?;
    if extras != 0 {
        return Err(ProjectionError::SchemaInconsistent {
            detail: "projection_meta must not retain rows with id != 1".into(),
        });
    }
    Ok(())
}

/// Accepted v1 expression after comment-strip + whitespace normalize: `id=1`.
fn require_projection_meta_check_expression_is_id_eq_1(conn: &Connection) -> ProjectionResult<()> {
    let sql: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='projection_meta'",
            [],
            |row| row.get(0),
        )
        .optional_sql()?;
    let Some(sql) = sql else {
        return Err(ProjectionError::SchemaInconsistent {
            detail: "projection_meta has no CREATE sql in sqlite_master".into(),
        });
    };
    let stripped = strip_sql_comments(&sql);
    let checks = extract_check_expressions(&stripped);
    if checks.is_empty() {
        return Err(ProjectionError::SchemaInconsistent {
            detail: "projection_meta must declare CHECK (id = 1)".into(),
        });
    }
    let normalized: Vec<String> = checks.iter().map(|c| normalize_check_expr(c)).collect();
    if !normalized.iter().any(|c| c == "id=1") {
        return Err(ProjectionError::SchemaInconsistent {
            detail: format!("projection_meta CHECK must be exactly (id = 1); found {normalized:?}"),
        });
    }
    // Reject additional permissive CHECKs or disjunctive companions.
    if normalized.iter().any(|c| c != "id=1") {
        return Err(ProjectionError::SchemaInconsistent {
            detail: format!(
                "projection_meta must not declare CHECK expressions other than (id = 1); found {normalized:?}"
            ),
        });
    }
    Ok(())
}

fn strip_sql_comments(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len());
    let bytes = sql.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'-' && bytes[i + 1] == b'-' {
            i += 2;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn extract_check_expressions(sql: &str) -> Vec<String> {
    let lower = sql.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut search_from = 0usize;
    while let Some(rel) = lower[search_from..].find("check") {
        let start = search_from + rel;
        let after = start + "check".len();
        let rest = sql[after..].trim_start();
        if !rest.starts_with('(') {
            search_from = after;
            continue;
        }
        let chars: Vec<char> = rest.chars().collect();
        let mut depth = 0i32;
        let mut end = None;
        for (idx, ch) in chars.iter().enumerate() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(idx);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(end) = end else {
            break;
        };
        // Exclude surrounding parentheses.
        let inner: String = chars[1..end].iter().collect();
        out.push(inner);
        search_from = after + rest[..end + 1].len();
    }
    out
}

fn normalize_check_expr(expr: &str) -> String {
    expr.chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase()
}

fn is_constraint_rejection(err: &rusqlite::Error) -> bool {
    match err {
        rusqlite::Error::SqliteFailure(efi, _) => efi.code == ErrorCode::ConstraintViolation,
        _ => false,
    }
}

fn compound_probe_cleanup_error(
    probe: &dyn std::fmt::Display,
    cleanup: rusqlite::Error,
) -> ProjectionError {
    // Non-replaceable: must not become SchemaInconsistent/CorruptDb.
    ProjectionError::Sqlite(rusqlite::Error::ToSqlConversionFailure(Box::new(
        std::io::Error::other(format!(
            "projection_meta id probe: {probe}; savepoint cleanup failed: {cleanup}"
        )),
    )))
}

fn release_meta_id_savepoint(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch("ROLLBACK TO cryptex_meta_id_check; RELEASE cryptex_meta_id_check;")
}

fn probe_projection_meta_id_rejected(conn: &Connection, id: i64) -> ProjectionResult<()> {
    conn.execute_batch("SAVEPOINT cryptex_meta_id_check")?;

    #[cfg(test)]
    let insert = if let Some(err) = INJECT_META_ID_PROBE_ERROR.with(|slot| slot.borrow_mut().take())
    {
        Err(err)
    } else {
        conn.execute(
            "INSERT INTO projection_meta (id, schema_version, built_at, build_id)
             VALUES (?1, 1, 'probe', 'probe')",
            [id],
        )
        .map(|_| ())
    };

    #[cfg(not(test))]
    let insert = conn
        .execute(
            "INSERT INTO projection_meta (id, schema_version, built_at, build_id)
             VALUES (?1, 1, 'probe', 'probe')",
            [id],
        )
        .map(|_| ());

    let cleanup = release_meta_id_savepoint(conn);

    match (insert, cleanup) {
        (Ok(()), Ok(())) => Err(ProjectionError::SchemaInconsistent {
            detail: format!(
                "projection_meta must reject id != 1 (accepted probe id={id}); required CHECK (id = 1)"
            ),
        }),
        (Ok(()), Err(cleanup_err)) => Err(compound_probe_cleanup_error(
            &format!("INSERT id={id} was accepted (constraint missing/permissive)"),
            cleanup_err,
        )),
        (Err(probe_err), Ok(())) => {
            if is_constraint_rejection(&probe_err) {
                Ok(())
            } else {
                Err(ProjectionError::Sqlite(probe_err))
            }
        }
        (Err(probe_err), Err(cleanup_err)) => {
            Err(compound_probe_cleanup_error(&probe_err, cleanup_err))
        }
    }
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
                        || detail.contains("projection_meta")
                        || detail.contains("id != 1"),
                    "{detail}"
                );
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn rejects_projection_meta_permissive_id_check_despite_misleading_comment() {
        let db = ProjectionDb::open_in_memory().expect("memory");
        // Substring `id = 1` appears in both the permissive OR-clause and a comment.
        // Semantic probe must still fail closed.
        db.connection()
            .execute_batch(
                "
                DROP TABLE projection_meta;
                CREATE TABLE projection_meta (
                    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1 OR 1=1 /* intended: id = 1 */),
                    schema_version INTEGER NOT NULL,
                    built_at TEXT NOT NULL,
                    build_id TEXT NOT NULL
                );
                INSERT INTO projection_meta (id, schema_version, built_at, build_id)
                VALUES (1, 1, '2026-10-08T00:00:00Z', 'cryptex-projection-schema-v1');
                ",
            )
            .unwrap();
        let err = validate_schema_v1(db.connection()).expect_err("must refuse permissive CHECK");
        match err {
            ProjectionError::SchemaInconsistent { detail } => {
                assert!(
                    detail.contains("id != 1")
                        || detail.to_ascii_lowercase().contains("check")
                        || detail.contains("id=1"),
                    "{detail}"
                );
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn rejects_projection_meta_check_allowing_id_1_or_3() {
        let db = ProjectionDb::open_in_memory().expect("memory");
        db.connection()
            .execute_batch(
                "
                DROP TABLE projection_meta;
                CREATE TABLE projection_meta (
                    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1 OR id = 3),
                    schema_version INTEGER NOT NULL,
                    built_at TEXT NOT NULL,
                    build_id TEXT NOT NULL
                );
                INSERT INTO projection_meta (id, schema_version, built_at, build_id)
                VALUES (1, 1, '2026-10-08T00:00:00Z', 'cryptex-projection-schema-v1');
                ",
            )
            .unwrap();
        let err = validate_schema_v1(db.connection()).expect_err("must refuse OR id=3");
        match err {
            ProjectionError::SchemaInconsistent { detail } => {
                assert!(
                    detail.to_ascii_lowercase().contains("check")
                        || detail.contains("id=1")
                        || detail.contains("id != 1")
                        || detail.contains("id=3"),
                    "{detail}"
                );
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn meta_id_probe_non_constraint_error_is_non_replaceable_and_leaves_autocommit() {
        let db = ProjectionDb::open_in_memory().expect("memory");
        assert!(db.connection().is_autocommit());

        let busy = rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ErrorCode::DatabaseBusy,
                extended_code: 5,
            },
            Some("database is locked".into()),
        );
        INJECT_META_ID_PROBE_ERROR.with(|slot| {
            *slot.borrow_mut() = Some(busy);
        });

        let err = validate_schema_v1(db.connection()).expect_err("busy must fail closed");
        assert!(
            matches!(err, ProjectionError::Sqlite(_)),
            "non-constraint probe errors must stay Sqlite (non-replaceable), got {err}"
        );
        assert!(
            db.connection().is_autocommit(),
            "savepoint cleanup must leave connection in autocommit"
        );
        // Connection remains usable for ordinary reads.
        let version: i64 = db
            .connection()
            .query_row(
                "SELECT schema_version FROM projection_meta WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, 1);
    }

    #[test]
    fn meta_id_probe_io_error_mentioning_constraint_is_not_accepted_as_rejection() {
        let db = ProjectionDb::open_in_memory().expect("memory");

        let misleading = rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ErrorCode::SystemIoFailure,
                extended_code: 10,
            },
            Some("disk I/O error while checking constraint".into()),
        );
        INJECT_META_ID_PROBE_ERROR.with(|slot| {
            *slot.borrow_mut() = Some(misleading);
        });

        let err = validate_schema_v1(db.connection())
            .expect_err("non-ConstraintViolation must fail closed");
        assert!(
            matches!(err, ProjectionError::Sqlite(_)),
            "message containing 'constraint' must not count as ConstraintViolation, got {err}"
        );
        assert!(
            db.connection().is_autocommit(),
            "savepoint cleanup must leave connection in autocommit"
        );
        let version: i64 = db
            .connection()
            .query_row(
                "SELECT schema_version FROM projection_meta WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, 1);
    }
}
