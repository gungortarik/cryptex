//! Daemon-owned Ed25519 event-signing key lifecycle (ADR-0016 §6).
//!
//! Only `cryptexd` may create or complete initialization. Private seed bytes live
//! only in SecureStore; public metadata is exclusive-create JSON under app-data.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use cryptex_core::ids::DeviceId;
use cryptex_core::validate::parse_rfc3339_str;
use cryptex_events::hex::SigningKeyId;
use cryptex_events::integrity::signing_key_id_from_verifying_key;
use cryptex_storage::event_signing_key_path;
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};

use crate::paths::{assert_outside_repository, ensure_runtime_root};
use crate::secure_store::{
    AccountLabel, SecureStore, SecureStoreEnvironment, SecureStoreError, ServiceLabel,
};

const PUBLIC_SCHEMA_VERSION: u32 = 1;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);
#[cfg(test)]
thread_local! {
    static FAIL_NEXT_METADATA_WRITE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Public, non-secret event-signing key metadata (ADR-0016 §6.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventSigningKeyPublic {
    pub schema_version: u32,
    pub device_id: String,
    pub signing_key_id: String,
    pub public_key: String,
    pub created_at: String,
}

/// Ready signing material after successful ensure/init.
pub struct ReadyEventSigningKey {
    pub public: EventSigningKeyPublic,
    pub signing_key: SigningKey,
}

impl std::fmt::Debug for ReadyEventSigningKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadyEventSigningKey")
            .field("public", &self.public)
            .field("signing_key", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventSigningKeyError {
    InvalidDeviceId(String),
    InvalidPublicMetadata(String),
    IncompleteInit(String),
    FailClosed(String),
    SecureStore(SecureStoreError),
    Io(String),
    AlreadyExists(String),
    Core(String),
}

impl std::fmt::Display for EventSigningKeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidDeviceId(m) => write!(f, "invalid device_id: {m}"),
            Self::InvalidPublicMetadata(m) => write!(f, "invalid event signing key metadata: {m}"),
            Self::IncompleteInit(m) => write!(f, "incomplete event signing key init: {m}"),
            Self::FailClosed(m) => write!(f, "event signing key fail-closed: {m}"),
            Self::SecureStore(e) => write!(f, "{e}"),
            Self::Io(m) => write!(f, "event signing key io: {m}"),
            Self::AlreadyExists(p) => write!(f, "event signing key metadata already exists at {p}"),
            Self::Core(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for EventSigningKeyError {}

impl From<SecureStoreError> for EventSigningKeyError {
    fn from(value: SecureStoreError) -> Self {
        Self::SecureStore(value)
    }
}

impl From<cryptex_events::EventError> for EventSigningKeyError {
    fn from(value: cryptex_events::EventError) -> Self {
        Self::Core(value.to_string())
    }
}

pub type EventSigningKeyResult<T> = Result<T, EventSigningKeyError>;

/// Ensure a device event-signing key is ready (crash-safe, idempotent).
///
/// `observed_signing_key_ids` lists `signing_key_id` values already referenced by
/// local accepted events for this device (empty when none exist). Callers supply
/// this until segment scan wiring lands.
pub fn ensure_event_signing_key<S: SecureStore>(
    store: &S,
    env: SecureStoreEnvironment,
    runtime_root: &Path,
    device_id: &DeviceId,
    observed_signing_key_ids: &[SigningKeyId],
) -> EventSigningKeyResult<ReadyEventSigningKey> {
    assert_outside_repository(runtime_root).map_err(|e| EventSigningKeyError::Io(e.to_string()))?;
    ensure_runtime_root(runtime_root).map_err(|e| EventSigningKeyError::Io(e.to_string()))?;

    let meta_path = event_signing_key_path(runtime_root, device_id);
    if let Some(parent) = meta_path.parent() {
        fs::create_dir_all(parent).map_err(|e| EventSigningKeyError::Io(e.to_string()))?;
    }

    let lock_path = meta_path.with_extension("json.lock");
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|e| EventSigningKeyError::Io(e.to_string()))?;
    fs2::FileExt::lock_exclusive(&lock).map_err(|e| EventSigningKeyError::Io(e.to_string()))?;

    let service = ServiceLabel::device_credentials(env);
    let account = AccountLabel::device_event_signing_seed(device_id.as_str());

    let seed_state = match store.get(service.as_str(), account.as_str()) {
        Ok(bytes) => Some(bytes),
        Err(SecureStoreError::Missing) => None,
        Err(err) => return Err(err.into()),
    };
    let meta_state = load_public_metadata_if_present(&meta_path)?;

    let events_exist = !observed_signing_key_ids.is_empty();

    match (seed_state.as_deref(), meta_state.as_ref()) {
        (Some(seed), Some(meta)) => {
            let ready = ready_from_seed_and_meta(seed, meta, device_id)?;
            enforce_single_active_key(&ready.public, observed_signing_key_ids)?;
            Ok(ready)
        }
        (None, None) => {
            if events_exist {
                return Err(EventSigningKeyError::FailClosed(
                    "accepted events exist but event-signing seed and metadata are both missing"
                        .into(),
                ));
            }
            fresh_create(
                store,
                &service,
                &account,
                runtime_root,
                device_id,
                &meta_path,
            )
        }
        (Some(seed), None) => complete_metadata_from_seed(
            seed,
            device_id,
            &meta_path,
            observed_signing_key_ids,
            events_exist,
        ),
        (None, Some(meta)) => {
            let _ = meta;
            Err(EventSigningKeyError::FailClosed(
                "public event-signing metadata present but SecureStore seed is missing; refusing silent replace"
                    .into(),
            ))
        }
    }
}

fn fresh_create<S: SecureStore>(
    store: &S,
    service: &ServiceLabel,
    account: &AccountLabel,
    _runtime_root: &Path,
    device_id: &DeviceId,
    meta_path: &Path,
) -> EventSigningKeyResult<ReadyEventSigningKey> {
    let mut seed = [0u8; 32];
    getrandom::getrandom(&mut seed)
        .map_err(|e| EventSigningKeyError::Io(format!("getrandom failed: {e}")))?;
    let signing_key = SigningKey::from_bytes(&seed);
    let public = public_from_signing_key(device_id, &signing_key, now_rfc3339()?)?;

    store.put(service.as_str(), account.as_str(), &seed)?;

    match write_public_metadata_exclusive(meta_path, &public) {
        Ok(()) => {
            let reloaded = load_public_metadata_if_present(meta_path)?.ok_or_else(|| {
                EventSigningKeyError::IncompleteInit(
                    "metadata missing after exclusive create".into(),
                )
            })?;
            let ready = ready_from_seed_and_meta(&seed, &reloaded, device_id)?;
            // Confirm durable seed readback.
            let stored = store.get(service.as_str(), account.as_str())?;
            if stored.as_slice() != seed.as_slice() {
                return Err(EventSigningKeyError::IncompleteInit(
                    "SecureStore seed readback mismatch after put".into(),
                ));
            }
            Ok(ready)
        }
        Err(err) => Err(EventSigningKeyError::IncompleteInit(format!(
            "seed persisted but public metadata create failed: {err}; retry must complete from existing seed"
        ))),
    }
}

fn complete_metadata_from_seed(
    seed: &[u8],
    device_id: &DeviceId,
    meta_path: &Path,
    observed_signing_key_ids: &[SigningKeyId],
    events_exist: bool,
) -> EventSigningKeyResult<ReadyEventSigningKey> {
    let signing_key = signing_key_from_seed(seed)?;
    let public = public_from_signing_key(device_id, &signing_key, now_rfc3339()?)?;
    let derived_id = SigningKeyId::parse(&public.signing_key_id)?;

    if events_exist {
        let all_match = observed_signing_key_ids.iter().all(|id| id == &derived_id);
        if !all_match {
            return Err(EventSigningKeyError::FailClosed(
                "seed present and metadata missing, but derived signing_key_id does not match existing accepted events"
                    .into(),
            ));
        }
    }

    write_public_metadata_exclusive(meta_path, &public)?;
    let reloaded = load_public_metadata_if_present(meta_path)?.ok_or_else(|| {
        EventSigningKeyError::IncompleteInit("metadata missing after completion create".into())
    })?;
    ready_from_seed_and_meta(seed, &reloaded, device_id)
}

fn ready_from_seed_and_meta(
    seed: &[u8],
    meta: &EventSigningKeyPublic,
    device_id: &DeviceId,
) -> EventSigningKeyResult<ReadyEventSigningKey> {
    if meta.schema_version != PUBLIC_SCHEMA_VERSION {
        return Err(EventSigningKeyError::InvalidPublicMetadata(format!(
            "unsupported schema_version {}",
            meta.schema_version
        )));
    }
    parse_rfc3339_str(&meta.created_at).map_err(|e| {
        EventSigningKeyError::InvalidPublicMetadata(format!("created_at must be RFC 3339 UTC: {e}"))
    })?;
    if meta.device_id != device_id.as_str() {
        return Err(EventSigningKeyError::FailClosed(
            "metadata device_id does not match requested device".into(),
        ));
    }
    let signing_key = signing_key_from_seed(seed)?;
    let derived = public_from_signing_key(device_id, &signing_key, meta.created_at.clone())?;
    if derived.public_key != meta.public_key || derived.signing_key_id != meta.signing_key_id {
        return Err(EventSigningKeyError::FailClosed(
            "SecureStore seed does not match public event-signing metadata".into(),
        ));
    }
    Ok(ReadyEventSigningKey {
        public: meta.clone(),
        signing_key,
    })
}

fn enforce_single_active_key(
    public: &EventSigningKeyPublic,
    observed_signing_key_ids: &[SigningKeyId],
) -> EventSigningKeyResult<()> {
    let bound = SigningKeyId::parse(&public.signing_key_id)?;
    for id in observed_signing_key_ids {
        if id != &bound {
            return Err(EventSigningKeyError::FailClosed(
                "accepted events reference a signing_key_id that differs from the bound active key"
                    .into(),
            ));
        }
    }
    Ok(())
}

fn public_from_signing_key(
    device_id: &DeviceId,
    signing_key: &SigningKey,
    created_at: String,
) -> EventSigningKeyResult<EventSigningKeyPublic> {
    let verifying = signing_key.verifying_key();
    let key_id = signing_key_id_from_verifying_key(&verifying)?;
    Ok(EventSigningKeyPublic {
        schema_version: PUBLIC_SCHEMA_VERSION,
        device_id: device_id.as_str().to_string(),
        signing_key_id: key_id.as_str().to_string(),
        public_key: lowercase_hex(verifying.as_bytes()),
        created_at,
    })
}

fn signing_key_from_seed(seed: &[u8]) -> EventSigningKeyResult<SigningKey> {
    if seed.len() != 32 {
        return Err(EventSigningKeyError::FailClosed(format!(
            "event-signing seed must be 32 bytes, got {}",
            seed.len()
        )));
    }
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(seed);
    Ok(SigningKey::from_bytes(&bytes))
}

fn load_public_metadata_if_present(
    path: &Path,
) -> EventSigningKeyResult<Option<EventSigningKeyPublic>> {
    match fs::read_to_string(path) {
        Ok(text) => {
            let meta: EventSigningKeyPublic = serde_json::from_str(&text)
                .map_err(|e| EventSigningKeyError::InvalidPublicMetadata(e.to_string()))?;
            parse_rfc3339_str(&meta.created_at).map_err(|e| {
                EventSigningKeyError::InvalidPublicMetadata(format!(
                    "created_at must be RFC 3339 UTC: {e}"
                ))
            })?;
            Ok(Some(meta))
        }
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(EventSigningKeyError::Io(err.to_string())),
    }
}

fn write_public_metadata_exclusive(
    path: &Path,
    meta: &EventSigningKeyPublic,
) -> EventSigningKeyResult<()> {
    #[cfg(test)]
    {
        let inject = FAIL_NEXT_METADATA_WRITE.with(|flag| {
            let armed = flag.get();
            if armed {
                flag.set(false);
            }
            armed
        });
        if inject {
            return Err(EventSigningKeyError::Io(
                "injected metadata write failure".into(),
            ));
        }
    }
    if path.exists() {
        return Err(EventSigningKeyError::AlreadyExists(
            path.display().to_string(),
        ));
    }
    let mut json =
        serde_json::to_string(meta).map_err(|e| EventSigningKeyError::Io(e.to_string()))?;
    json.push('\n');
    let tmp = unique_tmp(path);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(|e| EventSigningKeyError::Io(e.to_string()))?;
    file.write_all(json.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|e| {
            let _ = fs::remove_file(&tmp);
            EventSigningKeyError::Io(e.to_string())
        })?;
    drop(file);
    match place_new_file_no_replace(&tmp, path) {
        Ok(()) => Ok(()),
        Err(err) => {
            let _ = fs::remove_file(&tmp);
            Err(err)
        }
    }
}

fn unique_tmp(path: &Path) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|t| t.as_nanos())
        .unwrap_or(0);
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    path.with_extension(format!(
        "json.tmp.{}-{sequence}-{nanos}",
        std::process::id()
    ))
}

fn place_new_file_no_replace(tmp: &Path, dest: &Path) -> EventSigningKeyResult<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
        let from: Vec<u16> = tmp.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = dest.as_os_str().encode_wide().chain(Some(0)).collect();
        let ok = unsafe {
            windows_sys::Win32::Storage::FileSystem::MoveFileExW(
                from.as_ptr(),
                to.as_ptr(),
                MOVEFILE_WRITE_THROUGH,
            )
        };
        if ok == 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == ErrorKind::AlreadyExists {
                return Err(EventSigningKeyError::AlreadyExists(
                    dest.display().to_string(),
                ));
            }
            // Windows may report ERROR_ALREADY_EXISTS as other kinds.
            if err.raw_os_error() == Some(183) {
                return Err(EventSigningKeyError::AlreadyExists(
                    dest.display().to_string(),
                ));
            }
            return Err(EventSigningKeyError::Io(err.to_string()));
        }
        Ok(())
    }
    #[cfg(unix)]
    {
        fs::hard_link(tmp, dest).map_err(|err| {
            if err.kind() == ErrorKind::AlreadyExists {
                EventSigningKeyError::AlreadyExists(dest.display().to_string())
            } else {
                EventSigningKeyError::Io(err.to_string())
            }
        })?;
        let parent = dest.parent().ok_or_else(|| {
            EventSigningKeyError::Io("metadata path has no parent for directory sync".into())
        })?;
        fs::File::open(parent)
            .and_then(|d| d.sync_all())
            .map_err(|err| {
                EventSigningKeyError::Io(format!(
                    "directory sync failed after metadata placement: {err}"
                ))
            })?;
        fs::remove_file(tmp).map_err(|err| {
            EventSigningKeyError::Io(format!(
                "failed to remove metadata staging file after durable placement: {err}"
            ))
        })?;
        Ok(())
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (tmp, dest);
        Err(EventSigningKeyError::Io(
            "exclusive event-signing metadata create unsupported on this platform".into(),
        ))
    }
}

fn lowercase_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn now_rfc3339() -> EventSigningKeyResult<String> {
    // Display-only field for public metadata; wall-clock never authorizes acceptance.
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let formatted = format_unix_secs_rfc3339_utc(secs);
    parse_rfc3339_str(&formatted)
        .map_err(|e| EventSigningKeyError::InvalidPublicMetadata(e.to_string()))
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

/// Test helper: persist only the seed (simulate crash before metadata).
#[cfg(test)]
fn put_seed_only_for_test<S: SecureStore>(
    store: &S,
    env: SecureStoreEnvironment,
    device_id: &DeviceId,
    seed: &[u8; 32],
) -> EventSigningKeyResult<()> {
    let service = ServiceLabel::device_credentials(env);
    let account = AccountLabel::device_event_signing_seed(device_id.as_str());
    store.put(service.as_str(), account.as_str(), seed)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secure_store::FakeSecureStore;
    use std::sync::{Arc, Barrier};
    use std::thread;

    fn temp_root(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|t| t.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "cryptex-m2-signing-{}-{}-{}",
            label,
            std::process::id(),
            nanos
        ))
    }

    #[test]
    fn fresh_create_emits_valid_rfc3339_created_at() {
        let root = temp_root("rfc3339");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("device-rfc").unwrap();
        let ready =
            ensure_event_signing_key(&store, SecureStoreEnvironment::Dev, &root, &device, &[])
                .unwrap();
        parse_rfc3339_str(&ready.public.created_at).unwrap();
        assert!(
            ready.public.created_at.ends_with('Z')
                || ready.public.created_at.contains('+')
                || ready.public.created_at.contains('-')
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn fresh_create_is_idempotent() {
        let root = temp_root("fresh");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("device-a").unwrap();
        let first =
            ensure_event_signing_key(&store, SecureStoreEnvironment::Dev, &root, &device, &[])
                .expect("create");
        let second =
            ensure_event_signing_key(&store, SecureStoreEnvironment::Dev, &root, &device, &[])
                .expect("idempotent");
        assert_eq!(first.public.signing_key_id, second.public.signing_key_id);
        assert_eq!(first.public.public_key, second.public.public_key);
        assert_eq!(first.public.public_key.len(), 64);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn completes_metadata_after_seed_only_crash() {
        let root = temp_root("seed-only");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("device-b").unwrap();
        let seed = [0x42u8; 32];
        put_seed_only_for_test(&store, SecureStoreEnvironment::Dev, &device, &seed).unwrap();
        let ready =
            ensure_event_signing_key(&store, SecureStoreEnvironment::Dev, &root, &device, &[])
                .expect("complete");
        let expected = SigningKey::from_bytes(&seed);
        let expected_id = signing_key_id_from_verifying_key(&expected.verifying_key()).unwrap();
        assert_eq!(ready.public.signing_key_id, expected_id.as_str());
        assert!(event_signing_key_path(&root, &device).is_file());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn fail_closed_when_metadata_present_without_seed() {
        let root = temp_root("meta-only");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("device-c").unwrap();
        let path = event_signing_key_path(&root, &device);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let fake = EventSigningKeyPublic {
            schema_version: 1,
            device_id: device.as_str().into(),
            signing_key_id: "aa".repeat(16),
            public_key: "bb".repeat(32),
            created_at: "2026-10-07T00:00:00Z".into(),
        };
        write_public_metadata_exclusive(&path, &fake).unwrap();
        match ensure_event_signing_key(&store, SecureStoreEnvironment::Dev, &root, &device, &[]) {
            Err(EventSigningKeyError::FailClosed(_)) => {}
            other => panic!("expected FailClosed, got {other:?}"),
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn fail_closed_on_seed_metadata_mismatch() {
        let root = temp_root("mismatch");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("device-d").unwrap();
        let seed = [0x11u8; 32];
        put_seed_only_for_test(&store, SecureStoreEnvironment::Dev, &device, &seed).unwrap();
        let path = event_signing_key_path(&root, &device);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let wrong = EventSigningKeyPublic {
            schema_version: 1,
            device_id: device.as_str().into(),
            signing_key_id: "aa".repeat(16),
            public_key: "bb".repeat(32),
            created_at: "2026-10-07T00:00:00Z".into(),
        };
        write_public_metadata_exclusive(&path, &wrong).unwrap();
        let err =
            ensure_event_signing_key(&store, SecureStoreEnvironment::Dev, &root, &device, &[])
                .unwrap_err();
        assert!(matches!(err, EventSigningKeyError::FailClosed(_)));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn fail_closed_when_events_reference_foreign_key() {
        let root = temp_root("foreign");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("device-e").unwrap();
        let ready =
            ensure_event_signing_key(&store, SecureStoreEnvironment::Dev, &root, &device, &[])
                .unwrap();
        let foreign = SigningKeyId::parse(&"cd".repeat(16)).unwrap();
        let err = ensure_event_signing_key(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            &device,
            &[foreign],
        )
        .unwrap_err();
        assert!(matches!(err, EventSigningKeyError::FailClosed(_)));
        let _ = ready;
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn incomplete_init_when_metadata_write_fails_after_seed_put() {
        let root = temp_root("put-fail");
        let device = DeviceId::new("device-f").unwrap();
        let store = FakeSecureStore::new();
        FAIL_NEXT_METADATA_WRITE.with(|flag| flag.set(true));
        let err =
            ensure_event_signing_key(&store, SecureStoreEnvironment::Dev, &root, &device, &[])
                .unwrap_err();
        assert!(matches!(err, EventSigningKeyError::IncompleteInit(_)));
        let service = ServiceLabel::device_credentials(SecureStoreEnvironment::Dev);
        let account = AccountLabel::device_event_signing_seed(device.as_str());
        assert!(store.get(service.as_str(), account.as_str()).is_ok());
        // Retry completes from existing seed without generating a new one.
        let ready =
            ensure_event_signing_key(&store, SecureStoreEnvironment::Dev, &root, &device, &[])
                .expect("complete after crash");
        assert_eq!(ready.public.public_key.len(), 64);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn concurrent_ensure_converges_on_one_key() {
        let root = temp_root("conc");
        let store = Arc::new(FakeSecureStore::new());
        let device = DeviceId::new("device-g").unwrap();
        let barrier = Arc::new(Barrier::new(8));
        let mut handles = Vec::new();
        for _ in 0..8 {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            let root = root.clone();
            let device = device.clone();
            handles.push(thread::spawn(move || {
                barrier.wait();
                ensure_event_signing_key(
                    store.as_ref(),
                    SecureStoreEnvironment::Dev,
                    &root,
                    &device,
                    &[],
                )
                .map(|r| r.public.signing_key_id)
            }));
        }
        let ids: Vec<String> = handles
            .into_iter()
            .map(|h| h.join().unwrap().expect("ensure"))
            .collect();
        assert!(ids.iter().all(|id| id == &ids[0]));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_invalid_created_at_on_load_without_replacing_key() {
        let root = temp_root("bad-ts");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("device-ts").unwrap();
        let seed = [0x33u8; 32];
        put_seed_only_for_test(&store, SecureStoreEnvironment::Dev, &device, &seed).unwrap();
        let path = event_signing_key_path(&root, &device);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let bad = EventSigningKeyPublic {
            schema_version: 1,
            device_id: device.as_str().into(),
            signing_key_id: "aa".repeat(16),
            public_key: "bb".repeat(32),
            created_at: "not-rfc3339".into(),
        };
        // Bypass writer validation by writing raw JSON.
        fs::write(&path, serde_json::to_string(&bad).unwrap() + "\n").unwrap();
        let err =
            ensure_event_signing_key(&store, SecureStoreEnvironment::Dev, &root, &device, &[])
                .unwrap_err();
        assert!(
            matches!(err, EventSigningKeyError::InvalidPublicMetadata(_)),
            "{err}"
        );
        // Seed must remain; no silent replace.
        let service = ServiceLabel::device_credentials(SecureStoreEnvironment::Dev);
        let account = AccountLabel::device_event_signing_seed(device.as_str());
        assert_eq!(store.get(service.as_str(), account.as_str()).unwrap(), seed);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn public_metadata_never_contains_seed_bytes() {
        let root = temp_root("nosecret");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("device-h").unwrap();
        let seed = [0x7eu8; 32];
        // Force known seed via put then complete.
        put_seed_only_for_test(&store, SecureStoreEnvironment::Dev, &device, &seed).unwrap();
        let _ = ensure_event_signing_key(&store, SecureStoreEnvironment::Dev, &root, &device, &[])
            .unwrap();
        let text = fs::read_to_string(event_signing_key_path(&root, &device)).unwrap();
        assert!(!crate::secure_store::payload_bytes_recoverable(
            &seed, &text
        ));
        let _ = fs::remove_dir_all(&root);
    }
}
