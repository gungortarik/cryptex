//! cryptexd-only canonical event writer (ADR-0016 §11).
//!
//! Per-device exclusive lock (`writer.lock`) is shared by the canonical writer and
//! verified reader: reconcile → recover → allocate → append/seal → tip on write, and
//! discovery → active-open recovery → verified scan/callback on read.
//! `writer_tip.json` is rebuildable derived state; verified `.seg`/`.open` records are
//! authoritative for chain tip and active segment discovery.

use std::fs::{self, File, OpenOptions};
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
#[cfg(test)]
use cryptex_events::hex::ZERO_CONTENT_HASH_HEX;
use cryptex_events::hex::{ContentHashHex, SigningKeyId};
use cryptex_events::integrity::{accept_event, check_chain_link, check_first_event_prev_hash};
use cryptex_events::limits::DEFAULT_SEAL_EVENT_THRESHOLD_HINT;
use cryptex_events::{ValidationReasonCode, VersionedRecord, read_versioned_record};
use cryptex_storage::durability::sync_parent_directory;
use cryptex_storage::layout::{
    active_segment_path, canonical_events_device_dir, sealed_segment_path,
};
use cryptex_storage::quarantine::{
    QuarantineExtras, QuarantineReasonCode, QuarantineRequest, QuarantineSourceKind,
    quarantine_file_range,
};
use cryptex_storage::recovery::{FrameBodyValidator, recover_active_open};
use cryptex_storage::stream_frames::fold_verified_frames;
use cryptex_storage::{StorageError, append_frame, seal_active_segment};
use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};

use crate::event_signing_key::{EventSigningKeyError, ensure_event_signing_key};
use crate::paths::{assert_outside_repository, ensure_runtime_root};
use crate::secure_store::{SecureStore, SecureStoreEnvironment};
use crate::sequence_runtime::{allocate_device_sequence, initialize_device_sequence};

#[cfg(test)]
thread_local! {
    static FAIL_AFTER_APPEND_BEFORE_TIP: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static FAIL_AFTER_SEAL_BEFORE_TIP: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}

/// Test-only: fail once after durable append, before tip persistence.
#[cfg(test)]
pub fn fail_after_append_before_tip() {
    FAIL_AFTER_APPEND_BEFORE_TIP.with(|f| f.set(true));
}

/// Test-only: fail once after durable seal, before tip persistence.
#[cfg(test)]
pub fn fail_after_seal_before_tip() {
    FAIL_AFTER_SEAL_BEFORE_TIP.with(|f| f.set(true));
}

#[cfg(test)]
struct BeforeDeviceLockHook {
    thread_id: std::thread::ThreadId,
    hook: std::sync::Arc<dyn Fn() + Send + Sync>,
}

#[cfg(test)]
static BEFORE_DEVICE_LOCK_EXCLUSIVE: std::sync::Mutex<Option<BeforeDeviceLockHook>> =
    std::sync::Mutex::new(None);

/// Test-only: run `hook` on `thread_id` after opening `writer.lock`, before `lock_exclusive`.
/// Scoped to one thread so parallel tests are not blocked.
#[cfg(test)]
pub(crate) fn set_before_device_lock_exclusive_hook(
    thread_id: Option<std::thread::ThreadId>,
    hook: Option<std::sync::Arc<dyn Fn() + Send + Sync>>,
) {
    let mut slot = BEFORE_DEVICE_LOCK_EXCLUSIVE
        .lock()
        .expect("lock hook mutex");
    *slot = match (thread_id, hook) {
        (Some(thread_id), Some(hook)) => Some(BeforeDeviceLockHook { thread_id, hook }),
        _ => None,
    };
}

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct WriterTip {
    schema_version: u32,
    device_id: String,
    active_segment_index: u64,
    events_in_active: u64,
    last_content_hash: String,
    last_signing_key_id: String,
    last_device_seq: u64,
}

#[derive(Debug, Clone)]
struct ChainState {
    last_content_hash: ContentHashHex,
    last_signing_key_id: Option<SigningKeyId>,
    last_device_seq: Option<u64>,
    active_segment_index: u64,
    events_in_active: u64,
    observed_signing_key_ids: Vec<SigningKeyId>,
}

struct VerifyingBodyValidator {
    verifying_key: VerifyingKey,
    /// Device directory being scanned/recovered; every AcceptedEvent must match.
    expected_device_id: DeviceId,
}

impl FrameBodyValidator for VerifyingBodyValidator {
    fn validate_frame_body(&self, body: &[u8]) -> cryptex_storage::StorageResult<()> {
        let record = read_versioned_record(body, &self.verifying_key).map_err(|failure| {
            StorageError::Validation {
                reason_code: failure.reason.as_str().to_string(),
                detail: failure.detail,
                schema_version_if_parsed: failure.schema_version_if_parsed,
            }
        })?;
        let (claimed_device_id, schema_version) = match &record {
            VersionedRecord::VerifiedV1(event) => {
                (Some(event.device_id.as_str()), Some(EVENT_SCHEMA_VERSION))
            }
            VersionedRecord::VerifiedV2Supported(event) => {
                (Some(event.event().device_id.as_str()), Some(2))
            }
            VersionedRecord::VerifiedUnsupported(event) => {
                (Some(event.device_id.as_str()), Some(2))
            }
            VersionedRecord::Unverifiable(_) => (None, None),
        };
        if claimed_device_id.is_some_and(|claimed| claimed != self.expected_device_id.as_str()) {
            return Err(StorageError::Validation {
                reason_code: ValidationReasonCode::IntegrityFailure.as_str().to_string(),
                detail: "accepted event device_id does not match segment device".into(),
                schema_version_if_parsed: schema_version,
            });
        }
        let accepted = match record {
            VersionedRecord::VerifiedV1(accepted) => accepted,
            VersionedRecord::VerifiedV2Supported(_) | VersionedRecord::VerifiedUnsupported(_) => {
                // Integrity is verified. Preserve the record in the chain and
                // leave materialization to a reader that supports its payload.
                return Ok(());
            }
            VersionedRecord::Unverifiable(record) => {
                return Err(StorageError::CompatibilityBoundary(format!(
                    "event envelope version {:?} cannot be verified by this reader",
                    record.schema_version
                )));
            }
        };
        if accepted.device_id != self.expected_device_id.as_str() {
            return Err(StorageError::Validation {
                reason_code: ValidationReasonCode::IntegrityFailure.as_str().to_string(),
                detail: format!(
                    "accepted event device_id '{}' does not match segment device '{}'",
                    accepted.device_id,
                    self.expected_device_id.as_str()
                ),
                schema_version_if_parsed: Some(EVENT_SCHEMA_VERSION),
            });
        }
        Ok(())
    }
}

/// Inputs for [`append_memory_note`] (grouped to satisfy Clippy arity limits).
#[derive(Debug, Clone, Copy)]
pub struct AppendMemoryNoteArgs<'a> {
    pub runtime_root: &'a Path,
    pub brain_id: &'a str,
    pub device_id: &'a DeviceId,
    pub classification: Classification,
    pub text: &'a str,
    pub config: &'a CanonicalWriterConfig,
}

/// Append a signed `memory.note` event through the daemon-owned writer path.
/// RAII exclusive lock for one device's canonical event directory (`writer.lock`).
///
/// Held across reconcile/recover/append on the writer path and across discovery,
/// active-open recovery, and verified scan/callback on the reader path.
/// Crate-private: not a supported external API surface.
#[derive(Debug)]
pub(crate) struct DeviceCanonicalLock {
    device_id: DeviceId,
    _file: File,
}

impl DeviceCanonicalLock {
    pub(crate) fn device_id(&self) -> &DeviceId {
        &self.device_id
    }
}

/// Acquire the per-device exclusive canonical lock.
///
/// Enforces the repository-boundary check before any filesystem mutation.
/// Then creates the device directory and `writer.lock` so a reader cannot skip
/// locking merely because packs do not exist yet (first-create race).
pub(crate) fn acquire_device_canonical_lock(
    runtime_root: &Path,
    device_id: &DeviceId,
) -> CanonicalWriterResult<DeviceCanonicalLock> {
    assert_outside_repository(runtime_root)
        .map_err(|e| CanonicalWriterError::Path(e.to_string()))?;
    let device_dir = canonical_events_device_dir(runtime_root, device_id);
    fs::create_dir_all(&device_dir).map_err(|e| CanonicalWriterError::Path(e.to_string()))?;
    let lock_path = device_dir.join("writer.lock");
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|e| CanonicalWriterError::Path(e.to_string()))?;
    #[cfg(test)]
    {
        let hook = BEFORE_DEVICE_LOCK_EXCLUSIVE
            .lock()
            .expect("lock hook mutex")
            .as_ref()
            .and_then(|entry| {
                if entry.thread_id == std::thread::current().id() {
                    Some(std::sync::Arc::clone(&entry.hook))
                } else {
                    None
                }
            });
        if let Some(hook) = hook {
            hook();
        }
    }
    fs2::FileExt::lock_exclusive(&file)
        .map_err(|e| CanonicalWriterError::Path(format!("device canonical lock failed: {e}")))?;
    Ok(DeviceCanonicalLock {
        device_id: device_id.clone(),
        _file: file,
    })
}

pub fn append_memory_note<S: SecureStore>(
    store: &S,
    env: SecureStoreEnvironment,
    args: AppendMemoryNoteArgs<'_>,
) -> CanonicalWriterResult<AppendOutcome> {
    let runtime_root = args.runtime_root;
    let device_id = args.device_id;
    assert_outside_repository(runtime_root)
        .map_err(|e| CanonicalWriterError::Path(e.to_string()))?;
    ensure_runtime_root(runtime_root).map_err(|e| CanonicalWriterError::Path(e.to_string()))?;
    let lock = acquire_device_canonical_lock(runtime_root, device_id)?;
    append_memory_note_with_lock(store, env, &lock, args)
}

/// Append a `memory.note` while `lock` is already held for `args.device_id`.
pub(crate) fn append_memory_note_with_lock<S: SecureStore>(
    store: &S,
    env: SecureStoreEnvironment,
    lock: &DeviceCanonicalLock,
    args: AppendMemoryNoteArgs<'_>,
) -> CanonicalWriterResult<AppendOutcome> {
    let AppendMemoryNoteArgs {
        runtime_root,
        brain_id,
        device_id,
        classification,
        text,
        config,
    } = args;
    if lock.device_id() != device_id {
        return Err(CanonicalWriterError::Invalid(
            "device canonical lock does not match append device_id".into(),
        ));
    }
    parse_uuid_str(brain_id).map_err(|e| CanonicalWriterError::Invalid(e.to_string()))?;
    let payload = MemoryNotePayload::new(text)?;

    // Discover packs before any key mint so missing seed+metadata cannot mint over
    // existing history (ADR-0016 §6.3).
    let (sealed_indexes, open_index) = discover_segments(runtime_root, device_id)?;
    let packs_present = !sealed_indexes.is_empty() || open_index.is_some();
    let bootstrap =
        ensure_event_signing_key(store, env, runtime_root, device_id, &[], packs_present)?;
    let verifying = bootstrap.signing_key.verifying_key();
    let validator = VerifyingBodyValidator {
        verifying_key: verifying,
        expected_device_id: device_id.clone(),
    };

    let mut state = rebuild_chain_state(runtime_root, device_id, &validator)?;
    // Re-ensure with observed key ids from verified packs (single-active-key rule).
    let ready = ensure_event_signing_key(
        store,
        env,
        runtime_root,
        device_id,
        &state.observed_signing_key_ids,
        !state.observed_signing_key_ids.is_empty() || packs_present,
    )?;
    // Rebuild again only if key identity unchanged (it must be).
    if ready.public.signing_key_id != signing_key_id_string(&bootstrap.public.signing_key_id) {
        return Err(CanonicalWriterError::Integrity(
            "signing key identity changed during writer transaction".into(),
        ));
    }
    // Refresh tip cache from authoritative rebuild (stale/corrupt/missing tip healed).
    persist_tip_from_state(
        runtime_root,
        device_id,
        &state,
        &ready.public.signing_key_id,
    )?;

    ensure_sequence_ready(runtime_root, device_id)?;
    let device_seq_n = allocate_device_sequence(runtime_root, device_id)
        .map_err(|e| CanonicalWriterError::Sequence(e.to_string()))?;
    if let Some(last) = state.last_device_seq {
        if device_seq_n <= last {
            return Err(CanonicalWriterError::Sequence(format!(
                "allocated device_seq {device_seq_n} is not strictly greater than pack tip {last}"
            )));
        }
    } else if device_seq_n != 1 {
        // First pack event should normally be 1; gaps only arise after prior allocate-without-append.
        // Allow >1 only when packs empty but sequence already advanced.
        if device_seq_n < 1 {
            return Err(CanonicalWriterError::Sequence(format!(
                "invalid allocated device_seq {device_seq_n}"
            )));
        }
    }

    let prev_content_hash = state.last_content_hash.clone();
    if state.last_device_seq.is_none() {
        check_first_event_prev_hash(&prev_content_hash)
            .map_err(|e| CanonicalWriterError::Integrity(e.to_string()))?;
    }

    let signable = SignableEvent {
        schema_version: EVENT_SCHEMA_VERSION,
        event_id: new_event_id()?,
        brain_id: brain_id.to_string(),
        device_id: device_id.as_str().to_string(),
        device_seq: DeviceSeq::from_u64(device_seq_n)?,
        created_at: now_rfc3339()?,
        event_type: EventType::MemoryNote,
        classification,
        signing_key_id: SigningKeyId::parse(&ready.public.signing_key_id)?,
        payload: EventPayload::MemoryNote(payload),
        prev_content_hash: prev_content_hash.clone(),
    };

    let accepted = accept_event(signable, &ready.signing_key)?;
    let body = serde_json_canonicalizer::to_vec(&accepted).map_err(|err| {
        CanonicalWriterError::Integrity(format!("accepted event JCS failed: {err}"))
    })?;

    let segment_index = state.active_segment_index;
    append_frame(runtime_root, device_id, segment_index, &body)?;

    #[cfg(test)]
    {
        let fail = FAIL_AFTER_APPEND_BEFORE_TIP.with(|f| {
            let armed = f.get();
            if armed {
                f.set(false);
            }
            armed
        });
        if fail {
            return Err(CanonicalWriterError::Path(
                "injected failure after durable append before tip persist".into(),
            ));
        }
    }

    let mut events_in_active = state.events_in_active + 1;
    let mut sealed = false;
    let mut active_segment_index = segment_index;
    if events_in_active >= config.seal_event_threshold {
        seal_active_segment(runtime_root, device_id, segment_index)?;
        sealed = true;
        active_segment_index = segment_index + 1;
        events_in_active = 0;

        #[cfg(test)]
        {
            let fail = FAIL_AFTER_SEAL_BEFORE_TIP.with(|f| {
                let armed = f.get();
                if armed {
                    f.set(false);
                }
                armed
            });
            if fail {
                return Err(CanonicalWriterError::Path(
                    "injected failure after durable seal before tip persist".into(),
                ));
            }
        }
    }

    state.last_content_hash = accepted.content_hash.clone();
    state.last_signing_key_id = Some(SigningKeyId::parse(&ready.public.signing_key_id)?);
    state.last_device_seq = Some(device_seq_n);
    state.active_segment_index = active_segment_index;
    state.events_in_active = events_in_active;
    if !state
        .observed_signing_key_ids
        .iter()
        .any(|id| id.as_str() == ready.public.signing_key_id)
    {
        state
            .observed_signing_key_ids
            .push(SigningKeyId::parse(&ready.public.signing_key_id)?);
    }
    persist_tip_from_state(
        runtime_root,
        device_id,
        &state,
        &ready.public.signing_key_id,
    )?;

    Ok(AppendOutcome {
        accepted,
        segment_index,
        sealed,
    })
}

fn signing_key_id_string(id: &str) -> String {
    id.to_string()
}

/// Rebuild chain tip from verified canonical packs. Tip file is not authoritative.
fn rebuild_chain_state(
    runtime_root: &Path,
    device_id: &DeviceId,
    validator: &VerifyingBodyValidator,
) -> CanonicalWriterResult<ChainState> {
    let (sealed_indexes, open_index) = discover_segments(runtime_root, device_id)?;

    let mut expected_prev = ContentHashHex::zero();
    let mut expected_seq: Option<u64> = None;
    let mut last_hash = ContentHashHex::zero();
    let mut last_key: Option<SigningKeyId> = None;
    let mut last_seq: Option<u64> = None;
    let mut observed: Vec<SigningKeyId> = Vec::new();
    let mut events_in_active = 0u64;

    for index in &sealed_indexes {
        let path = sealed_segment_path(runtime_root, device_id, *index);
        let scan = scan_pack_events(
            &path,
            device_id,
            validator,
            &mut expected_prev,
            &mut expected_seq,
        );
        match scan {
            Ok(events) => {
                // Sealed packs must be fully consumed (no truncated tail / failed frames).
                let file_len = fs::metadata(&path)
                    .map_err(|e| CanonicalWriterError::Path(e.to_string()))?
                    .len();
                let (_, verified_len) = fold_verified_frames(&path, validator, (), |_, _| Ok(()))?;
                if events.is_empty() || verified_len != file_len {
                    quarantine_sealed_pack(runtime_root, device_id, *index, &path)?;
                    let source_relative = format!(
                        "canonical/events/{}/{:020}.seg",
                        device_id.as_filename_stem(),
                        *index
                    );
                    return Err(CanonicalWriterError::Integrity(format!(
                        "sealed pack {source_relative} failed verification (verified_len={verified_len}, file_len={file_len}, events={}); quarantined",
                        events.len()
                    )));
                }
                for ev in &events {
                    absorb_event(
                        ev,
                        &mut last_hash,
                        &mut last_key,
                        &mut last_seq,
                        &mut observed,
                    )?;
                }
            }
            Err(err) => {
                if matches!(
                    &err,
                    CanonicalWriterError::Storage(StorageError::CompatibilityBoundary(_))
                ) {
                    return Err(err);
                }
                quarantine_sealed_pack(runtime_root, device_id, *index, &path)?;
                return Err(err);
            }
        }
    }

    let active_segment_index = match open_index {
        Some(idx) => {
            // Recover first (quarantine-before-truncate), then verify remaining prefix.
            recover_active_open(runtime_root, device_id, idx, validator)?;
            let path = active_segment_path(runtime_root, device_id, idx);
            if path.exists() {
                let events = scan_pack_events(
                    &path,
                    device_id,
                    validator,
                    &mut expected_prev,
                    &mut expected_seq,
                )?;
                events_in_active = events.len() as u64;
                for ev in &events {
                    absorb_event(
                        ev,
                        &mut last_hash,
                        &mut last_key,
                        &mut last_seq,
                        &mut observed,
                    )?;
                }
            }
            idx
        }
        None => {
            // No open pack: next append uses next index after last sealed, or 0.
            sealed_indexes.last().map(|i| i + 1).unwrap_or(0)
        }
    };

    // If both a sealed N and open N somehow exist, seal reconcile should have handled it;
    // discover_segments fails closed on that conflict.
    Ok(ChainState {
        last_content_hash: last_hash,
        last_signing_key_id: last_key,
        last_device_seq: last_seq,
        active_segment_index,
        events_in_active,
        observed_signing_key_ids: observed,
    })
}

fn absorb_event(
    ev: &VersionedRecord,
    last_hash: &mut ContentHashHex,
    last_key: &mut Option<SigningKeyId>,
    last_seq: &mut Option<u64>,
    observed: &mut Vec<SigningKeyId>,
) -> CanonicalWriterResult<()> {
    let (hash, key, seq) = match ev {
        VersionedRecord::VerifiedV1(ev) => (&ev.content_hash, &ev.signing_key_id, &ev.device_seq),
        VersionedRecord::VerifiedV2Supported(ev) => (
            &ev.event().content_hash,
            &ev.event().signing_key_id,
            &ev.event().device_seq,
        ),
        VersionedRecord::VerifiedUnsupported(ev) => {
            (&ev.content_hash, &ev.signing_key_id, &ev.device_seq)
        }
        VersionedRecord::Unverifiable(_) => {
            return Err(CanonicalWriterError::Integrity(
                "unverifiable record cannot advance chain state".into(),
            ));
        }
    };
    *last_hash = hash.clone();
    *last_key = Some(key.clone());
    *last_seq = Some(seq.to_u64()?);
    if !observed.iter().any(|id| id == key) {
        observed.push(key.clone());
    }
    Ok(())
}

fn scan_pack_events(
    path: &Path,
    device_id: &DeviceId,
    validator: &VerifyingBodyValidator,
    expected_prev: &mut ContentHashHex,
    expected_seq: &mut Option<u64>,
) -> CanonicalWriterResult<Vec<VersionedRecord>> {
    let (events, _verified_len) =
        fold_verified_frames(path, validator, Vec::new(), |out, body| {
            let record =
                read_versioned_record(body, &validator.verifying_key).map_err(|failure| {
                    StorageError::Validation {
                        reason_code: failure.reason.as_str().to_string(),
                        detail: failure.detail,
                        schema_version_if_parsed: failure.schema_version_if_parsed,
                    }
                })?;
            out.push(record);
            Ok(())
        })?;

    for record in &events {
        let (record_device_id, seq, prev_content_hash, content_hash) = match record {
            VersionedRecord::VerifiedV1(event) => (
                event.device_id.as_str(),
                event.device_seq.to_u64()?,
                &event.prev_content_hash,
                &event.content_hash,
            ),
            VersionedRecord::VerifiedV2Supported(event) => (
                event.event().device_id.as_str(),
                event.event().device_seq.to_u64()?,
                &event.event().prev_content_hash,
                &event.event().content_hash,
            ),
            VersionedRecord::VerifiedUnsupported(event) => (
                event.device_id.as_str(),
                event.device_seq.to_u64()?,
                &event.prev_content_hash,
                &event.content_hash,
            ),
            VersionedRecord::Unverifiable(event) => {
                return Err(CanonicalWriterError::Storage(
                    StorageError::CompatibilityBoundary(format!(
                        "event envelope version {:?} cannot be verified",
                        event.schema_version
                    )),
                ));
            }
        };
        if record_device_id != device_id.as_str() {
            return Err(CanonicalWriterError::Integrity(format!(
                "event device_id '{}' does not match segment device '{}'",
                record_device_id,
                device_id.as_str()
            )));
        }
        match expected_seq {
            None => {
                check_first_event_prev_hash(prev_content_hash)
                    .map_err(|e| CanonicalWriterError::Integrity(e.to_string()))?;
                if *prev_content_hash != *expected_prev {
                    return Err(CanonicalWriterError::Integrity(
                        "first event prev_content_hash mismatch".into(),
                    ));
                }
                *expected_seq = Some(seq);
            }
            Some(prev_seq) => {
                // ADR-0015: unique and strictly increasing; contiguity is not required.
                // Durable allocate-before-append may leave gaps after a crashed append.
                if seq <= *prev_seq {
                    return Err(CanonicalWriterError::Integrity(format!(
                        "device_seq not strictly increasing: previous {prev_seq} found {seq}"
                    )));
                }
                check_chain_link(expected_prev, prev_content_hash)
                    .map_err(|e| CanonicalWriterError::Integrity(e.to_string()))?;
                *expected_seq = Some(seq);
            }
        }
        *expected_prev = content_hash.clone();
    }
    Ok(events)
}

fn quarantine_sealed_pack(
    runtime_root: &Path,
    device_id: &DeviceId,
    index: u64,
    path: &Path,
) -> CanonicalWriterResult<()> {
    let mut file = OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(|e| CanonicalWriterError::Path(e.to_string()))?;
    let len = file
        .metadata()
        .map_err(|e| CanonicalWriterError::Path(e.to_string()))?
        .len();
    let source_relative = format!(
        "canonical/events/{}/{:020}.seg",
        device_id.as_filename_stem(),
        index
    );
    if len == 0 {
        return Err(CanonicalWriterError::Integrity(format!(
            "refusing to quarantine empty sealed pack {source_relative}"
        )));
    }
    // Stream exact sealed bytes into quarantine before isolating the canonical object.
    // ADR-0016 §7.2: quarantine the whole sealed object (no in-place edit). Durability of
    // the quarantine entry is confirmed by quarantine_file_range (object + metadata sync)
    // before the canonical .seg is removed from the discoverable pack set.
    quarantine_file_range(
        QuarantineRequest {
            app_data_root: runtime_root,
            device_id: Some(device_id),
            reason_code: QuarantineReasonCode::CorruptSealedPack,
            source_kind: QuarantineSourceKind::SealedSeg,
            source_relative: &source_relative,
            byte_offset: Some(0),
            extras: QuarantineExtras::default(),
        },
        0,
        len,
        &mut file,
    )
    .map_err(CanonicalWriterError::from)?;
    // Release the read handle before unlink (required on Windows).
    drop(file);
    fs::remove_file(path).map_err(|e| CanonicalWriterError::Path(e.to_string()))?;
    sync_parent_directory(path).map_err(CanonicalWriterError::from)?;
    Ok(())
}

fn discover_segments(
    runtime_root: &Path,
    device_id: &DeviceId,
) -> CanonicalWriterResult<(Vec<u64>, Option<u64>)> {
    let dir = canonical_events_device_dir(runtime_root, device_id);
    if !dir.exists() {
        return Ok((Vec::new(), None));
    }
    let mut sealed = Vec::new();
    let mut open: Option<u64> = None;
    for entry in fs::read_dir(&dir).map_err(|e| CanonicalWriterError::Path(e.to_string()))? {
        let entry = entry.map_err(|e| CanonicalWriterError::Path(e.to_string()))?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if let Some(idx) = parse_segment_name(&name, "seg") {
            sealed.push(idx);
        } else if let Some(idx) = parse_segment_name(&name, "open") {
            if open.is_some() {
                return Err(CanonicalWriterError::Integrity(
                    "multiple active .open packs for one device".into(),
                ));
            }
            open = Some(idx);
        }
    }
    sealed.sort_unstable();
    // Contiguous sealed indexes from 0..n-1 expected when present.
    for (i, idx) in sealed.iter().enumerate() {
        if *idx != i as u64 {
            return Err(CanonicalWriterError::Integrity(format!(
                "sealed segment indexes are not contiguous from zero: missing {i} (found {idx})"
            )));
        }
    }
    if let Some(open_idx) = open {
        if sealed.contains(&open_idx) {
            // Both .open and .seg for same index: only OK if seal reconcile can heal.
            // Attempt seal reconcile path by calling seal_active_segment which reconciles
            // identical pairs; if divergent, fail closed.
            match seal_active_segment(runtime_root, device_id, open_idx) {
                Ok(()) => {
                    // Open removed; treat as sealed only.
                    if !sealed.contains(&open_idx) {
                        sealed.push(open_idx);
                        sealed.sort_unstable();
                    }
                    open = None;
                    // After reconcile, next active is open_idx+1 if no other open.
                    if open.is_none() {
                        // fall through with open=None; active becomes last_sealed+1 in rebuild
                    }
                }
                Err(err) => {
                    return Err(CanonicalWriterError::Integrity(format!(
                        "both .open and .seg exist for segment {open_idx} and could not be reconciled: {err}"
                    )));
                }
            }
        } else if let Some(last) = sealed.last() {
            if open_idx != last + 1 {
                return Err(CanonicalWriterError::Integrity(format!(
                    "active .open index {open_idx} does not follow last sealed {}",
                    last
                )));
            }
        } else if open_idx != 0 {
            return Err(CanonicalWriterError::Integrity(format!(
                "active .open index {open_idx} must be 0 when no sealed packs exist"
            )));
        }
    }
    Ok((sealed, open))
}

fn parse_segment_name(name: &str, ext: &str) -> Option<u64> {
    let suffix = format!(".{ext}");
    let stem = name.strip_suffix(&suffix)?;
    if stem.len() != 20 || !stem.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    stem.parse::<u64>().ok()
}

fn persist_tip_from_state(
    runtime_root: &Path,
    device_id: &DeviceId,
    state: &ChainState,
    signing_key_id: &str,
) -> CanonicalWriterResult<()> {
    let tip = WriterTip {
        schema_version: 1,
        device_id: device_id.as_str().to_string(),
        active_segment_index: state.active_segment_index,
        events_in_active: state.events_in_active,
        last_content_hash: state.last_content_hash.as_str().to_string(),
        last_signing_key_id: state
            .last_signing_key_id
            .as_ref()
            .map(|id| id.as_str().to_string())
            .unwrap_or_else(|| signing_key_id.to_string()),
        last_device_seq: state.last_device_seq.unwrap_or(0),
    };
    persist_tip(&writer_tip_path(runtime_root, device_id), &tip)
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
                _ => match crate::sequence_runtime::last_issued_device_sequence(
                    runtime_root,
                    device_id,
                ) {
                    Ok(_) => Ok(()),
                    Err(_) => Err(CanonicalWriterError::Sequence(err.to_string())),
                },
            }
        }
    }
}

fn writer_tip_path(runtime_root: &Path, device_id: &DeviceId) -> PathBuf {
    canonical_events_device_dir(runtime_root, device_id).join("writer_tip.json")
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

/// Visit verified accepted events for a device without retaining the full history Vec.
///
/// Uses the same discover / validate / sealed fail-closed quarantine / active-open
/// recover-then-prefix rules as [`load_verified_events`]. Events are yielded one pack
/// at a time so large histories need not be fully buffered.
pub fn for_each_verified_event<S, F>(
    store: &S,
    env: SecureStoreEnvironment,
    runtime_root: &Path,
    device_id: &DeviceId,
    on_event: F,
) -> CanonicalWriterResult<()>
where
    S: SecureStore,
    F: FnMut(&AcceptedEvent) -> CanonicalWriterResult<()>,
{
    let _lock = acquire_device_canonical_lock(runtime_root, device_id)?;
    for_each_verified_event_with_lock(store, env, runtime_root, device_id, &_lock, on_event)
}

/// Verified feed under an already-held [`DeviceCanonicalLock`].
///
/// Callers that also open a SQLite transaction must acquire the device lock
/// *before* beginning that transaction (lock order: device → SQLite).
pub(crate) fn for_each_verified_event_with_lock<S, F>(
    store: &S,
    env: SecureStoreEnvironment,
    runtime_root: &Path,
    device_id: &DeviceId,
    lock: &DeviceCanonicalLock,
    mut on_event: F,
) -> CanonicalWriterResult<()>
where
    S: SecureStore,
    F: FnMut(&AcceptedEvent) -> CanonicalWriterResult<()>,
{
    if lock.device_id() != device_id {
        return Err(CanonicalWriterError::Invalid(
            "device canonical lock does not match verified-reader device_id".into(),
        ));
    }
    let (sealed, open) = discover_segments(runtime_root, device_id)?;
    let packs_present = !sealed.is_empty() || open.is_some();
    let ready = ensure_event_signing_key(store, env, runtime_root, device_id, &[], packs_present)?;
    let validator = VerifyingBodyValidator {
        verifying_key: ready.signing_key.verifying_key(),
        expected_device_id: device_id.clone(),
    };
    let mut expected_prev = ContentHashHex::zero();
    let mut expected_seq: Option<u64> = None;
    for index in sealed {
        let path = sealed_segment_path(runtime_root, device_id, index);
        let events = scan_pack_events(
            &path,
            device_id,
            &validator,
            &mut expected_prev,
            &mut expected_seq,
        )?;
        // Sealed packs must be fully consumed (same fail-closed rule as writer rebuild).
        let file_len = fs::metadata(&path)
            .map_err(|e| CanonicalWriterError::Path(e.to_string()))?
            .len();
        let (_, verified_len) = fold_verified_frames(&path, &validator, (), |_, _| Ok(()))?;
        if events.is_empty() || verified_len != file_len {
            quarantine_sealed_pack(runtime_root, device_id, index, &path)?;
            let source_relative = format!(
                "canonical/events/{}/{:020}.seg",
                device_id.as_filename_stem(),
                index
            );
            return Err(CanonicalWriterError::Integrity(format!(
                "sealed pack {source_relative} failed verification (verified_len={verified_len}, file_len={file_len}, events={}); quarantined",
                events.len()
            )));
        }
        for record in &events {
            match record {
                VersionedRecord::VerifiedV1(ev) => on_event(ev)?,
                VersionedRecord::VerifiedV2Supported(_)
                | VersionedRecord::VerifiedUnsupported(_) => {
                    return Err(CanonicalWriterError::Storage(StorageError::CompatibilityBoundary(
                        "verified v2 records require M4 retrieval-safety and materialization support".into(),
                    )));
                }
                VersionedRecord::Unverifiable(_) => {
                    return Err(CanonicalWriterError::Storage(
                        StorageError::CompatibilityBoundary(
                            "unverifiable event blocks retrieval".into(),
                        ),
                    ));
                }
            }
        }
    }
    if let Some(idx) = open {
        recover_active_open(runtime_root, device_id, idx, &validator)?;
        let path = active_segment_path(runtime_root, device_id, idx);
        if path.exists() {
            let events = scan_pack_events(
                &path,
                device_id,
                &validator,
                &mut expected_prev,
                &mut expected_seq,
            )?;
            for record in &events {
                match record {
                    VersionedRecord::VerifiedV1(ev) => on_event(ev)?,
                    VersionedRecord::VerifiedV2Supported(_)
                    | VersionedRecord::VerifiedUnsupported(_) => {
                        return Err(CanonicalWriterError::Storage(StorageError::CompatibilityBoundary(
                            "verified v2 records require M4 retrieval-safety and materialization support".into(),
                        )));
                    }
                    VersionedRecord::Unverifiable(_) => {
                        return Err(CanonicalWriterError::Storage(
                            StorageError::CompatibilityBoundary(
                                "unverifiable event blocks retrieval".into(),
                            ),
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

/// Load all verified accepted events for a device (test / diagnostics).
///
/// Prefer [`for_each_verified_event`] when projecting large histories.
pub fn load_verified_events<S: SecureStore>(
    store: &S,
    env: SecureStoreEnvironment,
    runtime_root: &Path,
    device_id: &DeviceId,
) -> CanonicalWriterResult<Vec<AcceptedEvent>> {
    let mut all = Vec::new();
    for_each_verified_event(store, env, runtime_root, device_id, |ev| {
        all.push(ev.clone());
        Ok(())
    })?;
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secure_store::FakeSecureStore;
    use cryptex_events::framing::encode_frame_header;
    use cryptex_storage::layout::quarantine_events_root;
    use cryptex_storage::{event_signing_key_path, read_active_pack_bytes, sealed_segment_path};
    use ed25519_dalek::Signer;
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::sync::{Arc, Barrier};
    use std::thread;

    fn temp_root(label: &str) -> PathBuf {
        let base = {
            #[cfg(windows)]
            {
                let local = std::env::var_os("LOCALAPPDATA").expect("LOCALAPPDATA");
                PathBuf::from(local)
                    .join("Cryptex-Dev")
                    .join("p0-2")
                    .join("m2-tests")
            }
            #[cfg(target_os = "macos")]
            {
                let home = std::env::var_os("HOME").expect("HOME");
                PathBuf::from(home)
                    .join("Library")
                    .join("Application Support")
                    .join("Cryptex-Dev")
                    .join("p0-2")
                    .join("m2-tests")
            }
            #[cfg(not(any(windows, target_os = "macos")))]
            {
                std::env::temp_dir()
                    .join("Cryptex-Dev")
                    .join("p0-2")
                    .join("m2-tests")
            }
        };
        crate::test_temp::exclusive_temp_dir_in(&base, &format!("m2-t-w-{label}"))
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
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Internal,
                text: "hello-one",
                config: &cfg,
            },
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
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Internal,
                text: "hello-two",
                config: &cfg,
            },
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
    fn unknown_envelope_in_active_pack_is_preserved_and_blocks_legacy_append() {
        let root = temp_root("unknown-envelope-open");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("writer-unknown-open").unwrap();
        append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: "33333333-3333-4333-8333-333333333333",
                device_id: &device,
                classification: Classification::Internal,
                text: "synthetic v1 prefix",
                config: &CanonicalWriterConfig::default(),
            },
        )
        .unwrap();
        let unknown = br#"{"schema_version":77,"opaque":"synthetic-future-record"}"#;
        write_crafted_body_frame(&root, &device, unknown);
        let path = active_segment_path(&root, &device, 0);
        let before = fs::read(&path).unwrap();

        let err =
            load_verified_events(&store, SecureStoreEnvironment::Dev, &root, &device).unwrap_err();
        assert!(matches!(
            err,
            CanonicalWriterError::Storage(StorageError::CompatibilityBoundary(_))
        ));
        assert_eq!(fs::read(&path).unwrap(), before);
        assert!(!quarantine_events_root(&root).exists());

        let append_err = append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: "33333333-3333-4333-8333-333333333333",
                device_id: &device,
                classification: Classification::Internal,
                text: "must not cross unknown boundary",
                config: &CanonicalWriterConfig::default(),
            },
        )
        .unwrap_err();
        assert!(matches!(
            append_err,
            CanonicalWriterError::Storage(StorageError::CompatibilityBoundary(_))
        ));
        assert_eq!(fs::read(&path).unwrap(), before);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn unknown_envelope_in_sealed_pack_is_not_quarantined_or_removed() {
        let root = temp_root("unknown-envelope-sealed");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("writer-unknown-sealed").unwrap();
        append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: "33333333-3333-4333-8333-333333333333",
                device_id: &device,
                classification: Classification::Internal,
                text: "synthetic v1 prefix",
                config: &CanonicalWriterConfig::default(),
            },
        )
        .unwrap();
        write_crafted_body_frame(
            &root,
            &device,
            br#"{"schema_version":77,"opaque":"synthetic-future-record"}"#,
        );
        seal_active_segment(&root, &device, 0).unwrap();
        let path = sealed_segment_path(&root, &device, 0);
        let before = fs::read(&path).unwrap();

        let err =
            load_verified_events(&store, SecureStoreEnvironment::Dev, &root, &device).unwrap_err();
        assert!(matches!(
            err,
            CanonicalWriterError::Storage(StorageError::CompatibilityBoundary(_))
        ));
        assert_eq!(fs::read(&path).unwrap(), before);
        assert!(path.is_file());
        assert!(!quarantine_events_root(&root).exists());
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
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Public,
                text: "a",
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
                text: "b",
                config: &cfg,
            },
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
        let cfg = CanonicalWriterConfig::default();
        let err = append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &repoish,
                brain_id: "55555555-5555-4555-8555-555555555555",
                device_id: &device,
                classification: Classification::Private,
                text: "nope",
                config: &cfg,
            },
        )
        .unwrap_err();
        assert!(matches!(err, CanonicalWriterError::Path(_)), "{err}");
        assert!(
            !repoish.exists(),
            "rejected runtime root must not create directories"
        );
        assert!(
            !canonical_events_device_dir(&repoish, &device).exists(),
            "rejected root must leave no canonical device directory"
        );
    }

    #[test]
    fn verified_reader_rejects_repo_root_without_creating_device_dir() {
        let store = FakeSecureStore::new();
        let device = DeviceId::new("writer-device-repo-read").unwrap();
        let repoish = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("would-be-reader-runtime");
        let err = for_each_verified_event(
            &store,
            SecureStoreEnvironment::Dev,
            &repoish,
            &device,
            |_| Ok(()),
        )
        .unwrap_err();
        assert!(matches!(err, CanonicalWriterError::Path(_)), "{err}");
        assert!(!repoish.exists());
        assert!(!canonical_events_device_dir(&repoish, &device).exists());
        assert!(
            !acquire_device_canonical_lock(&repoish, &device).is_ok(),
            "direct acquire must also refuse"
        );
        assert!(!canonical_events_device_dir(&repoish, &device).exists());
    }

    #[test]
    fn locked_helpers_reject_device_id_mismatch() {
        let root = temp_root("lock-mismatch");
        let store = FakeSecureStore::new();
        let device_a = DeviceId::new("writer-device-a-lock").unwrap();
        let device_b = DeviceId::new("writer-device-b-lock").unwrap();
        let brain = "55555555-5555-4555-8555-555555555555";
        let cfg = CanonicalWriterConfig {
            seal_event_threshold: 1000,
        };
        let lock_a = acquire_device_canonical_lock(&root, &device_a).unwrap();
        let err = append_memory_note_with_lock(
            &store,
            SecureStoreEnvironment::Dev,
            &lock_a,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device_b,
                classification: Classification::Internal,
                text: "wrong-device",
                config: &cfg,
            },
        )
        .unwrap_err();
        assert!(matches!(err, CanonicalWriterError::Invalid(_)), "{err}");
        let err = for_each_verified_event_with_lock(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            &device_b,
            &lock_a,
            |_| Ok(()),
        )
        .unwrap_err();
        assert!(matches!(err, CanonicalWriterError::Invalid(_)), "{err}");
        drop(lock_a);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn retry_after_append_before_tip_does_not_fork_chain() {
        let root = temp_root("crash-tip");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("writer-device-d").unwrap();
        let brain = "66666666-6666-4666-8666-666666666666";
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
                text: "durable-one",
                config: &cfg,
            },
        )
        .unwrap();

        fail_after_append_before_tip();
        let err = append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Internal,
                text: "durable-two",
                config: &cfg,
            },
        )
        .unwrap_err();
        assert!(matches!(err, CanonicalWriterError::Path(_)), "{err}");

        // Stale tip: still points at first event (or missing). Corrupt tip deliberately.
        let tip = writer_tip_path(&root, &device);
        fs::write(&tip, b"{not-json\n").unwrap();

        let retry = append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Internal,
                text: "durable-three-retry",
                config: &cfg,
            },
        )
        .unwrap();

        let events =
            load_verified_events(&store, SecureStoreEnvironment::Dev, &root, &device).unwrap();
        // first + durable append from crashed attempt + retry
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].content_hash, first.accepted.content_hash);
        assert_eq!(events[1].prev_content_hash, events[0].content_hash);
        assert_eq!(events[2].prev_content_hash, events[1].content_hash);
        assert_eq!(events[2].content_hash, retry.accepted.content_hash);
        assert!(
            retry.accepted.device_seq.to_u64().unwrap() > events[1].device_seq.to_u64().unwrap()
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn retry_after_seal_before_tip_rebuilds_from_sealed_pack() {
        let root = temp_root("crash-seal");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("writer-device-e").unwrap();
        let brain = "77777777-7777-4777-8777-777777777777";
        let cfg = CanonicalWriterConfig {
            seal_event_threshold: 1,
        };

        fail_after_seal_before_tip();
        let err = append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Public,
                text: "seal-me",
                config: &cfg,
            },
        )
        .unwrap_err();
        assert!(matches!(err, CanonicalWriterError::Path(_)), "{err}");
        assert!(sealed_segment_path(&root, &device, 0).is_file());
        assert!(!active_segment_path(&root, &device, 0).exists());

        // Tip missing/stale.
        let tip = writer_tip_path(&root, &device);
        let _ = fs::remove_file(&tip);

        let second = append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Public,
                text: "after-seal-retry",
                config: &cfg,
            },
        )
        .unwrap();
        assert!(second.sealed);
        let events =
            load_verified_events(&store, SecureStoreEnvironment::Dev, &root, &device).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[1].prev_content_hash, events[0].content_hash);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn verified_reader_acquires_lock_file_even_when_device_dir_missing() {
        let root = temp_root("first-create");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("writer-device-first").unwrap();
        let device_dir = canonical_events_device_dir(&root, &device);
        assert!(!device_dir.exists());
        for_each_verified_event(&store, SecureStoreEnvironment::Dev, &root, &device, |_| {
            Ok(())
        })
        .expect("empty device read");
        assert!(device_dir.is_dir());
        assert!(device_dir.join("writer.lock").is_file());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn verified_reader_waits_for_device_lock_and_does_not_truncate_in_progress_append() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let root = temp_root("reader-lock");
        let store = Arc::new(FakeSecureStore::new());
        let device = DeviceId::new("writer-device-lock").unwrap();
        let brain = "99999999-9999-4999-8999-999999999999";
        let cfg = CanonicalWriterConfig {
            seal_event_threshold: 1000,
        };
        append_memory_note(
            store.as_ref(),
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Internal,
                text: "seed-before-race",
                config: &cfg,
            },
        )
        .unwrap();

        let open_path = active_segment_path(&root, &device, 0);
        assert!(open_path.is_file());
        let before_len = fs::metadata(&open_path).unwrap().len();

        // Ordering handshakes:
        // 1) writer_holding: writer owns lock + incomplete trailing bytes
        // 2) reader_start: main allows reader to enter for_each/acquire
        // 3) reader_at_lock: test hook fires after open(lock file), before lock_exclusive
        // 4) writer_release: writer may finish append and drop the lock
        let writer_holding = Arc::new(Barrier::new(2));
        let reader_start = Arc::new(Barrier::new(2));
        let reader_at_lock = Arc::new(Barrier::new(2));
        let writer_release = Arc::new(Barrier::new(2));
        let reader_finished = Arc::new(AtomicBool::new(false));

        let writer_root = root.clone();
        let writer_device = device.clone();
        let writer_store = Arc::clone(&store);
        let writer_holding_b = Arc::clone(&writer_holding);
        let writer_release_b = Arc::clone(&writer_release);
        let writer_open_path = open_path.clone();
        let writer = thread::spawn(move || {
            let lock = acquire_device_canonical_lock(&writer_root, &writer_device).unwrap();
            {
                let mut file = OpenOptions::new()
                    .append(true)
                    .open(&writer_open_path)
                    .unwrap();
                file.write_all(&[0xA5; 19]).unwrap();
                file.sync_all().unwrap();
            }
            assert!(fs::metadata(&writer_open_path).unwrap().len() > before_len);

            writer_holding_b.wait();
            writer_release_b.wait();

            {
                let file = OpenOptions::new()
                    .write(true)
                    .open(&writer_open_path)
                    .unwrap();
                file.set_len(before_len).unwrap();
                file.sync_all().unwrap();
            }
            append_memory_note_with_lock(
                writer_store.as_ref(),
                SecureStoreEnvironment::Dev,
                &lock,
                AppendMemoryNoteArgs {
                    runtime_root: &writer_root,
                    brain_id: brain,
                    device_id: &writer_device,
                    classification: Classification::Internal,
                    text: "completed-under-exclusion",
                    config: &cfg,
                },
            )
            .unwrap();
            drop(lock);
        });

        let reader_root = root.clone();
        let reader_device = device.clone();
        let reader_store = Arc::clone(&store);
        let reader_start_b = Arc::clone(&reader_start);
        let reader_finished_flag = Arc::clone(&reader_finished);
        let reader = thread::spawn(move || {
            reader_start_b.wait();
            for_each_verified_event(
                reader_store.as_ref(),
                SecureStoreEnvironment::Dev,
                &reader_root,
                &reader_device,
                |_| Ok(()),
            )
            .expect("verified reader after writer release");
            reader_finished_flag.store(true, Ordering::SeqCst);
        });

        // Writer holds exclusive lock with in-progress trailing bytes.
        writer_holding.wait();
        let mid_len = fs::metadata(&open_path).unwrap().len();
        assert!(mid_len > before_len);

        // Install a thread-scoped handshake only after the writer owns the lock.
        // Scoped to the reader thread so parallel tests are unaffected.
        let at_lock = Arc::clone(&reader_at_lock);
        set_before_device_lock_exclusive_hook(
            Some(reader.thread().id()),
            Some(Arc::new(move || {
                at_lock.wait();
            })),
        );

        // Allow reader to enter for_each/acquire; barrier proves it reached the lock site.
        reader_start.wait();
        reader_at_lock.wait();
        assert!(
            !reader_finished.load(Ordering::SeqCst),
            "reader must still be blocked on the shared device lock"
        );
        let lock_path = canonical_events_device_dir(&root, &device).join("writer.lock");
        let probe = OpenOptions::new()
            .create(false)
            .read(true)
            .write(true)
            .open(&lock_path)
            .unwrap();
        assert!(
            fs2::FileExt::try_lock_exclusive(&probe).is_err(),
            "device lock must remain held by the writer while the reader waits"
        );
        assert_eq!(
            fs::metadata(&open_path).unwrap().len(),
            mid_len,
            "verified reader must not recover/truncate while blocked on the lock"
        );

        writer_release.wait();
        writer.join().expect("writer thread");
        reader.join().expect("reader thread");
        set_before_device_lock_exclusive_hook(None, None);
        assert!(reader_finished.load(Ordering::SeqCst));

        let events =
            load_verified_events(store.as_ref(), SecureStoreEnvironment::Dev, &root, &device)
                .expect("completed append must remain verifiable");
        assert_eq!(events.len(), 2);
        match &events[1].payload {
            cryptex_events::envelope::EventPayload::MemoryNote(p) => {
                assert_eq!(p.text, "completed-under-exclusion");
            }
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn concurrent_same_device_writers_preserve_unique_seq_and_chain() {
        let root = temp_root("conc");
        let store = Arc::new(FakeSecureStore::new());
        let device = DeviceId::new("writer-device-f").unwrap();
        let brain = "88888888-8888-4888-8888-888888888888";
        let cfg = CanonicalWriterConfig {
            seal_event_threshold: 1000,
        };
        let barrier = Arc::new(Barrier::new(8));
        let mut handles = Vec::new();
        for i in 0..8 {
            let store = Arc::clone(&store);
            let root = root.clone();
            let device = device.clone();
            let barrier = Arc::clone(&barrier);
            let cfg = cfg.clone();
            handles.push(thread::spawn(move || {
                barrier.wait();
                append_memory_note(
                    store.as_ref(),
                    SecureStoreEnvironment::Dev,
                    AppendMemoryNoteArgs {
                        runtime_root: &root,
                        brain_id: brain,
                        device_id: &device,
                        classification: Classification::Internal,
                        text: &format!("concurrent-{i}"),
                        config: &cfg,
                    },
                )
            }));
        }
        let mut outcomes = Vec::new();
        for h in handles {
            outcomes.push(h.join().unwrap().expect("append"));
        }
        assert_eq!(outcomes.len(), 8);
        let mut seqs: Vec<u64> = outcomes
            .iter()
            .map(|o| o.accepted.device_seq.to_u64().unwrap())
            .collect();
        seqs.sort_unstable();
        seqs.dedup();
        assert_eq!(seqs.len(), 8, "sequences must be unique");

        let events =
            load_verified_events(store.as_ref(), SecureStoreEnvironment::Dev, &root, &device)
                .unwrap();
        assert_eq!(events.len(), 8);
        assert_eq!(events[0].prev_content_hash.as_str(), ZERO_CONTENT_HASH_HEX);
        for window in events.windows(2) {
            assert_eq!(window[1].prev_content_hash, window[0].content_hash);
            assert!(
                window[1].device_seq.to_u64().unwrap() > window[0].device_seq.to_u64().unwrap()
            );
        }
        let _ = fs::remove_dir_all(&root);
    }

    fn write_crafted_body_frame(root: &Path, device: &DeviceId, body: &[u8]) {
        let path = active_segment_path(root, device, 0);
        assert!(
            path.is_file(),
            "active segment must exist before crafting corrupt frame: {}",
            path.display()
        );
        let header = encode_frame_header(body.len() as u32).unwrap();
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .unwrap();
        file.write_all(&header).unwrap();
        file.write_all(body).unwrap();
        file.sync_all().unwrap();
    }

    fn craft_signed_v2_unknown_body(
        store: &FakeSecureStore,
        root: &Path,
        device: &DeviceId,
        brain_id: &str,
        seq: u64,
        prev_hash: &ContentHashHex,
    ) -> Vec<u8> {
        let ready =
            ensure_event_signing_key(store, SecureStoreEnvironment::Dev, root, device, &[], true)
                .unwrap();
        let mut record = serde_json::json!({
            "schema_version": 2,
            "event_id": new_event_id().unwrap(),
            "brain_id": brain_id,
            "device_id": device.as_str(),
            "device_seq": seq.to_string(),
            "created_at": now_rfc3339().unwrap(),
            "event_type": "future.synthetic",
            "payload_schema_version": 1,
            "classification": "internal",
            "signing_key_id": ready.public.signing_key_id.clone(),
            "payload": {"opaque": "synthetic-only"},
            "prev_content_hash": prev_hash.as_str(),
            "content_hash": "00".repeat(32),
            "signature": "00".repeat(64),
        });
        let mut signable = record.clone();
        signable.as_object_mut().unwrap().remove("content_hash");
        signable.as_object_mut().unwrap().remove("signature");
        let canonical = serde_json_canonicalizer::to_vec(&signable).unwrap();
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"cryptex-event-content-v2");
        hasher.update(&[0]);
        hasher.update(&canonical);
        let hash = *hasher.finalize().as_bytes();
        let signature = ready
            .signing_key
            .sign(&[&b"cryptex-event-sig-v2"[..], &[0], hash.as_slice()].concat());
        record.as_object_mut().unwrap().insert(
            "content_hash".into(),
            serde_json::Value::String(ContentHashHex::from_bytes(&hash).unwrap().as_str().into()),
        );
        record.as_object_mut().unwrap().insert(
            "signature".into(),
            serde_json::Value::String(
                cryptex_events::SignatureHex::from_bytes(&signature.to_bytes())
                    .unwrap()
                    .as_str()
                    .into(),
            ),
        );
        serde_json_canonicalizer::to_vec(&record).unwrap()
    }

    #[test]
    fn verified_unsupported_v2_is_chain_scanned_but_blocks_m3_retrieval() {
        let root = temp_root("verified-unsupported-v2");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("writer-v2-unsupported").unwrap();
        let brain = "33333333-3333-4333-8333-333333333333";
        let config = CanonicalWriterConfig::default();
        let first = append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Internal,
                text: "synthetic v1 prefix",
                config: &config,
            },
        )
        .unwrap();
        let v2_seq = allocate_device_sequence(&root, &device).unwrap();
        let body = craft_signed_v2_unknown_body(
            &store,
            &root,
            &device,
            brain,
            v2_seq,
            &first.accepted.content_hash,
        );
        write_crafted_body_frame(&root, &device, &body);
        let path = active_segment_path(&root, &device, 0);
        let bytes_with_v2 = fs::read(&path).unwrap();

        let retrieval =
            load_verified_events(&store, SecureStoreEnvironment::Dev, &root, &device).unwrap_err();
        assert!(matches!(
            retrieval,
            CanonicalWriterError::Storage(StorageError::CompatibilityBoundary(_))
        ));
        assert_eq!(fs::read(&path).unwrap(), bytes_with_v2);

        let third = append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Internal,
                text: "synthetic v1 after verified unsupported v2",
                config: &config,
            },
        )
        .unwrap();
        assert_eq!(third.accepted.device_seq.to_u64().unwrap(), v2_seq + 1);
        assert_eq!(
            third.accepted.prev_content_hash.as_str(),
            serde_json::from_slice::<serde_json::Value>(&body).unwrap()["content_hash"]
                .as_str()
                .unwrap()
        );
        let _ = fs::remove_dir_all(&root);
    }

    fn craft_signed_mismatched_device_body(
        store: &FakeSecureStore,
        root: &Path,
        directory_device: &DeviceId,
        claimed_device_id: &str,
        prev_content_hash: &ContentHashHex,
        device_seq: u64,
        brain: &str,
    ) -> Vec<u8> {
        let ready = ensure_event_signing_key(
            store,
            SecureStoreEnvironment::Dev,
            root,
            directory_device,
            &[],
            false,
        )
        .unwrap();
        let signable = SignableEvent {
            schema_version: EVENT_SCHEMA_VERSION,
            event_id: new_event_id().unwrap(),
            brain_id: brain.to_string(),
            device_id: claimed_device_id.to_string(),
            device_seq: DeviceSeq::from_u64(device_seq).unwrap(),
            created_at: now_rfc3339().unwrap(),
            event_type: EventType::MemoryNote,
            classification: Classification::Internal,
            signing_key_id: SigningKeyId::parse(&ready.public.signing_key_id).unwrap(),
            payload: EventPayload::MemoryNote(MemoryNotePayload::new("device-mismatch").unwrap()),
            prev_content_hash: prev_content_hash.clone(),
        };
        let accepted = accept_event(signable, &ready.signing_key).unwrap();
        serde_json_canonicalizer::to_vec(&accepted).unwrap()
    }

    #[test]
    fn mismatched_device_id_in_active_open_is_quarantined_and_prefix_kept() {
        let root = temp_root("dev-mismatch-open");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("wd-open").unwrap();
        let brain = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
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
                text: "prefix-ok",
                config: &cfg,
            },
        )
        .unwrap();

        let bad = craft_signed_mismatched_device_body(
            &store,
            &root,
            &device,
            "other-device-id",
            &first.accepted.content_hash,
            2,
            brain,
        );
        write_crafted_body_frame(&root, &device, &bad);

        let second = append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Internal,
                text: "after-recovery",
                config: &cfg,
            },
        )
        .unwrap();
        assert_eq!(second.accepted.device_id, device.as_str());
        assert_eq!(
            second.accepted.prev_content_hash.as_str(),
            first.accepted.content_hash.as_str()
        );

        let qroot = quarantine_events_root(&root);
        let mut found = false;
        for entry in walkdir_quarantine(&qroot) {
            let meta = fs::read_to_string(entry.join("quarantine.json")).unwrap();
            if meta.contains("integrity_failure") {
                found = true;
                break;
            }
        }
        assert!(
            found,
            "expected integrity_failure quarantine for device_id mismatch"
        );

        let events =
            load_verified_events(&store, SecureStoreEnvironment::Dev, &root, &device).unwrap();
        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|e| e.device_id == device.as_str()));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn mismatched_device_id_in_sealed_pack_fails_closed_and_quarantines() {
        let root = temp_root("dev-mismatch-seal");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("wd-seal").unwrap();
        let brain = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
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
                classification: Classification::Public,
                text: "seal-prefix",
                config: &cfg,
            },
        )
        .unwrap();

        let bad = craft_signed_mismatched_device_body(
            &store,
            &root,
            &device,
            "other-device-id",
            &first.accepted.content_hash,
            2,
            brain,
        );
        write_crafted_body_frame(&root, &device, &bad);
        seal_active_segment(&root, &device, 0).unwrap();
        assert!(sealed_segment_path(&root, &device, 0).is_file());

        let err = append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Public,
                text: "must-fail-closed",
                config: &cfg,
            },
        )
        .unwrap_err();
        assert!(
            matches!(err, CanonicalWriterError::Integrity(_)),
            "got {err:?}"
        );

        let qroot = quarantine_events_root(&root);
        let mut found = false;
        for entry in walkdir_quarantine(&qroot) {
            let meta = fs::read_to_string(entry.join("quarantine.json")).unwrap();
            if meta.contains("integrity_failure") || meta.contains("corrupt_sealed_pack") {
                found = true;
                break;
            }
        }
        assert!(
            found,
            "expected sealed pack quarantine after device_id mismatch"
        );
        let _ = fs::remove_dir_all(&root);
    }

    fn walkdir_quarantine(root: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        if !root.exists() {
            return out;
        }
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            for entry in fs::read_dir(dir).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if path.is_dir() {
                    if path.join("quarantine.json").is_file() {
                        out.push(path);
                    } else {
                        walk(&path, out);
                    }
                }
            }
        }
        walk(root, &mut out);
        out
    }

    #[test]
    fn unverifiable_schema_frame_is_preserved_and_blocks_append() {
        let root = temp_root("schema-m");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("writer-schema-m").unwrap();
        let brain = "55555555-5555-4555-8555-555555555555";
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
                text: "before-bad-schema",
                config: &cfg,
            },
        )
        .unwrap();

        let mut value = serde_json::to_value(&first.accepted).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("schema_version".into(), serde_json::json!(99));
        let bad = serde_json::to_vec(&value).unwrap();
        write_crafted_body_frame(&root, &device, &bad);
        let path = active_segment_path(&root, &device, 0);
        let before = fs::read(&path).unwrap();

        let err = append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Internal,
                text: "after-quarantine",
                config: &cfg,
            },
        )
        .unwrap_err();
        assert!(matches!(
            err,
            CanonicalWriterError::Storage(StorageError::CompatibilityBoundary(_))
        ));
        assert_eq!(fs::read(&path).unwrap(), before);
        assert!(!quarantine_events_root(&root).exists());
        assert_eq!(first.accepted.device_seq.to_u64().unwrap(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn integrity_failure_frame_is_quarantined_with_reason_code() {
        let root = temp_root("int-m");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("writer-int-m").unwrap();
        let brain = "66666666-6666-4666-8666-666666666666";
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
                text: "before-tamper",
                config: &cfg,
            },
        )
        .unwrap();

        let mut hash = first.accepted.content_hash.as_str().to_string();
        let flip = if hash.as_bytes()[0] == b'0' { '1' } else { '0' };
        hash.replace_range(0..1, &flip.to_string());
        let mut value = serde_json::to_value(&first.accepted).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("content_hash".into(), serde_json::json!(hash));
        let bad = serde_json::to_vec(&value).unwrap();
        write_crafted_body_frame(&root, &device, &bad);

        let second = append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Internal,
                text: "after-integrity-quarantine",
                config: &cfg,
            },
        )
        .unwrap();
        assert_eq!(second.accepted.device_seq.to_u64().unwrap(), 2);
        assert_eq!(
            second.accepted.prev_content_hash.as_str(),
            first.accepted.content_hash.as_str()
        );

        let qroot = quarantine_events_root(&root);
        let mut found = false;
        for entry in walkdir_quarantine(&qroot) {
            let meta = fs::read_to_string(entry.join("quarantine.json")).unwrap();
            if meta.contains("integrity_failure") {
                found = true;
                assert!(!meta.contains(root.to_string_lossy().as_ref()));
            }
        }
        assert!(found, "expected integrity_failure quarantine entry");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn sealed_pack_corruption_fails_closed_and_quarantines() {
        let root = temp_root("seal-m");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("writer-sealed-m").unwrap();
        let brain = "77777777-7777-4777-8777-777777777777";
        let cfg = CanonicalWriterConfig {
            seal_event_threshold: 1,
        };
        let _ = append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Public,
                text: "seal-me",
                config: &cfg,
            },
        )
        .unwrap();
        let seg = sealed_segment_path(&root, &device, 0);
        assert!(seg.is_file());
        let mut bytes = fs::read(&seg).unwrap();
        if bytes.len() > 20 {
            bytes[20] ^= 0x5a;
        }
        fs::write(&seg, &bytes).unwrap();

        let err = append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Public,
                text: "should-fail-closed",
                config: &cfg,
            },
        )
        .unwrap_err();
        assert!(
            matches!(err, CanonicalWriterError::Integrity(_)),
            "got {err:?}"
        );

        let qroot = quarantine_events_root(&root);
        let mut found = false;
        for entry in walkdir_quarantine(&qroot) {
            let meta = fs::read_to_string(entry.join("quarantine.json")).unwrap();
            if meta.contains("corrupt_sealed_pack")
                || meta.contains("integrity_failure")
                || meta.contains("invalid_record")
            {
                found = true;
                assert!(!meta.contains(root.to_string_lossy().as_ref()));
            }
        }
        assert!(found, "expected sealed pack quarantine");
        assert!(
            !seg.exists(),
            "corrupt sealed .seg must be removed from canonical tree after durable quarantine"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn fail_closed_when_packs_exist_but_signing_keys_missing_does_not_mint() {
        let root = temp_root("packs-no-keys");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("writer-no-keys").unwrap();
        let brain = "77777777-7777-4777-8777-777777777777";
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
                text: "before key loss",
                config: &cfg,
            },
        )
        .expect("seed history");

        // Wipe SecureStore seed and public metadata.
        let service =
            crate::secure_store::ServiceLabel::device_credentials(SecureStoreEnvironment::Dev);
        let account = crate::secure_store::AccountLabel::device_event_signing_seed(device.as_str());
        store
            .delete(service.as_str(), account.as_str())
            .expect("delete seed");
        let meta = event_signing_key_path(&root, &device);
        fs::remove_file(&meta).expect("remove metadata");

        let err = append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Internal,
                text: "must not mint",
                config: &cfg,
            },
        )
        .unwrap_err();
        assert!(
            matches!(err, CanonicalWriterError::Signing(_)),
            "expected SigningKey fail-closed, got {err:?}"
        );
        // Must not have minted a replacement seed.
        assert!(
            matches!(
                store.get(service.as_str(), account.as_str()),
                Err(crate::secure_store::SecureStoreError::Missing)
            ),
            "must not mint seed over existing packs"
        );
        // Existing open pack must still be present (no destructive quarantine of valid history
        // under a freshly minted wrong key).
        let (sealed, open) = discover_segments(&root, &device).expect("discover");
        assert!(
            open.is_some() || !sealed.is_empty(),
            "history packs must remain"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn load_verified_events_fail_closes_and_quarantines_corrupt_sealed_pack() {
        let root = temp_root("load-sealed-corrupt");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("writer-load-corrupt").unwrap();
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
                text: "seal-me",
                config: &cfg,
            },
        )
        .expect("append+seal");

        let seg = sealed_segment_path(&root, &device, 0);
        assert!(seg.is_file(), "expected sealed pack");
        {
            let mut f = OpenOptions::new().append(true).open(&seg).unwrap();
            f.write_all(b"TRAILING-CORRUPT-BYTES").unwrap();
            f.sync_all().unwrap();
        }

        let err =
            load_verified_events(&store, SecureStoreEnvironment::Dev, &root, &device).unwrap_err();
        assert!(
            matches!(err, CanonicalWriterError::Integrity(_)),
            "expected Integrity fail-closed, got {err:?}"
        );
        let err_text = err.to_string();
        assert!(
            err_text.contains("canonical/events/"),
            "integrity error should use source_relative, got {err_text}"
        );
        assert!(
            !err_text.contains(root.to_string_lossy().as_ref()),
            "integrity error must not embed absolute runtime root: {err_text}"
        );
        let qroot = quarantine_events_root(&root);
        assert!(qroot.exists(), "corrupt sealed pack must be quarantined");
        let q_entries_after_first = walkdir_quarantine(&qroot)
            .into_iter()
            .filter(|p| p.join("quarantine.json").is_file())
            .count();
        assert_eq!(
            q_entries_after_first, 1,
            "exactly one quarantine entry after first failure"
        );
        assert!(
            !seg.exists(),
            "ADR-0016 §7.2: corrupt sealed .seg must be isolated from canonical tree after durable quarantine"
        );

        // Repeated load must not re-quarantine (canonical object already isolated).
        let second = load_verified_events(&store, SecureStoreEnvironment::Dev, &root, &device)
            .expect("after isolation, empty history loads");
        assert!(
            second.is_empty(),
            "no sealed packs remain after quarantine isolation"
        );
        let q_entries_after_second = walkdir_quarantine(&qroot)
            .into_iter()
            .filter(|p| p.join("quarantine.json").is_file())
            .count();
        assert_eq!(
            q_entries_after_second, 1,
            "repeated load must not create additional sealed-pack quarantine entries"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn sequence_gap_after_allocate_without_append_preserves_hash_chain() {
        let root = temp_root("seq-gap");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("writer-seq-gap").unwrap();
        let brain = "88888888-8888-4888-8888-888888888888";
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
                text: "first",
                config: &cfg,
            },
        )
        .unwrap();
        assert_eq!(first.accepted.device_seq.to_u64().unwrap(), 1);

        // Consume a sequence durably without appending (simulates crash after allocate).
        let burned = allocate_device_sequence(&root, &device).unwrap();
        assert_eq!(burned, 2);

        let second = append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Internal,
                text: "after-gap",
                config: &cfg,
            },
        )
        .unwrap();
        assert_eq!(second.accepted.device_seq.to_u64().unwrap(), 3);
        assert_eq!(
            second.accepted.prev_content_hash.as_str(),
            first.accepted.content_hash.as_str()
        );

        let events =
            load_verified_events(&store, SecureStoreEnvironment::Dev, &root, &device).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].device_seq.to_u64().unwrap(), 1);
        assert_eq!(events[1].device_seq.to_u64().unwrap(), 3);
        assert_eq!(events[1].prev_content_hash, events[0].content_hash);
        let _ = fs::remove_dir_all(&root);
    }
}
