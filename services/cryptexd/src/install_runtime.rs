//! Daemon-owned install identity persistence (ADR-0015 section 2.2 / section 4).
//!
//! New install => new `install_id`. Existing durable identity is loaded and never
//! silently rewritten in place. First-create uses exclusive no-replace placement
//! so concurrent ensure/load_or_create callers cannot overwrite the winner.

use std::fs::{self, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use cryptex_core::{InstallId, InstallIdentity};

use crate::identity_runtime::IdentityRuntimeError;
use crate::paths::{assert_outside_repository, ensure_runtime_root};
use crate::sequence_runtime::open_sequence_store;

const IDENTITY_DIR: &str = "identity";
const INSTALL_FILE: &str = "install_identity.json";

static TMP_SEQ: AtomicU64 = AtomicU64::new(1);

pub fn install_identity_path(runtime_root: &Path) -> PathBuf {
    runtime_root.join(IDENTITY_DIR).join(INSTALL_FILE)
}

fn unique_install_tmp(path: &Path) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    path.with_extension(format!("json.tmp.{}-{}-{}", std::process::id(), seq, nanos))
}

fn remove_file_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

fn with_staging_cleanup(
    primary: IdentityRuntimeError,
    cleanup: io::Result<()>,
) -> IdentityRuntimeError {
    match cleanup {
        Ok(()) => primary,
        Err(cleanup_error) => IdentityRuntimeError::PersistenceCleanupFailed {
            primary: Box::new(primary),
            cleanup_error: cleanup_error.to_string(),
        },
    }
}

fn finish_staging_write<F, C>(
    tmp: &Path,
    mut file: fs::File,
    bytes: &[u8],
    write_and_sync: F,
    cleanup: C,
) -> Result<(), IdentityRuntimeError>
where
    F: FnOnce(&mut fs::File, &[u8]) -> io::Result<()>,
    C: FnOnce(&Path) -> io::Result<()>,
{
    if let Err(primary) = write_and_sync(&mut file, bytes) {
        drop(file);
        return Err(with_staging_cleanup(primary.into(), cleanup(tmp)));
    }
    Ok(())
}

/// Sync the directory that contains `path` so a new directory entry is durable.
///
/// Used on Unix/macOS after exclusive `hard_link` placement. Failures must be
/// propagated - callers must not treat the install identity as committed.
/// Compiled on Windows only for unit tests of the error path.
#[cfg(any(unix, test))]
fn sync_parent_directory(path: &Path) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            ErrorKind::InvalidInput,
            "install identity path has no parent directory",
        )
    })?;
    let dir = fs::File::open(parent)?;
    dir.sync_all()?;
    Ok(())
}

/// Remove an uncommitted destination link. `NotFound` is success.
#[cfg(unix)]
fn remove_uncommitted_dest(dest: &Path) -> io::Result<()> {
    remove_file_if_present(dest)
}

#[derive(Debug)]
enum PlacementError {
    Io(io::Error),
    AlreadyExists(String),
    #[cfg(unix)]
    CleanupFailed {
        primary: io::Error,
        cleanup_errors: Vec<io::Error>,
    },
}

impl PlacementError {
    fn from_io(error: io::Error, dest: &Path) -> Self {
        if error.kind() == ErrorKind::AlreadyExists {
            Self::AlreadyExists(dest.display().to_string())
        } else {
            Self::Io(error)
        }
    }
}

impl std::fmt::Display for PlacementError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::AlreadyExists(path) => write!(f, "identity already exists at {path}"),
            #[cfg(unix)]
            Self::CleanupFailed {
                primary,
                cleanup_errors,
            } => write!(
                f,
                "{primary}; placement rollback cleanup failed: {}",
                cleanup_errors
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
        }
    }
}

fn placement_error_to_identity_error(error: PlacementError, tmp: &Path) -> IdentityRuntimeError {
    match error {
        PlacementError::AlreadyExists(path) => with_staging_cleanup(
            IdentityRuntimeError::AlreadyExists(path),
            remove_file_if_present(tmp),
        ),
        PlacementError::Io(primary) => {
            with_staging_cleanup(primary.into(), remove_file_if_present(tmp))
        }
        #[cfg(unix)]
        PlacementError::CleanupFailed {
            primary,
            cleanup_errors,
        } => {
            let mut cleanup_messages: Vec<String> = cleanup_errors
                .into_iter()
                .map(|error| error.to_string())
                .collect();
            if let Err(error) = remove_file_if_present(tmp) {
                cleanup_messages.push(format!("staging-file cleanup failed: {error}"));
            }
            IdentityRuntimeError::PersistenceCleanupFailed {
                primary: Box::new(primary.into()),
                cleanup_error: cleanup_messages.join("; "),
            }
        }
    }
}

/// Place `tmp` at `dest` only if `dest` does not already exist.
///
/// - Windows: `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING` (Rust's
///   `fs::rename` replaces on Windows and must not be used here). Unchanged
///   Windows durability relies on `MOVEFILE_WRITE_THROUGH`.
/// - Unix: `hard_link` + parent-directory sync before success is returned so the
///   new directory entry is durable. Sync failure removes the uncommitted
///   destination and syncs that rollback before failing.
fn place_new_file_no_replace(tmp: &Path, dest: &Path) -> Result<(), PlacementError> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;

        // Do not set MOVEFILE_REPLACE_EXISTING - losing creators must fail.
        const MOVEFILE_WRITE_THROUGH: u32 = 0x8;

        let from: Vec<u16> = tmp.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = dest.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: pointers are null-terminated wide strings from OsStr encoding.
        let ok = unsafe {
            windows_sys::Win32::Storage::FileSystem::MoveFileExW(
                from.as_ptr(),
                to.as_ptr(),
                MOVEFILE_WRITE_THROUGH,
            )
        };
        if ok == 0 {
            return Err(PlacementError::from_io(io::Error::last_os_error(), dest));
        }
        Ok(())
    }

    #[cfg(unix)]
    {
        place_new_file_no_replace_unix(
            tmp,
            dest,
            sync_parent_directory,
            remove_uncommitted_dest,
            remove_file_if_present,
        )
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = (tmp, dest);
        Err(PlacementError::from_io(
            io::Error::new(
                ErrorKind::Unsupported,
                "exclusive install-identity create is unsupported on this platform",
            ),
            dest,
        ))
    }
}

/// Unix exclusive create with injectable parent-sync, dest-cleanup, and staging
/// cleanup hooks (tests). Matches the hardened Brain-identity placement path.
#[cfg(unix)]
fn place_new_file_no_replace_unix<S, R, T>(
    tmp: &Path,
    dest: &Path,
    mut sync_parent: S,
    remove_dest: R,
    remove_tmp: T,
) -> Result<(), PlacementError>
where
    S: FnMut(&Path) -> io::Result<()>,
    R: FnOnce(&Path) -> io::Result<()>,
    T: FnOnce(&Path) -> io::Result<()>,
{
    fs::hard_link(tmp, dest).map_err(|error| PlacementError::from_io(error, dest))?;

    // Remove the staging link before committing the directory update. If this
    // fails, remove the destination link and sync that rollback before failing.
    if let Err(primary) = remove_tmp(tmp) {
        let mut cleanup_errors = Vec::new();
        if let Err(cleanup_err) = remove_dest(dest) {
            cleanup_errors.push(cleanup_err);
        }
        if let Err(sync_err) = sync_parent(dest) {
            cleanup_errors.push(sync_err);
        }
        return if cleanup_errors.is_empty() {
            Err(PlacementError::Io(primary))
        } else {
            Err(PlacementError::CleanupFailed {
                primary,
                cleanup_errors,
            })
        };
    }

    if let Err(primary) = sync_parent(dest) {
        let mut cleanup_errors = Vec::new();
        if let Err(cleanup_err) = remove_dest(dest) {
            cleanup_errors.push(cleanup_err);
        }
        if let Err(sync_err) = sync_parent(dest) {
            cleanup_errors.push(sync_err);
        }
        return if cleanup_errors.is_empty() {
            Err(PlacementError::Io(primary))
        } else {
            Err(PlacementError::CleanupFailed {
                primary,
                cleanup_errors,
            })
        };
    }
    Ok(())
}

/// Persist a new install identity. Fails if one already exists (no replace).
pub fn write_install_identity(
    runtime_root: &Path,
    identity: &InstallIdentity,
) -> Result<(), IdentityRuntimeError> {
    identity.validate().map_err(IdentityRuntimeError::from)?;
    assert_outside_repository(runtime_root)?;
    ensure_runtime_root(runtime_root)?;
    let dir = runtime_root.join(IDENTITY_DIR);
    fs::create_dir_all(&dir)?;
    let path = install_identity_path(runtime_root);
    if path.exists() {
        return Err(IdentityRuntimeError::AlreadyExists(
            path.display().to_string(),
        ));
    }

    let json = identity.to_json_lf()?;
    let tmp = unique_install_tmp(&path);
    let file = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
    finish_staging_write(
        &tmp,
        file,
        json.as_bytes(),
        |file, bytes| {
            file.write_all(bytes)?;
            file.sync_all()
        },
        remove_file_if_present,
    )?;

    match place_new_file_no_replace(&tmp, &path) {
        Ok(()) => Ok(()),
        Err(error) => Err(placement_error_to_identity_error(error, &tmp)),
    }
}

pub fn load_install_identity(runtime_root: &Path) -> Result<InstallIdentity, IdentityRuntimeError> {
    let path = install_identity_path(runtime_root);
    let text = fs::read_to_string(&path)?;
    Ok(InstallIdentity::from_json(&text)?)
}

/// Load existing install identity, or create and persist a new one when absent.
///
/// Concurrent first-create losers observe [`IdentityRuntimeError::AlreadyExists`]
/// from exclusive placement and load the persisted winner instead of overwriting.
pub fn load_or_create_install_identity(
    runtime_root: &Path,
    new_install_id: InstallId,
) -> Result<InstallIdentity, IdentityRuntimeError> {
    let path = install_identity_path(runtime_root);
    if path.exists() {
        return load_install_identity(runtime_root);
    }
    let identity = InstallIdentity::new(new_install_id)?;
    match write_install_identity(runtime_root, &identity) {
        Ok(()) => Ok(identity),
        Err(IdentityRuntimeError::AlreadyExists(_)) => load_install_identity(runtime_root),
        Err(e) => Err(e),
    }
}

fn generate_install_id() -> Result<InstallId, IdentityRuntimeError> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    // Path-safe synthetic id for a new local installation (ADR-0015 section 2.2).
    let raw = format!("install-{}-{}", std::process::id(), nanos);
    InstallId::new(raw).map_err(IdentityRuntimeError::from)
}

/// Ensure a durable install identity exists for this runtime root (create-once).
pub fn ensure_install_identity(
    runtime_root: &Path,
) -> Result<InstallIdentity, IdentityRuntimeError> {
    if install_identity_path(runtime_root).exists() {
        return load_install_identity(runtime_root);
    }
    let id = generate_install_id()?;
    load_or_create_install_identity(runtime_root, id)
}

/// Live-daemon M1 foundation bind for a runtime root under cryptexd authority.
///
/// - Persists/loads install identity (`new install => new install_id`).
/// - Opens the daemon-owned sequence store directory (no allocate; allocation is
///   for enrolled `device_id`s when a later milestone issues events).
pub fn prepare_m1_runtime_foundation(
    runtime_root: &Path,
) -> Result<InstallIdentity, IdentityRuntimeError> {
    let install = ensure_install_identity(runtime_root)?;
    let _store = open_sequence_store(runtime_root)?;
    Ok(install)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sequence_runtime::sequence_dir;
    use cryptex_core::CoreError;
    use std::sync::{Arc, Barrier};
    use std::thread;

    fn temp_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("cryptexd-install-{}", nanos));
        let _ = fs::remove_dir_all(&root);
        root
    }

    #[test]
    fn create_load_round_trip_and_stable_across_reload() {
        let root = temp_root();
        let id = InstallId::new("install-daemon-001").unwrap();
        let created = load_or_create_install_identity(&root, id.clone()).expect("create");
        assert_eq!(created.install_id, id);
        let loaded = load_install_identity(&root).expect("load");
        assert_eq!(loaded, created);
        // Second create-or-load must not replace the durable id.
        let other = InstallId::new("install-daemon-OTHER").unwrap();
        let again = load_or_create_install_identity(&root, other).expect("reload");
        assert_eq!(again.install_id, id);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn write_refuses_duplicate_install_identity() {
        let root = temp_root();
        let identity = InstallIdentity::new(InstallId::new("install-dup-1").unwrap()).unwrap();
        write_install_identity(&root, &identity).expect("first");
        let before = fs::read(install_identity_path(&root)).expect("read before");
        let err = write_install_identity(&root, &identity).expect_err("dup");
        assert!(matches!(err, IdentityRuntimeError::AlreadyExists(_)));
        let after = fs::read(install_identity_path(&root)).expect("read after");
        assert_eq!(
            before, after,
            "duplicate write must not change persisted bytes"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn load_fails_closed_on_future_schema() {
        let root = temp_root();
        fs::create_dir_all(root.join(IDENTITY_DIR)).unwrap();
        fs::write(
            install_identity_path(&root),
            r#"{
  "schema_version": 3,
  "install_id": "install-daemon-001"
}
"#,
        )
        .unwrap();
        let err = load_install_identity(&root).unwrap_err();
        assert!(matches!(
            err,
            IdentityRuntimeError::Core(CoreError::UnsupportedSchema {
                found: 3,
                expected: 1,
                ..
            })
        ));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn prepare_m1_runtime_foundation_binds_install_and_sequence_dir() {
        let root = temp_root();
        let first = prepare_m1_runtime_foundation(&root).expect("prepare");
        assert!(install_identity_path(&root).is_file());
        assert!(sequence_dir(&root).is_dir());
        let second = prepare_m1_runtime_foundation(&root).expect("reprepare");
        assert_eq!(first.install_id, second.install_id);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn ensure_install_identity_is_stable_across_calls() {
        let root = temp_root();
        let a = ensure_install_identity(&root).expect("a");
        let b = ensure_install_identity(&root).expect("b");
        assert_eq!(a.install_id, b.install_id);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn staging_write_failure_removes_partial_temp_and_preserves_primary_error() {
        let root = temp_root();
        fs::create_dir_all(root.join(IDENTITY_DIR)).expect("dir");
        let tmp = unique_install_tmp(&install_identity_path(&root));
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .expect("staging temp");

        let err = finish_staging_write(
            &tmp,
            file,
            b"partial identity",
            |file, bytes| {
                file.write_all(&bytes[..7])?;
                Err(io::Error::other("injected staging write failure"))
            },
            remove_file_if_present,
        )
        .expect_err("injected failure");

        assert!(err.to_string().contains("injected staging write failure"));
        assert!(!tmp.exists(), "failed staging file must be removed");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn staging_sync_failure_removes_temp_and_preserves_primary_error() {
        let root = temp_root();
        fs::create_dir_all(root.join(IDENTITY_DIR)).expect("dir");
        let tmp = unique_install_tmp(&install_identity_path(&root));
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .expect("staging temp");

        let err = finish_staging_write(
            &tmp,
            file,
            b"partial identity",
            |file, bytes| {
                file.write_all(bytes)?;
                Err(io::Error::other("injected staging sync failure"))
            },
            remove_file_if_present,
        )
        .expect_err("injected failure");

        assert!(err.to_string().contains("injected staging sync failure"));
        assert!(!tmp.exists(), "failed staging file must be removed");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn staging_cleanup_failure_preserves_both_errors_and_forbids_retry() {
        let root = temp_root();
        fs::create_dir_all(root.join(IDENTITY_DIR)).expect("dir");
        let tmp = unique_install_tmp(&install_identity_path(&root));
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .expect("staging temp");

        let err = finish_staging_write(
            &tmp,
            file,
            b"partial identity",
            |file, bytes| {
                file.write_all(bytes)?;
                Err(io::Error::other("injected staging write failure"))
            },
            |_| {
                Err(io::Error::new(
                    ErrorKind::PermissionDenied,
                    "injected cleanup failure",
                ))
            },
        )
        .expect_err("injected failure");

        let message = err.to_string();
        assert!(
            message.contains("injected staging write failure"),
            "{message}"
        );
        assert!(message.contains("injected cleanup failure"), "{message}");
        assert_eq!(err.whole_operation_retry_safe(), Some(false));
        assert!(
            tmp.exists(),
            "fixture confirms cleanup failure was injected"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn sync_parent_directory_succeeds_for_existing_parent() {
        let root = temp_root();
        fs::create_dir_all(&root).expect("mkdir");
        let child = root.join("install_identity.json");
        fs::write(&child, b"{}\n").expect("touch");
        sync_parent_directory(&child).expect("parent sync must succeed");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn sync_parent_directory_fails_without_parent() {
        let err = sync_parent_directory(Path::new("/")).expect_err("root has no parent");
        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert!(
            err.to_string().contains("no parent directory"),
            "error must be actionable: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn unix_write_install_identity_requires_durable_directory_entry() {
        let root = temp_root();
        let identity =
            InstallIdentity::new(InstallId::new("install-unix-durable-001").unwrap()).unwrap();
        write_install_identity(&root, &identity).expect("create must sync parent then commit");
        let loaded = load_install_identity(&root).expect("load committed identity");
        assert_eq!(loaded.install_id, identity.install_id);
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn unix_injected_parent_sync_failure_with_successful_rollback_sync() {
        use std::cell::Cell;

        let root = temp_root();
        fs::create_dir_all(root.join(IDENTITY_DIR)).expect("identity dir");
        let dest = install_identity_path(&root);
        let tmp = unique_install_tmp(&dest);
        fs::write(
            &tmp,
            br#"{
  "schema_version": 1,
  "install_id": "install-sync-fail-001"
}
"#,
        )
        .expect("tmp");
        let sync_count = Cell::new(0);

        let err = place_new_file_no_replace_unix(
            &tmp,
            &dest,
            |_| {
                sync_count.set(sync_count.get() + 1);
                if sync_count.get() == 1 {
                    Err(io::Error::other("injected parent sync failure"))
                } else {
                    Ok(())
                }
            },
            remove_uncommitted_dest,
            remove_file_if_present,
        )
        .expect_err("sync failure must surface");

        assert!(
            err.to_string().contains("injected parent sync failure"),
            "sync error must be returned: {err}"
        );
        assert!(
            !matches!(err, PlacementError::CleanupFailed { .. }),
            "successful rollback sync must not be CleanupFailed: {err}"
        );
        assert_eq!(sync_count.get(), 2, "rollback must sync parent again");
        assert!(
            !dest.exists(),
            "destination link must be removed after sync failure"
        );
        assert!(
            !tmp.exists(),
            "staging link is removed before commit attempt"
        );
        assert!(
            load_install_identity(&root).is_err(),
            "identity must not be loadable as committed after sync failure"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn unix_injected_rollback_sync_failure_is_typed_non_retryable() {
        use std::cell::Cell;

        let root = temp_root();
        fs::create_dir_all(root.join(IDENTITY_DIR)).expect("identity dir");
        let dest = install_identity_path(&root);
        let tmp = unique_install_tmp(&dest);
        fs::write(&tmp, b"payload\n").expect("tmp");
        let sync_count = Cell::new(0);

        let placement_error = place_new_file_no_replace_unix(
            &tmp,
            &dest,
            |_| {
                sync_count.set(sync_count.get() + 1);
                if sync_count.get() == 1 {
                    Err(io::Error::other("injected parent sync failure"))
                } else {
                    Err(io::Error::other("injected rollback sync failure"))
                }
            },
            remove_uncommitted_dest,
            remove_file_if_present,
        )
        .expect_err("rollback sync failure");

        let err = placement_error_to_identity_error(placement_error, &tmp);
        let message = err.to_string();
        assert!(
            message.contains("injected parent sync failure"),
            "{message}"
        );
        assert!(
            message.contains("injected rollback sync failure"),
            "{message}"
        );
        assert_eq!(err.whole_operation_retry_safe(), Some(false));
        assert!(!dest.exists(), "destination must still be removed");
        assert_eq!(sync_count.get(), 2);
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn unix_destination_cleanup_failure_is_typed_non_retryable() {
        use std::cell::Cell;

        let root = temp_root();
        fs::create_dir_all(root.join(IDENTITY_DIR)).expect("identity dir");
        let dest = install_identity_path(&root);
        let tmp = unique_install_tmp(&dest);
        fs::write(&tmp, b"payload\n").expect("tmp");
        let sync_count = Cell::new(0);

        let placement_error = place_new_file_no_replace_unix(
            &tmp,
            &dest,
            |_| {
                sync_count.set(sync_count.get() + 1);
                if sync_count.get() == 1 {
                    Err(io::Error::other("injected parent sync failure"))
                } else {
                    Ok(())
                }
            },
            |_| {
                Err(io::Error::new(
                    ErrorKind::PermissionDenied,
                    "injected dest cleanup failure",
                ))
            },
            remove_file_if_present,
        )
        .expect_err("compound failure");

        let err = placement_error_to_identity_error(placement_error, &tmp);
        let message = err.to_string();
        assert!(
            message.contains("injected parent sync failure"),
            "must report sync failure: {message}"
        );
        assert!(
            message.contains("injected dest cleanup failure"),
            "must report cleanup failure: {message}"
        );
        assert_eq!(err.whole_operation_retry_safe(), Some(false));
        // Cleanup hook skipped real remove; dest from hard_link may still exist.
        assert!(
            dest.exists(),
            "fixture leaves dest when cleanup hook fails (proves reporting path)"
        );
        let _ = fs::remove_file(&dest);
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn unix_staging_link_cleanup_failure_removes_destination_and_is_reported() {
        let root = temp_root();
        fs::create_dir_all(root.join(IDENTITY_DIR)).expect("identity dir");
        let dest = install_identity_path(&root);
        let tmp = unique_install_tmp(&dest);
        fs::write(&tmp, b"payload\n").expect("tmp");

        let err = place_new_file_no_replace_unix(
            &tmp,
            &dest,
            sync_parent_directory,
            remove_uncommitted_dest,
            |_| {
                Err(io::Error::new(
                    ErrorKind::PermissionDenied,
                    "injected temp cleanup",
                ))
            },
        )
        .expect_err("staging link removal fails");

        assert!(err.to_string().contains("injected temp cleanup"));
        assert!(
            !dest.exists(),
            "failed placement must remove destination link"
        );
        assert!(
            tmp.exists(),
            "the injected cleanup failure leaves the staging link"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn concurrent_first_create_all_observe_same_install_id() {
        let root = Arc::new(temp_root());
        let n = 16;
        let barrier = Arc::new(Barrier::new(n));
        let mut handles = Vec::with_capacity(n);
        for i in 0..n {
            let root = Arc::clone(&root);
            let barrier = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier.wait();
                // Mix ensure and load_or_create with distinct candidate ids.
                if i % 2 == 0 {
                    ensure_install_identity(&root)
                } else {
                    let candidate =
                        InstallId::new(format!("install-candidate-{i}-{}", std::process::id()))
                            .expect("candidate id");
                    load_or_create_install_identity(&root, candidate)
                }
            }));
        }

        let mut observed = Vec::with_capacity(n);
        for handle in handles {
            let identity = handle.join().expect("thread").expect("create-or-load");
            observed.push(identity.install_id);
        }

        let winner = observed[0].clone();
        for id in &observed {
            assert_eq!(
                id, &winner,
                "every successful concurrent caller must observe the same install_id"
            );
        }

        let path = install_identity_path(&root);
        let persisted_bytes = fs::read(&path).expect("read persisted");
        let persisted = load_install_identity(&root).expect("load persisted");
        assert_eq!(persisted.install_id, winner);
        assert_eq!(
            fs::read(&path).expect("re-read persisted"),
            persisted_bytes,
            "persisted install identity file must remain unchanged"
        );

        // Explicit duplicate write still fails and must not mutate the winner.
        let rogue =
            InstallIdentity::new(InstallId::new("install-rogue-overwrite").unwrap()).unwrap();
        let err = write_install_identity(&root, &rogue).expect_err("dup write");
        assert!(matches!(err, IdentityRuntimeError::AlreadyExists(_)));
        assert_eq!(
            fs::read(&path).expect("read after refused write"),
            persisted_bytes,
            "refused duplicate write must leave persisted bytes unchanged"
        );

        let _ = fs::remove_dir_all(&*root);
    }
}
