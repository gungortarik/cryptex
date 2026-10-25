//! Daemon-owned, bounded snapshots of untrusted incoming sealed segments.
//!
//! This module snapshots and verifies candidate bytes, records a prepared
//! manifest, and reconciles the pre-publication state. It does not authorize
//! device trust/cutoff policy or publish into `canonical/events`.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use cryptex_core::DeviceId;
use cryptex_events::{
    ContentHashHex, FRAME_HEADER_BYTES, FRAME_VERSION_V1, TrustedDaemonKeyResolver,
    VersionedRecord, ZERO_CONTENT_HASH_HEX, decode_frame_header, read_versioned_record,
};
use cryptex_storage::layout::canonical_events_device_dir;
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
#[cfg(windows)]
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PreparedImportManifest {
    schema_version: u32,
    transaction_id: String,
    segments: Vec<PreparedImportSegment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PreparedImportSegment {
    device_id: String,
    segment_index: u64,
    staged_relative_path: String,
    destination_relative_path: String,
    length: u64,
    sha256: String,
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
            publish_staged_no_replace(&temp_path, &final_path)?;
            sync_staged_parent(&final_path)?;
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
    })
}

/// Runs under the daemon's single-instance lock before transport, IPC, or
/// canonical readers start. Unprepared staging-only trees are removed. A
/// prepared manifest is aborted only if every staged file still matches and
/// no canonical destination exists; any visible destination remains a typed
/// fail-closed conflict until replay publication is implemented.
pub fn recover_import_staging(runtime_root: &Path) -> Result<(), ImportStagingError> {
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
        } else {
            return Err(ImportStagingError::RecoveryConflict(
                "prepared or unknown journal entries require reviewed reconciliation".into(),
            ));
        }
    }
    if !staging_exists {
        return if prepared_path.is_some() {
            Err(ImportStagingError::RecoveryConflict(
                "prepared manifest has no matching staging directory".into(),
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

    let mut transactions = fs::read_dir(&staging_root)?;
    let transaction = transactions.next().transpose()?.map(|entry| entry.path());
    if transactions.next().is_some() {
        return Err(ImportStagingError::RecoveryConflict(
            "staging contains more than one transaction directory".into(),
        ));
    }
    let Some(transaction) = transaction else {
        if prepared_path.is_some() {
            return Err(ImportStagingError::RecoveryConflict(
                "prepared manifest has no matching staging transaction".into(),
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
        )?;
    } else {
        remove_unprepared_transaction(&transaction)?;
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

fn recover_prepared_without_publication(
    runtime_root: &Path,
    transaction: &Path,
    manifest_path: &Path,
    manifest: &PreparedImportManifest,
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

    ensure_manifest_matches_staging_tree(transaction, manifest).map_err(|error| {
        ImportStagingError::RecoveryConflict(format!(
            "prepared staging-tree verification failed: {error}"
        ))
    })?;

    for segment in &manifest.segments {
        let device = DeviceId::new(segment.device_id.clone()).map_err(|error| {
            ImportStagingError::RecoveryConflict(format!(
                "prepared manifest DeviceId is invalid: {error}"
            ))
        })?;
        let relative = Path::new(&segment.staged_relative_path);
        let staged_path = transaction.join(relative);
        let destination_path = canonical_events_device_dir(runtime_root, &device)
            .join(format!("{:020}.seg", segment.segment_index));
        if entry_exists(&destination_path).map_err(|error| {
            ImportStagingError::RecoveryConflict(format!(
                "prepared destination inspection failed: {error}"
            ))
        })? {
            return Err(ImportStagingError::RecoveryConflict(
                "prepared import has visible canonical destinations; replay recovery is not implemented".into(),
            ));
        }
        let parent_handles = inspect_directory_chain(
            staged_path.parent().ok_or_else(|| {
                ImportStagingError::RecoveryConflict("prepared staged path has no parent".into())
            })?,
            true,
        )
        .map_err(|error| {
            ImportStagingError::RecoveryConflict(format!(
                "prepared staged-path inspection failed: {error}"
            ))
        })?;
        let mut staged =
            open_source_from_directory_chain(&parent_handles, &staged_path).map_err(|error| {
                ImportStagingError::RecoveryConflict(format!(
                    "prepared staged-file open failed: {error}"
                ))
            })?;
        let before = source_snapshot(&staged).map_err(|error| {
            ImportStagingError::RecoveryConflict(format!(
                "prepared staged-file metadata read failed: {error}"
            ))
        })?;
        if !before.is_regular_file || before.link_count != 1 || before.length != segment.length {
            return Err(ImportStagingError::RecoveryConflict(
                "prepared staged file identity or length changed".into(),
            ));
        }
        if canonical_file_id_alias_exists(runtime_root, before.identity).map_err(|error| {
            ImportStagingError::RecoveryConflict(format!(
                "prepared canonical alias scan failed: {error}"
            ))
        })? {
            return Err(ImportStagingError::RecoveryConflict(
                "prepared staged file aliases an entry under canonical/events".into(),
            ));
        }
        staged.seek(SeekFrom::Start(0)).map_err(|error| {
            ImportStagingError::RecoveryConflict(format!(
                "prepared staged-file rewind failed: {error}"
            ))
        })?;
        let (length, digest) = hash_bounded(&mut staged, segment.length).map_err(|error| {
            ImportStagingError::RecoveryConflict(format!(
                "prepared staged-file hash failed: {error}"
            ))
        })?;
        if length != segment.length
            || digest != segment.sha256
            || source_snapshot(&staged)? != before
        {
            return Err(ImportStagingError::RecoveryConflict(
                "prepared staged file digest or identity changed".into(),
            ));
        }
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
struct SourceSnapshot {
    length: u64,
    modified: Option<SystemTime>,
    link_count: u32,
    is_regular_file: bool,
    identity: (u64, u64, u64),
}

fn source_snapshot(file: &File) -> Result<SourceSnapshot, ImportStagingError> {
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

fn open_source_no_follow(path: &Path) -> Result<File, ImportStagingError> {
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
        // MoveFileExW(MOVEFILE_WRITE_THROUGH) is the Windows placement barrier.
        // This deliberately does not claim parent-directory FlushFileBuffers.
        let _ = path;
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
        // Even a byte-identical visible destination is not replayed until
        // publication recovery has a committed-marker implementation.
        fs::write(&destination, &first_bytes).unwrap();
        assert!(
            recover_import_staging(&root)
                .unwrap_err()
                .to_string()
                .contains("visible canonical destinations")
        );
        assert!(manifest_path.is_file());
        fs::remove_file(&destination).unwrap();
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
}
