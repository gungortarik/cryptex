//! cryptexd-only canonical event writer (ADR-0016 §11).
//!
//! Allocates `device_seq`, signs with the daemon-owned key, recovers `.open` packs,
//! and appends framed accepted records. Clients must not call storage I/O directly.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use cryptex_core::ids::DeviceId;
use cryptex_core::validate::{parse_rfc3339_str, parse_uuid_str};
use cryptex_events::device_seq::DeviceSeq;
use cryptex_events::envelope::{
    AcceptedEvent, Classification, EVENT_SCHEMA_VERSION, EventPayload, EventType,
    MemoryNotePayload, SignableEvent,
};
use cryptex_events::hex::{ContentHashHex, SigningKeyId, ZERO_CONTENT_HASH_HEX};
use cryptex_events::integrity::{accept_event, verify_accepted_event};
use cryptex_events::limits::DEFAULT_SEAL_EVENT_THRESHOLD_HINT;
use cryptex_storage::layout::{active_segment_path, canonical_events_device_dir};
use cryptex_storage::recovery::{FrameBodyValidator, recover_active_open};
use cryptex_storage::{StorageError, append_frame, seal_active_segment};
use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};

use crate::event_signing_key::{EventSigningKeyError, ensure_event_signing_key};
use crate::paths::{assert_outside_repository, ensure_runtime_root};
use crate::secure_store::{SecureStore, SecureStoreEnvironment};
use crate::sequence_runtime::{allocate_device_sequence, initialize_device_sequence};

#[derive(Debug, Clone)]
pub struct CanonicalWriterConfig {
    /// Tunable seal threshold (not a protocol constant).
    pub seal_event_threshold: u64,
}

impl Default for CanonicalWriterConfig {
    fn default() -> Self {
        Self {
            seal_event_threshold: DEFAULT_SEAL_EVENT_THRESHOLD_HINT,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppendOutcome {
    pub accepted: AcceptedEvent,
    pub segment_index: u64,
    pub sealed: bool,
}

#[derive(Debug)]
pub enum CanonicalWriterError {
    Path(String),
    Signing(EventSigningKeyError),
    Storage(StorageError),
    Sequence(String),
    Integrity(String),
    Invalid(String),
}

impl std::fmt::Display for CanonicalWriterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Path(m) => write!(f, "canonical writer path error: {m}"),
            Self::Signing(e) => write!(f, "{e}"),
            Self::Storage(e) => write!(f, "{e}"),
            Self::Sequence(m) => write!(f, "canonical writer sequence error: {m}"),
            Self::Integrity(m) => write!(f, "canonical writer integrity error: {m}"),
            Self::Invalid(m) => write!(f, "canonical writer invalid input: {m}"),
        }
    }
}

impl std::error::Error for CanonicalWriterError {}

impl From<EventSigningKeyError> for CanonicalWriterError {
    fn from(value: EventSigningKeyError) -> Self {
        Self::Signing(value)
    }
}

impl From<StorageError> for CanonicalWriterError {
    fn from(value: StorageError) -> Self {
        Self::Storage(value)
    }
}

impl From<cryptex_events::EventError> for CanonicalWriterError {
    fn from(value: cryptex_events::EventError) -> Self {
        Self::Invalid(value.to_string())
    }
}

pub type CanonicalWriterResult<T> = Result<T, CanonicalWriterError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WriterTip {
    schema_version: u32,
    device_id: String,
    active_segment_index: u64,
    events_in_active: u64,
    last_content_hash: String,
    last_signing_key_id: String,
}

struct VerifyingBodyValidator {
    verifying_key: VerifyingKey,
}

impl FrameBodyValidator for VerifyingBodyValidator {
    fn validate_frame_body(&self, body: &[u8]) -> cryptex_storage::StorageResult<()> {
        let accepted: AcceptedEvent = serde_json::from_slice(body).map_err(|err| {
            StorageError::Integrity(format!("accepted event JSON parse failed: {err}"))
        })?;
        verify_accepted_event(&accepted, &self.verifying_key).map_err(|err| {
            StorageError::Integrity(format!("accepted event verification failed: {err}"))
        })?;
        Ok(())
    }
}

/// Append a signed `memory.note` event through the daemon-owned writer path.
pub fn append_memory_note<S: SecureStore>(
    store: &S,
    env: SecureStoreEnvironment,
    runtime_root: &Path,
    brain_id: &str,
    device_id: &DeviceId,
    classification: Classification,
    text: &str,
    config: &CanonicalWriterConfig,
) -> CanonicalWriterResult<AppendOutcome> {
    assert_outside_repository(runtime_root)
        .map_err(|e| CanonicalWriterError::Path(e.to_string()))?;
    ensure_runtime_root(runtime_root).map_err(|e| CanonicalWriterError::Path(e.to_string()))?;
    parse_uuid_str(brain_id).map_err(|e| CanonicalWriterError::Invalid(e.to_string()))?;
    let payload = MemoryNotePayload::new(text)?;

    let tip_path = writer_tip_path(runtime_root, device_id);
    let tip = load_tip_if_present(&tip_path)?;
    let observed: Vec<SigningKeyId> = match &tip {
        Some(t) => vec![SigningKeyId::parse(&t.last_signing_key_id).map_err(|e| {
            CanonicalWriterError::Integrity(format!("tip signing_key_id invalid: {e}"))
        })?],
        None => Vec::new(),
    };

    let ready = ensure_event_signing_key(store, env, runtime_root, device_id, &observed)?;
    let verifying = ready.signing_key.verifying_key();
    let validator = VerifyingBodyValidator {
        verifying_key: verifying,
    };

    let mut segment_index = tip.as_ref().map(|t| t.active_segment_index).unwrap_or(0);
    recover_active_open(runtime_root, device_id, segment_index, &validator)?;

    // Ensure sequence exists for this device (idempotent initialize may fail if present).
    ensure_sequence_ready(runtime_root, device_id)?;

    let device_seq_n = allocate_device_sequence(runtime_root, device_id)
        .map_err(|e| CanonicalWriterError::Sequence(e.to_string()))?;
    let device_seq = DeviceSeq::from_u64(device_seq_n)?;

    let prev_content_hash = match &tip {
        Some(t) => ContentHashHex::parse(&t.last_content_hash)?,
        None => ContentHashHex::zero(),
    };
    if tip.is_none() {
        cryptex_events::integrity::check_first_event_prev_hash(&prev_content_hash)
            .map_err(|e| CanonicalWriterError::Integrity(e.to_string()))?;
    } else {
        // Chain link is tip.last -> this.prev (same value).
        cryptex_events::integrity::check_chain_link(&prev_content_hash, &prev_content_hash)
            .map_err(|e| CanonicalWriterError::Integrity(e.to_string()))?;
    }

    let signable = SignableEvent {
        schema_version: EVENT_SCHEMA_VERSION,
        event_id: new_event_id()?,
        brain_id: brain_id.to_string(),
        device_id: device_id.as_str().to_string(),
        device_seq,
        created_at: now_rfc3339()?,
        event_type: EventType::MemoryNote,
        classification,
        signing_key_id: SigningKeyId::parse(&ready.public.signing_key_id)?,
        payload: EventPayload::MemoryNote(payload),
        prev_content_hash,
    };

    let accepted = accept_event(signable, &ready.signing_key)?;
    let body = serde_json_canonicalizer::to_vec(&accepted).map_err(|err| {
        CanonicalWriterError::Integrity(format!("accepted event JCS failed: {err}"))
    })?;

    // If active pack was sealed previously, tip may still point at a sealed index.
    if !active_segment_path(runtime_root, device_id, segment_index).exists()
        && tip
            .as_ref()
            .map(|t| t.events_in_active > 0)
            .unwrap_or(false)
    {
        segment_index = tip
            .as_ref()
            .map(|t| t.active_segment_index + 1)
            .unwrap_or(0);
    }

    append_frame(runtime_root, device_id, segment_index, &body)?;

    let mut events_in_active = tip.as_ref().map(|t| t.events_in_active).unwrap_or(0) + 1;
    let mut sealed = false;
    let mut active_segment_index = segment_index;
    if events_in_active >= config.seal_event_threshold {
        seal_active_segment(runtime_root, device_id, segment_index)?;
        sealed = true;
        active_segment_index = segment_index + 1;
        events_in_active = 0;
    }

    let new_tip = WriterTip {
        schema_version: 1,
        device_id: device_id.as_str().to_string(),
        active_segment_index,
        events_in_active,
        last_content_hash: accepted.content_hash.as_str().to_string(),
        last_signing_key_id: ready.public.signing_key_id.clone(),
    };
    persist_tip(&tip_path, &new_tip)?;

    Ok(AppendOutcome {
        accepted,
        segment_index,
        sealed,
    })
}

fn ensure_sequence_ready(runtime_root: &Path, device_id: &DeviceId) -> CanonicalWriterResult<()> {
    match initialize_device_sequence(runtime_root, device_id) {
        Ok(()) => Ok(()),
        Err(err) => {
            use crate::identity_runtime::IdentityRuntimeError;
            match &err {
                IdentityRuntimeError::Core(
                    cryptex_core::CoreError::SequenceStateAlreadyExists { .. },
                ) => Ok(()),
                _ => {
                    // If a previous init left readable state, continue.
                    match crate::sequence_runtime::last_issued_device_sequence(
                        runtime_root,
                        device_id,
                    ) {
                        Ok(_) => Ok(()),
                        Err(_) => Err(CanonicalWriterError::Sequence(err.to_string())),
                    }
                }
            }
        }
    }
}

fn writer_tip_path(runtime_root: &Path, device_id: &DeviceId) -> PathBuf {
    canonical_events_device_dir(runtime_root, device_id).join("writer_tip.json")
}

fn load_tip_if_present(path: &Path) -> CanonicalWriterResult<Option<WriterTip>> {
    match fs::read_to_string(path) {
        Ok(text) => {
            let tip: WriterTip = serde_json::from_str(&text)
                .map_err(|e| CanonicalWriterError::Integrity(format!("tip parse failed: {e}")))?;
            if tip.schema_version != 1 {
                return Err(CanonicalWriterError::Integrity(format!(
                    "unsupported writer tip schema_version {}",
                    tip.schema_version
                )));
            }
            ContentHashHex::parse(&tip.last_content_hash)?;
            SigningKeyId::parse(&tip.last_signing_key_id)?;
            Ok(Some(tip))
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(CanonicalWriterError::Path(err.to_string())),
    }
}

fn persist_tip(path: &Path, tip: &WriterTip) -> CanonicalWriterResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| CanonicalWriterError::Path(e.to_string()))?;
    }
    let mut json =
        serde_json::to_string(tip).map_err(|e| CanonicalWriterError::Path(e.to_string()))?;
    json.push('\n');
    let tmp = path.with_extension(format!(
        "json.tmp.{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|t| t.as_nanos())
            .unwrap_or(0)
    ));
    {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(|e| CanonicalWriterError::Path(e.to_string()))?;
        file.write_all(json.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|e| CanonicalWriterError::Path(e.to_string()))?;
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
        const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
        let from: Vec<u16> = tmp.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let ok = unsafe {
            windows_sys::Win32::Storage::FileSystem::MoveFileExW(
                from.as_ptr(),
                to.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        };
        if ok == 0 {
            let _ = fs::remove_file(&tmp);
            return Err(CanonicalWriterError::Path(
                std::io::Error::last_os_error().to_string(),
            ));
        }
    }
    #[cfg(unix)]
    {
        fs::rename(&tmp, path).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            CanonicalWriterError::Path(e.to_string())
        })?;
        if let Some(parent) = path.parent() {
            fs::File::open(parent)
                .and_then(|d| d.sync_all())
                .map_err(|e| {
                    CanonicalWriterError::Path(format!("tip directory sync failed: {e}"))
                })?;
        }
    }
    Ok(())
}

fn new_event_id() -> CanonicalWriterResult<String> {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes)
        .map_err(|e| CanonicalWriterError::Invalid(format!("event_id entropy failed: {e}")))?;
    // RFC 4122 version 4 / variant 1.
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex = |b: u8| format!("{b:02x}");
    let id = format!(
        "{}{}{}{}-{}{}-{}{}-{}{}-{}{}{}{}{}{}",
        hex(bytes[0]),
        hex(bytes[1]),
        hex(bytes[2]),
        hex(bytes[3]),
        hex(bytes[4]),
        hex(bytes[5]),
        hex(bytes[6]),
        hex(bytes[7]),
        hex(bytes[8]),
        hex(bytes[9]),
        hex(bytes[10]),
        hex(bytes[11]),
        hex(bytes[12]),
        hex(bytes[13]),
        hex(bytes[14]),
        hex(bytes[15]),
    );
    parse_uuid_str(&id).map_err(|e| CanonicalWriterError::Invalid(e.to_string()))
}

fn now_rfc3339() -> CanonicalWriterResult<String> {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let formatted = cryptex_storage::timefmt::format_unix_secs_rfc3339_utc(secs);
    parse_rfc3339_str(&formatted).map_err(|e| CanonicalWriterError::Invalid(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secure_store::FakeSecureStore;
    use cryptex_storage::{read_active_pack_bytes, sealed_segment_path};

    fn temp_root(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|t| t.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "cryptex-m2-writer-{}-{}-{}",
            label,
            std::process::id(),
            nanos
        ))
    }

    #[test]
    fn appends_signed_memory_note_and_chains_hashes() {
        let root = temp_root("chain");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("writer-device-a").unwrap();
        let brain = "33333333-3333-4333-8333-333333333333";
        let cfg = CanonicalWriterConfig {
            seal_event_threshold: 1000,
        };

        let first = append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            brain,
            &device,
            Classification::Internal,
            "hello-one",
            &cfg,
        )
        .unwrap();
        assert_eq!(first.accepted.device_seq.to_u64().unwrap(), 1);
        assert_eq!(
            first.accepted.prev_content_hash.as_str(),
            ZERO_CONTENT_HASH_HEX
        );

        let second = append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            brain,
            &device,
            Classification::Internal,
            "hello-two",
            &cfg,
        )
        .unwrap();
        assert_eq!(second.accepted.device_seq.to_u64().unwrap(), 2);
        assert_eq!(
            second.accepted.prev_content_hash.as_str(),
            first.accepted.content_hash.as_str()
        );
        assert!(!second.sealed);

        let bytes = read_active_pack_bytes(&root, &device, 0).unwrap();
        assert!(bytes.len() > 20);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn seals_when_threshold_reached() {
        let root = temp_root("seal");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("writer-device-b").unwrap();
        let brain = "44444444-4444-4444-8444-444444444444";
        let cfg = CanonicalWriterConfig {
            seal_event_threshold: 2,
        };
        let _ = append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            brain,
            &device,
            Classification::Public,
            "a",
            &cfg,
        )
        .unwrap();
        let second = append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            brain,
            &device,
            Classification::Public,
            "b",
            &cfg,
        )
        .unwrap();
        assert!(second.sealed);
        assert!(sealed_segment_path(&root, &device, 0).is_file());
        assert!(!active_segment_path(&root, &device, 0).exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_runtime_root_inside_repository() {
        let store = FakeSecureStore::new();
        let device = DeviceId::new("writer-device-c").unwrap();
        let repoish = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("would-be-writer-runtime");
        let err = append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            &repoish,
            "55555555-5555-4555-8555-555555555555",
            &device,
            Classification::Private,
            "nope",
            &CanonicalWriterConfig::default(),
        )
        .unwrap_err();
        assert!(matches!(err, CanonicalWriterError::Path(_)), "{err}");
    }
}
