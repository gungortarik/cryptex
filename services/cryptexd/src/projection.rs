//! cryptexd-owned projection of verified canonical events (M3 Slice 4/5).
//!
//! Reads only through [`crate::canonical_writer::for_each_verified_event`].
//! Never parses `.open`/`.seg` outside that path and never mutates canonical packs
//! for projection purposes. Checkpoints advance in the same SQLite transaction as
//! projected rows.

use std::fs;
use std::path::Path;

use cryptex_core::ids::DeviceId;
use cryptex_projection::{
    DeviceCheckpoint, ProjectionDb, ProjectionError, ProjectionOpenKind, clear_device_projection,
    project_memory_note, read_device_checkpoint, upsert_device_checkpoint,
};
use cryptex_storage::layout::{active_segment_path, canonical_events_device_dir};

use crate::canonical_writer::{
    CanonicalWriterError, acquire_device_canonical_lock, for_each_verified_event_with_lock,
};
use crate::secure_store::{SecureStore, SecureStoreEnvironment};

/// Errors from verified-feed projection into disposable SQLite.
#[derive(Debug)]
pub enum ProjectionFeedError {
    Canonical(CanonicalWriterError),
    Projection(ProjectionError),
}

impl std::fmt::Display for ProjectionFeedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Canonical(err) => write!(f, "projection feed canonical error: {err}"),
            Self::Projection(err) => write!(f, "projection feed sqlite error: {err}"),
        }
    }
}

impl std::error::Error for ProjectionFeedError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Canonical(err) => Some(err),
            Self::Projection(err) => Some(err),
        }
    }
}

impl From<CanonicalWriterError> for ProjectionFeedError {
    fn from(value: CanonicalWriterError) -> Self {
        Self::Canonical(value)
    }
}

impl From<ProjectionError> for ProjectionFeedError {
    fn from(value: ProjectionError) -> Self {
        Self::Projection(value)
    }
}

pub type ProjectionFeedResult<T> = Result<T, ProjectionFeedError>;

/// Why a device projection was fully rebuilt instead of incremented.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionRebuildReason {
    /// No checkpoint row yet (first catch-up / empty DB).
    MissingCheckpoint,
    /// Checkpoint tip not found in verified packs (ahead / unknown hash/seq).
    CheckpointUnknownOrAhead,
    /// Caller requested a full rebuild.
    Forced,
}

/// Outcome of syncing one device's projection to the verified tip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceProjectionSync {
    pub projected_events: u64,
    pub rebuilt: bool,
    pub rebuild_reason: Option<ProjectionRebuildReason>,
}

/// Open the disposable projection DB, replacing corrupt/stale/unsupported files.
///
/// Canonical packs are never modified by this function.
pub fn open_projection_db(
    runtime_root: &Path,
) -> ProjectionFeedResult<(ProjectionDb, ProjectionOpenKind)> {
    Ok(ProjectionDb::open_replacing_unusable(runtime_root)?)
}

/// Project one device's verified `memory.note` events into `db` in a single transaction.
///
/// Projects the full verified feed (idempotent inserts) and advances the device
/// checkpoint to the verified tip in the same transaction.
pub fn project_verified_device_events<S: SecureStore>(
    store: &S,
    env: SecureStoreEnvironment,
    runtime_root: &Path,
    device_id: &DeviceId,
    db: &ProjectionDb,
) -> ProjectionFeedResult<u64> {
    let report = run_device_projection(
        store,
        env,
        runtime_root,
        device_id,
        db,
        DeviceProjectMode::FullIdempotent,
    )?;
    Ok(report.projected_events)
}

/// Full rebuild for one device: clear disposable rows, project verified feed, checkpoint.
pub fn rebuild_device_projection<S: SecureStore>(
    store: &S,
    env: SecureStoreEnvironment,
    runtime_root: &Path,
    device_id: &DeviceId,
    db: &ProjectionDb,
) -> ProjectionFeedResult<DeviceProjectionSync> {
    run_device_projection(
        store,
        env,
        runtime_root,
        device_id,
        db,
        DeviceProjectMode::Rebuild {
            reason: ProjectionRebuildReason::Forced,
        },
    )
}

/// Incremental catch-up from checkpoint; rebuilds disposable state if the checkpoint
/// is missing, unknown, or ahead of verified packs.
pub fn catch_up_device_projection<S: SecureStore>(
    store: &S,
    env: SecureStoreEnvironment,
    runtime_root: &Path,
    device_id: &DeviceId,
    db: &ProjectionDb,
) -> ProjectionFeedResult<DeviceProjectionSync> {
    let checkpoint = read_device_checkpoint(db.connection(), device_id.as_str())
        .map_err(ProjectionFeedError::Projection)?;
    match checkpoint {
        None => run_device_projection(
            store,
            env,
            runtime_root,
            device_id,
            db,
            DeviceProjectMode::Rebuild {
                reason: ProjectionRebuildReason::MissingCheckpoint,
            },
        ),
        Some(cp) => {
            match run_device_projection(
                store,
                env,
                runtime_root,
                device_id,
                db,
                DeviceProjectMode::Incremental { checkpoint: cp },
            ) {
                Err(ProjectionFeedError::Projection(ProjectionError::SchemaInconsistent {
                    detail,
                })) if detail == CHECKPOINT_REBUILD_SIGNAL => run_device_projection(
                    store,
                    env,
                    runtime_root,
                    device_id,
                    db,
                    DeviceProjectMode::Rebuild {
                        reason: ProjectionRebuildReason::CheckpointUnknownOrAhead,
                    },
                ),
                other => other,
            }
        }
    }
}

const CHECKPOINT_REBUILD_SIGNAL: &str = "checkpoint unknown or ahead of verified packs";

enum DeviceProjectMode {
    Rebuild {
        reason: ProjectionRebuildReason,
    },
    Incremental {
        checkpoint: DeviceCheckpoint,
    },
    /// Project the full verified feed with idempotent inserts (no row clear).
    FullIdempotent,
}

fn run_device_projection<S: SecureStore>(
    store: &S,
    env: SecureStoreEnvironment,
    runtime_root: &Path,
    device_id: &DeviceId,
    db: &ProjectionDb,
    mode: DeviceProjectMode,
) -> ProjectionFeedResult<DeviceProjectionSync> {
    // Lock order: device canonical lock, then SQLite transaction (never reverse).
    let device_lock = acquire_device_canonical_lock(runtime_root, device_id)?;

    let conn = db.connection();
    let tx = conn
        .unchecked_transaction()
        .map_err(ProjectionError::from)?;

    let (rebuild_reason, start_checkpoint, force_full) = match &mode {
        DeviceProjectMode::Rebuild { reason } => {
            clear_device_projection(&tx, device_id.as_str())?;
            (Some(reason.clone()), None, true)
        }
        DeviceProjectMode::Incremental { checkpoint } => (None, Some(checkpoint.clone()), false),
        DeviceProjectMode::FullIdempotent => (None, None, true),
    };

    let mut projected = 0u64;
    let mut tip_seq: Option<String> = start_checkpoint
        .as_ref()
        .and_then(|c| c.last_device_seq.clone());
    let mut tip_hash: Option<String> = start_checkpoint
        .as_ref()
        .and_then(|c| c.last_content_hash.clone());
    // Empty tip (both null) means a prior successful catch-up on a device with no
    // verified events yet. Treat it as already at the start of the feed so a
    // second catch-up does not spuriously rebuild, and the first later append
    // projects incrementally.
    let empty_checkpoint_tip = start_checkpoint
        .as_ref()
        .is_some_and(|c| c.last_device_seq.is_none() && c.last_content_hash.is_none());
    let mut past_checkpoint = force_full || start_checkpoint.is_none() || empty_checkpoint_tip;
    let mut found_checkpoint = force_full || start_checkpoint.is_none() || empty_checkpoint_tip;
    let mut checkpoint_invalid = false;
    let mut projection_err: Option<ProjectionError> = None;

    let expected_seq = start_checkpoint
        .as_ref()
        .and_then(|c| c.last_device_seq.as_deref())
        .and_then(|s| s.parse::<u64>().ok());
    let expected_hash = start_checkpoint
        .as_ref()
        .and_then(|c| c.last_content_hash.clone());

    let feed_result = for_each_verified_event_with_lock(
        store,
        env,
        runtime_root,
        device_id,
        &device_lock,
        |event| {
            if !past_checkpoint {
                let seq = event.device_seq.to_u64().map_err(|err| {
                    CanonicalWriterError::Invalid(format!("device_seq parse failed: {err}"))
                })?;
                let hash = event.content_hash.as_str();
                let seq_matches = expected_seq == Some(seq);
                let hash_matches = expected_hash.as_deref() == Some(hash);
                if seq_matches && hash_matches {
                    found_checkpoint = true;
                    past_checkpoint = true;
                    tip_seq = Some(event.device_seq.as_str().to_string());
                    tip_hash = Some(hash.to_string());
                    return Ok(());
                }
                if seq_matches && !hash_matches {
                    checkpoint_invalid = true;
                    return Err(CanonicalWriterError::Invalid(
                        "projection checkpoint invalid".into(),
                    ));
                }
                if expected_seq.is_some_and(|exp| seq > exp) {
                    checkpoint_invalid = true;
                    return Err(CanonicalWriterError::Invalid(
                        "projection checkpoint invalid".into(),
                    ));
                }
                // Still before the checkpoint tip: already projected; skip.
                return Ok(());
            }

            match project_memory_note(&tx, event) {
                Ok(()) => {
                    projected = projected.saturating_add(1);
                    tip_seq = Some(event.device_seq.as_str().to_string());
                    tip_hash = Some(event.content_hash.as_str().to_string());
                    Ok(())
                }
                Err(err) => {
                    projection_err = Some(err);
                    Err(CanonicalWriterError::Invalid(
                        "projection insert failed".into(),
                    ))
                }
            }
        },
    );

    if let Some(err) = projection_err.take() {
        drop(tx);
        return Err(ProjectionFeedError::Projection(err));
    }

    if checkpoint_invalid || (start_checkpoint.is_some() && !found_checkpoint) {
        drop(tx);
        return Err(ProjectionFeedError::Projection(
            ProjectionError::SchemaInconsistent {
                detail: CHECKPOINT_REBUILD_SIGNAL.into(),
            },
        ));
    }

    match feed_result {
        Ok(()) => {
            let (last_sealed_index, open_verified_prefix_len) =
                pack_cursor(runtime_root, device_id).map_err(ProjectionFeedError::Canonical)?;
            let checkpoint = DeviceCheckpoint {
                device_id: device_id.as_str().to_string(),
                last_device_seq: tip_seq,
                last_content_hash: tip_hash,
                last_sealed_index,
                open_verified_prefix_len,
            };
            upsert_device_checkpoint(&tx, &checkpoint)?;
            tx.commit().map_err(ProjectionError::from)?;
            Ok(DeviceProjectionSync {
                projected_events: projected,
                rebuilt: rebuild_reason.is_some(),
                rebuild_reason,
            })
        }
        Err(err) => {
            drop(tx);
            Err(ProjectionFeedError::Canonical(err))
        }
    }
}

/// Best-effort pack cursor after a successful verified feed (sealed max index + open len).
fn pack_cursor(
    runtime_root: &Path,
    device_id: &DeviceId,
) -> Result<(Option<i64>, Option<i64>), CanonicalWriterError> {
    let dir = canonical_events_device_dir(runtime_root, device_id);
    if !dir.exists() {
        return Ok((None, None));
    }
    let mut last_sealed: Option<u64> = None;
    let mut open_idx: Option<u64> = None;
    for entry in fs::read_dir(&dir).map_err(|e| CanonicalWriterError::Path(e.to_string()))? {
        let entry = entry.map_err(|e| CanonicalWriterError::Path(e.to_string()))?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if let Some(rest) = name.strip_suffix(".seg")
            && let Ok(idx) = rest.parse::<u64>()
        {
            last_sealed = Some(last_sealed.map_or(idx, |cur| cur.max(idx)));
        } else if let Some(rest) = name.strip_suffix(".open")
            && let Ok(idx) = rest.parse::<u64>()
        {
            open_idx = Some(idx);
        }
    }
    let open_len = if let Some(idx) = open_idx {
        let path = active_segment_path(runtime_root, device_id, idx);
        if path.exists() {
            let len = fs::metadata(&path)
                .map_err(|e| CanonicalWriterError::Path(e.to_string()))?
                .len();
            Some(len as i64)
        } else {
            None
        }
    } else {
        None
    };
    Ok((last_sealed.map(|v| v as i64), open_len))
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::canonical_writer::{
        AppendMemoryNoteArgs, CanonicalWriterConfig, append_memory_note, load_verified_events,
    };
    use crate::secure_store::FakeSecureStore;
    use crate::test_temp::exclusive_temp_dir_in;
    use cryptex_events::Classification;
    use cryptex_projection::{
        ProjectionOpenKind, ProjectionReplaceReason, SearchQuery, projection_sqlite_path,
        read_device_checkpoint, remove_projection_files, search_events,
    };
    use cryptex_storage::layout::{active_segment_path, quarantine_events_root};
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;

    fn temp_root(label: &str) -> PathBuf {
        let base = {
            #[cfg(windows)]
            {
                let local = std::env::var_os("LOCALAPPDATA").expect("LOCALAPPDATA");
                PathBuf::from(local)
                    .join("Cryptex-Dev")
                    .join("p0-2")
                    .join("m3-tests")
            }
            #[cfg(target_os = "macos")]
            {
                let home = std::env::var_os("HOME").expect("HOME");
                PathBuf::from(home)
                    .join("Library")
                    .join("Application Support")
                    .join("Cryptex-Dev")
                    .join("p0-2")
                    .join("m3-tests")
            }
            #[cfg(not(any(windows, target_os = "macos")))]
            {
                std::env::temp_dir()
                    .join("Cryptex-Dev")
                    .join("p0-2")
                    .join("m3-tests")
            }
        };
        exclusive_temp_dir_in(&base, &format!("m3-proj-{label}"))
    }

    fn append_note(
        store: &FakeSecureStore,
        root: &Path,
        device: &DeviceId,
        brain: &str,
        text: &str,
        seal_threshold: u64,
    ) -> crate::canonical_writer::AppendOutcome {
        append_memory_note(
            store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: root,
                brain_id: brain,
                device_id: device,
                classification: Classification::Internal,
                text,
                config: &CanonicalWriterConfig {
                    seal_event_threshold: seal_threshold,
                },
            },
        )
        .unwrap()
    }

    fn event_count(db: &ProjectionDb) -> i64 {
        db.connection()
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .unwrap()
    }

    fn snapshot_canonical_packs(root: &Path, device: &DeviceId) -> Vec<(PathBuf, u64, [u8; 32])> {
        let dir = canonical_events_device_dir(root, device);
        let mut out = Vec::new();
        if !dir.exists() {
            return out;
        }
        for entry in fs::read_dir(&dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let name = path.file_name().unwrap().to_string_lossy();
            if !(name.ends_with(".seg") || name.ends_with(".open")) {
                continue;
            }
            let bytes = fs::read(&path).unwrap();
            let mut hash = [0u8; 32];
            // cheap stable fingerprint without extra deps: XOR-fold
            for (i, b) in bytes.iter().enumerate() {
                hash[i % 32] ^= *b;
                hash[(i + 7) % 32] = hash[(i + 7) % 32].wrapping_add(*b);
            }
            out.push((path, bytes.len() as u64, hash));
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    #[test]
    fn projects_verified_memory_notes_from_canonical_writer() {
        let root = temp_root("feed");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("proj-device-a").unwrap();
        let brain = "55555555-5555-4555-8555-555555555555";
        append_note(&store, &root, &device, brain, "alpha searchable", 1000);
        append_note(&store, &root, &device, brain, "beta other", 1000);

        let db = ProjectionDb::open_or_create(&root).expect("projection db");
        let n = project_verified_device_events(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            &device,
            &db,
        )
        .expect("project");
        assert_eq!(n, 2);
        assert_eq!(event_count(&db), 2);
        let cp = read_device_checkpoint(db.connection(), device.as_str())
            .unwrap()
            .expect("checkpoint");
        assert_eq!(cp.last_device_seq.as_deref(), Some("2"));
        drop(db);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn clean_rebuild_projects_all_verified_events_and_checkpoint() {
        let root = temp_root("rebuild-clean");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("proj-rebuild").unwrap();
        let brain = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
        append_note(&store, &root, &device, brain, "one", 1000);
        append_note(&store, &root, &device, brain, "two", 1000);
        append_note(&store, &root, &device, brain, "three", 1000);
        let before = snapshot_canonical_packs(&root, &device);

        let (db, kind) = open_projection_db(&root).expect("open");
        assert!(matches!(
            kind,
            ProjectionOpenKind::Created | ProjectionOpenKind::OpenedExisting
        ));
        let sync =
            rebuild_device_projection(&store, SecureStoreEnvironment::Dev, &root, &device, &db)
                .expect("rebuild");
        assert!(sync.rebuilt);
        assert_eq!(sync.projected_events, 3);
        assert_eq!(event_count(&db), 3);
        let cp = read_device_checkpoint(db.connection(), device.as_str())
            .unwrap()
            .expect("cp");
        assert_eq!(cp.last_device_seq.as_deref(), Some("3"));
        assert_eq!(before, snapshot_canonical_packs(&root, &device));
        drop(db);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn restart_catch_up_projects_only_new_events_without_duplicates() {
        let root = temp_root("catchup");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("proj-catchup").unwrap();
        let brain = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
        append_note(&store, &root, &device, brain, "first", 1000);
        append_note(&store, &root, &device, brain, "second", 1000);

        let (db, _) = open_projection_db(&root).unwrap();
        let first =
            catch_up_device_projection(&store, SecureStoreEnvironment::Dev, &root, &device, &db)
                .unwrap();
        assert_eq!(first.projected_events, 2);
        assert!(first.rebuilt);
        drop(db);

        append_note(&store, &root, &device, brain, "third", 1000);
        let (db, kind) = open_projection_db(&root).unwrap();
        assert!(matches!(kind, ProjectionOpenKind::OpenedExisting));
        let second =
            catch_up_device_projection(&store, SecureStoreEnvironment::Dev, &root, &device, &db)
                .unwrap();
        assert!(!second.rebuilt);
        assert_eq!(second.projected_events, 1);
        assert_eq!(event_count(&db), 3);
        let ids: i64 = db
            .connection()
            .query_row("SELECT COUNT(DISTINCT event_id) FROM events", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(ids, 3);
        drop(db);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_db_rebuild_restores_rows_and_leaves_canonical_unchanged() {
        let root = temp_root("missing-db");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("proj-missing").unwrap();
        let brain = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";
        append_note(&store, &root, &device, brain, "keep-me", 1000);
        let (db, _) = open_projection_db(&root).unwrap();
        catch_up_device_projection(&store, SecureStoreEnvironment::Dev, &root, &device, &db)
            .unwrap();
        assert_eq!(event_count(&db), 1);
        drop(db);

        let before = snapshot_canonical_packs(&root, &device);
        remove_projection_files(&root).unwrap();
        assert!(!projection_sqlite_path(&root).exists());

        let (db, kind) = open_projection_db(&root).unwrap();
        assert!(matches!(kind, ProjectionOpenKind::Created));
        let sync =
            catch_up_device_projection(&store, SecureStoreEnvironment::Dev, &root, &device, &db)
                .unwrap();
        assert_eq!(sync.projected_events, 1);
        assert_eq!(event_count(&db), 1);
        assert_eq!(before, snapshot_canonical_packs(&root, &device));
        drop(db);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn corrupt_db_rebuild_restores_rows_and_leaves_canonical_unchanged() {
        let root = temp_root("corrupt-db");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("proj-corrupt").unwrap();
        let brain = "dddddddd-dddd-4ddd-8ddd-dddddddddddd";
        append_note(&store, &root, &device, brain, "recoverable", 1000);
        let (db, _) = open_projection_db(&root).unwrap();
        catch_up_device_projection(&store, SecureStoreEnvironment::Dev, &root, &device, &db)
            .unwrap();
        drop(db);

        let before = snapshot_canonical_packs(&root, &device);
        let path = projection_sqlite_path(&root);
        fs::write(&path, b"garbage-not-sqlite").unwrap();

        let (db, kind) = open_projection_db(&root).unwrap();
        match kind {
            ProjectionOpenKind::Replaced {
                reason: ProjectionReplaceReason::Corrupt { .. },
            } => {}
            other => panic!("expected corrupt replace, got {other:?}"),
        }
        let sync =
            catch_up_device_projection(&store, SecureStoreEnvironment::Dev, &root, &device, &db)
                .unwrap();
        assert_eq!(sync.projected_events, 1);
        assert_eq!(event_count(&db), 1);
        assert_eq!(before, snapshot_canonical_packs(&root, &device));
        drop(db);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn unsupported_schema_replaces_disposable_db_then_catch_up() {
        let root = temp_root("unsup-schema");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("proj-unsup").unwrap();
        let brain = "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee";
        append_note(&store, &root, &device, brain, "schema", 1000);
        let db = ProjectionDb::open_or_create(&root).unwrap();
        db.connection()
            .execute(
                "UPDATE projection_meta SET schema_version = 77 WHERE id = 1",
                [],
            )
            .unwrap();
        drop(db);
        let before = snapshot_canonical_packs(&root, &device);
        let (db, kind) = open_projection_db(&root).unwrap();
        match kind {
            ProjectionOpenKind::Replaced {
                reason: ProjectionReplaceReason::UnsupportedSchema { found: 77 },
            } => {}
            other => panic!("unexpected {other:?}"),
        }
        catch_up_device_projection(&store, SecureStoreEnvironment::Dev, &root, &device, &db)
            .unwrap();
        assert_eq!(event_count(&db), 1);
        assert_eq!(before, snapshot_canonical_packs(&root, &device));
        drop(db);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn checkpoint_unknown_hash_triggers_rebuild_without_duplicates() {
        let root = temp_root("ckpt-hash");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("proj-ckhash").unwrap();
        let brain = "ffffffff-ffff-4fff-8fff-ffffffffffff";
        append_note(&store, &root, &device, brain, "a", 1000);
        append_note(&store, &root, &device, brain, "b", 1000);
        let (db, _) = open_projection_db(&root).unwrap();
        catch_up_device_projection(&store, SecureStoreEnvironment::Dev, &root, &device, &db)
            .unwrap();
        // Poison checkpoint hash while keeping seq=2.
        db.connection()
            .execute(
                "UPDATE device_checkpoint SET last_content_hash = ?1 WHERE device_id = ?2",
                ("aa".repeat(32), device.as_str()),
            )
            .unwrap();
        let sync =
            catch_up_device_projection(&store, SecureStoreEnvironment::Dev, &root, &device, &db)
                .unwrap();
        assert!(sync.rebuilt);
        assert_eq!(
            sync.rebuild_reason,
            Some(ProjectionRebuildReason::CheckpointUnknownOrAhead)
        );
        assert_eq!(event_count(&db), 2);
        drop(db);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn checkpoint_ahead_of_packs_triggers_rebuild() {
        let root = temp_root("ckpt-ahead");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("proj-ckahead").unwrap();
        let brain = "12121212-1212-4121-8121-121212121212";
        append_note(&store, &root, &device, brain, "only", 1000);
        let (db, _) = open_projection_db(&root).unwrap();
        catch_up_device_projection(&store, SecureStoreEnvironment::Dev, &root, &device, &db)
            .unwrap();
        db.connection()
            .execute(
                "UPDATE device_checkpoint
                 SET last_device_seq = '99', last_content_hash = ?1
                 WHERE device_id = ?2",
                ("bb".repeat(32), device.as_str()),
            )
            .unwrap();
        let sync =
            catch_up_device_projection(&store, SecureStoreEnvironment::Dev, &root, &device, &db)
                .unwrap();
        assert!(sync.rebuilt);
        assert_eq!(event_count(&db), 1);
        drop(db);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn interrupted_transaction_rolls_back_and_retry_catch_up_succeeds() {
        let root = temp_root("interrupted");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("proj-int").unwrap();
        let brain = "34343434-3434-4343-8343-343434343434";
        let first = append_note(&store, &root, &device, brain, "durable", 1000);
        let (db, _) = open_projection_db(&root).unwrap();
        {
            let tx = db.connection().unchecked_transaction().unwrap();
            // Simulate crash mid-projection: insert without checkpoint / commit.
            let verified =
                load_verified_events(&store, SecureStoreEnvironment::Dev, &root, &device).unwrap();
            project_memory_note(&tx, &verified[0]).unwrap();
            drop(tx); // rollback
        }
        assert_eq!(event_count(&db), 0);
        let sync =
            catch_up_device_projection(&store, SecureStoreEnvironment::Dev, &root, &device, &db)
                .unwrap();
        assert_eq!(sync.projected_events, 1);
        assert_eq!(event_count(&db), 1);
        let stored: String = db
            .connection()
            .query_row("SELECT event_id FROM events", [], |row| row.get(0))
            .unwrap();
        assert_eq!(stored, first.accepted.event_id);
        drop(db);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn projection_conflict_is_surfaced_as_projection_error_and_rolls_back() {
        let root = temp_root("conflict");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("proj-device-c").unwrap();
        let brain = "77777777-7777-4777-8777-777777777777";
        let cfg = CanonicalWriterConfig {
            seal_event_threshold: 1000,
        };
        let first = append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Internal,
                text: "stable-text",
                config: &cfg,
            },
        )
        .unwrap();
        let second = append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Public,
                text: "second-note",
                config: &cfg,
            },
        )
        .unwrap();

        let db = ProjectionDb::open_or_create(&root).expect("projection db");
        let bad_hash = "aa".repeat(32);
        db.connection()
            .execute(
                "INSERT INTO events (
                    event_id, brain_id, device_id, device_seq, created_at,
                    event_type, classification, content_hash, signing_key_id, text
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                (
                    second.accepted.event_id.as_str(),
                    second.accepted.brain_id.as_str(),
                    second.accepted.device_id.as_str(),
                    second.accepted.device_seq.as_str(),
                    second.accepted.created_at.as_str(),
                    "memory.note",
                    "public",
                    bad_hash.as_str(),
                    second.accepted.signing_key_id.as_str(),
                    "second-note",
                ),
            )
            .unwrap();

        let err = project_verified_device_events(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            &device,
            &db,
        )
        .expect_err("conflicting replay must fail");
        let msg = err.to_string();
        match &err {
            ProjectionFeedError::Projection(ProjectionError::EventConflict {
                event_id,
                differing_fields,
            }) => {
                assert_eq!(event_id, second.accepted.event_id.as_str());
                assert!(differing_fields.contains(&"content_hash"));
                assert!(!msg.contains("second-note"));
                assert!(!msg.contains(first.accepted.content_hash.as_str()));
            }
            other => panic!("expected Projection EventConflict, got {other}"),
        }

        let first_present: i64 = db
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM events WHERE event_id = ?1",
                [first.accepted.event_id.as_str()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            first_present, 0,
            "successful insert earlier in the failed txn must roll back"
        );
        let stored_hash: String = db
            .connection()
            .query_row(
                "SELECT content_hash FROM events WHERE event_id = ?1",
                [second.accepted.event_id.as_str()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            stored_hash, bad_hash,
            "pre-seeded conflicting row must stay"
        );
        drop(db);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn corrupt_sealed_pack_fails_closed_and_does_not_commit_rows() {
        let root = temp_root("corrupt");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("proj-device-b").unwrap();
        let brain = "66666666-6666-4666-8666-666666666666";
        let cfg = CanonicalWriterConfig {
            seal_event_threshold: 1,
        };
        append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Internal,
                text: "sealed-one",
                config: &cfg,
            },
        )
        .unwrap();

        let sealed = cryptex_storage::sealed_segment_path(&root, &device, 0);
        assert!(sealed.exists(), "expected sealed pack");
        {
            let mut file = fs::OpenOptions::new().write(true).open(&sealed).unwrap();
            file.write_all(&[0u8; 32]).unwrap();
        }

        let db = ProjectionDb::open_or_create(&root).expect("projection db");
        let err = project_verified_device_events(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            &device,
            &db,
        )
        .expect_err("corrupt sealed must fail");
        assert!(
            matches!(err, ProjectionFeedError::Canonical(_)),
            "unexpected error: {err}"
        );
        let count: i64 = db
            .connection()
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0, "failed feed must not commit projected rows");
        assert!(
            quarantine_events_root(&root).exists(),
            "M2 quarantine path should exist after sealed corruption"
        );
        drop(db);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn sealed_and_open_events_project_truncated_open_tail_excluded() {
        let root = temp_root("sealed-open");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("proj-seal-open").unwrap();
        let brain = "77777777-7777-4777-8777-777777777777";

        // Seal the first event into *.seg.
        append_note(&store, &root, &device, brain, "sealed_alpha unique", 1);
        // Keep subsequent events in the active *.open pack.
        append_note(&store, &root, &device, brain, "open_beta unique", 1000);
        append_note(&store, &root, &device, brain, "open_gamma unique", 1000);

        let sealed = cryptex_storage::sealed_segment_path(&root, &device, 0);
        let open = active_segment_path(&root, &device, 1);
        assert!(sealed.exists(), "expected sealed pack for first event");
        assert!(open.exists(), "expected active open pack for later events");

        // Trailing garbage must be recovered away and must not become projected rows.
        {
            let mut file = fs::OpenOptions::new().append(true).open(&open).unwrap();
            file.write_all(b"NOT-A-FRAME-TAIL-GARBAGE").unwrap();
        }

        let before = snapshot_canonical_packs(&root, &device);
        let db = ProjectionDb::open_or_create(&root).expect("projection db");
        catch_up_device_projection(&store, SecureStoreEnvironment::Dev, &root, &device, &db)
            .expect("project sealed+open");

        assert_eq!(event_count(&db), 3);
        let hits = search_events(db.connection(), &SearchQuery::default()).unwrap();
        let texts: Vec<_> = hits.iter().map(|h| h.text.as_str()).collect();
        assert!(texts.iter().any(|t| t.contains("sealed_alpha")));
        assert!(texts.iter().any(|t| t.contains("open_beta")));
        assert!(texts.iter().any(|t| t.contains("open_gamma")));
        assert_eq!(
            hits.len(),
            3,
            "truncated open tail must not invent extra projected rows"
        );

        let fts = search_events(
            db.connection(),
            &SearchQuery {
                text: Some("open_beta".into()),
                ..SearchQuery::default()
            },
        )
        .unwrap();
        assert_eq!(fts.len(), 1);
        assert!(fts[0].text.contains("open_beta"));

        // Verified sealed bytes must be unchanged; open may shrink after recovery truncate.
        let after = snapshot_canonical_packs(&root, &device);
        let sealed_before: Vec<_> = before
            .iter()
            .filter(|(p, _, _)| p.extension().and_then(|e| e.to_str()) == Some("seg"))
            .collect();
        let sealed_after: Vec<_> = after
            .iter()
            .filter(|(p, _, _)| p.extension().and_then(|e| e.to_str()) == Some("seg"))
            .collect();
        assert_eq!(
            sealed_before, sealed_after,
            "sealed packs must remain unchanged by projection/search"
        );
        drop(db);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn multi_device_union_search_filters_and_leaves_canonical_unchanged() {
        let root = temp_root("multi-device");
        let store = FakeSecureStore::new();
        let device_a = DeviceId::new("proj-md-a").unwrap();
        let device_b = DeviceId::new("proj-md-b").unwrap();
        let brain = "88888888-8888-4888-8888-888888888888";

        append_note(
            &store,
            &root,
            &device_a,
            brain,
            "alpha_from_device_a token",
            1000,
        );
        append_note(
            &store,
            &root,
            &device_b,
            brain,
            "beta_from_device_b token",
            1000,
        );

        let before_a = snapshot_canonical_packs(&root, &device_a);
        let before_b = snapshot_canonical_packs(&root, &device_b);

        let db = ProjectionDb::open_or_create(&root).expect("projection db");
        catch_up_device_projection(&store, SecureStoreEnvironment::Dev, &root, &device_a, &db)
            .expect("catch up a");
        catch_up_device_projection(&store, SecureStoreEnvironment::Dev, &root, &device_b, &db)
            .expect("catch up b");

        assert_eq!(event_count(&db), 2);
        let union = search_events(db.connection(), &SearchQuery::default()).unwrap();
        assert_eq!(union.len(), 2);

        let only_a = search_events(
            db.connection(),
            &SearchQuery {
                device_id: Some(device_a.as_str().into()),
                ..SearchQuery::default()
            },
        )
        .unwrap();
        assert_eq!(only_a.len(), 1);
        assert!(only_a[0].text.contains("alpha_from_device_a"));
        assert_eq!(only_a[0].device_id, device_a.as_str());

        let only_b = search_events(
            db.connection(),
            &SearchQuery {
                device_id: Some(device_b.as_str().into()),
                text: Some("beta_from_device_b".into()),
                ..SearchQuery::default()
            },
        )
        .unwrap();
        assert_eq!(only_b.len(), 1);
        assert_eq!(only_b[0].device_id, device_b.as_str());

        assert_eq!(
            before_a,
            snapshot_canonical_packs(&root, &device_a),
            "device-a canonical packs must be unchanged"
        );
        assert_eq!(
            before_b,
            snapshot_canonical_packs(&root, &device_b),
            "device-b canonical packs must be unchanged"
        );
        drop(db);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn idempotent_double_rebuild_stable_counts_and_canonical_unchanged() {
        let root = temp_root("rebuild-idempotent");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("proj-rebuild-2x").unwrap();
        let brain = "99999999-9999-4999-8999-999999999999";
        append_note(&store, &root, &device, brain, "rebuild-once", 1000);
        append_note(&store, &root, &device, brain, "rebuild-twice", 1000);

        let before = snapshot_canonical_packs(&root, &device);
        let db = ProjectionDb::open_or_create(&root).expect("projection db");
        rebuild_device_projection(&store, SecureStoreEnvironment::Dev, &root, &device, &db)
            .expect("first rebuild");
        let count_after_first = event_count(&db);
        assert_eq!(count_after_first, 2);

        rebuild_device_projection(&store, SecureStoreEnvironment::Dev, &root, &device, &db)
            .expect("second rebuild");
        assert_eq!(event_count(&db), count_after_first);
        let hits = search_events(db.connection(), &SearchQuery::default()).unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(
            before,
            snapshot_canonical_packs(&root, &device),
            "canonical packs must remain unchanged across idempotent rebuilds"
        );
        drop(db);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn empty_device_catch_up_twice_does_not_rebuild_again() {
        let root = temp_root("empty-ckpt");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("proj-empty-a").unwrap();
        let db = ProjectionDb::open_or_create(&root).expect("projection db");

        let first =
            catch_up_device_projection(&store, SecureStoreEnvironment::Dev, &root, &device, &db)
                .expect("first empty catch-up");
        assert_eq!(first.projected_events, 0);
        assert!(first.rebuilt, "first catch-up without checkpoint rebuilds");
        let cp = read_device_checkpoint(db.connection(), device.as_str())
            .unwrap()
            .expect("empty checkpoint written");
        assert!(cp.last_device_seq.is_none());
        assert!(cp.last_content_hash.is_none());

        let second =
            catch_up_device_projection(&store, SecureStoreEnvironment::Dev, &root, &device, &db)
                .expect("second empty catch-up");
        assert_eq!(second.projected_events, 0);
        assert!(
            !second.rebuilt,
            "valid empty checkpoint must not trigger another rebuild"
        );
        drop(db);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn empty_checkpoint_then_first_append_projects_without_full_rebuild() {
        let root = temp_root("empty-then-append");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("proj-empty-b").unwrap();
        let brain = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
        let db = ProjectionDb::open_or_create(&root).expect("projection db");

        let empty =
            catch_up_device_projection(&store, SecureStoreEnvironment::Dev, &root, &device, &db)
                .expect("empty catch-up");
        assert!(empty.rebuilt);
        assert_eq!(empty.projected_events, 0);

        append_note(&store, &root, &device, brain, "first_after_empty", 1000);
        let before = snapshot_canonical_packs(&root, &device);

        let sync =
            catch_up_device_projection(&store, SecureStoreEnvironment::Dev, &root, &device, &db)
                .expect("catch-up after first append");
        assert_eq!(sync.projected_events, 1);
        assert!(
            !sync.rebuilt,
            "first append after empty checkpoint must be incremental"
        );
        assert_eq!(event_count(&db), 1);
        let hits = search_events(
            db.connection(),
            &SearchQuery {
                text: Some("first_after_empty".into()),
                ..SearchQuery::default()
            },
        )
        .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(
            before,
            snapshot_canonical_packs(&root, &device),
            "canonical packs unchanged"
        );
        drop(db);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn fts_index_corruption_triggers_replace_rebuild_search_and_leaves_canonical() {
        let root = temp_root("fts-corrupt");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("proj-fts-c").unwrap();
        let brain = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
        append_note(&store, &root, &device, brain, "fts_corrupt_token", 1000);
        let before = snapshot_canonical_packs(&root, &device);

        let db = ProjectionDb::open_or_create(&root).expect("projection db");
        catch_up_device_projection(&store, SecureStoreEnvironment::Dev, &root, &device, &db)
            .expect("initial project");
        assert_eq!(event_count(&db), 1);
        db.connection()
            .execute(
                "INSERT INTO events_fts(events_fts) VALUES('delete-all')",
                [],
            )
            .unwrap();
        drop(db);

        let (db, kind) = open_projection_db(&root).expect("open replacing");
        assert!(
            matches!(
                kind,
                ProjectionOpenKind::Replaced {
                    reason: ProjectionReplaceReason::Corrupt { .. }
                }
            ),
            "inconsistent FTS must replace disposable DB, got {kind:?}"
        );
        let sync =
            catch_up_device_projection(&store, SecureStoreEnvironment::Dev, &root, &device, &db)
                .expect("rebuild after FTS replace");
        assert!(sync.rebuilt);
        assert_eq!(sync.projected_events, 1);
        let hits = search_events(
            db.connection(),
            &SearchQuery {
                text: Some("fts_corrupt_token".into()),
                ..SearchQuery::default()
            },
        )
        .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(
            before,
            snapshot_canonical_packs(&root, &device),
            "canonical packs must remain unchanged"
        );
        drop(db);
        let _ = fs::remove_dir_all(&root);
    }
}
