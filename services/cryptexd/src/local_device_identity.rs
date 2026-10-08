//! Explicit local-device enrollment for daemon-owned M4 retrieval (ADR-0018).
//!
//! This module never enrolls or trusts peer devices. Enrollment is an explicit
//! setup call; lifecycle startup and ordinary reads do not create identities.

use std::fs::{self, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use cryptex_core::ids::DeviceId;
use cryptex_core::validate::{parse_uuid_str, require_exact_schema};
use cryptex_events::TrustedDaemonKeyResolver;
use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};

use crate::event_signing_key::{
    EventSigningKeyError, ensure_event_signing_key, load_existing_event_verifying_key,
};
use crate::identity_runtime::{IdentityRuntimeError, load_brain_identity};
use crate::paths::{assert_outside_repository, ensure_runtime_root};
use crate::secure_store::{SecureStore, SecureStoreEnvironment};

const LOCAL_DEVICE_SCHEMA_VERSION: u32 = 1;
const IDENTITY_DIRECTORY: &str = "identity";
const LOCAL_DEVICE_FILE: &str = "local_device.json";
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Public local binding between this Brain, its stable DeviceId, and the
/// daemon-owned signing metadata under `identity/devices/<id>/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalDeviceIdentity {
    pub schema_version: u32,
    pub brain_id: String,
    pub device_id: String,
}

impl LocalDeviceIdentity {
    fn validate(&self) -> Result<(), LocalDeviceIdentityError> {
        require_exact_schema(
            "local device identity",
            self.schema_version,
            LOCAL_DEVICE_SCHEMA_VERSION,
        )
        .map_err(|error| LocalDeviceIdentityError::InvalidMetadata(error.to_string()))?;
        parse_uuid_str(&self.brain_id)
            .map_err(|error| LocalDeviceIdentityError::InvalidMetadata(error.to_string()))?;
        DeviceId::new(self.device_id.clone())
            .map_err(|error| LocalDeviceIdentityError::InvalidMetadata(error.to_string()))?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum LocalDeviceIdentityError {
    Io(String),
    Brain(IdentityRuntimeError),
    InvalidMetadata(String),
    BrainMismatch,
    ExistingDeviceState,
    EventSigningKey(EventSigningKeyError),
    EnrollmentCleanupFailed { primary: String, cleanup: String },
}

impl std::fmt::Display for LocalDeviceIdentityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "local device identity I/O failed: {error}"),
            Self::Brain(error) => write!(f, "local Brain identity unavailable: {error}"),
            Self::InvalidMetadata(error) => write!(f, "local device identity is invalid: {error}"),
            Self::BrainMismatch => write!(f, "local device identity belongs to another Brain"),
            Self::ExistingDeviceState => write!(
                f,
                "device or canonical state already exists; refusing implicit enrollment"
            ),
            Self::EventSigningKey(error) => {
                write!(f, "local signing identity unavailable: {error}")
            }
            Self::EnrollmentCleanupFailed { primary, cleanup } => write!(
                f,
                "local device identity create failed: {primary}; staging cleanup failed: {cleanup}"
            ),
        }
    }
}

impl std::error::Error for LocalDeviceIdentityError {}

pub type LocalDeviceIdentityResult<T> = Result<T, LocalDeviceIdentityError>;

/// Read-only resolver for this installation's enrolled daemon key.
///
/// It intentionally recognizes only the single local DeviceId/key binding;
/// discovered peers are not trusted by this M4 boundary.
pub struct LocalDeviceKeyResolver {
    device_id: String,
    signing_key_id: String,
    verifying_key: VerifyingKey,
}

impl LocalDeviceKeyResolver {
    pub fn load<S: SecureStore>(
        runtime_root: &Path,
        expected_brain_id: &str,
        store: &S,
        env: SecureStoreEnvironment,
    ) -> LocalDeviceIdentityResult<Self> {
        let identity = load_local_device_identity(runtime_root, expected_brain_id)?;
        let device_id = DeviceId::new(identity.device_id)
            .map_err(|error| LocalDeviceIdentityError::InvalidMetadata(error.to_string()))?;
        let (metadata, verifying_key) =
            load_existing_event_verifying_key(store, env, runtime_root, &device_id)
                .map_err(LocalDeviceIdentityError::EventSigningKey)?;
        Ok(Self {
            device_id: device_id.as_str().to_owned(),
            signing_key_id: metadata.signing_key_id,
            verifying_key,
        })
    }
}

impl TrustedDaemonKeyResolver for LocalDeviceKeyResolver {
    fn trusted_daemon_key(&mut self, device_id: &str, key_id: &str) -> Option<VerifyingKey> {
        (device_id == self.device_id && key_id == self.signing_key_id).then_some(self.verifying_key)
    }
}

pub fn local_device_identity_path(runtime_root: &Path) -> PathBuf {
    runtime_root
        .join(IDENTITY_DIRECTORY)
        .join(LOCAL_DEVICE_FILE)
}

/// Explicitly enroll this installation as the sole local trusted device.
///
/// Existing local identity is accepted only for the same Brain and after the
/// SecureStore seed/public signing metadata pair verifies. A first enrollment
/// refuses pre-existing canonical streams or device-key metadata; this avoids
/// guessing which identity owns old data. The local identity file is committed
/// before key initialization so a crash retries with the same DeviceId.
pub fn enroll_local_device<S: SecureStore>(
    runtime_root: &Path,
    store: &S,
    env: SecureStoreEnvironment,
) -> LocalDeviceIdentityResult<LocalDeviceIdentity> {
    assert_outside_repository(runtime_root)
        .map_err(|error| LocalDeviceIdentityError::Io(error.to_string()))?;
    ensure_runtime_root(runtime_root)
        .map_err(|error| LocalDeviceIdentityError::Io(error.to_string()))?;
    let brain = load_brain_identity(runtime_root).map_err(LocalDeviceIdentityError::Brain)?;
    let identity_path = local_device_identity_path(runtime_root);
    let identity_dir = identity_path
        .parent()
        .ok_or_else(|| LocalDeviceIdentityError::Io("identity path has no parent".into()))?;
    fs::create_dir_all(identity_dir)
        .map_err(|error| LocalDeviceIdentityError::Io(error.to_string()))?;

    let lock_path = identity_dir.join("local_device.lock");
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_path)
        .map_err(|error| LocalDeviceIdentityError::Io(error.to_string()))?;
    fs2::FileExt::lock_exclusive(&lock)
        .map_err(|error| LocalDeviceIdentityError::Io(error.to_string()))?;

    let existing = match read_local_device_identity(&identity_path)? {
        Some(identity) => {
            if identity.brain_id != brain.brain_id {
                return Err(LocalDeviceIdentityError::BrainMismatch);
            }
            identity
        }
        None => {
            refuse_existing_unbound_state(runtime_root)?;
            let identity = LocalDeviceIdentity {
                schema_version: LOCAL_DEVICE_SCHEMA_VERSION,
                brain_id: brain.brain_id,
                device_id: random_device_id()?,
            };
            identity.validate()?;
            write_local_device_identity_exclusive(&identity_path, &identity)?;
            identity
        }
    };

    let device_id = DeviceId::new(existing.device_id.clone())
        .map_err(|error| LocalDeviceIdentityError::InvalidMetadata(error.to_string()))?;
    let has_packs = device_has_canonical_packs(runtime_root, &device_id)?;
    ensure_event_signing_key(store, env, runtime_root, &device_id, &[], has_packs)
        .map_err(LocalDeviceIdentityError::EventSigningKey)?;
    drop(lock);
    Ok(existing)
}

/// Read and validate persisted local identity without creating or repairing it.
pub fn load_local_device_identity(
    runtime_root: &Path,
    expected_brain_id: &str,
) -> LocalDeviceIdentityResult<LocalDeviceIdentity> {
    parse_uuid_str(expected_brain_id)
        .map_err(|error| LocalDeviceIdentityError::InvalidMetadata(error.to_string()))?;
    let path = local_device_identity_path(runtime_root);
    let identity = read_local_device_identity(&path)?.ok_or_else(|| {
        LocalDeviceIdentityError::Io("local device enrollment is required".into())
    })?;
    if identity.brain_id != expected_brain_id {
        return Err(LocalDeviceIdentityError::BrainMismatch);
    }
    Ok(identity)
}

fn read_local_device_identity(
    path: &Path,
) -> LocalDeviceIdentityResult<Option<LocalDeviceIdentity>> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(LocalDeviceIdentityError::Io(error.to_string())),
    };
    if text.contains('\r') || !text.ends_with('\n') {
        return Err(LocalDeviceIdentityError::InvalidMetadata(
            "metadata must be UTF-8 LF text with a final newline".into(),
        ));
    }
    let identity: LocalDeviceIdentity = serde_json::from_str(&text)
        .map_err(|error| LocalDeviceIdentityError::InvalidMetadata(error.to_string()))?;
    identity.validate()?;
    Ok(Some(identity))
}

fn random_device_id() -> LocalDeviceIdentityResult<String> {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes)
        .map_err(|error| LocalDeviceIdentityError::Io(format!("secure random failed: {error}")))?;
    // RFC 4122 UUID v4 variant/version bits, formatted as a valid ADR-0015 DeviceId.
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    ))
}

fn device_has_canonical_packs(
    runtime_root: &Path,
    device_id: &DeviceId,
) -> LocalDeviceIdentityResult<bool> {
    let dir = cryptex_storage::canonical_events_device_dir(runtime_root, device_id);
    match fs::read_dir(dir) {
        Ok(entries) => {
            for entry in entries {
                let entry =
                    entry.map_err(|error| LocalDeviceIdentityError::Io(error.to_string()))?;
                if entry
                    .file_type()
                    .map_err(|error| LocalDeviceIdentityError::Io(error.to_string()))?
                    .is_file()
                {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(LocalDeviceIdentityError::Io(error.to_string())),
    }
}

fn refuse_existing_unbound_state(runtime_root: &Path) -> LocalDeviceIdentityResult<()> {
    for relative in [Path::new("canonical/events"), Path::new("identity/devices")] {
        let path = runtime_root.join(relative);
        match fs::read_dir(path) {
            Ok(mut entries) => {
                if entries
                    .next()
                    .transpose()
                    .map_err(|error| LocalDeviceIdentityError::Io(error.to_string()))?
                    .is_some()
                {
                    return Err(LocalDeviceIdentityError::ExistingDeviceState);
                }
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(LocalDeviceIdentityError::Io(error.to_string())),
        }
    }
    Ok(())
}

fn write_local_device_identity_exclusive(
    path: &Path,
    identity: &LocalDeviceIdentity,
) -> LocalDeviceIdentityResult<()> {
    let mut bytes = serde_json::to_vec_pretty(identity)
        .map_err(|error| LocalDeviceIdentityError::InvalidMetadata(error.to_string()))?;
    bytes.push(b'\n');
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let tmp = path.with_extension(format!(
        "json.tmp.{}-{sequence}-{nanos}",
        std::process::id()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(|error| LocalDeviceIdentityError::Io(error.to_string()))?;
    let write_result = file.write_all(&bytes).and_then(|()| file.sync_all());
    drop(file);
    if let Err(primary) = write_result {
        return Err(clean_staging(&tmp, primary));
    }
    match place_no_replace(&tmp, path) {
        Ok(()) => Ok(()),
        Err(primary) => Err(clean_staging(&tmp, primary)),
    }
}

fn clean_staging(tmp: &Path, primary: io::Error) -> LocalDeviceIdentityError {
    match fs::remove_file(tmp) {
        Ok(()) => LocalDeviceIdentityError::Io(primary.to_string()),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            LocalDeviceIdentityError::Io(primary.to_string())
        }
        Err(cleanup) => LocalDeviceIdentityError::EnrollmentCleanupFailed {
            primary: primary.to_string(),
            cleanup: cleanup.to_string(),
        },
    }
}

fn place_no_replace(tmp: &Path, path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
        let from: Vec<u16> = tmp.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: both pointers reference valid null-terminated UTF-16 paths.
        if unsafe {
            windows_sys::Win32::Storage::FileSystem::MoveFileExW(
                from.as_ptr(),
                to.as_ptr(),
                MOVEFILE_WRITE_THROUGH,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
    #[cfg(unix)]
    {
        place_no_replace_unix(tmp, path)
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = (tmp, path);
        Err(io::Error::new(
            ErrorKind::Unsupported,
            "exclusive placement unsupported",
        ))
    }
}

#[cfg(unix)]
fn place_no_replace_unix(tmp: &Path, path: &Path) -> io::Result<()> {
    fs::hard_link(tmp, path)?;

    let commit = fs::remove_file(tmp).and_then(|()| sync_identity_parent(path));
    if let Err(primary) = commit {
        let rollback = fs::remove_file(path).and_then(|()| sync_identity_parent(path));
        return match rollback {
            Ok(()) => Err(primary),
            Err(cleanup) if cleanup.kind() == ErrorKind::NotFound => Err(primary),
            Err(cleanup) => Err(io::Error::other(format!(
                "identity placement failed: {primary}; destination rollback failed: {cleanup}"
            ))),
        };
    }
    Ok(())
}

#[cfg(unix)]
fn sync_identity_parent(path: &Path) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(ErrorKind::InvalidInput, "identity path has no parent"))?;
    fs::File::open(parent)?.sync_all()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;
    use std::thread;

    use cryptex_core::BrainIdentity;

    use super::*;
    use crate::identity_runtime::write_brain_identity;
    use crate::secure_store::{FakeSecureStore, SecureStoreError};
    use crate::test_temp::exclusive_temp_dir;

    const BRAIN_ID: &str = "11111111-1111-4111-8111-111111111111";

    fn prepared_root() -> PathBuf {
        let root = exclusive_temp_dir("cryptex-local-device");
        let brain = BrainIdentity::new(BRAIN_ID, "2026-10-08T00:00:00Z").expect("brain");
        write_brain_identity(&root, &brain).expect("write Brain identity");
        root
    }

    #[test]
    fn explicit_enrollment_persists_device_and_signing_binding() {
        let root = prepared_root();
        let store = FakeSecureStore::new();

        let enrolled =
            enroll_local_device(&root, &store, SecureStoreEnvironment::Dev).expect("enrollment");
        let loaded = load_local_device_identity(&root, BRAIN_ID).expect("load identity");

        assert_eq!(loaded, enrolled);
        assert_eq!(loaded.brain_id, BRAIN_ID);
        assert_eq!(store.len(), 1, "private seed must be in SecureStore");
        assert!(
            cryptex_storage::event_signing_key_path(
                &root,
                &DeviceId::new(loaded.device_id.clone()).expect("device id")
            )
            .is_file(),
            "public key metadata should be bound to the enrolled DeviceId"
        );

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn repeated_enrollment_reuses_existing_identity() {
        let root = prepared_root();
        let store = FakeSecureStore::new();

        let first = enroll_local_device(&root, &store, SecureStoreEnvironment::Dev)
            .expect("first enrollment");
        let second = enroll_local_device(&root, &store, SecureStoreEnvironment::Dev)
            .expect("idempotent enrollment");

        assert_eq!(first, second);
        assert_eq!(store.len(), 1);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn secure_store_failure_retry_keeps_the_same_local_device_id() {
        let root = prepared_root();
        let store = FakeSecureStore::new();
        store.fail_next_put_with(SecureStoreError::Platform {
            op: "test_put",
            code: 5,
        });

        assert!(matches!(
            enroll_local_device(&root, &store, SecureStoreEnvironment::Dev),
            Err(LocalDeviceIdentityError::EventSigningKey(_))
        ));
        let persisted = load_local_device_identity(&root, BRAIN_ID).expect("stable local id");
        let retried = enroll_local_device(&root, &store, SecureStoreEnvironment::Dev)
            .expect("retry enrollment");

        assert_eq!(retried, persisted);
        assert_eq!(store.len(), 1);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn concurrent_first_enrollment_converges_on_one_device() {
        let root = prepared_root();
        let store = Arc::new(FakeSecureStore::new());
        let workers = (0..8)
            .map(|_| {
                let root = root.clone();
                let store = Arc::clone(&store);
                thread::spawn(move || {
                    enroll_local_device(&root, store.as_ref(), SecureStoreEnvironment::Dev)
                        .expect("concurrent enrollment")
                })
            })
            .collect::<Vec<_>>();
        let identities = workers
            .into_iter()
            .map(|worker| worker.join().expect("worker"))
            .collect::<Vec<_>>();

        assert!(identities.iter().all(|identity| identity == &identities[0]));
        assert_eq!(store.len(), 1);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn load_is_read_only_and_requires_explicit_enrollment() {
        let root = exclusive_temp_dir("cryptex-local-device-unenrolled");
        let error = load_local_device_identity(&root, BRAIN_ID).expect_err("not enrolled");

        assert!(matches!(error, LocalDeviceIdentityError::Io(_)));
        assert!(!local_device_identity_path(&root).exists());
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn enrollment_rejects_brain_mismatch_and_corrupt_local_metadata() {
        let root = prepared_root();
        let store = FakeSecureStore::new();
        let enrolled =
            enroll_local_device(&root, &store, SecureStoreEnvironment::Dev).expect("enrollment");
        let wrong_brain = "22222222-2222-4222-8222-222222222222";

        assert!(matches!(
            load_local_device_identity(&root, wrong_brain),
            Err(LocalDeviceIdentityError::BrainMismatch)
        ));
        fs::write(
            local_device_identity_path(&root),
            format!(
                "{{\"schema_version\":1,\"brain_id\":\"{}\",\"device_id\":\"../bad\"}}\n",
                enrolled.brain_id
            ),
        )
        .expect("corrupt metadata");
        assert!(matches!(
            enroll_local_device(&root, &store, SecureStoreEnvironment::Dev),
            Err(LocalDeviceIdentityError::InvalidMetadata(_))
        ));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn first_enrollment_refuses_unbound_canonical_state() {
        let root = prepared_root();
        let device_dir = root.join("canonical/events/unattributed");
        fs::create_dir_all(&device_dir).expect("canonical fixture");
        fs::write(device_dir.join("legacy.seg"), b"synthetic fixture").expect("pack fixture");
        let store = FakeSecureStore::new();

        assert!(matches!(
            enroll_local_device(&root, &store, SecureStoreEnvironment::Dev),
            Err(LocalDeviceIdentityError::ExistingDeviceState)
        ));
        assert!(!local_device_identity_path(&root).exists());
        assert!(store.is_empty());
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn local_resolver_trusts_only_the_enrolled_device_and_key() {
        let root = prepared_root();
        let store = FakeSecureStore::new();
        let identity =
            enroll_local_device(&root, &store, SecureStoreEnvironment::Dev).expect("enrollment");
        let mut resolver =
            LocalDeviceKeyResolver::load(&root, BRAIN_ID, &store, SecureStoreEnvironment::Dev)
                .expect("existing key resolver");

        assert!(
            resolver
                .trusted_daemon_key(&identity.device_id, &resolver.signing_key_id.clone())
                .is_some()
        );
        assert!(
            resolver
                .trusted_daemon_key("another-device", &resolver.signing_key_id.clone())
                .is_none()
        );
        assert!(
            resolver
                .trusted_daemon_key(&identity.device_id, "unknown-key")
                .is_none()
        );
        fs::remove_dir_all(root).expect("cleanup");
    }
}
