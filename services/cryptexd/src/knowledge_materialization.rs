//! Daemon-owned, read-only canonical scan followed by a transactional M4 rebuild.
//!
//! This path is intentionally separate from M2's repair-oriented v1 reader. It
//! never truncates, quarantines, seals, or rewrites canonical packs: unknown or
//! unverifiable envelopes stop the rebuild with source bytes intact.

use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::Instant;

use cryptex_core::DeviceId;
use cryptex_events::{
    AuthenticatedSecurityRecord, CancellableSecurityHistoryError, LocalTrustAnchor,
    SecurityEventTypeV3, SecurityPayloadV3, SigningKeyId, TrustedDaemonKeyResolver,
    VerifiedSecurityHistory, VerifiedV2History, VersionedRecord,
};
use cryptex_knowledge::sqlite::{
    CanonicalFramePosition, DeviceMaterializationProgress, KNOWLEDGE_BUILD_ID_V1,
    KNOWLEDGE_REDUCER_VERSION, KNOWLEDGE_SCHEMA_VERSION, KnowledgeStoreError,
    migrate_knowledge_schema, rebuild_from_verified_history,
};
use cryptex_projection::{
    ProjectionDb, ProjectionError, SearchHit, SearchQuery, TimelinePage, TimelineQuery,
    VaultEntity, VaultFact, VaultKnowledgeView, VaultRelation, event_detail, event_detail_until,
    search_events, timeline_events, timeline_events_until, validate_schema_v1_read_only,
};
use cryptex_storage::layout::{
    active_segment_path, canonical_events_device_dir, sealed_segment_path,
};
use cryptex_storage::{AcceptAllBodies, StorageError, fold_verified_frames_with_offsets};
use sha2::{Digest, Sha256};

use crate::canonical_writer::{
    CanonicalInventoryLock, CanonicalWriterError, DeviceCanonicalLock, SecurityAnchorAppendOutcome,
    acquire_canonical_inventory_lock, acquire_canonical_inventory_lock_until,
    acquire_existing_device_canonical_lock, acquire_existing_device_canonical_lock_until,
    append_peer_approved_anchor_with_lock,
};
use crate::event_signing_key::load_existing_event_signing_key;
use crate::local_device_identity::load_local_device_identity;
use crate::secure_store::{SecureStore, SecureStoreEnvironment};

#[derive(Debug)]
pub enum KnowledgeMaterializationError {
    Canonical(CanonicalWriterError),
    Storage(StorageError),
    Verification(String),
    Knowledge(KnowledgeStoreError),
    InvalidInput(String),
    DeadlineExceeded,
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
            Self::DeadlineExceeded => write!(f, "retrieval operation deadline exceeded"),
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

/// A stable, non-sensitive reason why content retrieval was withheld.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetrievalSafetyHold {
    OperationDeadlineExceeded,
    CanonicalVerificationUnavailable,
    UnsupportedSemantics,
    V2SemanticsNotIntegrated,
    UnresolvedDependency,
    MaterializationStateMissingOrStale,
    ProjectionDoesNotMatchCanonical,
}

/// Daemon-issued, lock-scoped decision shared by all content-returning
/// adapters. Its private fields prevent callers from manufacturing an allow.
pub struct RetrievalSafetyState {
    hold: Option<RetrievalSafetyHold>,
    snapshot: Option<VerifiedCanonicalSnapshot>,
}

impl RetrievalSafetyState {
    #[must_use]
    pub fn allows_content(&self) -> bool {
        self.hold.is_none() && self.snapshot.is_some()
    }

    #[must_use]
    pub const fn hold_reason(&self) -> Option<RetrievalSafetyHold> {
        self.hold
    }

    fn held(reason: RetrievalSafetyHold) -> Self {
        Self {
            hold: Some(reason),
            snapshot: None,
        }
    }

    fn allowed(snapshot: VerifiedCanonicalSnapshot) -> Self {
        Self {
            hold: None,
            snapshot: Some(snapshot),
        }
    }

    /// Consume a successful retrieval gate, verify the staged import against
    /// its exact canonical/trust snapshot, and return a proof object that keeps
    /// all canonical locks alive. Publication must consume this object and
    /// still perform destination/journal revalidation.
    #[allow(dead_code)] // The Windows publisher is intentionally not activated yet.
    pub(crate) fn verify_import_batch(
        self,
        session: &crate::import_staging::ImportStagingSession,
        expected_brain_id: &str,
    ) -> Result<ImportPolicyVerifiedBatch, crate::import_staging::ImportStagingError> {
        let snapshot = self.snapshot.as_ref().ok_or_else(|| {
            crate::import_staging::ImportStagingError::Invalid(format!(
                "retrieval safety gate is held: {:?}",
                self.hold_reason()
            ))
        })?;
        let verified = session.verify_batch_against_snapshot(expected_brain_id, snapshot)?;
        let post_import = verify_post_import_policy(snapshot, &verified)?;
        Ok(ImportPolicyVerifiedBatch {
            verified,
            post_import,
            safety: self,
        })
    }

    /// Consume the same locked M3/M4 and M8 policy snapshot after a prepared
    /// recovery batch has been verified from its exact staged/canonical files.
    /// This creates no publication decision; it only carries the validated
    /// result and locks to a later recovery publisher.
    pub(crate) fn verify_import_recovery_batch<F>(
        self,
        candidate_devices: &[DeviceId],
        verify: F,
    ) -> Result<ImportPolicyVerifiedBatch, crate::import_staging::ImportStagingError>
    where
        F: FnOnce(
            &VerifiedCanonicalSnapshot,
        ) -> Result<
            crate::import_staging::VerifiedStagedBatch,
            crate::import_staging::ImportStagingError,
        >,
    {
        let snapshot = self.snapshot.as_ref().ok_or_else(|| {
            crate::import_staging::ImportStagingError::Invalid(format!(
                "retrieval safety gate is held: {:?}",
                self.hold_reason()
            ))
        })?;
        if candidate_devices.is_empty() || !snapshot.locks_devices(candidate_devices) {
            return Err(crate::import_staging::ImportStagingError::Invalid(
                "recovery safety snapshot does not lock every verified import device".into(),
            ));
        }
        let verified = verify(snapshot)?;
        let mut verified_devices = verified
            .segments()
            .iter()
            .map(|segment| segment.device_id.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let mut expected_devices = candidate_devices.to_vec();
        verified_devices
            .sort_by(|left, right| left.as_str().as_bytes().cmp(right.as_str().as_bytes()));
        expected_devices
            .sort_by(|left, right| left.as_str().as_bytes().cmp(right.as_str().as_bytes()));
        expected_devices.dedup();
        if verified.segments().is_empty() || verified_devices != expected_devices {
            return Err(crate::import_staging::ImportStagingError::Invalid(
                "verified recovery batch does not match its locked device set".into(),
            ));
        }
        let post_import = verify_post_import_policy(snapshot, &verified)?;
        Ok(ImportPolicyVerifiedBatch {
            verified,
            post_import,
            safety: self,
        })
    }
}

/// Unforgeable, lock-bearing result of validating staged segments against the
/// exact M3/M4 retrieval-safety snapshot plus the post-import M8 trust/cutoff
/// reduction and content state. It is not itself a publication decision.
pub(crate) struct ImportPolicyVerifiedBatch {
    verified: crate::import_staging::VerifiedStagedBatch,
    post_import: PostImportRetrievalSafetyState,
    safety: RetrievalSafetyState,
}

impl ImportPolicyVerifiedBatch {
    #[allow(dead_code)] // Read by the not-yet-enabled canonical publisher.
    pub(crate) fn verified(&self) -> &crate::import_staging::VerifiedStagedBatch {
        &self.verified
    }

    #[allow(dead_code)] // Consumed by the reviewed publisher after replay integration.
    pub(crate) fn post_import_policy(&self) -> &PostImportRetrievalSafetyState {
        &self.post_import
    }

    #[allow(dead_code)] // Explicitly keep this token alive through publication.
    pub(crate) fn keeps_retrieval_snapshot(&self) -> bool {
        self.safety.allows_content()
    }

    #[allow(dead_code)] // Checked by the inactive Windows publisher before its first move.
    pub(crate) fn keeps_post_import_policy(&self) -> bool {
        self.post_import.content.pending.is_empty()
            && self.verified.records().all(|record| {
                if record.security_control {
                    return true;
                }
                record.content_v1
                    && self
                        .post_import
                        .trust
                        .effective_cutoffs
                        .get(&record.device_id)
                        .is_some_and(|cutoff| record.device_seq <= *cutoff)
            })
    }

    /// Check no-replace publication preconditions without relinquishing the
    /// exact M3/M4 canonical locks. This remains a read-only preflight and does
    /// not authorize a canonical move.
    #[allow(dead_code)] // Consumed by the not-yet-enabled publisher.
    pub(crate) fn validate_publication_preconditions(
        &self,
        session: &crate::import_staging::ImportStagingSession,
    ) -> Result<(), crate::import_staging::ImportStagingError> {
        session.validate_publication_preconditions(&self.verified)
    }
}

/// Lock-scoped policy result for the authenticated union of the canonical
/// snapshot and one staged batch. This is validation state only; it grants no
/// publication or cleanup authority.
pub(crate) struct PostImportRetrievalSafetyState {
    pub(crate) trust: crate::trust_reducer::TrustSnapshot,
    pub(crate) content: cryptex_knowledge::KnowledgeState,
}

fn verify_post_import_policy(
    snapshot: &VerifiedCanonicalSnapshot,
    verified: &crate::import_staging::VerifiedStagedBatch,
) -> Result<PostImportRetrievalSafetyState, crate::import_staging::ImportStagingError> {
    use crate::import_staging::ImportStagingError;

    if snapshot.verified_trust_context().is_none() {
        return Err(ImportStagingError::Invalid(
            "complete local-anchor trust context is unavailable".into(),
        ));
    }
    let local_history = snapshot.trust_history.as_ref().ok_or_else(|| {
        ImportStagingError::Invalid("verified local trust identity is unavailable".into())
    })?;
    let mut raw_records =
        Vec::with_capacity(snapshot.security_frames.len() + verified.records().count());
    raw_records.extend(
        snapshot
            .security_frames
            .iter()
            .map(|frame| frame.bytes.as_slice()),
    );
    raw_records.extend(verified.records().map(|record| record.bytes.as_slice()));

    let (history, reduction, _) =
        crate::trust_reducer::verify_history_from_local_anchors_with_verified_cancellable(
            &raw_records,
            snapshot.security_history.expected_brain_id(),
            local_history.local_device.clone(),
            local_history.local_signing_key_id.clone(),
            local_history.local_public_key,
            || false,
        )
        .map_err(|hold| {
            ImportStagingError::Invalid(format!("post-import trust history is held: {hold:?}"))
        })?;
    if history.records().len() != raw_records.len() {
        return Err(ImportStagingError::Invalid(
            "post-import policy union contains duplicate canonical records".into(),
        ));
    }
    let trust = match reduction {
        crate::trust_reducer::TrustReduction::Ready(trust) => trust,
        crate::trust_reducer::TrustReduction::Held(hold) => {
            return Err(ImportStagingError::Invalid(format!(
                "post-import trust reduction is held: {hold:?}"
            )));
        }
    };
    if snapshot
        .devices
        .iter()
        .chain(verified.segments().iter().map(|segment| &segment.device_id))
        .any(|device| {
            !trust.trusted_devices.contains(device) || !trust.effective_cutoffs.contains_key(device)
        })
    {
        return Err(ImportStagingError::Invalid(
            "post-import policy removes a canonical or candidate device from the trusted cutoff set"
                .into(),
        ));
    }

    let cutoffs = trust
        .effective_cutoffs
        .iter()
        .map(|(device, cutoff)| (device.as_str().to_owned(), *cutoff))
        .collect::<std::collections::BTreeMap<_, _>>();
    let admitted_content = history
        .content_records_through_cutoffs(&cutoffs)
        .map(|record| {
            (
                record.device_id().to_owned(),
                record.device_seq(),
                record.event_id().to_owned(),
            )
        })
        .collect::<std::collections::BTreeSet<_>>();
    for record in verified.records() {
        if record.security_control {
            continue;
        }
        if !record.content_v1 {
            return Err(ImportStagingError::Invalid(
                "post-import content semantics are not integrated for this record version".into(),
            ));
        }
        let admitted = admitted_content.contains(&(
            record.device_id.as_str().to_owned(),
            record.device_seq,
            record.event_id.clone(),
        ));
        if !admitted {
            return Err(ImportStagingError::Invalid(
                "staged content lies beyond the post-import effective trust cutoff".into(),
            ));
        }
    }

    let content = cryptex_knowledge::reduce_verified_security_history_cancellable(
        snapshot.security_history.expected_brain_id(),
        &history,
        &cutoffs,
        || false,
    )
    .map_err(|error| {
        ImportStagingError::Invalid(format!("post-import content reduction failed: {error:?}"))
    })?;
    if !content.pending.is_empty()
        || content.materialized_event_ids.len()
            != history
                .content_history_through_cutoffs(&cutoffs)
                .assertions()
                .len()
    {
        return Err(ImportStagingError::Invalid(
            "post-import content has unresolved or unmaterialized assertions".into(),
        ));
    }

    Ok(PostImportRetrievalSafetyState { trust, content })
}

#[derive(Debug)]
pub enum GuardedSearchError {
    BrainWideHold(RetrievalSafetyHold),
    Query(ProjectionError),
}

#[derive(Debug)]
pub enum GuardedVaultError {
    BrainWideHold(RetrievalSafetyHold),
    Projection(ProjectionError),
}

impl std::fmt::Display for GuardedVaultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BrainWideHold(reason) => {
                write!(
                    f,
                    "vault materialization withheld by Brain-wide safety hold: {reason:?}"
                )
            }
            Self::Projection(error) => write!(f, "vault materialization failed: {error}"),
        }
    }
}

impl std::error::Error for GuardedVaultError {}

impl std::fmt::Display for GuardedSearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BrainWideHold(reason) => {
                write!(
                    f,
                    "retrieval withheld by Brain-wide safety hold: {reason:?}"
                )
            }
            Self::Query(error) => write!(f, "projection search failed: {error}"),
        }
    }
}

impl std::error::Error for GuardedSearchError {}

#[derive(Debug, Clone)]
struct RawFrame {
    position: CanonicalFramePosition,
    bytes: Vec<u8>,
}

/// A transaction-owned sealed destination that may already have been moved
/// into canonical storage when startup recovery reconstructs the pre-import
/// policy snapshot. The caller must derive this from the closed prepared
/// manifest; the scanner verifies the file bytes before omitting the segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImportRecoverySegmentExclusion {
    pub(crate) device_id: DeviceId,
    pub(crate) segment_index: u64,
    pub(crate) length: u64,
    pub(crate) sha256: String,
}

struct ImportScanScope<'a> {
    candidate_devices: &'a [DeviceId],
    recovery_exclusions: &'a [ImportRecoverySegmentExclusion],
}

// The trust-bearing fields are retained for the staged-import validator; no
// production publication path consumes them until the full M8 gate exists.
#[allow(dead_code)]
pub(crate) struct VerifiedCanonicalSnapshot {
    runtime_root: std::path::PathBuf,
    devices: Vec<DeviceId>,
    frames: Vec<RawFrame>,
    security_frames: Vec<RawFrame>,
    history: VerifiedV2History,
    // Exact canonical record bodies used to rebuild the authenticated
    // post-import trust/content union while this snapshot's locks are held.
    #[allow(dead_code)]
    security_history: VerifiedSecurityHistory,
    trust_history: Option<crate::trust_reducer::VerifiedTrustHistory>,
    trust_reduction: Option<crate::trust_reducer::TrustReduction>,
    state: cryptex_knowledge::KnowledgeState,
    progress: Vec<DeviceMaterializationProgress>,
    // Keep inventory and canonical writer locks held through the subsequent
    // SQLite safety comparison and content query.
    _inventory_lock: CanonicalInventoryLock,
    _locks: Vec<DeviceCanonicalLock>,
}

/// Complete read-only M8 policy snapshot. Canonical inventory/device locks
/// stay held while a caller inspects the authenticated union and reduction.
#[allow(dead_code)] // Awaiting RetrievalSafetyState/policy consumer integration.
pub(crate) struct VerifiedTrustPolicySnapshot {
    pub(crate) devices: Vec<DeviceId>,
    pub(crate) history: VerifiedSecurityHistory,
    verified_trust_history: crate::trust_reducer::VerifiedTrustHistory,
    pub(crate) reduction: crate::trust_reducer::TrustReduction,
    pub(crate) content_state: Option<cryptex_knowledge::KnowledgeState>,
    _inventory_lock: CanonicalInventoryLock,
    _locks: Vec<DeviceCanonicalLock>,
}

impl VerifiedTrustPolicySnapshot {
    fn device_lock(&self, device_id: &DeviceId) -> Option<&DeviceCanonicalLock> {
        self._locks
            .iter()
            .find(|lock| lock.device_id() == device_id)
    }

    /// Freeze a cross-device restriction request from the complete
    /// lock-scoped policy snapshot. This creates display/digest data only; it
    /// does not sign or append a canonical event.
    #[allow(dead_code)] // Awaiting native owner-confirmation and writer wiring.
    pub(crate) fn device_restriction_approval_snapshot(
        &self,
        transaction_id: String,
        target: &DeviceId,
        reason: crate::bootstrap_journal::RestrictionReason,
        cutoff_seq: u64,
    ) -> KnowledgeMaterializationResult<crate::bootstrap_journal::ApprovalSnapshot> {
        use crate::trust_reducer::TrustReduction;

        let TrustReduction::Ready(trust) = &self.reduction else {
            return Err(KnowledgeMaterializationError::Verification(
                "M8 owner operation requires a ready trust reduction".into(),
            ));
        };
        if self.verified_trust_history.state != crate::trust_reducer::VerifiedHistoryState::Complete
            || target == &self.verified_trust_history.local_device
            || !self
                .verified_trust_history
                .directly_approved_peers
                .contains(target)
            || !trust.trusted_devices.contains(target)
        {
            return Err(KnowledgeMaterializationError::Verification(
                "restriction target is not a currently trusted, directly approved peer".into(),
            ));
        }
        let binding = self
            .verified_trust_history
            .approved_peer_binding(target)
            .ok_or_else(|| {
                KnowledgeMaterializationError::Verification(
                    "restriction target has no verified local approval binding".into(),
                )
            })?;
        let effective_cutoff = trust.effective_cutoffs.get(target).ok_or_else(|| {
            KnowledgeMaterializationError::Verification(
                "restriction target has no effective trusted cutoff".into(),
            )
        })?;
        if cutoff_seq > *effective_cutoff {
            return Err(KnowledgeMaterializationError::InvalidInput(
                "restriction cutoff cannot exceed the currently authorized prefix".into(),
            ));
        }
        let cutoff_content_hash = if cutoff_seq == 0 {
            cryptex_events::ZERO_CONTENT_HASH_HEX.to_owned()
        } else {
            let boundary = self
                .history
                .records()
                .iter()
                .find(|record| {
                    record.device_id() == target.as_str() && record.device_seq() == cutoff_seq
                })
                .ok_or_else(|| {
                    KnowledgeMaterializationError::Verification(
                        "restriction cutoff is not an exact verified target boundary".into(),
                    )
                })?;
            if boundary.signing_key_id() != &binding.signing_key_id {
                return Err(KnowledgeMaterializationError::Verification(
                    "restriction boundary does not use the directly approved event key".into(),
                ));
            }
            boundary.content_hash().as_str().to_owned()
        };
        self.reject_reused_security_transaction_id(&transaction_id)?;

        let snapshot = crate::bootstrap_journal::ApprovalSnapshot {
            brain_id: self.history.expected_brain_id().to_owned(),
            transaction_id,
            role: crate::bootstrap_journal::BootstrapRole::Issuer,
            operation: crate::bootstrap_journal::BootstrapOperation::DeviceRestriction,
            agreement_hash: None,
            previous_message_hash: None,
            local_identity: self.local_approval_identity(),
            peer_identity: Some(crate::bootstrap_journal::ApprovalIdentity {
                device_id: binding.device_id.as_str().to_owned(),
                signing_key_id: binding.signing_key_id.as_str().to_owned(),
                public_key_fingerprint_sha256: lower_hex(&binding.fingerprint_sha256),
            }),
            transport_association: Some(crate::bootstrap_journal::TransportAssociation {
                local_device_id: binding.local_transport_device_id.clone(),
                peer_device_id: binding.transport_device_id.clone(),
            }),
            restriction_target: Some(target.as_str().to_owned()),
            restriction_reason: Some(reason),
            cutoff_seq: Some(cutoff_seq.to_string()),
            cutoff_content_hash: Some(cutoff_content_hash),
            last_authorized_seq: None,
            last_authorized_content_hash: None,
        };
        snapshot
            .digest()
            .map_err(|error| KnowledgeMaterializationError::InvalidInput(error.to_string()))?;
        Ok(snapshot)
    }

    /// Freeze local self-retirement against the exact current local chain tip.
    /// The result is an owner-visible request only, not a signed event.
    #[allow(dead_code)] // Awaiting native owner-confirmation and writer wiring.
    pub(crate) fn self_retirement_approval_snapshot(
        &self,
        transaction_id: String,
    ) -> KnowledgeMaterializationResult<crate::bootstrap_journal::ApprovalSnapshot> {
        use crate::trust_reducer::{TrustReduction, VerifiedHistoryState};

        if !matches!(self.reduction, TrustReduction::Ready(_))
            || self.verified_trust_history.state != VerifiedHistoryState::Complete
        {
            return Err(KnowledgeMaterializationError::Verification(
                "M8 owner operation requires a complete, ready trust history".into(),
            ));
        }
        if self
            .verified_trust_history
            .self_retirements
            .iter()
            .any(|retirement| retirement.device == self.verified_trust_history.local_device)
        {
            return Err(KnowledgeMaterializationError::InvalidInput(
                "local device already has a self-retirement event".into(),
            ));
        }
        let previous = self
            .history
            .records()
            .iter()
            .filter(|record| {
                record.device_id() == self.verified_trust_history.local_device.as_str()
            })
            .max_by_key(|record| record.device_seq());
        let (last_authorized_seq, last_authorized_content_hash) = previous
            .map(|record| {
                (
                    record.device_seq(),
                    record.content_hash().as_str().to_owned(),
                )
            })
            .unwrap_or((0, cryptex_events::ZERO_CONTENT_HASH_HEX.to_owned()));
        self.reject_reused_security_transaction_id(&transaction_id)?;

        let snapshot = crate::bootstrap_journal::ApprovalSnapshot {
            brain_id: self.history.expected_brain_id().to_owned(),
            transaction_id,
            role: crate::bootstrap_journal::BootstrapRole::Issuer,
            operation: crate::bootstrap_journal::BootstrapOperation::SelfRetirement,
            agreement_hash: None,
            previous_message_hash: None,
            local_identity: self.local_approval_identity(),
            peer_identity: None,
            transport_association: None,
            restriction_target: None,
            restriction_reason: None,
            cutoff_seq: None,
            cutoff_content_hash: None,
            last_authorized_seq: Some(last_authorized_seq.to_string()),
            last_authorized_content_hash: Some(last_authorized_content_hash),
        };
        snapshot
            .digest()
            .map_err(|error| KnowledgeMaterializationError::InvalidInput(error.to_string()))?;
        Ok(snapshot)
    }

    fn local_approval_identity(&self) -> crate::bootstrap_journal::ApprovalIdentity {
        use sha2::{Digest, Sha256};

        crate::bootstrap_journal::ApprovalIdentity {
            device_id: self.verified_trust_history.local_device.as_str().to_owned(),
            signing_key_id: self
                .verified_trust_history
                .local_signing_key_id
                .as_str()
                .to_owned(),
            public_key_fingerprint_sha256: lower_hex(&Sha256::digest(
                self.verified_trust_history.local_public_key,
            )),
        }
    }

    fn reject_reused_security_transaction_id(
        &self,
        transaction_id: &str,
    ) -> KnowledgeMaterializationResult<()> {
        if self.history.records().iter().any(|record| {
            let cryptex_events::AuthenticatedSecurityRecord::SecurityV3(event) = record else {
                return false;
            };
            match event.payload() {
                cryptex_events::SecurityPayloadV3::PeerApproved {
                    transaction_id: existing,
                    ..
                }
                | cryptex_events::SecurityPayloadV3::DeviceRestricted {
                    transaction_id: existing,
                    ..
                }
                | cryptex_events::SecurityPayloadV3::DeviceSelfRetired {
                    transaction_id: existing,
                    ..
                } => existing == transaction_id,
            }
        }) {
            return Err(KnowledgeMaterializationError::InvalidInput(
                "security transaction ID already exists in canonical history".into(),
            ));
        }
        Ok(())
    }
}

fn lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

impl VerifiedCanonicalSnapshot {
    pub(crate) fn locks_devices(&self, devices: &[DeviceId]) -> bool {
        devices.iter().all(|device| {
            self.devices
                .binary_search_by(|existing| {
                    existing.as_str().as_bytes().cmp(device.as_str().as_bytes())
                })
                .is_ok()
        })
    }

    // Consumed by the validation-only staged-import gate, not by publication.
    #[allow(dead_code)]
    pub(crate) fn verified_trust_context(
        &self,
    ) -> Option<(
        &crate::trust_reducer::VerifiedTrustHistory,
        &crate::trust_reducer::TrustSnapshot,
    )> {
        let history = self.trust_history.as_ref()?;
        let Some(crate::trust_reducer::TrustReduction::Ready(trust)) =
            self.trust_reduction.as_ref()
        else {
            return None;
        };
        Some((history, trust))
    }

    // The import flow remains closed until recovery and publication are ready.
    #[allow(dead_code)]
    pub(crate) fn import_starting_tips_from_trust(
        &self,
        affected_devices: &[DeviceId],
        trust: &crate::trust_reducer::TrustSnapshot,
    ) -> KnowledgeMaterializationResult<Vec<(DeviceId, crate::import_staging::ImportStreamTip)>>
    {
        let mut affected = affected_devices.to_vec();
        affected.sort_by(|left, right| left.as_str().as_bytes().cmp(right.as_str().as_bytes()));
        if affected.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(KnowledgeMaterializationError::InvalidInput(
                "import candidate device list contains duplicates".into(),
            ));
        }

        let mut tips = affected
            .iter()
            .map(|device| {
                if !trust.trusted_devices.contains(device) {
                    return Err(KnowledgeMaterializationError::Verification(
                        "import candidate device is outside the locally approved trust set".into(),
                    ));
                }
                if canonical_device_has_open_segment(&self.runtime_root, device)? {
                    return Err(KnowledgeMaterializationError::InvalidInput(
                        "import cannot extend a device with an active canonical segment".into(),
                    ));
                }
                Ok((
                    device.clone(),
                    crate::import_staging::ImportStreamTip::genesis(),
                ))
            })
            .collect::<KnowledgeMaterializationResult<Vec<_>>>()?;

        for record in self.security_history.records() {
            let device_id = record.device_id();
            let Some((_, tip)) = tips
                .iter_mut()
                .find(|(device, _)| device.as_str() == device_id)
            else {
                continue;
            };
            let frame = self
                .security_frames
                .iter()
                .find(|frame| {
                    frame.position.device_id == device_id
                        && frame.position.event_id == record.event_id()
                })
                .ok_or_else(|| {
                    KnowledgeMaterializationError::InvalidInput(
                        "verified trust record is missing its canonical frame".into(),
                    )
                })?;
            tip.segment_index = Some(frame.position.pack_index);
            tip.device_seq = Some(record.device_seq());
            tip.content_hash = record.content_hash().clone();
        }
        Ok(tips)
    }

    /// Reverify the exact lock-scoped frame set using the local key and the
    /// peer roster reconstructed only from local approval anchors. This is a
    /// read-only policy result; retrieval and import authorization remain
    /// separate gates.
    #[allow(dead_code)] // Policy consumer activation is separately gated.
    pub(crate) fn trust_reduction(
        &self,
        local_device: DeviceId,
        local_signing_key_id: &SigningKeyId,
        local_public_key: &[u8; 32],
    ) -> crate::trust_reducer::TrustReduction {
        let raw_records = self
            .frames
            .iter()
            .map(|frame| frame.bytes.as_slice())
            .collect::<Vec<_>>();
        let verified = crate::trust_reducer::verify_history_from_local_anchors(
            &raw_records,
            self.security_history.expected_brain_id(),
            local_device,
            local_signing_key_id.clone(),
            *local_public_key,
        );
        match verified {
            Ok((_, reduction)) => match &reduction {
                crate::trust_reducer::TrustReduction::Ready(trust)
                    if self
                        .devices
                        .iter()
                        .all(|device| trust.trusted_devices.contains(device)) =>
                {
                    reduction
                }
                crate::trust_reducer::TrustReduction::Ready(_) => {
                    crate::trust_reducer::TrustReduction::Held(
                        crate::trust_reducer::TrustHold::MissingOrUnverifiableHistory,
                    )
                }
                crate::trust_reducer::TrustReduction::Held(_) => reduction,
            },
            Err(hold) => crate::trust_reducer::TrustReduction::Held(hold),
        }
    }

    /// Derive starting tips only from a complete versioned scan whose
    /// inventory and device locks remain held by this snapshot.
    pub(crate) fn import_starting_tips(
        &self,
        affected_devices: &[DeviceId],
    ) -> KnowledgeMaterializationResult<Vec<(DeviceId, crate::import_staging::ImportStreamTip)>>
    {
        let mut affected = affected_devices.to_vec();
        affected.sort_by(|left, right| left.as_str().as_bytes().cmp(right.as_str().as_bytes()));
        if affected.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(KnowledgeMaterializationError::InvalidInput(
                "import candidate device list contains duplicates".into(),
            ));
        }

        let mut tips = affected
            .iter()
            .map(|device| {
                if !self.devices.contains(device) {
                    return Err(KnowledgeMaterializationError::InvalidInput(
                        "import device is not in the supplied trusted-device inventory".into(),
                    ));
                }
                if canonical_device_has_open_segment(&self.runtime_root, device)? {
                    return Err(KnowledgeMaterializationError::InvalidInput(
                        "import cannot extend a device with an active canonical segment".into(),
                    ));
                }
                Ok((
                    device.clone(),
                    crate::import_staging::ImportStreamTip::genesis(),
                ))
            })
            .collect::<KnowledgeMaterializationResult<Vec<_>>>()?;

        for (record, frame) in self.history.records().iter().zip(&self.frames) {
            let Some((_, device_id, content_hash)) = record_identity(record) else {
                return Err(KnowledgeMaterializationError::InvalidInput(
                    "complete canonical snapshot contains an unverifiable record".into(),
                ));
            };
            let Some((_, tip)) = tips
                .iter_mut()
                .find(|(device, _)| device.as_str() == device_id)
            else {
                continue;
            };
            let device_seq = record_device_seq(record)?;
            tip.segment_index = Some(frame.position.pack_index);
            tip.device_seq = Some(device_seq);
            tip.content_hash =
                cryptex_events::ContentHashHex::parse(&content_hash).map_err(|error| {
                    KnowledgeMaterializationError::InvalidInput(format!(
                        "verified canonical content hash is invalid: {error}"
                    ))
                })?;
        }
        Ok(tips)
    }
}

/// Rebuild all M4-derived rows from an explicit set of trusted device streams.
///
/// The caller supplies the enrolled-device key resolver; this function does
/// not infer trust from event contents or implement device enrollment. Locks
/// are acquired in stable DeviceId order and held through verification,
/// reduction, and the SQLite transaction (device locks â†’ SQLite).
pub fn rebuild_knowledge_from_canonical<R: TrustedDaemonKeyResolver>(
    runtime_root: &Path,
    expected_brain_id: &str,
    trusted_devices: &[DeviceId],
    key_resolver: &mut R,
    db: &mut ProjectionDb,
) -> KnowledgeMaterializationResult<KnowledgeRebuildReport> {
    let snapshot = scan_verified_canonical(
        runtime_root,
        expected_brain_id,
        trusted_devices,
        key_resolver,
    )?;
    // Migrate only after the complete canonical set verified, so an unknown or
    // unverifiable envelope leaves both canonical packs and M3/M4 DB untouched.
    migrate_knowledge_schema(db.connection_mut())?;
    rebuild_from_verified_history(
        db.connection_mut(),
        expected_brain_id,
        &snapshot.history,
        &snapshot
            .frames
            .iter()
            .map(|frame| frame.position.clone())
            .collect::<Vec<_>>(),
        &snapshot.progress,
    )?;

    Ok(KnowledgeRebuildReport {
        verified_records: snapshot.history.records().len(),
        materialized_assertions: snapshot.history.assertions().len(),
        pending_dependencies: snapshot.state.pending.len(),
        device_count: snapshot.devices.len(),
    })
}

pub(crate) fn scan_verified_canonical<R: TrustedDaemonKeyResolver>(
    runtime_root: &Path,
    expected_brain_id: &str,
    trusted_devices: &[DeviceId],
    key_resolver: &mut R,
) -> KnowledgeMaterializationResult<VerifiedCanonicalSnapshot> {
    scan_verified_canonical_until(
        runtime_root,
        expected_brain_id,
        trusted_devices,
        key_resolver,
        None,
    )
}

/// Verify a complete, caller-enumerated canonical inventory from this
/// installation's local approval anchors. The caller must obtain the local
/// public key from the same OS-held key material used for canonical event
/// signing. This read-only path does not authorize publication or retrieval.
#[allow(dead_code)] // Deliberately separate until guarded-content activation gates pass.
pub(crate) fn scan_verified_trust_policy(
    runtime_root: &Path,
    expected_brain_id: &str,
    local_device: &DeviceId,
    local_signing_key_id: &SigningKeyId,
    local_public_key: &[u8; 32],
    complete_devices: &[DeviceId],
) -> KnowledgeMaterializationResult<VerifiedTrustPolicySnapshot> {
    crate::paths::assert_outside_repository(runtime_root)
        .map_err(|error| KnowledgeMaterializationError::InvalidInput(error.to_string()))?;
    let mut devices = complete_devices.to_vec();
    devices.sort_by(|left, right| left.as_str().as_bytes().cmp(right.as_str().as_bytes()));
    if devices.windows(2).any(|pair| pair[0] == pair[1]) || !devices.contains(local_device) {
        return Err(KnowledgeMaterializationError::InvalidInput(
            "M8 policy scan requires a unique complete inventory containing the local device"
                .into(),
        ));
    }

    let inventory_lock = acquire_canonical_inventory_lock(runtime_root)?;
    require_complete_device_inventory(runtime_root, &devices, None)?;
    let mut locks = Vec::with_capacity(devices.len());
    for device in &devices {
        if let Some(lock) = acquire_existing_device_canonical_lock(runtime_root, device)? {
            locks.push(lock);
        }
    }
    require_complete_device_inventory(runtime_root, &devices, None)?;

    let mut frames = Vec::<RawFrame>::new();
    for device in &devices {
        scan_device_read_only(runtime_root, device, &mut frames, None)?;
    }
    require_complete_device_inventory(runtime_root, &devices, None)?;

    let raw_records = frames
        .iter()
        .map(|frame| frame.bytes.as_slice())
        .collect::<Vec<_>>();
    let (history, mut reduction, verified_trust_history) =
        crate::trust_reducer::verify_history_from_local_anchors_with_verified_cancellable(
            &raw_records,
            expected_brain_id,
            local_device.clone(),
            local_signing_key_id.clone(),
            *local_public_key,
            || false,
        )
        .map_err(|hold| {
            KnowledgeMaterializationError::Verification(format!("M8 trust history held: {hold:?}"))
        })?;

    if history.records().len() != frames.len() {
        return Err(KnowledgeMaterializationError::InvalidInput(
            "M8 scan contains duplicate canonical records; refusing ambiguous frame binding".into(),
        ));
    }
    if let crate::trust_reducer::TrustReduction::Ready(trust) = &reduction
        && devices
            .iter()
            .any(|device| !trust.trusted_devices.contains(device))
    {
        reduction = crate::trust_reducer::TrustReduction::Held(
            crate::trust_reducer::TrustHold::MissingOrUnverifiableHistory,
        );
    }

    let content_state = match &reduction {
        crate::trust_reducer::TrustReduction::Ready(trust) => {
            let cutoffs = trust
                .effective_cutoffs
                .iter()
                .map(|(device, cutoff)| (device.as_str().to_owned(), *cutoff))
                .collect();
            match cryptex_knowledge::reduce_verified_security_history_cancellable(
                expected_brain_id,
                &history,
                &cutoffs,
                || false,
            ) {
                Ok(state) => Some(state),
                Err(cryptex_knowledge::ReducerError::Cancelled) => {
                    return Err(KnowledgeMaterializationError::DeadlineExceeded);
                }
                Err(_) => {
                    reduction = crate::trust_reducer::TrustReduction::Held(
                        crate::trust_reducer::TrustHold::InvalidPolicyHistory,
                    );
                    None
                }
            }
        }
        crate::trust_reducer::TrustReduction::Held(_) => None,
    };

    Ok(VerifiedTrustPolicySnapshot {
        devices,
        history,
        verified_trust_history,
        reduction,
        content_state,
        _inventory_lock: inventory_lock,
        _locks: locks,
    })
}

/// Append one locally approved peer anchor after a complete transcript has
/// been journaled. This operation holds the entire canonical inventory and
/// device lock set while checking global event IDs, local-stream transaction
/// IDs, the trust reducer, and the final append reread.
pub(crate) fn append_local_peer_approval_anchor<S: SecureStore + ?Sized>(
    runtime_root: &Path,
    expected_brain_id: &str,
    store: &S,
    env: SecureStoreEnvironment,
    transaction_id: &str,
    raw_messages: &[Vec<u8>],
) -> KnowledgeMaterializationResult<SecurityAnchorAppendOutcome> {
    append_local_peer_approval_anchor_with_mode(
        runtime_root,
        expected_brain_id,
        store,
        env,
        transaction_id,
        raw_messages,
        AnchorAppendMode::ExplicitCompletion,
    )
}

/// Append a non-admission owner operation only after the authenticated IPC
/// owner-confirmation flow has consumed its one-use capability and persisted
/// `owner_approved`. This method acquires the complete canonical lock set,
/// rederives the exact owner-visible snapshot, and refuses stale or changed
/// history before sequence allocation.
#[allow(dead_code)] // Wired by the Windows owner-operation IPC/UI slice.
pub(crate) fn append_confirmed_owner_security_operation<S: SecureStore + ?Sized>(
    runtime_root: &Path,
    expected_brain_id: &str,
    store: &S,
    env: SecureStoreEnvironment,
    transaction_id: &str,
) -> KnowledgeMaterializationResult<SecurityAnchorAppendOutcome> {
    use crate::bootstrap_journal::{BootstrapOperation, BootstrapState};
    use crate::trust_reducer::TrustReduction;

    crate::paths::assert_outside_repository(runtime_root)
        .map_err(|error| KnowledgeMaterializationError::InvalidInput(error.to_string()))?;
    let local_identity = load_local_device_identity(runtime_root, expected_brain_id)
        .map_err(|error| KnowledgeMaterializationError::Verification(error.to_string()))?;
    let local_device = DeviceId::new(local_identity.device_id)
        .map_err(|error| KnowledgeMaterializationError::InvalidInput(error.to_string()))?;
    let ready = load_existing_event_signing_key(store, env, runtime_root, &local_device)
        .map_err(|error| KnowledgeMaterializationError::Verification(error.to_string()))?;
    let local_key_id = SigningKeyId::parse(&ready.public.signing_key_id)
        .map_err(|error| KnowledgeMaterializationError::Verification(error.to_string()))?;
    let journal = crate::bootstrap_journal::BootstrapJournalStore::new(runtime_root)
        .map_err(|error| KnowledgeMaterializationError::Verification(error.to_string()))?;
    let initial_journal = journal
        .load()
        .map_err(|error| KnowledgeMaterializationError::Verification(error.to_string()))?;
    let initial_transaction = initial_journal
        .transactions
        .iter()
        .find(|transaction| transaction.snapshot.transaction_id == transaction_id)
        .cloned()
        .ok_or_else(|| {
            KnowledgeMaterializationError::Verification(
                "owner security operation is absent from the daemon journal".into(),
            )
        })?;
    if initial_transaction.state != BootstrapState::OwnerApproved
        || initial_transaction.snapshot.operation == BootstrapOperation::PeerAdmission
        || initial_transaction.snapshot.brain_id != expected_brain_id
        || initial_transaction.snapshot.local_identity.device_id != local_device.as_str()
        || initial_transaction.snapshot.local_identity.signing_key_id != local_key_id.as_str()
    {
        return Err(KnowledgeMaterializationError::Verification(
            "owner operation lacks exact durable confirmation for this local identity".into(),
        ));
    }

    let complete_devices = read_canonical_device_inventory(runtime_root, None)?;
    let policy = scan_verified_trust_policy(
        runtime_root,
        expected_brain_id,
        &local_device,
        &local_key_id,
        &ready.signing_key.verifying_key().to_bytes(),
        &complete_devices,
    )?;
    let TrustReduction::Ready(trust) = &policy.reduction else {
        return Err(KnowledgeMaterializationError::Verification(
            "local trust policy is held; owner operation append refused".into(),
        ));
    };
    if policy
        .devices
        .iter()
        .any(|device| !trust.trusted_devices.contains(device))
    {
        return Err(KnowledgeMaterializationError::Verification(
            "canonical inventory contains a device outside the local trust set".into(),
        ));
    }

    let snapshot = &initial_transaction.snapshot;
    let current_snapshot = match snapshot.operation {
        BootstrapOperation::DeviceRestriction => {
            let target = snapshot
                .restriction_target
                .as_deref()
                .and_then(|target| DeviceId::new(target.to_owned()).ok())
                .ok_or_else(|| {
                    KnowledgeMaterializationError::Verification(
                        "journaled restriction target is invalid".into(),
                    )
                })?;
            let reason = snapshot.restriction_reason.ok_or_else(|| {
                KnowledgeMaterializationError::Verification(
                    "journaled restriction reason is missing".into(),
                )
            })?;
            let cutoff = snapshot
                .cutoff_seq
                .as_deref()
                .and_then(|value| value.parse::<u64>().ok())
                .ok_or_else(|| {
                    KnowledgeMaterializationError::Verification(
                        "journaled restriction cutoff is invalid".into(),
                    )
                })?;
            policy.device_restriction_approval_snapshot(
                snapshot.transaction_id.clone(),
                &target,
                reason,
                cutoff,
            )?
        }
        BootstrapOperation::SelfRetirement => {
            policy.self_retirement_approval_snapshot(snapshot.transaction_id.clone())?
        }
        BootstrapOperation::PeerAdmission => {
            return Err(KnowledgeMaterializationError::Verification(
                "peer admission is not an owner security operation".into(),
            ));
        }
    };
    if current_snapshot != *snapshot
        || current_snapshot.digest().ok().as_deref()
            != Some(initial_transaction.approval_request_digest.as_str())
    {
        return Err(KnowledgeMaterializationError::Verification(
            "owner approval is stale; canonical history changed after display".into(),
        ));
    }

    // The journal is a hint. Re-read it after acquiring canonical locks so a
    // concurrent cancellation or approval transition cannot race the append.
    let current_journal = journal
        .load()
        .map_err(|error| KnowledgeMaterializationError::Verification(error.to_string()))?;
    let current_transaction = current_journal
        .transactions
        .iter()
        .find(|transaction| transaction.snapshot.transaction_id == transaction_id)
        .ok_or_else(|| {
            KnowledgeMaterializationError::Verification(
                "owner operation disappeared while canonical locks were acquired".into(),
            )
        })?;
    if current_transaction != &initial_transaction {
        return Err(KnowledgeMaterializationError::Verification(
            "owner operation journal changed during canonical validation".into(),
        ));
    }
    if policy
        .history
        .records()
        .iter()
        .any(|record| record.event_id() == initial_transaction.stable_event_id)
    {
        return Err(KnowledgeMaterializationError::Verification(
            "stable owner-operation event ID already exists in canonical history".into(),
        ));
    }
    let local_lock = policy.device_lock(&local_device).ok_or_else(|| {
        KnowledgeMaterializationError::Verification(
            "local canonical stream lock is unavailable".into(),
        )
    })?;
    let target_public_key = match snapshot.operation {
        BootstrapOperation::DeviceRestriction => {
            let target = snapshot
                .restriction_target
                .as_deref()
                .and_then(|target| DeviceId::new(target.to_owned()).ok())
                .ok_or_else(|| {
                    KnowledgeMaterializationError::Verification(
                        "journaled restriction target is invalid".into(),
                    )
                })?;
            Some(
                &policy
                    .verified_trust_history
                    .approved_peer_binding(&target)
                    .ok_or_else(|| {
                        KnowledgeMaterializationError::Verification(
                            "restriction peer approval binding disappeared".into(),
                        )
                    })?
                    .public_key,
            )
        }
        BootstrapOperation::SelfRetirement => None,
        BootstrapOperation::PeerAdmission => unreachable!("rejected above"),
    };
    crate::canonical_writer::append_owner_security_operation_with_lock(
        crate::canonical_writer::OwnerSecurityOperationAppendArgs {
            runtime_root,
            device_id: &local_device,
            lock: local_lock,
            signing_key: &ready.signing_key,
            signing_key_id: &ready.public.signing_key_id,
            snapshot,
            target_public_key,
            event_id: &initial_transaction.stable_event_id,
            journal: &journal,
        },
    )
    .map_err(KnowledgeMaterializationError::Canonical)
}

/// Build the owner-visible immutable request from the complete lock-scoped
/// local trust history. Renderer fields identify only the requested action;
/// identities, peer keys, transport associations, and cutoff hashes come from
/// verified canonical state.
pub(crate) struct OwnerSecuritySnapshotRequest<'a> {
    pub(crate) transaction_id: String,
    pub(crate) operation: crate::bootstrap_journal::BootstrapOperation,
    pub(crate) target_device_id: Option<&'a str>,
    pub(crate) reason: Option<crate::bootstrap_journal::RestrictionReason>,
    pub(crate) cutoff_seq: Option<u64>,
}

pub(crate) fn build_owner_security_operation_snapshot<S: SecureStore + ?Sized>(
    runtime_root: &Path,
    expected_brain_id: &str,
    store: &S,
    env: SecureStoreEnvironment,
    request: OwnerSecuritySnapshotRequest<'_>,
) -> KnowledgeMaterializationResult<crate::bootstrap_journal::ApprovalSnapshot> {
    use crate::bootstrap_journal::BootstrapOperation;
    use crate::trust_reducer::TrustReduction;

    if request.operation == BootstrapOperation::PeerAdmission {
        return Err(KnowledgeMaterializationError::InvalidInput(
            "owner security snapshot cannot describe peer admission".into(),
        ));
    }
    let local_identity = load_local_device_identity(runtime_root, expected_brain_id)
        .map_err(|error| KnowledgeMaterializationError::Verification(error.to_string()))?;
    let local_device = DeviceId::new(local_identity.device_id)
        .map_err(|error| KnowledgeMaterializationError::InvalidInput(error.to_string()))?;
    let ready = load_existing_event_signing_key(store, env, runtime_root, &local_device)
        .map_err(|error| KnowledgeMaterializationError::Verification(error.to_string()))?;
    let key_id = SigningKeyId::parse(&ready.public.signing_key_id)
        .map_err(|error| KnowledgeMaterializationError::Verification(error.to_string()))?;
    let inventory = read_canonical_device_inventory(runtime_root, None)?;
    let policy = scan_verified_trust_policy(
        runtime_root,
        expected_brain_id,
        &local_device,
        &key_id,
        &ready.signing_key.verifying_key().to_bytes(),
        &inventory,
    )?;
    let TrustReduction::Ready(trust) = &policy.reduction else {
        return Err(KnowledgeMaterializationError::Verification(
            "local trust history is held; owner operation cannot be prepared".into(),
        ));
    };
    if policy
        .devices
        .iter()
        .any(|device| !trust.trusted_devices.contains(device))
    {
        return Err(KnowledgeMaterializationError::Verification(
            "canonical inventory contains a device outside the local trust set".into(),
        ));
    }
    match request.operation {
        BootstrapOperation::DeviceRestriction => {
            let target = request
                .target_device_id
                .and_then(|target| DeviceId::new(target.to_owned()).ok())
                .ok_or_else(|| {
                    KnowledgeMaterializationError::InvalidInput(
                        "restriction target device ID is invalid".into(),
                    )
                })?;
            let reason = request.reason.ok_or_else(|| {
                KnowledgeMaterializationError::InvalidInput("restriction reason is required".into())
            })?;
            let cutoff = request.cutoff_seq.ok_or_else(|| {
                KnowledgeMaterializationError::InvalidInput("restriction cutoff is required".into())
            })?;
            policy.device_restriction_approval_snapshot(
                request.transaction_id,
                &target,
                reason,
                cutoff,
            )
        }
        BootstrapOperation::SelfRetirement => {
            if request.target_device_id.is_some()
                || request.reason.is_some()
                || request.cutoff_seq.is_some()
            {
                return Err(KnowledgeMaterializationError::InvalidInput(
                    "self-retirement request has unexpected peer fields".into(),
                ));
            }
            policy.self_retirement_approval_snapshot(request.transaction_id)
        }
        BootstrapOperation::PeerAdmission => unreachable!("rejected above"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnchorAppendMode {
    ExplicitCompletion,
    StartupReconciliation,
}

fn append_local_peer_approval_anchor_with_mode<S: SecureStore + ?Sized>(
    runtime_root: &Path,
    expected_brain_id: &str,
    store: &S,
    env: SecureStoreEnvironment,
    transaction_id: &str,
    raw_messages: &[Vec<u8>],
    mode: AnchorAppendMode,
) -> KnowledgeMaterializationResult<SecurityAnchorAppendOutcome> {
    crate::paths::assert_outside_repository(runtime_root)
        .map_err(|error| KnowledgeMaterializationError::InvalidInput(error.to_string()))?;
    let local_identity = load_local_device_identity(runtime_root, expected_brain_id)
        .map_err(|error| KnowledgeMaterializationError::Verification(error.to_string()))?;
    let local_device = DeviceId::new(local_identity.device_id)
        .map_err(|error| KnowledgeMaterializationError::InvalidInput(error.to_string()))?;
    let ready = load_existing_event_signing_key(store, env, runtime_root, &local_device)
        .map_err(|error| KnowledgeMaterializationError::Verification(error.to_string()))?;
    let local_key_id = SigningKeyId::parse(&ready.public.signing_key_id)
        .map_err(|error| KnowledgeMaterializationError::Verification(error.to_string()))?;
    let transcript_refs = raw_messages.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let transcript = cryptex_events::verify_bootstrap_transcript(&transcript_refs)
        .map_err(|error| KnowledgeMaterializationError::Verification(error.to_string()))?;
    if transcript.transaction_id() != transaction_id || transcript.brain_id() != expected_brain_id {
        return Err(KnowledgeMaterializationError::Verification(
            "completed bootstrap transcript differs from requested Brain or transaction".into(),
        ));
    }

    let journal = crate::bootstrap_journal::BootstrapJournalStore::new(runtime_root)
        .map_err(|error| KnowledgeMaterializationError::Verification(error.to_string()))?;
    let initial_journal = journal
        .load()
        .map_err(|error| KnowledgeMaterializationError::Verification(error.to_string()))?;
    let initial_transaction = initial_journal
        .transactions
        .iter()
        .find(|item| item.snapshot.transaction_id == transaction_id)
        .cloned()
        .ok_or_else(|| {
            KnowledgeMaterializationError::Verification(
                "peer approval transaction is absent from the daemon journal".into(),
            )
        })?;
    let mut checked_transaction = initial_transaction.clone();
    checked_transaction
        .record_verified_complete_transcript(raw_messages)
        .map_err(|error| KnowledgeMaterializationError::Verification(error.to_string()))?;
    if checked_transaction != initial_transaction
        || initial_transaction.snapshot.brain_id != expected_brain_id
        || initial_transaction.snapshot.local_identity.device_id != local_device.as_str()
        || initial_transaction.snapshot.local_identity.signing_key_id != local_key_id.as_str()
        || !initial_transaction.local_approval_signature_released
        || !matches!(
            initial_transaction.state,
            crate::bootstrap_journal::BootstrapState::TranscriptReady
                | crate::bootstrap_journal::BootstrapState::AppendUnknown
                | crate::bootstrap_journal::BootstrapState::Anchored
        )
    {
        return Err(KnowledgeMaterializationError::Verification(
            "journaled owner approval does not match the local identity or transcript".into(),
        ));
    }

    let complete_devices = read_canonical_device_inventory(runtime_root, None)?;
    let policy = scan_verified_trust_policy(
        runtime_root,
        expected_brain_id,
        &local_device,
        &local_key_id,
        &ready.signing_key.verifying_key().to_bytes(),
        &complete_devices,
    )?;
    let crate::trust_reducer::TrustReduction::Ready(trust) = &policy.reduction else {
        return Err(KnowledgeMaterializationError::Verification(
            "local trust policy is held; peer approval append refused".into(),
        ));
    };
    if policy
        .devices
        .iter()
        .any(|device| !trust.trusted_devices.contains(device))
    {
        return Err(KnowledgeMaterializationError::Verification(
            "canonical inventory includes a device outside the local trust set".into(),
        ));
    }

    // Re-read the journal only after canonical locks are held. No concurrent
    // approval transition may change the frozen transaction between lookup
    // and the append attempt.
    let current_journal = journal
        .load()
        .map_err(|error| KnowledgeMaterializationError::Verification(error.to_string()))?;
    let current_transaction = current_journal
        .transactions
        .iter()
        .find(|item| item.snapshot.transaction_id == transaction_id)
        .ok_or_else(|| {
            KnowledgeMaterializationError::Verification(
                "peer approval transaction disappeared while locks were acquired".into(),
            )
        })?;
    if current_transaction != &initial_transaction {
        return Err(KnowledgeMaterializationError::Verification(
            "peer approval journal changed during canonical validation".into(),
        ));
    }

    let operation_digest = cryptex_events::peer_approval_operation_digest(
        expected_brain_id,
        local_device.as_str(),
        &local_key_id,
        transaction_id,
        transcript.transcript(),
    )
    .map_err(|error| KnowledgeMaterializationError::Verification(error.to_string()))?;
    let mut event_id_records = Vec::new();
    let mut transaction_records = Vec::new();
    for record in policy.history.records() {
        if record.event_id() == initial_transaction.stable_event_id {
            event_id_records.push(record);
        }
        if let AuthenticatedSecurityRecord::SecurityV3(event) = record
            && matches!(event.payload(), SecurityPayloadV3::PeerApproved { transaction_id: id, .. }
                | SecurityPayloadV3::DeviceRestricted { transaction_id: id, .. }
                | SecurityPayloadV3::DeviceSelfRetired { transaction_id: id, .. } if id == transaction_id)
        {
            transaction_records.push(event);
        }
    }
    if !event_id_records.is_empty() || !transaction_records.is_empty() {
        if event_id_records.len() != 1 || transaction_records.len() != 1 {
            return Err(KnowledgeMaterializationError::Verification(
                "canonical event or security transaction ID is duplicated; Brain held".into(),
            ));
        }
        let AuthenticatedSecurityRecord::SecurityV3(event) = event_id_records[0] else {
            return Err(KnowledgeMaterializationError::Verification(
                "stable event ID is already used by a non-security record; Brain held".into(),
            ));
        };
        let exact = std::ptr::eq(event, transaction_records[0])
            && event.event_type() == SecurityEventTypeV3::PeerApproved
            && event.device_id() == &local_device
            && event.brain_id() == expected_brain_id
            && event.signing_key_id() == &local_key_id
            && event.operation_digest() == operation_digest
            && matches!(event.payload(), SecurityPayloadV3::PeerApproved {
                transaction_id: id,
                transcript: recorded,
            } if id == transaction_id && recorded == transcript.transcript());
        if !exact {
            return Err(KnowledgeMaterializationError::Verification(
                "existing canonical transaction differs from the exact owner-approved operation; Brain held".into(),
            ));
        }
        if initial_transaction.state == crate::bootstrap_journal::BootstrapState::Anchored {
            return Ok(SecurityAnchorAppendOutcome {
                event_id: event.event_id().to_owned(),
                device_seq: event.device_seq().to_u64().map_err(|error| {
                    KnowledgeMaterializationError::Verification(error.to_string())
                })?,
                content_hash: event.content_hash().as_str().to_owned(),
            });
        }
        if initial_transaction.state != crate::bootstrap_journal::BootstrapState::AppendUnknown {
            return Err(KnowledgeMaterializationError::Verification(
                "canonical anchor exists without a journaled uncertain append; Brain held".into(),
            ));
        }
        journal
            .record_verified_anchor(
                transaction_id,
                event.device_seq().to_u64().map_err(|error| {
                    KnowledgeMaterializationError::Verification(error.to_string())
                })?,
                event.content_hash().as_str(),
            )
            .map_err(|error| KnowledgeMaterializationError::Verification(error.to_string()))?;
        return Ok(SecurityAnchorAppendOutcome {
            event_id: event.event_id().to_owned(),
            device_seq: event
                .device_seq()
                .to_u64()
                .map_err(|error| KnowledgeMaterializationError::Verification(error.to_string()))?,
            content_hash: event.content_hash().as_str().to_owned(),
        });
    }

    if initial_transaction.state == crate::bootstrap_journal::BootstrapState::Anchored {
        return Err(KnowledgeMaterializationError::Verification(
            "journal claims an anchor absent from canonical history; Brain held".into(),
        ));
    }
    ensure_anchor_append_retry_is_safe(initial_transaction.state, mode)?;
    let local_lock = policy.device_lock(&local_device).ok_or_else(|| {
        KnowledgeMaterializationError::Verification(
            "local canonical stream lock is unavailable".into(),
        )
    })?;
    append_peer_approved_anchor_with_lock(crate::canonical_writer::SecurityAnchorAppendArgs {
        runtime_root,
        device_id: &local_device,
        lock: local_lock,
        signing_key: &ready.signing_key,
        signing_key_id: &ready.public.signing_key_id,
        event_id: &initial_transaction.stable_event_id,
        transcript: &transcript,
        transaction_id,
        journal: &journal,
    })
    .map_err(KnowledgeMaterializationError::Canonical)
}

/// Reconcile journal entries whose append outcome could have crossed a crash
/// boundary before the daemon exposes IPC, retrieval, or managed transport.
/// The journal is only a hint: each result is checked through the same full
/// canonical trust scan and lock-scoped exact-match path as an append retry.
pub fn reconcile_peer_approval_anchors_before_startup<S: SecureStore + ?Sized>(
    runtime_root: &Path,
    store: &S,
    env: SecureStoreEnvironment,
) -> KnowledgeMaterializationResult<()> {
    let journal = crate::bootstrap_journal::BootstrapJournalStore::new(runtime_root)
        .map_err(|error| KnowledgeMaterializationError::Verification(error.to_string()))?;
    let pending = journal
        .load()
        .map_err(|error| KnowledgeMaterializationError::Verification(error.to_string()))?;
    let transactions = pending
        .transactions
        .into_iter()
        .filter(|transaction| {
            matches!(
                transaction.state,
                crate::bootstrap_journal::BootstrapState::AppendUnknown
                    | crate::bootstrap_journal::BootstrapState::Anchored
            )
        })
        .map(|transaction| {
            let messages = transaction
                .transcript_messages
                .iter()
                .cloned()
                .map(String::into_bytes)
                .collect::<Vec<_>>();
            (transaction, messages)
        })
        .collect::<Vec<_>>();
    if transactions.is_empty() {
        return Ok(());
    }

    let brain = crate::identity_runtime::load_brain_identity(runtime_root)
        .map_err(|error| KnowledgeMaterializationError::Verification(error.to_string()))?;
    for (transaction, messages) in transactions {
        if transaction.snapshot.operation
            != crate::bootstrap_journal::BootstrapOperation::PeerAdmission
        {
            reconcile_owner_security_operation_before_startup(
                runtime_root,
                &brain.brain_id,
                store,
                env,
                &transaction,
            )?;
            continue;
        }
        let transaction_id = transaction.snapshot.transaction_id;
        if messages.len() != 4 {
            return Err(KnowledgeMaterializationError::Verification(
                "append-state journal transaction lacks a complete transcript; startup held".into(),
            ));
        }
        append_local_peer_approval_anchor_with_mode(
            runtime_root,
            &brain.brain_id,
            store,
            env,
            &transaction_id,
            &messages,
            AnchorAppendMode::StartupReconciliation,
        )?;
    }
    Ok(())
}

fn reconcile_owner_security_operation_before_startup<S: SecureStore + ?Sized>(
    runtime_root: &Path,
    expected_brain_id: &str,
    store: &S,
    env: SecureStoreEnvironment,
    transaction: &crate::bootstrap_journal::PendingBootstrapTransaction,
) -> KnowledgeMaterializationResult<()> {
    use crate::bootstrap_journal::{BootstrapOperation, BootstrapState};
    use crate::trust_reducer::TrustReduction;

    if !matches!(
        transaction.snapshot.operation,
        BootstrapOperation::DeviceRestriction | BootstrapOperation::SelfRetirement
    ) || !matches!(
        transaction.state,
        BootstrapState::AppendUnknown | BootstrapState::Anchored
    ) || transaction.append_attempts.len() != 1
        || transaction.snapshot.brain_id != expected_brain_id
    {
        return Err(KnowledgeMaterializationError::Verification(
            "owner-operation recovery journal shape is invalid; startup held".into(),
        ));
    }
    let local_identity = load_local_device_identity(runtime_root, expected_brain_id)
        .map_err(|error| KnowledgeMaterializationError::Verification(error.to_string()))?;
    let local_device = DeviceId::new(local_identity.device_id)
        .map_err(|error| KnowledgeMaterializationError::InvalidInput(error.to_string()))?;
    let ready = load_existing_event_signing_key(store, env, runtime_root, &local_device)
        .map_err(|error| KnowledgeMaterializationError::Verification(error.to_string()))?;
    let local_key_id = SigningKeyId::parse(&ready.public.signing_key_id)
        .map_err(|error| KnowledgeMaterializationError::Verification(error.to_string()))?;
    if transaction.snapshot.local_identity.device_id != local_device.as_str()
        || transaction.snapshot.local_identity.signing_key_id != local_key_id.as_str()
    {
        return Err(KnowledgeMaterializationError::Verification(
            "owner-operation recovery identity differs from the enrolled device".into(),
        ));
    }

    let complete_devices = read_canonical_device_inventory(runtime_root, None)?;
    let policy = scan_verified_trust_policy(
        runtime_root,
        expected_brain_id,
        &local_device,
        &local_key_id,
        &ready.signing_key.verifying_key().to_bytes(),
        &complete_devices,
    )?;
    let TrustReduction::Ready(trust) = &policy.reduction else {
        return Err(KnowledgeMaterializationError::Verification(
            "owner-operation recovery trust policy is held".into(),
        ));
    };
    if policy
        .devices
        .iter()
        .any(|device| !trust.trusted_devices.contains(device))
    {
        return Err(KnowledgeMaterializationError::Verification(
            "owner-operation recovery found an untrusted canonical stream".into(),
        ));
    }

    let attempt = &transaction.append_attempts[0];
    let mut event_id_records = Vec::new();
    let mut transaction_records = Vec::new();
    for record in policy.history.records() {
        if record.event_id() == transaction.stable_event_id {
            event_id_records.push(record);
        }
        if let AuthenticatedSecurityRecord::SecurityV3(event) = record
            && matches!(event.payload(),
                SecurityPayloadV3::DeviceRestricted { transaction_id: id, .. }
                | SecurityPayloadV3::DeviceSelfRetired { transaction_id: id, .. }
                if id == &transaction.snapshot.transaction_id)
        {
            transaction_records.push(event);
        }
    }
    if event_id_records.len() != 1 || transaction_records.len() != 1 {
        return Err(KnowledgeMaterializationError::Verification(
            "uncertain owner append has no unique exact canonical match; startup held".into(),
        ));
    }
    let AuthenticatedSecurityRecord::SecurityV3(event) = event_id_records[0] else {
        return Err(KnowledgeMaterializationError::Verification(
            "owner-operation event ID is occupied by a non-security record".into(),
        ));
    };
    if !std::ptr::eq(event, transaction_records[0])
        || event.device_id() != &local_device
        || event.brain_id() != expected_brain_id
        || event.signing_key_id() != &local_key_id
        || event.device_seq().to_u64().ok() != Some(attempt.reserved_sequence)
        || event.prev_content_hash().as_str() != attempt.predecessor_hash
        || event.content_hash().as_str()
            != attempt
                .candidate_content_hash
                .as_deref()
                .unwrap_or_default()
    {
        return Err(KnowledgeMaterializationError::Verification(
            "canonical owner-operation event differs from the pinned append attempt".into(),
        ));
    }

    let snapshot = &transaction.snapshot;
    let payload_matches = match (snapshot.operation, event.payload()) {
        (
            BootstrapOperation::DeviceRestriction,
            SecurityPayloadV3::DeviceRestricted {
                transaction_id,
                target_device_id,
                target_signing_key_id,
                target_public_key,
                reason,
                cutoff_seq,
                cutoff_content_hash,
            },
        ) => {
            let target = snapshot.restriction_target.as_deref().unwrap_or_default();
            let peer = snapshot.peer_identity.as_ref();
            let binding = DeviceId::new(target.to_owned())
                .ok()
                .and_then(|device| policy.verified_trust_history.approved_peer_binding(&device));
            let (expected_reason, expected_public_key) =
                match (snapshot.restriction_reason, binding) {
                    (Some(crate::bootstrap_journal::RestrictionReason::Retired), Some(binding)) => {
                        (
                            cryptex_events::RestrictionReasonV3::Retired,
                            lower_hex(&binding.public_key),
                        )
                    }
                    (
                        Some(crate::bootstrap_journal::RestrictionReason::Compromised),
                        Some(binding),
                    ) => (
                        cryptex_events::RestrictionReasonV3::Compromised,
                        lower_hex(&binding.public_key),
                    ),
                    _ => {
                        return Err(KnowledgeMaterializationError::Verification(
                            "owner-operation recovery lacks its approved target binding".into(),
                        ));
                    }
                };
            let expected_cutoff = snapshot
                .cutoff_seq
                .as_deref()
                .and_then(|value| value.parse::<u64>().ok());
            transaction_id == &snapshot.transaction_id
                && peer.is_some_and(|peer| {
                    peer.device_id == target
                        && peer.signing_key_id == target_signing_key_id.as_str()
                })
                && target_device_id.as_str() == target
                && target_public_key == &expected_public_key
                && *reason == expected_reason
                && Some(*cutoff_seq) == expected_cutoff
                && snapshot.cutoff_content_hash.as_deref() == Some(cutoff_content_hash.as_str())
        }
        (
            BootstrapOperation::SelfRetirement,
            SecurityPayloadV3::DeviceSelfRetired {
                transaction_id,
                last_authorized_seq,
                last_authorized_content_hash,
            },
        ) => {
            transaction_id == &snapshot.transaction_id
                && snapshot
                    .last_authorized_seq
                    .as_deref()
                    .and_then(|value| value.parse::<u64>().ok())
                    == Some(*last_authorized_seq)
                && snapshot.last_authorized_content_hash.as_deref()
                    == Some(last_authorized_content_hash.as_str())
        }
        _ => false,
    };
    if !payload_matches {
        return Err(KnowledgeMaterializationError::Verification(
            "canonical owner-operation payload differs from the frozen owner snapshot".into(),
        ));
    }

    if transaction.state == BootstrapState::AppendUnknown {
        let journal = crate::bootstrap_journal::BootstrapJournalStore::new(runtime_root)
            .map_err(|error| KnowledgeMaterializationError::Verification(error.to_string()))?;
        let current = journal
            .load()
            .map_err(|error| KnowledgeMaterializationError::Verification(error.to_string()))?;
        if current
            .transactions
            .iter()
            .find(|item| item.snapshot.transaction_id == snapshot.transaction_id)
            != Some(transaction)
        {
            return Err(KnowledgeMaterializationError::Verification(
                "owner-operation journal changed during recovery".into(),
            ));
        }
        journal
            .record_verified_anchor(
                &snapshot.transaction_id,
                attempt.reserved_sequence,
                event.content_hash().as_str(),
            )
            .map_err(|error| KnowledgeMaterializationError::Verification(error.to_string()))?;
    }
    Ok(())
}

fn ensure_anchor_append_retry_is_safe(
    state: crate::bootstrap_journal::BootstrapState,
    mode: AnchorAppendMode,
) -> KnowledgeMaterializationResult<()> {
    if state == crate::bootstrap_journal::BootstrapState::AppendUnknown
        && mode == AnchorAppendMode::StartupReconciliation
    {
        return Err(KnowledgeMaterializationError::Verification(
            "journal records an uncertain append absent from canonical history; startup will not retry automatically".into(),
        ));
    }
    Ok(())
}

fn scan_verified_canonical_until<R: TrustedDaemonKeyResolver>(
    runtime_root: &Path,
    expected_brain_id: &str,
    trusted_devices: &[DeviceId],
    key_resolver: &mut R,
    deadline: Option<Instant>,
) -> KnowledgeMaterializationResult<VerifiedCanonicalSnapshot> {
    check_deadline(deadline)?;
    crate::paths::assert_outside_repository(runtime_root)
        .map_err(|error| KnowledgeMaterializationError::InvalidInput(error.to_string()))?;
    let mut devices = trusted_devices.to_vec();
    devices.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    if devices.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(KnowledgeMaterializationError::InvalidInput(
            "trusted device list contains duplicates".into(),
        ));
    }
    // First-create writers must take this same lock before adding a device
    // directory. This makes inventory enumeration stable for daemon-owned
    // writers while all per-device locks are acquired and scans run.
    let inventory_lock = match deadline {
        Some(deadline) => acquire_canonical_inventory_lock_until(runtime_root, deadline)?,
        None => acquire_canonical_inventory_lock(runtime_root)?,
    };
    require_complete_device_inventory(runtime_root, &devices, deadline)?;

    let mut locks: Vec<DeviceCanonicalLock> = Vec::with_capacity(devices.len());
    for device in &devices {
        check_deadline(deadline)?;
        let lock = match deadline {
            Some(deadline) => {
                acquire_existing_device_canonical_lock_until(runtime_root, device, deadline)?
            }
            None => acquire_existing_device_canonical_lock(runtime_root, device)?,
        };
        if let Some(lock) = lock {
            locks.push(lock);
        }
    }
    // Re-check after per-device locks: this catches any unexpected inventory
    // mutation outside the daemon lock protocol before accepting the snapshot.
    require_complete_device_inventory(runtime_root, &devices, deadline)?;

    let mut frames = Vec::<RawFrame>::new();
    for device in &devices {
        check_deadline(deadline)?;
        scan_device_read_only(runtime_root, device, &mut frames, deadline)?;
    }
    // Detect inventory changes by actors outside the daemon lock protocol
    // (for example a not-yet-integrated sync importer) before accepting this
    // verified snapshot. Such actors must eventually join the same lock
    // protocol for a race-free production guarantee.
    require_complete_device_inventory(runtime_root, &devices, deadline)?;

    let raw_refs: Vec<&[u8]> = frames.iter().map(|frame| frame.bytes.as_slice()).collect();
    let history = cryptex_events::verify_v2_history_cancellable(
        &raw_refs,
        BorrowedResolver(key_resolver),
        || deadline.is_some_and(|deadline| Instant::now() >= deadline),
    )
    .map_err(|error| match error {
        cryptex_events::CancellableHistoryVerificationError::Cancelled => {
            KnowledgeMaterializationError::DeadlineExceeded
        }
        cryptex_events::CancellableHistoryVerificationError::InvalidRecord(error) => {
            KnowledgeMaterializationError::Verification(error.to_string())
        }
    })?;

    // Keep the M8 trust reader separate from the M4 materializer's history
    // type. Both are checked against the exact lock-scoped frame set so the
    // trust reducer can consume only a complete, authenticated snapshot.
    let security_history = cryptex_events::verify_security_history_cancellable(
        &raw_refs,
        expected_brain_id,
        BorrowedResolver(key_resolver),
        || deadline.is_some_and(|deadline| Instant::now() >= deadline),
    )
    .map_err(|error| match error {
        CancellableSecurityHistoryError::Cancelled => {
            KnowledgeMaterializationError::DeadlineExceeded
        }
        CancellableSecurityHistoryError::InvalidRecord(error) => {
            KnowledgeMaterializationError::Verification(error.to_string())
        }
    })?;

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

    let state =
        cryptex_knowledge::reduce_verified_events_cancellable(expected_brain_id, &history, || {
            deadline.is_some_and(|deadline| Instant::now() >= deadline)
        })
        .map_err(|error| {
            KnowledgeMaterializationError::InvalidInput(format!(
                "reducer rejected verified history: {error:?}"
            ))
        })?;
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
        deadline,
    )?;
    check_deadline(deadline)?;
    Ok(VerifiedCanonicalSnapshot {
        runtime_root: runtime_root.to_path_buf(),
        devices,
        security_frames: frames.clone(),
        frames,
        history,
        security_history,
        trust_history: None,
        trust_reduction: None,
        state,
        progress,
        _inventory_lock: inventory_lock,
        _locks: locks,
    })
}

/// Verify the complete on-disk inventory from the OS-held local identity and
/// locally signed peer-approval anchors. Only cutoff-admitted content enters
/// the returned snapshot; the full security history and all writer locks stay
/// attached to it through the retrieval operation.
pub(crate) fn scan_verified_canonical_from_local_anchor_until(
    runtime_root: &Path,
    expected_brain_id: &str,
    anchor: &LocalTrustAnchor,
    deadline: Option<Instant>,
) -> KnowledgeMaterializationResult<VerifiedCanonicalSnapshot> {
    scan_verified_canonical_from_local_anchor_with_import_devices_until(
        runtime_root,
        expected_brain_id,
        anchor,
        &[],
        deadline,
    )
}

pub(crate) fn scan_verified_canonical_from_local_anchor_with_import_devices_until(
    runtime_root: &Path,
    expected_brain_id: &str,
    anchor: &LocalTrustAnchor,
    import_devices: &[DeviceId],
    deadline: Option<Instant>,
) -> KnowledgeMaterializationResult<VerifiedCanonicalSnapshot> {
    scan_verified_canonical_from_local_anchor_with_import_devices_and_exclusions_until(
        runtime_root,
        expected_brain_id,
        anchor,
        import_devices,
        &[],
        deadline,
    )
}

pub(crate) fn scan_verified_canonical_from_local_anchor_with_import_devices_and_exclusions_until(
    runtime_root: &Path,
    expected_brain_id: &str,
    anchor: &LocalTrustAnchor,
    import_devices: &[DeviceId],
    exclusions: &[ImportRecoverySegmentExclusion],
    deadline: Option<Instant>,
) -> KnowledgeMaterializationResult<VerifiedCanonicalSnapshot> {
    check_deadline(deadline)?;
    crate::paths::assert_outside_repository(runtime_root)
        .map_err(|error| KnowledgeMaterializationError::InvalidInput(error.to_string()))?;
    let inventory_lock = match deadline {
        Some(deadline) => acquire_canonical_inventory_lock_until(runtime_root, deadline)?,
        None => acquire_canonical_inventory_lock(runtime_root)?,
    };
    let mut devices = read_canonical_device_inventory(runtime_root, deadline)?;
    let current_devices = devices.clone();
    if !devices.contains(anchor.device_id()) {
        return Err(KnowledgeMaterializationError::Verification(
            "local trust anchor has no canonical device stream".into(),
        ));
    }

    let mut locks = Vec::with_capacity(devices.len());
    for device in &devices {
        check_deadline(deadline)?;
        let lock = match deadline {
            Some(deadline) => {
                acquire_existing_device_canonical_lock_until(runtime_root, device, deadline)?
            }
            None => acquire_existing_device_canonical_lock(runtime_root, device)?,
        };
        if let Some(lock) = lock {
            locks.push(lock);
        }
    }
    if read_canonical_device_inventory(runtime_root, deadline)? != devices {
        return Err(KnowledgeMaterializationError::InvalidInput(
            "canonical device inventory changed while acquiring policy locks".into(),
        ));
    }

    let mut frames = Vec::<RawFrame>::new();
    for device in &devices {
        check_deadline(deadline)?;
        scan_device_read_only_with_import_exclusions(
            runtime_root,
            device,
            &mut frames,
            deadline,
            exclusions,
        )?;
    }
    if read_canonical_device_inventory(runtime_root, deadline)? != devices {
        return Err(KnowledgeMaterializationError::InvalidInput(
            "canonical device inventory changed during policy scan".into(),
        ));
    }
    let raw_refs = frames
        .iter()
        .map(|frame| frame.bytes.as_slice())
        .collect::<Vec<_>>();
    let (mut security_history, mut reduction, mut verified_trust) =
        crate::trust_reducer::verify_history_from_local_anchors_with_verified_cancellable(
            &raw_refs,
            expected_brain_id,
            anchor.device_id().clone(),
            anchor.signing_key_id().clone(),
            *anchor.public_key(),
            || deadline.is_some_and(|deadline| Instant::now() >= deadline),
        )
        .map_err(|hold| match hold {
            crate::trust_reducer::TrustHold::Cancelled => {
                KnowledgeMaterializationError::DeadlineExceeded
            }
            reason => KnowledgeMaterializationError::Verification(format!(
                "M8 trust history held: {reason:?}"
            )),
        })?;
    if security_history.records().len() != frames.len() {
        return Err(KnowledgeMaterializationError::InvalidInput(
            "M8 policy scan contains duplicate canonical records".into(),
        ));
    }
    for record in security_history.records() {
        check_deadline(deadline)?;
        let frame = frames
            .iter_mut()
            .find(|frame| {
                frame.position.event_id == record.event_id()
                    && frame.position.device_id == record.device_id()
            })
            .ok_or_else(|| {
                KnowledgeMaterializationError::InvalidInput(
                    "verified policy record is missing its canonical frame".into(),
                )
            })?;
        frame.position.content_hash = record.content_hash().as_str().to_owned();
    }
    let mut trust = match &reduction {
        crate::trust_reducer::TrustReduction::Ready(trust) => trust.clone(),
        crate::trust_reducer::TrustReduction::Held(reason) => {
            return Err(KnowledgeMaterializationError::Verification(format!(
                "M8 trust reduction held: {reason:?}"
            )));
        }
    };
    if devices
        .iter()
        .any(|device| !trust.trusted_devices.contains(device))
    {
        return Err(KnowledgeMaterializationError::Verification(
            "canonical inventory contains a device outside the reduced trust set".into(),
        ));
    }

    // Import candidate IDs are untrusted until the complete existing history
    // has reduced. Only approved first-stream devices may be added to the
    // eventual lock set.
    let mut requested_import_devices = import_devices.to_vec();
    requested_import_devices
        .sort_by(|left, right| left.as_str().as_bytes().cmp(right.as_str().as_bytes()));
    if requested_import_devices
        .windows(2)
        .any(|pair| pair[0] == pair[1])
    {
        return Err(KnowledgeMaterializationError::InvalidInput(
            "import device lock request contains duplicates".into(),
        ));
    }
    for device in &requested_import_devices {
        if !trust.trusted_devices.contains(device) {
            return Err(KnowledgeMaterializationError::Verification(
                "import candidate device is outside the locally approved trust set".into(),
            ));
        }
    }

    let mut added_import_devices = requested_import_devices
        .iter()
        .filter(|device| !current_devices.contains(device))
        .cloned()
        .collect::<Vec<_>>();
    if !added_import_devices.is_empty() {
        // The inventory lock stays held while existing locks are replaced by
        // the full bytewise-sorted lock set. This preserves the normative lock
        // order even when an approved first-stream device sorts before an
        // existing one.
        drop(locks);
        devices.append(&mut added_import_devices);
        devices.sort_by(|left, right| left.as_str().as_bytes().cmp(right.as_str().as_bytes()));
        locks = Vec::with_capacity(devices.len());
        for device in &devices {
            check_deadline(deadline)?;
            let lock = if current_devices.contains(device) {
                match deadline {
                    Some(deadline) => acquire_existing_device_canonical_lock_until(
                        runtime_root,
                        device,
                        deadline,
                    )?,
                    None => acquire_existing_device_canonical_lock(runtime_root, device)?,
                }
                .ok_or_else(|| {
                    KnowledgeMaterializationError::InvalidInput(
                        "canonical device disappeared while reacquiring import locks".into(),
                    )
                })?
            } else {
                crate::canonical_writer::acquire_device_canonical_lock_under_inventory(
                    runtime_root,
                    device,
                    &inventory_lock,
                )
                .map_err(|error| KnowledgeMaterializationError::InvalidInput(error.to_string()))?
            };
            locks.push(lock);
        }
        if read_canonical_device_inventory(runtime_root, deadline)? != devices {
            return Err(KnowledgeMaterializationError::InvalidInput(
                "canonical device inventory changed while acquiring the sorted import lock set"
                    .into(),
            ));
        }

        frames.clear();
        for device in &devices {
            check_deadline(deadline)?;
            scan_device_read_only_with_import_exclusions(
                runtime_root,
                device,
                &mut frames,
                deadline,
                exclusions,
            )?;
        }
        if read_canonical_device_inventory(runtime_root, deadline)? != devices {
            return Err(KnowledgeMaterializationError::InvalidInput(
                "canonical device inventory changed during locked import rescan".into(),
            ));
        }
        let raw_refs = frames
            .iter()
            .map(|frame| frame.bytes.as_slice())
            .collect::<Vec<_>>();
        (security_history, reduction, verified_trust) =
            crate::trust_reducer::verify_history_from_local_anchors_with_verified_cancellable(
                &raw_refs,
                expected_brain_id,
                anchor.device_id().clone(),
                anchor.signing_key_id().clone(),
                *anchor.public_key(),
                || deadline.is_some_and(|deadline| Instant::now() >= deadline),
            )
            .map_err(|hold| match hold {
                crate::trust_reducer::TrustHold::Cancelled => {
                    KnowledgeMaterializationError::DeadlineExceeded
                }
                reason => KnowledgeMaterializationError::Verification(format!(
                    "M8 import policy rescan held: {reason:?}"
                )),
            })?;
        if security_history.records().len() != frames.len() {
            return Err(KnowledgeMaterializationError::InvalidInput(
                "M8 locked import rescan contains duplicate canonical records".into(),
            ));
        }
        for record in security_history.records() {
            check_deadline(deadline)?;
            let frame = frames
                .iter_mut()
                .find(|frame| {
                    frame.position.event_id == record.event_id()
                        && frame.position.device_id == record.device_id()
                })
                .ok_or_else(|| {
                    KnowledgeMaterializationError::InvalidInput(
                        "verified import policy record is missing its canonical frame".into(),
                    )
                })?;
            frame.position.content_hash = record.content_hash().as_str().to_owned();
        }
        trust = match &reduction {
            crate::trust_reducer::TrustReduction::Ready(trust) => trust.clone(),
            crate::trust_reducer::TrustReduction::Held(reason) => {
                return Err(KnowledgeMaterializationError::Verification(format!(
                    "M8 import trust reduction held after lock reacquisition: {reason:?}"
                )));
            }
        };
        if devices
            .iter()
            .any(|device| !trust.trusted_devices.contains(device))
            || requested_import_devices
                .iter()
                .any(|device| !trust.trusted_devices.contains(device))
        {
            return Err(KnowledgeMaterializationError::Verification(
                "locked import inventory or candidate device is outside the reduced trust set"
                    .into(),
            ));
        }
    }
    let security_frames = frames.clone();
    let cutoffs = trust
        .effective_cutoffs
        .iter()
        .map(|(device, cutoff)| (device.as_str().to_owned(), *cutoff))
        .collect::<std::collections::BTreeMap<_, _>>();
    let history = security_history
        .content_history_through_cutoffs_cancellable(&cutoffs, || {
            deadline.is_some_and(|deadline| Instant::now() >= deadline)
        })
        .map_err(|error| match error {
            CancellableSecurityHistoryError::Cancelled => {
                KnowledgeMaterializationError::DeadlineExceeded
            }
            CancellableSecurityHistoryError::InvalidRecord(error) => {
                KnowledgeMaterializationError::Verification(error.to_string())
            }
        })?;

    let mut content_frames = Vec::with_capacity(history.records().len());
    for record in history.records() {
        check_deadline(deadline)?;
        let (event_id, device_id, content_hash) = record_identity(record).ok_or_else(|| {
            KnowledgeMaterializationError::InvalidInput(
                "cutoff content view contains an unverifiable event".into(),
            )
        })?;
        let frame = frames
            .iter()
            .find(|frame| {
                frame.position.event_id == event_id && frame.position.device_id == device_id
            })
            .ok_or_else(|| {
                KnowledgeMaterializationError::InvalidInput(
                    "cutoff content record is missing its canonical frame".into(),
                )
            })?;
        if frame.position.content_hash != content_hash {
            return Err(KnowledgeMaterializationError::InvalidInput(
                "cutoff content hash differs from its canonical frame".into(),
            ));
        }
        content_frames.push(frame.clone());
    }

    let state =
        cryptex_knowledge::reduce_verified_events_cancellable(expected_brain_id, &history, || {
            deadline.is_some_and(|deadline| Instant::now() >= deadline)
        })
        .map_err(|error| match error {
            cryptex_knowledge::ReducerError::Cancelled => {
                KnowledgeMaterializationError::DeadlineExceeded
            }
            _ => KnowledgeMaterializationError::InvalidInput(format!(
                "cutoff-aware reducer rejected verified content: {error:?}"
            )),
        })?;
    if !state.pending.is_empty() || state.materialized_event_ids.len() != history.assertions().len()
    {
        return Err(KnowledgeMaterializationError::InvalidInput(
            "cutoff-aware content has unresolved or unmaterialized assertions".into(),
        ));
    }
    let progress = build_progress(
        &devices,
        history.records(),
        &content_frames,
        &state.pending,
        &history
            .assertions()
            .iter()
            .map(|assertion| assertion.event().event_id.clone())
            .collect::<Vec<_>>(),
        &state.materialized_event_ids,
        deadline,
    )?;
    check_deadline(deadline)?;

    Ok(VerifiedCanonicalSnapshot {
        runtime_root: runtime_root.to_path_buf(),
        devices,
        frames: content_frames,
        security_frames,
        history,
        security_history,
        trust_history: Some(verified_trust),
        trust_reduction: Some(reduction),
        state,
        progress,
        _inventory_lock: inventory_lock,
        _locks: locks,
    })
}

/// Search current M3 rows only after a fresh trusted-history verification,
/// exact M4 per-device tip check, and byte-derived M3 row comparison. Any
/// missing, corrupt, stale, unsupported, or unresolved boundary withholds the
/// entire Brain. Canonical locks remain held through the SQLite query.
pub fn search_events_with_retrieval_safety<R: TrustedDaemonKeyResolver>(
    runtime_root: &Path,
    expected_brain_id: &str,
    trusted_devices: &[DeviceId],
    key_resolver: &mut R,
    db: &ProjectionDb,
    query: &SearchQuery,
) -> Result<Vec<SearchHit>, GuardedSearchError> {
    search_events_with_retrieval_safety_deadline(
        runtime_root,
        expected_brain_id,
        trusted_devices,
        key_resolver,
        db,
        query,
        None,
    )
}

pub fn search_events_with_retrieval_safety_deadline<R: TrustedDaemonKeyResolver>(
    runtime_root: &Path,
    expected_brain_id: &str,
    trusted_devices: &[DeviceId],
    key_resolver: &mut R,
    db: &ProjectionDb,
    query: &SearchQuery,
    deadline: Option<Instant>,
) -> Result<Vec<SearchHit>, GuardedSearchError> {
    let safety = evaluate_retrieval_safety_until(
        runtime_root,
        expected_brain_id,
        trusted_devices,
        key_resolver,
        db,
        deadline,
    );
    if safety.snapshot.is_none() {
        return Err(GuardedSearchError::BrainWideHold(
            safety
                .hold_reason()
                .unwrap_or(RetrievalSafetyHold::CanonicalVerificationUnavailable),
        ));
    }
    let result = {
        // Keep the verified snapshot borrowed throughout the query. The state
        // owns its RAII locks and is dropped only after this scope completes.
        let _snapshot = safety.snapshot.as_ref().expect("checked above");
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return Err(GuardedSearchError::BrainWideHold(
                RetrievalSafetyHold::OperationDeadlineExceeded,
            ));
        }
        let mut guarded_query = query.clone();
        guarded_query.brain_id = Some(expected_brain_id.to_owned());
        match deadline {
            Some(deadline) => match cryptex_projection::search_events_until(
                db.connection(),
                &guarded_query,
                deadline,
            ) {
                Ok(_) if Instant::now() >= deadline => Err(GuardedSearchError::BrainWideHold(
                    RetrievalSafetyHold::OperationDeadlineExceeded,
                )),
                Ok(hits) => Ok(hits),
                Err(_) if Instant::now() >= deadline => Err(GuardedSearchError::BrainWideHold(
                    RetrievalSafetyHold::OperationDeadlineExceeded,
                )),
                Err(error) => Err(GuardedSearchError::Query(error)),
            },
            None => {
                search_events(db.connection(), &guarded_query).map_err(GuardedSearchError::Query)
            }
        }
    };
    // Make the lock lifetime explicit: both canonical locks and inventory
    // lock remain held until the query has completed.
    drop(safety);
    result
}

/// Return a bounded timeline page only after the same Brain-wide verification
/// and materialization checks used by search. Canonical locks remain held
/// through the complete page query.
pub fn timeline_events_with_retrieval_safety_deadline<R: TrustedDaemonKeyResolver>(
    runtime_root: &Path,
    expected_brain_id: &str,
    trusted_devices: &[DeviceId],
    key_resolver: &mut R,
    db: &ProjectionDb,
    query: &TimelineQuery,
    deadline: Option<Instant>,
) -> Result<TimelinePage, GuardedSearchError> {
    let safety = evaluate_retrieval_safety_until(
        runtime_root,
        expected_brain_id,
        trusted_devices,
        key_resolver,
        db,
        deadline,
    );
    if safety.snapshot.is_none() {
        return Err(GuardedSearchError::BrainWideHold(
            safety
                .hold_reason()
                .unwrap_or(RetrievalSafetyHold::CanonicalVerificationUnavailable),
        ));
    }
    let result = {
        let _snapshot = safety.snapshot.as_ref().expect("checked above");
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return Err(GuardedSearchError::BrainWideHold(
                RetrievalSafetyHold::OperationDeadlineExceeded,
            ));
        }
        let mut guarded_query = query.clone();
        // Timeline is scoped to the verified local Brain. The projection
        // query receives that trusted ID; the Desktop cannot choose it.
        guarded_query.brain_id = Some(expected_brain_id.to_owned());
        match deadline {
            Some(deadline) => {
                match timeline_events_until(db.connection(), &guarded_query, deadline) {
                    Ok(_) if Instant::now() >= deadline => Err(GuardedSearchError::BrainWideHold(
                        RetrievalSafetyHold::OperationDeadlineExceeded,
                    )),
                    Ok(page) => Ok(page),
                    Err(_) if Instant::now() >= deadline => Err(GuardedSearchError::BrainWideHold(
                        RetrievalSafetyHold::OperationDeadlineExceeded,
                    )),
                    Err(error) => Err(GuardedSearchError::Query(error)),
                }
            }
            None => {
                timeline_events(db.connection(), &guarded_query).map_err(GuardedSearchError::Query)
            }
        }
    };
    drop(safety);
    result
}

/// Return one complete note only after a fresh Brain-wide safety check. The
/// cursor or event identifier never bypasses canonical verification.
pub fn event_detail_with_retrieval_safety_deadline<R: TrustedDaemonKeyResolver>(
    runtime_root: &Path,
    expected_brain_id: &str,
    trusted_devices: &[DeviceId],
    key_resolver: &mut R,
    db: &ProjectionDb,
    event_id: &str,
    deadline: Option<Instant>,
) -> Result<Option<SearchHit>, GuardedSearchError> {
    let safety = evaluate_retrieval_safety_until(
        runtime_root,
        expected_brain_id,
        trusted_devices,
        key_resolver,
        db,
        deadline,
    );
    if safety.snapshot.is_none() {
        return Err(GuardedSearchError::BrainWideHold(
            safety
                .hold_reason()
                .unwrap_or(RetrievalSafetyHold::CanonicalVerificationUnavailable),
        ));
    }
    let result = {
        let _snapshot = safety.snapshot.as_ref().expect("checked above");
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return Err(GuardedSearchError::BrainWideHold(
                RetrievalSafetyHold::OperationDeadlineExceeded,
            ));
        }
        match deadline {
            Some(deadline) => {
                match event_detail_until(db.connection(), expected_brain_id, event_id, deadline) {
                    Ok(_) if Instant::now() >= deadline => Err(GuardedSearchError::BrainWideHold(
                        RetrievalSafetyHold::OperationDeadlineExceeded,
                    )),
                    Ok(detail) => Ok(detail),
                    Err(_) if Instant::now() >= deadline => Err(GuardedSearchError::BrainWideHold(
                        RetrievalSafetyHold::OperationDeadlineExceeded,
                    )),
                    Err(error) => Err(GuardedSearchError::Query(error)),
                }
            }
            None => event_detail(db.connection(), expected_brain_id, event_id)
                .map_err(GuardedSearchError::Query),
        }
    };
    drop(safety);
    result
}

/// Evaluate the Brain-wide retrieval gate once for any content-returning
/// adapter. A successful state retains inventory/device locks until dropped;
/// callers must keep it alive for the complete retrieval operation.
pub fn evaluate_retrieval_safety<R: TrustedDaemonKeyResolver>(
    runtime_root: &Path,
    expected_brain_id: &str,
    trusted_devices: &[DeviceId],
    key_resolver: &mut R,
    db: &ProjectionDb,
) -> RetrievalSafetyState {
    evaluate_retrieval_safety_until(
        runtime_root,
        expected_brain_id,
        trusted_devices,
        key_resolver,
        db,
        None,
    )
}

/// Evaluate the same M3/M4 retrieval gate used for content access while also
/// locking each locally approved import candidate stream. Missing candidate
/// directories are created only after the local trust history approves them,
/// and their writer locks remain held by the returned state. This is an
/// import-validation primitive, not permission to publish without a later
/// locked revalidation and durable journal.
#[allow(dead_code)] // The Windows publisher will consume this lock-bearing gate.
pub(crate) fn evaluate_import_retrieval_safety<R: TrustedDaemonKeyResolver>(
    runtime_root: &Path,
    expected_brain_id: &str,
    candidate_devices: &[DeviceId],
    key_resolver: &mut R,
    db: &ProjectionDb,
) -> RetrievalSafetyState {
    evaluate_retrieval_safety_with_import_devices_until(
        runtime_root,
        expected_brain_id,
        &[],
        ImportScanScope {
            candidate_devices,
            recovery_exclusions: &[],
        },
        key_resolver,
        db,
        None,
    )
}

/// Evaluate M3/M4 retrieval safety against the canonical history preceding a
/// prepared import. Only exact manifest-bound tail destinations may be
/// omitted; the caller must verify the complete import batch before replay.
#[allow(dead_code)] // Used by the startup prepared-import recovery gate.
pub(crate) fn evaluate_import_recovery_retrieval_safety<R: TrustedDaemonKeyResolver>(
    runtime_root: &Path,
    expected_brain_id: &str,
    candidate_devices: &[DeviceId],
    exclusions: &[ImportRecoverySegmentExclusion],
    key_resolver: &mut R,
    db: &ProjectionDb,
) -> RetrievalSafetyState {
    evaluate_retrieval_safety_with_import_devices_until(
        runtime_root,
        expected_brain_id,
        &[],
        ImportScanScope {
            candidate_devices,
            recovery_exclusions: exclusions,
        },
        key_resolver,
        db,
        None,
    )
}

/// Rebuild the Obsidian projection only from the complete, currently verified
/// v1 history. The retrieval gate owns canonical locks through publication;
/// unsupported or stale state fails closed before the vault is touched.
pub fn rebuild_vault_with_retrieval_safety<R: TrustedDaemonKeyResolver>(
    runtime_root: &Path,
    expected_brain_id: &str,
    trusted_devices: &[DeviceId],
    key_resolver: &mut R,
    db: &ProjectionDb,
    vault_root: &Path,
) -> Result<cryptex_projection::VaultProjectionSummary, GuardedVaultError> {
    let safety = evaluate_retrieval_safety(
        runtime_root,
        expected_brain_id,
        trusted_devices,
        key_resolver,
        db,
    );
    let Some(snapshot) = safety.snapshot.as_ref() else {
        return Err(GuardedVaultError::BrainWideHold(
            safety
                .hold_reason()
                .unwrap_or(RetrievalSafetyHold::CanonicalVerificationUnavailable),
        ));
    };

    let events = snapshot
        .history
        .records()
        .iter()
        .filter_map(|record| match record {
            VersionedRecord::VerifiedV1(event) => Some(event),
            VersionedRecord::VerifiedV2Supported(_)
            | VersionedRecord::VerifiedUnsupported(_)
            | VersionedRecord::VerifiedSecurityV3(_)
            | VersionedRecord::Unverifiable(_) => None,
        })
        .collect::<Vec<_>>();
    let knowledge = vault_knowledge_view(&snapshot.state);
    cryptex_projection::rebuild_vault_with_knowledge(vault_root, events, &knowledge)
        .map_err(GuardedVaultError::Projection)
}

fn vault_knowledge_view(state: &cryptex_knowledge::KnowledgeState) -> VaultKnowledgeView {
    use std::collections::BTreeMap;

    use cryptex_events::FactValue;

    // A consolidated entity note uses the reducer's representative UUID. The
    // representative's own declaration provides the display label.
    let mut active_entities = BTreeMap::<String, String>::new();
    for entity in &state.entities {
        if entity.entity_id == entity.representative_id {
            active_entities.insert(
                entity.representative_id.to_ascii_lowercase(),
                entity.label.clone(),
            );
        }
    }
    let entities = active_entities
        .iter()
        .map(|(entity_id, display_label)| VaultEntity {
            entity_id: entity_id.clone(),
            display_label: display_label.clone(),
        })
        .collect();

    let facts = state
        .facts
        .iter()
        .filter_map(|fact| {
            let subject_entity_id = fact.subject_entity_id.to_ascii_lowercase();
            if !active_entities.contains_key(&subject_entity_id) {
                return None;
            }
            let (value_kind, value) = match &fact.value {
                FactValue::Text(value) => ("text", value.clone()),
                FactValue::Decimal(value) => ("decimal", value.clone()),
                FactValue::Boolean(value) => ("boolean", value.to_string()),
                FactValue::Date(value) => ("date", value.clone()),
                FactValue::EntityRef(value) => ("entity_ref", value.to_ascii_lowercase()),
            };
            Some(VaultFact {
                subject_entity_id,
                semantic_id: fact.semantic_id.clone(),
                predicate: fact.predicate.clone(),
                value_kind: value_kind.into(),
                value,
            })
        })
        .collect();

    let relations = state
        .relations
        .iter()
        .filter_map(|relation| {
            let subject_entity_id = relation.subject_entity_id.to_ascii_lowercase();
            let object_entity_id = relation.object_entity_id.to_ascii_lowercase();
            if !active_entities.contains_key(&subject_entity_id)
                || !active_entities.contains_key(&object_entity_id)
            {
                return None;
            }
            Some(VaultRelation {
                subject_entity_id,
                semantic_id: relation.semantic_id.clone(),
                predicate: relation.predicate.clone(),
                object_entity_id,
            })
        })
        .collect();

    VaultKnowledgeView {
        entities,
        facts,
        relations,
    }
}

fn evaluate_retrieval_safety_until<R: TrustedDaemonKeyResolver>(
    runtime_root: &Path,
    expected_brain_id: &str,
    trusted_devices: &[DeviceId],
    key_resolver: &mut R,
    db: &ProjectionDb,
    deadline: Option<Instant>,
) -> RetrievalSafetyState {
    evaluate_retrieval_safety_with_import_devices_until(
        runtime_root,
        expected_brain_id,
        trusted_devices,
        ImportScanScope {
            candidate_devices: &[],
            recovery_exclusions: &[],
        },
        key_resolver,
        db,
        deadline,
    )
}

fn evaluate_retrieval_safety_with_import_devices_until<R: TrustedDaemonKeyResolver>(
    runtime_root: &Path,
    expected_brain_id: &str,
    trusted_devices: &[DeviceId],
    import_scope: ImportScanScope<'_>,
    key_resolver: &mut R,
    db: &ProjectionDb,
    deadline: Option<Instant>,
) -> RetrievalSafetyState {
    let import_devices = import_scope.candidate_devices;
    let exclusions = import_scope.recovery_exclusions;
    let scanned = match key_resolver.local_trust_anchor() {
        Some(anchor) if !import_devices.is_empty() || !exclusions.is_empty() => {
            scan_verified_canonical_from_local_anchor_with_import_devices_and_exclusions_until(
                runtime_root,
                expected_brain_id,
                &anchor,
                import_devices,
                exclusions,
                deadline,
            )
        }
        Some(anchor) => scan_verified_canonical_from_local_anchor_until(
            runtime_root,
            expected_brain_id,
            &anchor,
            deadline,
        ),
        None if !import_devices.is_empty() || !exclusions.is_empty() => {
            Err(KnowledgeMaterializationError::Verification(
                "M8 import safety requires an OS-held local trust anchor".into(),
            ))
        }
        None => scan_verified_canonical_until(
            runtime_root,
            expected_brain_id,
            trusted_devices,
            key_resolver,
            deadline,
        ),
    };
    let snapshot = match scanned {
        Ok(snapshot) => snapshot,
        Err(KnowledgeMaterializationError::DeadlineExceeded) => {
            return RetrievalSafetyState::held(RetrievalSafetyHold::OperationDeadlineExceeded);
        }
        Err(_) if deadline.is_some_and(|deadline| Instant::now() >= deadline) => {
            return RetrievalSafetyState::held(RetrievalSafetyHold::OperationDeadlineExceeded);
        }
        Err(_) => {
            return RetrievalSafetyState::held(
                RetrievalSafetyHold::CanonicalVerificationUnavailable,
            );
        }
    };

    if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        return RetrievalSafetyState::held(RetrievalSafetyHold::OperationDeadlineExceeded);
    }

    // M3 indexes only v1 memory.note. Until v2 privacy/classification effects
    // are integrated across retrieval, any v2 or verified-unsupported record
    // conservatively holds all retrieval.
    for record in snapshot.history.records() {
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return RetrievalSafetyState::held(RetrievalSafetyHold::OperationDeadlineExceeded);
        }
        let hold = match record {
            VersionedRecord::VerifiedV1(_) => continue,
            // The local-anchor security-history reducer separately validates
            // v3 trust effects. A v3 control record is never an M3 note.
            VersionedRecord::VerifiedSecurityV3(_) => continue,
            VersionedRecord::VerifiedUnsupported(_) => RetrievalSafetyHold::UnsupportedSemantics,
            VersionedRecord::VerifiedV2Supported(_) => {
                RetrievalSafetyHold::V2SemanticsNotIntegrated
            }
            VersionedRecord::Unverifiable(_) => {
                RetrievalSafetyHold::CanonicalVerificationUnavailable
            }
        };
        return RetrievalSafetyState::held(hold);
    }
    if !snapshot.state.pending.is_empty()
        || snapshot.state.materialized_event_ids.len() != snapshot.history.assertions().len()
    {
        return RetrievalSafetyState::held(RetrievalSafetyHold::UnresolvedDependency);
    }

    if validate_schema_v1_read_only(db.connection()).is_err() {
        return RetrievalSafetyState::held(RetrievalSafetyHold::MaterializationStateMissingOrStale);
    }
    if !m4_materialization_state_matches(db, &snapshot) {
        return RetrievalSafetyState::held(RetrievalSafetyHold::MaterializationStateMissingOrStale);
    }
    let projection_matches =
        m3_projection_matches_history(db, expected_brain_id, &snapshot.history, deadline);
    if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        return RetrievalSafetyState::held(RetrievalSafetyHold::OperationDeadlineExceeded);
    }
    if !projection_matches {
        return RetrievalSafetyState::held(RetrievalSafetyHold::ProjectionDoesNotMatchCanonical);
    }
    RetrievalSafetyState::allowed(snapshot)
}

fn m4_materialization_state_matches(
    db: &ProjectionDb,
    snapshot: &VerifiedCanonicalSnapshot,
) -> bool {
    let meta = db.connection().query_row(
        "SELECT schema_version, reducer_version, cardinality_policy_version,
                normalization_version, build_id, rebuild_status
         FROM knowledge_meta WHERE id = 1",
        [],
        |row| {
            Ok((
                row.get::<_, u32>(0)?,
                row.get::<_, u32>(1)?,
                row.get::<_, u32>(2)?,
                row.get::<_, u32>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        },
    );
    let Ok((schema, reducer, cardinality, normalization, build, status)) = meta else {
        return false;
    };
    if schema != KNOWLEDGE_SCHEMA_VERSION
        || reducer != KNOWLEDGE_REDUCER_VERSION
        || cardinality != cryptex_knowledge::CARDINALITY_POLICY_VERSION
        || normalization != 1
        || build != KNOWLEDGE_BUILD_ID_V1
        || status != "ready"
    {
        return false;
    }

    let pending_count = db.connection().query_row(
        "SELECT COUNT(*) FROM knowledge_pending_dependencies",
        [],
        |row| row.get::<_, i64>(0),
    );
    if !matches!(pending_count, Ok(0)) {
        return false;
    }

    let mut rows = match db.connection().prepare(
        "SELECT device_id, integrity_pack_index, integrity_frame_offset,
                integrity_content_hash, safe_pack_index, safe_frame_offset,
                safe_content_hash, blocking_reason
         FROM knowledge_materialization_state ORDER BY device_id",
    ) {
        Ok(statement) => statement,
        Err(_) => return false,
    };
    let loaded = rows.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<i64>>(1)?,
            row.get::<_, Option<i64>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<i64>>(4)?,
            row.get::<_, Option<i64>>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, Option<String>>(7)?,
        ))
    });
    let Ok(loaded) = loaded else {
        return false;
    };
    let actual: Result<Vec<_>, _> = loaded.collect();
    let Ok(actual) = actual else {
        return false;
    };
    if actual.len() != snapshot.progress.len() {
        return false;
    }
    snapshot
        .progress
        .iter()
        .zip(actual)
        .all(|(expected, actual)| {
            let integrity = expected.integrity_position.as_ref();
            let safe = expected.safe_position.as_ref();
            actual.0 == expected.device_id
                && actual.1 == integrity.and_then(|p| i64::try_from(p.pack_index).ok())
                && actual.2 == integrity.and_then(|p| i64::try_from(p.frame_offset).ok())
                && actual.3.as_deref() == integrity.map(|p| p.content_hash.as_str())
                && actual.4 == safe.and_then(|p| i64::try_from(p.pack_index).ok())
                && actual.5 == safe.and_then(|p| i64::try_from(p.frame_offset).ok())
                && actual.6.as_deref() == safe.map(|p| p.content_hash.as_str())
                && actual.7.is_none()
                && expected.blocking_reason.is_none()
                && expected.safe_position == expected.integrity_position
        })
}

fn m3_projection_matches_history(
    db: &ProjectionDb,
    expected_brain_id: &str,
    history: &VerifiedV2History,
    deadline: Option<Instant>,
) -> bool {
    let mut expected = std::collections::BTreeMap::new();
    for record in history.records() {
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return false;
        }
        let VersionedRecord::VerifiedV1(event) = record else {
            return false;
        };
        let cryptex_events::EventPayload::MemoryNote(payload) = &event.payload;
        let classification = match event.classification {
            cryptex_events::Classification::Public => "public",
            cryptex_events::Classification::Internal => "internal",
            cryptex_events::Classification::Private => "private",
            cryptex_events::Classification::Restricted => "restricted",
        };
        let hit = SearchHit {
            event_id: event.event_id.clone(),
            brain_id: event.brain_id.clone(),
            device_id: event.device_id.clone(),
            device_seq: event.device_seq.as_str().to_owned(),
            created_at: event.created_at.clone(),
            event_type: event.event_type.as_str().to_owned(),
            classification: classification.to_owned(),
            content_hash: event.content_hash.as_str().to_owned(),
            signing_key_id: event.signing_key_id.as_str().to_owned(),
            text: payload.text.clone(),
        };
        if expected.insert(event.event_id.clone(), hit).is_some() {
            return false;
        }
    }
    if expected
        .values()
        .any(|hit| hit.brain_id != expected_brain_id)
    {
        return false;
    }

    let mut statement = match db.connection().prepare(
        "SELECT event_id, brain_id, device_id, device_seq, created_at, event_type,
                classification, content_hash, signing_key_id, text
         FROM events WHERE brain_id = ?1 ORDER BY event_id",
    ) {
        Ok(statement) => statement,
        Err(_) => return false,
    };
    let rows = statement.query_map([expected_brain_id], |row| {
        Ok(SearchHit {
            event_id: row.get(0)?,
            brain_id: row.get(1)?,
            device_id: row.get(2)?,
            device_seq: row.get(3)?,
            created_at: row.get(4)?,
            event_type: row.get(5)?,
            classification: row.get(6)?,
            content_hash: row.get(7)?,
            signing_key_id: row.get(8)?,
            text: row.get(9)?,
        })
    });
    let Ok(rows) = rows else {
        return false;
    };
    let mut actual = std::collections::BTreeMap::new();
    for row in rows {
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return false;
        }
        let Ok(hit) = row else {
            return false;
        };
        actual.insert(hit.event_id.clone(), hit);
    }
    actual == expected
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
    deadline: Option<Instant>,
) -> KnowledgeMaterializationResult<()> {
    scan_device_read_only_with_import_exclusions(runtime_root, device_id, output, deadline, &[])
}

/// Scan a canonical stream as it existed immediately before one prepared M8
/// transaction by validating and omitting only that transaction's visible
/// sealed destinations. Excluded destinations must form the canonical tail;
/// an exclusion cannot hide unrelated history in the middle of a stream.
#[allow(dead_code)] // Used by the policy-bound prepared-import recovery gate.
fn scan_device_read_only_with_import_exclusions(
    runtime_root: &Path,
    device_id: &DeviceId,
    output: &mut Vec<RawFrame>,
    deadline: Option<Instant>,
    exclusions: &[ImportRecoverySegmentExclusion],
) -> KnowledgeMaterializationResult<()> {
    check_deadline(deadline)?;
    let mut device_exclusions = exclusions
        .iter()
        .filter(|exclusion| &exclusion.device_id == device_id)
        .collect::<Vec<_>>();
    device_exclusions.sort_by_key(|exclusion| exclusion.segment_index);
    if device_exclusions.windows(2).any(|pair| {
        pair[0].segment_index == pair[1].segment_index
            || pair[0].length == 0
            || !is_lower_hex_sha256(&pair[0].sha256)
    }) || device_exclusions
        .last()
        .is_some_and(|exclusion| exclusion.length == 0 || !is_lower_hex_sha256(&exclusion.sha256))
    {
        return Err(KnowledgeMaterializationError::InvalidInput(
            "M8 recovery exclusions contain duplicate IDs or invalid file metadata".into(),
        ));
    }
    let directory = canonical_events_device_dir(runtime_root, device_id);
    if !directory.exists() {
        return Ok(());
    }
    let mut sealed = Vec::new();
    let mut excluded_present = Vec::new();
    let mut open = None;
    for entry in fs::read_dir(&directory).map_err(StorageError::Io)? {
        check_deadline(deadline)?;
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
            (Some(index), _) => {
                if let Some(exclusion) = device_exclusions
                    .iter()
                    .find(|exclusion| exclusion.segment_index == index)
                {
                    check_deadline(deadline)?;
                    let path = sealed_segment_path(runtime_root, device_id, index);
                    verify_import_excluded_segment(&path, exclusion, deadline)?;
                    excluded_present.push(index);
                } else {
                    sealed.push(index);
                }
            }
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
    excluded_present.sort_unstable();
    let mut physical_sealed = sealed.clone();
    physical_sealed.extend(excluded_present.iter().copied());
    physical_sealed.sort_unstable();
    if physical_sealed
        .iter()
        .enumerate()
        .any(|(expected, actual)| expected as u64 != *actual)
    {
        return Err(KnowledgeMaterializationError::Storage(
            StorageError::Integrity("sealed pack indexes are not contiguous".into()),
        ));
    }
    if let Some(first_excluded) = excluded_present.first()
        && sealed.iter().any(|index| index > first_excluded)
    {
        return Err(KnowledgeMaterializationError::Storage(
            StorageError::Integrity(
                "M8 recovery exclusions do not form a canonical stream tail".into(),
            ),
        ));
    }
    if let Some(open_index) = open
        && (physical_sealed.contains(&open_index)
            || physical_sealed
                .last()
                .is_some_and(|last| open_index != last + 1)
            || (physical_sealed.is_empty() && open_index != 0))
    {
        return Err(KnowledgeMaterializationError::Storage(
            StorageError::Integrity("active pack layout conflicts with sealed history".into()),
        ));
    }

    for index in sealed {
        let path = sealed_segment_path(runtime_root, device_id, index);
        let before = output.len();
        let verified_len = fold_pack(&path, device_id, index, output, deadline)?;
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
        let verified_len = fold_pack(&path, device_id, index, output, deadline)?;
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

fn is_lower_hex_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn verify_import_excluded_segment(
    path: &Path,
    exclusion: &ImportRecoverySegmentExclusion,
    deadline: Option<Instant>,
) -> KnowledgeMaterializationResult<()> {
    let (mut file, before) =
        crate::import_staging::open_recovery_source(path).map_err(|error| {
            KnowledgeMaterializationError::InvalidInput(format!(
                "M8 recovery destination could not be opened safely: {error}"
            ))
        })?;
    if before.length != exclusion.length {
        return Err(KnowledgeMaterializationError::Storage(
            StorageError::Integrity(
                "M8 recovery destination identity or length differs from its prepared manifest"
                    .into(),
            ),
        ));
    }
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut length = 0u64;
    loop {
        check_deadline(deadline)?;
        let read = file.read(&mut buffer).map_err(StorageError::Io)?;
        if read == 0 {
            break;
        }
        length = length.checked_add(read as u64).ok_or_else(|| {
            KnowledgeMaterializationError::InvalidInput(
                "M8 recovery destination length overflow".into(),
            )
        })?;
        if length > exclusion.length {
            return Err(KnowledgeMaterializationError::Storage(
                StorageError::Integrity(
                    "M8 recovery destination exceeds its prepared manifest length".into(),
                ),
            ));
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let after = crate::import_staging::source_snapshot(&file).map_err(|error| {
        KnowledgeMaterializationError::InvalidInput(format!(
            "M8 recovery destination identity could not be rechecked: {error}"
        ))
    })?;
    if length != exclusion.length || digest != exclusion.sha256 || after != before {
        return Err(KnowledgeMaterializationError::Storage(
            StorageError::Integrity(
                "M8 recovery destination changed or differs from its prepared manifest".into(),
            ),
        ));
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
    deadline: Option<Instant>,
) -> KnowledgeMaterializationResult<()> {
    check_deadline(deadline)?;
    let events_root = runtime_root.join("canonical").join("events");
    if !events_root.exists() {
        return Ok(());
    }
    let mut on_disk = Vec::<String>::new();
    for entry in fs::read_dir(&events_root).map_err(StorageError::Io)? {
        check_deadline(deadline)?;
        let entry = entry.map_err(StorageError::Io)?;
        let file_type = entry.file_type().map_err(StorageError::Io)?;
        if file_type.is_file() && entry.file_name().to_string_lossy() == "inventory.lock" {
            continue;
        }
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

/// Enumerate the complete canonical device inventory while the caller holds
/// the inventory lock. Malformed names and unexpected entries fail closed.
fn read_canonical_device_inventory(
    runtime_root: &Path,
    deadline: Option<Instant>,
) -> KnowledgeMaterializationResult<Vec<DeviceId>> {
    check_deadline(deadline)?;
    let events_root = runtime_root.join("canonical").join("events");
    if !events_root.exists() {
        return Ok(Vec::new());
    }
    let mut devices = Vec::new();
    for entry in fs::read_dir(events_root).map_err(StorageError::Io)? {
        check_deadline(deadline)?;
        let entry = entry.map_err(StorageError::Io)?;
        let file_type = entry.file_type().map_err(StorageError::Io)?;
        if file_type.is_file() && entry.file_name().to_string_lossy() == "inventory.lock" {
            continue;
        }
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
        devices.push(
            DeviceId::new(decoded)
                .map_err(|error| KnowledgeMaterializationError::InvalidInput(error.to_string()))?,
        );
    }
    devices.sort_by(|left, right| left.as_str().as_bytes().cmp(right.as_str().as_bytes()));
    Ok(devices)
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

fn canonical_device_has_open_segment(
    runtime_root: &Path,
    device_id: &DeviceId,
) -> KnowledgeMaterializationResult<bool> {
    let directory = canonical_events_device_dir(runtime_root, device_id);
    if !directory.exists() {
        return Ok(false);
    }
    for entry in fs::read_dir(directory).map_err(StorageError::Io)? {
        let entry = entry.map_err(StorageError::Io)?;
        let name = entry.file_name();
        let name = name.to_str().ok_or_else(|| {
            KnowledgeMaterializationError::InvalidInput(
                "canonical segment filename is not valid Unicode".into(),
            )
        })?;
        if parse_pack_index(name, "open").is_some() {
            return Ok(true);
        }
    }
    Ok(false)
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
    deadline: Option<Instant>,
) -> KnowledgeMaterializationResult<u64> {
    let (frames, verified_len) = fold_verified_frames_with_offsets(
        path,
        &AcceptAllBodies,
        Vec::new(),
        |frames, offset, body| {
            check_deadline(deadline).map_err(|_| {
                StorageError::CompatibilityBoundary(
                    "canonical retrieval scan exceeded its operation deadline".into(),
                )
            })?;
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

fn check_deadline(deadline: Option<Instant>) -> KnowledgeMaterializationResult<()> {
    if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        Err(KnowledgeMaterializationError::DeadlineExceeded)
    } else {
        Ok(())
    }
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
        VersionedRecord::VerifiedSecurityV3(event) => Some((
            event.event_id().to_owned(),
            event.device_id().as_str().to_owned(),
            event.content_hash().as_str().to_owned(),
        )),
        VersionedRecord::Unverifiable(_) => None,
    }
}

fn record_device_seq(record: &VersionedRecord) -> KnowledgeMaterializationResult<u64> {
    let sequence = match record {
        VersionedRecord::VerifiedV1(event) => event.device_seq.to_u64(),
        VersionedRecord::VerifiedV2Supported(event) => event.event().device_seq.to_u64(),
        VersionedRecord::VerifiedUnsupported(event) => event.device_seq.to_u64(),
        VersionedRecord::VerifiedSecurityV3(event) => event.device_seq().to_u64(),
        VersionedRecord::Unverifiable(_) => {
            return Err(KnowledgeMaterializationError::InvalidInput(
                "unverifiable record has no import starting sequence".into(),
            ));
        }
    };
    sequence.map_err(|error| {
        KnowledgeMaterializationError::InvalidInput(format!(
            "verified canonical device sequence is invalid: {error}"
        ))
    })
}

fn build_progress(
    devices: &[DeviceId],
    records: &[VersionedRecord],
    frames: &[RawFrame],
    pending: &[cryptex_knowledge::PendingDependency],
    supported_event_ids: &[String],
    materialized_event_ids: &[String],
    deadline: Option<Instant>,
) -> KnowledgeMaterializationResult<Vec<DeviceMaterializationProgress>> {
    let mut pending_ids = std::collections::BTreeSet::new();
    for item in pending {
        check_deadline(deadline)?;
        pending_ids.insert(item.event_id.to_ascii_lowercase());
    }
    let mut materialized_ids = std::collections::BTreeSet::new();
    for event_id in materialized_event_ids {
        check_deadline(deadline)?;
        materialized_ids.insert(event_id.to_ascii_lowercase());
    }
    let mut unsupported_ids = std::collections::BTreeSet::new();
    for record in records {
        check_deadline(deadline)?;
        if let VersionedRecord::VerifiedUnsupported(event) = record {
            unsupported_ids.insert(event.event_id.to_ascii_lowercase());
        }
    }
    // Supported v2 records omitted by the reducer are not safe progress. This
    // includes semantically invalid records retained as reviewable conflicts;
    // conservatively stop the contiguous watermark instead of implying that
    // their semantics were successfully materialized.
    let mut unmaterialized_supported_ids = std::collections::BTreeSet::new();
    for event_id in supported_event_ids {
        check_deadline(deadline)?;
        let event_id = event_id.to_ascii_lowercase();
        if !materialized_ids.contains(&event_id) {
            unmaterialized_supported_ids.insert(event_id);
        }
    }
    let mut output = Vec::with_capacity(devices.len());
    for device in devices {
        check_deadline(deadline)?;
        let mut indexes = Vec::new();
        for (index, frame) in frames.iter().enumerate() {
            check_deadline(deadline)?;
            if frame.position.device_id == device.as_str() {
                indexes.push(index);
            }
        }
        let integrity_position = indexes.last().map(|index| frames[*index].position.clone());
        let mut first_blocker = None;
        for (position, index) in indexes.iter().enumerate() {
            check_deadline(deadline)?;
            let event_id = frames[*index].position.event_id.to_ascii_lowercase();
            if pending_ids.contains(&event_id)
                || unsupported_ids.contains(&event_id)
                || unmaterialized_supported_ids.contains(&event_id)
            {
                first_blocker = Some(position);
                break;
            }
        }
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
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canonical_writer::{AppendMemoryNoteArgs, append_memory_note};
    use crate::event_signing_key::ensure_event_signing_key;
    use crate::local_device_identity::LocalDeviceKeyResolver;
    use crate::projection::rebuild_device_projection;
    use crate::secure_store::{FakeSecureStore, SecureStoreEnvironment};
    use crate::test_temp::exclusive_temp_dir;
    use cryptex_events::{
        Classification, DeviceSeq, EventPayload, EventType, MemoryNotePayload, SignableEvent,
        SigningKeyId, TrustedDaemonKeyResolver,
    };
    use cryptex_projection::ProjectionDb;
    use ed25519_dalek::{SigningKey, VerifyingKey};
    use std::fs;
    use std::io::Write;

    #[test]
    fn uncertain_anchor_append_without_canonical_match_is_not_retried() {
        assert!(
            ensure_anchor_append_retry_is_safe(
                crate::bootstrap_journal::BootstrapState::AppendUnknown,
                AnchorAppendMode::StartupReconciliation,
            )
            .is_err()
        );
        assert!(
            ensure_anchor_append_retry_is_safe(
                crate::bootstrap_journal::BootstrapState::AppendUnknown,
                AnchorAppendMode::ExplicitCompletion,
            )
            .is_ok()
        );
        assert!(
            ensure_anchor_append_retry_is_safe(
                crate::bootstrap_journal::BootstrapState::TranscriptReady,
                AnchorAppendMode::StartupReconciliation,
            )
            .is_ok()
        );
    }

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
    fn vault_view_uses_only_representative_entities_and_reduced_relations() {
        use cryptex_events::{EntityKind, FactValue};
        use cryptex_knowledge::{Entity, Fact, KnowledgeState, Provenance, Relation};

        let representative = "11111111-1111-4111-8111-111111111111";
        let merged_id = "22222222-2222-4222-8222-222222222222";
        let target = "33333333-3333-4333-8333-333333333333";
        let provenance = Provenance {
            event_id: "44444444-4444-4444-8444-444444444444".into(),
            device_id: "fixture-device".into(),
            device_seq: 1,
            signing_key_id: "fixture-key".into(),
            confidence: None,
            authorization_receipt_id: "55555555-5555-4555-8555-555555555555".into(),
            classification: Classification::Public,
        };
        let state = KnowledgeState {
            entities: vec![
                Entity {
                    entity_id: representative.into(),
                    representative_id: representative.into(),
                    kind: EntityKind::Person,
                    label: "Tarık".into(),
                    provenance: vec![provenance.clone()],
                },
                Entity {
                    entity_id: merged_id.into(),
                    representative_id: representative.into(),
                    kind: EntityKind::Person,
                    label: "T. G.".into(),
                    provenance: vec![provenance.clone()],
                },
                Entity {
                    entity_id: target.into(),
                    representative_id: target.into(),
                    kind: EntityKind::Project,
                    label: "Cryptex".into(),
                    provenance: vec![provenance.clone()],
                },
            ],
            facts: vec![Fact {
                semantic_id: "fact-key".into(),
                subject_entity_id: representative.into(),
                predicate: "person.name".into(),
                value: FactValue::Text("Tarık".into()),
                provenance: vec![provenance.clone()],
            }],
            relations: vec![Relation {
                semantic_id: "relation-key".into(),
                subject_entity_id: representative.into(),
                predicate: "works_on".into(),
                object_entity_id: target.into(),
                provenance: vec![provenance],
            }],
            ..KnowledgeState::default()
        };

        let view = vault_knowledge_view(&state);
        assert_eq!(view.entities.len(), 2);
        assert_eq!(view.entities[0].entity_id, representative);
        assert_eq!(view.entities[0].display_label, "Tarık");
        assert_eq!(view.facts.len(), 1);
        assert_eq!(view.facts[0].value_kind, "text");
        assert_eq!(view.relations.len(), 1);
        assert_eq!(view.relations[0].object_entity_id, target);
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
                event_id: None,
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
            key_id: ready.public.signing_key_id.clone(),
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

        let snapshot =
            scan_verified_canonical(&root, brain, std::slice::from_ref(&device), &mut resolver)
                .expect("the lock-scoped scan also builds authenticated security history");
        let local_key_id = SigningKeyId::parse(&ready.public.signing_key_id).unwrap();
        let reduction = snapshot.trust_reduction(
            device.clone(),
            &local_key_id,
            &ready.signing_key.verifying_key().to_bytes(),
        );
        assert!(matches!(
            reduction,
            crate::trust_reducer::TrustReduction::Ready(ref trust)
                if trust.trusted_devices == std::collections::BTreeSet::from([device.clone()])
                    && trust.effective_cutoffs.get(&device) == Some(&u64::MAX)
        ));
        drop(snapshot);

        let policy_snapshot = scan_verified_trust_policy(
            &root,
            brain,
            &device,
            &local_key_id,
            &ready.signing_key.verifying_key().to_bytes(),
            std::slice::from_ref(&device),
        )
        .expect("M8 scan authenticates the lock-scoped local-only event set");
        assert!(matches!(
            policy_snapshot.reduction,
            crate::trust_reducer::TrustReduction::Ready(ref trust)
                if trust.trusted_devices == std::collections::BTreeSet::from([device.clone()])
        ));
        let reduced = policy_snapshot
            .content_state
            .as_ref()
            .expect("locally trusted records reduce only through effective cutoffs");
        assert!(reduced.entities.is_empty());
        assert!(reduced.materialized_event_ids.is_empty());
        let last_local_record = policy_snapshot
            .history
            .records()
            .iter()
            .filter(|record| record.device_id() == device.as_str())
            .max_by_key(|record| record.device_seq())
            .expect("synthetic local pilot has a verified canonical tip");
        let retirement = policy_snapshot
            .self_retirement_approval_snapshot("dd".repeat(32))
            .expect("self-retirement request freezes the exact verified local tip");
        assert_eq!(
            retirement.operation,
            crate::bootstrap_journal::BootstrapOperation::SelfRetirement
        );
        assert_eq!(retirement.local_identity.device_id, device.as_str());
        assert_eq!(
            retirement.last_authorized_seq.as_deref(),
            Some(last_local_record.device_seq().to_string().as_str())
        );
        assert_eq!(
            retirement.last_authorized_content_hash.as_deref(),
            Some(last_local_record.content_hash().as_str())
        );
        assert!(retirement.digest().is_ok());
        let unapproved = DeviceId::new("synthetic-unapproved-peer").unwrap();
        assert!(
            policy_snapshot
                .device_restriction_approval_snapshot(
                    "ee".repeat(32),
                    &unapproved,
                    crate::bootstrap_journal::RestrictionReason::Compromised,
                    0,
                )
                .is_err()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn restriction_request_binds_approved_peer_and_exact_verified_cutoff() {
        use crate::trust_reducer::{
            ApprovedPeerBinding, TrustReduction, VerifiedHistoryState, VerifiedTrustHistory,
            reduce_trust_history,
        };
        use cryptex_events::{AuthenticatedSecurityRecord, verify_security_history};
        use std::collections::{BTreeMap, BTreeSet};

        let root = exclusive_temp_dir("m8-owner-restriction-snapshot");
        let brain = "33333333-3333-4333-8333-333333333333";
        let local = DeviceId::new("local-owner").unwrap();
        let peer = DeviceId::new("approved-peer").unwrap();
        let local_key = SigningKey::from_bytes(&[0x51; 32]);
        let peer_key = SigningKey::from_bytes(&[0x52; 32]);
        let local_key_id =
            cryptex_events::signing_key_id_from_verifying_key(&local_key.verifying_key()).unwrap();
        let peer_key_id =
            cryptex_events::signing_key_id_from_verifying_key(&peer_key.verifying_key()).unwrap();
        let target_event = cryptex_events::accept_event(
            SignableEvent {
                schema_version: cryptex_events::EVENT_SCHEMA_VERSION,
                event_id: "11111111-1111-4111-8111-111111111111".into(),
                brain_id: brain.into(),
                device_id: peer.as_str().into(),
                device_seq: DeviceSeq::from_u64(1).unwrap(),
                created_at: "2026-10-29T00:00:00Z".into(),
                event_type: EventType::MemoryNote,
                classification: Classification::Public,
                signing_key_id: peer_key_id.clone(),
                payload: EventPayload::MemoryNote(
                    MemoryNotePayload::new("synthetic target").unwrap(),
                ),
                prev_content_hash: cryptex_events::ContentHashHex::zero(),
            },
            &peer_key,
        )
        .unwrap();
        let target_bytes = serde_json::to_vec(&target_event).unwrap();
        let raw = [target_bytes.as_slice()];
        let history = verify_security_history(
            &raw,
            brain,
            OneTrustedDevice {
                device: peer.as_str().to_owned(),
                key_id: peer_key_id.as_str().to_owned(),
                key: peer_key.verifying_key(),
            },
        )
        .unwrap();

        let local_transport_device_id =
            "AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA".to_owned();
        let transport_device_id =
            "BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB".to_owned();
        let peer_binding = ApprovedPeerBinding {
            device_id: peer.clone(),
            signing_key_id: peer_key_id.clone(),
            public_key: peer_key.verifying_key().to_bytes(),
            fingerprint_sha256: Sha256::digest(peer_key.verifying_key().as_bytes()).into(),
            local_transport_device_id: local_transport_device_id.clone(),
            transport_device_id: transport_device_id.clone(),
        };
        let verified_trust_history = VerifiedTrustHistory {
            local_device: local.clone(),
            local_signing_key_id: local_key_id,
            local_public_key: local_key.verifying_key().to_bytes(),
            known_devices: BTreeSet::from([local.clone(), peer.clone()]),
            directly_approved_peers: BTreeSet::from([peer.clone()]),
            approved_peer_bindings: BTreeMap::from([(peer.clone(), peer_binding)]),
            restrictions: Vec::new(),
            self_retirements: Vec::new(),
            state: VerifiedHistoryState::Complete,
        };
        let reduction = reduce_trust_history(&verified_trust_history);
        assert!(matches!(reduction, TrustReduction::Ready(_)));
        assert!(
            history
                .records()
                .iter()
                .all(|record| matches!(record, AuthenticatedSecurityRecord::V1(_)))
        );
        let snapshot = VerifiedTrustPolicySnapshot {
            devices: vec![local, peer.clone()],
            history,
            verified_trust_history,
            reduction,
            content_state: None,
            _inventory_lock: acquire_canonical_inventory_lock(&root).unwrap(),
            _locks: Vec::new(),
        };

        let request = snapshot
            .device_restriction_approval_snapshot(
                "ab".repeat(32),
                &peer,
                crate::bootstrap_journal::RestrictionReason::Compromised,
                1,
            )
            .expect("approved peer and exact boundary produce a closed request");
        assert_eq!(
            request.peer_identity.as_ref().unwrap().device_id,
            peer.as_str()
        );
        assert_eq!(
            request.peer_identity.as_ref().unwrap().signing_key_id,
            peer_key_id.as_str()
        );
        assert_eq!(
            request
                .transport_association
                .as_ref()
                .unwrap()
                .local_device_id,
            local_transport_device_id
        );
        assert_eq!(
            request
                .transport_association
                .as_ref()
                .unwrap()
                .peer_device_id,
            transport_device_id
        );
        assert_eq!(request.cutoff_seq.as_deref(), Some("1"));
        assert_eq!(
            request.cutoff_content_hash.as_deref(),
            Some(target_event.content_hash.as_str())
        );
        assert!(request.digest().is_ok());
        assert!(
            snapshot
                .device_restriction_approval_snapshot(
                    "cd".repeat(32),
                    &peer,
                    crate::bootstrap_journal::RestrictionReason::Retired,
                    2,
                )
                .is_err()
        );
        drop(snapshot);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn confirmed_self_retirement_append_recovers_after_durable_append() {
        use crate::bootstrap_journal::{BootstrapJournalStore, BootstrapState};
        use cryptex_core::BrainIdentity;

        let root = exclusive_temp_dir("m8-owner-self-retirement-recovery");
        let store = FakeSecureStore::new();
        let brain = "33333333-3333-4333-8333-333333333333";
        crate::identity_runtime::write_brain_identity(
            &root,
            &BrainIdentity::new(brain, "2026-10-29T00:00:00Z").unwrap(),
        )
        .unwrap();
        let enrolled = crate::local_device_identity::enroll_local_device(
            &root,
            &store,
            SecureStoreEnvironment::Dev,
        )
        .unwrap();
        let device = DeviceId::new(enrolled.device_id).unwrap();
        append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Public,
                text: "synthetic pre-retirement event",
                event_id: None,
                config: &crate::canonical_writer::CanonicalWriterConfig::default(),
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
        let key_id = SigningKeyId::parse(&ready.public.signing_key_id).unwrap();
        let inventory = read_canonical_device_inventory(&root, None).unwrap();
        let policy = scan_verified_trust_policy(
            &root,
            brain,
            &device,
            &key_id,
            &ready.signing_key.verifying_key().to_bytes(),
            &inventory,
        )
        .unwrap();
        let transaction_id = "ab".repeat(32);
        let snapshot = policy
            .self_retirement_approval_snapshot(transaction_id.clone())
            .unwrap();
        drop(policy);

        let journal = BootstrapJournalStore::new(&root).unwrap();
        let stable_event_id = "11111111-1111-4111-8111-111111111111";
        journal
            .begin_owner_operation(snapshot.clone(), stable_event_id)
            .unwrap();
        journal
            .confirm_owner_operation(&transaction_id, &snapshot.digest().unwrap())
            .unwrap();

        crate::canonical_writer::fail_after_append_before_tip();
        assert!(
            append_confirmed_owner_security_operation(
                &root,
                brain,
                &store,
                SecureStoreEnvironment::Dev,
                &transaction_id,
            )
            .is_err()
        );
        assert_eq!(
            journal.load().unwrap().transactions[0].state,
            BootstrapState::AppendUnknown
        );

        reconcile_peer_approval_anchors_before_startup(&root, &store, SecureStoreEnvironment::Dev)
            .expect("startup proves the exact durable event and does not append it twice");
        let recovered = journal.load().unwrap();
        assert_eq!(recovered.transactions.len(), 1);
        assert_eq!(recovered.transactions[0].state, BootstrapState::Anchored);
        assert_eq!(recovered.transactions[0].append_attempts.len(), 1);

        let inventory = read_canonical_device_inventory(&root, None).unwrap();
        let recovered_policy = scan_verified_trust_policy(
            &root,
            brain,
            &device,
            &key_id,
            &ready.signing_key.verifying_key().to_bytes(),
            &inventory,
        )
        .unwrap();
        let matching = recovered_policy
            .history
            .records()
            .iter()
            .filter(|record| record.event_id() == stable_event_id)
            .collect::<Vec<_>>();
        assert_eq!(matching.len(), 1);
        assert!(matches!(
            matching[0],
            AuthenticatedSecurityRecord::SecurityV3(event)
                if matches!(event.payload(), SecurityPayloadV3::DeviceSelfRetired {
                    transaction_id: found,
                    last_authorized_seq: 1,
                    ..
                } if found == &transaction_id)
        ));
        drop(recovered_policy);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn guarded_search_allows_only_current_verified_and_materialized_v1_projection() {
        use cryptex_core::BrainIdentity;

        use crate::identity_runtime::write_brain_identity;

        let root = exclusive_temp_dir("m4-safe-search");
        let store = FakeSecureStore::new();
        let brain = "33333333-3333-4333-8333-333333333333";
        let brain_identity = BrainIdentity::new(brain, "2026-10-29T00:00:00Z").unwrap();
        write_brain_identity(&root, &brain_identity).unwrap();
        let enrollment = crate::local_device_identity::enroll_local_device(
            &root,
            &store,
            SecureStoreEnvironment::Dev,
        )
        .unwrap();
        let device = DeviceId::new(enrollment.device_id).unwrap();
        append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Internal,
                text: "synthetic safe search note",
                event_id: None,
                config: &crate::canonical_writer::CanonicalWriterConfig {
                    seal_event_threshold: 1,
                },
            },
        )
        .unwrap();
        let _ready = ensure_event_signing_key(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            &device,
            &[],
            true,
        )
        .unwrap();
        let mut resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
            &root,
            brain,
            &store,
            SecureStoreEnvironment::Dev,
        )
        .expect("test resolver carries the OS-store-backed local trust anchor");
        let diagnostic_snapshot = scan_verified_canonical_from_local_anchor_until(
            &root,
            brain,
            &resolver.local_trust_anchor().unwrap(),
            None,
        )
        .expect("local-anchor scan should authenticate the local-only stream");
        let (verified_trust_history, trust) = diagnostic_snapshot
            .verified_trust_context()
            .expect("scanner returns a private, verified import trust context");
        assert!(trust.trusted_devices.contains(&device));
        assert!(
            !trust
                .trusted_devices
                .contains(&DeviceId::new("unapproved-peer").unwrap())
        );
        let starting_tips = diagnostic_snapshot
            .import_starting_tips_from_trust(std::slice::from_ref(&device), trust)
            .expect("authorized local stream has a lock-scoped import tip");
        assert_eq!(starting_tips[0].1.segment_index, Some(0));
        assert_eq!(starting_tips[0].1.device_seq, Some(1));
        let mut import_resolver = verified_trust_history.key_resolver();
        assert!(
            import_resolver
                .trusted_daemon_key(
                    &device.to_string(),
                    resolver
                        .local_trust_anchor()
                        .unwrap()
                        .signing_key_id()
                        .as_str()
                )
                .is_some()
        );
        drop(diagnostic_snapshot);
        let mut db = ProjectionDb::open_in_memory().unwrap();

        let held = search_events_with_retrieval_safety(
            &root,
            brain,
            std::slice::from_ref(&device),
            &mut resolver,
            &db,
            &SearchQuery::default(),
        )
        .unwrap_err();
        assert!(
            matches!(
                &held,
                GuardedSearchError::BrainWideHold(
                    RetrievalSafetyHold::MaterializationStateMissingOrStale
                )
            ),
            "unexpected first-read gate result: {held:?}"
        );

        rebuild_device_projection(&store, SecureStoreEnvironment::Dev, &root, &device, &db)
            .unwrap();
        rebuild_knowledge_from_canonical(
            &root,
            brain,
            std::slice::from_ref(&device),
            &mut resolver,
            &mut db,
        )
        .unwrap();

        let hits = search_events_with_retrieval_safety(
            &root,
            brain,
            std::slice::from_ref(&device),
            &mut resolver,
            &db,
            &SearchQuery::default(),
        )
        .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].text, "synthetic safe search note");

        db.connection()
            .execute(
                "UPDATE events SET text = 'tampered projection text' WHERE brain_id = ?1",
                [brain],
            )
            .unwrap();
        let held = search_events_with_retrieval_safety(
            &root,
            brain,
            std::slice::from_ref(&device),
            &mut resolver,
            &db,
            &SearchQuery::default(),
        )
        .unwrap_err();
        assert!(matches!(
            held,
            GuardedSearchError::BrainWideHold(RetrievalSafetyHold::ProjectionDoesNotMatchCanonical)
        ));

        append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Internal,
                text: "new note makes safety snapshot stale",
                event_id: None,
                config: &crate::canonical_writer::CanonicalWriterConfig::default(),
            },
        )
        .unwrap();
        let held = search_events_with_retrieval_safety(
            &root,
            brain,
            std::slice::from_ref(&device),
            &mut resolver,
            &db,
            &SearchQuery::default(),
        )
        .unwrap_err();
        assert!(matches!(
            held,
            GuardedSearchError::BrainWideHold(
                RetrievalSafetyHold::MaterializationStateMissingOrStale
            )
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn guarded_vault_rebuild_publishes_verified_history_and_holds_stale_history() {
        let root = exclusive_temp_dir("m6-guarded-vault-runtime");
        let vault = exclusive_temp_dir("m6-guarded-vault-output");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("m6-vault-device").unwrap();
        let brain = "33333333-3333-4333-8333-333333333333";
        append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Internal,
                text: "synthetic verified vault note",
                event_id: None,
                config: &crate::canonical_writer::CanonicalWriterConfig::default(),
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
        let mut db = ProjectionDb::open_in_memory().unwrap();
        rebuild_device_projection(&store, SecureStoreEnvironment::Dev, &root, &device, &db)
            .unwrap();
        rebuild_knowledge_from_canonical(
            &root,
            brain,
            std::slice::from_ref(&device),
            &mut resolver,
            &mut db,
        )
        .unwrap();

        assert_eq!(
            rebuild_vault_with_retrieval_safety(
                &root,
                brain,
                std::slice::from_ref(&device),
                &mut resolver,
                &db,
                &vault,
            )
            .unwrap()
            .note_count,
            1
        );
        let notes_dir = vault.join("Cryptex").join("notes");
        let note_path = fs::read_dir(&notes_dir)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let published = fs::read(&note_path).unwrap();
        assert!(String::from_utf8_lossy(&published).contains("synthetic verified vault note"));

        append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Internal,
                text: "new note makes projection stale",
                event_id: None,
                config: &crate::canonical_writer::CanonicalWriterConfig::default(),
            },
        )
        .unwrap();
        assert!(matches!(
            rebuild_vault_with_retrieval_safety(
                &root,
                brain,
                std::slice::from_ref(&device),
                &mut resolver,
                &db,
                &vault,
            ),
            Err(GuardedVaultError::BrainWideHold(
                RetrievalSafetyHold::MaterializationStateMissingOrStale
            ))
        ));
        assert_eq!(fs::read(&note_path).unwrap(), published);

        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(vault).unwrap();
    }

    #[test]
    fn expired_retrieval_deadline_issues_brain_wide_hold_without_reading_content() {
        let root = exclusive_temp_dir("retrieval-expired-deadline");
        let db = ProjectionDb::open_in_memory().unwrap();
        let device = DeviceId::new("deadline-test").unwrap();
        let key = ed25519_dalek::SigningKey::from_bytes(&[0x42; 32]).verifying_key();
        let mut resolver = OneTrustedDevice {
            device: device.as_str().to_owned(),
            key_id: cryptex_events::signing_key_id_from_verifying_key(&key)
                .unwrap()
                .as_str()
                .to_owned(),
            key,
        };
        let safety = evaluate_retrieval_safety_until(
            &root,
            "22222222-2222-4222-8222-222222222222",
            &[device],
            &mut resolver,
            &db,
            Some(Instant::now() - std::time::Duration::from_millis(1)),
        );
        assert!(!safety.allows_content());
        assert_eq!(
            safety.hold_reason(),
            Some(RetrievalSafetyHold::OperationDeadlineExceeded)
        );
        assert_eq!(
            db.connection()
                .query_row("SELECT COUNT(*) FROM events", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
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
        let progress = build_progress(&[device], &[], &frames, &[pending], &[], &[], None).unwrap();
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
            None,
        )
        .unwrap();

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
        let error = require_complete_device_inventory(&root, &[trusted], None).unwrap_err();
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
        let progress =
            build_progress(&[device], &[unsupported], &frames, &[], &[], &[], None).unwrap();
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
                event_id: None,
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
                event_id: None,
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
        let result = scan_device_read_only(&root, &device, &mut Vec::new(), None);
        assert!(matches!(
            result,
            Err(KnowledgeMaterializationError::Storage(
                StorageError::Integrity(_)
            ))
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn m8_recovery_scan_omits_only_hash_matched_canonical_tail_segments() {
        let root = exclusive_temp_dir("m8-recovery-policy-baseline");
        let store = FakeSecureStore::new();
        let device = DeviceId::new("m8-recovery-baseline-device").unwrap();
        let brain = "33333333-3333-4333-8333-333333333333";
        let config = crate::canonical_writer::CanonicalWriterConfig {
            seal_event_threshold: 1,
        };
        for text in ["base history", "already-published import tail"] {
            append_memory_note(
                &store,
                SecureStoreEnvironment::Dev,
                AppendMemoryNoteArgs {
                    runtime_root: &root,
                    brain_id: brain,
                    device_id: &device,
                    classification: Classification::Public,
                    text,
                    event_id: None,
                    config: &config,
                },
            )
            .unwrap();
        }

        let tail_path = sealed_segment_path(&root, &device, 1);
        let tail_bytes = fs::read(&tail_path).unwrap();
        let exclusion = ImportRecoverySegmentExclusion {
            device_id: device.clone(),
            segment_index: 1,
            length: tail_bytes.len() as u64,
            sha256: Sha256::digest(&tail_bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
        };

        let mut baseline = Vec::new();
        scan_device_read_only_with_import_exclusions(
            &root,
            &device,
            &mut baseline,
            None,
            std::slice::from_ref(&exclusion),
        )
        .unwrap();
        assert_eq!(baseline.len(), 1);

        let mut hidden_middle = Vec::new();
        let mut middle_exclusion = exclusion.clone();
        let first_bytes = fs::read(sealed_segment_path(&root, &device, 0)).unwrap();
        middle_exclusion.segment_index = 0;
        middle_exclusion.length = first_bytes.len() as u64;
        middle_exclusion.sha256 = Sha256::digest(&first_bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let error = scan_device_read_only_with_import_exclusions(
            &root,
            &device,
            &mut hidden_middle,
            None,
            std::slice::from_ref(&middle_exclusion),
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("do not form a canonical stream tail")
        );

        let mut mismatched = Vec::new();
        let mut bad_digest = exclusion;
        bad_digest.sha256 = "0".repeat(64);
        assert!(
            scan_device_read_only_with_import_exclusions(
                &root,
                &device,
                &mut mismatched,
                None,
                &[bad_digest],
            )
            .is_err()
        );
        assert!(mismatched.is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn m8_import_recovery_safety_checks_projection_against_preimport_history() {
        let root = exclusive_temp_dir("m8-recovery-retrieval-safety");
        let store = FakeSecureStore::new();
        let brain = "33333333-3333-4333-8333-333333333333";
        crate::identity_runtime::write_brain_identity(
            &root,
            &cryptex_core::BrainIdentity::new(brain, "2026-10-29T00:00:00Z").unwrap(),
        )
        .unwrap();
        let local = crate::local_device_identity::enroll_local_device(
            &root,
            &store,
            SecureStoreEnvironment::Dev,
        )
        .unwrap();
        let device = DeviceId::new(local.device_id).unwrap();
        let config = crate::canonical_writer::CanonicalWriterConfig {
            seal_event_threshold: 1,
        };
        append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Public,
                text: "pre-import baseline note",
                event_id: None,
                config: &config,
            },
        )
        .unwrap();

        let mut db = ProjectionDb::open_in_memory().unwrap();
        rebuild_device_projection(&store, SecureStoreEnvironment::Dev, &root, &device, &db)
            .unwrap();
        rebuild_knowledge_from_canonical(
            &root,
            brain,
            std::slice::from_ref(&device),
            &mut LocalDeviceKeyResolver::load(&root, brain, &store, SecureStoreEnvironment::Dev)
                .unwrap(),
            &mut db,
        )
        .unwrap();

        append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: brain,
                device_id: &device,
                classification: Classification::Public,
                text: "already-published synthetic import",
                event_id: None,
                config: &config,
            },
        )
        .unwrap();
        let imported_path = sealed_segment_path(&root, &device, 1);
        let imported_bytes = fs::read(&imported_path).unwrap();
        let exclusion = ImportRecoverySegmentExclusion {
            device_id: device.clone(),
            segment_index: 1,
            length: imported_bytes.len() as u64,
            sha256: Sha256::digest(&imported_bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
        };
        let mut resolver =
            LocalDeviceKeyResolver::load(&root, brain, &store, SecureStoreEnvironment::Dev)
                .unwrap();
        let recovery_safety = evaluate_import_recovery_retrieval_safety(
            &root,
            brain,
            std::slice::from_ref(&device),
            std::slice::from_ref(&exclusion),
            &mut resolver,
            &db,
        );
        assert!(recovery_safety.allows_content());
        drop(recovery_safety);

        let mut ordinary_resolver =
            LocalDeviceKeyResolver::load(&root, brain, &store, SecureStoreEnvironment::Dev)
                .unwrap();
        let ordinary_safety = evaluate_retrieval_safety(
            &root,
            brain,
            std::slice::from_ref(&device),
            &mut ordinary_resolver,
            &db,
        );
        assert!(!ordinary_safety.allows_content());
        drop(ordinary_safety);
        fs::remove_dir_all(root).unwrap();
    }
}
