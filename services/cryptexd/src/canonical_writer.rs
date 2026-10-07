//! cryptexd-only canonical event writer (ADR-0016 §11).
//!
//! Per-device exclusive lock covers reconcile → recover → allocate → append/seal → tip.
//! `writer_tip.json` is rebuildable derived state; verified `.seg`/`.open` records are
//! authoritative for chain tip and active segment discovery.

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
#[cfg(test)]
use cryptex_events::hex::ZERO_CONTENT_HASH_HEX;
use cryptex_events::hex::{ContentHashHex, SigningKeyId};
use cryptex_events::integrity::{accept_event, check_chain_link, check_first_event_prev_hash};
use cryptex_events::limits::DEFAULT_SEAL_EVENT_THRESHOLD_HINT;
use cryptex_events::validate_accepted_record_bytes;
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
}

impl FrameBodyValidator for VerifyingBodyValidator {
    fn validate_frame_body(&self, body: &[u8]) -> cryptex_storage::StorageResult<()> {
        validate_accepted_record_bytes(body, &self.verifying_key).map_err(|failure| {
            StorageError::Validation {
                reason_code: failure.reason.as_str().to_string(),
                detail: failure.detail,
                schema_version_if_parsed: failure.schema_version_if_parsed,
            }
        })?;
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
pub fn append_memory_note<S: SecureStore>(
    store: &S,
    env: SecureStoreEnvironment,
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
    assert_outside_repository(runtime_root)
        .map_err(|e| CanonicalWriterError::Path(e.to_string()))?;
    ensure_runtime_root(runtime_root).map_err(|e| CanonicalWriterError::Path(e.to_string()))?;
    parse_uuid_str(brain_id).map_err(|e| CanonicalWriterError::Invalid(e.to_string()))?;
    let payload = MemoryNotePayload::new(text)?;

    let device_dir = canonical_events_device_dir(runtime_root, device_id);
    fs::create_dir_all(&device_dir).map_err(|e| CanonicalWriterError::Path(e.to_string()))?;

    // Exclusive per-device writer lock for the entire transaction.
    let lock_path = device_dir.join("writer.lock");
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|e| CanonicalWriterError::Path(e.to_string()))?;
    fs2::FileExt::lock_exclusive(&lock)
        .map_err(|e| CanonicalWriterError::Path(format!("writer lock failed: {e}")))?;

    // Bootstrap signing key with empty observed set first if no packs yet; rebuild may
    // refine observed ids after key is available for verification.
    let bootstrap = ensure_event_signing_key(store, env, runtime_root, device_id, &[])?;
    let verifying = bootstrap.signing_key.verifying_key();
    let validator = VerifyingBodyValidator {
        verifying_key: verifying,
    };

    let mut state = rebuild_chain_state(runtime_root, device_id, &validator)?;
    // Re-ensure with observed key ids from verified packs (single-active-key rule).
    let ready = ensure_event_signing_key(
        store,
        env,
        runtime_root,
        device_id,
        &state.observed_signing_key_ids,
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
        let scan = scan_pack_events(&path, validator, &mut expected_prev, &mut expected_seq);
        match scan {
            Ok(events) => {
                // Sealed packs must be fully consumed (no truncated tail / failed frames).
                let file_len = fs::metadata(&path)
                    .map_err(|e| CanonicalWriterError::Path(e.to_string()))?
                    .len();
                let (_, verified_len) = fold_verified_frames(&path, validator, (), |_, _| Ok(()))?;
                if events.is_empty() || verified_len != file_len {
                    quarantine_sealed_pack(runtime_root, device_id, *index, &path)?;
                    return Err(CanonicalWriterError::Integrity(format!(
                        "sealed pack {} failed verification (verified_len={verified_len}, file_len={file_len}, events={}); quarantined",
                        path.display(),
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
                let events =
                    scan_pack_events(&path, validator, &mut expected_prev, &mut expected_seq)?;
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
    ev: &AcceptedEvent,
    last_hash: &mut ContentHashHex,
    last_key: &mut Option<SigningKeyId>,
    last_seq: &mut Option<u64>,
    observed: &mut Vec<SigningKeyId>,
) -> CanonicalWriterResult<()> {
    *last_hash = ev.content_hash.clone();
    *last_key = Some(ev.signing_key_id.clone());
    *last_seq = Some(ev.device_seq.to_u64()?);
    if !observed.iter().any(|id| id == &ev.signing_key_id) {
        observed.push(ev.signing_key_id.clone());
    }
    Ok(())
}

fn scan_pack_events(
    path: &Path,
    validator: &VerifyingBodyValidator,
    expected_prev: &mut ContentHashHex,
    expected_seq: &mut Option<u64>,
) -> CanonicalWriterResult<Vec<AcceptedEvent>> {
    let (events, _verified_len) =
        fold_verified_frames(path, validator, Vec::new(), |out, body| {
            let accepted: AcceptedEvent = serde_json::from_slice(body).map_err(|err| {
                StorageError::Integrity(format!("accepted JSON parse failed: {err}"))
            })?;
            // Chain rules inside fold via temporary check — validated below in Rust after fold
            // because expected_prev is outside StorageError mapping. Collect then verify.
            out.push(accepted);
            Ok(())
        })?;

    for accepted in &events {
        let seq = accepted.device_seq.to_u64()?;
        match expected_seq {
            None => {
                check_first_event_prev_hash(&accepted.prev_content_hash)
                    .map_err(|e| CanonicalWriterError::Integrity(e.to_string()))?;
                if accepted.prev_content_hash != *expected_prev {
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
                check_chain_link(expected_prev, &accepted.prev_content_hash)
                    .map_err(|e| CanonicalWriterError::Integrity(e.to_string()))?;
                *expected_seq = Some(seq);
            }
        }
        *expected_prev = accepted.content_hash.clone();
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
    if len == 0 {
        return Err(CanonicalWriterError::Integrity(format!(
            "refusing to quarantine empty sealed pack {}",
            path.display()
        )));
    }
    let source_relative = format!(
        "canonical/events/{}/{:020}.seg",
        device_id.as_filename_stem(),
        index
    );
    // Stream exact sealed bytes into quarantine before returning the validation failure.
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

/// Load all verified accepted events for a device (test / diagnostics).
pub fn load_verified_events<S: SecureStore>(
    store: &S,
    env: SecureStoreEnvironment,
    runtime_root: &Path,
    device_id: &DeviceId,
) -> CanonicalWriterResult<Vec<AcceptedEvent>> {
    let ready = ensure_event_signing_key(store, env, runtime_root, device_id, &[])?;
    let validator = VerifyingBodyValidator {
        verifying_key: ready.signing_key.verifying_key(),
    };
    let (sealed, open) = discover_segments(runtime_root, device_id)?;
    let mut expected_prev = ContentHashHex::zero();
    let mut expected_seq: Option<u64> = None;
    let mut all = Vec::new();
    for index in sealed {
        let path = sealed_segment_path(runtime_root, device_id, index);
        let mut events =
            scan_pack_events(&path, &validator, &mut expected_prev, &mut expected_seq)?;
        all.append(&mut events);
    }
    if let Some(idx) = open {
        recover_active_open(runtime_root, device_id, idx, &validator)?;
        let path = active_segment_path(runtime_root, device_id, idx);
        if path.exists() {
            let mut events =
                scan_pack_events(&path, &validator, &mut expected_prev, &mut expected_seq)?;
            all.append(&mut events);
        }
    }
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secure_store::FakeSecureStore;
    use cryptex_events::framing::encode_frame_header;
    use cryptex_storage::layout::quarantine_events_root;
    use cryptex_storage::{read_active_pack_bytes, sealed_segment_path};
    use std::io::Write;
    use std::sync::{Arc, Barrier};
    use std::thread;

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
    fn unsupported_schema_frame_is_quarantined_and_does_not_fork_chain() {
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

        let second = append_memory_note(
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
            if meta.contains("unsupported_schema") {
                found = true;
                assert!(meta.contains(r#""schema_version_if_parsed": 99"#));
                assert!(!meta.contains(root.to_string_lossy().as_ref()));
            }
        }
        assert!(found, "expected unsupported_schema quarantine entry");
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
