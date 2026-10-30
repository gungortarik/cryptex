//! Daemon-owned, bounded snapshots of untrusted incoming sealed segments.
//!
//! This module snapshots and verifies candidate bytes, records a prepared
//! manifest, and reconciles import transactions. The internal publisher
//! requires the local trust/cutoff and M3/M4 proof token; managed transport
//! does not call it yet.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use cryptex_core::DeviceId;
use cryptex_events::{
    ContentHashHex, FRAME_HEADER_BYTES, FRAME_VERSION_V1, TrustedDaemonKeyResolver,
    VersionedRecord, ZERO_CONTENT_HASH_HEX, decode_frame_header, read_versioned_record,
};
use cryptex_storage::layout::{
    active_segment_path, canonical_events_device_dir, sealed_segment_path,
};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::canonical_writer::{
    acquire_canonical_inventory_lock, acquire_existing_device_canonical_lock,
};
use crate::import_limits::{ImportBatchBudget, ImportLimitError};
use crate::paths::assert_outside_repository;

const COPY_BUFFER_BYTES: usize = 64 * 1024;
const MAX_PREPARED_MANIFEST_BYTES: u64 = 64 * 1024;
const MAX_COMMITTED_MARKER_BYTES: u64 = 4 * 1024;
#[cfg(windows)]
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

type PreparedRecoveryPolicyVerifier<'a> =
    dyn FnMut(&Path, &Path, &Path) -> Result<(), ImportStagingError> + 'a;

#[derive(Debug)]
pub enum ImportStagingError {
    Io(String),
    Invalid(String),
    Limit(ImportLimitError),
    RecoveryConflict(String),
}

impl std::fmt::Display for ImportStagingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(message) => write!(f, "import staging I/O: {message}"),
            Self::Invalid(message) => write!(f, "import staging rejected: {message}"),
            Self::Limit(error) => write!(f, "import staging limit: {error}"),
            Self::RecoveryConflict(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ImportStagingError {}

impl From<io::Error> for ImportStagingError {
    fn from(value: io::Error) -> Self {
        Self::Io(value.to_string())
    }
}

impl From<ImportLimitError> for ImportStagingError {
    fn from(value: ImportLimitError) -> Self {
        Self::Limit(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedSegment {
    pub device_id: DeviceId,
    pub segment_index: u64,
    pub length: u64,
    pub sha256: String,
    pub staged_path: PathBuf,
}

/// Last verified event boundary immediately before a staged segment.
/// Callers must derive this from the complete verified local stream while
/// holding the canonical locks; a transport claim is never a stream tip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportStreamTip {
    /// Last fully verified sealed segment already present for this device.
    /// `None` means the canonical stream has no sealed segments.
    pub(crate) segment_index: Option<u64>,
    pub(crate) device_seq: Option<u64>,
    pub(crate) content_hash: ContentHashHex,
}

impl ImportStreamTip {
    #[must_use]
    pub fn genesis() -> Self {
        Self {
            segment_index: None,
            device_seq: None,
            content_hash: ContentHashHex::zero(),
        }
    }

    #[must_use]
    pub fn segment_index(&self) -> Option<u64> {
        self.segment_index
    }

    #[must_use]
    pub fn device_seq(&self) -> Option<u64> {
        self.device_seq
    }

    #[must_use]
    pub fn content_hash(&self) -> &ContentHashHex {
        &self.content_hash
    }
}

/// Result of validating the exact staged snapshot. This establishes record
/// integrity and stream continuity only; it does not authorize publication or
/// replace RetrievalSafetyState/cutoff policy checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedStagedSegment {
    pub device_id: DeviceId,
    pub segment_index: u64,
    pub event_count: u64,
    pub device_seq: u64,
    pub content_hash: ContentHashHex,
    pub sha256: String,
    records: Vec<VerifiedStagedRecord>,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct VerifiedStagedRecord {
    pub(crate) device_id: DeviceId,
    pub(crate) device_seq: u64,
    pub(crate) event_id: String,
    pub(crate) security_control: bool,
    pub(crate) content_v1: bool,
    pub(crate) bytes: Vec<u8>,
}

impl std::fmt::Debug for VerifiedStagedRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VerifiedStagedRecord")
            .field("device_id", &self.device_id)
            .field("device_seq", &self.device_seq)
            .field("event_id", &self.event_id)
            .field("security_control", &self.security_control)
            .field("content_v1", &self.content_v1)
            .field("bytes_len", &self.bytes.len())
            .finish()
    }
}

/// Deterministically verified staged segments and the resulting per-device
/// tips. The supplied starting tips must come from a complete verified local
/// inventory while canonical locks are held; this value alone never
/// authorizes publication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedStagedBatch {
    segments: Vec<VerifiedStagedSegment>,
    end_tips: Vec<(DeviceId, ImportStreamTip)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct PreparedImportManifest {
    schema_version: u32,
    transaction_id: String,
    segments: Vec<PreparedImportSegment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct PreparedImportSegment {
    device_id: String,
    segment_index: u64,
    staged_relative_path: String,
    destination_relative_path: String,
    length: u64,
    sha256: String,
}

/// Closed, immutable commit decision for a prepared import transaction.
/// Parsing this record does not itself authorize publication or recovery.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct CommittedImportMarker {
    protocol_version: u32,
    transaction_id: String,
    prepared_manifest_sha256: String,
}

impl VerifiedStagedBatch {
    #[must_use]
    pub fn segments(&self) -> &[VerifiedStagedSegment] {
        &self.segments
    }

    #[must_use]
    pub fn end_tips(&self) -> &[(DeviceId, ImportStreamTip)] {
        &self.end_tips
    }

    pub(crate) fn records(&self) -> impl Iterator<Item = &VerifiedStagedRecord> {
        self.segments
            .iter()
            .flat_map(|segment| segment.records.iter())
    }
}

/// One in-progress import batch. The OS file lock enforces the one-prepared-
/// transaction bound across daemon processes and is released when this value
/// is dropped. Callers must drop it before acquiring canonical writer locks.
pub struct ImportStagingSession {
    runtime_root: PathBuf,
    transaction_id: String,
    staging_dir: PathBuf,
    _single_flight: Option<File>,
    _directory_handles: Vec<File>,
    budget: ImportBatchBudget,
    candidates: Vec<StagedSegment>,
    failed: bool,
    prepared: bool,
}

impl ImportStagingSession {
    pub fn begin(runtime_root: &Path) -> Result<Self, ImportStagingError> {
        if !runtime_root.is_absolute() {
            return Err(ImportStagingError::Invalid(
                "runtime root must be absolute".into(),
            ));
        }
        assert_outside_repository(runtime_root)
            .map_err(|error| ImportStagingError::Invalid(error.to_string()))?;
        ensure_plain_directory(runtime_root)?;

        let canonical = runtime_root.join("canonical");
        let events_root = canonical.join("events");
        let staging_root = canonical.join("import-staging");
        let journal_root = canonical.join("import-journal");
        ensure_directory_chain(&canonical)?;
        ensure_directory_chain(&events_root)?;
        ensure_directory_chain(&staging_root)?;
        ensure_directory_chain(&journal_root)?;
        if !same_volume(&events_root, &staging_root)? {
            return Err(ImportStagingError::Invalid(
                "canonical events and import staging must be on the same volume".into(),
            ));
        }

        let mut directory_handles = Vec::new();
        for directory in [
            runtime_root,
            canonical.as_path(),
            events_root.as_path(),
            staging_root.as_path(),
            journal_root.as_path(),
        ] {
            directory_handles.extend(inspect_directory_chain(directory, true)?);
        }

        let lock_path = journal_root.join("import.lock");
        let single_flight = open_lock_file(&lock_path)?;
        single_flight.try_lock_exclusive().map_err(|error| {
            ImportStagingError::Invalid(format!(
                "another import transaction owns the staging slot: {error}"
            ))
        })?;

        let mut staged_transactions = fs::read_dir(&staging_root)?;
        if let Some(entry) = staged_transactions.next() {
            entry?;
            return Err(ImportStagingError::RecoveryConflict(
                "an unprepared staging transaction already occupies the import slot".into(),
            ));
        }
        for entry in fs::read_dir(&journal_root)? {
            if entry?.file_name() != "import.lock" {
                return Err(ImportStagingError::RecoveryConflict(
                    "prepared or unknown journal state occupies the import slot".into(),
                ));
            }
        }

        let transaction_id = random_transaction_id()?;
        let transaction_dir = staging_root.join(&transaction_id);
        fs::create_dir(&transaction_dir)?;
        ensure_plain_directory(&transaction_dir)?;
        directory_handles.extend(inspect_directory_chain(&transaction_dir, true)?);

        Ok(Self {
            runtime_root: runtime_root.to_path_buf(),
            transaction_id,
            staging_dir: transaction_dir,
            _single_flight: Some(single_flight),
            _directory_handles: directory_handles,
            budget: ImportBatchBudget::empty(),
            candidates: Vec::new(),
            failed: false,
            prepared: false,
        })
    }

    /// Copy one `.seg` candidate from the fixed incoming folder into this
    /// transaction's private staging tree. The returned digest identifies the
    /// exact copied snapshot; semantic/signature validation remains a later
    /// required transaction step.
    pub fn stage_segment(
        &mut self,
        incoming_relative_path: &Path,
        device_id: &DeviceId,
        segment_index: u64,
    ) -> Result<&StagedSegment, ImportStagingError> {
        if self._single_flight.is_none() || self.prepared {
            return Err(ImportStagingError::Invalid(
                "staging is closed or already prepared; no new candidates may be added".into(),
            ));
        }
        if self.failed {
            return Err(ImportStagingError::Invalid(
                "batch already failed; discard the entire staging transaction".into(),
            ));
        }
        if self.candidates.iter().any(|candidate| {
            candidate.device_id == *device_id && candidate.segment_index == segment_index
        }) {
            self.failed = true;
            self._directory_handles.pop();
            return match remove_unprepared_transaction(&self.staging_dir) {
                Ok(()) => Err(ImportStagingError::Invalid(
                    "batch contains a duplicate device and segment index".into(),
                )),
                Err(cleanup) => Err(ImportStagingError::Invalid(format!(
                    "batch contains a duplicate device and segment index; staging rollback also failed: {cleanup}"
                ))),
            };
        }
        let result = self.stage_segment_inner(incoming_relative_path, device_id, segment_index);
        if let Err(primary) = result {
            self.failed = true;
            // Keep the pinned ancestors through rollback so a path-based cleanup
            // cannot be redirected by replacing the transaction's parent chain.
            // The transaction directory itself must be unpinned for removal.
            self._directory_handles.pop();
            return match remove_unprepared_transaction(&self.staging_dir) {
                Ok(()) => Err(primary),
                Err(cleanup) => Err(ImportStagingError::Invalid(format!(
                    "{primary}; staging rollback also failed: {cleanup}"
                ))),
            };
        }
        Ok(self
            .candidates
            .last()
            .expect("successful stage appends candidate"))
    }

    /// Snapshot one deterministic batch of paths discovered in the fixed
    /// `transport/incoming` tree. Device identity and segment index are
    /// decoded from the canonical relative filename instead of being supplied
    /// separately by the caller. This only copies untrusted bytes into private
    /// staging; callers must still verify the staged batch and apply the M8
    /// trust/cutoff/retrieval-safety gates before any canonical publication.
    pub fn stage_batch_from_transport_paths(
        &mut self,
        incoming_relative_paths: &[PathBuf],
    ) -> Result<&[StagedSegment], ImportStagingError> {
        if self._single_flight.is_none() || self.prepared || self.failed {
            return Err(ImportStagingError::Invalid(
                "staging is closed, failed, or already prepared".into(),
            ));
        }
        if !self.candidates.is_empty() {
            return Err(ImportStagingError::Invalid(
                "transport batch must be staged into an empty transaction".into(),
            ));
        }
        if incoming_relative_paths.is_empty() {
            return Err(ImportStagingError::Invalid(
                "transport batch contains no sealed segment paths".into(),
            ));
        }
        if incoming_relative_paths.len() > crate::import_limits::MAX_IMPORT_FILES_PER_BATCH {
            return Err(ImportStagingError::Limit(ImportLimitError::TooManyFiles));
        }

        let mut parsed = incoming_relative_paths
            .iter()
            .map(|path| {
                let (device_id, segment_index) = parse_transport_segment_path(path)?;
                Ok((device_id, segment_index, path.clone()))
            })
            .collect::<Result<Vec<_>, ImportStagingError>>()?;
        parsed.sort_by(|left, right| {
            left.0
                .as_str()
                .as_bytes()
                .cmp(right.0.as_str().as_bytes())
                .then_with(|| left.1.cmp(&right.1))
        });
        if parsed
            .windows(2)
            .any(|pair| pair[0].0 == pair[1].0 && pair[0].1 == pair[1].1)
        {
            return Err(ImportStagingError::Invalid(
                "transport batch contains a duplicate device and segment index".into(),
            ));
        }

        for (device_id, segment_index, relative_path) in parsed {
            self.stage_segment(&relative_path, &device_id, segment_index)?;
        }
        Ok(&self.candidates)
    }

    /// Explicitly discard this unprepared staging transaction. Cleanup errors
    /// are returned; callers must not proceed to canonical locks after failure.
    pub fn discard(mut self) -> Result<(), ImportStagingError> {
        if self.prepared {
            return Err(ImportStagingError::RecoveryConflict(
                "a prepared import transaction requires startup reconciliation".into(),
            ));
        }
        self._directory_handles.clear();
        remove_unprepared_transaction(&self.staging_dir)
    }

    fn stage_segment_inner(
        &mut self,
        incoming_relative_path: &Path,
        device_id: &DeviceId,
        segment_index: u64,
    ) -> Result<(), ImportStagingError> {
        validate_relative_segment_path(incoming_relative_path)?;
        validate_import_path_binding(incoming_relative_path, device_id, segment_index)?;
        let incoming_root = self.runtime_root.join("transport").join("incoming");
        if let Some(transport_root) = incoming_root.parent() {
            ensure_existing_plain_directory(transport_root)?;
        }
        ensure_existing_plain_directory(&incoming_root)?;
        let source_path = incoming_root.join(incoming_relative_path);
        validate_plain_path_below(&incoming_root, incoming_relative_path)?;
        let incoming_handles = inspect_directory_chain(
            source_path.parent().ok_or_else(|| {
                ImportStagingError::Invalid("incoming candidate has no parent directory".into())
            })?,
            true,
        )?;

        let mut source = open_source_from_directory_chain(&incoming_handles, &source_path)?;
        let initial = source_snapshot(&source)?;
        if !initial.is_regular_file {
            return Err(ImportStagingError::Invalid(
                "incoming candidate is not an ordinary file".into(),
            ));
        }
        if canonical_file_id_alias_exists(&self.runtime_root, initial.identity)? {
            return Err(ImportStagingError::Invalid(
                "incoming candidate aliases an entry under canonical/events".into(),
            ));
        }
        if initial.link_count != 1 {
            return Err(ImportStagingError::Invalid(format!(
                "incoming candidate has {} links; exactly one is required",
                initial.link_count
            )));
        }
        self.budget.admit_file(initial.length)?;

        let device_dir = self.staging_dir.join(device_id.as_filename_stem());
        ensure_directory_chain(&device_dir)?;
        let _device_handles = inspect_directory_chain(&device_dir, true)?;
        let final_path = device_dir.join(format!("{segment_index:020}.seg"));
        let temp_path = device_dir.join(format!(".{}.tmp", random_hex_16()?));
        let mut staged = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&temp_path)?;

        let (copied, digest) = copy_bounded(&mut source, &mut staged, initial.length)?;
        if copied != initial.length {
            return Err(ImportStagingError::Invalid(format!(
                "candidate changed length during snapshot: expected {}, copied {copied}",
                initial.length
            )));
        }
        let final_source = source_snapshot(&source)?;
        if initial != final_source {
            return Err(ImportStagingError::Invalid(
                "candidate identity or metadata changed during snapshot".into(),
            ));
        }
        staged.sync_all()?;
        staged.seek(SeekFrom::Start(0))?;
        let (readback_length, readback_digest) = hash_bounded(&mut staged, initial.length)?;
        if readback_length != copied || readback_digest != digest {
            return Err(ImportStagingError::Invalid(
                "staged bytes did not match the copied source snapshot".into(),
            ));
        }
        staged.sync_all()?;
        drop(staged);
        publish_staged_no_replace(&temp_path, &final_path)?;
        sync_staged_parent(&final_path)?;

        self.candidates.push(StagedSegment {
            device_id: device_id.clone(),
            segment_index,
            length: copied,
            sha256: digest,
            staged_path: final_path,
        });
        Ok(())
    }

    pub fn transaction_id(&self) -> &str {
        &self.transaction_id
    }

    pub fn candidates(&self) -> &[StagedSegment] {
        &self.candidates
    }

    /// Verify the exact daemon-staged snapshot against a caller-supplied,
    /// fully verified stream tip and trusted daemon-key resolver. This is a
    /// validation primitive only; it does not publish or grant trust.
    pub fn verify_candidate<R: TrustedDaemonKeyResolver>(
        &self,
        candidate: &StagedSegment,
        expected_brain_id: &str,
        prior_tip: &ImportStreamTip,
        key_resolver: &mut R,
    ) -> Result<VerifiedStagedSegment, ImportStagingError> {
        if !self.candidates.iter().any(|staged| staged == candidate) {
            return Err(ImportStagingError::Invalid(
                "candidate does not belong to this staging transaction".into(),
            ));
        }
        let expected_path = self
            .staging_dir
            .join(candidate.device_id.as_filename_stem())
            .join(format!("{:020}.seg", candidate.segment_index));
        if candidate.staged_path != expected_path {
            return Err(ImportStagingError::Invalid(
                "candidate path does not match its staging identity".into(),
            ));
        }
        let parent_handles = inspect_directory_chain(
            expected_path.parent().ok_or_else(|| {
                ImportStagingError::Invalid("staged segment has no parent directory".into())
            })?,
            true,
        )?;
        let mut staged = open_source_from_directory_chain(&parent_handles, &expected_path)?;
        let initial = source_snapshot(&staged)?;
        if !initial.is_regular_file || initial.link_count != 1 || initial.length != candidate.length
        {
            return Err(ImportStagingError::Invalid(
                "staged segment identity or length changed before validation".into(),
            ));
        }

        let summary = verify_segment_stream(
            &mut staged,
            candidate,
            expected_brain_id,
            prior_tip,
            key_resolver,
        )?;
        if initial != source_snapshot(&staged)? {
            return Err(ImportStagingError::Invalid(
                "staged segment changed while it was being validated".into(),
            ));
        }
        Ok(summary)
    }

    /// Verify every staged candidate in deterministic per-device segment
    /// order. This rejects missing segment indexes and broken cross-segment
    /// event chains before a caller can prepare a batch. `prior_tips` must
    /// contain exactly one complete, lock-scoped canonical tip for each
    /// candidate device; an absent stream must be represented explicitly by
    /// `ImportStreamTip::genesis()`.
    pub fn verify_batch<R: TrustedDaemonKeyResolver>(
        &self,
        expected_brain_id: &str,
        prior_tips: &[(DeviceId, ImportStreamTip)],
        key_resolver: &mut R,
    ) -> Result<VerifiedStagedBatch, ImportStagingError> {
        if self.failed || self.prepared || self.candidates.is_empty() {
            return Err(ImportStagingError::Invalid(
                "cannot verify an empty, failed, or prepared staging batch".into(),
            ));
        }

        let mut candidates = self.candidates.iter().collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            left.device_id
                .as_str()
                .as_bytes()
                .cmp(right.device_id.as_str().as_bytes())
                .then_with(|| left.segment_index.cmp(&right.segment_index))
        });
        let candidate_devices = candidates
            .iter()
            .map(|candidate| candidate.device_id.clone())
            .collect::<std::collections::HashSet<_>>();
        let prior_devices = prior_tips
            .iter()
            .map(|(device, _)| device.clone())
            .collect::<std::collections::HashSet<_>>();
        if candidate_devices.len() != prior_tips.len()
            || prior_devices.len() != prior_tips.len()
            || candidate_devices != prior_devices
        {
            return Err(ImportStagingError::Invalid(
                "verified starting tips must exactly cover the candidate devices".into(),
            ));
        }

        let mut current_tips = prior_tips.to_vec();
        let mut summaries = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            let tip_index = current_tips
                .iter()
                .position(|(device, _)| device == &candidate.device_id)
                .ok_or_else(|| {
                    ImportStagingError::Invalid(
                        "candidate device has no complete verified starting tip".into(),
                    )
                })?;
            let tip = &current_tips[tip_index].1;
            let expected_index = match tip.segment_index {
                Some(index) => index.checked_add(1).ok_or_else(|| {
                    ImportStagingError::Invalid("canonical segment index overflow".into())
                })?,
                None => 0,
            };
            if candidate.segment_index != expected_index {
                return Err(ImportStagingError::Invalid(format!(
                    "candidate segment index {} is not contiguous after verified index {:?}",
                    candidate.segment_index, tip.segment_index
                )));
            }

            let verified =
                self.verify_candidate(candidate, expected_brain_id, tip, key_resolver)?;
            current_tips[tip_index] = (
                candidate.device_id.clone(),
                ImportStreamTip {
                    segment_index: Some(candidate.segment_index),
                    device_seq: Some(verified.device_seq),
                    content_hash: verified.content_hash.clone(),
                },
            );
            summaries.push(verified);
        }

        current_tips
            .sort_by(|left, right| left.0.as_str().as_bytes().cmp(right.0.as_str().as_bytes()));
        Ok(VerifiedStagedBatch {
            segments: summaries,
            end_tips: current_tips,
        })
    }

    /// Verify a staged batch against a complete canonical snapshot while the
    /// canonical inventory and device locks remain held. The importer-only
    /// staging slot must be released first so lock acquisition follows the
    /// global inventory -> device order without a staging-lock inversion.
    pub fn verify_batch_against_canonical<R: TrustedDaemonKeyResolver>(
        &self,
        expected_brain_id: &str,
        trusted_devices: &[DeviceId],
        key_resolver: &mut R,
    ) -> Result<VerifiedStagedBatch, ImportStagingError> {
        if self._single_flight.is_some() {
            return Err(ImportStagingError::Invalid(
                "release the staging slot before acquiring canonical locks".into(),
            ));
        }
        if self.failed || self.candidates.is_empty() {
            return Err(ImportStagingError::Invalid(
                "cannot verify an empty or failed staging batch".into(),
            ));
        }

        let mut affected_devices = self
            .candidates
            .iter()
            .map(|candidate| candidate.device_id.clone())
            .collect::<Vec<_>>();
        affected_devices
            .sort_by(|left, right| left.as_str().as_bytes().cmp(right.as_str().as_bytes()));
        affected_devices.dedup();

        let snapshot = crate::knowledge_materialization::scan_verified_canonical(
            &self.runtime_root,
            expected_brain_id,
            trusted_devices,
            key_resolver,
        )
        .map_err(|error| ImportStagingError::Invalid(error.to_string()))?;
        let prior_tips = snapshot
            .import_starting_tips(&affected_devices)
            .map_err(|error| ImportStagingError::Invalid(error.to_string()))?;
        // `snapshot` stays alive across staged verification, retaining the
        // inventory/device locks that made these starting tips authoritative.
        self.verify_batch(expected_brain_id, &prior_tips, key_resolver)
    }

    /// Verify staged content against the complete canonical trust history
    /// anchored by this device's enrolled signing key. This is a validation
    /// primitive only: the returned batch does not authorize publication and
    /// the caller must still bind `RetrievalSafetyState` and revalidate under
    /// the full publication lock set immediately before any canonical move.
    // Validation is deliberately not wired into the canonical publisher yet.
    // Keep this narrowly marked until the publication/recovery gate consumes it.
    #[allow(dead_code)]
    pub(crate) fn verify_batch_against_local_policy(
        &self,
        expected_brain_id: &str,
        local_anchor: &cryptex_events::LocalTrustAnchor,
    ) -> Result<VerifiedStagedBatch, ImportStagingError> {
        if self._single_flight.is_some() {
            return Err(ImportStagingError::Invalid(
                "release the staging slot before acquiring canonical locks".into(),
            ));
        }
        if self.failed || self.candidates.is_empty() {
            return Err(ImportStagingError::Invalid(
                "cannot verify an empty or failed staging batch".into(),
            ));
        }

        let mut candidate_devices = self
            .candidates
            .iter()
            .map(|candidate| candidate.device_id.clone())
            .collect::<Vec<_>>();
        candidate_devices
            .sort_by(|left, right| left.as_str().as_bytes().cmp(right.as_str().as_bytes()));
        candidate_devices.dedup();
        let snapshot = crate::knowledge_materialization::scan_verified_canonical_from_local_anchor_with_import_devices_until(
            &self.runtime_root,
            expected_brain_id,
            local_anchor,
            &candidate_devices,
            None,
        )
        .map_err(|error| ImportStagingError::Invalid(error.to_string()))?;
        self.verify_batch_against_snapshot(expected_brain_id, &snapshot)
    }

    pub(crate) fn verify_batch_against_snapshot(
        &self,
        expected_brain_id: &str,
        snapshot: &crate::knowledge_materialization::VerifiedCanonicalSnapshot,
    ) -> Result<VerifiedStagedBatch, ImportStagingError> {
        if self._single_flight.is_some() {
            return Err(ImportStagingError::Invalid(
                "release the staging slot before acquiring canonical locks".into(),
            ));
        }
        if self.failed || self.candidates.is_empty() {
            return Err(ImportStagingError::Invalid(
                "cannot verify an empty or failed staging batch".into(),
            ));
        }

        let mut candidate_devices = self
            .candidates
            .iter()
            .map(|candidate| candidate.device_id.clone())
            .collect::<Vec<_>>();
        candidate_devices
            .sort_by(|left, right| left.as_str().as_bytes().cmp(right.as_str().as_bytes()));
        candidate_devices.dedup();
        if !snapshot.locks_devices(&candidate_devices) {
            return Err(ImportStagingError::Invalid(
                "retrieval safety snapshot does not lock every import candidate device".into(),
            ));
        }
        let (verified_history, trust) = snapshot.verified_trust_context().ok_or_else(|| {
            ImportStagingError::Invalid("complete local-anchor trust context is unavailable".into())
        })?;
        let prior_tips = snapshot
            .import_starting_tips_from_trust(&candidate_devices, trust)
            .map_err(|error| ImportStagingError::Invalid(error.to_string()))?;

        // Existing streams must themselves remain within current policy before
        // they can serve as append tips. A newly approved device has a genesis
        // tip and is still constrained below by its effective cutoff.
        for (device, tip) in &prior_tips {
            let cutoff = trust.effective_cutoffs.get(device).ok_or_else(|| {
                ImportStagingError::Invalid("trusted import device has no effective cutoff".into())
            })?;
            if tip.device_seq.is_some_and(|sequence| sequence > *cutoff) {
                return Err(ImportStagingError::Invalid(
                    "canonical import starting tip is beyond the effective trust cutoff".into(),
                ));
            }
        }

        let mut resolver = verified_history.key_resolver();
        let verified = self.verify_batch(expected_brain_id, &prior_tips, &mut resolver)?;
        Ok(verified)
    }

    /// Recheck staged-file identity, canonical aliases, and destination
    /// conflicts while the caller's M3/M4 policy token still owns the complete
    /// canonical inventory/device lock set. This is a read-only preflight; it
    /// does not publish or make the prepared transaction recoverable.
    pub(crate) fn validate_publication_preconditions(
        &self,
        verified: &VerifiedStagedBatch,
    ) -> Result<(), ImportStagingError> {
        if self._single_flight.is_some() || self.failed || !self.prepared {
            return Err(ImportStagingError::Invalid(
                "publication preflight requires a complete, released, prepared staging batch"
                    .into(),
            ));
        }
        if verified.segments.len() != self.candidates.len() {
            return Err(ImportStagingError::Invalid(
                "publication preflight batch does not cover every staged candidate".into(),
            ));
        }

        let manifest_path = self
            .runtime_root
            .join("canonical/import-journal")
            .join(format!("{}.prepared.json", self.transaction_id));
        let manifest = read_prepared_manifest(&manifest_path)?;
        if manifest.transaction_id != self.transaction_id
            || manifest.segments.len() != verified.segments.len()
        {
            return Err(ImportStagingError::RecoveryConflict(
                "prepared manifest does not bind this verified import batch".into(),
            ));
        }
        for summary in &verified.segments {
            let prepared = manifest.segments.iter().find(|segment| {
                segment.device_id == summary.device_id.as_str()
                    && segment.segment_index == summary.segment_index
            });
            let candidate = self.candidates.iter().find(|candidate| {
                candidate.device_id == summary.device_id
                    && candidate.segment_index == summary.segment_index
            });
            let (Some(prepared), Some(candidate)) = (prepared, candidate) else {
                return Err(ImportStagingError::RecoveryConflict(
                    "prepared manifest does not cover a verified import segment".into(),
                ));
            };
            let expected_relative = format!(
                "{}/{:020}.seg",
                candidate.device_id.as_filename_stem(),
                candidate.segment_index
            );
            if prepared.staged_relative_path != expected_relative
                || prepared.destination_relative_path != expected_relative
                || prepared.length != candidate.length
                || prepared.sha256 != summary.sha256
            {
                return Err(ImportStagingError::RecoveryConflict(
                    "prepared manifest segment differs from its verified batch".into(),
                ));
            }
        }
        ensure_manifest_matches_staging_tree(&self.staging_dir, &manifest)?;

        for summary in &verified.segments {
            let candidate = self
                .candidates
                .iter()
                .find(|candidate| {
                    candidate.device_id == summary.device_id
                        && candidate.segment_index == summary.segment_index
                })
                .ok_or_else(|| {
                    ImportStagingError::Invalid(
                        "publication preflight segment does not belong to this transaction".into(),
                    )
                })?;
            let expected_path = self
                .staging_dir
                .join(candidate.device_id.as_filename_stem())
                .join(format!("{:020}.seg", candidate.segment_index));
            if candidate.staged_path != expected_path
                || candidate.length == 0
                || candidate.length > crate::import_limits::MAX_IMPORT_FILE_BYTES
                || candidate.sha256 != summary.sha256
            {
                return Err(ImportStagingError::Invalid(
                    "publication preflight candidate identity differs from verified batch".into(),
                ));
            }

            let parent_handles = inspect_directory_chain(
                expected_path.parent().ok_or_else(|| {
                    ImportStagingError::Invalid("staged segment has no parent".into())
                })?,
                true,
            )?;
            let mut staged = open_source_from_directory_chain(&parent_handles, &expected_path)?;
            let before = source_snapshot(&staged)?;
            if !before.is_regular_file
                || before.link_count != 1
                || before.length != candidate.length
            {
                return Err(ImportStagingError::RecoveryConflict(
                    "staged segment identity changed before publication preflight".into(),
                ));
            }
            staged.seek(SeekFrom::Start(0))?;
            let (length, digest) = hash_bounded(&mut staged, candidate.length)?;
            if length != candidate.length || digest != summary.sha256 {
                return Err(ImportStagingError::RecoveryConflict(
                    "staged segment digest changed before publication preflight".into(),
                ));
            }
            if source_snapshot(&staged)? != before {
                return Err(ImportStagingError::RecoveryConflict(
                    "staged segment changed during publication preflight".into(),
                ));
            }
            if canonical_file_id_alias_exists(&self.runtime_root, before.identity)? {
                return Err(ImportStagingError::RecoveryConflict(
                    "staged segment identity is already present in canonical storage".into(),
                ));
            }
            if source_snapshot(&staged)? != before {
                return Err(ImportStagingError::RecoveryConflict(
                    "staged segment changed during canonical alias scan".into(),
                ));
            }

            let destination = sealed_segment_path(
                &self.runtime_root,
                &candidate.device_id,
                candidate.segment_index,
            );
            let active = active_segment_path(
                &self.runtime_root,
                &candidate.device_id,
                candidate.segment_index,
            );
            if entry_exists(&destination)? || entry_exists(&active)? {
                return Err(ImportStagingError::RecoveryConflict(
                    "canonical import destination or matching active segment already exists".into(),
                ));
            }
            drop(staged);
            drop(parent_handles);
        }
        Ok(())
    }

    /// Publish a prepared batch only while the M3/M4 and local trust/cutoff
    /// token retains the complete canonical lock set. The operation is
    /// additive and no-replace. Any error after the first move leaves the
    /// journal and staging evidence in place for startup's fail-closed hold.
    /// This method is intentionally not wired to transport or IPC until the
    /// Windows durability and full recovery gates are implemented.
    #[allow(dead_code)] // Activated only after the M8 publication/recovery gates.
    pub(crate) fn publish_verified_batch(
        &self,
        policy_verified: &crate::knowledge_materialization::ImportPolicyVerifiedBatch,
    ) -> Result<PathBuf, ImportStagingError> {
        if !policy_verified.keeps_retrieval_snapshot()
            || !policy_verified.keeps_post_import_policy()
        {
            return Err(ImportStagingError::Invalid(
                "canonical publication requires successful M3/M4 and post-import M8 policy tokens"
                    .into(),
            ));
        }
        policy_verified.validate_publication_preconditions(self)?;

        let journal_root = self.runtime_root.join("canonical/import-journal");
        let manifest_path = journal_root.join(format!("{}.prepared.json", self.transaction_id));
        let manifest = read_prepared_manifest(&manifest_path)?;
        let manifest_bytes = serde_json_canonicalizer::to_vec(&manifest).map_err(|error| {
            ImportStagingError::RecoveryConflict(format!(
                "prepared manifest canonicalization failed before publication: {error}"
            ))
        })?;
        let manifest_digest = encode_hex(&Sha256::digest(&manifest_bytes));

        for segment in &manifest.segments {
            let source = self.staging_dir.join(&segment.staged_relative_path);
            let device = DeviceId::new(segment.device_id.clone()).map_err(|error| {
                ImportStagingError::RecoveryConflict(format!(
                    "prepared manifest DeviceId is invalid before publication: {error}"
                ))
            })?;
            let destination = canonical_events_device_dir(&self.runtime_root, &device).join(
                Path::new(&segment.destination_relative_path)
                    .file_name()
                    .ok_or_else(|| {
                        ImportStagingError::RecoveryConflict(
                            "prepared destination has no segment filename".into(),
                        )
                    })?,
            );
            let active = active_segment_path(&self.runtime_root, &device, segment.segment_index);

            if !manifest_file_matches(&source, segment, false)? {
                return Err(ImportStagingError::RecoveryConflict(
                    "staged segment changed immediately before publication".into(),
                ));
            }
            let source_file = open_source_no_follow(&source)?;
            let source_identity = source_snapshot(&source_file)?;
            if !source_identity.is_regular_file
                || source_identity.link_count != 1
                || canonical_file_id_alias_exists(&self.runtime_root, source_identity.identity)?
            {
                return Err(ImportStagingError::RecoveryConflict(
                    "staged segment identity aliases canonical storage before publication".into(),
                ));
            }
            if source_snapshot(&source_file)? != source_identity {
                return Err(ImportStagingError::RecoveryConflict(
                    "staged segment identity changed during publication revalidation".into(),
                ));
            }
            if entry_exists(&destination)? || entry_exists(&active)? {
                return Err(ImportStagingError::RecoveryConflict(
                    "canonical destination or matching active segment appeared before publication"
                        .into(),
                ));
            }
            drop(source_file);

            publish_staged_no_replace(&source, &destination)?;
            sync_staged_parent(&destination)?;
            if !manifest_file_matches(&destination, segment, true)? {
                return Err(ImportStagingError::RecoveryConflict(
                    "published canonical segment failed read-back verification".into(),
                ));
            }
            #[cfg(test)]
            {
                let boundary = format!("m8_after_canonical_segment_move_{}", segment.segment_index);
                crate::test_interrupt::maybe_abrupt_exit(
                    &boundary,
                    &self.runtime_root,
                    "one canonical segment moved and destination parent sync returned",
                );
            }
        }

        let marker = CommittedImportMarker {
            protocol_version: 1,
            transaction_id: self.transaction_id.clone(),
            prepared_manifest_sha256: manifest_digest,
        };
        write_committed_marker(&self.runtime_root, &marker)
    }

    pub fn budget(&self) -> ImportBatchBudget {
        self.budget
    }

    /// Persist the immutable prepared manifest after full staged-batch
    /// verification. This does not publish canonical files. Until prepared
    /// transaction reconciliation is implemented, startup intentionally
    /// stops on this manifest rather than guessing whether publication began.
    pub fn prepare_verified_manifest(
        &mut self,
        verified: &VerifiedStagedBatch,
    ) -> Result<PathBuf, ImportStagingError> {
        if self._single_flight.is_some() {
            return Err(ImportStagingError::Invalid(
                "release the staging slot before preparing the manifest".into(),
            ));
        }
        if self.failed || self.prepared || self.candidates.is_empty() {
            return Err(ImportStagingError::Invalid(
                "cannot prepare an empty, failed, or already prepared transaction".into(),
            ));
        }
        if verified.segments.len() != self.candidates.len() {
            return Err(ImportStagingError::Invalid(
                "verified batch does not cover every staged candidate".into(),
            ));
        }

        let mut segments = Vec::with_capacity(verified.segments.len());
        let mut staged_file_pins = Vec::with_capacity(verified.segments.len());
        for summary in &verified.segments {
            let candidate = self
                .candidates
                .iter()
                .find(|candidate| {
                    candidate.device_id == summary.device_id
                        && candidate.segment_index == summary.segment_index
                })
                .ok_or_else(|| {
                    ImportStagingError::Invalid(
                        "verified segment does not belong to this staging transaction".into(),
                    )
                })?;
            if candidate.length == 0
                || candidate.length > crate::import_limits::MAX_IMPORT_FILE_BYTES
                || candidate.sha256 != summary.sha256
            {
                return Err(ImportStagingError::Invalid(
                    "verified segment metadata does not match the staged snapshot".into(),
                ));
            }
            let expected_path = self
                .staging_dir
                .join(candidate.device_id.as_filename_stem())
                .join(format!("{:020}.seg", candidate.segment_index));
            if candidate.staged_path != expected_path {
                return Err(ImportStagingError::Invalid(
                    "staged path does not match manifest identity".into(),
                ));
            }
            let parent_handles = inspect_directory_chain(
                expected_path.parent().ok_or_else(|| {
                    ImportStagingError::Invalid("staged segment has no parent".into())
                })?,
                true,
            )?;
            let mut staged = open_source_from_directory_chain(&parent_handles, &expected_path)?;
            let initial = source_snapshot(&staged)?;
            if !initial.is_regular_file
                || initial.link_count != 1
                || initial.length != candidate.length
            {
                return Err(ImportStagingError::Invalid(
                    "staged segment changed before manifest preparation".into(),
                ));
            }
            staged.seek(SeekFrom::Start(0))?;
            let (length, digest) = hash_bounded(&mut staged, candidate.length)?;
            if length != candidate.length || digest != summary.sha256 {
                return Err(ImportStagingError::Invalid(
                    "staged segment digest changed before manifest preparation".into(),
                ));
            }
            if source_snapshot(&staged)? != initial {
                return Err(ImportStagingError::Invalid(
                    "staged segment identity or metadata changed during manifest preparation"
                        .into(),
                ));
            }
            let relative = format!(
                "{}/{:020}.seg",
                candidate.device_id.as_filename_stem(),
                candidate.segment_index
            );
            segments.push(PreparedImportSegment {
                device_id: candidate.device_id.as_str().to_owned(),
                segment_index: candidate.segment_index,
                staged_relative_path: relative.clone(),
                destination_relative_path: relative,
                length: candidate.length,
                sha256: candidate.sha256.clone(),
            });
            // Keep the read-only file and parent handles open through the
            // manifest placement. Windows sharing denies concurrent writes;
            // on Unix the daemon-owned staging tree and single-instance
            // protocol exclude supported concurrent writers.
            staged_file_pins.push((parent_handles, staged));
        }
        segments.sort_by(|left, right| {
            left.device_id
                .as_bytes()
                .cmp(right.device_id.as_bytes())
                .then_with(|| left.segment_index.cmp(&right.segment_index))
        });
        let manifest = PreparedImportManifest {
            schema_version: 1,
            transaction_id: self.transaction_id.clone(),
            segments,
        };
        validate_prepared_manifest(&manifest)?;
        let bytes = serde_json_canonicalizer::to_vec(&manifest).map_err(|error| {
            ImportStagingError::Invalid(format!("cannot canonicalize prepared manifest: {error}"))
        })?;
        if bytes.len() as u64 > MAX_PREPARED_MANIFEST_BYTES {
            return Err(ImportStagingError::Invalid(
                "prepared manifest exceeds the schema size bound".into(),
            ));
        }

        let journal_root = self.runtime_root.join("canonical/import-journal");
        ensure_existing_plain_directory(&journal_root)?;
        let final_path = journal_root.join(format!("{}.prepared.json", self.transaction_id));
        let temp_path = journal_root.join(format!(".{}.prepared.tmp", self.transaction_id));
        let write_result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp_path)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            #[cfg(test)]
            crate::test_interrupt::maybe_abrupt_exit(
                "m8_after_prepared_temp_sync",
                &self.runtime_root,
                "prepared manifest temporary file synced before no-replace placement",
            );
            publish_staged_no_replace(&temp_path, &final_path)?;
            sync_staged_parent(&final_path)?;
            #[cfg(test)]
            crate::test_interrupt::maybe_abrupt_exit(
                "m8_after_prepared_manifest_sync",
                &self.runtime_root,
                "prepared manifest placed and parent directory synced",
            );
            Ok::<(), ImportStagingError>(())
        })();
        if let Err(error) = write_result {
            let _ = fs::remove_file(&temp_path);
            return Err(error);
        }
        drop(staged_file_pins);
        self.prepared = true;
        Ok(final_path)
    }

    /// Release the importer-only process lock after the staged snapshot is
    /// complete and before acquiring canonical inventory/device locks. The
    /// unique staging transaction directory remains the durable single-flight
    /// claim: another `begin` call refuses while it exists, and startup may
    /// remove it only as an unprepared transaction.
    pub fn release_staging_slot(&mut self) -> Result<(), ImportStagingError> {
        if self.failed || self.candidates.is_empty() {
            return Err(ImportStagingError::Invalid(
                "cannot release an empty or failed staging transaction".into(),
            ));
        }
        if self._single_flight.take().is_none() {
            return Err(ImportStagingError::Invalid(
                "staging slot was already released".into(),
            ));
        }
        Ok(())
    }
}

fn validate_prepared_manifest(manifest: &PreparedImportManifest) -> Result<(), ImportStagingError> {
    if manifest.schema_version != 1 {
        return Err(ImportStagingError::Invalid(
            "prepared manifest has an unsupported schema version".into(),
        ));
    }
    cryptex_core::validate::parse_uuid_str(&manifest.transaction_id).map_err(|_| {
        ImportStagingError::Invalid("prepared manifest transaction ID is invalid".into())
    })?;
    if manifest
        .transaction_id
        .bytes()
        .any(|byte| byte.is_ascii_uppercase())
    {
        return Err(ImportStagingError::Invalid(
            "prepared manifest transaction ID is not canonical lowercase UUID text".into(),
        ));
    }
    if manifest.segments.is_empty()
        || manifest.segments.len() > crate::import_limits::MAX_IMPORT_FILES_PER_BATCH
    {
        return Err(ImportStagingError::Invalid(
            "prepared manifest segment count is outside the configured bound".into(),
        ));
    }

    let mut total_bytes = 0_u64;
    let mut previous: Option<(&str, u64)> = None;
    for segment in &manifest.segments {
        let device = DeviceId::new(segment.device_id.clone()).map_err(|error| {
            ImportStagingError::Invalid(format!("prepared manifest DeviceId is invalid: {error}"))
        })?;
        if device.as_str() != segment.device_id {
            return Err(ImportStagingError::Invalid(
                "prepared manifest DeviceId is not in canonical form".into(),
            ));
        }
        let expected_relative = format!(
            "{}/{:020}.seg",
            device.as_filename_stem(),
            segment.segment_index
        );
        if segment.staged_relative_path != expected_relative
            || segment.destination_relative_path != expected_relative
        {
            return Err(ImportStagingError::Invalid(
                "prepared manifest paths do not match the DeviceId and segment index".into(),
            ));
        }
        if segment.length == 0 || segment.length > crate::import_limits::MAX_IMPORT_FILE_BYTES {
            return Err(ImportStagingError::Invalid(
                "prepared manifest segment length is outside the configured bound".into(),
            ));
        }
        total_bytes = total_bytes
            .checked_add(segment.length)
            .ok_or_else(|| ImportStagingError::Invalid("manifest byte count overflow".into()))?;
        if total_bytes > crate::import_limits::MAX_IMPORT_BATCH_BYTES {
            return Err(ImportStagingError::Invalid(
                "prepared manifest exceeds the configured batch byte bound".into(),
            ));
        }
        if segment.sha256.len() != 64
            || !segment
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(ImportStagingError::Invalid(
                "prepared manifest SHA-256 is not canonical lowercase hex".into(),
            ));
        }
        if let Some((previous_device, previous_index)) = previous {
            let ordering = previous_device
                .as_bytes()
                .cmp(segment.device_id.as_bytes())
                .then_with(|| previous_index.cmp(&segment.segment_index));
            if ordering != std::cmp::Ordering::Less {
                return Err(ImportStagingError::Invalid(
                    "prepared manifest segments are duplicated or not canonically ordered".into(),
                ));
            }
            if previous_device == segment.device_id
                && previous_index.checked_add(1) != Some(segment.segment_index)
            {
                return Err(ImportStagingError::Invalid(
                    "prepared manifest contains a per-device segment-index gap".into(),
                ));
            }
        }
        previous = Some((&segment.device_id, segment.segment_index));
    }
    Ok(())
}

fn verify_segment_stream<R: TrustedDaemonKeyResolver>(
    staged: &mut File,
    candidate: &StagedSegment,
    expected_brain_id: &str,
    prior_tip: &ImportStreamTip,
    key_resolver: &mut R,
) -> Result<VerifiedStagedSegment, ImportStagingError> {
    let expected_brain_id = cryptex_core::validate::parse_uuid_str(expected_brain_id)
        .map_err(|error| ImportStagingError::Invalid(error.to_string()))?
        .to_ascii_lowercase();
    if prior_tip.device_seq.is_none() && prior_tip.content_hash.as_str() != ZERO_CONTENT_HASH_HEX {
        return Err(ImportStagingError::Invalid(
            "genesis stream tip must use the zero content-hash sentinel".into(),
        ));
    }

    staged.seek(SeekFrom::Start(0))?;
    let mut offset = 0_u64;
    let mut event_count = 0_u64;
    let mut last_seq = prior_tip.device_seq;
    let mut last_hash = prior_tip.content_hash.clone();
    let mut seen_event_ids = std::collections::HashSet::new();
    let mut verified_records = Vec::new();
    let mut digest = Sha256::new();
    let mut header = [0_u8; FRAME_HEADER_BYTES];

    while offset < candidate.length {
        let remaining = candidate.length - offset;
        if remaining < FRAME_HEADER_BYTES as u64 {
            return Err(ImportStagingError::Invalid(
                "staged segment ends with an incomplete frame header".into(),
            ));
        }
        staged.read_exact(&mut header)?;
        digest.update(header);
        let (frame_version, body_len) = decode_frame_header(&header)
            .map_err(|error| ImportStagingError::Invalid(error.to_string()))?;
        if frame_version != FRAME_VERSION_V1 {
            return Err(ImportStagingError::Invalid(
                "staged segment uses an unsupported frame version".into(),
            ));
        }
        let frame_len = (FRAME_HEADER_BYTES as u64)
            .checked_add(u64::from(body_len))
            .ok_or_else(|| ImportStagingError::Invalid("frame length overflow".into()))?;
        if frame_len > remaining {
            return Err(ImportStagingError::Invalid(
                "staged segment ends with an incomplete event frame".into(),
            ));
        }
        let mut body = vec![0_u8; body_len as usize];
        staged.read_exact(&mut body)?;
        digest.update(&body);

        let envelope: serde_json::Value = serde_json::from_slice(&body)
            .map_err(|_| ImportStagingError::Invalid("event frame is not valid JSON".into()))?;
        let device_id = envelope
            .get("device_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                ImportStagingError::Invalid("event frame has no string device_id".into())
            })?;
        if device_id != candidate.device_id.as_str() {
            return Err(ImportStagingError::Invalid(
                "event device_id does not match the staged segment".into(),
            ));
        }
        let brain_id = envelope
            .get("brain_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                ImportStagingError::Invalid("event frame has no string brain_id".into())
            })?;
        if !brain_id.eq_ignore_ascii_case(&expected_brain_id) {
            return Err(ImportStagingError::Invalid(
                "event brain_id does not match the selected Brain".into(),
            ));
        }
        let key_id = envelope
            .get("signing_key_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                ImportStagingError::Invalid("event frame has no string signing_key_id".into())
            })?;
        let verifying_key = key_resolver
            .trusted_daemon_key(device_id, key_id)
            .ok_or_else(|| {
                ImportStagingError::Invalid(
                    "no trusted daemon signing key is bound to the event".into(),
                )
            })?;
        let record = read_versioned_record(&body, &verifying_key)
            .map_err(|error| ImportStagingError::Invalid(error.to_string()))?;
        let (event_id, verified_device_id, verified_brain_id, device_seq, prev_hash, content_hash) =
            match &record {
                VersionedRecord::VerifiedV1(event) => (
                    event.event_id.as_str(),
                    event.device_id.as_str(),
                    event.brain_id.as_str(),
                    event.device_seq.to_u64().map_err(|error| {
                        ImportStagingError::Invalid(format!("invalid event sequence: {error}"))
                    })?,
                    &event.prev_content_hash,
                    &event.content_hash,
                ),
                VersionedRecord::VerifiedV2Supported(event) => {
                    let event = event.event();
                    (
                        event.event_id.as_str(),
                        event.device_id.as_str(),
                        event.brain_id.as_str(),
                        event.device_seq.to_u64().map_err(|error| {
                            ImportStagingError::Invalid(format!("invalid event sequence: {error}"))
                        })?,
                        &event.prev_content_hash,
                        &event.content_hash,
                    )
                }
                VersionedRecord::VerifiedSecurityV3(event) => (
                    event.event_id(),
                    event.device_id().as_str(),
                    event.brain_id(),
                    event.device_seq().to_u64().map_err(|error| {
                        ImportStagingError::Invalid(format!("invalid event sequence: {error}"))
                    })?,
                    event.prev_content_hash(),
                    event.content_hash(),
                ),
                VersionedRecord::VerifiedUnsupported(_) => {
                    return Err(ImportStagingError::Invalid(
                        "event payload schema is not supported for import".into(),
                    ));
                }
                VersionedRecord::Unverifiable(_) => {
                    return Err(ImportStagingError::Invalid(
                        "event envelope version cannot be verified for import".into(),
                    ));
                }
            };
        if verified_device_id != candidate.device_id.as_str()
            || !verified_brain_id.eq_ignore_ascii_case(&expected_brain_id)
        {
            return Err(ImportStagingError::Invalid(
                "verified event identity does not match the staged segment".into(),
            ));
        }
        if !seen_event_ids.insert(event_id.to_ascii_lowercase()) {
            return Err(ImportStagingError::Invalid(
                "staged segment contains a duplicate event_id".into(),
            ));
        }
        if last_seq.is_some_and(|previous| device_seq <= previous) {
            return Err(ImportStagingError::Invalid(
                "event device_seq is not strictly increasing".into(),
            ));
        }
        if prev_hash != &last_hash {
            return Err(ImportStagingError::Invalid(
                "event prev_content_hash does not match the verified stream tip".into(),
            ));
        }
        last_seq = Some(device_seq);
        last_hash.clone_from(content_hash);
        verified_records.push(VerifiedStagedRecord {
            device_id: candidate.device_id.clone(),
            device_seq,
            event_id: event_id.to_owned(),
            security_control: matches!(&record, VersionedRecord::VerifiedSecurityV3(_)),
            content_v1: matches!(&record, VersionedRecord::VerifiedV1(_)),
            bytes: body,
        });
        event_count = event_count
            .checked_add(1)
            .ok_or_else(|| ImportStagingError::Invalid("event count overflow".into()))?;
        offset = offset
            .checked_add(frame_len)
            .ok_or_else(|| ImportStagingError::Invalid("segment offset overflow".into()))?;
    }

    let sha256 = encode_hex(&digest.finalize());
    if event_count == 0 || sha256 != candidate.sha256 {
        return Err(ImportStagingError::Invalid(
            "staged segment is empty or its digest changed before validation".into(),
        ));
    }
    let device_seq = last_seq.expect("non-empty validated segment has a sequence");
    Ok(VerifiedStagedSegment {
        device_id: candidate.device_id.clone(),
        segment_index: candidate.segment_index,
        event_count,
        device_seq,
        content_hash: last_hash,
        sha256,
        records: verified_records,
    })
}

/// Runs under the daemon's single-instance lock before transport, IPC, or
/// canonical readers start. Unprepared staging-only trees are removed. A
/// prepared manifest is aborted only if every staged file still matches and
/// no canonical destination exists; any visible destination remains a typed
/// fail-closed conflict. Recognized committed-marker files are parsed and held
/// for reviewed replay; their digest is bound to the prepared companion and
/// staged/destination files are checked for observation only. Every recognized
/// final or temporary marker remains a typed hold, including a valid final
/// marker whose destinations match. No marker completion, publication, replay,
/// A valid final committed marker is already the immutable commit decision:
/// after its exact companion and every destination are revalidated under the
/// canonical lock set, recovery may remove only that transaction's staging,
/// manifest, and marker. A valid temporary marker is promoted only when every
/// canonical destination matches; missing, partial, or mismatched destinations
/// remain typed holds. Unmarked prepared transactions with visible destinations
/// are classified and held until full policy-bound replay is implemented.
pub fn recover_import_staging(runtime_root: &Path) -> Result<(), ImportStagingError> {
    recover_import_staging_inner(runtime_root, None)
}

/// Run the local policy verifier when a prepared transaction already has one
/// or more visible canonical destinations. Verification is diagnostic only:
/// recovery still holds and preserves the transaction until the reviewed
/// publisher/committed-marker path exists.
pub fn recover_import_staging_with_policy_verifier<F>(
    runtime_root: &Path,
    mut verify_visible: F,
) -> Result<(), ImportStagingError>
where
    F: FnMut(&Path, &Path, &Path) -> Result<(), ImportStagingError>,
{
    recover_import_staging_inner(runtime_root, Some(&mut verify_visible))
}

fn recover_import_staging_inner(
    runtime_root: &Path,
    mut verify_visible: Option<&mut PreparedRecoveryPolicyVerifier<'_>>,
) -> Result<(), ImportStagingError> {
    assert_outside_repository(runtime_root)
        .map_err(|error| ImportStagingError::Invalid(error.to_string()))?;
    let canonical = runtime_root.join("canonical");
    let staging_root = canonical.join("import-staging");
    let journal_root = canonical.join("import-journal");
    let canonical_exists = entry_exists(&canonical)?;
    if !canonical_exists {
        return Ok(());
    }
    ensure_existing_plain_directory(&canonical)?;
    let staging_exists = entry_exists(&staging_root)?;
    let journal_exists = entry_exists(&journal_root)?;
    if !staging_exists && !journal_exists {
        return Ok(());
    }
    if !journal_exists {
        return Err(ImportStagingError::RecoveryConflict(
            "staging exists without its journal directory".into(),
        ));
    }
    ensure_existing_plain_directory(&journal_root)?;

    let mut _directory_handles = Vec::new();
    for directory in [
        runtime_root,
        canonical.as_path(),
        staging_root.as_path(),
        journal_root.as_path(),
    ] {
        if entry_exists(directory)? {
            _directory_handles.extend(inspect_directory_chain(directory, true)?);
        }
    }

    let mut has_lock_entry = false;
    let mut prepared_path = None;
    let mut prepared_temp_path = None;
    let mut committed_path = None;
    let mut committed_temp_path = None;
    for entry in fs::read_dir(&journal_root)? {
        let path = entry?.path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                ImportStagingError::RecoveryConflict(
                    "import journal entry name is not valid UTF-8".into(),
                )
            })?;
        if name == "import.lock" && !has_lock_entry {
            has_lock_entry = true;
        } else if name.ends_with(".prepared.json") && prepared_path.is_none() {
            prepared_path = Some(path);
        } else if prepared_temp_transaction_id(name).is_some() && prepared_temp_path.is_none() {
            prepared_temp_path = Some(path);
        } else if committed_marker_transaction_id(name).is_some() && committed_path.is_none() {
            committed_path = Some(path);
        } else if committed_marker_temp_transaction_id(name).is_some()
            && committed_temp_path.is_none()
        {
            committed_temp_path = Some(path);
        } else {
            return Err(ImportStagingError::RecoveryConflict(
                "unknown or conflicting journal entries require reviewed reconciliation".into(),
            ));
        }
    }
    if committed_path.is_some() && committed_temp_path.is_some() {
        return Err(ImportStagingError::RecoveryConflict(
            "committed marker and marker temporary coexist".into(),
        ));
    }
    if prepared_path.is_some() && prepared_temp_path.is_some() {
        return Err(ImportStagingError::RecoveryConflict(
            "prepared manifest and temporary manifest coexist".into(),
        ));
    }
    if !staging_exists {
        if let Some(path) = committed_path {
            let _marker = read_committed_marker(&path)?;
            return Err(ImportStagingError::RecoveryConflict(
                "committed marker has no staging companion; preserve evidence for reviewed replay"
                    .into(),
            ));
        }
        if let Some(path) = committed_temp_path {
            let _marker = read_committed_marker(&path)?;
            return Err(ImportStagingError::RecoveryConflict(
                "committed marker temporary has no staging companion; preserve evidence".into(),
            ));
        }
        return if prepared_path.is_some() || prepared_temp_path.is_some() {
            Err(ImportStagingError::RecoveryConflict(
                "prepared manifest state has no matching staging directory".into(),
            ))
        } else {
            Ok(())
        };
    }
    if staging_exists {
        ensure_existing_plain_directory(&staging_root)?;
    }
    if !has_lock_entry {
        return Err(ImportStagingError::RecoveryConflict(
            "staging exists without the single-flight lock record".into(),
        ));
    }

    let lock = open_lock_file(&journal_root.join("import.lock")).map_err(|error| {
        ImportStagingError::RecoveryConflict(format!(
            "cannot safely acquire import recovery slot: {error}"
        ))
    })?;
    lock.try_lock_exclusive().map_err(|error| {
        ImportStagingError::RecoveryConflict(format!(
            "another process still owns the import staging slot: {error}"
        ))
    })?;

    if let Some(path) = committed_path {
        // Windows component handles intentionally omit DELETE sharing. The
        // committed cleanup path reopens and revalidates its exact bindings
        // under the canonical locks, so release this earlier read-only walk
        // before removing the transaction entries.
        drop(_directory_handles);
        let marker = read_committed_marker(&path)?;
        let destinations = inspect_committed_marker_companion(runtime_root, &path, &marker)?;
        return if destinations == "all destinations match; transaction cleanup completed" {
            Ok(())
        } else {
            Err(ImportStagingError::RecoveryConflict(format!(
                "committed marker and destinations classified as {destinations}; preserve evidence"
            )))
        };
    }
    if let Some(path) = committed_temp_path {
        // A recovery move changes a journal entry, so release the initial
        // read-only directory walk before reopening and pinning the exact
        // marker/manifest under the canonical locks.
        drop(_directory_handles);
        let marker = read_committed_marker(&path)?;
        let destinations = inspect_committed_marker_companion(runtime_root, &path, &marker)?;
        return if destinations == "all destinations match; transaction cleanup completed" {
            Ok(())
        } else {
            Err(ImportStagingError::RecoveryConflict(format!(
                "committed marker temporary and destinations classified as {destinations}; preserve evidence"
            )))
        };
    }

    let mut transactions = fs::read_dir(&staging_root)?;
    let transaction = transactions.next().transpose()?.map(|entry| entry.path());
    if transactions.next().is_some() {
        return Err(ImportStagingError::RecoveryConflict(
            "staging contains more than one transaction directory".into(),
        ));
    }
    let Some(transaction) = transaction else {
        if prepared_path.is_some() || prepared_temp_path.is_some() {
            return Err(ImportStagingError::RecoveryConflict(
                "prepared manifest state has no matching staging transaction".into(),
            ));
        }
        return Ok(());
    };
    ensure_existing_plain_directory(&transaction)?;
    let transaction_name = transaction
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            ImportStagingError::RecoveryConflict(
                "staging transaction name is not valid UTF-8".into(),
            )
        })?;
    cryptex_core::validate::parse_uuid_str(transaction_name).map_err(|_| {
        ImportStagingError::RecoveryConflict(
            "staging contains an invalid transaction directory name".into(),
        )
    })?;
    if transaction_name
        .bytes()
        .any(|byte| byte.is_ascii_uppercase())
    {
        return Err(ImportStagingError::RecoveryConflict(
            "staging transaction ID is not canonical lowercase UUID text".into(),
        ));
    }
    let transaction_handles = inspect_directory_chain(&transaction, true)?;
    drop(transaction_handles);

    if let Some(prepared_path) = prepared_path {
        let manifest = read_prepared_manifest(&prepared_path)?;
        if manifest.transaction_id != transaction_name {
            return Err(ImportStagingError::RecoveryConflict(
                "prepared manifest does not match the staging transaction".into(),
            ));
        }
        // Do not hold the importer mutex while waiting for canonical locks.
        // The daemon instance lock and the prepared tree/manifest preserve the
        // startup reservation throughout this reconciliation.
        drop(lock);
        recover_prepared_without_publication(
            runtime_root,
            &transaction,
            &prepared_path,
            &manifest,
            &mut verify_visible,
        )?;
    } else if let Some(temp_path) = prepared_temp_path {
        let temp_name = temp_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                ImportStagingError::RecoveryConflict(
                    "temporary prepared manifest name is not valid UTF-8".into(),
                )
            })?;
        let temp_transaction_id = prepared_temp_transaction_id(temp_name).ok_or_else(|| {
            ImportStagingError::RecoveryConflict(
                "temporary prepared manifest name is outside the schema".into(),
            )
        })?;
        if temp_transaction_id != transaction_name {
            return Err(ImportStagingError::RecoveryConflict(
                "temporary prepared manifest does not match the staging transaction".into(),
            ));
        }
        validate_prepared_manifest_temp(&temp_path)?;
        // The final prepared manifest is the boundary before any publication
        // can begin. A leftover temp therefore belongs to an unprepared
        // transaction; remove that temp and bounded staging tree so retrying
        // from the still-present transport source is possible.
        fs::remove_file(&temp_path)?;
        sync_staged_parent(&temp_path)?;
        remove_unprepared_transaction(&transaction)?;
    } else {
        remove_unprepared_transaction(&transaction)?;
    }
    Ok(())
}

fn prepared_temp_transaction_id(name: &str) -> Option<&str> {
    let transaction_id = name.strip_prefix('.')?.strip_suffix(".prepared.tmp")?;
    if transaction_id.is_empty()
        || cryptex_core::validate::parse_uuid_str(transaction_id).is_err()
        || transaction_id.bytes().any(|byte| byte.is_ascii_uppercase())
    {
        return None;
    }
    Some(transaction_id)
}

fn committed_marker_transaction_id(name: &str) -> Option<&str> {
    let transaction_id = name.strip_suffix(".committed.json")?;
    canonical_transaction_id(transaction_id).then_some(transaction_id)
}

fn committed_marker_temp_transaction_id(name: &str) -> Option<&str> {
    let transaction_id = name.strip_prefix('.')?.strip_suffix(".committed.tmp")?;
    canonical_transaction_id(transaction_id).then_some(transaction_id)
}

fn canonical_transaction_id(transaction_id: &str) -> bool {
    !transaction_id.is_empty()
        && cryptex_core::validate::parse_uuid_str(transaction_id).is_ok()
        && !transaction_id.bytes().any(|byte| byte.is_ascii_uppercase())
}

fn validate_prepared_manifest_temp(path: &Path) -> Result<(), ImportStagingError> {
    let metadata = fs::symlink_metadata(path)?;
    if is_link_or_reparse(&metadata)
        || !metadata.is_file()
        || metadata.len() > MAX_PREPARED_MANIFEST_BYTES
    {
        return Err(ImportStagingError::RecoveryConflict(
            "temporary prepared manifest is not an ordinary bounded file".into(),
        ));
    }
    let file = open_source_no_follow(path)?;
    let snapshot = source_snapshot(&file)?;
    let revalidated = open_source_no_follow(path)?;
    let revalidated_snapshot = source_snapshot(&revalidated)?;
    if !snapshot.is_regular_file
        || snapshot.link_count != 1
        || snapshot.length > MAX_PREPARED_MANIFEST_BYTES
        || source_snapshot(&file)? != snapshot
        || revalidated_snapshot != snapshot
    {
        return Err(ImportStagingError::RecoveryConflict(
            "temporary prepared manifest identity is not an ordinary single-link file".into(),
        ));
    }
    Ok(())
}

fn read_prepared_manifest(path: &Path) -> Result<PreparedImportManifest, ImportStagingError> {
    let metadata = fs::symlink_metadata(path)?;
    if is_link_or_reparse(&metadata)
        || !metadata.is_file()
        || metadata.len() > MAX_PREPARED_MANIFEST_BYTES
    {
        return Err(ImportStagingError::RecoveryConflict(
            "prepared manifest is not an ordinary bounded file".into(),
        ));
    }
    let mut file = open_source_no_follow(path)?;
    let before = source_snapshot(&file)?;
    if !before.is_regular_file || before.link_count != 1 {
        return Err(ImportStagingError::RecoveryConflict(
            "prepared manifest identity is not an ordinary single-link file".into(),
        ));
    }
    let mut bytes = Vec::with_capacity(before.length as usize);
    (&mut file)
        .take(MAX_PREPARED_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 != before.length
        || bytes.len() as u64 > MAX_PREPARED_MANIFEST_BYTES
        || source_snapshot(&file)? != before
    {
        return Err(ImportStagingError::RecoveryConflict(
            "prepared manifest changed while being read".into(),
        ));
    }
    let manifest: PreparedImportManifest = serde_json::from_slice(&bytes).map_err(|error| {
        ImportStagingError::RecoveryConflict(format!("prepared manifest is invalid: {error}"))
    })?;
    validate_prepared_manifest(&manifest).map_err(|error| {
        ImportStagingError::RecoveryConflict(format!("prepared manifest rejected: {error}"))
    })?;
    let canonical = serde_json_canonicalizer::to_vec(&manifest).map_err(|error| {
        ImportStagingError::RecoveryConflict(format!(
            "prepared manifest canonicalization failed: {error}"
        ))
    })?;
    if canonical != bytes {
        return Err(ImportStagingError::RecoveryConflict(
            "prepared manifest is not in canonical JSON form".into(),
        ));
    }
    let expected_name = format!("{}.prepared.json", manifest.transaction_id);
    if path.file_name().and_then(|name| name.to_str()) != Some(expected_name.as_str()) {
        return Err(ImportStagingError::RecoveryConflict(
            "prepared manifest filename does not match its transaction ID".into(),
        ));
    }
    Ok(manifest)
}

fn read_committed_marker(path: &Path) -> Result<CommittedImportMarker, ImportStagingError> {
    let metadata = fs::symlink_metadata(path)?;
    if is_link_or_reparse(&metadata)
        || !metadata.is_file()
        || metadata.len() > MAX_COMMITTED_MARKER_BYTES
    {
        return Err(ImportStagingError::RecoveryConflict(
            "committed marker is not an ordinary bounded file".into(),
        ));
    }
    let mut file = open_source_no_follow(path)?;
    let before = source_snapshot(&file)?;
    if !before.is_regular_file || before.link_count != 1 {
        return Err(ImportStagingError::RecoveryConflict(
            "committed marker is not an ordinary single-link file".into(),
        ));
    }
    let mut bytes = Vec::with_capacity(before.length as usize);
    (&mut file)
        .take(MAX_COMMITTED_MARKER_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 != before.length
        || bytes.len() as u64 > MAX_COMMITTED_MARKER_BYTES
        || source_snapshot(&file)? != before
    {
        return Err(ImportStagingError::RecoveryConflict(
            "committed marker changed while being read".into(),
        ));
    }

    let marker: CommittedImportMarker = serde_json::from_slice(&bytes).map_err(|error| {
        ImportStagingError::RecoveryConflict(format!("committed marker is invalid: {error}"))
    })?;
    validate_committed_marker(&marker).map_err(|error| {
        ImportStagingError::RecoveryConflict(format!("committed marker rejected: {error}"))
    })?;
    let canonical = serde_json_canonicalizer::to_vec(&marker).map_err(|error| {
        ImportStagingError::RecoveryConflict(format!(
            "committed marker canonicalization failed: {error}"
        ))
    })?;
    if canonical != bytes {
        return Err(ImportStagingError::RecoveryConflict(
            "committed marker is not in canonical JSON form".into(),
        ));
    }
    let file_name = path.file_name().and_then(|name| name.to_str());
    let expected_final_name = format!("{}.committed.json", marker.transaction_id);
    let expected_temp_name = format!(".{}.committed.tmp", marker.transaction_id);
    if file_name != Some(expected_final_name.as_str())
        && file_name != Some(expected_temp_name.as_str())
    {
        return Err(ImportStagingError::RecoveryConflict(
            "committed marker filename does not match its transaction ID".into(),
        ));
    }
    Ok(marker)
}

fn write_committed_marker(
    runtime_root: &Path,
    marker: &CommittedImportMarker,
) -> Result<PathBuf, ImportStagingError> {
    validate_committed_marker(marker).map_err(|error| {
        ImportStagingError::Invalid(format!("refusing invalid committed marker: {error}"))
    })?;
    let bytes = serde_json_canonicalizer::to_vec(marker).map_err(|error| {
        ImportStagingError::Invalid(format!("cannot canonicalize committed marker: {error}"))
    })?;
    if bytes.len() as u64 > MAX_COMMITTED_MARKER_BYTES {
        return Err(ImportStagingError::Invalid(
            "committed marker exceeds the schema size bound".into(),
        ));
    }

    let journal_root = runtime_root.join("canonical/import-journal");
    ensure_existing_plain_directory(&journal_root)?;
    let final_path = journal_root.join(format!("{}.committed.json", marker.transaction_id));
    let temp_path = journal_root.join(format!(".{}.committed.tmp", marker.transaction_id));
    if entry_exists(&final_path)? || entry_exists(&temp_path)? {
        return Err(ImportStagingError::RecoveryConflict(
            "committed marker destination or temporary already exists".into(),
        ));
    }

    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    #[cfg(test)]
    crate::test_interrupt::maybe_abrupt_exit(
        "m8_after_committed_marker_temp_sync",
        runtime_root,
        "committed marker temporary synced before no-replace placement",
    );

    publish_staged_no_replace(&temp_path, &final_path)?;
    sync_staged_parent(&final_path)?;
    let read_back = read_committed_marker(&final_path)?;
    if &read_back != marker {
        return Err(ImportStagingError::RecoveryConflict(
            "committed marker read-back differs from the publication decision".into(),
        ));
    }
    #[cfg(test)]
    crate::test_interrupt::maybe_abrupt_exit(
        "m8_after_committed_marker_final_sync",
        runtime_root,
        "committed marker placed, parent sync returned, and final marker read back",
    );
    Ok(final_path)
}

fn validate_committed_marker(marker: &CommittedImportMarker) -> Result<(), &'static str> {
    if marker.protocol_version != 1 {
        return Err("unsupported protocol version");
    }
    cryptex_core::validate::parse_uuid_str(&marker.transaction_id)
        .map_err(|_| "transaction ID is not a canonical UUID")?;
    if marker
        .transaction_id
        .bytes()
        .any(|byte| byte.is_ascii_uppercase())
    {
        return Err("transaction ID must use lowercase UUID text");
    }
    if marker.prepared_manifest_sha256.len() != 64
        || !marker
            .prepared_manifest_sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("prepared manifest digest must be 64 lowercase hexadecimal characters");
    }
    Ok(())
}

/// Validates a committed marker's companion and destinations while holding
/// canonical inventory/device locks. A fully matching final marker permits
/// cleanup; a fully matching temporary marker may first be moved to its final
/// name with no-replace semantics, parent sync, and read-back. Any absent or
/// mismatched destination remains a typed hold. Neither path changes canonical
/// files or re-admits trust.
fn inspect_committed_marker_companion(
    runtime_root: &Path,
    marker_path: &Path,
    marker: &CommittedImportMarker,
) -> Result<&'static str, ImportStagingError> {
    let transaction = runtime_root
        .join("canonical/import-staging")
        .join(&marker.transaction_id);
    let manifest_path = runtime_root
        .join("canonical/import-journal")
        .join(format!("{}.prepared.json", marker.transaction_id));
    let staging_root = transaction.parent().ok_or_else(|| {
        ImportStagingError::RecoveryConflict(
            "committed marker staging path has no parent directory".into(),
        )
    })?;
    let mut staged_transactions = fs::read_dir(staging_root)?;
    let only_transaction = staged_transactions
        .next()
        .transpose()?
        .map(|entry| entry.path());
    if staged_transactions.next().is_some()
        || only_transaction.as_deref() != Some(transaction.as_path())
    {
        return Err(ImportStagingError::RecoveryConflict(
            "committed marker does not have exactly one matching staging transaction".into(),
        ));
    }
    if !entry_exists(&transaction)? || !entry_exists(&manifest_path)? {
        return Err(ImportStagingError::RecoveryConflict(
            "committed marker is missing its prepared manifest or staging companion".into(),
        ));
    }

    let manifest = read_prepared_manifest(&manifest_path)?;
    if manifest.transaction_id != marker.transaction_id {
        return Err(ImportStagingError::RecoveryConflict(
            "committed marker transaction does not match its prepared companion".into(),
        ));
    }
    let manifest_bytes = serde_json_canonicalizer::to_vec(&manifest).map_err(|error| {
        ImportStagingError::RecoveryConflict(format!(
            "committed marker companion canonicalization failed: {error}"
        ))
    })?;
    let manifest_digest = encode_hex(&Sha256::digest(&manifest_bytes));
    if manifest_digest != marker.prepared_manifest_sha256 {
        return Err(ImportStagingError::RecoveryConflict(
            "committed marker digest does not match its prepared companion".into(),
        ));
    }
    ensure_manifest_staging_subset(&transaction, &manifest).map_err(|error| {
        ImportStagingError::RecoveryConflict(format!(
            "committed marker staging companion is unverifiable: {error}"
        ))
    })?;

    let inventory_lock = acquire_canonical_inventory_lock(runtime_root).map_err(|error| {
        ImportStagingError::RecoveryConflict(format!(
            "cannot acquire canonical inventory lock for committed-marker inspection: {error}"
        ))
    })?;
    let mut devices = manifest
        .segments
        .iter()
        .map(|segment| {
            DeviceId::new(segment.device_id.clone()).map_err(|error| {
                ImportStagingError::RecoveryConflict(format!(
                    "prepared manifest DeviceId is invalid: {error}"
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    devices.sort_by(|left, right| left.as_str().as_bytes().cmp(right.as_str().as_bytes()));
    devices.dedup();
    let mut device_locks = Vec::with_capacity(devices.len());
    for device in &devices {
        if let Some(lock) =
            acquire_existing_device_canonical_lock(runtime_root, device).map_err(|error| {
                ImportStagingError::RecoveryConflict(format!(
                    "cannot acquire canonical device lock for committed-marker inspection: {error}"
                ))
            })?
        {
            device_locks.push(lock);
        }
    }

    // The marker and companion are untrusted filesystem state. Rebind both to
    // the bytes that selected this transaction while the canonical lock set
    // is held. Inspection currently only returns a typed hold, but keeping the
    // read boundary here prevents future replay/cleanup code from relying on
    // the earlier unlocked snapshot.
    let locked_manifest =
        read_bound_committed_marker_companion(marker_path, marker, &manifest_path, &manifest)?;
    ensure_manifest_staging_subset(&transaction, &locked_manifest).map_err(|error| {
        ImportStagingError::RecoveryConflict(format!(
            "committed marker staging companion changed under canonical locks: {error}"
        ))
    })?;

    let mut visible = 0usize;
    for segment in &locked_manifest.segments {
        let device = DeviceId::new(segment.device_id.clone()).map_err(|error| {
            ImportStagingError::RecoveryConflict(format!(
                "prepared manifest DeviceId is invalid: {error}"
            ))
        })?;
        let staged_path = transaction.join(&segment.staged_relative_path);
        let staged_matches = manifest_file_matches(&staged_path, segment, true)?;
        let destination_path = canonical_events_device_dir(runtime_root, &device).join(
            Path::new(&segment.destination_relative_path)
                .file_name()
                .ok_or_else(|| {
                    ImportStagingError::RecoveryConflict(
                        "prepared destination path has no file name".into(),
                    )
                })?,
        );
        let destination_matches = manifest_file_matches(&destination_path, segment, true)?;
        if !staged_matches && !destination_matches {
            return Err(ImportStagingError::RecoveryConflict(
                "committed marker companion has neither a staged nor matching destination file"
                    .into(),
            ));
        }
        if destination_matches {
            visible += 1;
        } else if entry_exists(&destination_path)? {
            return Err(ImportStagingError::RecoveryConflict(
                "committed marker destination is visible but does not match its manifest".into(),
            ));
        }
    }

    if visible == locked_manifest.segments.len() {
        let final_marker_path = runtime_root
            .join("canonical/import-journal")
            .join(format!("{}.committed.json", marker.transaction_id));
        let cleanup_marker_path = if marker_path == final_marker_path.as_path() {
            marker_path.to_path_buf()
        } else {
            if entry_exists(&final_marker_path)? {
                return Err(ImportStagingError::RecoveryConflict(
                    "final committed marker appeared beside its temporary; preserve evidence"
                        .into(),
                ));
            }
            if read_committed_marker(marker_path)? != *marker {
                return Err(ImportStagingError::RecoveryConflict(
                    "committed marker temporary changed before completion; preserve evidence"
                        .into(),
                ));
            }
            publish_staged_no_replace(marker_path, &final_marker_path).map_err(|error| {
                ImportStagingError::RecoveryConflict(format!(
                    "cannot complete committed marker temporary without replacement: {error}"
                ))
            })?;
            #[cfg(test)]
            crate::test_interrupt::maybe_abrupt_exit(
                "m8_after_recovery_committed_marker_temp_move",
                runtime_root,
                "recovery moved the committed marker temp without replacement before parent sync",
            );
            sync_staged_parent(&final_marker_path).map_err(|error| {
                ImportStagingError::RecoveryConflict(format!(
                    "completed committed marker parent sync failed: {error}"
                ))
            })?;
            if read_committed_marker(&final_marker_path)? != *marker {
                return Err(ImportStagingError::RecoveryConflict(
                    "completed final marker read-back differs; preserve evidence".into(),
                ));
            }
            #[cfg(test)]
            crate::test_interrupt::maybe_abrupt_exit(
                "m8_after_recovery_committed_marker_final_sync",
                runtime_root,
                "recovery synced and read back the final marker before transaction cleanup",
            );
            final_marker_path
        };

        // A verified final marker is the commit decision. A verified temp is
        // promoted only after every destination matches. Remove transaction
        // evidence while the complete canonical lock set and import-journal
        // lock remain held; remove the marker last so interruption fails closed.
        remove_unprepared_transaction(&transaction).map_err(|error| {
            ImportStagingError::RecoveryConflict(format!(
                "committed transaction staging cleanup failed; preserve remaining evidence: {error}"
            ))
        })?;
        sync_staged_parent(&transaction).map_err(|error| {
            ImportStagingError::RecoveryConflict(format!(
                "committed transaction staging cleanup sync failed: {error}"
            ))
        })?;
        fs::remove_file(&manifest_path).map_err(|error| {
            ImportStagingError::RecoveryConflict(format!(
                "committed transaction manifest cleanup failed: {error}"
            ))
        })?;
        sync_staged_parent(&manifest_path).map_err(|error| {
            ImportStagingError::RecoveryConflict(format!(
                "committed transaction manifest cleanup sync failed: {error}"
            ))
        })?;
        if read_committed_marker(&cleanup_marker_path)? != *marker {
            return Err(ImportStagingError::RecoveryConflict(
                "committed marker changed before final cleanup; preserve evidence".into(),
            ));
        }
        fs::remove_file(&cleanup_marker_path).map_err(|error| {
            ImportStagingError::RecoveryConflict(format!(
                "committed marker cleanup failed: {error}"
            ))
        })?;
        sync_staged_parent(&cleanup_marker_path).map_err(|error| {
            ImportStagingError::RecoveryConflict(format!(
                "committed marker cleanup sync failed: {error}"
            ))
        })?;
        return Ok("all destinations match; transaction cleanup completed");
    }

    drop(device_locks);
    drop(inventory_lock);
    Ok(if visible == 0 {
        "all destinations absent"
    } else {
        "matching destination subset visible"
    })
}

fn read_bound_committed_marker_companion(
    marker_path: &Path,
    expected_marker: &CommittedImportMarker,
    manifest_path: &Path,
    expected_manifest: &PreparedImportManifest,
) -> Result<PreparedImportManifest, ImportStagingError> {
    let locked_marker = read_committed_marker(marker_path)?;
    if &locked_marker != expected_marker {
        return Err(ImportStagingError::RecoveryConflict(
            "committed marker changed while acquiring canonical locks".into(),
        ));
    }
    let locked_manifest = read_prepared_manifest(manifest_path)?;
    let locked_manifest_bytes =
        serde_json_canonicalizer::to_vec(&locked_manifest).map_err(|error| {
            ImportStagingError::RecoveryConflict(format!(
                "committed marker companion canonicalization failed under canonical locks: {error}"
            ))
        })?;
    if &locked_manifest != expected_manifest
        || encode_hex(&Sha256::digest(&locked_manifest_bytes))
            != expected_marker.prepared_manifest_sha256
    {
        return Err(ImportStagingError::RecoveryConflict(
            "committed marker companion changed while acquiring canonical locks".into(),
        ));
    }
    Ok(locked_manifest)
}

/// Returns true for an exact manifest match, false for an absent optional
/// destination, and fails closed for invalid identity or changed contents.
fn manifest_file_matches(
    path: &Path,
    segment: &PreparedImportSegment,
    missing_is_absent: bool,
) -> Result<bool, ImportStagingError> {
    let parent = path.parent().ok_or_else(|| {
        ImportStagingError::RecoveryConflict("manifest file has no parent directory".into())
    })?;
    if !entry_exists(parent)? {
        return if missing_is_absent {
            Ok(false)
        } else {
            Err(ImportStagingError::RecoveryConflict(
                "manifest staged-file parent is missing".into(),
            ))
        };
    }
    let parent_handles = inspect_directory_chain(parent, true)?;
    if !entry_exists(path)? {
        return if missing_is_absent {
            Ok(false)
        } else {
            Err(ImportStagingError::RecoveryConflict(
                "manifest staged file is missing".into(),
            ))
        };
    }
    let mut file = open_source_from_directory_chain(&parent_handles, path)?;
    let before = source_snapshot(&file)?;
    if !before.is_regular_file || before.link_count != 1 || before.length != segment.length {
        return Err(ImportStagingError::RecoveryConflict(
            "manifest file identity or length is invalid".into(),
        ));
    }
    file.seek(SeekFrom::Start(0))?;
    let (length, digest) = hash_bounded(&mut file, segment.length)?;
    if length != segment.length || digest != segment.sha256 || source_snapshot(&file)? != before {
        return Err(ImportStagingError::RecoveryConflict(
            "manifest file digest or identity changed".into(),
        ));
    }
    Ok(true)
}

fn ensure_manifest_matches_staging_tree(
    transaction: &Path,
    manifest: &PreparedImportManifest,
) -> Result<(), ImportStagingError> {
    let expected = manifest
        .segments
        .iter()
        .map(|segment| segment.staged_relative_path.as_str())
        .collect::<std::collections::HashSet<_>>();
    let mut actual = std::collections::HashSet::new();
    for device_entry in fs::read_dir(transaction)? {
        let device_entry = device_entry?;
        let device_path = device_entry.path();
        let metadata = fs::symlink_metadata(&device_path)?;
        if is_link_or_reparse(&metadata) || !metadata.is_dir() {
            return Err(ImportStagingError::RecoveryConflict(
                "prepared staging tree contains a link or non-directory".into(),
            ));
        }
        let device_name = device_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                ImportStagingError::RecoveryConflict(
                    "prepared staging device directory is not valid Unicode".into(),
                )
            })?;
        let mut device_file_count = 0_usize;
        for file_entry in fs::read_dir(&device_path)? {
            let file_entry = file_entry?;
            let file_path = file_entry.path();
            let metadata = fs::symlink_metadata(&file_path)?;
            if is_link_or_reparse(&metadata) || !metadata.is_file() {
                return Err(ImportStagingError::RecoveryConflict(
                    "prepared staging tree contains a link or non-file".into(),
                ));
            }
            validate_staging_leaf_name(&file_path)?;
            let file_name = file_path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| {
                    ImportStagingError::RecoveryConflict(
                        "prepared staging filename is not valid Unicode".into(),
                    )
                })?;
            let relative = format!("{device_name}/{file_name}");
            if !expected.contains(relative.as_str()) || !actual.insert(relative) {
                return Err(ImportStagingError::RecoveryConflict(
                    "prepared staging tree contains an unmanifested or duplicate file".into(),
                ));
            }
            device_file_count += 1;
        }
        if device_file_count == 0 {
            return Err(ImportStagingError::RecoveryConflict(
                "prepared staging tree contains an empty device directory".into(),
            ));
        }
    }
    if actual.len() != expected.len() {
        return Err(ImportStagingError::RecoveryConflict(
            "prepared manifest does not cover the complete staging tree".into(),
        ));
    }
    Ok(())
}

/// Validate the transaction tree after no-replace moves may have consumed a
/// subset (or all) of its staged files. Missing staged entries are accepted
/// only if the caller separately proves their canonical destinations match
/// the same prepared manifest while holding canonical locks.
fn ensure_manifest_staging_subset(
    transaction: &Path,
    manifest: &PreparedImportManifest,
) -> Result<(), ImportStagingError> {
    let expected = manifest
        .segments
        .iter()
        .map(|segment| segment.staged_relative_path.as_str())
        .collect::<std::collections::HashSet<_>>();
    let expected_devices = manifest
        .segments
        .iter()
        .filter_map(|segment| {
            segment
                .staged_relative_path
                .split_once('/')
                .map(|(device, _)| device)
        })
        .collect::<std::collections::HashSet<_>>();
    let mut actual = std::collections::HashSet::new();
    for device_entry in fs::read_dir(transaction)? {
        let device_entry = device_entry?;
        let device_path = device_entry.path();
        let metadata = fs::symlink_metadata(&device_path)?;
        if is_link_or_reparse(&metadata) || !metadata.is_dir() {
            return Err(ImportStagingError::RecoveryConflict(
                "import staging companion contains a link or non-directory".into(),
            ));
        }
        let device_name = device_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                ImportStagingError::RecoveryConflict(
                    "import staging device directory is not valid Unicode".into(),
                )
            })?;
        if !expected_devices.contains(device_name) {
            return Err(ImportStagingError::RecoveryConflict(
                "import staging tree contains an unmanifested device directory".into(),
            ));
        }
        for file_entry in fs::read_dir(&device_path)? {
            let file_entry = file_entry?;
            let file_path = file_entry.path();
            let metadata = fs::symlink_metadata(&file_path)?;
            if is_link_or_reparse(&metadata) || !metadata.is_file() {
                return Err(ImportStagingError::RecoveryConflict(
                    "import staging tree contains a link or non-file".into(),
                ));
            }
            validate_staging_leaf_name(&file_path)?;
            let file_name = file_path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| {
                    ImportStagingError::RecoveryConflict(
                        "import staging filename is not valid Unicode".into(),
                    )
                })?;
            let relative = format!("{device_name}/{file_name}");
            if !expected.contains(relative.as_str()) || !actual.insert(relative) {
                return Err(ImportStagingError::RecoveryConflict(
                    "import staging tree contains an unmanifested or duplicate file".into(),
                ));
            }
        }
    }
    Ok(())
}

fn recover_prepared_without_publication(
    runtime_root: &Path,
    transaction: &Path,
    manifest_path: &Path,
    manifest: &PreparedImportManifest,
    verify_visible: &mut Option<&mut PreparedRecoveryPolicyVerifier<'_>>,
) -> Result<(), ImportStagingError> {
    let inventory_lock = acquire_canonical_inventory_lock(runtime_root).map_err(|error| {
        ImportStagingError::RecoveryConflict(format!(
            "cannot acquire canonical inventory lock for prepared recovery: {error}"
        ))
    })?;
    let mut devices = manifest
        .segments
        .iter()
        .map(|segment| {
            DeviceId::new(segment.device_id.clone()).map_err(|error| {
                ImportStagingError::RecoveryConflict(format!(
                    "prepared manifest DeviceId is invalid: {error}"
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    devices.sort_by(|left, right| left.as_str().as_bytes().cmp(right.as_str().as_bytes()));
    devices.dedup();
    let mut device_locks = Vec::with_capacity(devices.len());
    for device in &devices {
        if let Some(lock) =
            acquire_existing_device_canonical_lock(runtime_root, device).map_err(|error| {
                ImportStagingError::RecoveryConflict(format!(
                    "cannot acquire canonical device lock for prepared recovery: {error}"
                ))
            })?
        {
            device_locks.push(lock);
        }
    }

    ensure_manifest_staging_subset(transaction, manifest).map_err(|error| {
        ImportStagingError::RecoveryConflict(format!(
            "prepared staging-tree verification failed: {error}"
        ))
    })?;

    let mut visible_destinations = 0usize;
    for segment in &manifest.segments {
        let device = DeviceId::new(segment.device_id.clone()).map_err(|error| {
            ImportStagingError::RecoveryConflict(format!(
                "prepared manifest DeviceId is invalid: {error}"
            ))
        })?;
        let staged_path = transaction.join(&segment.staged_relative_path);
        let destination_path = canonical_events_device_dir(runtime_root, &device).join(
            Path::new(&segment.destination_relative_path)
                .file_name()
                .ok_or_else(|| {
                    ImportStagingError::RecoveryConflict(
                        "prepared destination has no segment filename".into(),
                    )
                })?,
        );
        let staged_matches = manifest_file_matches(&staged_path, segment, true)?;
        let destination_matches = manifest_file_matches(&destination_path, segment, true)?;
        let staged_exists = entry_exists(&staged_path)?;
        let destination_exists = entry_exists(&destination_path)?;
        if staged_exists != staged_matches || destination_exists != destination_matches {
            return Err(ImportStagingError::RecoveryConflict(
                "prepared staged or canonical file does not match its manifest".into(),
            ));
        }
        if staged_exists && destination_exists {
            return Err(ImportStagingError::RecoveryConflict(
                "prepared segment exists in both staging and canonical destinations".into(),
            ));
        }
        if !staged_exists && !destination_exists {
            return Err(ImportStagingError::RecoveryConflict(
                "prepared segment is missing from both staging and canonical destinations".into(),
            ));
        }
        if destination_exists {
            visible_destinations += 1;
        } else {
            let parent = staged_path.parent().ok_or_else(|| {
                ImportStagingError::RecoveryConflict("prepared staged path has no parent".into())
            })?;
            let handles = inspect_directory_chain(parent, true).map_err(|error| {
                ImportStagingError::RecoveryConflict(format!(
                    "prepared staged-path inspection failed: {error}"
                ))
            })?;
            let staged =
                open_source_from_directory_chain(&handles, &staged_path).map_err(|error| {
                    ImportStagingError::RecoveryConflict(format!(
                        "prepared staged-file open failed: {error}"
                    ))
                })?;
            let identity = source_snapshot(&staged).map_err(|error| {
                ImportStagingError::RecoveryConflict(format!(
                    "prepared staged-file metadata read failed: {error}"
                ))
            })?;
            if canonical_file_id_alias_exists(runtime_root, identity.identity).map_err(|error| {
                ImportStagingError::RecoveryConflict(format!(
                    "prepared canonical alias scan failed: {error}"
                ))
            })? {
                return Err(ImportStagingError::RecoveryConflict(
                    "prepared staged file aliases an entry under canonical/events".into(),
                ));
            }
        }
    }

    if visible_destinations != 0 {
        let classification = if visible_destinations == manifest.segments.len() {
            "all canonical destinations match"
        } else {
            "matching canonical destination subset is visible"
        };
        if let Some(verify) = verify_visible.as_deref_mut() {
            // The policy verifier takes the same canonical lock set itself and
            // rebuilds its exact snapshot. Release this classification pass's
            // locks first; the daemon instance lock remains held by startup.
            drop(device_locks);
            drop(inventory_lock);
            verify(runtime_root, transaction, manifest_path)?;
            return Err(ImportStagingError::RecoveryConflict(format!(
                "prepared transaction policy verified as {classification}; publication and cleanup remain disabled"
            )));
        }
        return Err(ImportStagingError::RecoveryConflict(format!(
            "prepared transaction classified as {classification}; policy-bound replay is not implemented"
        )));
    }

    // No publication move exists in this implementation slice. With all
    // destinations absent and every staged digest intact, aborting this
    // prepared transaction is safe and repeatable after interruption.
    fs::remove_file(manifest_path).map_err(|error| {
        ImportStagingError::Io(format!("remove prepared import manifest: {error}"))
    })?;
    sync_staged_parent(manifest_path).map_err(|error| {
        ImportStagingError::Io(format!("sync prepared import journal parent: {error}"))
    })?;
    drop(device_locks);
    drop(inventory_lock);
    remove_unprepared_transaction(transaction).map_err(|error| {
        ImportStagingError::Io(format!("remove prepared staging transaction: {error}"))
    })?;
    Ok(())
}

/// Validate every member of a prepared transaction from exactly one of its
/// staged or canonical locations, against the current lock-bearing M3/M4 and
/// local trust/cutoff snapshot. The returned token keeps the canonical locks
/// alive, but this function does not move files or write a committed marker.
#[allow(dead_code)] // Startup recovery will consume this before enabling replay.
fn verify_prepared_recovery_batch<R: TrustedDaemonKeyResolver>(
    runtime_root: &Path,
    transaction: &Path,
    manifest: &PreparedImportManifest,
    expected_brain_id: &str,
    key_resolver: &mut R,
    db: &cryptex_projection::ProjectionDb,
) -> Result<crate::knowledge_materialization::ImportPolicyVerifiedBatch, ImportStagingError> {
    validate_prepared_manifest(manifest)?;
    if transaction.file_name().and_then(|name| name.to_str())
        != Some(manifest.transaction_id.as_str())
    {
        return Err(ImportStagingError::RecoveryConflict(
            "prepared recovery manifest does not match its transaction directory".into(),
        ));
    }
    let mut devices = manifest
        .segments
        .iter()
        .map(|segment| {
            DeviceId::new(segment.device_id.clone()).map_err(|error| {
                ImportStagingError::RecoveryConflict(format!(
                    "prepared manifest DeviceId is invalid for policy recovery: {error}"
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    devices.sort_by(|left, right| left.as_str().as_bytes().cmp(right.as_str().as_bytes()));
    devices.dedup();
    let exclusions =
        manifest
            .segments
            .iter()
            .map(|segment| {
                Ok(crate::knowledge_materialization::ImportRecoverySegmentExclusion {
                device_id: DeviceId::new(segment.device_id.clone()).map_err(|error| {
                    ImportStagingError::RecoveryConflict(format!(
                        "prepared manifest DeviceId is invalid for recovery exclusion: {error}"
                    ))
                })?,
                segment_index: segment.segment_index,
                length: segment.length,
                sha256: segment.sha256.clone(),
            })
            })
            .collect::<Result<Vec<_>, ImportStagingError>>()?;
    let safety = crate::knowledge_materialization::evaluate_import_recovery_retrieval_safety(
        runtime_root,
        expected_brain_id,
        &devices,
        &exclusions,
        key_resolver,
        db,
    );

    safety.verify_import_recovery_batch(&devices, |snapshot| {
        let (history, trust) = snapshot.verified_trust_context().ok_or_else(|| {
            ImportStagingError::RecoveryConflict(
                "prepared recovery lacks verified local trust history".into(),
            )
        })?;
        let prior_tips = snapshot
            .import_starting_tips_from_trust(&devices, trust)
            .map_err(|error| ImportStagingError::RecoveryConflict(error.to_string()))?;
        let mut tips = prior_tips.into_iter().collect::<std::collections::BTreeMap<_, _>>();
        for (device, tip) in &tips {
            let cutoff = trust.effective_cutoffs.get(device).ok_or_else(|| {
                ImportStagingError::RecoveryConflict(
                    "trusted recovery device has no effective cutoff".into(),
                )
            })?;
            if tip.device_seq.is_some_and(|sequence| sequence > *cutoff) {
                return Err(ImportStagingError::RecoveryConflict(
                    "canonical recovery starting tip exceeds the effective trust cutoff".into(),
                ));
            }
        }

        let mut resolver = history.key_resolver();
        let mut verified_segments = Vec::with_capacity(manifest.segments.len());
        for segment in &manifest.segments {
            let device = DeviceId::new(segment.device_id.clone()).map_err(|error| {
                ImportStagingError::RecoveryConflict(format!(
                    "prepared manifest DeviceId is invalid during verification: {error}"
                ))
            })?;
            let prior_tip = tips.get(&device).ok_or_else(|| {
                ImportStagingError::RecoveryConflict(
                    "prepared segment has no local verified starting tip".into(),
                )
            })?;
            let expected_index = prior_tip
                .segment_index
                .map_or(Some(0), |index| index.checked_add(1))
                .ok_or_else(|| {
                    ImportStagingError::RecoveryConflict(
                        "canonical recovery segment index overflow".into(),
                    )
                })?;
            if segment.segment_index != expected_index {
                return Err(ImportStagingError::RecoveryConflict(
                    "prepared recovery segment does not immediately follow the verified canonical tip".into(),
                ));
            }

            let staged_path = transaction.join(&segment.staged_relative_path);
            let destination_path = canonical_events_device_dir(runtime_root, &device).join(
                Path::new(&segment.destination_relative_path)
                    .file_name()
                    .ok_or_else(|| {
                        ImportStagingError::RecoveryConflict(
                            "prepared recovery destination has no filename".into(),
                        )
                    })?,
            );
            let staged_exists = manifest_file_matches(&staged_path, segment, true)?;
            let destination_exists = manifest_file_matches(&destination_path, segment, true)?;
            if staged_exists == destination_exists {
                return Err(ImportStagingError::RecoveryConflict(
                    "prepared recovery segment must exist in exactly one staged or canonical location".into(),
                ));
            }
            let source_path = if staged_exists {
                &staged_path
            } else {
                &destination_path
            };
            let (mut file, before) = open_recovery_source(source_path)?;
            if before.length != segment.length {
                return Err(ImportStagingError::RecoveryConflict(
                    "prepared recovery source length changed after manifest verification".into(),
                ));
            }
            if staged_exists && canonical_file_id_alias_exists(runtime_root, before.identity)? {
                return Err(ImportStagingError::RecoveryConflict(
                    "staged recovery source aliases an entry under canonical/events".into(),
                ));
            }
            let candidate = StagedSegment {
                device_id: device.clone(),
                segment_index: segment.segment_index,
                length: segment.length,
                sha256: segment.sha256.clone(),
                staged_path: source_path.to_path_buf(),
            };
            let summary = verify_segment_stream(
                &mut file,
                &candidate,
                expected_brain_id,
                prior_tip,
                &mut resolver,
            )?;
            if source_snapshot(&file)? != before {
                return Err(ImportStagingError::RecoveryConflict(
                    "prepared recovery source changed during semantic verification".into(),
                ));
            }
            tips.insert(
                device,
                ImportStreamTip {
                    segment_index: Some(summary.segment_index),
                    device_seq: Some(summary.device_seq),
                    content_hash: summary.content_hash.clone(),
                },
            );
            verified_segments.push(summary);
        }
        Ok(VerifiedStagedBatch {
            segments: verified_segments,
            end_tips: tips.into_iter().collect(),
        })
    })
}

/// Build the local read-only policy context for a startup-visible prepared
/// destination and verify it against the transaction's exact manifest. This
/// function is called only on that recovery branch; it never creates identity,
/// projection, or trust state.
pub fn verify_visible_prepared_recovery_with_local_state<
    S: crate::secure_store::SecureStore + ?Sized,
>(
    runtime_root: &Path,
    transaction: &Path,
    manifest_path: &Path,
    store: &S,
    env: crate::secure_store::SecureStoreEnvironment,
) -> Result<(), ImportStagingError> {
    let manifest = read_prepared_manifest(manifest_path)?;
    let brain = crate::identity_runtime::load_brain_identity(runtime_root).map_err(|error| {
        ImportStagingError::RecoveryConflict(format!(
            "cannot load existing Brain identity for prepared recovery policy: {error}"
        ))
    })?;
    let mut key_resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
        runtime_root,
        &brain.brain_id,
        store,
        env,
    )
    .map_err(|error| {
        ImportStagingError::RecoveryConflict(format!(
            "cannot load existing local device trust anchor for prepared recovery policy: {error}"
        ))
    })?;
    let db = cryptex_projection::ProjectionDb::open_existing_read_only(runtime_root).map_err(
        |error| {
            ImportStagingError::RecoveryConflict(format!(
                "cannot open existing projection read-only for prepared recovery policy: {error}"
            ))
        },
    )?;
    let verified = verify_prepared_recovery_batch(
        runtime_root,
        transaction,
        &manifest,
        &brain.brain_id,
        &mut key_resolver,
        &db,
    )?;
    drop(verified);
    Ok(())
}

fn remove_unprepared_transaction(transaction: &Path) -> Result<(), ImportStagingError> {
    if !entry_exists(transaction)? {
        return Ok(());
    }
    ensure_existing_plain_directory(transaction).map_err(|error| {
        ImportStagingError::RecoveryConflict(format!(
            "staging transaction root is not an ordinary directory: {error}"
        ))
    })?;
    let mut entries = Vec::new();
    let mut total_bytes = 0u64;
    let mut file_count = 0usize;
    let mut device_dir_count = 0usize;
    for device_entry in fs::read_dir(transaction)? {
        let device_entry = device_entry?;
        let device_path = device_entry.path();
        let metadata = fs::symlink_metadata(&device_path)?;
        if is_link_or_reparse(&metadata) || !metadata.is_dir() {
            return Err(ImportStagingError::RecoveryConflict(
                "staging transaction contains a non-directory or link".into(),
            ));
        }
        let device_stem = device_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                ImportStagingError::RecoveryConflict(
                    "staging device directory name is not valid UTF-8".into(),
                )
            })?;
        if device_stem.is_empty()
            || device_stem.len() > 128
            || device_stem.len() % 2 != 0
            || !device_stem
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(ImportStagingError::RecoveryConflict(
                "staging device directory name is outside the filename-ID schema".into(),
            ));
        }
        device_dir_count += 1;
        if device_dir_count > crate::import_limits::MAX_IMPORT_FILES_PER_BATCH {
            return Err(ImportStagingError::RecoveryConflict(
                "staging contains too many device directories".into(),
            ));
        }
        for file_entry in fs::read_dir(&device_path)? {
            let file_entry = file_entry?;
            let file_path = file_entry.path();
            let metadata = fs::symlink_metadata(&file_path)?;
            if is_link_or_reparse(&metadata) || !metadata.is_file() {
                return Err(ImportStagingError::RecoveryConflict(
                    "staging transaction contains a non-file or link".into(),
                ));
            }
            validate_staging_leaf_name(&file_path)?;
            let snapshot =
                open_source_no_follow(&file_path).and_then(|file| source_snapshot(&file))?;
            if snapshot.link_count != 1
                || snapshot.length > crate::import_limits::MAX_IMPORT_FILE_BYTES
            {
                return Err(ImportStagingError::RecoveryConflict(
                    "staging file identity or size is outside the bounded schema".into(),
                ));
            }
            file_count += 1;
            total_bytes = total_bytes.checked_add(snapshot.length).ok_or_else(|| {
                ImportStagingError::RecoveryConflict("staging byte count overflow".into())
            })?;
            if file_count > crate::import_limits::MAX_IMPORT_FILES_PER_BATCH + 1
                || total_bytes > crate::import_limits::MAX_IMPORT_BATCH_BYTES
            {
                return Err(ImportStagingError::RecoveryConflict(
                    "staging transaction exceeds the bounded cleanup limits".into(),
                ));
            }
            entries.push(file_path);
        }
        entries.push(device_path);
    }
    for path in entries.iter().filter(|path| path.is_file()) {
        fs::remove_file(path)?;
    }
    for path in entries.iter().filter(|path| path.is_dir()).rev() {
        fs::remove_dir(path)?;
    }
    fs::remove_dir(transaction)?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceSnapshot {
    pub(crate) length: u64,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) link_count: u32,
    pub(crate) is_regular_file: bool,
    pub(crate) identity: (u64, u64, u64),
}

pub(crate) fn source_snapshot(file: &File) -> Result<SourceSnapshot, ImportStagingError> {
    let metadata = file.metadata()?;
    #[cfg(windows)]
    {
        use std::mem::zeroed;
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
        };
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { zeroed() };
        if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
            return Err(io::Error::last_os_error().into());
        }
        if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(ImportStagingError::Invalid(
                "candidate is a Windows reparse point".into(),
            ));
        }
        Ok(SourceSnapshot {
            length: metadata.len(),
            modified: metadata.modified().ok(),
            link_count: info.nNumberOfLinks,
            is_regular_file: info.dwFileAttributes
                & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_DIRECTORY
                == 0,
            identity: (
                u64::from(info.dwVolumeSerialNumber),
                u64::from(info.nFileIndexHigh),
                u64::from(info.nFileIndexLow),
            ),
        })
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(SourceSnapshot {
            length: metadata.len(),
            modified: metadata.modified().ok(),
            link_count: metadata.nlink().try_into().unwrap_or(u32::MAX),
            is_regular_file: metadata.is_file(),
            identity: (metadata.dev(), metadata.ino(), 0),
        })
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = metadata;
        Err(ImportStagingError::Invalid(
            "no supported no-follow source identity adapter on this platform".into(),
        ))
    }
}

fn open_source_from_directory_chain(
    directory_handles: &[File],
    source_path: &Path,
) -> Result<File, ImportStagingError> {
    #[cfg(windows)]
    {
        let parent = directory_handles.last().ok_or_else(|| {
            ImportStagingError::Invalid("incoming directory handle chain is empty".into())
        })?;
        let leaf = source_path.file_name().ok_or_else(|| {
            ImportStagingError::Invalid("incoming candidate has no filename".into())
        })?;
        open_nt_child(
            parent,
            leaf,
            windows_sys::Win32::Storage::FileSystem::FILE_READ_DATA
                | windows_sys::Win32::Storage::FileSystem::FILE_READ_ATTRIBUTES
                | windows_sys::Win32::Storage::FileSystem::READ_CONTROL
                | windows_sys::Win32::Storage::FileSystem::SYNCHRONIZE,
            windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ,
            windows_sys::Wdk::Storage::FileSystem::FILE_OPEN_REPARSE_POINT
                | windows_sys::Wdk::Storage::FileSystem::FILE_SYNCHRONOUS_IO_NONALERT,
            windows_sys::Wdk::Storage::FileSystem::FILE_OPEN,
            windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL,
        )
    }
    #[cfg(not(windows))]
    {
        let _ = directory_handles;
        open_source_no_follow(source_path)
    }
}

/// Open only enough of a regular file to compare its identity while preserving
/// handle-relative containment. Broader sharing is required for coordination
/// files such as `writer.lock`, whose existing handle is intentionally held.
fn open_identity_from_directory_chain(
    directory_handles: &[File],
    path: &Path,
) -> Result<File, ImportStagingError> {
    #[cfg(windows)]
    {
        let parent = directory_handles.last().ok_or_else(|| {
            ImportStagingError::Invalid("canonical directory handle chain is empty".into())
        })?;
        let leaf = path.file_name().ok_or_else(|| {
            ImportStagingError::Invalid("canonical identity path has no filename".into())
        })?;
        open_nt_child(
            parent,
            leaf,
            windows_sys::Win32::Storage::FileSystem::FILE_READ_ATTRIBUTES
                | windows_sys::Win32::Storage::FileSystem::SYNCHRONIZE,
            windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ
                | windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE
                | windows_sys::Win32::Storage::FileSystem::FILE_SHARE_DELETE,
            windows_sys::Wdk::Storage::FileSystem::FILE_OPEN_REPARSE_POINT
                | windows_sys::Wdk::Storage::FileSystem::FILE_SYNCHRONOUS_IO_NONALERT,
            windows_sys::Wdk::Storage::FileSystem::FILE_OPEN,
            windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL,
        )
    }
    #[cfg(not(windows))]
    {
        let _ = directory_handles;
        open_source_no_follow(path)
    }
}

#[cfg(windows)]
fn open_nt_child(
    parent: &File,
    name: &std::ffi::OsStr,
    desired_access: u32,
    share_access: u32,
    create_options: u32,
    create_disposition: u32,
    file_attributes: u32,
) -> Result<File, ImportStagingError> {
    let display_name = name.to_string_lossy();
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::{AsRawHandle, FromRawHandle};
    use windows_sys::Wdk::Foundation::OBJECT_ATTRIBUTES;
    use windows_sys::Wdk::Storage::FileSystem::NtCreateFile;
    use windows_sys::Win32::Foundation::{HANDLE, UNICODE_STRING};
    use windows_sys::Win32::Storage::FileSystem::{FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_MODE};
    use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;

    let mut wide = name.encode_wide().collect::<Vec<_>>();
    if wide.is_empty() || wide.len() >= (u16::MAX as usize / 2) || wide.contains(&0) {
        return Err(ImportStagingError::Invalid(
            "handle-relative filename is empty or outside the supported UTF-16 bounds".into(),
        ));
    }
    if wide
        .iter()
        .any(|unit| *unit == b'/' as u16 || *unit == b'\\' as u16)
    {
        return Err(ImportStagingError::Invalid(
            "handle-relative filename must be a single path component".into(),
        ));
    }
    let length_bytes = (wide.len() * std::mem::size_of::<u16>()) as u16;
    wide.push(0);
    let mut name = UNICODE_STRING {
        Length: length_bytes,
        MaximumLength: length_bytes + std::mem::size_of::<u16>() as u16,
        Buffer: wide.as_mut_ptr(),
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: std::mem::size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: parent.as_raw_handle() as HANDLE,
        ObjectName: &mut name,
        Attributes: windows_sys::Win32::Foundation::OBJ_CASE_INSENSITIVE,
        SecurityDescriptor: std::ptr::null(),
        SecurityQualityOfService: std::ptr::null(),
    };
    let mut handle: HANDLE = std::ptr::null_mut();
    let mut io_status = IO_STATUS_BLOCK::default();
    let status = unsafe {
        NtCreateFile(
            &mut handle,
            desired_access,
            &attributes,
            &mut io_status,
            std::ptr::null(),
            file_attributes as FILE_FLAGS_AND_ATTRIBUTES,
            share_access as FILE_SHARE_MODE,
            create_disposition,
            create_options,
            std::ptr::null(),
            0,
        )
    };
    if status < 0 || handle.is_null() {
        return Err(ImportStagingError::Io(format!(
            "handle-relative NtCreateFile for {:?} failed with NTSTATUS 0x{:08X}",
            display_name, status as u32,
        )));
    }
    Ok(unsafe { File::from_raw_handle(handle) })
}

pub(crate) fn open_source_no_follow(path: &Path) -> Result<File, ImportStagingError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_FLAG_OPEN_REPARSE_POINT, FILE_FLAG_SEQUENTIAL_SCAN, FILE_SHARE_READ,
        };
        // Excluding write/delete sharing freezes the opened file while copied.
        options
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_SEQUENTIAL_SCAN);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    Ok(options.open(path)?)
}

/// Open a recovery candidate through a checked parent-directory chain and pin
/// its identity before hashing. Callers must compare `source_snapshot` again
/// after reading; the returned file handle itself does not follow a final link.
pub(crate) fn open_recovery_source(
    path: &Path,
) -> Result<(File, SourceSnapshot), ImportStagingError> {
    let parent = path.parent().ok_or_else(|| {
        ImportStagingError::RecoveryConflict("recovery file has no parent directory".into())
    })?;
    let parent_handles = inspect_directory_chain(parent, true)?;
    let file = open_source_from_directory_chain(&parent_handles, path)?;
    let before = source_snapshot(&file)?;
    if !before.is_regular_file || before.link_count != 1 {
        return Err(ImportStagingError::RecoveryConflict(
            "recovery file is not a single-link ordinary file".into(),
        ));
    }
    drop(parent_handles);
    Ok((file, before))
}

fn copy_bounded(
    source: &mut File,
    destination: &mut File,
    expected_length: u64,
) -> Result<(u64, String), ImportStagingError> {
    let mut digest = Sha256::new();
    let mut buffer = [0u8; COPY_BUFFER_BYTES];
    let mut total = 0u64;
    loop {
        let read = source.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read as u64)
            .ok_or_else(|| ImportStagingError::Invalid("copy length overflow".into()))?;
        if total > expected_length {
            return Err(ImportStagingError::Invalid(
                "candidate grew beyond its frozen size while copying".into(),
            ));
        }
        destination.write_all(&buffer[..read])?;
        digest.update(&buffer[..read]);
    }
    Ok((total, encode_hex(&digest.finalize())))
}

fn hash_bounded(
    file: &mut File,
    expected_length: u64,
) -> Result<(u64, String), ImportStagingError> {
    let mut digest = Sha256::new();
    let mut buffer = [0u8; COPY_BUFFER_BYTES];
    let mut total = 0u64;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read as u64)
            .ok_or_else(|| ImportStagingError::Invalid("readback length overflow".into()))?;
        if total > expected_length {
            return Err(ImportStagingError::Invalid(
                "staged file exceeds its frozen length".into(),
            ));
        }
        digest.update(&buffer[..read]);
    }
    Ok((total, encode_hex(&digest.finalize())))
}

fn validate_staging_leaf_name(path: &Path) -> Result<(), ImportStagingError> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            ImportStagingError::RecoveryConflict("staging file name is not valid UTF-8".into())
        })?;
    let bytes = name.as_bytes();
    let is_segment =
        bytes.len() == 24 && &bytes[20..] == b".seg" && bytes[..20].iter().all(u8::is_ascii_digit);
    let is_temporary = bytes.len() == 37
        && bytes[0] == b'.'
        && &bytes[33..] == b".tmp"
        && bytes[1..33]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte));
    if !is_segment && !is_temporary {
        return Err(ImportStagingError::RecoveryConflict(
            "staging file name is outside the generated schema".into(),
        ));
    }
    Ok(())
}

fn validate_relative_segment_path(path: &Path) -> Result<(), ImportStagingError> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        || path.extension().and_then(|extension| extension.to_str()) != Some("seg")
    {
        return Err(ImportStagingError::Invalid(
            "candidate path must be relative, traversal-free, and end in .seg".into(),
        ));
    }
    Ok(())
}

fn validate_import_path_binding(
    path: &Path,
    device_id: &DeviceId,
    segment_index: u64,
) -> Result<(), ImportStagingError> {
    let components = path.components().collect::<Vec<_>>();
    if components.len() != 2 {
        return Err(ImportStagingError::Invalid(
            "incoming segment path must be <device-id-hex>/<20-digit-index>.seg".into(),
        ));
    }
    let Component::Normal(device_stem) = components[0] else {
        return Err(ImportStagingError::Invalid(
            "incoming segment path has an invalid device directory".into(),
        ));
    };
    let Component::Normal(segment_name) = components[1] else {
        return Err(ImportStagingError::Invalid(
            "incoming segment path has an invalid segment filename".into(),
        ));
    };
    let expected_device_stem = device_id.as_filename_stem();
    let expected_segment_name = format!("{segment_index:020}.seg");
    if device_stem != expected_device_stem.as_str()
        || segment_name != expected_segment_name.as_str()
    {
        return Err(ImportStagingError::Invalid(
            "incoming path device/index does not match the candidate identity".into(),
        ));
    }
    Ok(())
}

fn parse_transport_segment_path(path: &Path) -> Result<(DeviceId, u64), ImportStagingError> {
    validate_relative_segment_path(path)?;
    let components = path.components().collect::<Vec<_>>();
    if components.len() != 2 {
        return Err(ImportStagingError::Invalid(
            "incoming segment path must be <device-id-hex>/<20-digit-index>.seg".into(),
        ));
    }
    let Component::Normal(device_stem) = components[0] else {
        return Err(ImportStagingError::Invalid(
            "incoming segment path has an invalid device directory".into(),
        ));
    };
    let device_stem = device_stem.to_str().ok_or_else(|| {
        ImportStagingError::Invalid("incoming device directory is not valid UTF-8".into())
    })?;
    if device_stem.is_empty()
        || device_stem.len() % 2 != 0
        || !device_stem
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ImportStagingError::Invalid(
            "incoming device directory is not canonical lowercase hex".into(),
        ));
    }
    let device_bytes = device_stem
        .as_bytes()
        .chunks(2)
        .map(|pair| {
            let high = hex_nibble(pair[0]).expect("validated lowercase hex");
            let low = hex_nibble(pair[1]).expect("validated lowercase hex");
            (high << 4) | low
        })
        .collect::<Vec<_>>();
    let device_text = String::from_utf8(device_bytes)
        .map_err(|_| ImportStagingError::Invalid("incoming device ID is not valid UTF-8".into()))?;
    let device_id = DeviceId::new(device_text)
        .map_err(|error| ImportStagingError::Invalid(error.to_string()))?;
    if device_id.as_filename_stem() != device_stem {
        return Err(ImportStagingError::Invalid(
            "incoming device ID does not round-trip to its canonical filename".into(),
        ));
    }

    let Component::Normal(segment_name) = components[1] else {
        return Err(ImportStagingError::Invalid(
            "incoming segment filename is invalid".into(),
        ));
    };
    let segment_name = segment_name.to_str().ok_or_else(|| {
        ImportStagingError::Invalid("incoming segment filename is not valid UTF-8".into())
    })?;
    let Some(index_text) = segment_name.strip_suffix(".seg") else {
        return Err(ImportStagingError::Invalid(
            "incoming segment filename must end in .seg".into(),
        ));
    };
    if index_text.len() != 20 || !index_text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ImportStagingError::Invalid(
            "incoming segment index must contain exactly 20 decimal digits".into(),
        ));
    }
    let segment_index = index_text.parse::<u64>().map_err(|_| {
        ImportStagingError::Invalid("incoming segment index is outside the u64 range".into())
    })?;
    if format!("{segment_index:020}.seg") != segment_name {
        return Err(ImportStagingError::Invalid(
            "incoming segment filename is not in canonical decimal form".into(),
        ));
    }
    Ok((device_id, segment_index))
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn validate_plain_path_below(root: &Path, relative: &Path) -> Result<(), ImportStagingError> {
    ensure_existing_plain_directory(root)?;
    let mut cursor = root.to_path_buf();
    let components = relative.components().collect::<Vec<_>>();
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(part) = component else {
            return Err(ImportStagingError::Invalid("invalid relative path".into()));
        };
        cursor.push(part);
        let metadata = fs::symlink_metadata(&cursor)?;
        if is_link_or_reparse(&metadata) {
            return Err(ImportStagingError::Invalid(
                "incoming path contains a link or reparse point".into(),
            ));
        }
        if index + 1 < components.len() && !metadata.is_dir() {
            return Err(ImportStagingError::Invalid(
                "incoming parent is not an ordinary directory".into(),
            ));
        }
    }
    Ok(())
}

fn ensure_directory_chain(path: &Path) -> Result<(), ImportStagingError> {
    let mut missing = Vec::new();
    let mut cursor = path;
    while !cursor.exists() {
        missing.push(cursor.to_path_buf());
        cursor = cursor.parent().ok_or_else(|| {
            ImportStagingError::Invalid("directory path has no existing ancestor".into())
        })?;
    }
    ensure_existing_plain_directory(cursor)?;
    for directory in missing.iter().rev() {
        match fs::create_dir(directory) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
        ensure_plain_directory(directory)?;
    }
    Ok(())
}

fn ensure_existing_plain_directory(path: &Path) -> Result<(), ImportStagingError> {
    let metadata = fs::symlink_metadata(path)?;
    ensure_metadata_plain_directory(&metadata)
}

fn ensure_plain_directory(path: &Path) -> Result<(), ImportStagingError> {
    ensure_existing_plain_directory(path)
}

fn entry_exists(path: &Path) -> Result<bool, ImportStagingError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

/// Reject an incoming source whose opened file identity is already present in
/// canonical storage. This is an admission-time scan only; publication must
/// repeat alias/destination checks while canonical locks are held.
fn canonical_file_id_alias_exists(
    runtime_root: &Path,
    source_identity: (u64, u64, u64),
) -> Result<bool, ImportStagingError> {
    let events_root = runtime_root.join("canonical/events");
    if !entry_exists(&events_root)? {
        return Ok(false);
    }
    ensure_existing_plain_directory(&events_root)?;
    let events_handles = inspect_directory_chain(&events_root, true)?;
    for device_entry in fs::read_dir(&events_root)? {
        let device_entry = device_entry?;
        let device_path = device_entry.path();
        let metadata = fs::symlink_metadata(&device_path)?;
        if device_entry.file_name() == "inventory.lock" && metadata.is_file() {
            let file = open_identity_from_directory_chain(&events_handles, &device_path)?;
            if source_snapshot(&file)?.identity == source_identity {
                return Ok(true);
            }
            continue;
        }
        ensure_metadata_plain_directory(&metadata)?;
        let device_handles = inspect_directory_chain(&device_path, true)?;
        for file_entry in fs::read_dir(&device_path)? {
            let file_entry = file_entry?;
            let file_path = file_entry.path();
            let metadata = fs::symlink_metadata(&file_path)?;
            if is_link_or_reparse(&metadata) || !metadata.is_file() {
                return Err(ImportStagingError::Invalid(
                    "canonical device directory contains a link or non-file entry".into(),
                ));
            }
            if file_entry.file_name() == "writer.lock" {
                // This is coordination metadata, but still compare its file
                // identity so a staged hard link to the lock cannot bypass
                // the canonical alias check. The held handle requires broad
                // share access on Windows.
                let file = open_identity_from_directory_chain(&device_handles, &file_path)?;
                if source_snapshot(&file)?.identity == source_identity {
                    return Ok(true);
                }
                continue;
            }
            let file = open_source_from_directory_chain(&device_handles, &file_path)?;
            if source_snapshot(&file)?.identity == source_identity {
                return Ok(true);
            }
        }
        drop(device_handles);
    }
    drop(events_handles);
    Ok(false)
}

fn same_volume(first: &Path, second: &Path) -> Result<bool, ImportStagingError> {
    let first_metadata = fs::metadata(first)?;
    let second_metadata = fs::metadata(second)?;
    #[cfg(windows)]
    {
        let _ = (first_metadata, second_metadata);
        let first_volume = directory_volume_serial(first)?;
        let second_volume = directory_volume_serial(second)?;
        Ok(first_volume == second_volume)
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(first_metadata.dev() == second_metadata.dev())
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (first_metadata, second_metadata);
        Err(ImportStagingError::Invalid(
            "same-volume verification unsupported on this platform".into(),
        ))
    }
}

#[cfg(windows)]
fn directory_volume_serial(path: &Path) -> Result<u32, ImportStagingError> {
    use std::mem::zeroed;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::{AsRawHandle, FromRawHandle};
    use std::ptr;
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, CreateFileW, FILE_ATTRIBUTE_DIRECTORY,
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES,
        FILE_SHARE_READ, FILE_SHARE_WRITE, GetFileInformationByHandle, OPEN_EXISTING,
    };

    let wide = path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS,
            ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error().into());
    }
    let file = unsafe { File::from_raw_handle(handle) };
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { zeroed() };
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err(io::Error::last_os_error().into());
    }
    if info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0
        || info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || info.nNumberOfLinks != 1
    {
        return Err(ImportStagingError::Invalid(
            "canonical/staging volume roots must be ordinary non-linked directories".into(),
        ));
    }
    Ok(info.dwVolumeSerialNumber)
}

fn ensure_metadata_plain_directory(metadata: &fs::Metadata) -> Result<(), ImportStagingError> {
    if is_link_or_reparse(metadata) || !metadata.is_dir() {
        return Err(ImportStagingError::Invalid(
            "runtime staging path must contain only ordinary directories".into(),
        ));
    }
    Ok(())
}

/// Opens each directory component relative to the previously opened handle,
/// requesting the leaf itself when it is a reparse point. This avoids walking
/// unchecked ancestor components between path validation and open. Windows
/// also verifies that the target directory is owned by the current user.
#[cfg(windows)]
fn inspect_directory_chain(
    path: &Path,
    require_current_user_owner: bool,
) -> Result<Vec<File>, ImportStagingError> {
    use std::os::windows::io::{AsRawHandle, FromRawHandle};
    use std::ptr;
    use windows_sys::Wdk::Storage::FileSystem::{FILE_OPEN, FILE_OPEN_REPARSE_POINT};
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE, LocalFree};
    use windows_sys::Win32::Security::Authorization::{GetSecurityInfo, SE_FILE_OBJECT};
    use windows_sys::Win32::Security::{
        EqualSid, GetLengthSid, GetTokenInformation, OWNER_SECURITY_INFORMATION, TOKEN_QUERY,
        TOKEN_USER, TokenUser,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, CreateFileW, FILE_ATTRIBUTE_DIRECTORY,
        FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        FILE_READ_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_TRAVERSE,
        GetFileInformationByHandle, OPEN_EXISTING, READ_CONTROL,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    struct TokenGuard(HANDLE);
    impl Drop for TokenGuard {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    struct SecurityDescriptor(*mut std::ffi::c_void);
    impl Drop for SecurityDescriptor {
        fn drop(&mut self) {
            unsafe {
                LocalFree(self.0);
            }
        }
    }

    fn win_error() -> ImportStagingError {
        ImportStagingError::Io(io::Error::last_os_error().to_string())
    }

    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(ImportStagingError::Invalid(
            "directory pin path must be absolute and traversal-free".into(),
        ));
    }

    let mut components = path.components();
    let prefix = match components.next() {
        Some(Component::Prefix(prefix)) if matches!(prefix.kind(), std::path::Prefix::Disk(_)) => {
            prefix
        }
        _ => {
            return Err(ImportStagingError::Invalid(
                "Windows staging paths must use a local drive letter".into(),
            ));
        }
    };
    if !matches!(components.next(), Some(Component::RootDir)) {
        return Err(ImportStagingError::Invalid(
            "Windows staging path is not rooted at its drive".into(),
        ));
    }
    let volume_root = format!("{}\\", prefix.as_os_str().to_string_lossy());
    let mut root_wide = volume_root.encode_utf16().collect::<Vec<_>>();
    root_wide.push(0);
    let root_handle = unsafe {
        CreateFileW(
            root_wide.as_ptr(),
            FILE_READ_ATTRIBUTES | READ_CONTROL | FILE_TRAVERSE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS,
            ptr::null_mut(),
        )
    };
    if root_handle == INVALID_HANDLE_VALUE {
        return Err(win_error());
    }
    let root = unsafe { File::from_raw_handle(root_handle) };
    let mut root_info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    if unsafe { GetFileInformationByHandle(root.as_raw_handle(), &mut root_info) } == 0 {
        return Err(win_error());
    }
    if root_info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0
        || root_info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(ImportStagingError::Invalid(
            "Windows volume root is not an ordinary directory".into(),
        ));
    }
    let mut pins = vec![root];
    for component in components {
        let Component::Normal(name) = component else {
            return Err(ImportStagingError::Invalid(
                "Windows directory chain contains a non-name component".into(),
            ));
        };
        let parent = pins.last().expect("volume root handle is present");
        let file = open_nt_child(
            parent,
            name,
            FILE_READ_ATTRIBUTES | READ_CONTROL | FILE_TRAVERSE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            FILE_OPEN_REPARSE_POINT,
            FILE_OPEN,
            0,
        )?;
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
            return Err(win_error());
        }
        if info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0
            || info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
            || info.nNumberOfLinks != 1
        {
            return Err(ImportStagingError::Invalid(
                "directory chain contains a reparse point, alias, or non-directory".into(),
            ));
        }
        pins.push(file);
    }

    if require_current_user_owner {
        let mut token: HANDLE = ptr::null_mut();
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return Err(win_error());
        }
        let token = TokenGuard(token);
        let mut required = 0;
        unsafe {
            GetTokenInformation(token.0, TokenUser, ptr::null_mut(), 0, &mut required);
        }
        if required < std::mem::size_of::<TOKEN_USER>() as u32 || required > 65536 {
            return Err(ImportStagingError::Invalid(
                "current user SID size is outside the supported bounds".into(),
            ));
        }
        let mut storage = vec![0usize; (required as usize).div_ceil(std::mem::size_of::<usize>())];
        if unsafe {
            GetTokenInformation(
                token.0,
                TokenUser,
                storage.as_mut_ptr().cast(),
                required,
                &mut required,
            )
        } == 0
        {
            return Err(win_error());
        }
        let current_sid = unsafe { (*storage.as_ptr().cast::<TOKEN_USER>()).User.Sid };
        let current_sid_len = unsafe { GetLengthSid(current_sid) } as usize;
        if current_sid_len == 0 || current_sid_len > 65536 {
            return Err(ImportStagingError::Invalid(
                "current user SID length is outside the supported bounds".into(),
            ));
        }
        let current_sid = unsafe {
            std::slice::from_raw_parts(current_sid.cast::<u8>(), current_sid_len).to_vec()
        };
        let target = pins
            .last()
            .ok_or_else(|| ImportStagingError::Invalid("directory pin chain is empty".into()))?;
        let mut owner = ptr::null_mut();
        let mut descriptor = ptr::null_mut();
        let result = unsafe {
            GetSecurityInfo(
                target.as_raw_handle(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION,
                &mut owner,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                &mut descriptor,
            )
        };
        if result != 0 {
            return Err(ImportStagingError::Io(format!(
                "cannot read staging directory owner: Windows error {result}"
            )));
        }
        let _descriptor = SecurityDescriptor(descriptor);
        if owner.is_null()
            || unsafe { EqualSid(owner, current_sid.as_ptr().cast_mut().cast()) } == 0
        {
            return Err(ImportStagingError::Invalid(
                "staging directory is not owned by the current user".into(),
            ));
        }
    }
    Ok(pins)
}

#[cfg(not(windows))]
fn inspect_directory_chain(
    path: &Path,
    _require_current_user_owner: bool,
) -> Result<Vec<File>, ImportStagingError> {
    ensure_existing_plain_directory(path)?;
    Ok(Vec::new())
}

fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    false
}

fn open_lock_file(path: &Path) -> Result<File, ImportStagingError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(ImportStagingError::Invalid(
                "import single-flight lock is not an ordinary file".into(),
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let mut options = OpenOptions::new();
    options.create(true).truncate(false).read(true).write(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
        };
        options
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let file = options.open(path)?;
    let snapshot = source_snapshot(&file)?;
    if !snapshot.is_regular_file || snapshot.link_count != 1 {
        return Err(ImportStagingError::Invalid(
            "import single-flight lock must be an unlinked ordinary file".into(),
        ));
    }
    Ok(file)
}

fn random_transaction_id() -> Result<String, ImportStagingError> {
    let bytes = random_bytes::<16>()?;
    let hex = encode_hex(&bytes);
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    ))
}

fn random_hex_16() -> Result<String, ImportStagingError> {
    Ok(encode_hex(&random_bytes::<16>()?))
}

fn random_bytes<const N: usize>() -> Result<[u8; N], ImportStagingError> {
    let mut bytes = [0u8; N];
    getrandom::getrandom(&mut bytes)
        .map_err(|error| ImportStagingError::Io(format!("secure random unavailable: {error}")))?;
    Ok(bytes)
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn publish_staged_no_replace(source: &Path, destination: &Path) -> Result<(), ImportStagingError> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{MOVEFILE_WRITE_THROUGH, MoveFileExW};
        let source_wide = source
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let destination_wide = destination
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        if unsafe {
            MoveFileExW(
                source_wide.as_ptr(),
                destination_wide.as_ptr(),
                MOVEFILE_WRITE_THROUGH,
            )
        } == 0
        {
            return Err(io::Error::last_os_error().into());
        }
        Ok(())
    }
    #[cfg(unix)]
    {
        fs::hard_link(source, destination)?;
        fs::remove_file(source)?;
        Ok(())
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (source, destination);
        Err(ImportStagingError::Invalid(
            "no no-replace staging publication adapter on this platform".into(),
        ))
    }
}

fn sync_staged_parent(path: &Path) -> Result<(), ImportStagingError> {
    #[cfg(unix)]
    {
        let parent = path.parent().ok_or_else(|| {
            ImportStagingError::Invalid("staged segment path has no parent".into())
        })?;
        File::open(parent)?.sync_all()?;
    }
    #[cfg(windows)]
    {
        use std::os::windows::{ffi::OsStrExt, io::FromRawHandle};
        use std::ptr;
        use windows_sys::Win32::Foundation::{GENERIC_WRITE, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, CreateFileW, FILE_ATTRIBUTE_DIRECTORY,
            FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
            FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FlushFileBuffers,
            GetFileInformationByHandle, OPEN_EXISTING,
        };

        let parent = path.parent().ok_or_else(|| {
            ImportStagingError::Invalid("staged path has no parent directory".into())
        })?;
        let wide = parent
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let raw = unsafe {
            CreateFileW(
                wide.as_ptr(),
                GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                ptr::null_mut(),
            )
        };
        if raw == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error().into());
        }
        let directory = unsafe { File::from_raw_handle(raw) };
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { GetFileInformationByHandle(raw, &mut info) } == 0 {
            return Err(io::Error::last_os_error().into());
        }
        if info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0
            || info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        {
            return Err(ImportStagingError::Invalid(
                "refusing to flush a non-directory or reparse-point parent".into(),
            ));
        }
        if unsafe { FlushFileBuffers(raw) } == 0 {
            return Err(io::Error::last_os_error().into());
        }
        drop(directory);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cryptex_events::envelope::{
        Classification, EventPayload, EventType, MemoryNotePayload, SignableEvent,
    };
    use cryptex_events::{DeviceSeq, accept_event, signing_key_id_from_verifying_key};
    use ed25519_dalek::{SigningKey, VerifyingKey};
    use std::time::{SystemTime, UNIX_EPOCH};

    const TEST_BRAIN_ID: &str = "22222222-2222-4222-8222-222222222222";

    struct TestTrustedKey(VerifyingKey);

    impl TrustedDaemonKeyResolver for TestTrustedKey {
        fn trusted_daemon_key(&mut self, device_id: &str, _key_id: &str) -> Option<VerifyingKey> {
            (device_id == "device-a").then_some(self.0)
        }
    }

    fn signed_test_segment(
        device: &DeviceId,
    ) -> (Vec<u8>, cryptex_events::AcceptedEvent, SigningKey) {
        let key = SigningKey::from_bytes(&[7_u8; 32]);
        let (bytes, event) = signed_test_segment_with_key(
            device,
            1,
            "11111111-1111-4111-8111-111111111111",
            ContentHashHex::zero(),
            &key,
        );
        (bytes, event, key)
    }

    fn signed_test_segment_with_key(
        device: &DeviceId,
        device_seq: u64,
        event_id: &str,
        prev_content_hash: ContentHashHex,
        key: &SigningKey,
    ) -> (Vec<u8>, cryptex_events::AcceptedEvent) {
        let key_id = signing_key_id_from_verifying_key(&key.verifying_key()).unwrap();
        let signable = SignableEvent {
            schema_version: cryptex_events::EVENT_SCHEMA_VERSION,
            event_id: event_id.into(),
            brain_id: TEST_BRAIN_ID.into(),
            device_id: device.as_str().into(),
            device_seq: DeviceSeq::from_u64(device_seq).unwrap(),
            created_at: "2026-10-24T00:00:00Z".into(),
            event_type: EventType::MemoryNote,
            classification: Classification::Public,
            signing_key_id: key_id,
            payload: EventPayload::MemoryNote(MemoryNotePayload::new("synthetic import").unwrap()),
            prev_content_hash,
        };
        let event = accept_event(signable, key).unwrap();
        let body = serde_json::to_vec(&event).unwrap();
        let mut segment = cryptex_events::encode_frame_header(body.len() as u32)
            .unwrap()
            .to_vec();
        segment.extend_from_slice(&body);
        (segment, event)
    }

    fn runtime_root(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "cryptex-m8-import-staging-{label}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("transport/incoming")).unwrap();
        root
    }

    fn compact_runtime_root() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("m8-{}-{stamp:x}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn prepared_policy_verified_fixture(
        root: &Path,
    ) -> (
        DeviceId,
        ImportStagingSession,
        crate::knowledge_materialization::ImportPolicyVerifiedBatch,
        crate::secure_store::FakeSecureStore,
        cryptex_projection::ProjectionDb,
    ) {
        let store = crate::secure_store::FakeSecureStore::new();
        let brain = TEST_BRAIN_ID;
        let identity = cryptex_core::BrainIdentity::new(brain, "2026-10-29T00:00:00Z").unwrap();
        crate::identity_runtime::write_brain_identity(root, &identity).unwrap();
        let enrollment = crate::local_device_identity::enroll_local_device(
            root,
            &store,
            crate::secure_store::SecureStoreEnvironment::Dev,
        )
        .unwrap();
        let device = DeviceId::new(enrollment.device_id).unwrap();
        let first = crate::canonical_writer::append_memory_note(
            &store,
            crate::secure_store::SecureStoreEnvironment::Dev,
            crate::canonical_writer::AppendMemoryNoteArgs {
                runtime_root: root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Public,
                text: "synthetic local trust anchor fixture",
                event_id: None,
                config: &crate::canonical_writer::CanonicalWriterConfig {
                    seal_event_threshold: 1,
                },
            },
        )
        .unwrap()
        .accepted;
        let ready = crate::event_signing_key::ensure_event_signing_key(
            &store,
            crate::secure_store::SecureStoreEnvironment::Dev,
            root,
            &device,
            std::slice::from_ref(&first.signing_key_id),
            true,
        )
        .unwrap();
        let retirement_event_id = "22222222-2222-4222-8222-222222222224";
        let retirement_body = cryptex_events::security_v3::sign_device_self_retired_event(
            retirement_event_id,
            brain,
            &device,
            DeviceSeq::from_u64(2).unwrap(),
            "2026-10-29T00:00:01Z",
            &first.signing_key_id,
            &first.content_hash,
            &"a".repeat(64),
            1,
            &first.content_hash,
            &ready.signing_key,
        )
        .unwrap();
        let mut candidate_bytes = cryptex_events::encode_frame_header(retirement_body.len() as u32)
            .unwrap()
            .to_vec();
        candidate_bytes.extend_from_slice(&retirement_body);
        let relative = PathBuf::from(device.as_filename_stem()).join("00000000000000000001.seg");
        let source = root.join("transport/incoming").join(&relative);
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, candidate_bytes).unwrap();

        let mut session = ImportStagingSession::begin(root).unwrap();
        session.stage_segment(&relative, &device, 1).unwrap();
        session.release_staging_slot().unwrap();
        let mut resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
            root,
            brain,
            &store,
            crate::secure_store::SecureStoreEnvironment::Dev,
        )
        .unwrap();
        let mut db = cryptex_projection::ProjectionDb::open_or_create(root).unwrap();
        crate::projection::rebuild_device_projection(
            &store,
            crate::secure_store::SecureStoreEnvironment::Dev,
            root,
            &device,
            &db,
        )
        .unwrap();
        crate::knowledge_materialization::rebuild_knowledge_from_canonical(
            root,
            brain,
            std::slice::from_ref(&device),
            &mut resolver,
            &mut db,
        )
        .unwrap();
        let safety = crate::knowledge_materialization::evaluate_import_retrieval_safety(
            root,
            brain,
            std::slice::from_ref(&device),
            &mut resolver,
            &db,
        );
        assert!(safety.allows_content());
        let policy_verified = safety.verify_import_batch(&session, brain).unwrap();
        assert_eq!(
            policy_verified
                .post_import_policy()
                .trust
                .effective_cutoffs
                .get(&device),
            Some(&1),
            "the self-retirement control should reduce the local content cutoff"
        );
        assert!(
            !policy_verified
                .post_import_policy()
                .content
                .materialized_event_ids
                .contains(&retirement_event_id.to_owned()),
            "security control metadata must never enter materialized content"
        );
        session
            .prepare_verified_manifest(policy_verified.verified())
            .unwrap();
        (device, session, policy_verified, store, db)
    }

    #[test]
    fn prepared_recovery_batch_verifies_existing_destination_against_preimport_policy() {
        let root = compact_runtime_root();
        let (device, session, initial_policy, store, db) = prepared_policy_verified_fixture(&root);
        let transaction_id = session.transaction_id().to_owned();
        drop(initial_policy);
        drop(session);

        let transaction = root.join("canonical/import-staging").join(&transaction_id);
        let manifest_path = root
            .join("canonical/import-journal")
            .join(format!("{transaction_id}.prepared.json"));
        let manifest = read_prepared_manifest(&manifest_path).unwrap();
        let segment = &manifest.segments[0];
        let staged = transaction.join(&segment.staged_relative_path);
        let destination = canonical_events_device_dir(&root, &device).join(
            Path::new(&segment.destination_relative_path)
                .file_name()
                .unwrap(),
        );
        publish_staged_no_replace(&staged, &destination).unwrap();

        let brain = TEST_BRAIN_ID;
        let mut resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
            &root,
            brain,
            &store,
            crate::secure_store::SecureStoreEnvironment::Dev,
        )
        .unwrap();
        let verified = verify_prepared_recovery_batch(
            &root,
            &transaction,
            &manifest,
            brain,
            &mut resolver,
            &db,
        )
        .unwrap();
        assert!(verified.keeps_retrieval_snapshot());
        assert_eq!(verified.verified().segments().len(), 1);
        assert_eq!(verified.verified().segments()[0].device_id, device);
        assert_eq!(verified.verified().segments()[0].segment_index, 1);
        assert_eq!(
            verified
                .post_import_policy()
                .trust
                .effective_cutoffs
                .get(&device),
            Some(&1),
            "prepared recovery must reapply imported self-retirement policy"
        );
        drop(verified);
        drop(db);
        let read_only_db =
            cryptex_projection::ProjectionDb::open_existing_read_only(&root).unwrap();
        let mut read_only_resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
            &root,
            brain,
            &store,
            crate::secure_store::SecureStoreEnvironment::Dev,
        )
        .unwrap();
        verify_prepared_recovery_batch(
            &root,
            &transaction,
            &manifest,
            brain,
            &mut read_only_resolver,
            &read_only_db,
        )
        .unwrap();
        drop(read_only_db);

        let mut policy_verification_count = 0;
        let held = recover_import_staging_with_policy_verifier(
            &root,
            |seen_root, seen_transaction, seen_manifest| {
                policy_verification_count += 1;
                assert_eq!(seen_root, root);
                assert_eq!(seen_transaction, transaction);
                assert_eq!(seen_manifest, manifest_path);
                verify_visible_prepared_recovery_with_local_state(
                    seen_root,
                    seen_transaction,
                    seen_manifest,
                    &store,
                    crate::secure_store::SecureStoreEnvironment::Dev,
                )
            },
        )
        .unwrap_err();
        assert_eq!(policy_verification_count, 1);
        assert!(
            held.to_string()
                .contains("all canonical destinations match"),
            "unexpected recovery hold: {held}"
        );
        assert!(manifest_path.is_file());
        assert!(destination.is_file());
        assert!(!staged.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn committed_marker_parser_accepts_only_canonical_closed_v1_records() {
        let root = runtime_root("committed-marker-parser");
        let journal = root.join("canonical/import-journal");
        fs::create_dir_all(&journal).unwrap();
        let transaction = "123e4567-e89b-12d3-a456-426614174000";
        let path = journal.join(format!("{transaction}.committed.json"));
        let marker = CommittedImportMarker {
            protocol_version: 1,
            transaction_id: transaction.into(),
            prepared_manifest_sha256: "a".repeat(64),
        };
        let bytes = serde_json_canonicalizer::to_vec(&marker).unwrap();
        fs::write(&path, &bytes).unwrap();

        assert_eq!(read_committed_marker(&path).unwrap(), marker);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn committed_marker_parser_rejects_unknown_fields_and_noncanonical_json() {
        let root = runtime_root("committed-marker-closed-schema");
        let journal = root.join("canonical/import-journal");
        fs::create_dir_all(&journal).unwrap();
        let transaction = "123e4567-e89b-12d3-a456-426614174000";
        let path = journal.join(format!("{transaction}.committed.json"));
        let marker = format!(
            "{{\"protocol_version\":1,\"transaction_id\":\"{transaction}\",\"prepared_manifest_sha256\":\"{}\",\"extra\":true}}",
            "a".repeat(64)
        );
        fs::write(&path, marker).unwrap();
        assert!(matches!(
            read_committed_marker(&path),
            Err(ImportStagingError::RecoveryConflict(_))
        ));

        let noncanonical = format!(
            "{{ \"protocol_version\": 1, \"transaction_id\": \"{transaction}\", \"prepared_manifest_sha256\": \"{}\" }}",
            "a".repeat(64)
        );
        fs::write(&path, noncanonical).unwrap();
        assert!(matches!(
            read_committed_marker(&path),
            Err(ImportStagingError::RecoveryConflict(_))
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn committed_marker_parser_rejects_invalid_version_digest_and_filename_binding() {
        let root = runtime_root("committed-marker-fields");
        let journal = root.join("canonical/import-journal");
        fs::create_dir_all(&journal).unwrap();
        let transaction = "123e4567-e89b-12d3-a456-426614174000";
        let path = journal.join(format!("{transaction}.committed.json"));

        for (version, digest) in [
            (2, "a".repeat(64)),
            (1, "A".repeat(64)),
            (1, "a".repeat(63)),
        ] {
            let marker = CommittedImportMarker {
                protocol_version: version,
                transaction_id: transaction.into(),
                prepared_manifest_sha256: digest,
            };
            fs::write(&path, serde_json_canonicalizer::to_vec(&marker).unwrap()).unwrap();
            assert!(matches!(
                read_committed_marker(&path),
                Err(ImportStagingError::RecoveryConflict(_))
            ));
        }

        fs::write(&path, vec![b'a'; MAX_COMMITTED_MARKER_BYTES as usize + 1]).unwrap();
        assert!(matches!(
            read_committed_marker(&path),
            Err(ImportStagingError::RecoveryConflict(_))
        ));

        let wrong_name = journal.join("123e4567-e89b-12d3-a456-426614174001.committed.json");
        let marker = CommittedImportMarker {
            protocol_version: 1,
            transaction_id: transaction.into(),
            prepared_manifest_sha256: "a".repeat(64),
        };
        fs::write(
            &wrong_name,
            serde_json_canonicalizer::to_vec(&marker).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            read_committed_marker(&wrong_name),
            Err(ImportStagingError::RecoveryConflict(_))
        ));
        fs::remove_dir_all(root).unwrap();
    }

    fn committed_marker_companion_fixture(
        label: &str,
        marker_digest: Option<String>,
    ) -> (PathBuf, PathBuf, PathBuf, Vec<(PathBuf, Vec<u8>)>) {
        let root = runtime_root(label);
        let transaction_id = "123e4567-e89b-12d3-a456-426614174000";
        let device = DeviceId::new("device-a").unwrap();
        let device_stem = device.as_filename_stem();
        let transaction = root.join("canonical/import-staging").join(transaction_id);
        let journal = root.join("canonical/import-journal");
        fs::create_dir_all(&journal).unwrap();
        fs::create_dir_all(root.join("canonical/events")).unwrap();
        fs::write(journal.join("import.lock"), b"").unwrap();

        let mut segments = Vec::new();
        let mut destinations = Vec::new();
        for (index, segment_bytes) in [
            b"synthetic committed marker companion 0".to_vec(),
            b"synthetic committed marker companion 1".to_vec(),
        ]
        .into_iter()
        .enumerate()
        {
            let segment_name = format!("{index:020}.seg");
            let relative = format!("{device_stem}/{segment_name}");
            let staged_path = transaction.join(&relative);
            fs::create_dir_all(staged_path.parent().unwrap()).unwrap();
            fs::write(&staged_path, &segment_bytes).unwrap();
            let segment_digest = encode_hex(&Sha256::digest(&segment_bytes));
            segments.push(PreparedImportSegment {
                device_id: device.as_str().into(),
                segment_index: index as u64,
                staged_relative_path: relative.clone(),
                destination_relative_path: relative,
                length: segment_bytes.len() as u64,
                sha256: segment_digest,
            });
            destinations.push((
                root.join("canonical/events")
                    .join(&device_stem)
                    .join(segment_name),
                segment_bytes,
            ));
        }

        let manifest = PreparedImportManifest {
            schema_version: 1,
            transaction_id: transaction_id.into(),
            segments,
        };
        validate_prepared_manifest(&manifest).unwrap();
        let manifest_bytes = serde_json_canonicalizer::to_vec(&manifest).unwrap();
        let manifest_digest = encode_hex(&Sha256::digest(&manifest_bytes));
        let manifest_path = journal.join(format!("{transaction_id}.prepared.json"));
        fs::write(&manifest_path, manifest_bytes).unwrap();
        let marker_path = journal.join(format!("{transaction_id}.committed.json"));
        let marker = CommittedImportMarker {
            protocol_version: 1,
            transaction_id: transaction_id.into(),
            prepared_manifest_sha256: marker_digest.unwrap_or(manifest_digest),
        };
        fs::write(
            &marker_path,
            serde_json_canonicalizer::to_vec(&marker).unwrap(),
        )
        .unwrap();
        (root, manifest_path, marker_path, destinations)
    }

    #[test]
    fn committed_marker_locked_snapshot_rejects_marker_or_manifest_replacement() {
        let (root, manifest_path, marker_path, _) =
            committed_marker_companion_fixture("committed-marker-locked-marker-change", None);
        let marker = read_committed_marker(&marker_path).unwrap();
        let manifest = read_prepared_manifest(&manifest_path).unwrap();
        let mut changed_marker = marker.clone();
        changed_marker.prepared_manifest_sha256 = "b".repeat(64);
        fs::write(
            &marker_path,
            serde_json_canonicalizer::to_vec(&changed_marker).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            read_bound_committed_marker_companion(&marker_path, &marker, &manifest_path, &manifest,),
            Err(ImportStagingError::RecoveryConflict(_))
        ));
        assert!(marker_path.is_file());
        assert!(manifest_path.is_file());
        fs::remove_dir_all(root).unwrap();

        let (root, manifest_path, marker_path, _) =
            committed_marker_companion_fixture("committed-marker-locked-manifest-change", None);
        let marker = read_committed_marker(&marker_path).unwrap();
        let manifest = read_prepared_manifest(&manifest_path).unwrap();
        let mut changed_manifest = manifest.clone();
        changed_manifest.segments[0].sha256 = "b".repeat(64);
        fs::write(
            &manifest_path,
            serde_json_canonicalizer::to_vec(&changed_manifest).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            read_bound_committed_marker_companion(&marker_path, &marker, &manifest_path, &manifest,),
            Err(ImportStagingError::RecoveryConflict(_))
        ));
        assert!(marker_path.is_file());
        assert!(manifest_path.is_file());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn committed_marker_companion_digest_mismatch_holds_and_preserves_evidence() {
        let (root, manifest, marker, destinations) = committed_marker_companion_fixture(
            "committed-marker-bad-manifest-digest",
            Some("b".repeat(64)),
        );

        let error = recover_import_staging(&root).unwrap_err();
        assert!(error.to_string().contains("digest does not match"));
        assert!(marker.is_file());
        assert!(manifest.is_file());
        assert!(destinations.iter().all(|(path, _)| !path.exists()));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn committed_marker_companion_classifies_absent_destinations_without_cleanup() {
        let (root, manifest, marker, destinations) =
            committed_marker_companion_fixture("committed-marker-destinations-absent", None);
        let manifest_bytes = fs::read(&manifest).unwrap();
        let marker_bytes = fs::read(&marker).unwrap();

        let error = recover_import_staging(&root).unwrap_err();
        assert!(error.to_string().contains("all destinations absent"));
        assert!(marker.is_file());
        assert!(manifest.is_file());
        assert_eq!(fs::read(&marker).unwrap(), marker_bytes);
        assert_eq!(fs::read(&manifest).unwrap(), manifest_bytes);
        assert!(destinations.iter().all(|(path, _)| !path.exists()));
        assert!(
            root.join("canonical/import-staging/123e4567-e89b-12d3-a456-426614174000")
                .exists()
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn committed_marker_cleans_only_after_all_manifest_destinations_match() {
        let (root, manifest, marker, destinations) =
            committed_marker_companion_fixture("committed-marker-destinations-match", None);
        for (destination, bytes) in &destinations {
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::write(destination, bytes).unwrap();
        }

        recover_import_staging(&root).unwrap();
        assert!(!marker.exists());
        assert!(!manifest.exists());
        assert!(
            !root
                .join("canonical/import-staging/123e4567-e89b-12d3-a456-426614174000")
                .exists()
        );
        for (destination, bytes) in destinations {
            assert_eq!(fs::read(destination).unwrap(), bytes);
        }

        assert!(root.join("canonical/import-journal/import.lock").is_file());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn committed_marker_companion_mismatched_destination_holds_and_preserves_evidence() {
        let (root, manifest, marker, destinations) =
            committed_marker_companion_fixture("committed-marker-destination-mismatch", None);
        let (destination, _) = &destinations[0];
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::write(destination, b"different destination bytes").unwrap();

        let error = recover_import_staging(&root).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("manifest file identity or length is invalid"),
            "unexpected committed-marker mismatch result: {error}"
        );
        assert!(marker.is_file());
        assert!(manifest.is_file());
        assert!(
            root.join("canonical/import-staging/123e4567-e89b-12d3-a456-426614174000")
                .exists()
        );
        assert_eq!(
            fs::read(destination).unwrap(),
            b"different destination bytes"
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn committed_marker_companion_classifies_matching_subset_without_cleanup() {
        let (root, manifest, marker, destinations) =
            committed_marker_companion_fixture("committed-marker-destination-subset", None);
        let (first_destination, first_bytes) = &destinations[0];
        fs::create_dir_all(first_destination.parent().unwrap()).unwrap();
        fs::write(first_destination, first_bytes).unwrap();

        let error = recover_import_staging(&root).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("matching destination subset visible")
        );
        assert!(marker.is_file());
        assert!(manifest.is_file());
        assert!(
            root.join("canonical/import-staging/123e4567-e89b-12d3-a456-426614174000")
                .exists()
        );
        assert_eq!(fs::read(first_destination).unwrap(), first_bytes.as_slice());
        assert!(!destinations[1].0.exists());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn committed_marker_temporary_companion_is_verified_and_preserved() {
        let (root, manifest, marker, destinations) =
            committed_marker_companion_fixture("committed-marker-temp-companion", None);
        let temporary =
            marker.with_file_name(".123e4567-e89b-12d3-a456-426614174000.committed.tmp");
        fs::rename(&marker, &temporary).unwrap();
        let marker_bytes = fs::read(&temporary).unwrap();

        let error = recover_import_staging(&root).unwrap_err();
        assert!(error.to_string().contains("all destinations absent"));
        assert!(temporary.is_file());
        assert_eq!(fs::read(&temporary).unwrap(), marker_bytes);
        assert!(manifest.is_file());
        assert!(
            root.join("canonical/import-staging/123e4567-e89b-12d3-a456-426614174000")
                .exists()
        );
        assert!(destinations.iter().all(|(path, _)| !path.exists()));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn committed_marker_temporary_completes_only_after_all_destinations_match() {
        let (root, manifest, marker, destinations) =
            committed_marker_companion_fixture("committed-marker-temp-complete", None);
        let temporary =
            marker.with_file_name(".123e4567-e89b-12d3-a456-426614174000.committed.tmp");
        fs::rename(&marker, &temporary).unwrap();
        for (destination, bytes) in &destinations {
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::write(destination, bytes).unwrap();
        }

        recover_import_staging(&root).unwrap();
        assert!(!temporary.exists());
        assert!(!marker.exists());
        assert!(!manifest.exists());
        assert!(
            !root
                .join("canonical/import-staging/123e4567-e89b-12d3-a456-426614174000")
                .exists()
        );
        for (destination, bytes) in destinations {
            assert_eq!(fs::read(destination).unwrap(), bytes);
        }

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn startup_recognizes_committed_marker_but_holds_without_replay() {
        let root = runtime_root("committed-marker-recovery-hold");
        let journal = root.join("canonical/import-journal");
        fs::create_dir_all(&journal).unwrap();
        fs::write(journal.join("import.lock"), b"").unwrap();
        let transaction = "123e4567-e89b-12d3-a456-426614174000";
        let marker_path = journal.join(format!("{transaction}.committed.json"));
        let marker = CommittedImportMarker {
            protocol_version: 1,
            transaction_id: transaction.into(),
            prepared_manifest_sha256: "a".repeat(64),
        };
        fs::write(
            &marker_path,
            serde_json_canonicalizer::to_vec(&marker).unwrap(),
        )
        .unwrap();

        let error = recover_import_staging(&root).unwrap_err();
        assert!(matches!(error, ImportStagingError::RecoveryConflict(_)));
        assert!(
            marker_path.exists(),
            "recovery must preserve marker evidence"
        );

        fs::remove_file(&marker_path).unwrap();
        let marker_temp_path = journal.join(format!(".{transaction}.committed.tmp"));
        fs::write(
            &marker_temp_path,
            serde_json_canonicalizer::to_vec(&marker).unwrap(),
        )
        .unwrap();
        let error = recover_import_staging(&root).unwrap_err();
        assert!(matches!(error, ImportStagingError::RecoveryConflict(_)));
        assert!(
            marker_temp_path.exists(),
            "recovery must preserve marker temporary evidence"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn startup_preserves_malformed_committed_marker_bytes() {
        let root = runtime_root("malformed-committed-marker-preserved");
        let journal = root.join("canonical/import-journal");
        fs::create_dir_all(&journal).unwrap();
        let transaction = "123e4567-e89b-12d3-a456-426614174000";
        let marker_path = journal.join(format!("{transaction}.committed.json"));
        let bytes = b"{malformed marker bytes";
        fs::write(&marker_path, bytes).unwrap();

        assert!(matches!(
            recover_import_staging(&root),
            Err(ImportStagingError::RecoveryConflict(_))
        ));
        assert_eq!(fs::read(&marker_path).unwrap(), bytes);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn startup_preserves_malformed_committed_marker_with_staging_present() {
        let root = runtime_root("malformed-committed-marker-with-staging");
        let journal = root.join("canonical/import-journal");
        let transaction = "123e4567-e89b-12d3-a456-426614174000";
        let staged_transaction = root.join("canonical/import-staging").join(transaction);
        fs::create_dir_all(&journal).unwrap();
        fs::create_dir_all(&staged_transaction).unwrap();
        fs::write(journal.join("import.lock"), b"").unwrap();
        let marker_path = journal.join(format!("{transaction}.committed.json"));
        let marker_bytes = b"{malformed marker bytes with staging";
        fs::write(&marker_path, marker_bytes).unwrap();
        let staged_path = staged_transaction.join("preserve.bin");
        let staged_bytes = b"synthetic staging evidence";
        fs::write(&staged_path, staged_bytes).unwrap();

        assert!(matches!(
            recover_import_staging(&root),
            Err(ImportStagingError::RecoveryConflict(_))
        ));
        assert_eq!(fs::read(&marker_path).unwrap(), marker_bytes);
        assert_eq!(fs::read(&staged_path).unwrap(), staged_bytes);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn startup_preserves_malformed_committed_marker_temporary_with_staging_present() {
        let root = runtime_root("malformed-committed-marker-temp-with-staging");
        let journal = root.join("canonical/import-journal");
        let transaction = "123e4567-e89b-12d3-a456-426614174000";
        let staged_transaction = root.join("canonical/import-staging").join(transaction);
        fs::create_dir_all(&journal).unwrap();
        fs::create_dir_all(&staged_transaction).unwrap();
        fs::write(journal.join("import.lock"), b"").unwrap();
        let marker_path = journal.join(format!(".{transaction}.committed.tmp"));
        let marker_bytes = b"{malformed marker temporary bytes";
        fs::write(&marker_path, marker_bytes).unwrap();
        let staged_path = staged_transaction.join("preserve.bin");
        let staged_bytes = b"synthetic staging evidence";
        fs::write(&staged_path, staged_bytes).unwrap();

        assert!(matches!(
            recover_import_staging(&root),
            Err(ImportStagingError::RecoveryConflict(_))
        ));
        assert_eq!(fs::read(&marker_path).unwrap(), marker_bytes);
        assert_eq!(fs::read(&staged_path).unwrap(), staged_bytes);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn startup_preserves_coexisting_committed_marker_and_temporary() {
        let root = runtime_root("committed-marker-conflict-preserved");
        let journal = root.join("canonical/import-journal");
        fs::create_dir_all(&journal).unwrap();
        let transaction = "123e4567-e89b-12d3-a456-426614174000";
        let marker_path = journal.join(format!("{transaction}.committed.json"));
        let temporary_path = journal.join(format!(".{transaction}.committed.tmp"));
        let marker = CommittedImportMarker {
            protocol_version: 1,
            transaction_id: transaction.into(),
            prepared_manifest_sha256: "a".repeat(64),
        };
        let bytes = serde_json_canonicalizer::to_vec(&marker).unwrap();
        fs::write(&marker_path, &bytes).unwrap();
        fs::write(&temporary_path, &bytes).unwrap();

        assert!(matches!(
            recover_import_staging(&root),
            Err(ImportStagingError::RecoveryConflict(_))
        ));
        assert_eq!(fs::read(&marker_path).unwrap(), bytes);
        assert_eq!(fs::read(&temporary_path).unwrap(), bytes);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn startup_holds_and_preserves_committed_marker_with_prepared_companion() {
        let (root, manifest, marker, destinations) =
            committed_marker_companion_fixture("committed-marker-with-prepared", None);
        let marker_bytes = fs::read(&marker).unwrap();
        let manifest_bytes = fs::read(&manifest).unwrap();

        let error = recover_import_staging(&root).unwrap_err();
        assert!(error.to_string().contains("all destinations absent"));
        assert_eq!(fs::read(&marker).unwrap(), marker_bytes);
        assert_eq!(fs::read(&manifest).unwrap(), manifest_bytes);
        assert!(destinations.iter().all(|(path, _)| !path.exists()));
        assert!(
            root.join("canonical/import-staging/123e4567-e89b-12d3-a456-426614174000")
                .exists()
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn xproc_committed_marker_interrupt_worker_entry() {
        let Some(root) = std::env::var_os("CRYPTEX_TEST_M8_MARKER_ROOT") else {
            return;
        };
        let root = PathBuf::from(root);
        let journal = root.join("canonical/import-journal");
        fs::create_dir_all(&journal).unwrap();
        let _ = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(journal.join("import.lock"))
            .unwrap();

        let transaction = "123e4567-e89b-12d3-a456-426614174000";
        let marker = CommittedImportMarker {
            protocol_version: 1,
            transaction_id: transaction.into(),
            prepared_manifest_sha256: "a".repeat(64),
        };
        let bytes = serde_json_canonicalizer::to_vec(&marker).unwrap();
        let temporary = journal.join(format!(".{transaction}.committed.tmp"));
        let final_path = journal.join(format!("{transaction}.committed.json"));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .unwrap();
        file.write_all(&bytes).unwrap();
        file.sync_all().unwrap();
        drop(file);
        #[cfg(test)]
        crate::test_interrupt::maybe_abrupt_exit(
            "m8_after_committed_marker_temp_sync",
            &root,
            "committed marker temporary synced before no-replace placement",
        );

        publish_staged_no_replace(&temporary, &final_path).unwrap();
        sync_staged_parent(&final_path).unwrap();
        #[cfg(test)]
        crate::test_interrupt::maybe_abrupt_exit(
            "m8_after_committed_marker_final_sync",
            &root,
            "committed marker placed and parent sync returned",
        );
        std::process::exit(98);
    }

    #[test]
    fn xproc_committed_marker_recovery_worker_entry() {
        let Some(root) = std::env::var_os("CRYPTEX_TEST_M8_MARKER_RECOVERY_ROOT") else {
            return;
        };
        let root = PathBuf::from(root);
        let _ = recover_import_staging(&root);
        std::process::exit(98);
    }

    fn run_committed_marker_recovery_interrupt(root: &Path, boundary: &str) {
        use std::process::{Command, Stdio};
        use std::time::Duration;

        let ready = root.join(crate::test_interrupt::READY_NAME);
        let _ = fs::remove_file(&ready);
        let executable = std::env::current_exe().expect("test executable path");
        let mut child = Command::new(executable)
            .args([
                "--exact",
                "import_staging::tests::xproc_committed_marker_recovery_worker_entry",
                "--nocapture",
            ])
            .env("CRYPTEX_TEST_M8_MARKER_RECOVERY_ROOT", root)
            .env(crate::test_interrupt::INTERRUPT_ENV, boundary)
            .env("RUST_TEST_THREADS", "1")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn committed-marker recovery interrupt worker");
        assert!(
            crate::test_interrupt::wait_for_path(&ready, Duration::from_secs(30)),
            "committed-marker recovery interrupt handshake not observed for {boundary}"
        );
        let status = crate::test_interrupt::wait_child_bounded(&mut child, Duration::from_secs(15))
            .expect("wait committed-marker recovery interrupt worker");
        assert_eq!(
            status.code(),
            Some(crate::test_interrupt::INTERRUPT_EXIT_CODE),
            "expected abrupt recovery interruption, got {status:?}"
        );
        assert_eq!(
            crate::test_interrupt::read_ready(root).0,
            boundary,
            "interrupt handshake names the wrong recovery boundary"
        );
    }

    #[test]
    fn xproc_recovery_after_temporary_marker_move_is_repeatable() {
        for boundary in [
            "m8_after_recovery_committed_marker_temp_move",
            "m8_after_recovery_committed_marker_final_sync",
        ] {
            let (root, manifest, marker, destinations) = committed_marker_companion_fixture(
                "committed-marker-temp-recovery-interrupt",
                None,
            );
            let temporary =
                marker.with_file_name(".123e4567-e89b-12d3-a456-426614174000.committed.tmp");
            fs::rename(&marker, &temporary).unwrap();
            for (destination, bytes) in &destinations {
                fs::create_dir_all(destination.parent().unwrap()).unwrap();
                fs::write(destination, bytes).unwrap();
            }

            run_committed_marker_recovery_interrupt(&root, boundary);
            let final_marker = marker;
            assert!(final_marker.is_file());
            assert!(!temporary.exists());
            assert!(read_committed_marker(&final_marker).is_ok());
            recover_import_staging(&root).unwrap();
            assert!(!final_marker.exists());
            assert!(!manifest.exists());
            assert!(
                !root
                    .join("canonical/import-staging/123e4567-e89b-12d3-a456-426614174000")
                    .exists()
            );
            for (destination, bytes) in destinations {
                assert_eq!(fs::read(destination).unwrap(), bytes);
            }
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn xproc_publication_interrupt_worker_entry() {
        let Some(root) = std::env::var_os("CRYPTEX_TEST_M8_PUBLICATION_ROOT") else {
            return;
        };
        let root = PathBuf::from(root);
        fs::create_dir_all(&root).unwrap();
        let (_, session, policy_verified, _, _) = prepared_policy_verified_fixture(&root);
        let _ = session.publish_verified_batch(&policy_verified).unwrap();
        std::process::exit(98);
    }

    fn run_publication_interrupt(root: &Path, boundary: &str) {
        use std::process::{Command, Stdio};
        use std::time::Duration;

        let ready = root.join(crate::test_interrupt::READY_NAME);
        let _ = fs::remove_file(&ready);
        let executable = std::env::current_exe().expect("test executable path");
        let mut child = Command::new(executable)
            .args([
                "--exact",
                "import_staging::tests::xproc_publication_interrupt_worker_entry",
                "--nocapture",
            ])
            .env("CRYPTEX_TEST_M8_PUBLICATION_ROOT", root)
            .env(crate::test_interrupt::INTERRUPT_ENV, boundary)
            .env("RUST_TEST_THREADS", "1")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn publication interrupt worker");
        assert!(
            crate::test_interrupt::wait_for_path(&ready, Duration::from_secs(60)),
            "publication interrupt handshake not observed for {boundary}"
        );
        let status = crate::test_interrupt::wait_child_bounded(&mut child, Duration::from_secs(30))
            .expect("wait publication interrupt worker");
        assert_eq!(
            status.code(),
            Some(crate::test_interrupt::INTERRUPT_EXIT_CODE),
            "expected abrupt publication interruption, got {status:?}"
        );
        assert_eq!(
            crate::test_interrupt::read_ready(root).0,
            boundary,
            "interrupt handshake names the wrong publication boundary"
        );
    }

    #[test]
    fn xproc_publication_crashes_preserve_canonical_and_journal_evidence() {
        for (boundary, marker_name) in [
            ("m8_after_canonical_segment_move_1", None),
            ("m8_after_committed_marker_temp_sync", Some("temp")),
            ("m8_after_committed_marker_final_sync", Some("final")),
        ] {
            let root = compact_runtime_root();
            run_publication_interrupt(&root, boundary);
            let events = fs::read_dir(root.join("canonical/events"))
                .unwrap()
                .filter_map(Result::ok)
                .find(|entry| entry.file_name() != "inventory.lock")
                .map(|entry| entry.path())
                .expect("synthetic canonical device directory");
            let destination = events.join("00000000000000000001.seg");
            assert!(destination.is_file(), "canonical move survives {boundary}");

            let mut expected_marker_path = None;
            if let Some(marker_name) = marker_name {
                let transaction = fs::read_dir(root.join("canonical/import-staging"))
                    .unwrap()
                    .next()
                    .unwrap()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .into_owned();
                let name = match marker_name {
                    "temp" => format!(".{transaction}.committed.tmp"),
                    "final" => format!("{transaction}.committed.json"),
                    _ => unreachable!(),
                };
                let marker_path = root.join("canonical/import-journal").join(name);
                assert!(marker_path.is_file(), "marker evidence survives {boundary}");
                assert!(read_committed_marker(&marker_path).is_ok());
                expected_marker_path = Some(marker_path);
            } else {
                assert!(
                    fs::read_dir(root.join("canonical/import-journal"))
                        .unwrap()
                        .filter_map(Result::ok)
                        .all(|entry| !entry
                            .file_name()
                            .to_string_lossy()
                            .ends_with(".committed.json")),
                    "no commit decision exists at the partial-move boundary"
                );
            }

            let recovered = recover_import_staging(&root);
            if matches!(marker_name, Some("final" | "temp")) {
                recovered.expect(
                    "valid committed marker plus complete destinations permits completion and cleanup",
                );
                assert!(
                    expected_marker_path
                        .as_ref()
                        .is_some_and(|path| !path.exists()),
                    "successful marker recovery removes only this transaction's commit record"
                );
                assert!(
                    fs::read_dir(root.join("canonical/import-staging"))
                        .unwrap()
                        .next()
                        .is_none(),
                    "successful marker recovery removes its staging transaction"
                );
                assert!(
                    fs::read_dir(root.join("canonical/import-journal"))
                        .unwrap()
                        .all(|entry| entry.unwrap().file_name() == "import.lock"),
                    "successful marker recovery leaves no transaction journal"
                );
            } else {
                assert!(
                    recovered.is_err(),
                    "startup remains fail-closed before a final marker at {boundary}"
                );
                if let Some(marker_path) = expected_marker_path.as_ref() {
                    assert!(marker_path.exists(), "recovery preserves marker evidence");
                }
            }
            assert!(destination.is_file(), "recovery preserves canonical bytes");
            fs::remove_dir_all(root).unwrap();
        }
    }

    fn run_committed_marker_interrupt(root: &Path, boundary: &str) {
        use std::process::{Command, Stdio};
        use std::time::Duration;

        let ready = root.join(crate::test_interrupt::READY_NAME);
        let _ = fs::remove_file(&ready);
        let executable = std::env::current_exe().expect("test executable path");
        let mut child = Command::new(executable)
            .args([
                "--exact",
                "import_staging::tests::xproc_committed_marker_interrupt_worker_entry",
                "--nocapture",
            ])
            .env("CRYPTEX_TEST_M8_MARKER_ROOT", root)
            .env(crate::test_interrupt::INTERRUPT_ENV, boundary)
            .env("RUST_TEST_THREADS", "1")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn committed-marker interrupt worker");
        assert!(
            crate::test_interrupt::wait_for_path(&ready, Duration::from_secs(30)),
            "committed-marker interrupt handshake not observed for {boundary}"
        );
        let status = crate::test_interrupt::wait_child_bounded(&mut child, Duration::from_secs(15))
            .expect("wait committed-marker interrupt worker");
        assert_eq!(
            status.code(),
            Some(crate::test_interrupt::INTERRUPT_EXIT_CODE),
            "expected abrupt marker interruption, got {status:?}"
        );
        assert_eq!(
            crate::test_interrupt::read_ready(root).0,
            boundary,
            "interrupt handshake names the wrong marker boundary"
        );
    }

    fn assert_committed_marker_interrupt_holds(boundary: &str, marker_name: &str) {
        let root = runtime_root(boundary);
        let journal = root.join("canonical/import-journal");
        let transaction = "123e4567-e89b-12d3-a456-426614174000";
        run_committed_marker_interrupt(&root, boundary);

        let marker_path = journal.join(marker_name.replace("{transaction}", transaction));
        assert!(
            marker_path.is_file(),
            "interrupted marker evidence is retained"
        );
        assert!(read_committed_marker(&marker_path).is_ok());
        assert!(matches!(
            recover_import_staging(&root),
            Err(ImportStagingError::RecoveryConflict(_))
        ));
        assert!(
            marker_path.exists(),
            "recovery must not remove marker evidence"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn xproc_interrupt_after_committed_marker_temp_sync_fails_closed() {
        assert_committed_marker_interrupt_holds(
            "m8_after_committed_marker_temp_sync",
            ".{transaction}.committed.tmp",
        );
    }

    #[test]
    fn xproc_interrupt_after_committed_marker_final_sync_fails_closed() {
        assert_committed_marker_interrupt_holds(
            "m8_after_committed_marker_final_sync",
            "{transaction}.committed.json",
        );
    }

    #[test]
    fn xproc_import_prepare_interrupt_worker_entry() {
        let Some(root) = std::env::var_os("CRYPTEX_TEST_M8_IMPORT_ROOT") else {
            return;
        };
        let root = PathBuf::from(root);
        let device = DeviceId::new("device-a").unwrap();
        let relative = PathBuf::from(device.as_filename_stem()).join("00000000000000000000.seg");
        let mut session = ImportStagingSession::begin(&root).unwrap();
        session.stage_segment(&relative, &device, 0).unwrap();
        session.release_staging_slot().unwrap();
        let key = SigningKey::from_bytes(&[7_u8; 32]);
        let mut resolver = TestTrustedKey(key.verifying_key());
        let verified = session
            .verify_batch_against_canonical(
                TEST_BRAIN_ID,
                std::slice::from_ref(&device),
                &mut resolver,
            )
            .unwrap();
        session.prepare_verified_manifest(&verified).unwrap();
        std::process::exit(98);
    }

    fn run_import_prepare_interrupt(root: &Path, boundary: &str) {
        use std::process::{Command, Stdio};
        use std::time::Duration;

        let ready = root.join(crate::test_interrupt::READY_NAME);
        let _ = fs::remove_file(&ready);
        let executable = std::env::current_exe().expect("test executable path");
        let mut child = Command::new(executable)
            .args([
                "--exact",
                "import_staging::tests::xproc_import_prepare_interrupt_worker_entry",
                "--nocapture",
            ])
            .env("CRYPTEX_TEST_M8_IMPORT_ROOT", root)
            .env(crate::test_interrupt::INTERRUPT_ENV, boundary)
            .env("RUST_TEST_THREADS", "1")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn import interrupt worker");
        assert!(
            crate::test_interrupt::wait_for_path(&ready, Duration::from_secs(30)),
            "import interrupt handshake not observed for {boundary}"
        );
        let status = crate::test_interrupt::wait_child_bounded(&mut child, Duration::from_secs(15))
            .expect("wait import interrupt worker");
        assert_eq!(
            status.code(),
            Some(crate::test_interrupt::INTERRUPT_EXIT_CODE),
            "expected abrupt import interruption, got {status:?}"
        );
        assert_eq!(
            crate::test_interrupt::read_ready(root).0,
            boundary,
            "interrupt handshake names the wrong boundary"
        );
    }

    fn assert_import_interrupt_recovers(boundary: &str) {
        let root = runtime_root(boundary);
        let device = DeviceId::new("device-a").unwrap();
        let relative = PathBuf::from(device.as_filename_stem()).join("00000000000000000000.seg");
        let source = root.join("transport/incoming").join(&relative);
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        let (bytes, _, _) = signed_test_segment(&device);
        fs::write(&source, bytes).unwrap();

        run_import_prepare_interrupt(&root, boundary);
        assert!(source.is_file(), "transport input remains retryable");
        recover_import_staging(&root).unwrap();
        assert_eq!(
            fs::read_dir(root.join("canonical/import-staging"))
                .unwrap()
                .count(),
            0
        );
        let journal = root.join("canonical/import-journal");
        assert_eq!(fs::read_dir(journal).unwrap().count(), 1); // import.lock only
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn xproc_interrupt_after_prepared_temp_sync_recovers_unprepared_state() {
        assert_import_interrupt_recovers("m8_after_prepared_temp_sync");
    }

    #[test]
    fn xproc_interrupt_after_prepared_manifest_sync_recovers_prepublication_state() {
        assert_import_interrupt_recovers("m8_after_prepared_manifest_sync");
    }

    #[test]
    fn snapshots_candidate_under_daemon_staging_and_hashes_bytes() {
        let root = runtime_root("copy");
        let relative = Path::new("6465766963652d61/00000000000000000002.seg");
        let source = root.join("transport/incoming").join(relative);
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        let bytes = b"bounded synthetic sealed segment bytes";
        fs::write(&source, bytes).unwrap();
        let device = DeviceId::new("device-a").unwrap();
        let mut session = ImportStagingSession::begin(&root).unwrap();
        let candidate = session.stage_segment(relative, &device, 2).unwrap().clone();

        assert_eq!(fs::read(&candidate.staged_path).unwrap(), bytes);
        assert_eq!(candidate.length, bytes.len() as u64);
        assert_eq!(candidate.sha256, encode_hex(&Sha256::digest(bytes)));
        assert!(
            candidate
                .staged_path
                .starts_with(root.join("canonical/import-staging"))
        );
        assert_eq!(session.budget().files(), 1);

        drop(session);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_incoming_file_identity_already_present_in_canonical_events() {
        let root = runtime_root("canonical-alias");
        let relative = Path::new("6465766963652d61/00000000000000000000.seg");
        let source = root.join("transport/incoming").join(relative);
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, b"synthetic hard-linked candidate").unwrap();

        let device = DeviceId::new("device-a").unwrap();
        let canonical_device = root
            .join("canonical/events")
            .join(device.as_filename_stem());
        fs::create_dir_all(&canonical_device).unwrap();
        fs::hard_link(&source, canonical_device.join("00000000000000000000.seg")).unwrap();

        let mut session = ImportStagingSession::begin(&root).unwrap();
        let error = session.stage_segment(relative, &device, 0).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("aliases an entry under canonical/events")
        );
        assert!(session.failed);

        drop(session);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn validates_staged_snapshot_schema_signature_brain_and_stream_tip() {
        let root = runtime_root("verify-segment");
        let device = DeviceId::new("device-a").unwrap();
        let (bytes, accepted, key) = signed_test_segment(&device);
        let relative = PathBuf::from(device.as_filename_stem()).join("00000000000000000000.seg");
        let source = root.join("transport/incoming").join(&relative);
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, &bytes).unwrap();

        let mut session = ImportStagingSession::begin(&root).unwrap();
        let candidate = session
            .stage_segment(&relative, &device, 0)
            .unwrap()
            .clone();
        let mut resolver = TestTrustedKey(key.verifying_key());
        let verified = session
            .verify_candidate(
                &candidate,
                TEST_BRAIN_ID,
                &ImportStreamTip::genesis(),
                &mut resolver,
            )
            .unwrap();
        assert_eq!(verified.event_count, 1);
        assert_eq!(verified.device_seq, 1);
        assert_eq!(verified.content_hash, accepted.content_hash);
        assert_eq!(verified.sha256, candidate.sha256);

        let wrong_brain = session
            .verify_candidate(
                &candidate,
                "33333333-3333-4333-8333-333333333333",
                &ImportStreamTip::genesis(),
                &mut resolver,
            )
            .unwrap_err();
        assert!(wrong_brain.to_string().contains("brain_id"));
        let wrong_tip = ImportStreamTip {
            segment_index: None,
            device_seq: Some(7),
            content_hash: ContentHashHex::parse(&"ab".repeat(32)).unwrap(),
        };
        assert!(
            session
                .verify_candidate(&candidate, TEST_BRAIN_ID, &wrong_tip, &mut resolver)
                .is_err()
        );

        drop(session);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn verifies_staged_batch_in_contiguous_segment_and_event_chain_order() {
        let root = runtime_root("verify-batch");
        let device = DeviceId::new("device-a").unwrap();
        let key = SigningKey::from_bytes(&[7_u8; 32]);
        let (first_bytes, first_event) = signed_test_segment_with_key(
            &device,
            1,
            "11111111-1111-4111-8111-111111111111",
            ContentHashHex::zero(),
            &key,
        );
        let (second_bytes, second_event) = signed_test_segment_with_key(
            &device,
            2,
            "22222222-2222-4222-8222-222222222222",
            first_event.content_hash.clone(),
            &key,
        );
        let source_root = root.join("transport/incoming");
        let first_relative =
            PathBuf::from(device.as_filename_stem()).join("00000000000000000000.seg");
        let second_relative =
            PathBuf::from(device.as_filename_stem()).join("00000000000000000001.seg");
        for (relative, bytes) in [
            (&first_relative, &first_bytes),
            (&second_relative, &second_bytes),
        ] {
            let source = source_root.join(relative);
            fs::create_dir_all(source.parent().unwrap()).unwrap();
            fs::write(source, bytes).unwrap();
        }

        let mut session = ImportStagingSession::begin(&root).unwrap();
        session.stage_segment(&first_relative, &device, 0).unwrap();
        session.stage_segment(&second_relative, &device, 1).unwrap();
        let mut resolver = TestTrustedKey(key.verifying_key());

        assert!(
            session
                .verify_batch_against_canonical(
                    TEST_BRAIN_ID,
                    std::slice::from_ref(&device),
                    &mut resolver
                )
                .is_err()
        );
        session.release_staging_slot().unwrap();
        let verified = session
            .verify_batch_against_canonical(
                TEST_BRAIN_ID,
                std::slice::from_ref(&device),
                &mut resolver,
            )
            .unwrap();
        assert_eq!(verified.segments().len(), 2);
        assert_eq!(verified.segments()[0].device_seq, 1);
        assert_eq!(verified.segments()[1].device_seq, 2);
        assert_eq!(verified.end_tips()[0].0, device);
        assert_eq!(verified.end_tips()[0].1.segment_index, Some(1));
        assert_eq!(
            verified.end_tips()[0].1.content_hash,
            second_event.content_hash
        );

        let manifest_path = session.prepare_verified_manifest(&verified).unwrap();
        let manifest_bytes = fs::read(&manifest_path).unwrap();
        let manifest: PreparedImportManifest = serde_json::from_slice(&manifest_bytes).unwrap();
        validate_prepared_manifest(&manifest).unwrap();
        assert_eq!(
            serde_json_canonicalizer::to_vec(&manifest).unwrap(),
            manifest_bytes
        );
        let mut gap = manifest.clone();
        gap.segments[1].segment_index = 3;
        assert!(validate_prepared_manifest(&gap).is_err());
        let mut unknown_field = serde_json::to_value(&manifest).unwrap();
        unknown_field["unexpected"] = serde_json::Value::Bool(true);
        assert!(serde_json::from_value::<PreparedImportManifest>(unknown_field).is_err());
        assert!(ImportStagingSession::begin(&root).is_err());
        assert!(
            session
                .discard()
                .unwrap_err()
                .to_string()
                .contains("requires startup reconciliation")
        );
        let destination = root
            .join("canonical/events")
            .join(device.as_filename_stem())
            .join("00000000000000000000.seg");
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        // A no-marker subset is hash-classified but remains held until full
        // policy-bound replay is implemented.
        let first_staged = root
            .join("canonical/import-staging")
            .join(&manifest.transaction_id)
            .join(&manifest.segments[0].staged_relative_path);
        fs::write(&destination, &first_bytes).unwrap();
        assert!(
            recover_import_staging(&root)
                .unwrap_err()
                .to_string()
                .contains("exists in both staging and canonical destinations")
        );
        assert!(first_staged.is_file());
        assert!(destination.is_file());
        fs::remove_file(&destination).unwrap();
        publish_staged_no_replace(&first_staged, &destination).unwrap();
        assert!(
            recover_import_staging(&root)
                .unwrap_err()
                .to_string()
                .contains("matching canonical destination subset is visible")
        );
        assert!(manifest_path.is_file());
        assert!(!first_staged.exists());
        assert_eq!(fs::read(&destination).unwrap(), first_bytes);

        let second_destination = root
            .join("canonical/events")
            .join(device.as_filename_stem())
            .join("00000000000000000001.seg");
        let second_staged = root
            .join("canonical/import-staging")
            .join(&manifest.transaction_id)
            .join(&manifest.segments[1].staged_relative_path);
        publish_staged_no_replace(&second_staged, &second_destination).unwrap();
        fs::remove_file(&second_destination).unwrap();
        assert!(
            recover_import_staging(&root)
                .unwrap_err()
                .to_string()
                .contains("missing from both staging and canonical destinations")
        );
        fs::write(&second_destination, &second_bytes).unwrap();
        assert!(
            recover_import_staging(&root)
                .unwrap_err()
                .to_string()
                .contains("all canonical destinations match")
        );
        assert!(manifest_path.is_file());
        assert_eq!(fs::read(&destination).unwrap(), first_bytes);
        assert_eq!(fs::read(&second_destination).unwrap(), second_bytes);

        fs::rename(&destination, &first_staged).unwrap();
        fs::rename(&second_destination, &second_staged).unwrap();

        let staged_path = first_staged;
        let staged_bytes = fs::read(&staged_path).unwrap();
        let mut tampered_bytes = staged_bytes.clone();
        tampered_bytes[0] ^= 1;
        fs::write(&staged_path, tampered_bytes).unwrap();
        assert!(matches!(
            recover_import_staging(&root),
            Err(ImportStagingError::RecoveryConflict(_))
        ));
        assert!(manifest_path.is_file());
        assert!(staged_path.is_file());

        fs::write(&staged_path, staged_bytes).unwrap();
        recover_import_staging(&root).unwrap();
        assert!(!manifest_path.exists());
        assert!(
            !root
                .join("canonical/import-staging")
                .join(&manifest.transaction_id)
                .exists()
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_policy_batch_verification_rejects_unapproved_candidate_device() {
        let root = runtime_root("verify-batch-local-policy");
        let store = crate::secure_store::FakeSecureStore::new();
        let brain = "33333333-3333-4333-8333-333333333333";
        let identity = cryptex_core::BrainIdentity::new(brain, "2026-10-29T00:00:00Z").unwrap();
        crate::identity_runtime::write_brain_identity(&root, &identity).unwrap();
        let enrollment = crate::local_device_identity::enroll_local_device(
            &root,
            &store,
            crate::secure_store::SecureStoreEnvironment::Dev,
        )
        .unwrap();
        let device = DeviceId::new(enrollment.device_id).unwrap();
        let peer = DeviceId::new("unapproved-import-peer").unwrap();
        let (candidate_bytes, _, _) = signed_test_segment(&peer);
        let relative = PathBuf::from(peer.as_filename_stem()).join("00000000000000000000.seg");
        let source = root.join("transport/incoming").join(&relative);
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, candidate_bytes).unwrap();
        assert!(source.is_file());
        let mut session = ImportStagingSession::begin(&root).unwrap();
        session.stage_segment(&relative, &peer, 0).unwrap();

        crate::canonical_writer::append_memory_note(
            &store,
            crate::secure_store::SecureStoreEnvironment::Dev,
            crate::canonical_writer::AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Public,
                text: "synthetic local trust anchor fixture",
                event_id: None,
                config: &crate::canonical_writer::CanonicalWriterConfig {
                    seal_event_threshold: 1,
                },
            },
        )
        .unwrap();
        session.release_staging_slot().unwrap();
        let anchor = crate::local_device_identity::LocalDeviceKeyResolver::load(
            &root,
            brain,
            &store,
            crate::secure_store::SecureStoreEnvironment::Dev,
        )
        .unwrap()
        .local_trust_anchor()
        .unwrap();
        let error = session
            .verify_batch_against_local_policy(brain, &anchor)
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("outside the locally approved trust set")
        );
        assert!(
            !canonical_events_device_dir(&root, &peer).exists(),
            "an unapproved transport identity must not create a canonical device directory"
        );

        drop(session);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_policy_batch_verification_accepts_contiguous_local_candidate() {
        // The local DeviceId is a UUID encoded as a filename stem, so keep the
        // synthetic root short enough for Windows MAX_PATH-compatible runs.
        let root = compact_runtime_root();
        let store = crate::secure_store::FakeSecureStore::new();
        let brain = TEST_BRAIN_ID;
        let identity = cryptex_core::BrainIdentity::new(brain, "2026-10-29T00:00:00Z").unwrap();
        crate::identity_runtime::write_brain_identity(&root, &identity).unwrap();
        let enrollment = crate::local_device_identity::enroll_local_device(
            &root,
            &store,
            crate::secure_store::SecureStoreEnvironment::Dev,
        )
        .unwrap();
        let device = DeviceId::new(enrollment.device_id).unwrap();
        let first = crate::canonical_writer::append_memory_note(
            &store,
            crate::secure_store::SecureStoreEnvironment::Dev,
            crate::canonical_writer::AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Public,
                text: "synthetic local trust anchor fixture",
                event_id: None,
                config: &crate::canonical_writer::CanonicalWriterConfig {
                    seal_event_threshold: 1,
                },
            },
        )
        .unwrap()
        .accepted;
        let ready = crate::event_signing_key::ensure_event_signing_key(
            &store,
            crate::secure_store::SecureStoreEnvironment::Dev,
            &root,
            &device,
            std::slice::from_ref(&first.signing_key_id),
            true,
        )
        .unwrap();
        let (candidate_bytes, _) = signed_test_segment_with_key(
            &device,
            2,
            "22222222-2222-4222-8222-222222222223",
            first.content_hash,
            &ready.signing_key,
        );
        let relative = PathBuf::from(device.as_filename_stem()).join("00000000000000000001.seg");
        let source = root.join("transport/incoming").join(&relative);
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, candidate_bytes).unwrap();

        let mut session = ImportStagingSession::begin(&root).unwrap();
        session.stage_segment(&relative, &device, 1).unwrap();
        session.release_staging_slot().unwrap();
        let mut resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
            &root,
            brain,
            &store,
            crate::secure_store::SecureStoreEnvironment::Dev,
        )
        .unwrap();
        let mut db = cryptex_projection::ProjectionDb::open_in_memory().unwrap();
        crate::projection::rebuild_device_projection(
            &store,
            crate::secure_store::SecureStoreEnvironment::Dev,
            &root,
            &device,
            &db,
        )
        .unwrap();
        crate::knowledge_materialization::rebuild_knowledge_from_canonical(
            &root,
            brain,
            std::slice::from_ref(&device),
            &mut resolver,
            &mut db,
        )
        .unwrap();

        let safety = crate::knowledge_materialization::evaluate_import_retrieval_safety(
            &root,
            brain,
            std::slice::from_ref(&device),
            &mut resolver,
            &db,
        );
        assert!(
            safety.allows_content(),
            "unexpected M3/M4 import safety hold: {:?}",
            safety.hold_reason()
        );
        let policy_verified = safety.verify_import_batch(&session, brain).unwrap();
        assert!(policy_verified.keeps_retrieval_snapshot());
        assert_eq!(policy_verified.verified().segments().len(), 1);
        assert_eq!(policy_verified.verified().segments()[0].device_id, device);
        assert_eq!(policy_verified.verified().segments()[0].segment_index, 1);
        assert_eq!(policy_verified.verified().segments()[0].device_seq, 2);
        assert_eq!(
            policy_verified.verified().end_tips()[0].1.device_seq(),
            Some(2)
        );
        session
            .prepare_verified_manifest(policy_verified.verified())
            .unwrap();
        policy_verified
            .validate_publication_preconditions(&session)
            .unwrap();
        let staged_path = session.candidates[0].staged_path.clone();
        let alias_path = canonical_events_device_dir(&root, &device).join("m8-alias-probe.seg");
        fs::hard_link(&staged_path, &alias_path).unwrap();
        assert!(
            policy_verified
                .validate_publication_preconditions(&session)
                .is_err(),
            "publication preflight must reject a staged file with a canonical hard-link alias"
        );
        fs::remove_file(alias_path).unwrap();
        policy_verified
            .validate_publication_preconditions(&session)
            .unwrap();
        let destination = sealed_segment_path(&root, &device, 1);
        fs::write(&destination, b"synthetic conflicting destination").unwrap();
        assert!(
            policy_verified
                .validate_publication_preconditions(&session)
                .is_err(),
            "publication preflight must refuse an occupied sealed destination"
        );
        fs::remove_file(destination).unwrap();
        let active = active_segment_path(&root, &device, 1);
        fs::write(&active, b"synthetic conflicting active segment").unwrap();
        assert!(
            policy_verified
                .validate_publication_preconditions(&session)
                .is_err(),
            "publication preflight must refuse a matching active .open destination"
        );
        fs::remove_file(active).unwrap();
        policy_verified
            .validate_publication_preconditions(&session)
            .unwrap();
        let expected_length = session.candidates[0].length;
        let marker_path = session.publish_verified_batch(&policy_verified).unwrap();
        let committed = read_committed_marker(&marker_path).unwrap();
        assert_eq!(committed.transaction_id, session.transaction_id);
        let published = sealed_segment_path(&root, &device, 1);
        assert!(
            manifest_file_matches(
                &published,
                &PreparedImportSegment {
                    device_id: device.as_str().to_owned(),
                    segment_index: 1,
                    staged_relative_path: format!(
                        "{}/00000000000000000001.seg",
                        device.as_filename_stem()
                    ),
                    destination_relative_path: format!(
                        "{}/00000000000000000001.seg",
                        device.as_filename_stem()
                    ),
                    length: expected_length,
                    sha256: policy_verified.verified().segments()[0].sha256.clone(),
                },
                false,
            )
            .unwrap()
        );
        assert!(
            !session
                .staging_dir
                .join(device.as_filename_stem())
                .join("00000000000000000001.seg")
                .exists(),
            "same-volume publication move consumes its staged source"
        );

        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (finished_tx, finished_rx) = std::sync::mpsc::channel();
        let writer_root = root.clone();
        let writer_device = device.clone();
        let competing_writer = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            let lock = crate::canonical_writer::acquire_device_canonical_lock(
                &writer_root,
                &writer_device,
            )
            .unwrap();
            finished_tx.send(()).unwrap();
            lock
        });
        started_rx.recv().unwrap();
        assert!(
            finished_rx
                .recv_timeout(std::time::Duration::from_millis(100))
                .is_err(),
            "policy-verified import token must keep the retrieval locks through publication"
        );

        drop(policy_verified);
        finished_rx.recv().unwrap();
        drop(competing_writer.join().unwrap());
        let manifest_path = root
            .join("canonical/import-journal")
            .join(format!("{}.prepared.json", session.transaction_id));
        let transaction_path = session.staging_dir.clone();
        drop(session);
        recover_import_staging(&root).unwrap();
        assert!(!marker_path.exists());
        assert!(!manifest_path.exists());
        assert!(!transaction_path.exists());
        assert!(published.is_file());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_path_traversal_and_non_segment_names() {
        let root = runtime_root("path");
        let device = DeviceId::new("device-a").unwrap();
        let mut session = ImportStagingSession::begin(&root).unwrap();
        assert!(
            session
                .stage_segment(Path::new("../escape.seg"), &device, 0)
                .is_err()
        );
        assert!(session.failed);
        drop(session);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_incoming_path_not_bound_to_device_and_index() {
        let root = runtime_root("path-binding");
        let source = root.join("transport/incoming/6465766963652d61/00000000000000000003.seg");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, b"synthetic segment").unwrap();
        let device = DeviceId::new("device-a").unwrap();
        let mut session = ImportStagingSession::begin(&root).unwrap();

        assert!(
            session
                .stage_segment(
                    Path::new("6465766963652d61/00000000000000000003.seg"),
                    &device,
                    2,
                )
                .is_err()
        );
        assert!(session.failed);
        drop(session);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn duplicate_device_segment_index_fails_and_rolls_back_batch() {
        let root = runtime_root("duplicate-index");
        let relative = Path::new("6465766963652d61/00000000000000000002.seg");
        let source = root.join("transport/incoming").join(relative);
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, b"synthetic segment").unwrap();
        let device = DeviceId::new("device-a").unwrap();
        let mut session = ImportStagingSession::begin(&root).unwrap();
        session
            .stage_segment(relative, &device, 2)
            .expect("first candidate stages");

        assert!(session.stage_segment(relative, &device, 2).is_err());
        assert!(session.failed);
        assert_eq!(
            fs::read_dir(root.join("canonical/import-staging"))
                .unwrap()
                .count(),
            0
        );
        assert!(source.exists());
        drop(session);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn serializes_staging_sessions_and_releases_slot_on_drop() {
        let root = runtime_root("single-flight");
        let first = ImportStagingSession::begin(&root).unwrap();
        assert!(ImportStagingSession::begin(&root).is_err());
        first.discard().unwrap();
        assert!(ImportStagingSession::begin(&root).is_ok());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn released_staging_slot_stays_reserved_by_the_unprepared_tree() {
        let root = runtime_root("released-slot");
        let relative = Path::new("6465766963652d61/00000000000000000000.seg");
        let source = root.join("transport/incoming").join(relative);
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, b"synthetic sealed segment").unwrap();
        let device = DeviceId::new("device-a").unwrap();
        let mut session = ImportStagingSession::begin(&root).unwrap();
        session.stage_segment(relative, &device, 0).unwrap();
        session.release_staging_slot().unwrap();

        assert!(ImportStagingSession::begin(&root).is_err());
        assert!(session.stage_segment(relative, &device, 1).is_err());
        session.discard().unwrap();
        assert!(ImportStagingSession::begin(&root).is_ok());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn transport_batch_derives_identity_and_stages_in_deterministic_order() {
        let root = runtime_root("transport-batch-order");
        let device_b = DeviceId::new("device-b").unwrap();
        let device_a = DeviceId::new("device-a").unwrap();
        let relative_b =
            PathBuf::from(device_b.as_filename_stem()).join("00000000000000000000.seg");
        let relative_a =
            PathBuf::from(device_a.as_filename_stem()).join("00000000000000000003.seg");
        for (relative, bytes) in [
            (&relative_b, b"device b synthetic segment".as_slice()),
            (&relative_a, b"device a synthetic segment".as_slice()),
        ] {
            let source = root.join("transport/incoming").join(relative);
            fs::create_dir_all(source.parent().unwrap()).unwrap();
            fs::write(source, bytes).unwrap();
        }

        let mut session = ImportStagingSession::begin(&root).unwrap();
        let staged = session
            .stage_batch_from_transport_paths(&[relative_b, relative_a])
            .unwrap();
        assert_eq!(staged.len(), 2);
        assert_eq!(staged[0].device_id, device_a);
        assert_eq!(staged[0].segment_index, 3);
        assert_eq!(staged[1].device_id, device_b);
        assert_eq!(staged[1].segment_index, 0);
        assert_eq!(session.budget().files(), 2);
        session.discard().unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn transport_path_parser_requires_canonical_identity_and_index() {
        let valid = Path::new("6465766963652d61/00000000000000000003.seg");
        assert_eq!(
            parse_transport_segment_path(valid).unwrap(),
            (DeviceId::new("device-a").unwrap(), 3)
        );

        for invalid in [
            "6465766963652D61/00000000000000000003.seg",
            "6465766963652d61/3.seg",
            "6465766963652d61/00000000000000000003.SEG",
            "6465766963652d61/18446744073709551616.seg",
            "6465766963652d61/00000000000000000003.seg/extra",
            "../6465766963652d61/00000000000000000003.seg",
        ] {
            assert!(
                parse_transport_segment_path(Path::new(invalid)).is_err(),
                "accepted invalid transport path {invalid}"
            );
        }
    }

    #[test]
    fn transport_batch_enforces_file_limit_before_candidate_parsing() {
        let root = runtime_root("transport-batch-limit");
        let excessive_paths = vec![
            PathBuf::from("invalid/path.seg");
            crate::import_limits::MAX_IMPORT_FILES_PER_BATCH + 1
        ];
        let mut session = ImportStagingSession::begin(&root).unwrap();
        assert!(matches!(
            session.stage_batch_from_transport_paths(&excessive_paths),
            Err(ImportStagingError::Limit(ImportLimitError::TooManyFiles))
        ));
        assert!(session.candidates().is_empty());
        session.discard().unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn transport_batch_rejects_duplicate_identity_before_copying() {
        let root = runtime_root("transport-batch-duplicate");
        let relative = PathBuf::from("6465766963652d61/00000000000000000003.seg");
        let mut session = ImportStagingSession::begin(&root).unwrap();
        assert!(matches!(
            session.stage_batch_from_transport_paths(&[relative.clone(), relative]),
            Err(ImportStagingError::Invalid(message))
                if message.contains("duplicate device and segment index")
        ));
        assert!(session.candidates().is_empty());
        session.discard().unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn transport_batch_failure_rolls_back_earlier_staged_candidates() {
        let root = runtime_root("transport-batch-rollback");
        let first = PathBuf::from("6465766963652d61/00000000000000000000.seg");
        let missing = PathBuf::from("6465766963652d61/00000000000000000001.seg");
        let first_source = root.join("transport/incoming").join(&first);
        fs::create_dir_all(first_source.parent().unwrap()).unwrap();
        fs::write(&first_source, b"first synthetic segment").unwrap();

        let mut session = ImportStagingSession::begin(&root).unwrap();
        assert!(
            session
                .stage_batch_from_transport_paths(&[missing.clone(), first.clone()])
                .is_err()
        );
        assert!(session.failed);
        assert!(first_source.is_file(), "transport input remains retryable");
        assert!(
            !root
                .join("canonical/import-staging")
                .join(session.transaction_id())
                .exists(),
            "a failed batch must remove the earlier staged snapshot"
        );

        drop(session);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn startup_removes_only_unprepared_staging_before_daemon_readers() {
        let root = runtime_root("startup-cleanup");
        let relative = Path::new("6465766963652d61/00000000000000000002.seg");
        let source = root.join("transport/incoming").join(relative);
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, b"synthetic segment").unwrap();
        let device = DeviceId::new("device-a").unwrap();
        let mut session = ImportStagingSession::begin(&root).unwrap();
        let transaction_id = session.transaction_id().to_string();
        session.stage_segment(relative, &device, 2).unwrap();
        drop(session);

        recover_import_staging(&root).unwrap();
        let staging_root = root.join("canonical/import-staging");
        assert_eq!(fs::read_dir(staging_root).unwrap().count(), 0);
        assert!(source.exists());
        assert_eq!(device.as_filename_stem(), "6465766963652d61");
        assert_eq!(transaction_id.len(), 36);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn startup_discards_only_a_recognized_temporary_prepared_manifest() {
        let root = runtime_root("prepared-temp-recovery");
        let relative = Path::new("6465766963652d61/00000000000000000002.seg");
        let source = root.join("transport/incoming").join(relative);
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, b"synthetic transport segment").unwrap();
        let device = DeviceId::new("device-a").unwrap();
        let mut session = ImportStagingSession::begin(&root).unwrap();
        let transaction_id = session.transaction_id().to_string();
        session.stage_segment(relative, &device, 2).unwrap();
        drop(session);

        let temp_manifest = root
            .join("canonical/import-journal")
            .join(format!(".{transaction_id}.prepared.tmp"));
        fs::write(&temp_manifest, b"partial manifest write").unwrap();

        recover_import_staging(&root).unwrap();

        assert!(!temp_manifest.exists());
        assert!(
            !root
                .join("canonical/import-staging")
                .join(transaction_id)
                .exists()
        );
        assert!(source.is_file(), "transport source remains retryable");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn startup_preserves_unrecognized_prepared_temporary_journal_entry() {
        let root = runtime_root("unknown-prepared-temp");
        let journal = root.join("canonical/import-journal");
        fs::create_dir_all(&journal).unwrap();
        let unknown_temp = journal.join(".not-a-transaction.prepared.tmp");
        fs::write(&unknown_temp, b"preserve this unknown state").unwrap();

        assert!(matches!(
            recover_import_staging(&root),
            Err(ImportStagingError::RecoveryConflict(_))
        ));
        assert_eq!(
            fs::read(&unknown_temp).unwrap(),
            b"preserve this unknown state"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn startup_fails_closed_and_preserves_unknown_journal_state() {
        let root = runtime_root("prepared-journal");
        let journal = root.join("canonical/import-journal");
        fs::create_dir_all(&journal).unwrap();
        let prepared = journal.join("prepared-transaction.json");
        fs::write(&prepared, b"untrusted fixture").unwrap();

        let error = recover_import_staging(&root).unwrap_err();
        assert!(matches!(error, ImportStagingError::RecoveryConflict(_)));
        assert_eq!(fs::read(&prepared).unwrap(), b"untrusted fixture");
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_component_handles_block_ancestor_path_rename() {
        let root = runtime_root("directory-handles");
        let moved = root.with_extension("moved");
        let handles = inspect_directory_chain(&root, true).unwrap();

        assert!(!handles.is_empty());
        assert!(fs::rename(&root, &moved).is_err());
        assert!(root.exists());
        assert!(!moved.exists());

        drop(handles);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_component_walk_rejects_junctions_without_following_them() {
        use std::process::Command;

        let root = runtime_root("junction-check");
        let outside = root.with_extension("outside");
        let junction = root.join("redirect");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let output = Command::new("cmd.exe")
            .args(["/D", "/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&outside)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "junction fixture creation failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let error = inspect_directory_chain(&junction, false).unwrap_err();
        assert!(matches!(error, ImportStagingError::Invalid(_)));

        fs::remove_dir(&junction).unwrap();
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_staged_parent_sync_rejects_junctions() {
        use std::process::Command;

        let root = runtime_root("flush-junction-check");
        let target = root.join("target");
        let junction = root.join("redirect");
        fs::create_dir_all(&target).unwrap();
        let output = Command::new("cmd.exe")
            .args(["/D", "/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&target)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "junction fixture creation failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let error = sync_staged_parent(&junction.join("entry")).unwrap_err();
        assert!(matches!(error, ImportStagingError::Invalid(_)));

        fs::remove_dir(&junction).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_committed_marker_placement_flushes_import_journal_parent() {
        use std::io::Write;

        let root = runtime_root("committed-marker-parent-flush");
        let journal = root.join("canonical/import-journal");
        fs::create_dir_all(&journal).unwrap();
        let transaction = "123e4567-e89b-12d3-a456-426614174000";
        let temporary = journal.join(format!(".{transaction}.committed.tmp"));
        let final_path = journal.join(format!("{transaction}.committed.json"));
        let marker = br#"{"protocol_version":1,"transaction_id":"123e4567-e89b-12d3-a456-426614174000","prepared_manifest_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#;

        let mut file = File::create(&temporary).unwrap();
        file.write_all(marker).unwrap();
        file.sync_all().unwrap();
        drop(file);
        publish_staged_no_replace(&temporary, &final_path).unwrap();
        sync_staged_parent(&final_path).unwrap();

        assert_eq!(fs::read(&final_path).unwrap(), marker);
        assert!(!temporary.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_committed_marker_move_never_replaces_existing_destination() {
        let root = runtime_root("committed-marker-no-replace");
        let journal = root.join("canonical/import-journal");
        fs::create_dir_all(&journal).unwrap();
        let transaction = "123e4567-e89b-12d3-a456-426614174000";
        let temporary = journal.join(format!(".{transaction}.committed.tmp"));
        let final_path = journal.join(format!("{transaction}.committed.json"));
        fs::write(&temporary, b"new marker bytes").unwrap();
        fs::write(&final_path, b"existing marker bytes").unwrap();

        assert!(publish_staged_no_replace(&temporary, &final_path).is_err());
        assert_eq!(fs::read(&final_path).unwrap(), b"existing marker bytes");
        assert_eq!(fs::read(&temporary).unwrap(), b"new marker bytes");
        fs::remove_dir_all(root).unwrap();
    }
}
