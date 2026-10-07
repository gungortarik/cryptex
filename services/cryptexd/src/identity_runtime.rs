//! M1 daemon wiring: public Brain metadata on disk + private material via SecureStore.
//!
//! Public files never contain secret bytes. Private blobs go only through [`SecureStore`].

use std::fs::{self, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use cryptex_core::{BrainIdentity, CoreError};

use crate::error::LifecycleError;
use crate::paths::{assert_outside_repository, ensure_runtime_root};
use crate::secure_store::{
    AccountLabel, SecureStore, SecureStoreEnvironment, SecureStoreError, ServiceLabel,
};

const IDENTITY_DIR: &str = "identity";
const BRAIN_FILE: &str = "brain_identity.json";

static BRAIN_TMP_SEQ: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub enum IdentityRuntimeError {
    Lifecycle(LifecycleError),
    Core(CoreError),
    SecureStore(SecureStoreError),
    AlreadyExists(String),
    /// A failed persistence step could not complete its cleanup cleanly.
    /// The primary failure and cleanup failure are both retained.
    PersistenceCleanupFailed {
        primary: Box<IdentityRuntimeError>,
        cleanup_error: String,
    },
    /// Primary create/verify path failed and at least one compensating cleanup also failed.
    ///
    /// Explicit cleanup outcome flags are provided so callers can see which step
    /// succeeded. Whole-operation retry is never safe for this variant: if this
    /// error is returned, at least one cleanup failed.
    CompensatingCleanupFailed {
        primary: Box<IdentityRuntimeError>,
        public_rollback_succeeded: bool,
        private_cleanup_succeeded: bool,
        public_rollback_error: Option<Box<IdentityRuntimeError>>,
        private_cleanup_error: Option<Box<IdentityRuntimeError>>,
    },
}

impl std::fmt::Display for IdentityRuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Lifecycle(e) => write!(f, "{e}"),
            Self::Core(e) => write!(f, "{e}"),
            Self::SecureStore(e) => write!(f, "{e}"),
            Self::AlreadyExists(path) => write!(f, "identity already exists at {path}"),
            Self::PersistenceCleanupFailed {
                primary,
                cleanup_error,
            } => write!(
                f,
                "{primary}; persistence cleanup also failed: {cleanup_error}; inspect the metadata location before retrying"
            ),
            Self::CompensatingCleanupFailed {
                primary,
                public_rollback_succeeded,
                private_cleanup_succeeded,
                public_rollback_error,
                private_cleanup_error,
            } => {
                write!(f, "{primary}")?;
                if let Some(err) = public_rollback_error {
                    write!(f, "; public identity rollback failed: {err}")?;
                } else if *public_rollback_succeeded {
                    write!(f, "; public identity rollback succeeded")?;
                }
                if let Some(err) = private_cleanup_error {
                    write!(
                        f,
                        "; private SecureStore cleanup failed (orphaned credential may remain): {err}"
                    )?;
                } else if *private_cleanup_succeeded {
                    write!(
                        f,
                        "; private SecureStore cleanup succeeded or was not required"
                    )?;
                }
                write!(
                    f,
                    "; whole-operation retry is not safe after compensating cleanup failure"
                )
            }
        }
    }
}

impl std::error::Error for IdentityRuntimeError {}

impl From<LifecycleError> for IdentityRuntimeError {
    fn from(value: LifecycleError) -> Self {
        Self::Lifecycle(value)
    }
}

impl From<CoreError> for IdentityRuntimeError {
    fn from(value: CoreError) -> Self {
        Self::Core(value)
    }
}

impl From<SecureStoreError> for IdentityRuntimeError {
    fn from(value: SecureStoreError) -> Self {
        Self::SecureStore(value)
    }
}

impl From<std::io::Error> for IdentityRuntimeError {
    fn from(value: std::io::Error) -> Self {
        Self::Lifecycle(LifecycleError::Io(value.to_string()))
    }
}

impl IdentityRuntimeError {
    /// Map into a lifecycle error for binary entrypoints.
    pub fn into_lifecycle(self) -> LifecycleError {
        match self {
            Self::Lifecycle(inner) => inner,
            other => LifecycleError::Attention(other.to_string()),
        }
    }

    /// Whether retrying the entire Brain create is safe after this error.
    ///
    /// Returns `Some(false)` for errors that include a failed cleanup —
    /// that variant means at least one cleanup failed, so whole-operation
    /// retry must not be implied. Other variants return `None` (no cleanup
    /// outcome encoded; e.g. a clean SecureStore put failure after successful
    /// public rollback is just `SecureStore` and may be retried).
    pub fn whole_operation_retry_safe(&self) -> Option<bool> {
        match self {
            Self::CompensatingCleanupFailed { .. } | Self::PersistenceCleanupFailed { .. } => {
                Some(false)
            }
            _ => None,
        }
    }
}

pub type IdentityRuntimeResult<T> = Result<T, IdentityRuntimeError>;

pub fn identity_dir(runtime_root: &Path) -> PathBuf {
    runtime_root.join(IDENTITY_DIR)
}

pub fn brain_identity_path(runtime_root: &Path) -> PathBuf {
    identity_dir(runtime_root).join(BRAIN_FILE)
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
) -> IdentityRuntimeResult<()>
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

fn unique_brain_tmp(path: &Path) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seq = BRAIN_TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    path.with_extension(format!("json.tmp.{}-{}-{}", std::process::id(), seq, nanos))
}

/// Sync the directory that contains `path` so a new directory entry is durable (Unix).
#[cfg(any(unix, test))]
fn sync_parent_directory(path: &Path) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            ErrorKind::InvalidInput,
            "brain identity path has no parent directory",
        )
    })?;
    let dir = fs::File::open(parent)?;
    dir.sync_all()?;
    Ok(())
}

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
/// - Windows: `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING`.
/// - Unix: `hard_link` + parent-directory sync before success.
fn place_new_file_no_replace(tmp: &Path, dest: &Path) -> Result<(), PlacementError> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;

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
                "exclusive brain-identity create is unsupported on this platform",
            ),
            dest,
        ))
    }
}

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

/// Remove a just-written brain identity (and staging tmp files) so a failed create can retry.
///
/// Missing files are success. Any other removal error is returned and must not be ignored.
fn rollback_brain_identity(runtime_root: &Path) -> IdentityRuntimeResult<()> {
    let path = brain_identity_path(runtime_root);
    let mut first_err: Option<io::Error> = None;
    if let Err(e) = remove_file_if_present(&path) {
        first_err = Some(e);
    }
    // Remove legacy `.json.tmp` and unique `.json.tmp.*` staging names.
    let dir = identity_dir(runtime_root);
    match fs::read_dir(&dir) {
        Ok(entries) => {
            for entry in entries {
                match entry {
                    Ok(entry) => {
                        let name = entry.file_name();
                        let name = name.to_string_lossy();
                        if (name == "brain_identity.json.tmp"
                            || name.starts_with("brain_identity.json.tmp."))
                            && let Err(e) = remove_file_if_present(&entry.path())
                            && first_err.is_none()
                        {
                            first_err = Some(e);
                        }
                    }
                    Err(e) => {
                        if first_err.is_none() {
                            first_err = Some(e);
                        }
                        break;
                    }
                }
            }
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => {
            if first_err.is_none() {
                first_err = Some(e);
            }
        }
    }
    match first_err {
        Some(e) => Err(e.into()),
        None => Ok(()),
    }
}

fn delete_device_private<S: SecureStore>(
    store: &S,
    env: SecureStoreEnvironment,
    device_id: &str,
) -> IdentityRuntimeResult<()> {
    let service = ServiceLabel::device_credentials(env);
    let account = AccountLabel::device_private(device_id);
    store.delete(service.as_str(), account.as_str())?;
    Ok(())
}

/// Attach cleanup outcome to a primary failure.
///
/// Whole-operation retry is never implied when any compensating cleanup fails.
/// Both public and private cleanup errors are preserved when present.
fn with_cleanup_outcome(
    primary: IdentityRuntimeError,
    rollback: IdentityRuntimeResult<()>,
    private_delete: Option<IdentityRuntimeResult<()>>,
) -> IdentityRuntimeError {
    let (public_rollback_succeeded, public_rollback_error) = match rollback {
        Ok(()) => (true, None),
        Err(e) => (false, Some(Box::new(e))),
    };
    // When private delete was not attempted (e.g. put never succeeded), treat
    // private cleanup as succeeded/not-required - there is no orphaned put.
    let (private_cleanup_succeeded, private_cleanup_error) = match private_delete {
        None | Some(Ok(())) => (true, None),
        Some(Err(e)) => (false, Some(Box::new(e))),
    };

    if public_rollback_succeeded && private_cleanup_succeeded {
        return primary;
    }

    IdentityRuntimeError::CompensatingCleanupFailed {
        primary: Box::new(primary),
        public_rollback_succeeded,
        private_cleanup_succeeded,
        public_rollback_error,
        private_cleanup_error,
    }
}

/// Write public brain identity JSON (LF) under app-data runtime root.
///
/// First-create is exclusive/no-replace across Windows and Unix so concurrent
/// writers cannot overwrite a winning `brain_id` (ADR-0015 section 2.1).
pub fn write_brain_identity(
    runtime_root: &Path,
    identity: &BrainIdentity,
) -> IdentityRuntimeResult<()> {
    assert_outside_repository(runtime_root)?;
    ensure_runtime_root(runtime_root)?;
    let dir = identity_dir(runtime_root);
    fs::create_dir_all(&dir)?;
    let path = brain_identity_path(runtime_root);
    if path.exists() {
        return Err(IdentityRuntimeError::AlreadyExists(
            path.display().to_string(),
        ));
    }

    let json = identity.to_json_lf()?;
    let tmp = unique_brain_tmp(&path);
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

pub fn load_brain_identity(runtime_root: &Path) -> IdentityRuntimeResult<BrainIdentity> {
    let path = brain_identity_path(runtime_root);
    let text = fs::read_to_string(&path)?;
    Ok(BrainIdentity::from_json(&text)?)
}

/// Store synthetic device private material via SecureStore only (never in public files).
pub fn store_device_private_material<S: SecureStore>(
    store: &S,
    env: SecureStoreEnvironment,
    device_id: &str,
    secret: &[u8],
) -> IdentityRuntimeResult<()> {
    let service = ServiceLabel::device_credentials(env);
    let account = AccountLabel::device_private(device_id);
    store.put(service.as_str(), account.as_str(), secret)?;
    Ok(())
}

pub fn load_device_private_material<S: SecureStore>(
    store: &S,
    env: SecureStoreEnvironment,
    device_id: &str,
) -> IdentityRuntimeResult<Vec<u8>> {
    let service = ServiceLabel::device_credentials(env);
    let account = AccountLabel::device_private(device_id);
    Ok(store.get(service.as_str(), account.as_str())?)
}

/// Create a synthetic Brain: public metadata on disk + private blob in SecureStore.
///
/// If SecureStore write fails after the public identity was persisted, the identity
/// file is rolled back so a later retry is not blocked by `AlreadyExists`.
/// Rollback and compensating SecureStore delete errors are surfaced as
/// [`IdentityRuntimeError::CompensatingCleanupFailed`]. That variant never
/// implies whole-operation retry is safe.
pub fn create_synthetic_brain<S: SecureStore>(
    runtime_root: &Path,
    store: &S,
    env: SecureStoreEnvironment,
    brain_id: &str,
    device_id: &str,
    created_at: &str,
    device_secret: &[u8],
) -> IdentityRuntimeResult<BrainIdentity> {
    let identity = BrainIdentity::new(brain_id, created_at)?;
    write_brain_identity(runtime_root, &identity)?;
    if let Err(err) = store_device_private_material(store, env, device_id, device_secret) {
        return Err(with_cleanup_outcome(
            err,
            rollback_brain_identity(runtime_root),
            None,
        ));
    }
    // Verify public file has no secret bytes.
    let on_disk = match fs::read_to_string(brain_identity_path(runtime_root)) {
        Ok(text) => text,
        Err(e) => {
            return Err(with_cleanup_outcome(
                e.into(),
                rollback_brain_identity(runtime_root),
                Some(delete_device_private(store, env, device_id)),
            ));
        }
    };
    if crate::secure_store::payload_bytes_recoverable(device_secret, &on_disk) {
        return Err(with_cleanup_outcome(
            IdentityRuntimeError::Lifecycle(LifecycleError::Attention(
                "public brain identity file must not contain device secret bytes".into(),
            )),
            rollback_brain_identity(runtime_root),
            Some(delete_device_private(store, env, device_id)),
        ));
    }
    Ok(identity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secure_store::{FakeSecureStore, SecureStoreResult};
    use std::sync::{Arc, Barrier, Mutex};
    use std::thread;

    fn temp_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("cryptexd-identity-{}", nanos));
        let _ = fs::remove_dir_all(&root);
        root
    }

    /// On put: replace the public brain file with a directory so rollback cannot remove it.
    struct ReplaceBrainWithDirOnPut {
        inner: FakeSecureStore,
        root: PathBuf,
        put_err: Mutex<Option<SecureStoreError>>,
    }

    impl SecureStore for ReplaceBrainWithDirOnPut {
        fn put(&self, service: &str, account: &str, secret: &[u8]) -> SecureStoreResult<()> {
            let path = brain_identity_path(&self.root);
            let _ = fs::remove_file(&path);
            fs::create_dir(&path).map_err(|e| SecureStoreError::Platform {
                op: "test_replace_brain_with_dir",
                code: i64::from(e.raw_os_error().unwrap_or(-1)),
            })?;
            if let Some(err) = self.put_err.lock().expect("latch").take() {
                return Err(err);
            }
            self.inner.put(service, account, secret)
        }

        fn get(&self, service: &str, account: &str) -> SecureStoreResult<Vec<u8>> {
            self.inner.get(service, account)
        }

        fn delete(&self, service: &str, account: &str) -> SecureStoreResult<()> {
            self.inner.delete(service, account)
        }
    }

    /// After a successful put, append secret bytes into the public brain file so verify fails.
    struct LeakSecretIntoPublicOnPut {
        inner: FakeSecureStore,
        root: PathBuf,
        secret: Vec<u8>,
    }

    impl SecureStore for LeakSecretIntoPublicOnPut {
        fn put(&self, service: &str, account: &str, secret: &[u8]) -> SecureStoreResult<()> {
            self.inner.put(service, account, secret)?;
            let path = brain_identity_path(&self.root);
            let mut text = fs::read_to_string(&path).map_err(|e| SecureStoreError::Platform {
                op: "test_read_brain_for_leak",
                code: i64::from(e.raw_os_error().unwrap_or(-1)),
            })?;
            text.push_str(&String::from_utf8_lossy(&self.secret));
            fs::write(&path, text).map_err(|e| SecureStoreError::Platform {
                op: "test_leak_secret_into_brain",
                code: i64::from(e.raw_os_error().unwrap_or(-1)),
            })?;
            Ok(())
        }

        fn get(&self, service: &str, account: &str) -> SecureStoreResult<Vec<u8>> {
            self.inner.get(service, account)
        }

        fn delete(&self, service: &str, account: &str) -> SecureStoreResult<()> {
            self.inner.delete(service, account)
        }
    }

    #[test]
    fn synthetic_brain_keeps_secret_out_of_public_metadata() {
        let root = temp_root();
        let store = FakeSecureStore::new();
        let secret = b"SYNTHETIC_BRAIN_DEVICE_SECRET_NOT_REAL::v1";
        let identity = create_synthetic_brain(
            &root,
            &store,
            SecureStoreEnvironment::Dev,
            "11111111-1111-4111-8111-111111111111",
            "dev-device-001",
            "2026-10-06T00:00:00Z",
            secret,
        )
        .expect("create");

        let loaded = load_brain_identity(&root).expect("load public");
        assert_eq!(loaded, identity);

        let got =
            load_device_private_material(&store, SecureStoreEnvironment::Dev, "dev-device-001")
                .expect("load private");
        assert_eq!(got, secret);

        let public = fs::read_to_string(brain_identity_path(&root)).expect("read public");
        assert!(!public.contains("SYNTHETIC_BRAIN_DEVICE_SECRET_NOT_REAL"));
        assert!(!crate::secure_store::payload_bytes_recoverable(
            secret, &public
        ));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn sync_parent_directory_fails_without_parent() {
        let err = sync_parent_directory(Path::new("/")).expect_err("root has no parent");
        assert_eq!(err.kind(), ErrorKind::InvalidInput);
    }

    #[test]
    fn staging_write_failure_removes_partial_temp_and_preserves_primary_error() {
        let root = temp_root();
        fs::create_dir_all(identity_dir(&root)).expect("dir");
        let tmp = unique_brain_tmp(&brain_identity_path(&root));
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
        fs::create_dir_all(identity_dir(&root)).expect("dir");
        let tmp = unique_brain_tmp(&brain_identity_path(&root));
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
    fn unix_injected_parent_sync_failure_does_not_commit_brain() {
        let root = temp_root();
        fs::create_dir_all(identity_dir(&root)).expect("dir");
        let dest = brain_identity_path(&root);
        let tmp = unique_brain_tmp(&dest);
        let identity = BrainIdentity::new(
            "11111111-1111-4111-8111-111111111111",
            "2026-10-06T00:00:00Z",
        )
        .unwrap();
        fs::write(&tmp, identity.to_json_lf().unwrap().as_bytes()).expect("tmp");
        let err = place_new_file_no_replace_unix(
            &tmp,
            &dest,
            |_| Err(io::Error::other("injected parent sync failure")),
            remove_uncommitted_dest,
            remove_file_if_present,
        )
        .expect_err("sync fail");
        assert!(err.to_string().contains("injected parent sync failure"));
        assert!(!dest.exists(), "dest must be removed");
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn unix_staging_link_cleanup_failure_removes_destination_and_is_reported() {
        let root = temp_root();
        fs::create_dir_all(identity_dir(&root)).expect("dir");
        let dest = brain_identity_path(&root);
        let tmp = unique_brain_tmp(&dest);
        fs::write(&tmp, b"identity").expect("tmp");

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

    #[cfg(unix)]
    #[test]
    fn unix_destination_rollback_failure_is_typed_non_retryable() {
        use std::cell::Cell;

        let root = temp_root();
        fs::create_dir_all(identity_dir(&root)).expect("dir");
        let dest = brain_identity_path(&root);
        let tmp = unique_brain_tmp(&dest);
        fs::write(&tmp, b"identity").expect("tmp");
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
                    "injected destination rollback failure",
                ))
            },
            remove_file_if_present,
        )
        .expect_err("destination rollback failure");

        let err = placement_error_to_identity_error(placement_error, &tmp);
        let message = err.to_string();
        assert!(
            message.contains("injected parent sync failure"),
            "{message}"
        );
        assert!(
            message.contains("injected destination rollback failure"),
            "{message}"
        );
        assert_eq!(err.whole_operation_retry_safe(), Some(false));
        assert!(dest.exists(), "fixture confirms rollback failure");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn concurrent_first_create_brain_identity_single_winner() {
        let root = Arc::new(temp_root());
        let n = 16;
        let barrier = Arc::new(Barrier::new(n));
        let mut handles = Vec::with_capacity(n);
        for i in 0..n {
            let root = Arc::clone(&root);
            let barrier = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier.wait();
                let brain_id = format!("11111111-1111-4111-8111-{:012x}", i);
                let identity =
                    BrainIdentity::new(brain_id, "2026-10-06T00:00:00Z").expect("identity");
                write_brain_identity(&root, &identity)
            }));
        }

        let mut wins = 0usize;
        let mut already = 0usize;
        for handle in handles {
            match handle.join().expect("thread") {
                Ok(()) => wins += 1,
                Err(IdentityRuntimeError::AlreadyExists(_)) => already += 1,
                Err(other) => panic!("unexpected error: {other:?}"),
            }
        }
        assert_eq!(wins, 1, "exactly one first-create must win");
        assert_eq!(already, n - 1, "losers must see AlreadyExists");

        let persisted = load_brain_identity(&root).expect("load winner");
        let path = brain_identity_path(&root);
        let before = fs::read(&path).expect("read bytes");
        let rogue = BrainIdentity::new(
            "22222222-2222-4222-8222-222222222222",
            "2026-10-06T00:00:01Z",
        )
        .unwrap();
        let err = write_brain_identity(&root, &rogue).expect_err("dup");
        assert!(matches!(err, IdentityRuntimeError::AlreadyExists(_)));
        assert_eq!(
            fs::read(&path).expect("re-read"),
            before,
            "failed concurrent/duplicate create must not change persisted brain identity"
        );
        assert_eq!(
            load_brain_identity(&root).expect("reload").brain_id,
            persisted.brain_id
        );

        let _ = fs::remove_dir_all(&*root);
    }

    #[test]
    fn write_brain_identity_failure_path_leaves_no_partial_winner_bytes() {
        let root = temp_root();
        let first = BrainIdentity::new(
            "33333333-3333-4333-8333-333333333333",
            "2026-10-06T00:00:00Z",
        )
        .unwrap();
        write_brain_identity(&root, &first).expect("first");
        let before = fs::read(brain_identity_path(&root)).expect("bytes");

        // Simulate a racing loser after winner exists: must fail closed without mutation.
        let second = BrainIdentity::new(
            "44444444-4444-4444-8444-444444444444",
            "2026-10-06T00:00:01Z",
        )
        .unwrap();
        let err = write_brain_identity(&root, &second).expect_err("second");
        assert!(matches!(err, IdentityRuntimeError::AlreadyExists(_)));
        assert_eq!(fs::read(brain_identity_path(&root)).expect("bytes"), before);
        assert_eq!(
            load_brain_identity(&root).expect("load").brain_id,
            first.brain_id
        );

        // Staging leftovers must not remain after exclusive create attempts.
        let dir = identity_dir(&root);
        for entry in fs::read_dir(&dir).expect("read dir") {
            let entry = entry.expect("entry");
            let name = entry.file_name();
            let name = name.to_string_lossy();
            assert!(
                !name.contains(".tmp"),
                "no staging tmp should remain: {name}"
            );
        }

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn refuses_duplicate_brain_identity_file() {
        let root = temp_root();
        let store = FakeSecureStore::new();
        let secret = b"SYNTHETIC_BRAIN_DEVICE_SECRET_NOT_REAL::dup";
        create_synthetic_brain(
            &root,
            &store,
            SecureStoreEnvironment::Dev,
            "22222222-2222-4222-8222-222222222222",
            "dev-device-002",
            "2026-10-06T00:00:00Z",
            secret,
        )
        .expect("first");
        let err = create_synthetic_brain(
            &root,
            &store,
            SecureStoreEnvironment::Dev,
            "33333333-3333-4333-8333-333333333333",
            "dev-device-003",
            "2026-10-06T00:00:01Z",
            secret,
        )
        .expect_err("duplicate public file");
        assert!(matches!(err, IdentityRuntimeError::AlreadyExists(_)));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn secure_store_put_failure_rolls_back_brain_identity_for_retry() {
        let root = temp_root();
        let store = FakeSecureStore::new();
        store.fail_next_put_with(SecureStoreError::Platform {
            op: "injected_put_failure",
            code: 99,
        });
        let secret = b"SYNTHETIC_BRAIN_DEVICE_SECRET_NOT_REAL::fail";
        let err = create_synthetic_brain(
            &root,
            &store,
            SecureStoreEnvironment::Dev,
            "44444444-4444-4444-8444-444444444444",
            "dev-device-fail",
            "2026-10-06T00:00:00Z",
            secret,
        )
        .expect_err("secure store failure");
        assert!(matches!(
            err,
            IdentityRuntimeError::SecureStore(SecureStoreError::Platform {
                op: "injected_put_failure",
                code: 99
            })
        ));
        assert!(
            err.whole_operation_retry_safe().is_none(),
            "clean put failure after successful rollback is not CompensatingCleanupFailed"
        );
        assert!(
            !brain_identity_path(&root).exists(),
            "failed create must not leave brain identity that blocks retry"
        );
        assert!(store.is_empty());

        // Retry succeeds after rollback.
        let identity = create_synthetic_brain(
            &root,
            &store,
            SecureStoreEnvironment::Dev,
            "44444444-4444-4444-8444-444444444444",
            "dev-device-fail",
            "2026-10-06T00:00:00Z",
            secret,
        )
        .expect("retry after rollback");
        assert_eq!(identity.brain_id, "44444444-4444-4444-8444-444444444444");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn put_failure_when_rollback_fails_is_not_retry_safe() {
        let root = temp_root();
        let store = ReplaceBrainWithDirOnPut {
            inner: FakeSecureStore::new(),
            root: root.clone(),
            put_err: Mutex::new(Some(SecureStoreError::Platform {
                op: "injected_put_failure",
                code: 99,
            })),
        };
        let secret = b"SYNTHETIC_BRAIN_DEVICE_SECRET_NOT_REAL::rollback_fail";
        let err = create_synthetic_brain(
            &root,
            &store,
            SecureStoreEnvironment::Dev,
            "55555555-5555-4555-8555-555555555555",
            "dev-device-rollback-fail",
            "2026-10-06T00:00:00Z",
            secret,
        )
        .expect_err("put + rollback failure");

        match &err {
            IdentityRuntimeError::CompensatingCleanupFailed {
                primary,
                public_rollback_succeeded,
                private_cleanup_succeeded,
                public_rollback_error,
                private_cleanup_error,
            } => {
                assert!(matches!(
                    primary.as_ref(),
                    IdentityRuntimeError::SecureStore(SecureStoreError::Platform {
                        op: "injected_put_failure",
                        code: 99
                    })
                ));
                assert!(
                    !*public_rollback_succeeded,
                    "public rollback must be reported as failed"
                );
                assert!(
                    *private_cleanup_succeeded,
                    "private cleanup was not required after put failure"
                );
                assert!(matches!(
                    public_rollback_error.as_deref(),
                    Some(IdentityRuntimeError::Lifecycle(LifecycleError::Io(_)))
                ));
                assert!(private_cleanup_error.is_none());
            }
            other => panic!("expected CompensatingCleanupFailed, got {other:?}"),
        }
        assert_eq!(err.whole_operation_retry_safe(), Some(false));
        let display = format!("{err}");
        assert!(
            display.contains("public identity rollback failed"),
            "display must surface public rollback failure: {display}"
        );
        assert!(
            display.contains("whole-operation retry is not safe"),
            "display must not imply whole-operation retry is safe: {display}"
        );
        assert!(
            !display.to_lowercase().contains("retry is safe"),
            "must never claim retry is safe: {display}"
        );
        assert!(
            brain_identity_path(&root).is_dir(),
            "blocking path should remain when rollback fails"
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn secret_leak_verify_delete_failure_preserves_primary_and_surfaces_cleanup() {
        let root = temp_root();
        let inner = FakeSecureStore::new();
        inner.fail_next_delete_with(SecureStoreError::Platform {
            op: "injected_delete_failure",
            code: 77,
        });
        let secret = b"SYNTHETIC_BRAIN_DEVICE_SECRET_NOT_REAL::leak";
        let store = LeakSecretIntoPublicOnPut {
            inner,
            root: root.clone(),
            secret: secret.to_vec(),
        };
        let err = create_synthetic_brain(
            &root,
            &store,
            SecureStoreEnvironment::Dev,
            "66666666-6666-4666-8666-666666666666",
            "dev-device-leak",
            "2026-10-06T00:00:00Z",
            secret,
        )
        .expect_err("verify + delete failure");

        match &err {
            IdentityRuntimeError::CompensatingCleanupFailed {
                primary,
                public_rollback_succeeded,
                private_cleanup_succeeded,
                public_rollback_error,
                private_cleanup_error,
            } => {
                assert!(matches!(
                    primary.as_ref(),
                    IdentityRuntimeError::Lifecycle(LifecycleError::Attention(msg))
                        if msg.contains("must not contain device secret")
                ));
                assert!(
                    *public_rollback_succeeded,
                    "public rollback should succeed in this injection"
                );
                assert!(
                    !*private_cleanup_succeeded,
                    "private cleanup must be reported as failed"
                );
                assert!(public_rollback_error.is_none());
                assert!(matches!(
                    private_cleanup_error.as_deref(),
                    Some(IdentityRuntimeError::SecureStore(
                        SecureStoreError::Platform {
                            op: "injected_delete_failure",
                            code: 77
                        }
                    ))
                ));
            }
            other => panic!("expected CompensatingCleanupFailed, got {other:?}"),
        }
        assert_eq!(
            err.whole_operation_retry_safe(),
            Some(false),
            "orphaned private credential must make whole-operation retry unsafe"
        );
        assert!(
            !brain_identity_path(&root).exists(),
            "public identity must still be rolled back"
        );
        // Private material remains because delete failed.
        assert_eq!(store.inner.len(), 1);
        let display = format!("{err}");
        assert!(
            display.contains("orphaned credential may remain"),
            "display must clearly report orphaned credential: {display}"
        );
        assert!(
            display.contains("injected_delete_failure")
                || display.contains("private SecureStore cleanup failed"),
            "display must surface private cleanup failure: {display}"
        );
        assert!(
            display.contains("whole-operation retry is not safe"),
            "display must not imply whole-operation retry is safe: {display}"
        );
        assert!(
            !display.to_lowercase().contains("retry is safe"),
            "must never claim retry is safe: {display}"
        );

        let _ = fs::remove_dir_all(&root);
    }
}
