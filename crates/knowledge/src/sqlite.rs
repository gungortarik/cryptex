//! Additive M4 tables in the existing M3 projection database (ADR-0017 §6).
//!
//! Canonical event packs remain authoritative. Migration and rebuild helpers
//! operate only on derived tables and leave M3 schema v1 intact.
use crate::{
    Alias, CARDINALITY_POLICY_VERSION, Conflict, CorrectionRecord, EntityMergeRecord, Fact,
    KnowledgeState, PendingDependency, Provenance, Relation,
};
use cryptex_events::VerifiedV2History;
use cryptex_projection::{ProjectionError, validate_schema_v1};
use rusqlite::{Connection, Error as SqliteError, TransactionBehavior};
use serde_json::{Value, json};
use std::fmt;
pub const KNOWLEDGE_SCHEMA_VERSION: u32 = 1;
pub const KNOWLEDGE_REDUCER_VERSION: u32 = 1;
pub const KNOWLEDGE_BUILD_ID_V1: &str = "cryptex-knowledge-schema-v1";
#[derive(Debug)]
pub enum KnowledgeStoreError {
    Projection(ProjectionError),
    Sqlite(SqliteError),
    Schema(String),
    Reducer(crate::ReducerError),
}
impl fmt::Display for KnowledgeStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Projection(error) => write!(f, "M3 projection is not usable: {error}"),
            Self::Sqlite(error) => write!(f, "M4 knowledge SQLite failure: {error}"),
            Self::Schema(detail) => write!(f, "M4 knowledge schema is inconsistent: {detail}"),
            Self::Reducer(error) => write!(f, "verified history cannot be materialized: {error:?}"),
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CanonicalFramePosition {
    pub event_id: String,
    pub device_id: String,
    pub pack_index: u64,
    pub frame_offset: u64,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceMaterializationProgress {
    pub device_id: String,
    pub integrity_position: Option<CanonicalFramePosition>,
    pub safe_position: Option<CanonicalFramePosition>,
    pub blocking_reason: Option<String>,
}

/// Replace only M4-derived rows with the result of a deterministic rebuild.
/// Positions must come from the trusted canonical scanner; this API never
/// derives frame offsets from device sequence numbers.
/// Rebuild M4 state only from a complete history accepted by the trusted-key
/// versioned verifier. Derived rows cannot be supplied by callers.
pub fn rebuild_from_verified_history(
    conn: &mut Connection,
    expected_brain_id: &str,
    history: &VerifiedV2History,
    frame_positions: &[CanonicalFramePosition],
    progress: &[DeviceMaterializationProgress],
) -> KnowledgeStoreResult<()> {
    validate_history_positions(history, frame_positions)?;
    let state = crate::reduce_verified_events(expected_brain_id, history)
        .map_err(KnowledgeStoreError::Reducer)?;
    replace_reduced_knowledge_materialization(conn, &state, frame_positions, progress)
}

fn validate_history_positions(
    history: &VerifiedV2History,
    positions: &[CanonicalFramePosition],
) -> KnowledgeStoreResult<()> {
    use cryptex_events::VersionedRecord;

    if history.records().len() != positions.len() {
        return Err(KnowledgeStoreError::Schema(
            "canonical frame positions do not cover the verified history".into(),
        ));
    }
    let mut by_event = std::collections::BTreeMap::new();
    for position in positions {
        let event_id = position.event_id.to_ascii_lowercase();
        if by_event.insert(event_id, position).is_some() {
            return Err(KnowledgeStoreError::Schema(
                "duplicate event frame position".into(),
            ));
        }
    }
    for record in history.records() {
        let (event_id, device_id, content_hash) = match record {
            VersionedRecord::VerifiedV1(event) => (
                event.event_id.as_str(),
                event.device_id.as_str(),
                event.content_hash.as_str(),
            ),
            VersionedRecord::VerifiedV2Supported(event) => (
                event.event().event_id.as_str(),
                event.event().device_id.as_str(),
                event.event().content_hash.as_str(),
            ),
            VersionedRecord::VerifiedUnsupported(event) => (
                event.event_id.as_str(),
                event.device_id.as_str(),
                event.content_hash.as_str(),
            ),
            VersionedRecord::Unverifiable(_) => {
                return Err(KnowledgeStoreError::Schema(
                    "verified history contains an unverifiable envelope".into(),
                ));
            }
        };
        let position = by_event
            .get(&event_id.to_ascii_lowercase())
            .ok_or_else(|| {
                KnowledgeStoreError::Schema(
                    "verified event is missing its canonical frame position".into(),
                )
            })?;
        if position.device_id != device_id || position.content_hash != content_hash {
            return Err(KnowledgeStoreError::Schema(
                "canonical frame position does not match verified event identity".into(),
            ));
        }
    }
    Ok(())
}

fn replace_reduced_knowledge_materialization(
    conn: &mut Connection,
    state: &KnowledgeState,
    frame_positions: &[CanonicalFramePosition],
    progress: &[DeviceMaterializationProgress],
) -> KnowledgeStoreResult<()> {
    validate_knowledge_schema_v1(conn)?;
    validate_rebuild_inputs(state, frame_positions, progress)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute_batch(
        "DELETE FROM knowledge_pending_dependencies;
         DELETE FROM knowledge_materialization_state;
         DELETE FROM knowledge_corrections;
         DELETE FROM knowledge_conflicts;
         DELETE FROM knowledge_provenance;
         DELETE FROM knowledge_assertions;
         DELETE FROM knowledge_aliases;
         DELETE FROM knowledge_entities;",
    )?;

    for entity in &state.entities {
        let record = json!({
            "entity_id": entity.entity_id,
            "representative_id": entity.representative_id,
            "kind": entity_kind_name(entity.kind),
            "label": entity.label,
        });
        tx.execute(
            "INSERT INTO knowledge_entities
             (entity_id, entity_kind, display_label, lifecycle_state, record_json)
             VALUES (?1, ?2, ?3, 'active', ?4)",
            rusqlite::params![
                entity.entity_id,
                entity_kind_name(entity.kind),
                entity.label,
                json_text(&record)?
            ],
        )?;
        insert_provenance(
            &tx,
            &entity.entity_id,
            &entity.provenance,
            "entity_declaration",
        )?;
    }
    for alias in &state.aliases {
        insert_alias(&tx, alias)?;
    }
    for fact in &state.facts {
        insert_fact(&tx, fact)?;
    }
    for relation in &state.relations {
        insert_relation(&tx, relation)?;
    }
    for conflict in &state.conflicts {
        insert_conflict(&tx, conflict)?;
    }
    for correction in &state.corrections {
        insert_correction(&tx, correction)?;
    }
    for merge in &state.entity_merges {
        insert_merge(&tx, merge)?;
    }
    for pending in &state.pending {
        insert_pending(&tx, pending, frame_positions)?;
    }
    for item in progress {
        insert_progress(&tx, item)?;
    }

    tx.execute(
        "UPDATE knowledge_meta
         SET reducer_version = ?1, cardinality_policy_version = ?2,
             normalization_version = 1, build_id = ?3, rebuild_status = 'ready'
         WHERE id = 1",
        rusqlite::params![
            KNOWLEDGE_REDUCER_VERSION,
            CARDINALITY_POLICY_VERSION,
            KNOWLEDGE_BUILD_ID_V1
        ],
    )?;
    tx.commit()?;
    Ok(())
}

fn validate_rebuild_inputs(
    state: &KnowledgeState,
    positions: &[CanonicalFramePosition],
    progress: &[DeviceMaterializationProgress],
) -> KnowledgeStoreResult<()> {
    let mut by_event = std::collections::BTreeMap::new();
    for position in positions {
        cryptex_core::ids::DeviceId::new(position.device_id.clone())
            .map_err(|_| KnowledgeStoreError::Schema("invalid frame-position device id".into()))?;
        cryptex_core::validate::parse_uuid_str(&position.event_id)
            .map_err(|_| KnowledgeStoreError::Schema("invalid frame-position event id".into()))?;
        cryptex_events::ContentHashHex::parse(&position.content_hash).map_err(|_| {
            KnowledgeStoreError::Schema("invalid frame-position content hash".into())
        })?;
        if by_event
            .insert(position.event_id.as_str(), position)
            .is_some()
        {
            return Err(KnowledgeStoreError::Schema(
                "duplicate event frame position".into(),
            ));
        }
    }
    for pending in &state.pending {
        let position = by_event.get(pending.event_id.as_str()).ok_or_else(|| {
            KnowledgeStoreError::Schema(format!(
                "pending event {} lacks canonical frame position",
                pending.event_id
            ))
        })?;
        if position.device_id.is_empty() {
            return Err(KnowledgeStoreError::Schema(
                "pending source device is empty".into(),
            ));
        }
        if state.materialized_event_ids.contains(&pending.event_id) {
            return Err(KnowledgeStoreError::Schema(
                "pending event is also reported as materialized".into(),
            ));
        }
        let watermark = progress
            .iter()
            .find(|item| item.device_id == position.device_id)
            .and_then(|item| item.safe_position.as_ref());
        if watermark.is_some_and(|safe| {
            (safe.pack_index, safe.frame_offset) >= (position.pack_index, position.frame_offset)
        }) {
            return Err(KnowledgeStoreError::Schema(
                "safe watermark crosses an unresolved dependency".into(),
            ));
        }
    }
    let mut devices = std::collections::BTreeSet::new();
    for item in progress {
        cryptex_core::ids::DeviceId::new(item.device_id.clone())
            .map_err(|_| KnowledgeStoreError::Schema("invalid progress device id".into()))?;
        if !devices.insert(item.device_id.as_str()) {
            return Err(KnowledgeStoreError::Schema(
                "duplicate device progress row".into(),
            ));
        }
        for position in [
            item.integrity_position.as_ref(),
            item.safe_position.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            if position.device_id != item.device_id {
                return Err(KnowledgeStoreError::Schema(
                    "watermark device does not match row device".into(),
                ));
            }
            if by_event.get(position.event_id.as_str()).copied() != Some(position) {
                return Err(KnowledgeStoreError::Schema(
                    "watermark does not match canonical frame position".into(),
                ));
            }
        }
        if let (Some(safe), Some(integrity)) = (&item.safe_position, &item.integrity_position) {
            if (safe.pack_index, safe.frame_offset) > (integrity.pack_index, integrity.frame_offset)
            {
                return Err(KnowledgeStoreError::Schema(
                    "safe watermark exceeds integrity watermark".into(),
                ));
            }
        } else if item.safe_position.is_some() && item.integrity_position.is_none() {
            return Err(KnowledgeStoreError::Schema(
                "safe watermark cannot exist without integrity watermark".into(),
            ));
        }
    }
    Ok(())
}

fn insert_alias(conn: &Connection, alias: &Alias) -> KnowledgeStoreResult<()> {
    let record = json!({"entity_id": alias.entity_id, "spelling": alias.spelling, "normalized_key": alias.normalized_key});
    conn.execute(
        "INSERT INTO knowledge_aliases
         (alias_id, entity_id, normalization_version, normalized_key, display_alias, record_json)
         VALUES (?1, ?2, 1, ?3, ?4, ?5)",
        rusqlite::params![
            alias.semantic_id,
            alias.entity_id,
            alias.normalized_key,
            alias.spelling,
            json_text(&record)?
        ],
    )?;
    insert_provenance(conn, &alias.semantic_id, &alias.provenance, "alias")
}

fn insert_fact(conn: &Connection, fact: &Fact) -> KnowledgeStoreResult<()> {
    let value = fact_value_json(&fact.value);
    let record = json!({"semantic_id": fact.semantic_id, "subject_entity_id": fact.subject_entity_id, "predicate": fact.predicate, "value": value});
    conn.execute(
        "INSERT INTO knowledge_assertions
         (assertion_key, assertion_kind, subject_entity_id, predicate, value_json, object_entity_id,
          lifecycle_state, classification, record_json)
         VALUES (?1, 'fact', ?2, ?3, ?4, NULL, 'active', ?5, ?6)",
        rusqlite::params![
            fact.semantic_id,
            fact.subject_entity_id,
            fact.predicate,
            json_text(&value)?,
            most_restrictive_classification(&fact.provenance),
            json_text(&record)?
        ],
    )?;
    insert_provenance(conn, &fact.semantic_id, &fact.provenance, "fact_assertion")
}

fn insert_relation(conn: &Connection, relation: &Relation) -> KnowledgeStoreResult<()> {
    let record = json!({"semantic_id": relation.semantic_id, "subject_entity_id": relation.subject_entity_id, "predicate": relation.predicate, "object_entity_id": relation.object_entity_id});
    conn.execute(
        "INSERT INTO knowledge_assertions
         (assertion_key, assertion_kind, subject_entity_id, predicate, value_json, object_entity_id,
          lifecycle_state, classification, record_json)
         VALUES (?1, 'relation', ?2, ?3, NULL, ?4, 'active', ?5, ?6)",
        rusqlite::params![
            relation.semantic_id,
            relation.subject_entity_id,
            relation.predicate,
            relation.object_entity_id,
            most_restrictive_classification(&relation.provenance),
            json_text(&record)?
        ],
    )?;
    insert_provenance(
        conn,
        &relation.semantic_id,
        &relation.provenance,
        "relation_assertion",
    )
}

fn insert_conflict(conn: &Connection, conflict: &Conflict) -> KnowledgeStoreResult<()> {
    let event_ids = json!(&conflict.event_ids);
    let record = json!({"kind": conflict.kind, "key": conflict.key, "event_ids": event_ids});
    let key = format!("{}:{}", conflict.kind, conflict.key);
    conn.execute(
        "INSERT INTO knowledge_conflicts
         (conflict_key, conflict_kind, competing_assertion_ids_json, reason_code, record_json)
         VALUES (?1, ?2, ?3, ?2, ?4)",
        rusqlite::params![
            key,
            conflict.kind,
            json_text(&event_ids)?,
            json_text(&record)?
        ],
    )?;
    Ok(())
}

fn insert_correction(conn: &Connection, correction: &CorrectionRecord) -> KnowledgeStoreResult<()> {
    let replacement = correction.replacement.as_ref().map(semantic_assertion_json);
    let record = json!({
        "target_event_id": correction.target_event_id,
        "replacement": replacement,
        "resolution_status": correction.resolution_status,
        "provenance": provenance_json(&correction.provenance),
    });
    conn.execute(
        "INSERT INTO knowledge_corrections
         (correction_event_id, target_event_id, replacement_json, resolution_status, record_json)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            correction.correction_event_id,
            correction.target_event_id,
            replacement.as_ref().map(json_text).transpose()?,
            correction.resolution_status,
            json_text(&record)?,
        ],
    )?;
    insert_provenance(
        conn,
        &correction.correction_event_id,
        std::slice::from_ref(&correction.provenance),
        "correction",
    )
}

fn semantic_assertion_json(assertion: &cryptex_events::SemanticAssertion) -> Value {
    use cryptex_events::SemanticAssertion;
    match assertion {
        SemanticAssertion::Fact {
            subject_entity_id,
            predicate,
            value,
            confidence,
        } => json!({
            "kind": "fact",
            "subject_entity_id": subject_entity_id,
            "predicate": predicate,
            "value": fact_value_json(value),
            "confidence": format!("{confidence:?}").to_ascii_lowercase(),
        }),
        SemanticAssertion::Relation {
            subject_entity_id,
            predicate,
            object_entity_id,
            confidence,
        } => json!({
            "kind": "relation",
            "subject_entity_id": subject_entity_id,
            "predicate": predicate,
            "object_entity_id": object_entity_id,
            "confidence": format!("{confidence:?}").to_ascii_lowercase(),
        }),
    }
}

fn insert_merge(conn: &Connection, merge: &EntityMergeRecord) -> KnowledgeStoreResult<()> {
    let status = format!("{:?}", merge.status).to_ascii_lowercase();
    let record = json!({"event_id": merge.event_id, "entity_ids": merge.entity_ids, "representative_id": merge.representative_id, "status": status});
    conn.execute(
        "INSERT INTO knowledge_assertions
         (assertion_key, assertion_kind, subject_entity_id, predicate, value_json, object_entity_id,
          lifecycle_state, classification, record_json)
         VALUES (?1, 'entity_merge', ?2, NULL, ?3, NULL, ?4, ?5, ?6)",
        rusqlite::params![
            merge.event_id,
            merge
                .entity_ids
                .first()
                .map(String::as_str)
                .unwrap_or("unresolved"),
            json_text(&record)?,
            status,
            most_restrictive_classification(&merge.provenance),
            json_text(&record)?
        ],
    )?;
    insert_provenance(conn, &merge.event_id, &merge.provenance, "entity_merge")
}

fn insert_pending(
    conn: &Connection,
    pending: &PendingDependency,
    positions: &[CanonicalFramePosition],
) -> KnowledgeStoreResult<()> {
    let position = positions
        .iter()
        .find(|position| position.event_id == pending.event_id)
        .ok_or_else(|| KnowledgeStoreError::Schema("pending event position disappeared".into()))?;
    let pack_index = checked_i64(position.pack_index, "pending pack index")?;
    let frame_offset = checked_i64(position.frame_offset, "pending frame offset")?;
    conn.execute(
        "INSERT INTO knowledge_pending_dependencies
         (source_device_id, source_pack_index, source_frame_offset, source_event_id, dependency_id, dependency_event_id, reason_code)
         VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6)",
        rusqlite::params![position.device_id, pack_index, frame_offset, pending.event_id, pending.target_id, pending.reason],
    )?;
    Ok(())
}

fn insert_progress(
    conn: &Connection,
    progress: &DeviceMaterializationProgress,
) -> KnowledgeStoreResult<()> {
    let integrity = progress.integrity_position.as_ref();
    let safe = progress.safe_position.as_ref();
    let integrity_pack = integrity
        .map(|p| checked_i64(p.pack_index, "integrity pack index"))
        .transpose()?;
    let integrity_offset = integrity
        .map(|p| checked_i64(p.frame_offset, "integrity frame offset"))
        .transpose()?;
    let safe_pack = safe
        .map(|p| checked_i64(p.pack_index, "safe pack index"))
        .transpose()?;
    let safe_offset = safe
        .map(|p| checked_i64(p.frame_offset, "safe frame offset"))
        .transpose()?;
    conn.execute(
        "INSERT INTO knowledge_materialization_state
         (device_id, integrity_pack_index, integrity_frame_offset, integrity_content_hash,
          safe_pack_index, safe_frame_offset, safe_content_hash, blocking_reason)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            progress.device_id,
            integrity_pack,
            integrity_offset,
            integrity.map(|p| p.content_hash.as_str()),
            safe_pack,
            safe_offset,
            safe.map(|p| p.content_hash.as_str()),
            progress.blocking_reason
        ],
    )?;
    Ok(())
}

fn insert_provenance(
    conn: &Connection,
    materialized_key: &str,
    rows: &[Provenance],
    role: &str,
) -> KnowledgeStoreResult<()> {
    for row in rows {
        conn.execute(
            "INSERT INTO knowledge_provenance
             (materialized_key, source_event_id, source_device_id, source_device_seq,
              signing_key_id, confidence, authorization_receipt_id, classification, contribution_role)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![materialized_key, row.event_id, row.device_id, row.device_seq.to_string(), row.signing_key_id, row.confidence.map(|value| format!("{value:?}").to_ascii_lowercase()), row.authorization_receipt_id, format!("{:?}", row.classification).to_ascii_lowercase(), role],
        )?;
    }
    Ok(())
}

fn json_text(value: &Value) -> KnowledgeStoreResult<String> {
    serde_json::to_string(value)
        .map_err(|_| KnowledgeStoreError::Schema("derived row serialization failed".into()))
}

fn checked_i64(value: u64, label: &str) -> KnowledgeStoreResult<i64> {
    i64::try_from(value)
        .map_err(|_| KnowledgeStoreError::Schema(format!("{label} exceeds SQLite integer range")))
}

fn entity_kind_name(kind: cryptex_events::EntityKind) -> &'static str {
    match kind {
        cryptex_events::EntityKind::Person => "person",
        cryptex_events::EntityKind::Project => "project",
        cryptex_events::EntityKind::Course => "course",
        cryptex_events::EntityKind::Device => "device",
        cryptex_events::EntityKind::Organization => "organization",
        cryptex_events::EntityKind::Concept => "concept",
        cryptex_events::EntityKind::Other => "other",
    }
}

fn fact_value_json(value: &cryptex_events::FactValue) -> Value {
    use cryptex_events::FactValue;
    match value {
        FactValue::Text(value) => json!({"kind": "text", "value": value}),
        FactValue::Decimal(value) => json!({"kind": "decimal", "value": value}),
        FactValue::Boolean(value) => json!({"kind": "boolean", "value": value}),
        FactValue::Date(value) => json!({"kind": "date", "value": value}),
        FactValue::EntityRef(value) => json!({"kind": "entity_ref", "value": value}),
    }
}

fn provenance_json(value: &Provenance) -> Value {
    json!({
        "event_id": value.event_id,
        "device_id": value.device_id,
        "device_seq": value.device_seq.to_string(),
        "signing_key_id": value.signing_key_id,
        "confidence": value.confidence.map(|v| format!("{v:?}").to_ascii_lowercase()),
        "authorization_receipt_id": value.authorization_receipt_id,
        "classification": format!("{:?}", value.classification).to_ascii_lowercase(),
    })
}

fn most_restrictive_classification(rows: &[Provenance]) -> &'static str {
    use cryptex_events::Classification;
    let rank = rows
        .iter()
        .map(|row| match row.classification {
            Classification::Public => 0,
            Classification::Internal => 1,
            Classification::Private => 2,
            Classification::Restricted => 3,
        })
        .max()
        .unwrap_or(3);
    match rank {
        0 => "public",
        1 => "internal",
        2 => "private",
        _ => "restricted",
    }
}

/// Transactionally add/validate M4 schema v1 without changing M3 objects/data.
///
/// The caller must first open the database through `ProjectionDb`; this
/// function independently validates its M3 schema and version before taking an
/// IMMEDIATE transaction. Existing M4 tables are validated, never dropped.
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
            "dependency_id",
            "dependency_event_id",
            "reason_code",
        ],
    ),
];
fn apply_knowledge_schema_v1(conn: &Connection) -> KnowledgeStoreResult<()> {
    conn.execute_batch(        "CREATE TABLE IF NOT EXISTS knowledge_meta (            id INTEGER NOT NULL PRIMARY KEY CHECK (id = 1),            schema_version INTEGER NOT NULL,            reducer_version INTEGER NOT NULL,            cardinality_policy_version INTEGER NOT NULL,            normalization_version INTEGER NOT NULL,            build_id TEXT NOT NULL,            rebuild_status TEXT NOT NULL CHECK (rebuild_status IN ('not_built', 'ready', 'rebuilding'))        );        CREATE TABLE IF NOT EXISTS knowledge_entities (            entity_id TEXT NOT NULL PRIMARY KEY,            entity_kind TEXT NOT NULL,            display_label TEXT NOT NULL,            lifecycle_state TEXT NOT NULL,            record_json TEXT NOT NULL        );        CREATE TABLE IF NOT EXISTS knowledge_aliases (            alias_id TEXT NOT NULL PRIMARY KEY,            entity_id TEXT NOT NULL,            normalization_version INTEGER NOT NULL,            normalized_key TEXT NOT NULL,            display_alias TEXT NOT NULL,            record_json TEXT NOT NULL,            UNIQUE(entity_id, normalization_version, normalized_key)        );        CREATE TABLE IF NOT EXISTS knowledge_assertions (            assertion_key TEXT NOT NULL PRIMARY KEY,            assertion_kind TEXT NOT NULL,            subject_entity_id TEXT NOT NULL,            predicate TEXT,            value_json TEXT,            object_entity_id TEXT,            lifecycle_state TEXT NOT NULL,            classification TEXT NOT NULL,            record_json TEXT NOT NULL        );        CREATE TABLE IF NOT EXISTS knowledge_provenance (            materialized_key TEXT NOT NULL,            source_event_id TEXT NOT NULL,            source_device_id TEXT NOT NULL,            source_device_seq TEXT NOT NULL,            signing_key_id TEXT NOT NULL,            confidence TEXT,            authorization_receipt_id TEXT NOT NULL,            classification TEXT NOT NULL,            contribution_role TEXT NOT NULL,            PRIMARY KEY(materialized_key, source_event_id)        );        CREATE TABLE IF NOT EXISTS knowledge_conflicts (            conflict_key TEXT NOT NULL PRIMARY KEY,            conflict_kind TEXT NOT NULL,            competing_assertion_ids_json TEXT NOT NULL,            reason_code TEXT NOT NULL,            record_json TEXT NOT NULL        );        CREATE TABLE IF NOT EXISTS knowledge_corrections (            correction_event_id TEXT NOT NULL PRIMARY KEY,            target_event_id TEXT NOT NULL,            replacement_json TEXT,            resolution_status TEXT NOT NULL,            record_json TEXT NOT NULL        );        CREATE TABLE IF NOT EXISTS knowledge_materialization_state (            device_id TEXT NOT NULL PRIMARY KEY,            integrity_pack_index INTEGER,            integrity_frame_offset INTEGER,            integrity_content_hash TEXT,            safe_pack_index INTEGER,            safe_frame_offset INTEGER,            safe_content_hash TEXT,            blocking_reason TEXT        );        CREATE TABLE IF NOT EXISTS knowledge_pending_dependencies (            source_device_id TEXT NOT NULL,            source_pack_index INTEGER NOT NULL,            source_frame_offset INTEGER NOT NULL,            source_event_id TEXT NOT NULL,            dependency_id TEXT NOT NULL,            dependency_event_id TEXT,            reason_code TEXT NOT NULL,            PRIMARY KEY(source_device_id, source_pack_index, source_frame_offset, source_event_id, reason_code)        );        CREATE INDEX IF NOT EXISTS knowledge_assertions_subject_idx            ON knowledge_assertions(subject_entity_id, predicate);        CREATE INDEX IF NOT EXISTS knowledge_provenance_source_idx            ON knowledge_provenance(source_event_id);        CREATE INDEX IF NOT EXISTS knowledge_pending_dependency_idx            ON knowledge_pending_dependencies(dependency_event_id);",    )?;
    conn.execute(        "INSERT OR IGNORE INTO knowledge_meta         (id, schema_version, reducer_version, cardinality_policy_version,          normalization_version, build_id, rebuild_status)         VALUES (1, ?1, ?2, ?3, ?4, ?5, 'not_built')",        rusqlite::params![            KNOWLEDGE_SCHEMA_VERSION,            KNOWLEDGE_REDUCER_VERSION,            crate::CARDINALITY_POLICY_VERSION,            1u32,            KNOWLEDGE_BUILD_ID_V1        ],    )?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use cryptex_projection::{PROJECTION_SCHEMA_VERSION, ProjectionDb, project_memory_note};
    use std::fs;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_ROOT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    struct TestRoot(std::path::PathBuf);

    impl TestRoot {
        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn exclusive_test_root(label: &str) -> TestRoot {
        let parent = std::env::temp_dir();
        for _ in 0..10_000 {
            let sequence = TEST_ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let root = parent.join(format!(
                "cryptex-knowledge-{label}-{}-{sequence}",
                std::process::id()
            ));
            match fs::create_dir(&root) {
                Ok(()) => return TestRoot(root),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create migration test root {}: {error}", root.display()),
            }
        }
        panic!("exhausted unique migration test roots for {label}");
    }

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
    fn interrupted_migration_transaction_rolls_back_on_process_reopen() {
        const DB_ENV: &str = "CRYPTEX_TEST_INTERRUPTED_M4_MIGRATION_DB";
        const MARKER_ENV: &str = "CRYPTEX_TEST_INTERRUPTED_M4_MIGRATION_MARKER";

        // The child reaches this same test, stages M4 DDL in an open SQLite
        // transaction, writes a synchronization marker, then exits without
        // running Rust/SQLite destructors. The parent reopens the DB as a fresh
        // process would after interruption and verifies SQLite recovery.
        if let Some(path) = std::env::var_os(DB_ENV) {
            let marker = std::env::var_os(MARKER_ENV).expect("child marker path");
            let mut conn = Connection::open(&path).expect("open child migration database");
            conn.execute_batch(
                "PRAGMA journal_mode=DELETE;
                 PRAGMA synchronous=FULL;
                 PRAGMA cache_size=1;",
            )
            .expect("configure rollback-journal crash fixture");
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .expect("begin child migration transaction");
            apply_knowledge_schema_v1(&tx).expect("stage M4 schema inside transaction");
            tx.execute_batch("CREATE TABLE interrupted_migration_probe (value BLOB NOT NULL);")
                .expect("stage interruption probe table");
            let page_pressure = vec![b'x'; 8 * 1024];
            for _ in 0..128 {
                tx.execute(
                    "INSERT INTO interrupted_migration_probe(value) VALUES (?1)",
                    [&page_pressure],
                )
                .expect("stage interruption probe rows");
            }
            let mut journal_path = path;
            journal_path.push("-journal");
            let journal_len = fs::metadata(std::path::PathBuf::from(journal_path))
                .expect("transaction must create rollback journal")
                .len();
            assert!(
                journal_len > 0,
                "rollback journal must contain original pages"
            );
            let mut marker_file = fs::File::create(marker).expect("create child marker");
            use std::io::Write;
            writeln!(marker_file, "{journal_len}").expect("write child journal length");
            marker_file.sync_all().expect("sync child readiness marker");
            std::process::exit(73);
        }

        let root = exclusive_test_root("interrupted-migration");
        let mut db = ProjectionDb::open_or_create(root.path()).expect("create M3 database");
        let db_path = db.path().to_path_buf();
        drop(db);
        let marker = root.path().join("child-staged-transaction.txt");
        let test_executable = std::env::current_exe().expect("current test executable");
        let child = Command::new(test_executable)
            .args([
                "--exact",
                "sqlite::tests::interrupted_migration_transaction_rolls_back_on_process_reopen",
                "--nocapture",
            ])
            .env_clear()
            .env(DB_ENV, &db_path)
            .env(MARKER_ENV, &marker)
            .output()
            .expect("spawn interrupted migration process");

        assert_eq!(
            child.status.code(),
            Some(73),
            "child did not stop at probe: {}",
            String::from_utf8_lossy(&child.stderr)
        );
        let journal_len = fs::read_to_string(&marker)
            .expect("child reached staged rollback-journal transaction")
            .trim()
            .parse::<u64>()
            .expect("child recorded rollback-journal length");
        assert!(
            journal_len > 0,
            "child must observe a populated rollback journal"
        );

        db = ProjectionDb::open_or_create(root.path()).expect("reopen after interrupted migration");
        let mut journal_path = db_path.as_os_str().to_owned();
        journal_path.push("-journal");
        let journal_path = std::path::PathBuf::from(journal_path);
        match fs::metadata(&journal_path) {
            Ok(metadata) => assert_eq!(
                metadata.len(),
                0,
                "SQLite must consume the interrupted transaction's rollback journal"
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!("inspect recovered rollback journal: {error}"),
        }
        assert_eq!(
            db.schema_version().expect("M3 remains valid"),
            PROJECTION_SCHEMA_VERSION
        );
        let uncommitted_tables: i64 = db
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('knowledge_meta', 'interrupted_migration_probe')",
                [],
                |row| row.get(0),
            )
            .expect("inspect recovered schema");
        assert_eq!(
            uncommitted_tables, 0,
            "uncommitted M4 DDL must be rolled back"
        );

        migrate_knowledge_schema(db.connection_mut()).expect("migration retries cleanly");
        validate_knowledge_schema_v1(db.connection()).expect("M4 schema validates after retry");
        drop(db);
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

    #[test]
    fn m4_rebuild_is_idempotent_marks_ready_and_preserves_m3_rows() {
        let mut db = ProjectionDb::open_in_memory().unwrap();
        let event = cryptex_events::AcceptedEvent {
            schema_version: 1,
            event_id: "31111111-1111-4111-8111-111111111111".into(),
            brain_id: "22222222-2222-4222-8222-222222222222".into(),
            device_id: "rebuild-test".into(),
            device_seq: cryptex_events::DeviceSeq::from_u64(1).unwrap(),
            created_at: "2026-10-08T00:00:00Z".into(),
            event_type: cryptex_events::EventType::MemoryNote,
            classification: cryptex_events::Classification::Private,
            signing_key_id: cryptex_events::SigningKeyId::parse("00112233445566778899aabbccddeeff")
                .unwrap(),
            payload: cryptex_events::EventPayload::MemoryNote(
                cryptex_events::MemoryNotePayload::new("still in M3").unwrap(),
            ),
            prev_content_hash: cryptex_events::ContentHashHex::zero(),
            content_hash: cryptex_events::ContentHashHex::parse(&"1".repeat(64)).unwrap(),
            signature: cryptex_events::SignatureHex::parse(&"0".repeat(128)).unwrap(),
        };
        project_memory_note(db.connection(), &event).unwrap();
        migrate_knowledge_schema(db.connection_mut()).unwrap();
        let provenance = Provenance {
            event_id: event.event_id.clone(),
            device_id: event.device_id.clone(),
            device_seq: 1,
            signing_key_id: event.signing_key_id.as_str().into(),
            confidence: Some(cryptex_events::Confidence::Confirmed),
            authorization_receipt_id: "51111111-1111-4111-8111-111111111111".into(),
            classification: event.classification,
        };
        let entity_id = "61111111-1111-4111-8111-111111111111".to_owned();
        let mut state = KnowledgeState::default();
        state.entities.push(crate::Entity {
            entity_id: entity_id.clone(),
            representative_id: entity_id.clone(),
            kind: cryptex_events::EntityKind::Person,
            label: "Alice".into(),
            provenance: vec![provenance.clone()],
        });
        state.facts.push(Fact {
            semantic_id: "fact-key-1".into(),
            subject_entity_id: entity_id,
            predicate: "person.favorite_color".into(),
            value: cryptex_events::FactValue::Text("blue".into()),
            provenance: vec![provenance],
        });
        let position = CanonicalFramePosition {
            event_id: event.event_id.clone(),
            device_id: event.device_id.clone(),
            pack_index: 0,
            frame_offset: 42,
            content_hash: event.content_hash.as_str().into(),
        };
        let progress = DeviceMaterializationProgress {
            device_id: event.device_id.clone(),
            integrity_position: Some(position.clone()),
            safe_position: Some(position.clone()),
            blocking_reason: None,
        };
        replace_reduced_knowledge_materialization(
            db.connection_mut(),
            &state,
            std::slice::from_ref(&position),
            std::slice::from_ref(&progress),
        )
        .unwrap();
        replace_reduced_knowledge_materialization(
            db.connection_mut(),
            &state,
            std::slice::from_ref(&position),
            std::slice::from_ref(&progress),
        )
        .unwrap();
        let status: String = db
            .connection()
            .query_row(
                "SELECT rebuild_status FROM knowledge_meta WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(status, "ready");
        assert_eq!(db.schema_version().unwrap(), PROJECTION_SCHEMA_VERSION);
        assert_eq!(
            db.connection()
                .query_row("SELECT COUNT(*) FROM knowledge_entities", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            db.connection()
                .query_row("SELECT COUNT(*) FROM knowledge_assertions", [], |row| row
                    .get::<_, i64>(
                    0
                ))
                .unwrap(),
            1
        );
        assert_eq!(
            db.connection()
                .query_row("SELECT COUNT(*) FROM knowledge_provenance", [], |row| row
                    .get::<_, i64>(
                    0
                ))
                .unwrap(),
            2
        );
        assert_eq!(db.connection().query_row("SELECT safe_frame_offset FROM knowledge_materialization_state WHERE device_id = ?1", [&event.device_id], |row| row.get::<_, i64>(0)).unwrap(), 42);
        assert_eq!(
            db.connection()
                .query_row(
                    "SELECT signing_key_id FROM knowledge_provenance WHERE source_event_id = ?1",
                    [&event.event_id],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
            event.signing_key_id.as_str()
        );
        assert_eq!(
            db.connection()
                .query_row(
                    "SELECT text FROM events WHERE event_id = ?1",
                    [&event.event_id],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
            "still in M3"
        );
    }

    #[test]
    fn failed_rebuild_rolls_back_the_previous_m4_generation() {
        let mut db = ProjectionDb::open_in_memory().unwrap();
        migrate_knowledge_schema(db.connection_mut()).unwrap();
        db.connection()
            .execute(
                "INSERT INTO knowledge_conflicts
             (conflict_key, conflict_kind, competing_assertion_ids_json, reason_code, record_json)
             VALUES ('kept', 'test', '[]', 'test', '{}')",
                [],
            )
            .unwrap();
        db.connection()
            .execute_batch(
                "CREATE TRIGGER reject_knowledge_build BEFORE UPDATE ON knowledge_meta
             BEGIN SELECT RAISE(ABORT, 'injected rebuild failure'); END;",
            )
            .unwrap();
        let result = replace_reduced_knowledge_materialization(
            db.connection_mut(),
            &KnowledgeState::default(),
            &[],
            &[],
        );
        assert!(matches!(result, Err(KnowledgeStoreError::Sqlite(_))));
        assert_eq!(
            db.connection()
                .query_row(
                    "SELECT COUNT(*) FROM knowledge_conflicts WHERE conflict_key = 'kept'",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
        let status: String = db
            .connection()
            .query_row(
                "SELECT rebuild_status FROM knowledge_meta WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(status, "not_built");
    }

    #[test]
    fn unresolved_dependency_prevents_safe_watermark_from_crossing_its_frame() {
        let mut db = ProjectionDb::open_in_memory().unwrap();
        migrate_knowledge_schema(db.connection_mut()).unwrap();
        let position = CanonicalFramePosition {
            event_id: "41111111-1111-4111-8111-111111111111".into(),
            device_id: "dependency-test".into(),
            pack_index: 0,
            frame_offset: 64,
            content_hash: "0".repeat(64),
        };
        let mut state = KnowledgeState::default();
        state.pending.push(PendingDependency {
            event_id: position.event_id.clone(),
            target_id: "missing-entity".into(),
            reason: "missing_entity",
        });
        let progress = DeviceMaterializationProgress {
            device_id: position.device_id.clone(),
            integrity_position: Some(position.clone()),
            safe_position: Some(position.clone()),
            blocking_reason: Some("pending_dependency".into()),
        };
        let result = replace_reduced_knowledge_materialization(
            db.connection_mut(),
            &state,
            &[position],
            &[progress],
        );
        assert!(matches!(result, Err(KnowledgeStoreError::Schema(_))));
        assert_eq!(
            db.connection()
                .query_row(
                    "SELECT rebuild_status FROM knowledge_meta WHERE id = 1",
                    [],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
            "not_built"
        );
    }

    #[test]
    fn pending_dependency_is_persisted_without_advancing_safe_watermark() {
        let mut db = ProjectionDb::open_in_memory().unwrap();
        migrate_knowledge_schema(db.connection_mut()).unwrap();
        let prior = CanonicalFramePosition {
            event_id: "41111111-1111-4111-8111-111111111111".into(),
            device_id: "dependency-test".into(),
            pack_index: 0,
            frame_offset: 32,
            content_hash: "0".repeat(64),
        };
        let unresolved = CanonicalFramePosition {
            event_id: "51111111-1111-4111-8111-111111111111".into(),
            device_id: prior.device_id.clone(),
            pack_index: 0,
            frame_offset: 64,
            content_hash: "1".repeat(64),
        };
        let mut state = KnowledgeState::default();
        state.pending.push(PendingDependency {
            event_id: unresolved.event_id.clone(),
            target_id: "missing-entity".into(),
            reason: "missing_entity",
        });
        let progress = DeviceMaterializationProgress {
            device_id: prior.device_id.clone(),
            integrity_position: Some(unresolved.clone()),
            safe_position: Some(prior.clone()),
            blocking_reason: Some("pending_dependency".into()),
        };
        replace_reduced_knowledge_materialization(
            db.connection_mut(),
            &state,
            &[prior, unresolved.clone()],
            &[progress],
        )
        .unwrap();
        assert_eq!(
            db.connection()
                .query_row(
                    "SELECT dependency_id FROM knowledge_pending_dependencies
                     WHERE source_event_id = ?1",
                    [&unresolved.event_id],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
            "missing-entity"
        );
        assert_eq!(
            db.connection()
                .query_row(
                    "SELECT safe_frame_offset FROM knowledge_materialization_state
                 WHERE device_id = ?1",
                    [&unresolved.device_id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            32
        );
        assert_eq!(
            db.connection()
                .query_row(
                    "SELECT rebuild_status FROM knowledge_meta WHERE id = 1",
                    [],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
            "ready"
        );
    }
}
