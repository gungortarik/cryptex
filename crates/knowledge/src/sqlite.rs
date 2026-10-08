//! Additive M4 tables in the existing M3 projection database (ADR-0017 §6).
//!
//! Canonical event packs remain authoritative. Migration and rebuild helpers
//! operate only on derived tables and leave M3 schema v1 intact.
use cryptex_projection::{ProjectionError, validate_schema_v1};
use rusqlite::{Connection, Error as SqliteError, TransactionBehavior};
use std::fmt;
pub const KNOWLEDGE_SCHEMA_VERSION: u32 = 1;
pub const KNOWLEDGE_REDUCER_VERSION: u32 = 1;
pub const KNOWLEDGE_BUILD_ID_V1: &str = "cryptex-knowledge-schema-v1";
#[derive(Debug)]
pub enum KnowledgeStoreError {
    Projection(ProjectionError),
    Sqlite(SqliteError),
    Schema(String),
}
impl fmt::Display for KnowledgeStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Projection(error) => write!(f, "M3 projection is not usable: {error}"),
            Self::Sqlite(error) => write!(f, "M4 knowledge SQLite failure: {error}"),
            Self::Schema(detail) => write!(f, "M4 knowledge schema is inconsistent: {detail}"),
        }
    }
}
impl std::error::Error for KnowledgeStoreError {}
impl From<SqliteError> for KnowledgeStoreError {
    fn from(value: SqliteError) -> Self {
        Self::Sqlite(value)
    }
}
impl From<ProjectionError> for KnowledgeStoreError {
    fn from(value: ProjectionError) -> Self {
        Self::Projection(value)
    }
}
pub type KnowledgeStoreResult<T> = Result<T, KnowledgeStoreError>;
/// Transactionally add/validate M4 schema v1 without changing M3 objects/data.////// The caller must first open the database through `ProjectionDb`; this/// function independently validates its M3 schema and version before taking an/// IMMEDIATE transaction. Existing M4 tables are validated, never dropped.
pub fn migrate_knowledge_schema(conn: &mut Connection) -> KnowledgeStoreResult<()> {
    validate_schema_v1(conn)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    apply_knowledge_schema_v1(&tx)?;
    validate_knowledge_schema_v1(&tx)?;
    tx.commit()?;
    Ok(())
}
pub fn validate_knowledge_schema_v1(conn: &Connection) -> KnowledgeStoreResult<()> {
    let version = conn
        .query_row(
            "SELECT schema_version FROM knowledge_meta WHERE id = 1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| match error {
            SqliteError::QueryReturnedNoRows => {
                KnowledgeStoreError::Schema("knowledge_meta singleton is missing".into())
            }
            other => KnowledgeStoreError::Sqlite(other),
        })?;
    if version != i64::from(KNOWLEDGE_SCHEMA_VERSION) {
        return Err(KnowledgeStoreError::Schema(format!(
            "unsupported knowledge schema version {version}; expected {KNOWLEDGE_SCHEMA_VERSION}"
        )));
    }
    for (table, expected) in TABLE_COLUMNS {
        let mut statement = conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let actual: Vec<String> = statement
            .query_map([], |row| row.get(1))?
            .collect::<Result<_, _>>()?;
        if actual
            .iter()
            .map(String::as_str)
            .ne(expected.iter().copied())
        {
            return Err(KnowledgeStoreError::Schema(format!(
                "table {table} has an unexpected column layout"
            )));
        }
    }
    for (table, expected_pk) in TABLE_PRIMARY_KEYS {
        let mut statement = conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let mut actual_pk: Vec<(i64, String)> = statement
            .query_map([], |row| Ok((row.get(5)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, SqliteError>>()?
            .into_iter()
            .filter(|(order, _)| *order > 0)
            .collect();
        actual_pk.sort_by_key(|(order, _)| *order);
        if actual_pk
            .iter()
            .map(|(_, name)| name.as_str())
            .ne(expected_pk.iter().copied())
        {
            return Err(KnowledgeStoreError::Schema(format!(
                "table {table} has an unexpected primary key"
            )));
        }
    }
    validate_singleton_meta_constraint(conn)?;
    Ok(())
}

const TABLE_PRIMARY_KEYS: &[(&str, &[&str])] = &[
    ("knowledge_meta", &["id"]),
    ("knowledge_entities", &["entity_id"]),
    ("knowledge_aliases", &["alias_id"]),
    ("knowledge_assertions", &["assertion_key"]),
    (
        "knowledge_provenance",
        &["materialized_key", "source_event_id"],
    ),
    ("knowledge_conflicts", &["conflict_key"]),
    ("knowledge_corrections", &["correction_event_id"]),
    ("knowledge_materialization_state", &["device_id"]),
    (
        "knowledge_pending_dependencies",
        &[
            "source_device_id",
            "source_pack_index",
            "source_frame_offset",
            "source_event_id",
            "reason_code",
        ],
    ),
];

fn validate_singleton_meta_constraint(conn: &Connection) -> KnowledgeStoreResult<()> {
    conn.execute_batch("SAVEPOINT knowledge_meta_check_probe;")?;
    let probe = conn.execute(
        "INSERT INTO knowledge_meta
         (id, schema_version, reducer_version, cardinality_policy_version,
          normalization_version, build_id, rebuild_status)
         VALUES (2, 1, 1, 1, 1, 'probe', 'ready')",
        [],
    );
    let rejected_by_constraint = matches!(
        probe,
        Err(SqliteError::SqliteFailure(error, _))
            if error.code == rusqlite::ErrorCode::ConstraintViolation
    );
    conn.execute_batch(
        "ROLLBACK TO knowledge_meta_check_probe; RELEASE knowledge_meta_check_probe;",
    )?;
    if !rejected_by_constraint {
        return Err(KnowledgeStoreError::Schema(
            "knowledge_meta must enforce the singleton CHECK (id = 1)".into(),
        ));
    }
    Ok(())
}
const TABLE_COLUMNS: &[(&str, &[&str])] = &[
    (
        "knowledge_meta",
        &[
            "id",
            "schema_version",
            "reducer_version",
            "cardinality_policy_version",
            "normalization_version",
            "build_id",
            "rebuild_status",
        ],
    ),
    (
        "knowledge_entities",
        &[
            "entity_id",
            "entity_kind",
            "display_label",
            "lifecycle_state",
            "record_json",
        ],
    ),
    (
        "knowledge_aliases",
        &[
            "alias_id",
            "entity_id",
            "normalization_version",
            "normalized_key",
            "display_alias",
            "record_json",
        ],
    ),
    (
        "knowledge_assertions",
        &[
            "assertion_key",
            "assertion_kind",
            "subject_entity_id",
            "predicate",
            "value_json",
            "object_entity_id",
            "lifecycle_state",
            "classification",
            "record_json",
        ],
    ),
    (
        "knowledge_provenance",
        &[
            "materialized_key",
            "source_event_id",
            "source_device_id",
            "source_device_seq",
            "signing_key_id",
            "confidence",
            "authorization_receipt_id",
            "classification",
            "contribution_role",
        ],
    ),
    (
        "knowledge_conflicts",
        &[
            "conflict_key",
            "conflict_kind",
            "competing_assertion_ids_json",
            "reason_code",
            "record_json",
        ],
    ),
    (
        "knowledge_corrections",
        &[
            "correction_event_id",
            "target_event_id",
            "replacement_json",
            "resolution_status",
            "record_json",
        ],
    ),
    (
        "knowledge_materialization_state",
        &[
            "device_id",
            "integrity_pack_index",
            "integrity_frame_offset",
            "integrity_content_hash",
            "safe_pack_index",
            "safe_frame_offset",
            "safe_content_hash",
            "blocking_reason",
        ],
    ),
    (
        "knowledge_pending_dependencies",
        &[
            "source_device_id",
            "source_pack_index",
            "source_frame_offset",
            "source_event_id",
            "dependency_event_id",
            "reason_code",
        ],
    ),
];
fn apply_knowledge_schema_v1(conn: &Connection) -> KnowledgeStoreResult<()> {
    conn.execute_batch(        "CREATE TABLE IF NOT EXISTS knowledge_meta (            id INTEGER NOT NULL PRIMARY KEY CHECK (id = 1),            schema_version INTEGER NOT NULL,            reducer_version INTEGER NOT NULL,            cardinality_policy_version INTEGER NOT NULL,            normalization_version INTEGER NOT NULL,            build_id TEXT NOT NULL,            rebuild_status TEXT NOT NULL CHECK (rebuild_status IN ('ready', 'rebuilding'))        );        CREATE TABLE IF NOT EXISTS knowledge_entities (            entity_id TEXT NOT NULL PRIMARY KEY,            entity_kind TEXT NOT NULL,            display_label TEXT NOT NULL,            lifecycle_state TEXT NOT NULL,            record_json TEXT NOT NULL        );        CREATE TABLE IF NOT EXISTS knowledge_aliases (            alias_id TEXT NOT NULL PRIMARY KEY,            entity_id TEXT NOT NULL,            normalization_version INTEGER NOT NULL,            normalized_key TEXT NOT NULL,            display_alias TEXT NOT NULL,            record_json TEXT NOT NULL,            UNIQUE(entity_id, normalization_version, normalized_key)        );        CREATE TABLE IF NOT EXISTS knowledge_assertions (            assertion_key TEXT NOT NULL PRIMARY KEY,            assertion_kind TEXT NOT NULL,            subject_entity_id TEXT NOT NULL,            predicate TEXT,            value_json TEXT,            object_entity_id TEXT,            lifecycle_state TEXT NOT NULL,            classification TEXT NOT NULL,            record_json TEXT NOT NULL        );        CREATE TABLE IF NOT EXISTS knowledge_provenance (            materialized_key TEXT NOT NULL,            source_event_id TEXT NOT NULL,            source_device_id TEXT NOT NULL,            source_device_seq TEXT NOT NULL,            signing_key_id TEXT NOT NULL,            confidence TEXT,            authorization_receipt_id TEXT NOT NULL,            classification TEXT NOT NULL,            contribution_role TEXT NOT NULL,            PRIMARY KEY(materialized_key, source_event_id)        );        CREATE TABLE IF NOT EXISTS knowledge_conflicts (            conflict_key TEXT NOT NULL PRIMARY KEY,            conflict_kind TEXT NOT NULL,            competing_assertion_ids_json TEXT NOT NULL,            reason_code TEXT NOT NULL,            record_json TEXT NOT NULL        );        CREATE TABLE IF NOT EXISTS knowledge_corrections (            correction_event_id TEXT NOT NULL PRIMARY KEY,            target_event_id TEXT NOT NULL,            replacement_json TEXT,            resolution_status TEXT NOT NULL,            record_json TEXT NOT NULL        );        CREATE TABLE IF NOT EXISTS knowledge_materialization_state (            device_id TEXT NOT NULL PRIMARY KEY,            integrity_pack_index INTEGER,            integrity_frame_offset INTEGER,            integrity_content_hash TEXT,            safe_pack_index INTEGER,            safe_frame_offset INTEGER,            safe_content_hash TEXT,            blocking_reason TEXT        );        CREATE TABLE IF NOT EXISTS knowledge_pending_dependencies (            source_device_id TEXT NOT NULL,            source_pack_index INTEGER NOT NULL,            source_frame_offset INTEGER NOT NULL,            source_event_id TEXT NOT NULL,            dependency_event_id TEXT,            reason_code TEXT NOT NULL,            PRIMARY KEY(source_device_id, source_pack_index, source_frame_offset, source_event_id, reason_code)        );        CREATE INDEX IF NOT EXISTS knowledge_assertions_subject_idx            ON knowledge_assertions(subject_entity_id, predicate);        CREATE INDEX IF NOT EXISTS knowledge_provenance_source_idx            ON knowledge_provenance(source_event_id);        CREATE INDEX IF NOT EXISTS knowledge_pending_dependency_idx            ON knowledge_pending_dependencies(dependency_event_id);",    )?;
    conn.execute(        "INSERT OR IGNORE INTO knowledge_meta         (id, schema_version, reducer_version, cardinality_policy_version,          normalization_version, build_id, rebuild_status)         VALUES (1, ?1, ?2, ?3, ?4, ?5, 'ready')",        rusqlite::params![            KNOWLEDGE_SCHEMA_VERSION,            KNOWLEDGE_REDUCER_VERSION,            crate::CARDINALITY_POLICY_VERSION,            1u32,            KNOWLEDGE_BUILD_ID_V1        ],    )?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use cryptex_projection::{PROJECTION_SCHEMA_VERSION, ProjectionDb, project_memory_note};
    #[test]
    fn additive_migration_preserves_m3_schema_and_rows_and_is_idempotent() {
        let mut db = ProjectionDb::open_in_memory().unwrap();
        let event = cryptex_events::AcceptedEvent {
            schema_version: 1,
            event_id: "11111111-1111-4111-8111-111111111111".into(),
            brain_id: "22222222-2222-4222-8222-222222222222".into(),
            device_id: "migration-test".into(),
            device_seq: cryptex_events::DeviceSeq::from_u64(1).unwrap(),
            created_at: "2026-10-08T00:00:00Z".into(),
            event_type: cryptex_events::EventType::MemoryNote,
            classification: cryptex_events::Classification::Private,
            signing_key_id: cryptex_events::SigningKeyId::parse("00112233445566778899aabbccddeeff")
                .unwrap(),
            payload: cryptex_events::EventPayload::MemoryNote(
                cryptex_events::MemoryNotePayload::new("preserve m3").unwrap(),
            ),
            prev_content_hash: cryptex_events::ContentHashHex::zero(),
            content_hash: cryptex_events::ContentHashHex::parse(&"1".repeat(64)).unwrap(),
            signature: cryptex_events::SignatureHex::parse(&"0".repeat(128)).unwrap(),
        };
        project_memory_note(db.connection(), &event).unwrap();
        migrate_knowledge_schema(db.connection_mut()).unwrap();
        migrate_knowledge_schema(db.connection_mut()).unwrap();
        validate_knowledge_schema_v1(db.connection()).unwrap();
        assert_eq!(db.schema_version().unwrap(), PROJECTION_SCHEMA_VERSION);
        assert_eq!(
            db.connection()
                .query_row(
                    "SELECT text FROM events WHERE event_id = ?1",
                    [event.event_id],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            "preserve m3"
        );
        assert_eq!(
            db.connection()
                .query_row("SELECT COUNT(*) FROM knowledge_meta", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
    #[test]
    fn future_m4_schema_fails_closed_without_dropping_data() {
        let mut db = ProjectionDb::open_in_memory().unwrap();
        migrate_knowledge_schema(db.connection_mut()).unwrap();
        db.connection()
            .execute(
                "UPDATE knowledge_meta SET schema_version = 99 WHERE id = 1",
                [],
            )
            .unwrap();
        assert!(matches!(
            validate_knowledge_schema_v1(db.connection()),
            Err(KnowledgeStoreError::Schema(_))
        ));
        assert_eq!(
            db.connection()
                .query_row(
                    "SELECT build_id FROM knowledge_meta WHERE id = 1",
                    [],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            KNOWLEDGE_BUILD_ID_V1
        );
    }
    #[test]
    fn malformed_existing_table_rolls_back_migration_without_touching_m3() {
        let mut db = ProjectionDb::open_in_memory().unwrap();
        db.connection()
            .execute_batch(
                "CREATE TABLE knowledge_entities (entity_id TEXT PRIMARY KEY, extra TEXT);",
            )
            .unwrap();
        let result = migrate_knowledge_schema(db.connection_mut());
        assert!(matches!(result, Err(KnowledgeStoreError::Schema(_))));
        let exists: i64 = db
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='knowledge_meta'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            exists, 0,
            "failed M4 migration must roll back all added tables"
        );
        assert_eq!(db.schema_version().unwrap(), PROJECTION_SCHEMA_VERSION);
    }

    #[test]
    fn permissive_meta_singleton_constraint_fails_closed_and_rolls_back() {
        let mut db = ProjectionDb::open_in_memory().unwrap();
        db.connection()
            .execute_batch(
                "CREATE TABLE knowledge_meta (
                    id INTEGER NOT NULL PRIMARY KEY CHECK (id = 1 OR id = 2),
                    schema_version INTEGER NOT NULL,
                    reducer_version INTEGER NOT NULL,
                    cardinality_policy_version INTEGER NOT NULL,
                    normalization_version INTEGER NOT NULL,
                    build_id TEXT NOT NULL,
                    rebuild_status TEXT NOT NULL
                );",
            )
            .unwrap();
        let result = migrate_knowledge_schema(db.connection_mut());
        assert!(matches!(result, Err(KnowledgeStoreError::Schema(_))));
        let meta_count: i64 = db
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='knowledge_meta'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(meta_count, 1, "pre-existing incompatible data is retained");
        let created_tables: i64 = db
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='knowledge_entities'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            created_tables, 0,
            "the migration must roll back its new tables"
        );
        assert_eq!(db.schema_version().unwrap(), PROJECTION_SCHEMA_VERSION);
    }
}
