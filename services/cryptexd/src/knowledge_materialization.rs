//! Daemon-owned, read-only canonical scan followed by a transactional M4 rebuild.
//!
//! This path is intentionally separate from M2's repair-oriented v1 reader. It
//! never truncates, quarantines, seals, or rewrites canonical packs: unknown or
//! unverifiable envelopes stop the rebuild with source bytes intact.

use std::fs;
use std::path::Path;

use cryptex_core::DeviceId;
use cryptex_events::{TrustedDaemonKeyResolver, VersionedRecord, verify_v2_history};
use cryptex_knowledge::sqlite::{
    CanonicalFramePosition, DeviceMaterializationProgress, KnowledgeStoreError,
    migrate_knowledge_schema, rebuild_from_verified_history,
};
use cryptex_projection::ProjectionDb;
use cryptex_storage::layout::{
    active_segment_path, canonical_events_device_dir, sealed_segment_path,
};
use cryptex_storage::{AcceptAllBodies, StorageError, fold_verified_frames_with_offsets};

use crate::canonical_writer::{
    CanonicalWriterError, DeviceCanonicalLock, acquire_device_canonical_lock,
};

#[derive(Debug)]
pub enum KnowledgeMaterializationError {
    Canonical(CanonicalWriterError),
    Storage(StorageError),
    Verification(String),
    Knowledge(KnowledgeStoreError),
    InvalidInput(String),
}

impl std::fmt::Display for KnowledgeMaterializationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Canonical(error) => write!(f, "canonical scan failed: {error}"),
            Self::Storage(error) => write!(f, "canonical frame scan failed: {error}"),
            Self::Verification(error) => {
                write!(f, "canonical history verification failed: {error}")
            }
            Self::Knowledge(error) => write!(f, "knowledge rebuild failed: {error}"),
            Self::InvalidInput(error) => write!(f, "invalid M4 rebuild input: {error}"),
        }
    }
}

impl std::error::Error for KnowledgeMaterializationError {}

impl From<CanonicalWriterError> for KnowledgeMaterializationError {
    fn from(value: CanonicalWriterError) -> Self {
        Self::Canonical(value)
    }
}
impl From<StorageError> for KnowledgeMaterializationError {
    fn from(value: StorageError) -> Self {
        Self::Storage(value)
    }
}
impl From<KnowledgeStoreError> for KnowledgeMaterializationError {
    fn from(value: KnowledgeStoreError) -> Self {
        Self::Knowledge(value)
    }
}

pub type KnowledgeMaterializationResult<T> = Result<T, KnowledgeMaterializationError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnowledgeRebuildReport {
    pub verified_records: usize,
    pub materialized_assertions: usize,
    pub pending_dependencies: usize,
    pub device_count: usize,
}

#[derive(Debug)]
struct RawFrame {
    position: CanonicalFramePosition,
    bytes: Vec<u8>,
}

/// Rebuild all M4-derived rows from an explicit set of trusted device streams.
///
/// The caller supplies the enrolled-device key resolver; this function does
/// not infer trust from event contents or implement device enrollment. Locks
/// are acquired in stable DeviceId order and held through verification,
/// reduction, and the SQLite transaction (device locks → SQLite).
pub fn rebuild_knowledge_from_canonical<R: TrustedDaemonKeyResolver>(
    runtime_root: &Path,
    expected_brain_id: &str,
    trusted_devices: &[DeviceId],
    key_resolver: &mut R,
    db: &mut ProjectionDb,
) -> KnowledgeMaterializationResult<KnowledgeRebuildReport> {
    crate::paths::assert_outside_repository(runtime_root)
        .map_err(|error| KnowledgeMaterializationError::InvalidInput(error.to_string()))?;
    let mut devices = trusted_devices.to_vec();
    devices.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    if devices.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(KnowledgeMaterializationError::InvalidInput(
            "trusted device list contains duplicates".into(),
        ));
    }
    require_complete_device_inventory(runtime_root, &devices)?;

    let mut locks: Vec<DeviceCanonicalLock> = Vec::with_capacity(devices.len());
    for device in &devices {
        locks.push(acquire_device_canonical_lock(runtime_root, device)?);
    }

    let mut frames = Vec::<RawFrame>::new();
    for device in &devices {
        scan_device_read_only(runtime_root, device, &mut frames)?;
    }

    let raw_refs: Vec<&[u8]> = frames.iter().map(|frame| frame.bytes.as_slice()).collect();
    let history = verify_v2_history(&raw_refs, BorrowedResolver(key_resolver))
        .map_err(|error| KnowledgeMaterializationError::Verification(error.to_string()))?;
    if history.records().len() != frames.len() {
        return Err(KnowledgeMaterializationError::InvalidInput(
            "canonical scan contains duplicate event records; refusing ambiguous frame binding"
                .into(),
        ));
    }

    for (record, frame) in history.records().iter().zip(&mut frames) {
        let (event_id, device_id, content_hash) = record_identity(record).ok_or_else(|| {
            KnowledgeMaterializationError::InvalidInput(
                "verified history unexpectedly contains an unverifiable record".into(),
            )
        })?;
        if frame.position.event_id != event_id || frame.position.device_id != device_id {
            return Err(KnowledgeMaterializationError::InvalidInput(
                "verified record does not match its scanned canonical frame position".into(),
            ));
        }
        frame.position.content_hash = content_hash;
    }

    let state = cryptex_knowledge::reduce_verified_events(expected_brain_id, &history).map_err(
        |error| {
            KnowledgeMaterializationError::InvalidInput(format!(
                "reducer rejected verified history: {error:?}"
            ))
        },
    )?;
    let progress = build_progress(
        &devices,
        history.records(),
        &frames,
        &state.pending,
        &history
            .assertions()
            .iter()
            .map(|assertion| assertion.event().event_id.clone())
            .collect::<Vec<_>>(),
        &state.materialized_event_ids,
    );
    // Migrate only after the complete canonical set verified, so an unknown or
    // unverifiable envelope leaves both canonical packs and M3/M4 DB untouched.
    migrate_knowledge_schema(db.connection_mut())?;
    rebuild_from_verified_history(
        db.connection_mut(),
        expected_brain_id,
        &history,
        &frames
            .iter()
            .map(|frame| frame.position.clone())
            .collect::<Vec<_>>(),
        &progress,
    )?;

    // Keep locks alive through the end of the SQLite rebuild above.
    drop(locks);
    Ok(KnowledgeRebuildReport {
        verified_records: history.records().len(),
        materialized_assertions: history.assertions().len(),
        pending_dependencies: state.pending.len(),
        device_count: devices.len(),
    })
}

struct BorrowedResolver<'a, R>(&'a mut R);
impl<R: TrustedDaemonKeyResolver> TrustedDaemonKeyResolver for BorrowedResolver<'_, R> {
    fn trusted_daemon_key(
        &mut self,
        device_id: &str,
        key_id: &str,
    ) -> Option<ed25519_dalek::VerifyingKey> {
        self.0.trusted_daemon_key(device_id, key_id)
    }
}

fn scan_device_read_only(
    runtime_root: &Path,
    device_id: &DeviceId,
    output: &mut Vec<RawFrame>,
) -> KnowledgeMaterializationResult<()> {
    let directory = canonical_events_device_dir(runtime_root, device_id);
    if !directory.exists() {
        return Ok(());
    }
    let mut sealed = Vec::new();
    let mut open = None;
    for entry in fs::read_dir(&directory).map_err(StorageError::Io)? {
        let entry = entry.map_err(StorageError::Io)?;
        if !entry.file_type().map_err(StorageError::Io)?.is_file() {
            return Err(KnowledgeMaterializationError::Storage(
                StorageError::Integrity(
                    "canonical device directory contains a non-file entry".into(),
                ),
            ));
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        match (
            parse_pack_index(&name, "seg"),
            parse_pack_index(&name, "open"),
        ) {
            (Some(index), _) => sealed.push(index),
            (None, Some(index)) => {
                if open.replace(index).is_some() {
                    return Err(KnowledgeMaterializationError::Storage(
                        StorageError::Integrity("multiple active packs for one device".into()),
                    ));
                }
            }
            (None, None) if is_known_housekeeping_file(&name) => {}
            (None, None) if name.ends_with(".seg") || name.ends_with(".open") => {
                return Err(KnowledgeMaterializationError::Storage(
                    StorageError::Integrity(
                        "canonical directory contains a malformed pack filename".into(),
                    ),
                ));
            }
            (None, None) => {
                return Err(KnowledgeMaterializationError::Storage(
                    StorageError::Integrity(
                        "canonical device directory contains an unexpected entry".into(),
                    ),
                ));
            }
        }
    }
    sealed.sort_unstable();
    if sealed
        .iter()
        .enumerate()
        .any(|(expected, actual)| expected as u64 != *actual)
    {
        return Err(KnowledgeMaterializationError::Storage(
            StorageError::Integrity("sealed pack indexes are not contiguous".into()),
        ));
    }
    if let Some(open_index) = open
        && (sealed.contains(&open_index)
            || sealed.last().is_some_and(|last| open_index != last + 1)
            || (sealed.is_empty() && open_index != 0))
    {
        return Err(KnowledgeMaterializationError::Storage(
            StorageError::Integrity("active pack layout conflicts with sealed history".into()),
        ));
    }

    for index in sealed {
        let path = sealed_segment_path(runtime_root, device_id, index);
        let before = output.len();
        let verified_len = fold_pack(&path, device_id, index, output)?;
        let file_len = fs::metadata(&path).map_err(StorageError::Io)?.len();
        if verified_len != file_len || output.len() == before {
            return Err(KnowledgeMaterializationError::Storage(StorageError::Integrity(
                "sealed pack is incomplete or empty; M4 rebuild stopped without modifying canonical bytes".into(),
            )));
        }
    }
    if let Some(index) = open {
        let path = active_segment_path(runtime_root, device_id, index);
        let file_len = fs::metadata(&path).map_err(StorageError::Io)?.len();
        let verified_len = fold_pack(&path, device_id, index, output)?;
        if verified_len != file_len {
            return Err(KnowledgeMaterializationError::Storage(
                StorageError::CompatibilityBoundary(
                    "active pack has an incomplete or unverified suffix; M4 rebuild stopped without modifying canonical bytes".into(),
                ),
            ));
        }
    }
    Ok(())
}

fn is_known_housekeeping_file(name: &str) -> bool {
    if matches!(name, "writer.lock" | "writer_tip.json") {
        return true;
    }
    if let Some(suffix) = name.strip_prefix("writer_tip.json.tmp.") {
        return !suffix.is_empty()
            && suffix
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b'-');
    }
    name.strip_suffix(".open.lock")
        .is_some_and(|stem| stem.len() == 20 && stem.bytes().all(|byte| byte.is_ascii_digit()))
}

/// A full rebuild must not silently omit a canonical device stream. The
/// caller-provided trusted set is checked against the on-disk stream inventory
/// before locks are acquired; unknown or malformed directory names fail closed.
fn require_complete_device_inventory(
    runtime_root: &Path,
    trusted_devices: &[DeviceId],
) -> KnowledgeMaterializationResult<()> {
    let events_root = runtime_root.join("canonical").join("events");
    if !events_root.exists() {
        return Ok(());
    }
    let mut on_disk = Vec::<String>::new();
    for entry in fs::read_dir(&events_root).map_err(StorageError::Io)? {
        let entry = entry.map_err(StorageError::Io)?;
        let file_type = entry.file_type().map_err(StorageError::Io)?;
        if !file_type.is_dir() {
            return Err(KnowledgeMaterializationError::Storage(
                StorageError::Integrity(
                    "unexpected non-directory in canonical device inventory".into(),
                ),
            ));
        }
        let stem = entry.file_name().to_string_lossy().into_owned();
        let decoded = decode_device_stem(&stem).ok_or_else(|| {
            KnowledgeMaterializationError::Storage(StorageError::Integrity(
                "canonical device inventory contains an invalid id stem".into(),
            ))
        })?;
        on_disk.push(decoded);
    }
    on_disk.sort();
    let mut supplied: Vec<String> = trusted_devices
        .iter()
        .map(|device| device.as_str().to_owned())
        .collect();
    supplied.sort();
    if on_disk
        .iter()
        .any(|device| supplied.binary_search(device).is_err())
    {
        return Err(KnowledgeMaterializationError::InvalidInput(
            "trusted device set does not cover the complete canonical device inventory".into(),
        ));
    }
    Ok(())
}

fn decode_device_stem(stem: &str) -> Option<String> {
    if stem.is_empty()
        || !stem.len().is_multiple_of(2)
        || !stem
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    let mut bytes = Vec::with_capacity(stem.len() / 2);
    for pair in stem.as_bytes().as_chunks::<2>().0 {
        let high = hex_nibble(pair[0])?;
        let low = hex_nibble(pair[1])?;
        bytes.push((high << 4) | low);
    }
    let value = String::from_utf8(bytes).ok()?;
    DeviceId::new(value.clone()).ok()?;
    Some(value)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn fold_pack(
    path: &Path,
    device_id: &DeviceId,
    pack_index: u64,
    output: &mut Vec<RawFrame>,
) -> KnowledgeMaterializationResult<u64> {
    let (frames, verified_len) = fold_verified_frames_with_offsets(
        path,
        &AcceptAllBodies,
        Vec::new(),
        |frames, offset, body| {
            frames.push((offset, body.to_vec()));
            Ok(())
        },
    )?;
    for (offset, bytes) in frames {
        let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| {
            StorageError::CompatibilityBoundary(
                "canonical event is not valid JSON; M4 rebuild stopped without modifying source"
                    .into(),
            )
        })?;
        let event_id = value
            .get("event_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                StorageError::CompatibilityBoundary("canonical event id is unavailable".into())
            })?;
        let claimed_device = value
            .get("device_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                StorageError::CompatibilityBoundary("canonical device id is unavailable".into())
            })?;
        if claimed_device != device_id.as_str() {
            return Err(KnowledgeMaterializationError::Storage(
                StorageError::Integrity(
                    "canonical event device does not match its pack directory".into(),
                ),
            ));
        }
        output.push(RawFrame {
            position: CanonicalFramePosition {
                event_id: event_id.to_owned(),
                device_id: device_id.as_str().to_owned(),
                pack_index,
                frame_offset: offset,
                // Filled from the cryptographically verified classification
                // after the full chain has been checked.
                content_hash: String::new(),
            },
            bytes,
        });
    }
    Ok(verified_len)
}

fn parse_pack_index(name: &str, extension: &str) -> Option<u64> {
    let stem = name.strip_suffix(&format!(".{extension}"))?;
    if stem.len() != 20 || !stem.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    stem.parse().ok()
}

fn record_identity(record: &VersionedRecord) -> Option<(String, String, String)> {
    match record {
        VersionedRecord::VerifiedV1(event) => Some((
            event.event_id.clone(),
            event.device_id.as_str().to_owned(),
            event.content_hash.as_str().to_owned(),
        )),
        VersionedRecord::VerifiedV2Supported(event) => Some((
            event.event().event_id.clone(),
            event.event().device_id.as_str().to_owned(),
            event.event().content_hash.as_str().to_owned(),
        )),
        VersionedRecord::VerifiedUnsupported(event) => Some((
            event.event_id.clone(),
            event.device_id.clone(),
            event.content_hash.as_str().to_owned(),
        )),
        VersionedRecord::Unverifiable(_) => None,
    }
}

fn build_progress(
    devices: &[DeviceId],
    records: &[VersionedRecord],
    frames: &[RawFrame],
    pending: &[cryptex_knowledge::PendingDependency],
    supported_event_ids: &[String],
    materialized_event_ids: &[String],
) -> Vec<DeviceMaterializationProgress> {
    let pending_ids: std::collections::BTreeSet<String> = pending
        .iter()
        .map(|item| item.event_id.to_ascii_lowercase())
        .collect();
    let materialized_ids: std::collections::BTreeSet<String> = materialized_event_ids
        .iter()
        .map(|event_id| event_id.to_ascii_lowercase())
        .collect();
    let unsupported_ids: std::collections::BTreeSet<String> = records
        .iter()
        .filter_map(|record| match record {
            VersionedRecord::VerifiedUnsupported(event) => {
                Some(event.event_id.to_ascii_lowercase())
            }
            _ => None,
        })
        .collect();
    // Supported v2 records omitted by the reducer are not safe progress. This
    // includes semantically invalid records retained as reviewable conflicts;
    // conservatively stop the contiguous watermark instead of implying that
    // their semantics were successfully materialized.
    let unmaterialized_supported_ids: std::collections::BTreeSet<String> = supported_event_ids
        .iter()
        .map(|event_id| event_id.to_ascii_lowercase())
        .filter(|event_id| !materialized_ids.contains(event_id))
        .collect();
    let mut output = Vec::with_capacity(devices.len());
    for device in devices {
        let indexes: Vec<usize> = frames
            .iter()
            .enumerate()
            .filter_map(|(index, frame)| {
                (frame.position.device_id == device.as_str()).then_some(index)
            })
            .collect();
        let integrity_position = indexes.last().map(|index| frames[*index].position.clone());
        let first_blocker = indexes.iter().position(|index| {
            let event_id = frames[*index].position.event_id.to_ascii_lowercase();
            pending_ids.contains(&event_id)
                || unsupported_ids.contains(&event_id)
                || unmaterialized_supported_ids.contains(&event_id)
        });
        let safe_index = first_blocker.unwrap_or(indexes.len());
        let safe_position = safe_index
            .checked_sub(1)
            .and_then(|last| indexes.get(last))
            .map(|index| frames[*index].position.clone());
        let blocking_reason = first_blocker.map(|position| {
            let event_id = frames[indexes[position]]
                .position
                .event_id
                .to_ascii_lowercase();
            if unsupported_ids.contains(&event_id) {
                "verified_unsupported_event".to_owned()
            } else if unmaterialized_supported_ids.contains(&event_id) {
                "unmaterialized_supported_event".to_owned()
            } else {
                "pending_dependency".to_owned()
            }
        });
        output.push(DeviceMaterializationProgress {
            device_id: device.as_str().to_owned(),
            integrity_position,
            safe_position,
            blocking_reason,
        });
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canonical_writer::{AppendMemoryNoteArgs, append_memory_note};
    use crate::event_signing_key::ensure_event_signing_key;
    use crate::secure_store::{FakeSecureStore, SecureStoreEnvironment};
    use crate::test_temp::exclusive_temp_dir;
    use cryptex_events::{Classification, TrustedDaemonKeyResolver};
    use cryptex_projection::ProjectionDb;
    use ed25519_dalek::VerifyingKey;
    use std::fs;
    use std::io::Write;

    struct OneTrustedDevice {
        device: String,
        key_id: String,
        key: VerifyingKey,
    }

    impl TrustedDaemonKeyResolver for OneTrustedDevice {
        fn trusted_daemon_key(&mut self, device_id: &str, key_id: &str) -> Option<VerifyingKey> {
            (device_id == self.device && key_id == self.key_id).then_some(self.key)
        }
    }

    #[test]
    fn daemon_rebuild_scans_verified_v1_frames_without_changing_canonical_bytes() {
        let root = exclusive_temp_dir("m4-knowledge-reader");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("m4-reader-device").unwrap();
        let brain = "33333333-3333-4333-8333-333333333333";
        let config = crate::canonical_writer::CanonicalWriterConfig::default();
        append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Internal,
                text: "synthetic note",
                config: &config,
            },
        )
        .unwrap();
        let pack = active_segment_path(&root, &device, 0);
        let before = fs::read(&pack).unwrap();
        let ready = ensure_event_signing_key(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            &device,
            &[],
            true,
        )
        .unwrap();
        let mut resolver = OneTrustedDevice {
            device: device.as_str().to_owned(),
            key_id: ready.public.signing_key_id,
            key: ready.signing_key.verifying_key(),
        };
        let mut db = ProjectionDb::open_in_memory().unwrap();

        let report = rebuild_knowledge_from_canonical(
            &root,
            brain,
            std::slice::from_ref(&device),
            &mut resolver,
            &mut db,
        )
        .unwrap();

        assert_eq!(report.verified_records, 1);
        assert_eq!(report.materialized_assertions, 0);
        assert_eq!(report.pending_dependencies, 0);
        assert_eq!(report.device_count, 1);
        assert_eq!(fs::read(&pack).unwrap(), before);
        let row_count: i64 = db
            .connection()
            .query_row("SELECT COUNT(*) FROM knowledge_meta", [], |row| row.get(0))
            .unwrap();
        assert_eq!(row_count, 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn pending_dependency_keeps_safe_watermark_before_blocking_frame() {
        let device = DeviceId::new("progress-device").unwrap();
        let frames = [1, 2, 3].map(|index| RawFrame {
            position: CanonicalFramePosition {
                event_id: format!("event-{index}"),
                device_id: device.as_str().to_owned(),
                pack_index: 0,
                frame_offset: (index - 1) * 100,
                content_hash: format!("hash-{index}"),
            },
            bytes: Vec::new(),
        });
        let pending = cryptex_knowledge::PendingDependency {
            event_id: "event-2".into(),
            target_id: "missing-event".into(),
            reason: "missing_assertion",
        };
        let progress = build_progress(&[device], &[], &frames, &[pending], &[], &[]);
        assert_eq!(
            progress[0].integrity_position,
            Some(frames[2].position.clone())
        );
        assert_eq!(progress[0].safe_position, Some(frames[0].position.clone()));
        assert_eq!(
            progress[0].blocking_reason.as_deref(),
            Some("pending_dependency")
        );
    }

    #[test]
    fn supported_event_omitted_by_reducer_blocks_safe_watermark() {
        let device = DeviceId::new("invalid-semantics-device").unwrap();
        let frames = [1, 2, 3].map(|index| RawFrame {
            position: CanonicalFramePosition {
                event_id: format!("event-{index}"),
                device_id: device.as_str().to_owned(),
                pack_index: 0,
                frame_offset: (index - 1) * 100,
                content_hash: format!("hash-{index}"),
            },
            bytes: Vec::new(),
        });

        // `event-2` is a supported signed v2 record whose semantics were not
        // accepted by the reducer (for example, a wrong-kind reference).
        let progress = build_progress(
            &[device],
            &[],
            &frames,
            &[],
            &["event-2".into()],
            &["event-1".into()],
        );

        assert_eq!(
            progress[0].integrity_position,
            Some(frames[2].position.clone())
        );
        assert_eq!(progress[0].safe_position, Some(frames[0].position.clone()));
        assert_eq!(
            progress[0].blocking_reason.as_deref(),
            Some("unmaterialized_supported_event")
        );
    }

    #[test]
    fn full_rebuild_refuses_to_omit_existing_canonical_device_stream() {
        let root = exclusive_temp_dir("m4-device-inventory");
        let unexpected = DeviceId::new("not-in-trust-set").unwrap();
        fs::create_dir_all(canonical_events_device_dir(&root, &unexpected)).unwrap();
        let trusted = DeviceId::new("trusted-device").unwrap();
        let error = require_complete_device_inventory(&root, &[trusted]).unwrap_err();
        assert!(matches!(
            error,
            KnowledgeMaterializationError::InvalidInput(_)
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn verified_unsupported_event_blocks_safe_watermark_but_not_integrity() {
        let device = DeviceId::new("unsupported-progress-device").unwrap();
        let frames = [1, 2, 3].map(|index| RawFrame {
            position: CanonicalFramePosition {
                event_id: format!("event-{index}"),
                device_id: device.as_str().to_owned(),
                pack_index: 0,
                frame_offset: (index - 1) * 100,
                content_hash: format!("hash-{index}"),
            },
            bytes: Vec::new(),
        });
        let unsupported =
            VersionedRecord::VerifiedUnsupported(cryptex_events::VerifiedUnsupportedV2 {
                event_id: "event-2".into(),
                device_id: device.as_str().to_owned(),
                device_seq: cryptex_events::DeviceSeq::from_u64(2).unwrap(),
                signing_key_id: cryptex_events::hex::SigningKeyId::parse(&"11".repeat(16)).unwrap(),
                prev_content_hash: cryptex_events::ContentHashHex::zero(),
                content_hash: cryptex_events::ContentHashHex::parse(&"22".repeat(32)).unwrap(),
                raw_record: b"verified unsupported synthetic record".to_vec(),
            });
        let progress = build_progress(&[device], &[unsupported], &frames, &[], &[], &[]);
        assert_eq!(
            progress[0].integrity_position,
            Some(frames[2].position.clone())
        );
        assert_eq!(progress[0].safe_position, Some(frames[0].position.clone()));
        assert_eq!(
            progress[0].blocking_reason.as_deref(),
            Some("verified_unsupported_event")
        );
    }

    #[test]
    fn unverifiable_envelope_stops_rebuild_without_rewriting_source_or_db() {
        let root = exclusive_temp_dir("m4-unknown-envelope");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("m4-unknown-device").unwrap();
        let brain = "33333333-3333-4333-8333-333333333333";
        let config = crate::canonical_writer::CanonicalWriterConfig::default();
        append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Internal,
                text: "synthetic note",
                config: &config,
            },
        )
        .unwrap();
        let ready = ensure_event_signing_key(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            &device,
            &[],
            true,
        )
        .unwrap();
        let unknown = serde_json::json!({
            "schema_version": 99,
            "event_id": "44444444-4444-4444-8444-444444444444",
            "device_id": device.as_str(),
            "signing_key_id": ready.public.signing_key_id,
            "opaque": ["do not parse or rewrite"]
        });
        let unknown_bytes = serde_json::to_vec(&unknown).unwrap();
        cryptex_storage::append_frame(&root, &device, 0, &unknown_bytes).unwrap();
        let pack = active_segment_path(&root, &device, 0);
        let before = fs::read(&pack).unwrap();
        let mut resolver = OneTrustedDevice {
            device: device.as_str().to_owned(),
            key_id: ready.public.signing_key_id,
            key: ready.signing_key.verifying_key(),
        };
        let mut db = ProjectionDb::open_in_memory().unwrap();
        db.connection()
            .execute_batch(
                "CREATE TABLE rebuild_sentinel (value TEXT NOT NULL);
                 INSERT INTO rebuild_sentinel VALUES ('keep');",
            )
            .unwrap();

        let result = rebuild_knowledge_from_canonical(
            &root,
            brain,
            std::slice::from_ref(&device),
            &mut resolver,
            &mut db,
        );

        assert!(matches!(
            result,
            Err(KnowledgeMaterializationError::Verification(_))
        ));
        assert_eq!(fs::read(&pack).unwrap(), before);
        let sentinel: String = db
            .connection()
            .query_row("SELECT value FROM rebuild_sentinel", [], |row| row.get(0))
            .unwrap();
        assert_eq!(sentinel, "keep");
        let m4_tables: i64 = db
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='knowledge_meta'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(m4_tables, 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn incomplete_active_suffix_stops_rebuild_before_migration_and_keeps_bytes() {
        let root = exclusive_temp_dir("m4-incomplete-open");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("m4-incomplete-open-device").unwrap();
        let brain = "33333333-3333-4333-8333-333333333333";
        let config = crate::canonical_writer::CanonicalWriterConfig::default();
        append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Internal,
                text: "synthetic note",
                config: &config,
            },
        )
        .unwrap();
        let ready = ensure_event_signing_key(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            &device,
            &[],
            true,
        )
        .unwrap();
        let mut resolver = OneTrustedDevice {
            device: device.as_str().to_owned(),
            key_id: ready.public.signing_key_id,
            key: ready.signing_key.verifying_key(),
        };
        let pack = active_segment_path(&root, &device, 0);
        let mut file = fs::OpenOptions::new().append(true).open(&pack).unwrap();
        file.write_all(&[0x43, 0x58]).unwrap();
        file.sync_all().unwrap();
        let before = fs::read(&pack).unwrap();
        let mut db = ProjectionDb::open_in_memory().unwrap();
        db.connection()
            .execute_batch(
                "CREATE TABLE rebuild_sentinel (value TEXT NOT NULL);
                 INSERT INTO rebuild_sentinel VALUES ('keep');",
            )
            .unwrap();

        let result = rebuild_knowledge_from_canonical(
            &root,
            brain,
            std::slice::from_ref(&device),
            &mut resolver,
            &mut db,
        );

        assert!(matches!(
            result,
            Err(KnowledgeMaterializationError::Storage(
                StorageError::CompatibilityBoundary(_)
            ))
        ));
        assert_eq!(fs::read(&pack).unwrap(), before);
        let sentinel: String = db
            .connection()
            .query_row("SELECT value FROM rebuild_sentinel", [], |row| row.get(0))
            .unwrap();
        assert_eq!(sentinel, "keep");
        let m4_tables: i64 = db
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='knowledge_meta'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(m4_tables, 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn malformed_pack_name_blocks_full_rebuild_instead_of_being_ignored() {
        let root = exclusive_temp_dir("m4-malformed-pack-name");
        let device = DeviceId::new("m4-malformed-name-device").unwrap();
        let directory = canonical_events_device_dir(&root, &device);
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("not-an-index.seg"), b"must not be omitted").unwrap();
        let result = scan_device_read_only(&root, &device, &mut Vec::new());
        assert!(matches!(
            result,
            Err(KnowledgeMaterializationError::Storage(
                StorageError::Integrity(_)
            ))
        ));
        fs::remove_dir_all(root).unwrap();
    }
}
