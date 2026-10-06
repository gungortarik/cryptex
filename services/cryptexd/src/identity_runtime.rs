//! M1 daemon wiring: public Brain metadata on disk + private material via SecureStore.
//!
//! Public files never contain secret bytes. Private blobs go only through [`SecureStore`].

use std::fs;
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
}

impl std::fmt::Display for IdentityRuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Lifecycle(e) => write!(f, "{e}"),
            Self::Core(e) => write!(f, "{e}"),
            Self::SecureStore(e) => write!(f, "{e}"),
            Self::AlreadyExists(path) => write!(f, "identity already exists at {path}"),
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

pub type IdentityRuntimeResult<T> = Result<T, IdentityRuntimeError>;

pub fn identity_dir(runtime_root: &Path) -> PathBuf {
    runtime_root.join(IDENTITY_DIR)
}

pub fn brain_identity_path(runtime_root: &Path) -> PathBuf {
    identity_dir(runtime_root).join(BRAIN_FILE)
}

/// Best-effort removal of a just-written brain identity so SecureStore failure can retry.
fn rollback_brain_identity(runtime_root: &Path) {
    let path = brain_identity_path(runtime_root);
    let tmp = path.with_extension("json.tmp");
    let _ = fs::remove_file(&path);
    let _ = fs::remove_file(&tmp);
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
        rollback_brain_identity(runtime_root);
        return Err(err);
    }
    // Verify public file has no secret bytes.
    let on_disk = match fs::read_to_string(brain_identity_path(runtime_root)) {
        Ok(text) => text,
        Err(e) => {
            rollback_brain_identity(runtime_root);
            let _ = store.delete(
                ServiceLabel::device_credentials(env).as_str(),
                AccountLabel::device_private(device_id).as_str(),
            );
            return Err(e.into());
        }
    };
    if crate::secure_store::payload_bytes_recoverable(device_secret, &on_disk) {
        rollback_brain_identity(runtime_root);
        let _ = store.delete(
            ServiceLabel::device_credentials(env).as_str(),
            AccountLabel::device_private(device_id).as_str(),
        );
        return Err(IdentityRuntimeError::Lifecycle(LifecycleError::Attention(
            "public brain identity file must not contain device secret bytes".into(),
        )));
    }
    Ok(identity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secure_store::FakeSecureStore;
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
}
