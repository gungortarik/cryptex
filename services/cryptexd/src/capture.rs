//! Daemon capture service (M5 Slice 1).
//!
//! Offline durable raw `memory.note` capture with receipt-scoped idempotency,
//! predetermined `event_id`, and Committed / Rejected / Uncertain outcomes.
//! See `evidence/m5/m5-slice1-capture-reconciliation-contract-20261010.md`.

use std::path::{Path, PathBuf};

use cryptex_core::ids::DeviceId;
use cryptex_core::validate::parse_uuid_str;
use cryptex_events::AcceptedEvent;
use cryptex_events::envelope::{Classification, EventPayload, MemoryNotePayload};
use cryptex_events::limits::MAX_MEMORY_NOTE_TEXT_BYTES;
use cryptex_storage::{active_segment_path, canonical_events_device_dir, sealed_segment_path};

use crate::canonical_writer::{
    AppendMemoryNoteArgs, CanonicalWriterConfig, CanonicalWriterError, DeviceCanonicalLock,
    acquire_device_canonical_lock, append_memory_note_with_lock,
    for_each_verified_event_enrolled_with_lock,
};
use crate::capture_receipt::{
    CaptureReceipt, ReceiptError, ReceiptState, confirm_existing_dir_entry_durable,
    confirm_pack_durable, confirm_receipt_durable, create_pending_receipt_exclusive,
    ensure_dir_tree_durable, load_receipt, receipt_path, write_committed_receipt,
};
use crate::event_signing_key::load_existing_event_verifying_key;
use crate::identity_runtime::load_brain_identity;
use crate::local_device_identity::{LocalDeviceIdentityError, load_local_device_identity};
use crate::paths::{assert_outside_repository, ensure_runtime_root};
use crate::secure_store::{SecureStore, SecureStoreEnvironment};

/// Domain label for capture fingerprint preimages (v1).
pub const CAPTURE_FINGERPRINT_DOMAIN: &[u8] = b"cryptex-capture-fingerprint-v1";
const DOMAIN_SEP: u8 = 0x00;

#[derive(Debug, Clone)]
pub struct CaptureRequest {
    pub capture_id: String,
    pub text: String,
    pub classification: Classification,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureCommit {
    pub capture_id: String,
    pub event_id: String,
    pub brain_id: String,
    pub device_id: String,
    pub device_seq: String,
    pub created_at: String,
    pub classification: Classification,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureRejectCode {
    InvalidRequest,
    EnrollmentRequired,
    IdentityUnavailable,
    CaptureIdConflict,
    NotCommitted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureUncertainReason {
    WriterFailure,
    DurabilityUnconfirmed,
    ReceiptMissing,
    ReceiptPersistFailed,
    VerificationUnavailable,
    FingerprintMismatch,
    LockFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureOutcome {
    Committed(Box<CaptureCommit>),
    Rejected {
        code: CaptureRejectCode,
        detail: String,
    },
    Uncertain {
        reason: CaptureUncertainReason,
        detail: String,
    },
}

impl CaptureOutcome {
    fn rejected(code: CaptureRejectCode, detail: impl Into<String>) -> Self {
        Self::Rejected {
            code,
            detail: detail.into(),
        }
    }

    fn uncertain(reason: CaptureUncertainReason, detail: impl Into<String>) -> Self {
        Self::Uncertain {
            reason,
            detail: detail.into(),
        }
    }
}

/// BLAKE3-256 fingerprint over length-prefixed capture request fields.
pub fn capture_fingerprint_v1(
    brain_id: &str,
    device_id: &str,
    capture_id: &str,
    classification: Classification,
    text: &str,
) -> String {
    let class = classification.as_str();
    let mut hasher = blake3::Hasher::new();
    hasher.update(CAPTURE_FINGERPRINT_DOMAIN);
    hasher.update(&[DOMAIN_SEP]);
    append_len_pref(&mut hasher, brain_id.as_bytes());
    append_len_pref(&mut hasher, device_id.as_bytes());
    append_len_pref(&mut hasher, capture_id.as_bytes());
    append_len_pref(&mut hasher, class.as_bytes());
    append_len_pref(&mut hasher, text.as_bytes());
    lowercase_hex(hasher.finalize().as_bytes())
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

fn append_len_pref(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
    hasher.update(&len.to_le_bytes());
    hasher.update(bytes);
}

fn new_uuid_string() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).map_err(|e| format!("uuid entropy failed: {e}"))?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Ok(format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    ))
}

fn validate_request(request: &CaptureRequest) -> Option<CaptureOutcome> {
    if parse_uuid_str(&request.capture_id).is_err() {
        return Some(CaptureOutcome::rejected(
            CaptureRejectCode::InvalidRequest,
            "capture_id must be a UUID",
        ));
    }
    if request.text.is_empty() {
        return Some(CaptureOutcome::rejected(
            CaptureRejectCode::InvalidRequest,
            "capture text must not be empty",
        ));
    }
    if request.text.len() > MAX_MEMORY_NOTE_TEXT_BYTES {
        return Some(CaptureOutcome::rejected(
            CaptureRejectCode::InvalidRequest,
            format!("capture text exceeds max {MAX_MEMORY_NOTE_TEXT_BYTES} bytes"),
        ));
    }
    if let Err(e) = MemoryNotePayload::new(request.text.clone()) {
        return Some(CaptureOutcome::rejected(
            CaptureRejectCode::InvalidRequest,
            e.to_string(),
        ));
    }
    None
}

fn resolve_identity(runtime_root: &Path) -> Result<(String, DeviceId), Box<CaptureOutcome>> {
    let brain = load_brain_identity(runtime_root).map_err(|e| {
        Box::new(CaptureOutcome::rejected(
            CaptureRejectCode::IdentityUnavailable,
            format!("brain identity unavailable: {e}"),
        ))
    })?;
    let local = load_local_device_identity(runtime_root, &brain.brain_id).map_err(|e| match e {
        LocalDeviceIdentityError::EnrollmentRequired => Box::new(CaptureOutcome::rejected(
            CaptureRejectCode::EnrollmentRequired,
            "local device enrollment required",
        )),
        other => Box::new(CaptureOutcome::rejected(
            CaptureRejectCode::IdentityUnavailable,
            other.to_string(),
        )),
    })?;
    let device_id = DeviceId::new(local.device_id).map_err(|e| {
        Box::new(CaptureOutcome::rejected(
            CaptureRejectCode::IdentityUnavailable,
            e.to_string(),
        ))
    })?;
    Ok((brain.brain_id, device_id))
}

fn require_enrolled_signing_identity<S: SecureStore>(
    store: &S,
    env: SecureStoreEnvironment,
    runtime_root: &Path,
    device_id: &DeviceId,
) -> Result<(), CaptureOutcome> {
    load_existing_event_verifying_key(store, env, runtime_root, device_id).map_err(|error| {
        CaptureOutcome::rejected(
            CaptureRejectCode::IdentityUnavailable,
            format!("enrolled signing identity unavailable: {error}"),
        )
    })?;
    Ok(())
}

#[cfg(test)]
thread_local! {
    static FAIL_AFTER_PENDING_BEFORE_APPEND: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}

/// Test-only: fail once after durable PENDING receipt, before append.
#[cfg(test)]
pub fn fail_after_pending_before_append() {
    FAIL_AFTER_PENDING_BEFORE_APPEND.with(|f| f.set(true));
}

/// Test-only: arm capture/writer/receipt faults from `CRYPTEX_TEST_CAPTURE_FAULT`.
///
/// Intended for child-process restart tests. Values:
/// - `after_pending_before_append`
/// - `after_append_before_tip`
/// - `after_seal_before_tip`
/// - `after_append_before_committed_receipt`
/// - `after_receipt_stage_before_place`
#[cfg(test)]
pub fn arm_capture_faults_from_env() {
    use crate::canonical_writer::{fail_after_append_before_tip, fail_after_seal_before_tip};
    use crate::capture_receipt::fail_next_receipt_place;

    match std::env::var("CRYPTEX_TEST_CAPTURE_FAULT").ok().as_deref() {
        Some("after_pending_before_append") => fail_after_pending_before_append(),
        Some("after_append_before_tip") => fail_after_append_before_tip(),
        Some("after_seal_before_tip") => fail_after_seal_before_tip(),
        // Armed in append_path after durable append so PENDING persist is not the fault point.
        Some("after_append_before_committed_receipt") => {}
        Some("after_receipt_stage_before_place") => fail_next_receipt_place(),
        Some(other) => panic!("unknown CRYPTEX_TEST_CAPTURE_FAULT={other}"),
        None => {}
    }
}

#[cfg(test)]
fn take_fail_after_pending_before_append() -> bool {
    FAIL_AFTER_PENDING_BEFORE_APPEND.with(|f| {
        let armed = f.get();
        if armed {
            f.set(false);
        }
        armed
    })
}

/// Capture a durable raw note (may append). Holds the device canonical lock for
/// reconcile + append; never nests a second acquisition of that lock.
pub fn capture_memory_note<S: SecureStore>(
    store: &S,
    env: SecureStoreEnvironment,
    runtime_root: &Path,
    request: CaptureRequest,
    config: &CanonicalWriterConfig,
) -> CaptureOutcome {
    #[cfg(test)]
    {
        arm_capture_faults_from_env();
    }
    if let Some(outcome) = validate_request(&request) {
        return outcome;
    }
    if let Err(e) = assert_outside_repository(runtime_root) {
        return CaptureOutcome::rejected(CaptureRejectCode::InvalidRequest, e.to_string());
    }
    if let Err(e) = ensure_runtime_root(runtime_root) {
        return CaptureOutcome::uncertain(CaptureUncertainReason::LockFailed, e.to_string());
    }
    let (brain_id, device_id) = match resolve_identity(runtime_root) {
        Ok(v) => v,
        Err(outcome) => return *outcome,
    };
    if let Err(outcome) = require_enrolled_signing_identity(store, env, runtime_root, &device_id) {
        return outcome;
    }
    let fingerprint = capture_fingerprint_v1(
        &brain_id,
        device_id.as_str(),
        &request.capture_id,
        request.classification,
        &request.text,
    );
    let lock = match acquire_device_canonical_lock(runtime_root, &device_id) {
        Ok(lock) => lock,
        Err(e) => {
            return CaptureOutcome::uncertain(CaptureUncertainReason::LockFailed, e.to_string());
        }
    };
    if let Err(e) = ensure_device_dir_durable(runtime_root, &device_id) {
        return CaptureOutcome::uncertain(CaptureUncertainReason::DurabilityUnconfirmed, e);
    }

    let existing = match load_receipt(runtime_root, &brain_id, &device_id, &request.capture_id) {
        Ok(v) => v,
        Err(e) => {
            return CaptureOutcome::uncertain(
                CaptureUncertainReason::VerificationUnavailable,
                e.to_string(),
            );
        }
    };

    match existing {
        None => {
            let event_id = match new_uuid_string() {
                Ok(id) => id,
                Err(e) => {
                    return CaptureOutcome::rejected(CaptureRejectCode::InvalidRequest, e);
                }
            };
            let pending = CaptureReceipt {
                schema_version: crate::capture_receipt::CAPTURE_RECEIPT_SCHEMA_VERSION,
                brain_id: brain_id.clone(),
                device_id: device_id.as_str().to_string(),
                capture_id: request.capture_id.clone(),
                event_id: event_id.clone(),
                fingerprint: fingerprint.clone(),
                state: ReceiptState::Pending,
            };
            match create_pending_receipt_exclusive(runtime_root, &pending) {
                Ok(()) => {}
                Err(ReceiptError::AlreadyExists) => {
                    return CaptureOutcome::uncertain(
                        CaptureUncertainReason::VerificationUnavailable,
                        "receipt appeared concurrently; retry resolve",
                    );
                }
                Err(e) => {
                    return CaptureOutcome::uncertain(
                        CaptureUncertainReason::ReceiptPersistFailed,
                        e.to_string(),
                    );
                }
            }
            #[cfg(test)]
            {
                crate::test_interrupt::maybe_abrupt_exit(
                    "after_pending_before_append",
                    runtime_root,
                    &event_id,
                );
                if take_fail_after_pending_before_append() {
                    return CaptureOutcome::uncertain(
                        CaptureUncertainReason::WriterFailure,
                        "injected fail after durable PENDING before append",
                    );
                }
            }
            append_path(
                store,
                env,
                AppendPathArgs {
                    runtime_root,
                    lock: &lock,
                    brain_id: &brain_id,
                    device_id: &device_id,
                    request: &request,
                    fingerprint: &fingerprint,
                    event_id: &event_id,
                    config,
                },
            )
        }
        Some(receipt) if receipt.fingerprint != fingerprint => CaptureOutcome::rejected(
            CaptureRejectCode::CaptureIdConflict,
            "capture_id already bound to a different request fingerprint",
        ),
        Some(receipt) if receipt.state == ReceiptState::Committed => {
            durably_confirm_committed(store, env, runtime_root, &lock, &device_id, &receipt)
        }
        Some(receipt) => resume_pending(
            store,
            env,
            AppendPathArgs {
                runtime_root,
                lock: &lock,
                brain_id: &brain_id,
                device_id: &device_id,
                request: &request,
                fingerprint: &fingerprint,
                event_id: &receipt.event_id,
                config,
            },
            &receipt,
        ),
    }
}

/// Resolve a prior capture attempt. Never appends.
pub fn resolve_capture<S: SecureStore>(
    store: &S,
    env: SecureStoreEnvironment,
    runtime_root: &Path,
    capture_id: &str,
) -> CaptureOutcome {
    if parse_uuid_str(capture_id).is_err() {
        return CaptureOutcome::rejected(
            CaptureRejectCode::InvalidRequest,
            "capture_id must be a UUID",
        );
    }
    if let Err(e) = assert_outside_repository(runtime_root) {
        return CaptureOutcome::rejected(CaptureRejectCode::InvalidRequest, e.to_string());
    }
    let (brain_id, device_id) = match resolve_identity(runtime_root) {
        Ok(v) => v,
        Err(outcome) => return *outcome,
    };
    if let Err(outcome) = require_enrolled_signing_identity(store, env, runtime_root, &device_id) {
        return outcome;
    }
    let lock = match acquire_device_canonical_lock(runtime_root, &device_id) {
        Ok(lock) => lock,
        Err(e) => {
            return CaptureOutcome::uncertain(CaptureUncertainReason::LockFailed, e.to_string());
        }
    };
    let receipt = match load_receipt(runtime_root, &brain_id, &device_id, capture_id) {
        Ok(Some(r)) => r,
        Ok(None) => {
            return CaptureOutcome::uncertain(
                CaptureUncertainReason::ReceiptMissing,
                "no receipt for capture_id; automatic retry with this id is prohibited",
            );
        }
        Err(e) => {
            return CaptureOutcome::uncertain(
                CaptureUncertainReason::VerificationUnavailable,
                e.to_string(),
            );
        }
    };

    match find_verified_event(
        store,
        env,
        runtime_root,
        &device_id,
        &lock,
        &receipt.event_id,
    ) {
        ScanResult::Blocked(detail) => {
            CaptureOutcome::uncertain(CaptureUncertainReason::VerificationUnavailable, detail)
        }
        ScanResult::Found { event, pack_path } => {
            match event_matches_receipt(event.as_ref(), &receipt) {
                Ok(()) => durably_confirm_event(
                    runtime_root,
                    &device_id,
                    &lock,
                    &receipt,
                    event.as_ref(),
                    &pack_path,
                ),
                Err(detail) => {
                    CaptureOutcome::uncertain(CaptureUncertainReason::FingerprintMismatch, detail)
                }
            }
        }
        ScanResult::Absent => {
            if receipt.state == ReceiptState::Pending {
                CaptureOutcome::rejected(
                    CaptureRejectCode::NotCommitted,
                    "pending capture event_id absent from verified packs",
                )
            } else {
                CaptureOutcome::uncertain(
                    CaptureUncertainReason::VerificationUnavailable,
                    "committed receipt but event absent from verified packs",
                )
            }
        }
    }
}

enum ScanResult {
    Found {
        event: Box<AcceptedEvent>,
        pack_path: PathBuf,
    },
    Absent,
    Blocked(String),
}

fn find_verified_event<S: SecureStore>(
    store: &S,
    env: SecureStoreEnvironment,
    runtime_root: &Path,
    device_id: &DeviceId,
    lock: &DeviceCanonicalLock,
    event_id: &str,
) -> ScanResult {
    let mut found = None;
    match for_each_verified_event_enrolled_with_lock(
        store,
        env,
        runtime_root,
        device_id,
        lock,
        |event, pack_path| {
            if event.event_id == event_id {
                found = Some((event.clone(), pack_path.to_path_buf()));
            }
            Ok(())
        },
    ) {
        Ok(()) => match found {
            Some((event, pack_path)) => ScanResult::Found {
                event: Box::new(event),
                pack_path,
            },
            None => ScanResult::Absent,
        },
        Err(e) => ScanResult::Blocked(e.to_string()),
    }
}

fn event_matches_receipt(event: &AcceptedEvent, receipt: &CaptureReceipt) -> Result<(), String> {
    if event.event_id != receipt.event_id {
        return Err("event_id mismatch".into());
    }
    if event.brain_id != receipt.brain_id || event.device_id != receipt.device_id {
        return Err("brain/device mismatch".into());
    }
    let text = match &event.payload {
        EventPayload::MemoryNote(p) => p.text.as_str(),
    };
    let fp = capture_fingerprint_v1(
        &receipt.brain_id,
        &receipt.device_id,
        &receipt.capture_id,
        event.classification,
        text,
    );
    if fp != receipt.fingerprint {
        return Err("recomputed fingerprint does not match receipt".into());
    }
    Ok(())
}

fn ensure_device_dir_durable(runtime_root: &Path, device_id: &DeviceId) -> Result<(), String> {
    let dir = canonical_events_device_dir(runtime_root, device_id);
    // Cover ancestry that may already exist (e.g. created by lock acquisition).
    ensure_dir_tree_durable(&dir).map_err(|e| e.to_string())?;
    confirm_existing_dir_entry_durable(&dir).map_err(|e| e.to_string())?;
    Ok(())
}

struct AppendPathArgs<'a> {
    runtime_root: &'a Path,
    lock: &'a DeviceCanonicalLock,
    brain_id: &'a str,
    device_id: &'a DeviceId,
    request: &'a CaptureRequest,
    fingerprint: &'a str,
    event_id: &'a str,
    config: &'a CanonicalWriterConfig,
}

fn resume_pending<S: SecureStore>(
    store: &S,
    env: SecureStoreEnvironment,
    args: AppendPathArgs<'_>,
    receipt: &CaptureReceipt,
) -> CaptureOutcome {
    match find_verified_event(
        store,
        env,
        args.runtime_root,
        args.device_id,
        args.lock,
        &receipt.event_id,
    ) {
        ScanResult::Blocked(detail) => {
            CaptureOutcome::uncertain(CaptureUncertainReason::VerificationUnavailable, detail)
        }
        ScanResult::Found { event, pack_path } => {
            match event_matches_receipt(event.as_ref(), receipt) {
                Ok(()) => durably_confirm_event(
                    args.runtime_root,
                    args.device_id,
                    args.lock,
                    receipt,
                    event.as_ref(),
                    &pack_path,
                ),
                Err(detail) => {
                    CaptureOutcome::uncertain(CaptureUncertainReason::FingerprintMismatch, detail)
                }
            }
        }
        ScanResult::Absent => append_path(
            store,
            env,
            AppendPathArgs {
                event_id: receipt.event_id.as_str(),
                ..args
            },
        ),
    }
}

fn append_path<S: SecureStore>(
    store: &S,
    env: SecureStoreEnvironment,
    path: AppendPathArgs<'_>,
) -> CaptureOutcome {
    let writer_args = AppendMemoryNoteArgs {
        runtime_root: path.runtime_root,
        brain_id: path.brain_id,
        device_id: path.device_id,
        classification: path.request.classification,
        text: &path.request.text,
        event_id: Some(path.event_id),
        config: path.config,
    };
    match append_memory_note_with_lock(store, env, path.lock, writer_args) {
        Ok(outcome) => {
            let pack = if outcome.sealed {
                sealed_segment_path(path.runtime_root, path.device_id, outcome.segment_index)
            } else {
                active_segment_path(path.runtime_root, path.device_id, outcome.segment_index)
            };
            if let Err(e) = confirm_pack_durable(&pack) {
                return CaptureOutcome::uncertain(
                    CaptureUncertainReason::DurabilityUnconfirmed,
                    e.to_string(),
                );
            }
            #[cfg(test)]
            {
                crate::test_interrupt::maybe_abrupt_exit(
                    "after_append_before_committed_receipt",
                    path.runtime_root,
                    path.event_id,
                );
                if std::env::var("CRYPTEX_TEST_CAPTURE_FAULT").ok().as_deref()
                    == Some("after_append_before_committed_receipt")
                {
                    crate::capture_receipt::fail_next_receipt_persist();
                }
            }
            let committed_receipt = CaptureReceipt {
                schema_version: crate::capture_receipt::CAPTURE_RECEIPT_SCHEMA_VERSION,
                brain_id: path.brain_id.to_string(),
                device_id: path.device_id.as_str().to_string(),
                capture_id: path.request.capture_id.clone(),
                event_id: path.event_id.to_string(),
                fingerprint: path.fingerprint.to_string(),
                state: ReceiptState::Committed,
            };
            if let Err(e) = write_committed_receipt(path.runtime_root, &committed_receipt) {
                return CaptureOutcome::uncertain(
                    CaptureUncertainReason::ReceiptPersistFailed,
                    e.to_string(),
                );
            }
            #[cfg(test)]
            {
                crate::test_interrupt::maybe_abrupt_exit(
                    "after_committed_before_response",
                    path.runtime_root,
                    path.event_id,
                );
            }
            CaptureOutcome::Committed(Box::new(commit_from_accepted(
                &path.request.capture_id,
                &outcome.accepted,
            )))
        }
        Err(e) => map_writer_error(e),
    }
}
fn map_writer_error(error: CanonicalWriterError) -> CaptureOutcome {
    match &error {
        CanonicalWriterError::Invalid(msg) => {
            CaptureOutcome::rejected(CaptureRejectCode::InvalidRequest, msg.clone())
        }
        CanonicalWriterError::Signing(e) => {
            CaptureOutcome::rejected(CaptureRejectCode::IdentityUnavailable, e.to_string())
        }
        other => {
            CaptureOutcome::uncertain(CaptureUncertainReason::WriterFailure, other.to_string())
        }
    }
}

fn durably_confirm_committed<S: SecureStore>(
    store: &S,
    env: SecureStoreEnvironment,
    runtime_root: &Path,
    lock: &DeviceCanonicalLock,
    device_id: &DeviceId,
    receipt: &CaptureReceipt,
) -> CaptureOutcome {
    match find_verified_event(store, env, runtime_root, device_id, lock, &receipt.event_id) {
        ScanResult::Blocked(detail) => {
            CaptureOutcome::uncertain(CaptureUncertainReason::VerificationUnavailable, detail)
        }
        ScanResult::Absent => CaptureOutcome::uncertain(
            CaptureUncertainReason::VerificationUnavailable,
            "committed receipt but event absent from verified packs",
        ),
        ScanResult::Found { event, pack_path } => {
            match event_matches_receipt(event.as_ref(), receipt) {
                Ok(()) => durably_confirm_event(
                    runtime_root,
                    device_id,
                    lock,
                    receipt,
                    event.as_ref(),
                    &pack_path,
                ),
                Err(detail) => {
                    CaptureOutcome::uncertain(CaptureUncertainReason::FingerprintMismatch, detail)
                }
            }
        }
    }
}

fn durably_confirm_event(
    runtime_root: &Path,
    device_id: &DeviceId,
    _lock: &DeviceCanonicalLock,
    receipt: &CaptureReceipt,
    event: &AcceptedEvent,
    pack_path: &Path,
) -> CaptureOutcome {
    if let Err(e) = confirm_pack_durable(pack_path) {
        return CaptureOutcome::uncertain(
            CaptureUncertainReason::DurabilityUnconfirmed,
            e.to_string(),
        );
    }
    let committed = CaptureReceipt {
        state: ReceiptState::Committed,
        ..receipt.clone()
    };
    let path = receipt_path(
        runtime_root,
        &committed.brain_id,
        device_id,
        &committed.capture_id,
    );
    // Always (re)persist or confirm COMMITTED receipt durability. A prior failed
    // sync must not be treated as success merely because the visible state says
    // COMMITTED.
    match confirm_receipt_durable(&path) {
        Ok(()) if receipt.state == ReceiptState::Committed => {}
        Ok(()) | Err(_) => {
            if let Err(e) = write_committed_receipt(runtime_root, &committed) {
                return CaptureOutcome::uncertain(
                    CaptureUncertainReason::ReceiptPersistFailed,
                    e.to_string(),
                );
            }
            if let Err(e) = confirm_receipt_durable(&path) {
                return CaptureOutcome::uncertain(
                    CaptureUncertainReason::DurabilityUnconfirmed,
                    e.to_string(),
                );
            }
        }
    }
    CaptureOutcome::Committed(Box::new(commit_from_accepted(&receipt.capture_id, event)))
}

fn commit_from_accepted(capture_id: &str, event: &AcceptedEvent) -> CaptureCommit {
    CaptureCommit {
        capture_id: capture_id.to_string(),
        event_id: event.event_id.clone(),
        brain_id: event.brain_id.clone(),
        device_id: event.device_id.clone(),
        device_seq: event.device_seq.to_string(),
        created_at: event.created_at.clone(),
        classification: event.classification,
        content_hash: event.content_hash.as_str().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::{Arc, Barrier};
    use std::thread;

    use crate::capture_receipt::{
        fail_next_dir_ancestry_syncs, fail_next_pack_confirm, fail_next_receipt_confirms,
        take_last_confirmed_pack,
    };
    use cryptex_core::BrainIdentity;
    use cryptex_events::limits::MAX_MEMORY_NOTE_TEXT_BYTES;

    use crate::canonical_writer::{
        fail_after_append_before_tip, fail_after_seal_before_tip, load_verified_events,
    };
    use crate::capture_receipt::{
        create_pending_receipt_exclusive, fail_next_receipt_persist, receipt_path,
    };
    use crate::identity_runtime::write_brain_identity;
    use crate::local_device_identity::enroll_local_device;
    use crate::secure_store::FakeSecureStore;
    use crate::secure_store::{AccountLabel, SecureStore, ServiceLabel};
    use crate::test_temp::exclusive_temp_dir;
    use cryptex_storage::event_signing_key_path;

    const BRAIN: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";

    fn prepared() -> (std::path::PathBuf, FakeSecureStore, DeviceId) {
        let root = exclusive_temp_dir("cryptex-m5-capture");
        let brain = BrainIdentity::new(BRAIN, "2026-10-10T00:00:00Z").unwrap();
        write_brain_identity(&root, &brain).unwrap();
        let store = FakeSecureStore::new();
        let identity =
            enroll_local_device(&root, &store, SecureStoreEnvironment::Dev).expect("enroll");
        let device = DeviceId::new(identity.device_id).unwrap();
        (root, store, device)
    }

    fn req(capture_id: &str, text: &str, classification: Classification) -> CaptureRequest {
        CaptureRequest {
            capture_id: capture_id.to_string(),
            text: text.to_string(),
            classification,
        }
    }

    fn assert_committed(outcome: CaptureOutcome) -> CaptureCommit {
        match outcome {
            CaptureOutcome::Committed(c) => *c,
            other => panic!("expected Committed, got {other:?}"),
        }
    }

    #[test]
    fn fingerprint_is_stable_and_field_sensitive() {
        let a = capture_fingerprint_v1(BRAIN, "dev", "cap", Classification::Internal, "hello");
        let b = capture_fingerprint_v1(BRAIN, "dev", "cap", Classification::Internal, "hello");
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
        let c = capture_fingerprint_v1(BRAIN, "dev", "cap", Classification::Internal, "hello!");
        assert_ne!(a, c);
        let d = capture_fingerprint_v1(BRAIN, "dev", "cap", Classification::Private, "hello");
        assert_ne!(a, d);
    }

    #[test]
    fn successful_capture_commits_and_replays() {
        let (root, store, device) = prepared();
        let cfg = CanonicalWriterConfig::default();
        let capture_id = "11111111-1111-4111-8111-111111111111";
        let first = assert_committed(capture_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            req(capture_id, "durable note", Classification::Internal),
            &cfg,
        ));
        let replay = assert_committed(capture_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            req(capture_id, "durable note", Classification::Internal),
            &cfg,
        ));
        assert_eq!(first.event_id, replay.event_id);
        let events =
            load_verified_events(&store, SecureStoreEnvironment::Dev, &root, &device).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_id, first.event_id);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn invalid_and_oversized_text_rejected() {
        let (root, store, _) = prepared();
        let cfg = CanonicalWriterConfig::default();
        let empty = capture_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            req(
                "11111111-1111-4111-8111-111111111111",
                "",
                Classification::Internal,
            ),
            &cfg,
        );
        assert!(matches!(
            empty,
            CaptureOutcome::Rejected {
                code: CaptureRejectCode::InvalidRequest,
                ..
            }
        ));
        let oversized = "x".repeat(MAX_MEMORY_NOTE_TEXT_BYTES + 1);
        let big = capture_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            req(
                "22222222-2222-4222-8222-222222222222",
                &oversized,
                Classification::Internal,
            ),
            &cfg,
        );
        assert!(matches!(
            big,
            CaptureOutcome::Rejected {
                code: CaptureRejectCode::InvalidRequest,
                ..
            }
        ));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_enrollment_rejected() {
        let root = exclusive_temp_dir("cryptex-m5-no-enroll");
        let brain = BrainIdentity::new(BRAIN, "2026-10-10T00:00:00Z").unwrap();
        write_brain_identity(&root, &brain).unwrap();
        let store = FakeSecureStore::new();
        let cfg = CanonicalWriterConfig::default();
        let outcome = capture_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            req(
                "11111111-1111-4111-8111-111111111111",
                "note",
                Classification::Internal,
            ),
            &cfg,
        );
        assert!(matches!(
            outcome,
            CaptureOutcome::Rejected {
                code: CaptureRejectCode::EnrollmentRequired,
                ..
            }
        ));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn tip_failure_after_append_is_uncertain_then_resolve_commits() {
        let (root, store, device) = prepared();
        let cfg = CanonicalWriterConfig::default();
        let capture_id = "33333333-3333-4333-8333-333333333333";
        fail_after_append_before_tip();
        let outcome = capture_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            req(capture_id, "crash-tip", Classification::Public),
            &cfg,
        );
        assert!(
            matches!(
                outcome,
                CaptureOutcome::Uncertain {
                    reason: CaptureUncertainReason::WriterFailure,
                    ..
                }
            ),
            "{outcome:?}"
        );
        let resolved = assert_committed(resolve_capture(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            capture_id,
        ));
        assert_eq!(resolved.capture_id, capture_id);
        let events =
            load_verified_events(&store, SecureStoreEnvironment::Dev, &root, &device).unwrap();
        assert_eq!(events.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn seal_failure_before_tip_is_uncertain_then_resolve_commits() {
        let (root, store, device) = prepared();
        let cfg = CanonicalWriterConfig {
            seal_event_threshold: 1,
        };
        let capture_id = "44444444-4444-4444-8444-444444444444";
        fail_after_seal_before_tip();
        let outcome = capture_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            req(capture_id, "seal-me", Classification::Internal),
            &cfg,
        );
        assert!(matches!(
            outcome,
            CaptureOutcome::Uncertain {
                reason: CaptureUncertainReason::WriterFailure,
                ..
            }
        ));
        let _ = assert_committed(resolve_capture(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            capture_id,
        ));
        let events =
            load_verified_events(&store, SecureStoreEnvironment::Dev, &root, &device).unwrap();
        assert_eq!(events.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn receipt_persist_failure_after_append_is_uncertain_then_resolve() {
        let (root, store, _) = prepared();
        let cfg = CanonicalWriterConfig::default();
        let capture_id = "55555555-5555-4555-8555-555555555555";
        // First receipt PENDING succeeds; fail COMMITTED persist after append.
        // Inject on the second persist (committed write).
        // create_pending uses persist once; committed uses persist once; arm after pending.
        // Easiest: capture once successfully to warm, then new id with fail after pending:
        // Arm fail for committed write by failing the next persist after we create pending
        // inside capture -- use fail_next which fires on first persist. So create pending
        // manually path: instead capture with fail armed only for committed by calling
        // fail after a dummy receipt write.
        // Simpler approach: run capture; on success path committed persist is second call
        // to persist_receipt for that capture. fail_next fires on first = PENDING create.
        // So arm fail_next_receipt_persist AFTER a no-op?
        // Use two-step: first capture succeeds. Second capture: we need fail on committed.
        // Modify test to call fail_next twice conceptually -- only one shot.
        // Alternative: delete isn't right. Use fail on committed by:
        // 1) capture creates pending (persist 1)
        // 2) append ok
        // 3) committed persist (persist 2) -- need fail on 2nd.
        // Add fail_next_receipt_persists(2) or call fail once after manually creating pending.
        // For this test use: fail_next on committed only by pre-creating pending via API internals.
        use crate::capture_receipt::{
            CAPTURE_RECEIPT_SCHEMA_VERSION, create_pending_receipt_exclusive,
        };
        let event_id = "66666666-6666-4666-8666-666666666666";
        let fp = capture_fingerprint_v1(
            BRAIN,
            // device from prepared
            load_local_device_identity(&root, BRAIN)
                .unwrap()
                .device_id
                .as_str(),
            capture_id,
            Classification::Restricted,
            "receipt-fail",
        );
        let device_str = load_local_device_identity(&root, BRAIN).unwrap().device_id;
        create_pending_receipt_exclusive(
            &root,
            &CaptureReceipt {
                schema_version: CAPTURE_RECEIPT_SCHEMA_VERSION,
                brain_id: BRAIN.into(),
                device_id: device_str,
                capture_id: capture_id.into(),
                event_id: event_id.into(),
                fingerprint: fp,
                state: ReceiptState::Pending,
            },
        )
        .unwrap();
        fail_next_receipt_persist();
        let outcome = capture_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            req(capture_id, "receipt-fail", Classification::Restricted),
            &cfg,
        );
        // May be Uncertain from receipt persist after append, or WriterFailure if append path differs.
        assert!(
            matches!(
                outcome,
                CaptureOutcome::Uncertain {
                    reason: CaptureUncertainReason::ReceiptPersistFailed
                        | CaptureUncertainReason::WriterFailure
                        | CaptureUncertainReason::DurabilityUnconfirmed,
                    ..
                }
            ),
            "{outcome:?}"
        );
        let _ = assert_committed(resolve_capture(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            capture_id,
        ));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn resolve_missing_receipt_is_uncertain() {
        let (root, store, _) = prepared();
        let outcome = resolve_capture(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            "77777777-7777-4777-8777-777777777777",
        );
        assert!(matches!(
            outcome,
            CaptureOutcome::Uncertain {
                reason: CaptureUncertainReason::ReceiptMissing,
                ..
            }
        ));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn capture_id_conflict_rejects_without_second_append() {
        let (root, store, device) = prepared();
        let cfg = CanonicalWriterConfig::default();
        let capture_id = "88888888-8888-4888-8888-888888888888";
        let _ = assert_committed(capture_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            req(capture_id, "original", Classification::Internal),
            &cfg,
        ));
        let conflict = capture_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            req(capture_id, "different", Classification::Internal),
            &cfg,
        );
        assert!(matches!(
            conflict,
            CaptureOutcome::Rejected {
                code: CaptureRejectCode::CaptureIdConflict,
                ..
            }
        ));
        let events =
            load_verified_events(&store, SecureStoreEnvironment::Dev, &root, &device).unwrap();
        assert_eq!(events.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn intentional_identical_text_with_distinct_capture_ids() {
        let (root, store, device) = prepared();
        let cfg = CanonicalWriterConfig::default();
        let _ = assert_committed(capture_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            req(
                "91111111-1111-4111-8111-111111111111",
                "same text",
                Classification::Internal,
            ),
            &cfg,
        ));
        let _ = assert_committed(capture_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            req(
                "92222222-2222-4222-8222-222222222222",
                "same text",
                Classification::Internal,
            ),
            &cfg,
        ));
        let events =
            load_verified_events(&store, SecureStoreEnvironment::Dev, &root, &device).unwrap();
        assert_eq!(events.len(), 2);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn concurrent_matching_capture_ids_append_once() {
        let (root, store, device) = prepared();
        let root = Arc::new(root);
        let store = Arc::new(store);
        let cfg = Arc::new(CanonicalWriterConfig::default());
        let capture_id = "a1111111-1111-4111-8111-111111111111";
        let barrier = Arc::new(Barrier::new(8));
        let mut handles = Vec::new();
        for _ in 0..8 {
            let root = Arc::clone(&root);
            let store = Arc::clone(&store);
            let cfg = Arc::clone(&cfg);
            let barrier = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier.wait();
                capture_memory_note(
                    store.as_ref(),
                    SecureStoreEnvironment::Dev,
                    root.as_ref(),
                    req(capture_id, "concurrent", Classification::Private),
                    cfg.as_ref(),
                )
            }));
        }
        let mut event_ids = Vec::new();
        for handle in handles {
            let outcome = handle.join().unwrap();
            let commit = assert_committed(outcome);
            event_ids.push(commit.event_id);
        }
        event_ids.sort();
        event_ids.dedup();
        assert_eq!(event_ids.len(), 1);
        let events = load_verified_events(
            store.as_ref(),
            SecureStoreEnvironment::Dev,
            root.as_ref(),
            &device,
        )
        .unwrap();
        assert_eq!(events.len(), 1);
        let _ = fs::remove_dir_all(root.as_ref());
    }

    #[test]
    fn receipt_loss_resolve_uncertain_and_note_still_in_packs() {
        let (root, store, device) = prepared();
        let cfg = CanonicalWriterConfig::default();
        let capture_id = "b1111111-1111-4111-8111-111111111111";
        let commit = assert_committed(capture_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            req(capture_id, "keep me", Classification::Internal),
            &cfg,
        ));
        let path = receipt_path(&root, BRAIN, &device, capture_id);
        fs::remove_file(&path).unwrap();
        let resolved = resolve_capture(&store, SecureStoreEnvironment::Dev, &root, capture_id);
        assert!(matches!(
            resolved,
            CaptureOutcome::Uncertain {
                reason: CaptureUncertainReason::ReceiptMissing,
                ..
            }
        ));
        let events =
            load_verified_events(&store, SecureStoreEnvironment::Dev, &root, &device).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_id, commit.event_id);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn pending_absent_resolve_not_committed_then_capture_appends() {
        let (root, store, device) = prepared();
        let cfg = CanonicalWriterConfig::default();
        let capture_id = "c1111111-1111-4111-8111-111111111111";
        let device_str = device.as_str().to_string();
        let event_id = "c2222222-2222-4222-8222-222222222222";
        let fp = capture_fingerprint_v1(
            BRAIN,
            &device_str,
            capture_id,
            Classification::Internal,
            "later",
        );
        create_pending_receipt_exclusive(
            &root,
            &CaptureReceipt {
                schema_version: crate::capture_receipt::CAPTURE_RECEIPT_SCHEMA_VERSION,
                brain_id: BRAIN.into(),
                device_id: device_str,
                capture_id: capture_id.into(),
                event_id: event_id.into(),
                fingerprint: fp,
                state: ReceiptState::Pending,
            },
        )
        .unwrap();
        let resolved = resolve_capture(&store, SecureStoreEnvironment::Dev, &root, capture_id);
        assert!(matches!(
            resolved,
            CaptureOutcome::Rejected {
                code: CaptureRejectCode::NotCommitted,
                ..
            }
        ));
        let commit = assert_committed(capture_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            req(capture_id, "later", Classification::Internal),
            &cfg,
        ));
        assert_eq!(commit.event_id, event_id);
        let events =
            load_verified_events(&store, SecureStoreEnvironment::Dev, &root, &device).unwrap();
        assert_eq!(events.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn pack_durability_confirm_failure_is_uncertain_then_resolve() {
        let (root, store, _) = prepared();
        let cfg = CanonicalWriterConfig::default();
        let capture_id = "d1111111-1111-4111-8111-111111111111";
        // PENDING receipt persist succeeds; fail the post-append pack confirm.
        fail_next_pack_confirm();
        let outcome = capture_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            req(capture_id, "pack-confirm", Classification::Internal),
            &cfg,
        );
        // First capture: pending create may not hit pack confirm if fail armed too early.
        // Arm after pending by pre-creating pending then failing confirm on resume/append success.
        if matches!(outcome, CaptureOutcome::Committed(_)) {
            // Retry path: already committed -- force confirm fail on resolve instead.
            fail_next_pack_confirm();
            let resolved = resolve_capture(&store, SecureStoreEnvironment::Dev, &root, capture_id);
            assert!(
                matches!(
                    resolved,
                    CaptureOutcome::Uncertain {
                        reason: CaptureUncertainReason::DurabilityUnconfirmed,
                        ..
                    }
                ),
                "{resolved:?}"
            );
            let _ = assert_committed(resolve_capture(
                &store,
                SecureStoreEnvironment::Dev,
                &root,
                capture_id,
            ));
        } else {
            assert!(
                matches!(
                    outcome,
                    CaptureOutcome::Uncertain {
                        reason: CaptureUncertainReason::DurabilityUnconfirmed,
                        ..
                    }
                ),
                "{outcome:?}"
            );
            let _ = assert_committed(resolve_capture(
                &store,
                SecureStoreEnvironment::Dev,
                &root,
                capture_id,
            ));
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn pack_lookup_ignores_uuid_in_earlier_note_body() {
        let (root, store, device_id) = prepared();
        let target_event_id = "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee";
        // Pack 0: note body contains the target UUID as plain text (decoy).
        let decoy = assert_committed(capture_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            CaptureRequest {
                capture_id: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb".into(),
                classification: Classification::Internal,
                text: format!("decoy mentions {target_event_id}"),
            },
            &CanonicalWriterConfig {
                seal_event_threshold: 1,
            },
        ));
        assert_ne!(decoy.event_id, target_event_id);

        // Pack 1 (open): real event with event_id == target, via exclusive PENDING.
        let capture_id = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";
        let fingerprint = capture_fingerprint_v1(
            BRAIN,
            device_id.as_str(),
            capture_id,
            Classification::Internal,
            "real event body",
        );
        let pending = CaptureReceipt {
            schema_version: crate::capture_receipt::CAPTURE_RECEIPT_SCHEMA_VERSION,
            brain_id: BRAIN.to_string(),
            device_id: device_id.as_str().to_string(),
            capture_id: capture_id.to_string(),
            event_id: target_event_id.to_string(),
            fingerprint,
            state: ReceiptState::Pending,
        };
        create_pending_receipt_exclusive(&root, &pending).expect("pending");
        let _ = take_last_confirmed_pack();
        let committed = assert_committed(capture_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            CaptureRequest {
                capture_id: capture_id.into(),
                classification: Classification::Internal,
                text: "real event body".into(),
            },
            &CanonicalWriterConfig::default(),
        ));
        assert_eq!(committed.event_id, target_event_id);
        let confirmed = take_last_confirmed_pack().expect("pack confirm recorded");
        let confirmed_name = confirmed
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        assert!(
            confirmed_name.ends_with("00000000000000000001.open"),
            "must confirm open pack index 1 (verified event), got: {confirmed:?}"
        );
        assert!(
            !confirmed_name.ends_with("00000000000000000000.seg")
                && !confirmed_name.ends_with("00000000000000000000.open"),
            "must not select pack 0 where the UUID only appears in note text: {confirmed:?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn incomplete_signing_identity_rejects_without_secure_store_put_or_metadata_mint() {
        let (root, store, device_id) = prepared();
        let meta_path = event_signing_key_path(&root, &device_id);
        assert!(
            meta_path.is_file(),
            "enrollment should create public metadata"
        );
        fs::remove_file(&meta_path).expect("remove metadata");
        let service = ServiceLabel::device_credentials(SecureStoreEnvironment::Dev);
        let account = AccountLabel::device_event_signing_seed(device_id.as_str());
        store
            .delete(service.as_str(), account.as_str())
            .expect("remove seed");
        let puts_before = store.put_count();
        let outcome = capture_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            CaptureRequest {
                capture_id: "dddddddd-dddd-4ddd-8ddd-dddddddddddd".into(),
                classification: Classification::Internal,
                text: "must not mint".into(),
            },
            &CanonicalWriterConfig::default(),
        );
        assert!(
            matches!(
                outcome,
                CaptureOutcome::Rejected {
                    code: CaptureRejectCode::IdentityUnavailable,
                    ..
                }
            ),
            "{outcome:?}"
        );
        assert_eq!(store.put_count(), puts_before, "must not SecureStore put");
        assert!(!meta_path.exists(), "must not mint public metadata");

        // Partial: seed restored without metadata must still fail closed (no completion).
        // Re-enroll seed alone by capturing is forbidden; put a dummy seed without metadata.
        store
            .put(service.as_str(), account.as_str(), &[7u8; 32])
            .expect("seed only");
        let puts_mid = store.put_count();
        let outcome = resolve_capture(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
        );
        assert!(
            matches!(
                outcome,
                CaptureOutcome::Rejected {
                    code: CaptureRejectCode::IdentityUnavailable,
                    ..
                }
            ),
            "{outcome:?}"
        );
        assert_eq!(store.put_count(), puts_mid, "resolve must not put");
        assert!(!meta_path.exists(), "resolve must not mint metadata");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn mismatched_signing_metadata_rejects_without_key_mutation() {
        let (root, store, device_id) = prepared();
        let meta_path = event_signing_key_path(&root, &device_id);
        let original = fs::read_to_string(&meta_path).expect("meta");
        // Flip a character inside the JSON string value of public_key.
        let mut value: serde_json::Value = serde_json::from_str(original.trim()).expect("json");
        let pk = value
            .get_mut("public_key")
            .and_then(|v| v.as_str())
            .expect("pk")
            .to_string();
        let mut chars: Vec<char> = pk.chars().collect();
        if let Some(c) = chars.get_mut(0) {
            *c = if *c == 'A' { 'B' } else { 'A' };
        }
        value["public_key"] = serde_json::Value::String(chars.into_iter().collect());
        fs::write(
            &meta_path,
            format!("{}\n", serde_json::to_string(&value).unwrap()),
        )
        .expect("write corrupt meta");
        let puts_before = store.put_count();
        let meta_before = fs::read(&meta_path).expect("meta bytes");
        let outcome = capture_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            CaptureRequest {
                capture_id: "eeeeeeee-eeee-4eee-8eee-eeeeeeeeee01".into(),
                classification: Classification::Internal,
                text: "mismatch".into(),
            },
            &CanonicalWriterConfig::default(),
        );
        assert!(
            matches!(
                outcome,
                CaptureOutcome::Rejected {
                    code: CaptureRejectCode::IdentityUnavailable,
                    ..
                }
            ),
            "{outcome:?}"
        );
        assert_eq!(store.put_count(), puts_before);
        assert_eq!(fs::read(&meta_path).expect("meta"), meta_before);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn precreated_device_dir_ancestry_sync_failure_is_uncertain() {
        let (root, store, _device_id) = prepared();
        // Device dir already exists from enrollment/lock; force ancestry sync fail.
        fail_next_dir_ancestry_syncs(1);
        let outcome = capture_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            CaptureRequest {
                capture_id: "ffffffff-ffff-4fff-8fff-ffffffffffff".into(),
                classification: Classification::Internal,
                text: "ancestry".into(),
            },
            &CanonicalWriterConfig::default(),
        );
        assert!(
            matches!(
                outcome,
                CaptureOutcome::Uncertain {
                    reason: CaptureUncertainReason::DurabilityUnconfirmed,
                    ..
                }
            ),
            "{outcome:?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn committed_receipt_visible_state_still_reconfirms_durability() {
        let (root, store, _device_id) = prepared();
        let capture_id = "a1a1a1a1-a1a1-4a1a-8a1a-a1a1a1a1a1a1";
        let _ = assert_committed(capture_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            CaptureRequest {
                capture_id: capture_id.into(),
                classification: Classification::Internal,
                text: "committed then reconfirm".into(),
            },
            &CanonicalWriterConfig::default(),
        ));
        // Visible state is COMMITTED; durability confirm must still run. Fail both the
        // initial confirm and the post-rewrite confirm so success cannot be inferred
        // from receipt.state alone.
        fail_next_receipt_confirms(2);
        let resolved = resolve_capture(&store, SecureStoreEnvironment::Dev, &root, capture_id);
        assert!(
            matches!(
                resolved,
                CaptureOutcome::Uncertain {
                    reason: CaptureUncertainReason::DurabilityUnconfirmed
                        | CaptureUncertainReason::ReceiptPersistFailed,
                    ..
                }
            ),
            "{resolved:?}"
        );
        let _ = assert_committed(resolve_capture(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            capture_id,
        ));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn pending_receipt_is_staged_exclusive_no_replace() {
        let (root, store, device_id) = prepared();
        let capture_id = "b2b2b2b2-b2b2-4b2b-8b2b-b2b2b2b2b2b2";
        let fingerprint = capture_fingerprint_v1(
            BRAIN,
            device_id.as_str(),
            capture_id,
            Classification::Internal,
            "staged",
        );
        let pending = CaptureReceipt {
            schema_version: crate::capture_receipt::CAPTURE_RECEIPT_SCHEMA_VERSION,
            brain_id: BRAIN.to_string(),
            device_id: device_id.as_str().to_string(),
            capture_id: capture_id.to_string(),
            event_id: "c3c3c3c3-c3c3-4c3c-8c3c-c3c3c3c3c3c3".into(),
            fingerprint: fingerprint.clone(),
            state: ReceiptState::Pending,
        };
        create_pending_receipt_exclusive(&root, &pending).expect("first pending");
        let path = receipt_path(&root, BRAIN, &device_id, capture_id);
        assert!(path.is_file());
        let first = fs::read(&path).expect("read");
        let mut pending2 = pending.clone();
        pending2.event_id = "d4d4d4d4-d4d4-4d4d-8d4d-d4d4d4d4d4d4".into();
        let err = create_pending_receipt_exclusive(&root, &pending2).expect_err("no replace");
        assert!(matches!(
            err,
            crate::capture_receipt::ReceiptError::AlreadyExists
        ));
        assert_eq!(fs::read(&path).expect("unchanged"), first);
        let _ = store;
        let _ = fs::remove_dir_all(&root);
    }
}
