//! Projection database open/create and disposable replace (M3 Slice 3/5).
//!
//! Opens disposable SQLite under the derived projection path. Does not ingest
//! canonical events. Slice 5 may delete/replace this file when unusable; that
//! must never touch canonical packs.

#[cfg(test)]
use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use crate::layout::{derived_projection_dir, projection_sqlite_path};
use crate::schema::{
    PROJECTION_SCHEMA_VERSION, apply_schema_v1, read_schema_version, validate_schema_v1,
};
use crate::{ProjectionError, ProjectionResult};

#[cfg(test)]
thread_local! {
    static INJECT_TRY_OPEN_ERROR: RefCell<Option<ProjectionError>> = const { RefCell::new(None) };
    /// Injected after schema init; mapped via [`map_fts_integrity_sqlite_error`].
    static INJECT_FTS_INTEGRITY_ERROR: RefCell<Option<rusqlite::Error>> =
        const { RefCell::new(None) };
    /// Injected raw open/quick_check failure; mapped via [`map_open_sqlite_error`].
    static INJECT_OPEN_SQLITE_ERROR: RefCell<Option<rusqlite::Error>> =
        const { RefCell::new(None) };
    /// Optional -journal marker written at integrity-check time (after Connection::open).
    /// Only `-journal` is planted here: on Windows an open WAL connection maps `-wal`/`-shm`.
    static FTS_INTEGRITY_SIDECAR_MARKER: RefCell<Option<(PathBuf, Vec<u8>)>> =
        const { RefCell::new(None) };
}

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

    /// Open an existing projection without creating, migrating, or repairing
    /// it. This is for startup gates that must compare derived state before
    /// normal projection setup; missing, corrupt, and unsupported databases
    /// fail closed and remain untouched.
    pub fn open_existing_read_only(app_data_root: &Path) -> ProjectionResult<Self> {
        let path = projection_sqlite_path(app_data_root);
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(ProjectionError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "existing projection path is not an ordinary file",
            )));
        }
        let conn = Connection::open_with_flags(
            &path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        let check: String = conn.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
        if check != "ok" {
            return Err(ProjectionError::CorruptDb {
                detail: format!("quick_check reported {check}"),
            });
        }
        crate::schema::validate_schema_v1_read_only(&conn)?;
        Ok(Self { conn, path })
    }

    /// Open the projection DB, replacing only explicitly unusable disposable files.
    ///
    /// Replacement runs solely for [`ProjectionError::CorruptDb`],
    /// [`ProjectionError::UnsupportedSchema`], or
    /// [`ProjectionError::SchemaInconsistent`]. Transient I/O, ordinary SQLite
    /// errors (busy/locked/permission), and [`ProjectionError::Fts5Unavailable`]
    /// propagate without deleting the DB or sidecars.
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
            Err(err) => match replaceable_unusable_reason(&err) {
                Some(reason) => {
                    remove_projection_files(app_data_root)?;
                    let db = Self::open_or_create(app_data_root)?;
                    Ok((db, ProjectionOpenKind::Replaced { reason }))
                }
                None => Err(err),
            },
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

    /// Mutable connection access for additive derived-store migrations.
    ///
    /// M4 extensions must preserve the M3 schema and use this handle only after
    /// the projection has been opened and validated by this type.
    pub fn connection_mut(&mut self) -> &mut Connection {
        &mut self.conn
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

/// Sidecar paths for a projection DB, built without lossy Unicode conversion.
///
/// SQLite uses the main DB path bytes/OS string plus `-wal`, `-shm`, or `-journal`.
pub fn projection_sidecar_paths(db_path: &Path) -> [PathBuf; 3] {
    [
        append_os_suffix(db_path, "-wal"),
        append_os_suffix(db_path, "-shm"),
        append_os_suffix(db_path, "-journal"),
    ]
}

fn append_os_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut os = path.as_os_str().to_owned();
    os.push(suffix);
    PathBuf::from(os)
}

/// Delete disposable projection DB files under `derived/projection/` (not canonical).
pub fn remove_projection_files(app_data_root: &Path) -> ProjectionResult<()> {
    let path = projection_sqlite_path(app_data_root);
    let mut candidates = Vec::with_capacity(4);
    candidates.push(path.clone());
    candidates.extend(projection_sidecar_paths(&path));
    for candidate in candidates {
        match fs::remove_file(&candidate) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }
    }
    Ok(())
}

fn try_open_existing(path: &Path) -> ProjectionResult<ProjectionDb> {
    #[cfg(test)]
    if let Some(err) = INJECT_TRY_OPEN_ERROR.with(|slot| slot.borrow_mut().take()) {
        return Err(err);
    }
    #[cfg(test)]
    if let Some(err) = INJECT_OPEN_SQLITE_ERROR.with(|slot| slot.borrow_mut().take()) {
        return Err(map_open_sqlite_error(err));
    }
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
    // FTS5 external-content consistency (rank=1 compares index to `events`).
    check_fts5_external_content_integrity(&conn)?;
    Ok(ProjectionDb {
        conn,
        path: path.to_path_buf(),
    })
}

/// Fail closed when the disposable FTS5 index disagrees with `events` content.
///
/// Uses FTS5 `integrity-check` with `rank=1` so external-content tables are
/// compared against the content table. Confirmed corruption / FTS integrity
/// mismatches become [`ProjectionError::CorruptDb`] (replaceable). Busy/locked,
/// permission, and other ordinary SQLite failures stay [`ProjectionError::Sqlite`]
/// and must not delete disposable files.
fn check_fts5_external_content_integrity(conn: &Connection) -> ProjectionResult<()> {
    #[cfg(test)]
    if let Some((path, bytes)) = FTS_INTEGRITY_SIDECAR_MARKER.with(|slot| slot.borrow_mut().take())
    {
        fs::write(&path, bytes).expect("plant integrity-stage sidecar marker");
    }
    #[cfg(test)]
    if let Some(err) = INJECT_FTS_INTEGRITY_ERROR.with(|slot| slot.borrow_mut().take()) {
        return Err(map_fts_integrity_sqlite_error(err));
    }
    match conn.execute(
        "INSERT INTO events_fts(events_fts, rank) VALUES('integrity-check', 1)",
        [],
    ) {
        Ok(_) => Ok(()),
        Err(err) => Err(map_fts_integrity_sqlite_error(err)),
    }
}

/// SQLITE_CORRUPT_VTAB = SQLITE_CORRUPT | (1 << 8). FTS5 integrity-check uses this.
const SQLITE_CORRUPT_VTAB: i32 = 267;

/// True only for SQLite corruption / non-database result classes — never from message text.
fn is_sqlite_corruption_class(efi: &rusqlite::ffi::Error) -> bool {
    matches!(
        efi.code,
        rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase
    ) || efi.extended_code == SQLITE_CORRUPT_VTAB
}

/// Map FTS5 integrity-check failures: only confirmed corruption is replaceable.
fn map_fts_integrity_sqlite_error(err: rusqlite::Error) -> ProjectionError {
    match &err {
        rusqlite::Error::SqliteFailure(efi, _) if is_sqlite_corruption_class(efi) => {
            // Includes SQLITE_CORRUPT / SQLITE_NOTADB / SQLITE_CORRUPT_VTAB (FTS virtual table).
            ProjectionError::CorruptDb {
                detail: format!("events_fts external-content integrity-check failed: {err}"),
            }
        }
        _ => ProjectionError::Sqlite(err),
    }
}

/// Map only clear on-disk corruption signals to [`ProjectionError::CorruptDb`].
///
/// Busy/locked/permission and other ordinary SQLite failures stay
/// [`ProjectionError::Sqlite`]. Classification uses SQLite error/extended codes only.
fn map_open_sqlite_error(err: rusqlite::Error) -> ProjectionError {
    match &err {
        rusqlite::Error::SqliteFailure(efi, _) if is_sqlite_corruption_class(efi) => {
            ProjectionError::CorruptDb {
                detail: "sqlite reported a corrupt or non-database file".into(),
            }
        }
        _ => ProjectionError::Sqlite(err),
    }
}

/// Returns a replace reason only for explicitly unusable disposable state.
fn replaceable_unusable_reason(err: &ProjectionError) -> Option<ProjectionReplaceReason> {
    match err {
        ProjectionError::UnsupportedSchema { found, .. } => {
            Some(ProjectionReplaceReason::UnsupportedSchema { found: *found })
        }
        ProjectionError::SchemaInconsistent { detail } => {
            Some(ProjectionReplaceReason::SchemaInconsistent {
                detail: detail.clone(),
            })
        }
        ProjectionError::CorruptDb { detail } => Some(ProjectionReplaceReason::Corrupt {
            detail: detail.clone(),
        }),
        ProjectionError::Io(_)
        | ProjectionError::Sqlite(_)
        | ProjectionError::Fts5Unavailable { .. }
        | ProjectionError::EventConflict { .. }
        | ProjectionError::InvalidQuery { .. }
        | ProjectionError::OperationDeadlineExceeded
        | ProjectionError::InvalidVaultEvent => None,
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
    use crate::ingest::project_memory_note;
    use crate::layout::projection_sqlite_path;
    use crate::schema::PROJECTION_SCHEMA_VERSION;
    use crate::search::{SearchQuery, search_events};
    use rusqlite::Connection;
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
    fn existing_read_only_open_validates_without_mutating_projection_file() {
        let root = exclusive_temp_root("readonly-open");
        let created = ProjectionDb::open_or_create(root.path()).expect("create baseline");
        drop(created);
        let path = projection_sqlite_path(root.path());
        let before = fs::read(&path).expect("read baseline bytes");

        let db = ProjectionDb::open_existing_read_only(root.path()).expect("read-only open");
        assert_eq!(db.schema_version().unwrap(), PROJECTION_SCHEMA_VERSION);
        assert!(
            db.connection()
                .execute_batch("CREATE TABLE read_only_probe(value TEXT)")
                .is_err()
        );
        drop(db);

        assert_eq!(fs::read(&path).unwrap(), before);
        assert!(!path.with_extension("sqlite-wal").exists());
        assert!(!path.with_extension("sqlite-shm").exists());
    }

    #[test]
    fn existing_read_only_open_does_not_create_a_missing_database() {
        let root = exclusive_temp_root("readonly-missing");
        assert!(ProjectionDb::open_existing_read_only(root.path()).is_err());
        assert!(!derived_projection_dir(root.path()).exists());
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
    fn projection_sidecar_paths_append_without_lossy_display() {
        let db = PathBuf::from("derived")
            .join("projection")
            .join("projection.sqlite");
        let [wal, shm, journal] = projection_sidecar_paths(&db);
        assert!(
            wal.as_os_str()
                .to_string_lossy()
                .ends_with("projection.sqlite-wal")
        );
        assert!(
            shm.as_os_str()
                .to_string_lossy()
                .ends_with("projection.sqlite-shm")
        );
        assert!(
            journal
                .as_os_str()
                .to_string_lossy()
                .ends_with("projection.sqlite-journal")
        );
        // Parent path is preserved as OsStr concatenation, not re-parsed via Display.
        assert_eq!(wal.parent(), db.parent());
    }

    #[test]
    fn replaceable_unusable_reason_ignores_transient_and_fts_errors() {
        assert!(
            replaceable_unusable_reason(&ProjectionError::Fts5Unavailable {
                detail: "simulated".into(),
            })
            .is_none()
        );
        assert!(
            replaceable_unusable_reason(&ProjectionError::Io(std::io::Error::other(
                "simulated io"
            )))
            .is_none()
        );
        let sqlite_err = Connection::open_in_memory()
            .unwrap()
            .execute_batch("this is not sql")
            .expect_err("sql");
        assert!(replaceable_unusable_reason(&ProjectionError::Sqlite(sqlite_err)).is_none());
        assert!(matches!(
            replaceable_unusable_reason(&ProjectionError::CorruptDb { detail: "x".into() }),
            Some(ProjectionReplaceReason::Corrupt { .. })
        ));
        assert!(matches!(
            replaceable_unusable_reason(&ProjectionError::UnsupportedSchema {
                found: 9,
                expected: PROJECTION_SCHEMA_VERSION,
            }),
            Some(ProjectionReplaceReason::UnsupportedSchema { found: 9 })
        ));
        assert!(matches!(
            replaceable_unusable_reason(&ProjectionError::SchemaInconsistent {
                detail: "missing".into(),
            }),
            Some(ProjectionReplaceReason::SchemaInconsistent { .. })
        ));
    }

    fn seed_db_with_sidecar_markers(
        root: &TempRoot,
    ) -> (PathBuf, String, [PathBuf; 3], &'static [u8]) {
        let db = ProjectionDb::open_or_create(root.path()).expect("create");
        let path = db.path().to_path_buf();
        let built_at: String = db
            .connection()
            .query_row(
                "SELECT built_at FROM projection_meta WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        drop(db);
        let marker: &'static [u8] = b"cryptex-sidecar-marker";
        let sidecars = projection_sidecar_paths(&path);
        for side in &sidecars {
            fs::write(side, marker).unwrap();
        }
        (path, built_at, sidecars, marker)
    }

    fn assert_sidecars_untouched(sidecars: &[PathBuf; 3], marker: &[u8]) {
        for side in sidecars {
            assert_eq!(
                fs::read(side).expect("sidecar"),
                marker,
                "sidecar must remain untouched: {}",
                side.display()
            );
        }
    }

    #[test]
    fn open_replacing_unusable_preserves_files_on_injected_non_corrupt_errors() {
        let cases = [
            ProjectionError::Fts5Unavailable {
                detail: "injected fts unavailable".into(),
            },
            ProjectionError::Io(std::io::Error::other("injected io")),
            ProjectionError::Sqlite(
                Connection::open_in_memory()
                    .unwrap()
                    .execute_batch("not valid sql ;;;")
                    .expect_err("sqlite"),
            ),
        ];

        for injected in cases {
            let root = exclusive_temp_root("inject-preserve");
            let (path, built_at, sidecars, marker) = seed_db_with_sidecar_markers(&root);
            INJECT_TRY_OPEN_ERROR.with(|slot| {
                *slot.borrow_mut() = Some(injected);
            });
            let err = ProjectionDb::open_replacing_unusable(root.path())
                .expect_err("non-replaceable error must propagate");
            assert!(
                matches!(
                    err,
                    ProjectionError::Fts5Unavailable { .. }
                        | ProjectionError::Io(_)
                        | ProjectionError::Sqlite(_)
                ),
                "unexpected error: {err}"
            );
            assert!(path.is_file(), "main DB must remain");
            assert_sidecars_untouched(&sidecars, marker);

            let (reopen, kind) =
                ProjectionDb::open_replacing_unusable(root.path()).expect("reopen healthy");
            assert!(matches!(kind, ProjectionOpenKind::OpenedExisting));
            let built_at2: String = reopen
                .connection()
                .query_row(
                    "SELECT built_at FROM projection_meta WHERE id = 1",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(built_at, built_at2);
            drop(reopen);
        }
    }

    fn system_io_failure_with_message(message: &str) -> rusqlite::Error {
        rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ErrorCode::SystemIoFailure,
                extended_code: 10,
            },
            Some(message.into()),
        )
    }

    const MISLEADING_CORRUPTION_MESSAGES: [&str; 4] = [
        "database disk image is malformed",
        "file appears corrupt on read",
        "fts5: integrity-check reported a problem",
        "file is not a database",
    ];

    #[test]
    fn map_fts_integrity_sqlite_error_classifies_corrupt_vs_transient() {
        let busy = rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ErrorCode::DatabaseBusy,
                extended_code: 5,
            },
            Some("database is locked".into()),
        );
        assert!(matches!(
            map_fts_integrity_sqlite_error(busy),
            ProjectionError::Sqlite(_)
        ));

        let locked = rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ErrorCode::DatabaseLocked,
                extended_code: 6,
            },
            Some("database table is locked".into()),
        );
        assert!(matches!(
            map_fts_integrity_sqlite_error(locked),
            ProjectionError::Sqlite(_)
        ));

        let io = system_io_failure_with_message("disk I/O error");
        assert!(matches!(
            map_fts_integrity_sqlite_error(io),
            ProjectionError::Sqlite(_)
        ));

        let perm = rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ErrorCode::PermissionDenied,
                extended_code: 3,
            },
            Some("access permission denied".into()),
        );
        assert!(matches!(
            map_fts_integrity_sqlite_error(perm),
            ProjectionError::Sqlite(_)
        ));

        for message in MISLEADING_CORRUPTION_MESSAGES {
            let err = system_io_failure_with_message(message);
            assert!(
                matches!(
                    map_fts_integrity_sqlite_error(err),
                    ProjectionError::Sqlite(_)
                ),
                "SystemIoFailure message must not become CorruptDb: {message}"
            );
        }

        let corrupt = rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ErrorCode::DatabaseCorrupt,
                extended_code: 11,
            },
            Some("database disk image is malformed".into()),
        );
        assert!(matches!(
            map_fts_integrity_sqlite_error(corrupt),
            ProjectionError::CorruptDb { .. }
        ));

        let not_a_db = rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ErrorCode::NotADatabase,
                extended_code: 26,
            },
            Some("file is not a database".into()),
        );
        assert!(matches!(
            map_fts_integrity_sqlite_error(not_a_db),
            ProjectionError::CorruptDb { .. }
        ));

        let fts_mismatch = rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ErrorCode::DatabaseCorrupt,
                extended_code: SQLITE_CORRUPT_VTAB,
            },
            Some("fts5: integrity-check failed for table events_fts".into()),
        );
        match map_fts_integrity_sqlite_error(fts_mismatch) {
            ProjectionError::CorruptDb { detail } => {
                assert!(detail.to_ascii_lowercase().contains("integrity-check"));
            }
            other => panic!("expected CorruptDb, got {other}"),
        }
    }

    #[test]
    fn map_open_sqlite_error_classifies_corrupt_vs_transient() {
        for message in MISLEADING_CORRUPTION_MESSAGES {
            let err = system_io_failure_with_message(message);
            assert!(
                matches!(map_open_sqlite_error(err), ProjectionError::Sqlite(_)),
                "SystemIoFailure message must not become CorruptDb: {message}"
            );
        }

        let corrupt = rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ErrorCode::DatabaseCorrupt,
                extended_code: 11,
            },
            Some("ignored malformed text".into()),
        );
        assert!(matches!(
            map_open_sqlite_error(corrupt),
            ProjectionError::CorruptDb { .. }
        ));

        let not_a_db = rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ErrorCode::NotADatabase,
                extended_code: 26,
            },
            Some("ignored".into()),
        );
        assert!(matches!(
            map_open_sqlite_error(not_a_db),
            ProjectionError::CorruptDb { .. }
        ));
    }

    #[test]
    fn injected_io_messages_mentioning_corruption_preserve_db_and_sidecars() {
        for message in MISLEADING_CORRUPTION_MESSAGES {
            // Via FTS integrity-check mapper: plant -journal after open (WAL maps -wal/-shm).
            let root = exclusive_temp_root("io-msg-fts");
            let (path, built_at, sidecars, marker) = seed_db_with_sidecar_markers(&root);
            let before_bytes = fs::read(&path).unwrap();
            let journal = sidecars[2].clone();
            FTS_INTEGRITY_SIDECAR_MARKER.with(|slot| {
                *slot.borrow_mut() = Some((journal.clone(), marker.to_vec()));
            });
            INJECT_FTS_INTEGRITY_ERROR.with(|slot| {
                *slot.borrow_mut() = Some(system_io_failure_with_message(message));
            });
            let err = ProjectionDb::open_replacing_unusable(root.path())
                .expect_err("misleading I/O must not replace");
            assert!(
                matches!(err, ProjectionError::Sqlite(_)),
                "expected Sqlite for FTS path message {message:?}, got {err}"
            );
            assert_eq!(
                fs::read(&path).unwrap(),
                before_bytes,
                "main DB bytes must be unchanged for {message}"
            );
            assert_eq!(
                fs::read(&journal).expect("journal sidecar must remain"),
                marker,
                "journal must remain for {message}"
            );
            let (reopen, kind) =
                ProjectionDb::open_replacing_unusable(root.path()).expect("reopen");
            assert!(matches!(kind, ProjectionOpenKind::OpenedExisting));
            let built_at2: String = reopen
                .connection()
                .query_row(
                    "SELECT built_at FROM projection_meta WHERE id = 1",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(built_at, built_at2);
            drop(reopen);

            // Via open mapper: inject before Connection::open so all planted sidecars remain.
            let root = exclusive_temp_root("io-msg-open");
            let (path, built_at, sidecars, marker) = seed_db_with_sidecar_markers(&root);
            let before_bytes = fs::read(&path).unwrap();
            INJECT_OPEN_SQLITE_ERROR.with(|slot| {
                *slot.borrow_mut() = Some(system_io_failure_with_message(message));
            });
            let err = ProjectionDb::open_replacing_unusable(root.path())
                .expect_err("misleading I/O must not replace");
            assert!(
                matches!(err, ProjectionError::Sqlite(_)),
                "expected Sqlite for open path message {message:?}, got {err}"
            );
            assert_eq!(
                fs::read(&path).unwrap(),
                before_bytes,
                "main DB bytes must be unchanged for {message}"
            );
            assert_sidecars_untouched(&sidecars, marker);
            let (reopen, kind) =
                ProjectionDb::open_replacing_unusable(root.path()).expect("reopen");
            assert!(matches!(kind, ProjectionOpenKind::OpenedExisting));
            let built_at2: String = reopen
                .connection()
                .query_row(
                    "SELECT built_at FROM projection_meta WHERE id = 1",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(built_at, built_at2);
            drop(reopen);
        }
    }

    #[test]
    fn transient_fts_integrity_error_preserves_db_and_sidecars() {
        let root = exclusive_temp_root("fts-integrity-busy");
        let (path, built_at, sidecars, marker) = seed_db_with_sidecar_markers(&root);
        let before_bytes = fs::read(&path).unwrap();

        // Plant -journal at integrity-check time (after Connection::open). In WAL
        // mode SQLite leaves -journal alone; remove_projection_files would delete it.
        let journal = sidecars[2].clone();
        FTS_INTEGRITY_SIDECAR_MARKER.with(|slot| {
            *slot.borrow_mut() = Some((journal.clone(), marker.to_vec()));
        });

        let busy = rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ErrorCode::DatabaseBusy,
                extended_code: 5,
            },
            Some("database is locked".into()),
        );
        INJECT_FTS_INTEGRITY_ERROR.with(|slot| {
            *slot.borrow_mut() = Some(busy);
        });

        let err = ProjectionDb::open_replacing_unusable(root.path())
            .expect_err("transient integrity failure must not replace");
        assert!(
            matches!(err, ProjectionError::Sqlite(_)),
            "expected non-replaceable Sqlite error, got {err}"
        );
        assert!(path.is_file(), "main DB must remain");
        assert_eq!(
            fs::read(&path).unwrap(),
            before_bytes,
            "main projection DB bytes must be unchanged"
        );
        assert_eq!(
            fs::read(&journal).expect("journal sidecar marker must remain"),
            marker,
            "sidecar must remain when integrity-check fails as non-replaceable Sqlite"
        );
        // Also re-plant all three and prove early-open inject still preserves them.
        for side in &sidecars {
            fs::write(side, marker).unwrap();
        }
        let busy2 = rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ErrorCode::DatabaseBusy,
                extended_code: 5,
            },
            Some("database is locked".into()),
        );
        INJECT_TRY_OPEN_ERROR.with(|slot| {
            *slot.borrow_mut() = Some(ProjectionError::Sqlite(busy2));
        });
        let _ = ProjectionDb::open_replacing_unusable(root.path()).expect_err("busy");
        assert_sidecars_untouched(&sidecars, marker);

        let (reopen, kind) =
            ProjectionDb::open_replacing_unusable(root.path()).expect("reopen healthy");
        assert!(matches!(kind, ProjectionOpenKind::OpenedExisting));
        let built_at2: String = reopen
            .connection()
            .query_row(
                "SELECT built_at FROM projection_meta WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(built_at, built_at2);
        drop(reopen);
    }

    #[test]
    fn open_replacing_unusable_preserves_files_on_io_open_failure() {
        let root = exclusive_temp_root("io-preserve");
        let db = ProjectionDb::open_or_create(root.path()).expect("create");
        let path = db.path().to_path_buf();
        let built_at: String = db
            .connection()
            .query_row(
                "SELECT built_at FROM projection_meta WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        drop(db);

        let marker = b"cryptex-io-sidecar-marker";
        let sidecars = projection_sidecar_paths(&path);
        for side in &sidecars {
            fs::write(side, marker).unwrap();
        }

        // Replace the DB file with a directory so open fails without corruption
        // classification; disposable files must not be deleted.
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();

        let err = ProjectionDb::open_replacing_unusable(root.path())
            .expect_err("directory-as-db must not replace");
        assert!(
            matches!(err, ProjectionError::Sqlite(_) | ProjectionError::Io(_)),
            "unexpected error: {err}"
        );
        assert!(path.is_dir(), "path must remain a directory (not deleted)");
        for side in &sidecars {
            assert_eq!(fs::read(side).unwrap(), marker);
        }

        // Restore a real DB beside preserved sidecars and confirm open does not
        // require prior destructive cleanup of markers for success path.
        fs::remove_dir(&path).unwrap();
        let (recreate, kind) =
            ProjectionDb::open_replacing_unusable(root.path()).expect("create after restore");
        assert!(matches!(kind, ProjectionOpenKind::Created));
        let _ = built_at;
        drop(recreate);
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

    #[test]
    fn fts_external_content_inconsistency_is_replaced_and_search_restorable() {
        use cryptex_events::device_seq::DeviceSeq;
        use cryptex_events::envelope::{
            AcceptedEvent, Classification, EVENT_SCHEMA_VERSION, EventPayload, EventType,
            MemoryNotePayload,
        };
        use cryptex_events::hex::{ContentHashHex, SignatureHex, SigningKeyId};

        let root = exclusive_temp_root("fts-integrity");
        let db = ProjectionDb::open_or_create(root.path()).expect("create");
        let note = AcceptedEvent {
            schema_version: EVENT_SCHEMA_VERSION,
            event_id: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".into(),
            brain_id: "22222222-2222-4222-8222-222222222222".into(),
            device_id: "fts-device".into(),
            device_seq: DeviceSeq::from_u64(1).unwrap(),
            created_at: "2026-10-08T00:00:00Z".into(),
            event_type: EventType::MemoryNote,
            classification: Classification::Public,
            signing_key_id: SigningKeyId::parse(&"ab".repeat(16)).unwrap(),
            payload: EventPayload::MemoryNote(
                MemoryNotePayload::new("fts_integrity_token unique").unwrap(),
            ),
            prev_content_hash: ContentHashHex::zero(),
            content_hash: ContentHashHex::parse(&"11".repeat(32)).unwrap(),
            signature: SignatureHex::parse(&"ef".repeat(64)).unwrap(),
        };
        project_memory_note(db.connection(), &note).unwrap();
        let before = search_events(
            db.connection(),
            &SearchQuery {
                text: Some("fts_integrity_token".into()),
                ..SearchQuery::default()
            },
        )
        .unwrap();
        assert_eq!(before.len(), 1);
        drop(db);

        // Corrupt the external-content FTS index while leaving `events` rows intact.
        {
            let path = projection_sqlite_path(root.path());
            let conn = rusqlite::Connection::open(&path).unwrap();
            conn.execute(
                "INSERT INTO events_fts(events_fts) VALUES('delete-all')",
                [],
            )
            .unwrap();
            let broken = search_events(
                &conn,
                &SearchQuery {
                    text: Some("fts_integrity_token".into()),
                    ..SearchQuery::default()
                },
            )
            .unwrap();
            assert!(
                broken.is_empty(),
                "delete-all must clear FTS hits while events rows remain"
            );
            let events: i64 = conn
                .query_row("SELECT COUNT(*) FROM events", [], |r| r.get::<_, i64>(0))
                .unwrap();
            assert_eq!(events, 1);
        }

        let (db, kind) = ProjectionDb::open_replacing_unusable(root.path()).expect("replace");
        match kind {
            ProjectionOpenKind::Replaced {
                reason: ProjectionReplaceReason::Corrupt { detail },
            } => {
                assert!(
                    detail.to_ascii_lowercase().contains("integrity-check")
                        || detail.to_ascii_lowercase().contains("fts"),
                    "detail={detail}"
                );
            }
            other => panic!("expected Replaced Corrupt for inconsistent FTS, got {other:?}"),
        }
        // Replacement yields an empty disposable DB; callers rebuild from canonical.
        assert_eq!(
            db.connection()
                .query_row("SELECT COUNT(*) FROM events", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0i64
        );
        project_memory_note(db.connection(), &note).unwrap();
        let restored = search_events(
            db.connection(),
            &SearchQuery {
                text: Some("fts_integrity_token".into()),
                ..SearchQuery::default()
            },
        )
        .unwrap();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].event_id, note.event_id);
        drop(db);
    }
}
