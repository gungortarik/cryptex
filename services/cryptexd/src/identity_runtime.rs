//! M1 daemon wiring: public Brain metadata on disk + private material via SecureStore.
//!
//! Public files never contain secret bytes. Private blobs go only through [`SecureStore`].

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use cryptex_core::{BrainIdentity, CoreError};

use crate::error::LifecycleError;
use crate::paths::{assert_outside_repository, ensure_runtime_root};
use crate::secure_store::{
    AccountLabel, SecureStore, SecureStoreEnvironment, SecureStoreError, ServiceLabel,
};

const IDENTITY_DIR: &str = "identity";
const BRAIN_FILE: &str = "brain_identity.json";

#[derive(Debug)]
pub enum IdentityRuntimeError {
    Lifecycle(LifecycleError),
    Core(CoreError),
    SecureStore(SecureStoreError),
    AlreadyExists(String),
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
    /// Returns `Some(false)` for any [`Self::CompensatingCleanupFailed`] —
    /// that variant means at least one cleanup failed, so whole-operation
    /// retry must not be implied. Other variants return `None` (no cleanup
    /// outcome encoded; e.g. a clean SecureStore put failure after successful
    /// public rollback is just `SecureStore` and may be retried).
    pub fn whole_operation_retry_safe(&self) -> Option<bool> {
        match self {
            Self::CompensatingCleanupFailed { .. } => Some(false),
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
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Remove a just-written brain identity (and staging tmp) so a failed create can retry.
///
/// Missing files are success. Any other removal error is returned and must not be ignored.
fn rollback_brain_identity(runtime_root: &Path) -> IdentityRuntimeResult<()> {
    let path = brain_identity_path(runtime_root);
    let tmp = path.with_extension("json.tmp");
    let path_result = remove_file_if_present(&path);
    let tmp_result = remove_file_if_present(&tmp);
    match (path_result, tmp_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(e), _) | (Ok(()), Err(e)) => Err(e.into()),
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
    // private cleanup as succeeded/not-required — there is no orphaned put.
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
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json.as_bytes())?;
    fs::rename(&tmp, &path)?;
    Ok(())
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
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

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
