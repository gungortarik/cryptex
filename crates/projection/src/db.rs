//! Projection database open/create and disposable replace (M3 Slice 3/5).
//!
//! Opens disposable SQLite under the derived projection path. Does not ingest
//! canonical events. Slice 5 may delete/replace this file when unusable; that
//! must never touch canonical packs.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use crate::layout::{derived_projection_dir, projection_sqlite_path};
use crate::schema::{
    PROJECTION_SCHEMA_VERSION, apply_schema_v1, read_schema_version, validate_schema_v1,
};
use crate::{ProjectionError, ProjectionResult};

/// Why an on-disk projection DB was deleted and recreated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionReplaceReason {
    /// File missing (first create is not a replace; see [`ProjectionOpenKind::Created`]).
    Corrupt {
        detail: String,
    },
    UnsupportedSchema {
        found: u32,
    },
    SchemaInconsistent {
        detail: String,
    },
}

/// Result of opening the disposable projection database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionOpenKind {
    /// Fresh schema v1 database created.
    Created,
    /// Existing healthy v1 database opened.
    OpenedExisting,
    /// Unusable disposable DB removed and replaced with empty v1.
    Replaced { reason: ProjectionReplaceReason },
}

/// Open handle to a disposable projection database.
#[derive(Debug)]
pub struct ProjectionDb {
    conn: Connection,
    path: PathBuf,
}

impl ProjectionDb {
    /// Create or open `<app-data-root>/derived/projection/projection.sqlite`.
    ///
    /// Fresh databases receive schema v1. Existing databases must already be v1
    /// or the open fails closed with [`ProjectionError::UnsupportedSchema`].
    /// Prefer [`Self::open_replacing_unusable`] for daemon catch-up/rebuild.
    pub fn open_or_create(app_data_root: &Path) -> ProjectionResult<Self> {
        let dir = derived_projection_dir(app_data_root);
        fs::create_dir_all(&dir)?;
        let path = projection_sqlite_path(app_data_root);
        let conn = Connection::open(&path)?;
        initialize_connection(&conn)?;
        Ok(Self { conn, path })
    }

    /// Open the projection DB, replacing corrupt/stale/unsupported disposable files.
    ///
    /// Never modifies canonical event packs. Sidecars (`-wal`/`-shm`/`-journal`) are
    /// removed together with the main DB file when replacing.
    pub fn open_replacing_unusable(
        app_data_root: &Path,
    ) -> ProjectionResult<(Self, ProjectionOpenKind)> {
        let dir = derived_projection_dir(app_data_root);
        fs::create_dir_all(&dir)?;
        let path = projection_sqlite_path(app_data_root);

        if !path.exists() {
            let db = Self::open_or_create(app_data_root)?;
            return Ok((db, ProjectionOpenKind::Created));
        }

        match try_open_existing(&path) {
            Ok(db) => Ok((db, ProjectionOpenKind::OpenedExisting)),
            Err(err) => {
                let reason = replace_reason_from_error(&err);
                remove_projection_files(app_data_root)?;
                let db = Self::open_or_create(app_data_root)?;
                Ok((db, ProjectionOpenKind::Replaced { reason }))
            }
        }
    }

    /// In-memory empty projection DB (tests / disposable probes).
    pub fn open_in_memory() -> ProjectionResult<Self> {
        let conn = Connection::open_in_memory()?;
        initialize_connection(&conn)?;
        Ok(Self {
            conn,
            path: PathBuf::from(":memory:"),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    pub fn schema_version(&self) -> ProjectionResult<u32> {
        match read_schema_version(&self.conn)? {
            Some(v) => Ok(v),
            None => Err(ProjectionError::SchemaInconsistent {
                detail: "projection_meta missing after open".into(),
            }),
        }
    }
}

/// Delete disposable projection DB files under `derived/projection/` (not canonical).
pub fn remove_projection_files(app_data_root: &Path) -> ProjectionResult<()> {
    let path = projection_sqlite_path(app_data_root);
    for candidate in [
        path.clone(),
        PathBuf::from(format!("{}-wal", path.display())),
        PathBuf::from(format!("{}-shm", path.display())),
        PathBuf::from(format!("{}-journal", path.display())),
    ] {
        match fs::remove_file(&candidate) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }
    }
    Ok(())
}

fn try_open_existing(path: &Path) -> ProjectionResult<ProjectionDb> {
    let conn = Connection::open(path).map_err(map_open_sqlite_error)?;
    let check: String = conn
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .map_err(map_open_sqlite_error)?;
    if check != "ok" {
        return Err(ProjectionError::CorruptDb {
            detail: format!("quick_check reported {check}"),
        });
    }
    initialize_connection(&conn)?;
    Ok(ProjectionDb {
        conn,
        path: path.to_path_buf(),
    })
}

fn map_open_sqlite_error(err: rusqlite::Error) -> ProjectionError {
    let message = err.to_string().to_ascii_lowercase();
    if message.contains("not a database")
        || message.contains("file is encrypted")
        || message.contains("malformed")
    {
        return ProjectionError::CorruptDb {
            detail: "sqlite open/integrity failed".into(),
        };
    }
    ProjectionError::from(err)
}

fn replace_reason_from_error(err: &ProjectionError) -> ProjectionReplaceReason {
    match err {
        ProjectionError::UnsupportedSchema { found, .. } => {
            ProjectionReplaceReason::UnsupportedSchema { found: *found }
        }
        ProjectionError::SchemaInconsistent { detail } => {
            ProjectionReplaceReason::SchemaInconsistent {
                detail: detail.clone(),
            }
        }
        ProjectionError::CorruptDb { detail } => ProjectionReplaceReason::Corrupt {
            detail: detail.clone(),
        },
        other => ProjectionReplaceReason::Corrupt {
            detail: other.to_string(),
        },
    }
}

fn initialize_connection(conn: &Connection) -> ProjectionResult<()> {
    match read_schema_version(conn)? {
        None => {
            apply_schema_v1(conn, &rfc3339_now())?;
            validate_schema_v1(conn)?;
            Ok(())
        }
        Some(found) if found == PROJECTION_SCHEMA_VERSION => {
            validate_schema_v1(conn)?;
            Ok(())
        }
        Some(found) => Err(ProjectionError::UnsupportedSchema {
            found,
            expected: PROJECTION_SCHEMA_VERSION,
        }),
    }
}

fn rfc3339_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_unix_secs_rfc3339_utc(secs)
}

/// Format Unix seconds as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
fn format_unix_secs_rfc3339_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let tod = (secs % 86_400) as u32;
    let hour = tod / 3_600;
    let minute = (tod % 3_600) / 60;
    let second = tod % 60;
    let (year, month, day) = civil_from_days(days + 719_468);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Howard Hinnant civil-from-days (days since 0000-03-01).
fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::projection_sqlite_path;
    use crate::schema::PROJECTION_SCHEMA_VERSION;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_ROOT_SEQ: AtomicU64 = AtomicU64::new(1);

    /// RAII exclusive fixture root under Cryptex-Dev (or temp) that removes itself.
    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            match fs::remove_dir_all(&self.path) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => {
                    let msg = format!(
                        "failed to clean projection test root {}: {err}",
                        self.path.display()
                    );
                    if std::thread::panicking() {
                        eprintln!("{msg}");
                    } else {
                        panic!("{msg}");
                    }
                }
            }
        }
    }

    /// Create a unique directory with exclusive `create_dir` (never pre-delete).
    ///
    /// Mirrors `cryptexd::test_temp`: `{prefix}-{pid}-{seq}` + retry on AlreadyExists.
    fn exclusive_temp_root(prefix: &str) -> TempRoot {
        let parent = {
            #[cfg(windows)]
            {
                let local = std::env::var_os("LOCALAPPDATA").expect("LOCALAPPDATA");
                PathBuf::from(local)
                    .join("Cryptex-Dev")
                    .join("m3-projection-tests")
            }
            #[cfg(target_os = "macos")]
            {
                let home = std::env::var_os("HOME").expect("HOME");
                PathBuf::from(home)
                    .join("Library")
                    .join("Application Support")
                    .join("Cryptex-Dev")
                    .join("m3-projection-tests")
            }
            #[cfg(not(any(windows, target_os = "macos")))]
            {
                std::env::temp_dir()
                    .join("Cryptex-Dev")
                    .join("m3-projection-tests")
            }
        };
        fs::create_dir_all(&parent).unwrap_or_else(|err| {
            panic!(
                "exclusive_temp_root: create_dir_all failed for {}: {err}",
                parent.display()
            )
        });
        for _ in 0..10_000 {
            let seq = TEMP_ROOT_SEQ.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!("{prefix}-{}-{seq}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return TempRoot { path },
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => panic!(
                    "exclusive_temp_root: create_dir failed for {}: {err}",
                    path.display()
                ),
            }
        }
        panic!("exclusive_temp_root: exhausted unique candidates for prefix {prefix}");
    }

    #[test]
    fn exclusive_temp_roots_are_distinct() {
        let a = exclusive_temp_root("distinct");
        let b = exclusive_temp_root("distinct");
        assert_ne!(a.path(), b.path());
        assert!(a.path().is_dir());
        assert!(b.path().is_dir());
    }

    #[test]
    fn fresh_create_writes_v1_schema_under_derived_projection() {
        let root = exclusive_temp_root("fresh");
        let db = ProjectionDb::open_or_create(root.path()).expect("create");
        assert_eq!(db.schema_version().unwrap(), PROJECTION_SCHEMA_VERSION);
        assert!(
            db.path().ends_with("derived/projection/projection.sqlite")
                || db.path().ends_with(r"derived\projection\projection.sqlite")
        );
        assert!(db.path().starts_with(root.path()));

        let count: i64 = db
            .connection()
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
        let checkpoints: i64 = db
            .connection()
            .query_row("SELECT COUNT(*) FROM device_checkpoint", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(checkpoints, 0);
        // Close DB before TempRoot cleanup removes the directory (Windows file locks).
        drop(db);
    }

    #[test]
    fn repeated_open_is_idempotent_for_v1() {
        let root = exclusive_temp_root("reopen");
        let first = ProjectionDb::open_or_create(root.path()).expect("first");
        let built_at: String = first
            .connection()
            .query_row(
                "SELECT built_at FROM projection_meta WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        drop(first);

        let second = ProjectionDb::open_or_create(root.path()).expect("second");
        assert_eq!(second.schema_version().unwrap(), PROJECTION_SCHEMA_VERSION);
        let built_at2: String = second
            .connection()
            .query_row(
                "SELECT built_at FROM projection_meta WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(built_at, built_at2);
        drop(second);
    }

    #[test]
    fn schema_and_fts_are_consistent_after_create() {
        let db = ProjectionDb::open_in_memory().expect("memory");
        validate_schema_v1(db.connection()).expect("validate");
        let sql: String = db
            .connection()
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name='events_fts'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let lowered = sql.to_ascii_lowercase();
        assert!(lowered.contains("fts5"));
        assert!(lowered.contains("content='events'") || lowered.contains("content=\"events\""));
        // text-only FTS: no entity/fact columns in the fts definition.
        assert!(!lowered.contains("entity"));
        assert!(!lowered.contains("relation"));
    }

    #[test]
    fn open_replacing_unusable_recreates_corrupt_file() {
        let root = exclusive_temp_root("corrupt-db");
        let path = projection_sqlite_path(root.path());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"not-a-sqlite-database").unwrap();
        let (db, kind) = ProjectionDb::open_replacing_unusable(root.path()).expect("replace");
        match kind {
            ProjectionOpenKind::Replaced {
                reason: ProjectionReplaceReason::Corrupt { .. },
            } => {}
            other => panic!("expected Replaced Corrupt, got {other:?}"),
        }
        assert_eq!(db.schema_version().unwrap(), PROJECTION_SCHEMA_VERSION);
        drop(db);
    }

    #[test]
    fn open_replacing_unusable_recreates_unsupported_schema() {
        let root = exclusive_temp_root("badver-replace");
        let db = ProjectionDb::open_or_create(root.path()).expect("create v1");
        db.connection()
            .execute(
                "UPDATE projection_meta SET schema_version = 99 WHERE id = 1",
                [],
            )
            .unwrap();
        drop(db);
        let (db, kind) = ProjectionDb::open_replacing_unusable(root.path()).expect("replace");
        match kind {
            ProjectionOpenKind::Replaced {
                reason: ProjectionReplaceReason::UnsupportedSchema { found: 99 },
            } => {}
            other => panic!("expected UnsupportedSchema replace, got {other:?}"),
        }
        assert_eq!(db.schema_version().unwrap(), PROJECTION_SCHEMA_VERSION);
        drop(db);
    }

    #[test]
    fn unsupported_projection_schema_version_fails_closed() {
        let root = exclusive_temp_root("badver");
        let db = ProjectionDb::open_or_create(root.path()).expect("create v1");
        db.connection()
            .execute(
                "UPDATE projection_meta SET schema_version = 99 WHERE id = 1",
                [],
            )
            .unwrap();
        drop(db);

        let err = ProjectionDb::open_or_create(root.path()).expect_err("must refuse");
        assert_eq!(
            err,
            ProjectionError::UnsupportedSchema {
                found: 99,
                expected: PROJECTION_SCHEMA_VERSION,
            }
        );
    }
}
