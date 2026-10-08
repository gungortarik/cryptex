//! cryptexd-owned projection of verified canonical events (M3 Slice 4).
//!
//! Reads only through [`crate::canonical_writer::for_each_verified_event`].
//! Never parses `.open`/`.seg` outside that path and never mutates canonical packs
//! for projection purposes.

use std::path::Path;

use cryptex_core::ids::DeviceId;
use cryptex_projection::{ProjectionDb, ProjectionError, project_memory_note};

use crate::canonical_writer::{CanonicalWriterError, for_each_verified_event};
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

/// Project one device's verified `memory.note` events into `db` in a single transaction.
///
/// Canonical packs remain read-only aside from M2 recover/quarantine performed by
/// the verified reader. Checkpoint advancement is deferred to a later slice.
pub fn project_verified_device_events<S: SecureStore>(
    store: &S,
    env: SecureStoreEnvironment,
    runtime_root: &Path,
    device_id: &DeviceId,
    db: &ProjectionDb,
) -> ProjectionFeedResult<u64> {
    let conn = db.connection();
    let tx = conn
        .unchecked_transaction()
        .map_err(ProjectionError::from)?;
    let mut projected = 0u64;
    // Stash typed projection failures so they are not re-labeled as canonical.
    let mut projection_err: Option<ProjectionError> = None;
    let feed_result = for_each_verified_event(store, env, runtime_root, device_id, |event| {
        match project_memory_note(&tx, event) {
            Ok(()) => {
                projected = projected.saturating_add(1);
                Ok(())
            }
            Err(err) => {
                projection_err = Some(err);
                Err(CanonicalWriterError::Invalid(
                    "projection insert failed".into(),
                ))
            }
        }
    });
    match (feed_result, projection_err.take()) {
        (Ok(()), None) => {
            tx.commit().map_err(ProjectionError::from)?;
            Ok(projected)
        }
        (_, Some(err)) => {
            // Dropping `tx` without commit rolls back projected rows.
            drop(tx);
            Err(ProjectionFeedError::Projection(err))
        }
        (Err(err), None) => {
            drop(tx);
            Err(ProjectionFeedError::Canonical(err))
        }
    }
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
    use cryptex_storage::layout::quarantine_events_root;
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
                    .join("m3-projection-feed-tests")
            }
            #[cfg(target_os = "macos")]
            {
                let home = std::env::var_os("HOME").expect("HOME");
                PathBuf::from(home)
                    .join("Library")
                    .join("Application Support")
                    .join("Cryptex-Dev")
                    .join("p0-2")
                    .join("m3-projection-feed-tests")
            }
            #[cfg(not(any(windows, target_os = "macos")))]
            {
                std::env::temp_dir()
                    .join("Cryptex-Dev")
                    .join("p0-2")
                    .join("m3-projection-feed-tests")
            }
        };
        exclusive_temp_dir_in(&base, &format!("m3-feed-{label}"))
    }

    #[test]
    fn projects_verified_memory_notes_from_canonical_writer() {
        let root = temp_root("ok");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("proj-device-a").unwrap();
        let brain = "55555555-5555-4555-8555-555555555555";
        let cfg = CanonicalWriterConfig {
            seal_event_threshold: 1000,
        };
        append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Internal,
                text: "alpha note",
                config: &cfg,
            },
        )
        .unwrap();
        append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Public,
                text: "beta searchable",
                config: &cfg,
            },
        )
        .unwrap();

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

        let verified =
            load_verified_events(&store, SecureStoreEnvironment::Dev, &root, &device).unwrap();
        assert_eq!(verified.len(), 2);

        let count: i64 = db
            .connection()
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2);
        let hit: i64 = db
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM events_fts WHERE events_fts MATCH 'searchable'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(hit, 1);
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
        // Pre-seed a conflicting row for the *second* event so the feed inserts
        // the first event in-txn, then fails on reconcile — proving rollback.
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

        // Corrupt the sealed pack bytes after a successful seal.
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
}
