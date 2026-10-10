//! Daemon-owned, read-only canonical scan followed by a transactional M4 rebuild.
//!
//! This path is intentionally separate from M2's repair-oriented v1 reader. It
//! never truncates, quarantines, seals, or rewrites canonical packs: unknown or
//! unverifiable envelopes stop the rebuild with source bytes intact.

use std::fs;
use std::path::Path;
use std::time::Instant;

use cryptex_core::DeviceId;
use cryptex_events::{TrustedDaemonKeyResolver, VerifiedV2History, VersionedRecord};
use cryptex_knowledge::sqlite::{
    CanonicalFramePosition, DeviceMaterializationProgress, KNOWLEDGE_BUILD_ID_V1,
    KNOWLEDGE_REDUCER_VERSION, KNOWLEDGE_SCHEMA_VERSION, KnowledgeStoreError,
    migrate_knowledge_schema, rebuild_from_verified_history,
};
use cryptex_projection::{
    ProjectionDb, ProjectionError, SearchHit, SearchQuery, search_events, validate_schema_v1,
};
use cryptex_storage::layout::{
    active_segment_path, canonical_events_device_dir, sealed_segment_path,
};
use cryptex_storage::{AcceptAllBodies, StorageError, fold_verified_frames_with_offsets};

use crate::canonical_writer::{
    CanonicalInventoryLock, CanonicalWriterError, DeviceCanonicalLock,
    acquire_canonical_inventory_lock, acquire_canonical_inventory_lock_until,
    acquire_existing_device_canonical_lock, acquire_existing_device_canonical_lock_until,
};

#[derive(Debug)]
pub enum KnowledgeMaterializationError {
    Canonical(CanonicalWriterError),
    Storage(StorageError),
    Verification(String),
    Knowledge(KnowledgeStoreError),
    InvalidInput(String),
    DeadlineExceeded,
}

impl std::fmt::Display for KnowledgeMaterializationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Canonical(error) => write!(f, "canonical scan failed: {error}"),
            Self::Storage(error) => write!(f, "canonical frame scan failed: {error}"),
            Self::Verification(error) => {
                write!(f, "canonical history verification failed: {error}")
            }
            Self::Knowledge(error) => write!(f, "knowledge rebuild failed: {error}"),
            Self::InvalidInput(error) => write!(f, "invalid M4 rebuild input: {error}"),
            Self::DeadlineExceeded => write!(f, "retrieval operation deadline exceeded"),
        }
    }
}

impl std::error::Error for KnowledgeMaterializationError {}

impl From<CanonicalWriterError> for KnowledgeMaterializationError {
    fn from(value: CanonicalWriterError) -> Self {
        Self::Canonical(value)
    }
}
impl From<StorageError> for KnowledgeMaterializationError {
    fn from(value: StorageError) -> Self {
        Self::Storage(value)
    }
}
impl From<KnowledgeStoreError> for KnowledgeMaterializationError {
    fn from(value: KnowledgeStoreError) -> Self {
        Self::Knowledge(value)
    }
}

pub type KnowledgeMaterializationResult<T> = Result<T, KnowledgeMaterializationError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnowledgeRebuildReport {
    pub verified_records: usize,
    pub materialized_assertions: usize,
    pub pending_dependencies: usize,
    pub device_count: usize,
}

/// A stable, non-sensitive reason why content retrieval was withheld.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetrievalSafetyHold {
    OperationDeadlineExceeded,
    CanonicalVerificationUnavailable,
    UnsupportedSemantics,
    V2SemanticsNotIntegrated,
    UnresolvedDependency,
    MaterializationStateMissingOrStale,
    ProjectionDoesNotMatchCanonical,
}

/// Daemon-issued, lock-scoped decision shared by all content-returning
/// adapters. Its private fields prevent callers from manufacturing an allow.
pub struct RetrievalSafetyState {
    hold: Option<RetrievalSafetyHold>,
    snapshot: Option<VerifiedCanonicalSnapshot>,
}

impl RetrievalSafetyState {
    #[must_use]
    pub fn allows_content(&self) -> bool {
        self.hold.is_none() && self.snapshot.is_some()
    }

    #[must_use]
    pub const fn hold_reason(&self) -> Option<RetrievalSafetyHold> {
        self.hold
    }

    fn held(reason: RetrievalSafetyHold) -> Self {
        Self {
            hold: Some(reason),
            snapshot: None,
        }
    }

    fn allowed(snapshot: VerifiedCanonicalSnapshot) -> Self {
        Self {
            hold: None,
            snapshot: Some(snapshot),
        }
    }
}

#[derive(Debug)]
pub enum GuardedSearchError {
    BrainWideHold(RetrievalSafetyHold),
    Query(ProjectionError),
}

impl std::fmt::Display for GuardedSearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BrainWideHold(reason) => {
                write!(
                    f,
                    "retrieval withheld by Brain-wide safety hold: {reason:?}"
                )
            }
            Self::Query(error) => write!(f, "projection search failed: {error}"),
        }
    }
}

impl std::error::Error for GuardedSearchError {}

#[derive(Debug)]
struct RawFrame {
    position: CanonicalFramePosition,
    bytes: Vec<u8>,
}

struct VerifiedCanonicalSnapshot {
    devices: Vec<DeviceId>,
    frames: Vec<RawFrame>,
    history: VerifiedV2History,
    state: cryptex_knowledge::KnowledgeState,
    progress: Vec<DeviceMaterializationProgress>,
    // Keep inventory and canonical writer locks held through the subsequent
    // SQLite safety comparison and content query.
    _inventory_lock: CanonicalInventoryLock,
    _locks: Vec<DeviceCanonicalLock>,
}

/// Rebuild all M4-derived rows from an explicit set of trusted device streams.
///
/// The caller supplies the enrolled-device key resolver; this function does
/// not infer trust from event contents or implement device enrollment. Locks
/// are acquired in stable DeviceId order and held through verification,
/// reduction, and the SQLite transaction (device locks â†’ SQLite).
pub fn rebuild_knowledge_from_canonical<R: TrustedDaemonKeyResolver>(
    runtime_root: &Path,
    expected_brain_id: &str,
    trusted_devices: &[DeviceId],
    key_resolver: &mut R,
    db: &mut ProjectionDb,
) -> KnowledgeMaterializationResult<KnowledgeRebuildReport> {
    let snapshot = scan_verified_canonical(
        runtime_root,
        expected_brain_id,
        trusted_devices,
        key_resolver,
    )?;
    // Migrate only after the complete canonical set verified, so an unknown or
    // unverifiable envelope leaves both canonical packs and M3/M4 DB untouched.
    migrate_knowledge_schema(db.connection_mut())?;
    rebuild_from_verified_history(
        db.connection_mut(),
        expected_brain_id,
        &snapshot.history,
        &snapshot
            .frames
            .iter()
            .map(|frame| frame.position.clone())
            .collect::<Vec<_>>(),
        &snapshot.progress,
    )?;

    Ok(KnowledgeRebuildReport {
        verified_records: snapshot.history.records().len(),
        materialized_assertions: snapshot.history.assertions().len(),
        pending_dependencies: snapshot.state.pending.len(),
        device_count: snapshot.devices.len(),
    })
}

fn scan_verified_canonical<R: TrustedDaemonKeyResolver>(
    runtime_root: &Path,
    expected_brain_id: &str,
    trusted_devices: &[DeviceId],
    key_resolver: &mut R,
) -> KnowledgeMaterializationResult<VerifiedCanonicalSnapshot> {
    scan_verified_canonical_until(
        runtime_root,
        expected_brain_id,
        trusted_devices,
        key_resolver,
        None,
    )
}

fn scan_verified_canonical_until<R: TrustedDaemonKeyResolver>(
    runtime_root: &Path,
    expected_brain_id: &str,
    trusted_devices: &[DeviceId],
    key_resolver: &mut R,
    deadline: Option<Instant>,
) -> KnowledgeMaterializationResult<VerifiedCanonicalSnapshot> {
    check_deadline(deadline)?;
    crate::paths::assert_outside_repository(runtime_root)
        .map_err(|error| KnowledgeMaterializationError::InvalidInput(error.to_string()))?;
    let mut devices = trusted_devices.to_vec();
    devices.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    if devices.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(KnowledgeMaterializationError::InvalidInput(
            "trusted device list contains duplicates".into(),
        ));
    }
    // First-create writers must take this same lock before adding a device
    // directory. This makes inventory enumeration stable for daemon-owned
    // writers while all per-device locks are acquired and scans run.
    let inventory_lock = match deadline {
        Some(deadline) => acquire_canonical_inventory_lock_until(runtime_root, deadline)?,
        None => acquire_canonical_inventory_lock(runtime_root)?,
    };
    require_complete_device_inventory(runtime_root, &devices, deadline)?;

    let mut locks: Vec<DeviceCanonicalLock> = Vec::with_capacity(devices.len());
    for device in &devices {
        check_deadline(deadline)?;
        let lock = match deadline {
            Some(deadline) => {
                acquire_existing_device_canonical_lock_until(runtime_root, device, deadline)?
            }
            None => acquire_existing_device_canonical_lock(runtime_root, device)?,
        };
        if let Some(lock) = lock {
            locks.push(lock);
        }
    }
    // Re-check after per-device locks: this catches any unexpected inventory
    // mutation outside the daemon lock protocol before accepting the snapshot.
    require_complete_device_inventory(runtime_root, &devices, deadline)?;

    let mut frames = Vec::<RawFrame>::new();
    for device in &devices {
        check_deadline(deadline)?;
        scan_device_read_only(runtime_root, device, &mut frames, deadline)?;
    }
    // Detect inventory changes by actors outside the daemon lock protocol
    // (for example a not-yet-integrated sync importer) before accepting this
    // verified snapshot. Such actors must eventually join the same lock
    // protocol for a race-free production guarantee.
    require_complete_device_inventory(runtime_root, &devices, deadline)?;

    let raw_refs: Vec<&[u8]> = frames.iter().map(|frame| frame.bytes.as_slice()).collect();
    let history = cryptex_events::verify_v2_history_cancellable(
        &raw_refs,
        BorrowedResolver(key_resolver),
        || deadline.is_some_and(|deadline| Instant::now() >= deadline),
    )
    .map_err(|error| match error {
        cryptex_events::CancellableHistoryVerificationError::Cancelled => {
            KnowledgeMaterializationError::DeadlineExceeded
        }
        cryptex_events::CancellableHistoryVerificationError::InvalidRecord(error) => {
            KnowledgeMaterializationError::Verification(error.to_string())
        }
    })?;
    if history.records().len() != frames.len() {
        return Err(KnowledgeMaterializationError::InvalidInput(
            "canonical scan contains duplicate event records; refusing ambiguous frame binding"
                .into(),
        ));
    }

    for (record, frame) in history.records().iter().zip(&mut frames) {
        let (event_id, device_id, content_hash) = record_identity(record).ok_or_else(|| {
            KnowledgeMaterializationError::InvalidInput(
                "verified history unexpectedly contains an unverifiable record".into(),
            )
        })?;
        if frame.position.event_id != event_id || frame.position.device_id != device_id {
            return Err(KnowledgeMaterializationError::InvalidInput(
                "verified record does not match its scanned canonical frame position".into(),
            ));
        }
        frame.position.content_hash = content_hash;
    }

    let state =
        cryptex_knowledge::reduce_verified_events_cancellable(expected_brain_id, &history, || {
            deadline.is_some_and(|deadline| Instant::now() >= deadline)
        })
        .map_err(|error| {
            KnowledgeMaterializationError::InvalidInput(format!(
                "reducer rejected verified history: {error:?}"
            ))
        })?;
    let progress = build_progress(
        &devices,
        history.records(),
        &frames,
        &state.pending,
        &history
            .assertions()
            .iter()
            .map(|assertion| assertion.event().event_id.clone())
            .collect::<Vec<_>>(),
        &state.materialized_event_ids,
        deadline,
    )?;
    check_deadline(deadline)?;
    Ok(VerifiedCanonicalSnapshot {
        devices,
        frames,
        history,
        state,
        progress,
        _inventory_lock: inventory_lock,
        _locks: locks,
    })
}

/// Search current M3 rows only after a fresh trusted-history verification,
/// exact M4 per-device tip check, and byte-derived M3 row comparison. Any
/// missing, corrupt, stale, unsupported, or unresolved boundary withholds the
/// entire Brain. Canonical locks remain held through the SQLite query.
pub fn search_events_with_retrieval_safety<R: TrustedDaemonKeyResolver>(
    runtime_root: &Path,
    expected_brain_id: &str,
    trusted_devices: &[DeviceId],
    key_resolver: &mut R,
    db: &ProjectionDb,
    query: &SearchQuery,
) -> Result<Vec<SearchHit>, GuardedSearchError> {
    search_events_with_retrieval_safety_deadline(
        runtime_root,
        expected_brain_id,
        trusted_devices,
        key_resolver,
        db,
        query,
        None,
    )
}

pub fn search_events_with_retrieval_safety_deadline<R: TrustedDaemonKeyResolver>(
    runtime_root: &Path,
    expected_brain_id: &str,
    trusted_devices: &[DeviceId],
    key_resolver: &mut R,
    db: &ProjectionDb,
    query: &SearchQuery,
    deadline: Option<Instant>,
) -> Result<Vec<SearchHit>, GuardedSearchError> {
    let safety = evaluate_retrieval_safety_until(
        runtime_root,
        expected_brain_id,
        trusted_devices,
        key_resolver,
        db,
        deadline,
    );
    if safety.snapshot.is_none() {
        return Err(GuardedSearchError::BrainWideHold(
            safety
                .hold_reason()
                .unwrap_or(RetrievalSafetyHold::CanonicalVerificationUnavailable),
        ));
    }
    let result = {
        // Keep the verified snapshot borrowed throughout the query. The state
        // owns its RAII locks and is dropped only after this scope completes.
        let _snapshot = safety.snapshot.as_ref().expect("checked above");
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return Err(GuardedSearchError::BrainWideHold(
                RetrievalSafetyHold::OperationDeadlineExceeded,
            ));
        }
        let mut guarded_query = query.clone();
        guarded_query.brain_id = Some(expected_brain_id.to_owned());
        match deadline {
            Some(deadline) => match cryptex_projection::search_events_until(
                db.connection(),
                &guarded_query,
                deadline,
            ) {
                Ok(_) if Instant::now() >= deadline => Err(GuardedSearchError::BrainWideHold(
                    RetrievalSafetyHold::OperationDeadlineExceeded,
                )),
                Ok(hits) => Ok(hits),
                Err(_) if Instant::now() >= deadline => Err(GuardedSearchError::BrainWideHold(
                    RetrievalSafetyHold::OperationDeadlineExceeded,
                )),
                Err(error) => Err(GuardedSearchError::Query(error)),
            },
            None => {
                search_events(db.connection(), &guarded_query).map_err(GuardedSearchError::Query)
            }
        }
    };
    // Make the lock lifetime explicit: both canonical locks and inventory
    // lock remain held until the query has completed.
    drop(safety);
    result
}

/// Evaluate the Brain-wide retrieval gate once for any content-returning
/// adapter. A successful state retains inventory/device locks until dropped;
/// callers must keep it alive for the complete retrieval operation.
pub fn evaluate_retrieval_safety<R: TrustedDaemonKeyResolver>(
    runtime_root: &Path,
    expected_brain_id: &str,
    trusted_devices: &[DeviceId],
    key_resolver: &mut R,
    db: &ProjectionDb,
) -> RetrievalSafetyState {
    evaluate_retrieval_safety_until(
        runtime_root,
        expected_brain_id,
        trusted_devices,
        key_resolver,
        db,
        None,
    )
}

fn evaluate_retrieval_safety_until<R: TrustedDaemonKeyResolver>(
    runtime_root: &Path,
    expected_brain_id: &str,
    trusted_devices: &[DeviceId],
    key_resolver: &mut R,
    db: &ProjectionDb,
    deadline: Option<Instant>,
) -> RetrievalSafetyState {
    let snapshot = match scan_verified_canonical_until(
        runtime_root,
        expected_brain_id,
        trusted_devices,
        key_resolver,
        deadline,
    ) {
        Ok(snapshot) => snapshot,
        Err(KnowledgeMaterializationError::DeadlineExceeded) => {
            return RetrievalSafetyState::held(RetrievalSafetyHold::OperationDeadlineExceeded);
        }
        Err(_) if deadline.is_some_and(|deadline| Instant::now() >= deadline) => {
            return RetrievalSafetyState::held(RetrievalSafetyHold::OperationDeadlineExceeded);
        }
        Err(_) => {
            return RetrievalSafetyState::held(
                RetrievalSafetyHold::CanonicalVerificationUnavailable,
            );
        }
    };

    if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        return RetrievalSafetyState::held(RetrievalSafetyHold::OperationDeadlineExceeded);
    }

    // M3 indexes only v1 memory.note. Until v2 privacy/classification effects
    // are integrated across retrieval, any v2 or verified-unsupported record
    // conservatively holds all retrieval.
    for record in snapshot.history.records() {
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return RetrievalSafetyState::held(RetrievalSafetyHold::OperationDeadlineExceeded);
        }
        let hold = match record {
            VersionedRecord::VerifiedV1(_) => continue,
            VersionedRecord::VerifiedUnsupported(_) => RetrievalSafetyHold::UnsupportedSemantics,
            VersionedRecord::VerifiedV2Supported(_) => {
                RetrievalSafetyHold::V2SemanticsNotIntegrated
            }
            VersionedRecord::Unverifiable(_) => {
                RetrievalSafetyHold::CanonicalVerificationUnavailable
            }
        };
        return RetrievalSafetyState::held(hold);
    }
    if !snapshot.state.pending.is_empty()
        || snapshot.state.materialized_event_ids.len() != snapshot.history.assertions().len()
    {
        return RetrievalSafetyState::held(RetrievalSafetyHold::UnresolvedDependency);
    }

    if validate_schema_v1(db.connection()).is_err() {
        return RetrievalSafetyState::held(RetrievalSafetyHold::MaterializationStateMissingOrStale);
    }
    if !m4_materialization_state_matches(db, &snapshot) {
        return RetrievalSafetyState::held(RetrievalSafetyHold::MaterializationStateMissingOrStale);
    }
    let projection_matches =
        m3_projection_matches_history(db, expected_brain_id, &snapshot.history, deadline);
    if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        return RetrievalSafetyState::held(RetrievalSafetyHold::OperationDeadlineExceeded);
    }
    if !projection_matches {
        return RetrievalSafetyState::held(RetrievalSafetyHold::ProjectionDoesNotMatchCanonical);
    }
    RetrievalSafetyState::allowed(snapshot)
}

fn m4_materialization_state_matches(
    db: &ProjectionDb,
    snapshot: &VerifiedCanonicalSnapshot,
) -> bool {
    let meta = db.connection().query_row(
        "SELECT schema_version, reducer_version, cardinality_policy_version,
                normalization_version, build_id, rebuild_status
         FROM knowledge_meta WHERE id = 1",
        [],
        |row| {
            Ok((
                row.get::<_, u32>(0)?,
                row.get::<_, u32>(1)?,
                row.get::<_, u32>(2)?,
                row.get::<_, u32>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        },
    );
    let Ok((schema, reducer, cardinality, normalization, build, status)) = meta else {
        return false;
    };
    if schema != KNOWLEDGE_SCHEMA_VERSION
        || reducer != KNOWLEDGE_REDUCER_VERSION
        || cardinality != cryptex_knowledge::CARDINALITY_POLICY_VERSION
        || normalization != 1
        || build != KNOWLEDGE_BUILD_ID_V1
        || status != "ready"
    {
        return false;
    }

    let pending_count = db.connection().query_row(
        "SELECT COUNT(*) FROM knowledge_pending_dependencies",
        [],
        |row| row.get::<_, i64>(0),
    );
    if !matches!(pending_count, Ok(0)) {
        return false;
    }

    let mut rows = match db.connection().prepare(
        "SELECT device_id, integrity_pack_index, integrity_frame_offset,
                integrity_content_hash, safe_pack_index, safe_frame_offset,
                safe_content_hash, blocking_reason
         FROM knowledge_materialization_state ORDER BY device_id",
    ) {
        Ok(statement) => statement,
        Err(_) => return false,
    };
    let loaded = rows.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<i64>>(1)?,
            row.get::<_, Option<i64>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<i64>>(4)?,
            row.get::<_, Option<i64>>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, Option<String>>(7)?,
        ))
    });
    let Ok(loaded) = loaded else {
        return false;
    };
    let actual: Result<Vec<_>, _> = loaded.collect();
    let Ok(actual) = actual else {
        return false;
    };
    if actual.len() != snapshot.progress.len() {
        return false;
    }
    snapshot
        .progress
        .iter()
        .zip(actual)
        .all(|(expected, actual)| {
            let integrity = expected.integrity_position.as_ref();
            let safe = expected.safe_position.as_ref();
            actual.0 == expected.device_id
                && actual.1 == integrity.and_then(|p| i64::try_from(p.pack_index).ok())
                && actual.2 == integrity.and_then(|p| i64::try_from(p.frame_offset).ok())
                && actual.3.as_deref() == integrity.map(|p| p.content_hash.as_str())
                && actual.4 == safe.and_then(|p| i64::try_from(p.pack_index).ok())
                && actual.5 == safe.and_then(|p| i64::try_from(p.frame_offset).ok())
                && actual.6.as_deref() == safe.map(|p| p.content_hash.as_str())
                && actual.7.is_none()
                && expected.blocking_reason.is_none()
                && expected.safe_position == expected.integrity_position
        })
}

fn m3_projection_matches_history(
    db: &ProjectionDb,
    expected_brain_id: &str,
    history: &VerifiedV2History,
    deadline: Option<Instant>,
) -> bool {
    let mut expected = std::collections::BTreeMap::new();
    for record in history.records() {
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return false;
        }
        let VersionedRecord::VerifiedV1(event) = record else {
            return false;
        };
        let cryptex_events::EventPayload::MemoryNote(payload) = &event.payload;
        let classification = match event.classification {
            cryptex_events::Classification::Public => "public",
            cryptex_events::Classification::Internal => "internal",
            cryptex_events::Classification::Private => "private",
            cryptex_events::Classification::Restricted => "restricted",
        };
        let hit = SearchHit {
            event_id: event.event_id.clone(),
            brain_id: event.brain_id.clone(),
            device_id: event.device_id.clone(),
            device_seq: event.device_seq.as_str().to_owned(),
            created_at: event.created_at.clone(),
            event_type: event.event_type.as_str().to_owned(),
            classification: classification.to_owned(),
            content_hash: event.content_hash.as_str().to_owned(),
            signing_key_id: event.signing_key_id.as_str().to_owned(),
            text: payload.text.clone(),
        };
        if expected.insert(event.event_id.clone(), hit).is_some() {
            return false;
        }
    }
    if expected
        .values()
        .any(|hit| hit.brain_id != expected_brain_id)
    {
        return false;
    }

    let mut statement = match db.connection().prepare(
        "SELECT event_id, brain_id, device_id, device_seq, created_at, event_type,
                classification, content_hash, signing_key_id, text
         FROM events WHERE brain_id = ?1 ORDER BY event_id",
    ) {
        Ok(statement) => statement,
        Err(_) => return false,
    };
    let rows = statement.query_map([expected_brain_id], |row| {
        Ok(SearchHit {
            event_id: row.get(0)?,
            brain_id: row.get(1)?,
            device_id: row.get(2)?,
            device_seq: row.get(3)?,
            created_at: row.get(4)?,
            event_type: row.get(5)?,
            classification: row.get(6)?,
            content_hash: row.get(7)?,
            signing_key_id: row.get(8)?,
            text: row.get(9)?,
        })
    });
    let Ok(rows) = rows else {
        return false;
    };
    let mut actual = std::collections::BTreeMap::new();
    for row in rows {
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return false;
        }
        let Ok(hit) = row else {
            return false;
        };
        actual.insert(hit.event_id.clone(), hit);
    }
    actual == expected
}

struct BorrowedResolver<'a, R>(&'a mut R);
impl<R: TrustedDaemonKeyResolver> TrustedDaemonKeyResolver for BorrowedResolver<'_, R> {
    fn trusted_daemon_key(
        &mut self,
        device_id: &str,
        key_id: &str,
    ) -> Option<ed25519_dalek::VerifyingKey> {
        self.0.trusted_daemon_key(device_id, key_id)
    }
}

fn scan_device_read_only(
    runtime_root: &Path,
    device_id: &DeviceId,
    output: &mut Vec<RawFrame>,
    deadline: Option<Instant>,
) -> KnowledgeMaterializationResult<()> {
    check_deadline(deadline)?;
    let directory = canonical_events_device_dir(runtime_root, device_id);
    if !directory.exists() {
        return Ok(());
    }
    let mut sealed = Vec::new();
    let mut open = None;
    for entry in fs::read_dir(&directory).map_err(StorageError::Io)? {
        check_deadline(deadline)?;
        let entry = entry.map_err(StorageError::Io)?;
        if !entry.file_type().map_err(StorageError::Io)?.is_file() {
            return Err(KnowledgeMaterializationError::Storage(
                StorageError::Integrity(
                    "canonical device directory contains a non-file entry".into(),
                ),
            ));
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        match (
            parse_pack_index(&name, "seg"),
            parse_pack_index(&name, "open"),
        ) {
            (Some(index), _) => sealed.push(index),
            (None, Some(index)) => {
                if open.replace(index).is_some() {
                    return Err(KnowledgeMaterializationError::Storage(
                        StorageError::Integrity("multiple active packs for one device".into()),
                    ));
                }
            }
            (None, None) if is_known_housekeeping_file(&name) => {}
            (None, None) if name.ends_with(".seg") || name.ends_with(".open") => {
                return Err(KnowledgeMaterializationError::Storage(
                    StorageError::Integrity(
                        "canonical directory contains a malformed pack filename".into(),
                    ),
                ));
            }
            (None, None) => {
                return Err(KnowledgeMaterializationError::Storage(
                    StorageError::Integrity(
                        "canonical device directory contains an unexpected entry".into(),
                    ),
                ));
            }
        }
    }
    sealed.sort_unstable();
    if sealed
        .iter()
        .enumerate()
        .any(|(expected, actual)| expected as u64 != *actual)
    {
        return Err(KnowledgeMaterializationError::Storage(
            StorageError::Integrity("sealed pack indexes are not contiguous".into()),
        ));
    }
    if let Some(open_index) = open
        && (sealed.contains(&open_index)
            || sealed.last().is_some_and(|last| open_index != last + 1)
            || (sealed.is_empty() && open_index != 0))
    {
        return Err(KnowledgeMaterializationError::Storage(
            StorageError::Integrity("active pack layout conflicts with sealed history".into()),
        ));
    }

    for index in sealed {
        let path = sealed_segment_path(runtime_root, device_id, index);
        let before = output.len();
        let verified_len = fold_pack(&path, device_id, index, output, deadline)?;
        let file_len = fs::metadata(&path).map_err(StorageError::Io)?.len();
        if verified_len != file_len || output.len() == before {
            return Err(KnowledgeMaterializationError::Storage(StorageError::Integrity(
                "sealed pack is incomplete or empty; M4 rebuild stopped without modifying canonical bytes".into(),
            )));
        }
    }
    if let Some(index) = open {
        let path = active_segment_path(runtime_root, device_id, index);
        let file_len = fs::metadata(&path).map_err(StorageError::Io)?.len();
        let verified_len = fold_pack(&path, device_id, index, output, deadline)?;
        if verified_len != file_len {
            return Err(KnowledgeMaterializationError::Storage(
                StorageError::CompatibilityBoundary(
                    "active pack has an incomplete or unverified suffix; M4 rebuild stopped without modifying canonical bytes".into(),
                ),
            ));
        }
    }
    Ok(())
}

fn is_known_housekeeping_file(name: &str) -> bool {
    if matches!(name, "writer.lock" | "writer_tip.json") {
        return true;
    }
    if let Some(suffix) = name.strip_prefix("writer_tip.json.tmp.") {
        return !suffix.is_empty()
            && suffix
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b'-');
    }
    name.strip_suffix(".open.lock")
        .is_some_and(|stem| stem.len() == 20 && stem.bytes().all(|byte| byte.is_ascii_digit()))
}

/// A full rebuild must not silently omit a canonical device stream. The
/// caller-provided trusted set is checked against the on-disk stream inventory
/// before locks are acquired; unknown or malformed directory names fail closed.
fn require_complete_device_inventory(
    runtime_root: &Path,
    trusted_devices: &[DeviceId],
    deadline: Option<Instant>,
) -> KnowledgeMaterializationResult<()> {
    check_deadline(deadline)?;
    let events_root = runtime_root.join("canonical").join("events");
    if !events_root.exists() {
        return Ok(());
    }
    let mut on_disk = Vec::<String>::new();
    for entry in fs::read_dir(&events_root).map_err(StorageError::Io)? {
        check_deadline(deadline)?;
        let entry = entry.map_err(StorageError::Io)?;
        let file_type = entry.file_type().map_err(StorageError::Io)?;
        if file_type.is_file() && entry.file_name().to_string_lossy() == "inventory.lock" {
            continue;
        }
        if !file_type.is_dir() {
            return Err(KnowledgeMaterializationError::Storage(
                StorageError::Integrity(
                    "unexpected non-directory in canonical device inventory".into(),
                ),
            ));
        }
        let stem = entry.file_name().to_string_lossy().into_owned();
        let decoded = decode_device_stem(&stem).ok_or_else(|| {
            KnowledgeMaterializationError::Storage(StorageError::Integrity(
                "canonical device inventory contains an invalid id stem".into(),
            ))
        })?;
        on_disk.push(decoded);
    }
    on_disk.sort();
    let mut supplied: Vec<String> = trusted_devices
        .iter()
        .map(|device| device.as_str().to_owned())
        .collect();
    supplied.sort();
    if on_disk
        .iter()
        .any(|device| supplied.binary_search(device).is_err())
    {
        return Err(KnowledgeMaterializationError::InvalidInput(
            "trusted device set does not cover the complete canonical device inventory".into(),
        ));
    }
    Ok(())
}

fn decode_device_stem(stem: &str) -> Option<String> {
    if stem.is_empty()
        || !stem.len().is_multiple_of(2)
        || !stem
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    let mut bytes = Vec::with_capacity(stem.len() / 2);
    for pair in stem.as_bytes().as_chunks::<2>().0 {
        let high = hex_nibble(pair[0])?;
        let low = hex_nibble(pair[1])?;
        bytes.push((high << 4) | low);
    }
    let value = String::from_utf8(bytes).ok()?;
    DeviceId::new(value.clone()).ok()?;
    Some(value)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn fold_pack(
    path: &Path,
    device_id: &DeviceId,
    pack_index: u64,
    output: &mut Vec<RawFrame>,
    deadline: Option<Instant>,
) -> KnowledgeMaterializationResult<u64> {
    let (frames, verified_len) = fold_verified_frames_with_offsets(
        path,
        &AcceptAllBodies,
        Vec::new(),
        |frames, offset, body| {
            check_deadline(deadline).map_err(|_| {
                StorageError::CompatibilityBoundary(
                    "canonical retrieval scan exceeded its operation deadline".into(),
                )
            })?;
            frames.push((offset, body.to_vec()));
            Ok(())
        },
    )?;
    for (offset, bytes) in frames {
        let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| {
            StorageError::CompatibilityBoundary(
                "canonical event is not valid JSON; M4 rebuild stopped without modifying source"
                    .into(),
            )
        })?;
        let event_id = value
            .get("event_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                StorageError::CompatibilityBoundary("canonical event id is unavailable".into())
            })?;
        let claimed_device = value
            .get("device_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                StorageError::CompatibilityBoundary("canonical device id is unavailable".into())
            })?;
        if claimed_device != device_id.as_str() {
            return Err(KnowledgeMaterializationError::Storage(
                StorageError::Integrity(
                    "canonical event device does not match its pack directory".into(),
                ),
            ));
        }
        output.push(RawFrame {
            position: CanonicalFramePosition {
                event_id: event_id.to_owned(),
                device_id: device_id.as_str().to_owned(),
                pack_index,
                frame_offset: offset,
                // Filled from the cryptographically verified classification
                // after the full chain has been checked.
                content_hash: String::new(),
            },
            bytes,
        });
    }
    Ok(verified_len)
}

fn check_deadline(deadline: Option<Instant>) -> KnowledgeMaterializationResult<()> {
    if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        Err(KnowledgeMaterializationError::DeadlineExceeded)
    } else {
        Ok(())
    }
}

fn parse_pack_index(name: &str, extension: &str) -> Option<u64> {
    let stem = name.strip_suffix(&format!(".{extension}"))?;
    if stem.len() != 20 || !stem.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    stem.parse().ok()
}

fn record_identity(record: &VersionedRecord) -> Option<(String, String, String)> {
    match record {
        VersionedRecord::VerifiedV1(event) => Some((
            event.event_id.clone(),
            event.device_id.as_str().to_owned(),
            event.content_hash.as_str().to_owned(),
        )),
        VersionedRecord::VerifiedV2Supported(event) => Some((
            event.event().event_id.clone(),
            event.event().device_id.as_str().to_owned(),
            event.event().content_hash.as_str().to_owned(),
        )),
        VersionedRecord::VerifiedUnsupported(event) => Some((
            event.event_id.clone(),
            event.device_id.clone(),
            event.content_hash.as_str().to_owned(),
        )),
        VersionedRecord::Unverifiable(_) => None,
    }
}

fn build_progress(
    devices: &[DeviceId],
    records: &[VersionedRecord],
    frames: &[RawFrame],
    pending: &[cryptex_knowledge::PendingDependency],
    supported_event_ids: &[String],
    materialized_event_ids: &[String],
    deadline: Option<Instant>,
) -> KnowledgeMaterializationResult<Vec<DeviceMaterializationProgress>> {
    let mut pending_ids = std::collections::BTreeSet::new();
    for item in pending {
        check_deadline(deadline)?;
        pending_ids.insert(item.event_id.to_ascii_lowercase());
    }
    let mut materialized_ids = std::collections::BTreeSet::new();
    for event_id in materialized_event_ids {
        check_deadline(deadline)?;
        materialized_ids.insert(event_id.to_ascii_lowercase());
    }
    let mut unsupported_ids = std::collections::BTreeSet::new();
    for record in records {
        check_deadline(deadline)?;
        if let VersionedRecord::VerifiedUnsupported(event) = record {
            unsupported_ids.insert(event.event_id.to_ascii_lowercase());
        }
    }
    // Supported v2 records omitted by the reducer are not safe progress. This
    // includes semantically invalid records retained as reviewable conflicts;
    // conservatively stop the contiguous watermark instead of implying that
    // their semantics were successfully materialized.
    let mut unmaterialized_supported_ids = std::collections::BTreeSet::new();
    for event_id in supported_event_ids {
        check_deadline(deadline)?;
        let event_id = event_id.to_ascii_lowercase();
        if !materialized_ids.contains(&event_id) {
            unmaterialized_supported_ids.insert(event_id);
        }
    }
    let mut output = Vec::with_capacity(devices.len());
    for device in devices {
        check_deadline(deadline)?;
        let mut indexes = Vec::new();
        for (index, frame) in frames.iter().enumerate() {
            check_deadline(deadline)?;
            if frame.position.device_id == device.as_str() {
                indexes.push(index);
            }
        }
        let integrity_position = indexes.last().map(|index| frames[*index].position.clone());
        let mut first_blocker = None;
        for (position, index) in indexes.iter().enumerate() {
            check_deadline(deadline)?;
            let event_id = frames[*index].position.event_id.to_ascii_lowercase();
            if pending_ids.contains(&event_id)
                || unsupported_ids.contains(&event_id)
                || unmaterialized_supported_ids.contains(&event_id)
            {
                first_blocker = Some(position);
                break;
            }
        }
        let safe_index = first_blocker.unwrap_or(indexes.len());
        let safe_position = safe_index
            .checked_sub(1)
            .and_then(|last| indexes.get(last))
            .map(|index| frames[*index].position.clone());
        let blocking_reason = first_blocker.map(|position| {
            let event_id = frames[indexes[position]]
                .position
                .event_id
                .to_ascii_lowercase();
            if unsupported_ids.contains(&event_id) {
                "verified_unsupported_event".to_owned()
            } else if unmaterialized_supported_ids.contains(&event_id) {
                "unmaterialized_supported_event".to_owned()
            } else {
                "pending_dependency".to_owned()
            }
        });
        output.push(DeviceMaterializationProgress {
            device_id: device.as_str().to_owned(),
            integrity_position,
            safe_position,
            blocking_reason,
        });
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canonical_writer::{AppendMemoryNoteArgs, append_memory_note};
    use crate::event_signing_key::ensure_event_signing_key;
    use crate::projection::rebuild_device_projection;
    use crate::secure_store::{FakeSecureStore, SecureStoreEnvironment};
    use crate::test_temp::exclusive_temp_dir;
    use cryptex_events::{Classification, TrustedDaemonKeyResolver};
    use cryptex_projection::ProjectionDb;
    use ed25519_dalek::VerifyingKey;
    use std::fs;
    use std::io::Write;

    struct OneTrustedDevice {
        device: String,
        key_id: String,
        key: VerifyingKey,
    }

    impl TrustedDaemonKeyResolver for OneTrustedDevice {
        fn trusted_daemon_key(&mut self, device_id: &str, key_id: &str) -> Option<VerifyingKey> {
            (device_id == self.device && key_id == self.key_id).then_some(self.key)
        }
    }

    #[test]
    fn daemon_rebuild_scans_verified_v1_frames_without_changing_canonical_bytes() {
        let root = exclusive_temp_dir("m4-knowledge-reader");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("m4-reader-device").unwrap();
        let brain = "33333333-3333-4333-8333-333333333333";
        let config = crate::canonical_writer::CanonicalWriterConfig::default();
        append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Internal,
                text: "synthetic note",
                event_id: None,
                config: &config,
            },
        )
        .unwrap();
        let pack = active_segment_path(&root, &device, 0);
        let before = fs::read(&pack).unwrap();
        let ready = ensure_event_signing_key(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            &device,
            &[],
            true,
        )
        .unwrap();
        let mut resolver = OneTrustedDevice {
            device: device.as_str().to_owned(),
            key_id: ready.public.signing_key_id,
            key: ready.signing_key.verifying_key(),
        };
        let mut db = ProjectionDb::open_in_memory().unwrap();

        let report = rebuild_knowledge_from_canonical(
            &root,
            brain,
            std::slice::from_ref(&device),
            &mut resolver,
            &mut db,
        )
        .unwrap();

        assert_eq!(report.verified_records, 1);
        assert_eq!(report.materialized_assertions, 0);
        assert_eq!(report.pending_dependencies, 0);
        assert_eq!(report.device_count, 1);
        assert_eq!(fs::read(&pack).unwrap(), before);
        let row_count: i64 = db
            .connection()
            .query_row("SELECT COUNT(*) FROM knowledge_meta", [], |row| row.get(0))
            .unwrap();
        assert_eq!(row_count, 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn guarded_search_allows_only_current_verified_and_materialized_v1_projection() {
        let root = exclusive_temp_dir("m4-safe-search");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("m4-safe-search-device").unwrap();
        let brain = "33333333-3333-4333-8333-333333333333";
        append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Internal,
                text: "synthetic safe search note",
                event_id: None,
                config: &crate::canonical_writer::CanonicalWriterConfig::default(),
            },
        )
        .unwrap();
        let ready = ensure_event_signing_key(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            &device,
            &[],
            true,
        )
        .unwrap();
        let mut resolver = OneTrustedDevice {
            device: device.as_str().to_owned(),
            key_id: ready.public.signing_key_id,
            key: ready.signing_key.verifying_key(),
        };
        let mut db = ProjectionDb::open_in_memory().unwrap();

        let held = search_events_with_retrieval_safety(
            &root,
            brain,
            std::slice::from_ref(&device),
            &mut resolver,
            &db,
            &SearchQuery::default(),
        )
        .unwrap_err();
        assert!(matches!(
            held,
            GuardedSearchError::BrainWideHold(
                RetrievalSafetyHold::MaterializationStateMissingOrStale
            )
        ));

        rebuild_device_projection(&store, SecureStoreEnvironment::Dev, &root, &device, &db)
            .unwrap();
        rebuild_knowledge_from_canonical(
            &root,
            brain,
            std::slice::from_ref(&device),
            &mut resolver,
            &mut db,
        )
        .unwrap();

        let hits = search_events_with_retrieval_safety(
            &root,
            brain,
            std::slice::from_ref(&device),
            &mut resolver,
            &db,
            &SearchQuery::default(),
        )
        .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].text, "synthetic safe search note");

        db.connection()
            .execute(
                "UPDATE events SET text = 'tampered projection text' WHERE brain_id = ?1",
                [brain],
            )
            .unwrap();
        let held = search_events_with_retrieval_safety(
            &root,
            brain,
            std::slice::from_ref(&device),
            &mut resolver,
            &db,
            &SearchQuery::default(),
        )
        .unwrap_err();
        assert!(matches!(
            held,
            GuardedSearchError::BrainWideHold(RetrievalSafetyHold::ProjectionDoesNotMatchCanonical)
        ));

        append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Internal,
                text: "new note makes safety snapshot stale",
                event_id: None,
                config: &crate::canonical_writer::CanonicalWriterConfig::default(),
            },
        )
        .unwrap();
        let held = search_events_with_retrieval_safety(
            &root,
            brain,
            std::slice::from_ref(&device),
            &mut resolver,
            &db,
            &SearchQuery::default(),
        )
        .unwrap_err();
        assert!(matches!(
            held,
            GuardedSearchError::BrainWideHold(
                RetrievalSafetyHold::MaterializationStateMissingOrStale
            )
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn expired_retrieval_deadline_issues_brain_wide_hold_without_reading_content() {
        let root = exclusive_temp_dir("retrieval-expired-deadline");
        let db = ProjectionDb::open_in_memory().unwrap();
        let device = DeviceId::new("deadline-test").unwrap();
        let key = ed25519_dalek::SigningKey::from_bytes(&[0x42; 32]).verifying_key();
        let mut resolver = OneTrustedDevice {
            device: device.as_str().to_owned(),
            key_id: cryptex_events::signing_key_id_from_verifying_key(&key)
                .unwrap()
                .as_str()
                .to_owned(),
            key,
        };
        let safety = evaluate_retrieval_safety_until(
            &root,
            "22222222-2222-4222-8222-222222222222",
            &[device],
            &mut resolver,
            &db,
            Some(Instant::now() - std::time::Duration::from_millis(1)),
        );
        assert!(!safety.allows_content());
        assert_eq!(
            safety.hold_reason(),
            Some(RetrievalSafetyHold::OperationDeadlineExceeded)
        );
        assert_eq!(
            db.connection()
                .query_row("SELECT COUNT(*) FROM events", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn pending_dependency_keeps_safe_watermark_before_blocking_frame() {
        let device = DeviceId::new("progress-device").unwrap();
        let frames = [1, 2, 3].map(|index| RawFrame {
            position: CanonicalFramePosition {
                event_id: format!("event-{index}"),
                device_id: device.as_str().to_owned(),
                pack_index: 0,
                frame_offset: (index - 1) * 100,
                content_hash: format!("hash-{index}"),
            },
            bytes: Vec::new(),
        });
        let pending = cryptex_knowledge::PendingDependency {
            event_id: "event-2".into(),
            target_id: "missing-event".into(),
            reason: "missing_assertion",
        };
        let progress = build_progress(&[device], &[], &frames, &[pending], &[], &[], None).unwrap();
        assert_eq!(
            progress[0].integrity_position,
            Some(frames[2].position.clone())
        );
        assert_eq!(progress[0].safe_position, Some(frames[0].position.clone()));
        assert_eq!(
            progress[0].blocking_reason.as_deref(),
            Some("pending_dependency")
        );
    }

    #[test]
    fn supported_event_omitted_by_reducer_blocks_safe_watermark() {
        let device = DeviceId::new("invalid-semantics-device").unwrap();
        let frames = [1, 2, 3].map(|index| RawFrame {
            position: CanonicalFramePosition {
                event_id: format!("event-{index}"),
                device_id: device.as_str().to_owned(),
                pack_index: 0,
                frame_offset: (index - 1) * 100,
                content_hash: format!("hash-{index}"),
            },
            bytes: Vec::new(),
        });

        // `event-2` is a supported signed v2 record whose semantics were not
        // accepted by the reducer (for example, a wrong-kind reference).
        let progress = build_progress(
            &[device],
            &[],
            &frames,
            &[],
            &["event-2".into()],
            &["event-1".into()],
            None,
        )
        .unwrap();

        assert_eq!(
            progress[0].integrity_position,
            Some(frames[2].position.clone())
        );
        assert_eq!(progress[0].safe_position, Some(frames[0].position.clone()));
        assert_eq!(
            progress[0].blocking_reason.as_deref(),
            Some("unmaterialized_supported_event")
        );
    }

    #[test]
    fn full_rebuild_refuses_to_omit_existing_canonical_device_stream() {
        let root = exclusive_temp_dir("m4-device-inventory");
        let unexpected = DeviceId::new("not-in-trust-set").unwrap();
        fs::create_dir_all(canonical_events_device_dir(&root, &unexpected)).unwrap();
        let trusted = DeviceId::new("trusted-device").unwrap();
        let error = require_complete_device_inventory(&root, &[trusted], None).unwrap_err();
        assert!(matches!(
            error,
            KnowledgeMaterializationError::InvalidInput(_)
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn verified_unsupported_event_blocks_safe_watermark_but_not_integrity() {
        let device = DeviceId::new("unsupported-progress-device").unwrap();
        let frames = [1, 2, 3].map(|index| RawFrame {
            position: CanonicalFramePosition {
                event_id: format!("event-{index}"),
                device_id: device.as_str().to_owned(),
                pack_index: 0,
                frame_offset: (index - 1) * 100,
                content_hash: format!("hash-{index}"),
            },
            bytes: Vec::new(),
        });
        let unsupported =
            VersionedRecord::VerifiedUnsupported(cryptex_events::VerifiedUnsupportedV2 {
                event_id: "event-2".into(),
                device_id: device.as_str().to_owned(),
                device_seq: cryptex_events::DeviceSeq::from_u64(2).unwrap(),
                signing_key_id: cryptex_events::hex::SigningKeyId::parse(&"11".repeat(16)).unwrap(),
                prev_content_hash: cryptex_events::ContentHashHex::zero(),
                content_hash: cryptex_events::ContentHashHex::parse(&"22".repeat(32)).unwrap(),
                raw_record: b"verified unsupported synthetic record".to_vec(),
            });
        let progress =
            build_progress(&[device], &[unsupported], &frames, &[], &[], &[], None).unwrap();
        assert_eq!(
            progress[0].integrity_position,
            Some(frames[2].position.clone())
        );
        assert_eq!(progress[0].safe_position, Some(frames[0].position.clone()));
        assert_eq!(
            progress[0].blocking_reason.as_deref(),
            Some("verified_unsupported_event")
        );
    }

    #[test]
    fn unverifiable_envelope_stops_rebuild_without_rewriting_source_or_db() {
        let root = exclusive_temp_dir("m4-unknown-envelope");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("m4-unknown-device").unwrap();
        let brain = "33333333-3333-4333-8333-333333333333";
        let config = crate::canonical_writer::CanonicalWriterConfig::default();
        append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Internal,
                text: "synthetic note",
                event_id: None,
                config: &config,
            },
        )
        .unwrap();
        let ready = ensure_event_signing_key(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            &device,
            &[],
            true,
        )
        .unwrap();
        let unknown = serde_json::json!({
            "schema_version": 99,
            "event_id": "44444444-4444-4444-8444-444444444444",
            "device_id": device.as_str(),
            "signing_key_id": ready.public.signing_key_id,
            "opaque": ["do not parse or rewrite"]
        });
        let unknown_bytes = serde_json::to_vec(&unknown).unwrap();
        cryptex_storage::append_frame(&root, &device, 0, &unknown_bytes).unwrap();
        let pack = active_segment_path(&root, &device, 0);
        let before = fs::read(&pack).unwrap();
        let mut resolver = OneTrustedDevice {
            device: device.as_str().to_owned(),
            key_id: ready.public.signing_key_id,
            key: ready.signing_key.verifying_key(),
        };
        let mut db = ProjectionDb::open_in_memory().unwrap();
        db.connection()
            .execute_batch(
                "CREATE TABLE rebuild_sentinel (value TEXT NOT NULL);
                 INSERT INTO rebuild_sentinel VALUES ('keep');",
            )
            .unwrap();

        let result = rebuild_knowledge_from_canonical(
            &root,
            brain,
            std::slice::from_ref(&device),
            &mut resolver,
            &mut db,
        );

        assert!(matches!(
            result,
            Err(KnowledgeMaterializationError::Verification(_))
        ));
        assert_eq!(fs::read(&pack).unwrap(), before);
        let sentinel: String = db
            .connection()
            .query_row("SELECT value FROM rebuild_sentinel", [], |row| row.get(0))
            .unwrap();
        assert_eq!(sentinel, "keep");
        let m4_tables: i64 = db
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='knowledge_meta'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(m4_tables, 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn incomplete_active_suffix_stops_rebuild_before_migration_and_keeps_bytes() {
        let root = exclusive_temp_dir("m4-incomplete-open");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("m4-incomplete-open-device").unwrap();
        let brain = "33333333-3333-4333-8333-333333333333";
        let config = crate::canonical_writer::CanonicalWriterConfig::default();
        append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Internal,
                text: "synthetic note",
                event_id: None,
                config: &config,
            },
        )
        .unwrap();
        let ready = ensure_event_signing_key(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            &device,
            &[],
            true,
        )
        .unwrap();
        let mut resolver = OneTrustedDevice {
            device: device.as_str().to_owned(),
            key_id: ready.public.signing_key_id,
            key: ready.signing_key.verifying_key(),
        };
        let pack = active_segment_path(&root, &device, 0);
        let mut file = fs::OpenOptions::new().append(true).open(&pack).unwrap();
        file.write_all(&[0x43, 0x58]).unwrap();
        file.sync_all().unwrap();
        let before = fs::read(&pack).unwrap();
        let mut db = ProjectionDb::open_in_memory().unwrap();
        db.connection()
            .execute_batch(
                "CREATE TABLE rebuild_sentinel (value TEXT NOT NULL);
                 INSERT INTO rebuild_sentinel VALUES ('keep');",
            )
            .unwrap();

        let result = rebuild_knowledge_from_canonical(
            &root,
            brain,
            std::slice::from_ref(&device),
            &mut resolver,
            &mut db,
        );

        assert!(matches!(
            result,
            Err(KnowledgeMaterializationError::Storage(
                StorageError::CompatibilityBoundary(_)
            ))
        ));
        assert_eq!(fs::read(&pack).unwrap(), before);
        let sentinel: String = db
            .connection()
            .query_row("SELECT value FROM rebuild_sentinel", [], |row| row.get(0))
            .unwrap();
        assert_eq!(sentinel, "keep");
        let m4_tables: i64 = db
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='knowledge_meta'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(m4_tables, 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn malformed_pack_name_blocks_full_rebuild_instead_of_being_ignored() {
        let root = exclusive_temp_dir("m4-malformed-pack-name");
        let device = DeviceId::new("m4-malformed-name-device").unwrap();
        let directory = canonical_events_device_dir(&root, &device);
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("not-an-index.seg"), b"must not be omitted").unwrap();
        let result = scan_device_read_only(&root, &device, &mut Vec::new(), None);
        assert!(matches!(
            result,
            Err(KnowledgeMaterializationError::Storage(
                StorageError::Integrity(_)
            ))
        ));
        fs::remove_dir_all(root).unwrap();
    }
}
