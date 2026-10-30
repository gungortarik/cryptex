//! Bounded daemon-owned cache for resumable M8 bootstrap transactions.
//!
//! The journal contains public protocol data only. It is never a trust source:
//! the canonical local approval event remains authoritative. The Windows
//! approval IPC, local canonical append, and startup reconciliation consume
//! this journal; none treats journal state alone as trust authority. macOS
//! completion remains fail-closed pending process-instance peer binding.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use cryptex_core::ids::DeviceId;
use cryptex_core::validate::parse_uuid_str;
use cryptex_events::{
    parse_bootstrap_message, reject_duplicate_json_keys, verify_bootstrap_transcript,
};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::paths::assert_outside_repository;

const JOURNAL_SCHEMA_VERSION: u32 = 2;
const LEGACY_JOURNAL_SCHEMA_VERSION: u32 = 1;
const MAX_JOURNAL_BYTES: usize = 1024 * 1024;
const MAX_PENDING_TRANSACTIONS: usize = 16;
const MAX_TRANSCRIPT_MESSAGES: usize = 16;
const MAX_APPEND_ATTEMPTS: usize = 4096;
const JOURNAL_DIR: &str = "bootstrap";
const JOURNAL_FILE: &str = "pending.json";
const JOURNAL_LOCK: &str = "pending.lock";
const SNAPSHOT_DIGEST_DOMAIN: &[u8] = b"cryptex-owner-approval-request-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootstrapJournalError {
    Io(String),
    Corrupt(&'static str),
    Limit(&'static str),
    Conflict(&'static str),
    Durability(String),
}

impl std::fmt::Display for BootstrapJournalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "bootstrap journal I/O failed: {error}"),
            Self::Corrupt(reason) => write!(f, "bootstrap journal is invalid: {reason}"),
            Self::Limit(reason) => write!(f, "bootstrap journal limit exceeded: {reason}"),
            Self::Conflict(reason) => write!(f, "bootstrap transaction conflict: {reason}"),
            Self::Durability(error) => {
                write!(f, "bootstrap journal durability is unconfirmed: {error}")
            }
        }
    }
}

impl std::error::Error for BootstrapJournalError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapOperation {
    PeerAdmission,
    DeviceRestriction,
    SelfRetirement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestrictionReason {
    Retired,
    Compromised,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapRole {
    Initiator,
    Responder,
    Issuer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapExchangeRole {
    Initiator,
    Responder,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalIdentity {
    pub device_id: String,
    pub signing_key_id: String,
    pub public_key_fingerprint_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransportAssociation {
    pub local_device_id: String,
    pub peer_device_id: String,
}

/// Exact immutable R1 owner-visible operation snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalSnapshot {
    pub brain_id: String,
    pub transaction_id: String,
    pub role: BootstrapRole,
    pub operation: BootstrapOperation,
    pub agreement_hash: Option<String>,
    pub previous_message_hash: Option<String>,
    pub local_identity: ApprovalIdentity,
    pub peer_identity: Option<ApprovalIdentity>,
    pub transport_association: Option<TransportAssociation>,
    pub restriction_target: Option<String>,
    pub restriction_reason: Option<RestrictionReason>,
    pub cutoff_seq: Option<String>,
    pub cutoff_content_hash: Option<String>,
    pub last_authorized_seq: Option<String>,
    pub last_authorized_content_hash: Option<String>,
}

impl ApprovalSnapshot {
    /// Freeze the owner-visible snapshot only from an authenticated bootstrap
    /// prefix, never from renderer-supplied identity or transport fields.
    pub fn from_verified_bootstrap_request(
        request: &cryptex_events::VerifiedBootstrapApprovalRequest,
    ) -> Result<Self, BootstrapJournalError> {
        let snapshot = Self {
            brain_id: request.brain_id().to_owned(),
            transaction_id: request.transaction_id().to_owned(),
            role: match request.role() {
                cryptex_events::BootstrapApprovalRole::Initiator => BootstrapRole::Initiator,
                cryptex_events::BootstrapApprovalRole::Responder => BootstrapRole::Responder,
            },
            operation: BootstrapOperation::PeerAdmission,
            agreement_hash: Some(request.agreement_hash().to_owned()),
            previous_message_hash: Some(request.previous_message_hash().to_owned()),
            local_identity: approval_identity_from_bootstrap(request.local_identity()),
            peer_identity: Some(approval_identity_from_bootstrap(request.peer_identity())),
            transport_association: Some(TransportAssociation {
                local_device_id: request.local_transport_device_id().to_owned(),
                peer_device_id: request.peer_transport_device_id().to_owned(),
            }),
            restriction_target: None,
            restriction_reason: None,
            cutoff_seq: None,
            cutoff_content_hash: None,
            last_authorized_seq: None,
            last_authorized_content_hash: None,
        };
        snapshot.validate_shape()?;
        Ok(snapshot)
    }

    pub fn digest(&self) -> Result<String, BootstrapJournalError> {
        self.validate_shape()?;
        let value = serde_json::to_value(self)
            .map_err(|_| BootstrapJournalError::Corrupt("snapshot serialization failed"))?;
        let canonical = serde_json_canonicalizer::to_vec(&value)
            .map_err(|_| BootstrapJournalError::Corrupt("snapshot canonicalization failed"))?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(SNAPSHOT_DIGEST_DOMAIN);
        hasher.update(&[0]);
        hasher.update(&canonical);
        Ok(hex_lower(hasher.finalize().as_bytes()))
    }

    pub fn matches_bootstrap_request(
        &self,
        request: &cryptex_events::VerifiedBootstrapApprovalRequest,
    ) -> bool {
        self.operation == BootstrapOperation::PeerAdmission
            && self.brain_id == request.brain_id()
            && self.transaction_id == request.transaction_id()
            && self.role
                == match request.role() {
                    cryptex_events::BootstrapApprovalRole::Initiator => BootstrapRole::Initiator,
                    cryptex_events::BootstrapApprovalRole::Responder => BootstrapRole::Responder,
                }
            && self.agreement_hash.as_deref() == Some(request.agreement_hash())
            && self.previous_message_hash.as_deref() == Some(request.previous_message_hash())
            && identity_matches(&self.local_identity, request.local_identity())
            && self
                .peer_identity
                .as_ref()
                .is_some_and(|identity| identity_matches(identity, request.peer_identity()))
            && self
                .transport_association
                .as_ref()
                .is_some_and(|association| {
                    association.local_device_id == request.local_transport_device_id()
                        && association.peer_device_id == request.peer_transport_device_id()
                })
            && self.restriction_target.is_none()
            && self.restriction_reason.is_none()
            && self.cutoff_seq.is_none()
            && self.cutoff_content_hash.is_none()
            && self.last_authorized_seq.is_none()
            && self.last_authorized_content_hash.is_none()
    }

    fn validate_shape(&self) -> Result<(), BootstrapJournalError> {
        validate_canonical_uuid(&self.brain_id, "Brain ID")?;
        validate_lower_hex(&self.transaction_id, 64, "transaction ID")?;
        validate_identity(&self.local_identity)?;
        match self.operation {
            BootstrapOperation::PeerAdmission => {
                if !matches!(
                    self.role,
                    BootstrapRole::Initiator | BootstrapRole::Responder
                ) || self.peer_identity.is_none()
                    || self.transport_association.is_none()
                    || self.restriction_target.is_some()
                    || self.restriction_reason.is_some()
                    || self.cutoff_seq.is_some()
                    || self.cutoff_content_hash.is_some()
                    || self.last_authorized_seq.is_some()
                    || self.last_authorized_content_hash.is_some()
                {
                    return Err(BootstrapJournalError::Corrupt(
                        "admission snapshot fields do not match operation",
                    ));
                }
                validate_lower_hex(
                    self.agreement_hash
                        .as_deref()
                        .ok_or(BootstrapJournalError::Corrupt(
                            "admission agreement missing",
                        ))?,
                    64,
                    "agreement hash",
                )?;
                validate_lower_hex(
                    self.previous_message_hash
                        .as_deref()
                        .ok_or(BootstrapJournalError::Corrupt(
                            "admission predecessor missing",
                        ))?,
                    64,
                    "previous message hash",
                )?;
                validate_identity(self.peer_identity.as_ref().expect("checked above"))?;
                validate_transport(self.transport_association.as_ref().expect("checked above"))?;
                let peer = self.peer_identity.as_ref().expect("checked above");
                if self.local_identity.device_id == peer.device_id
                    || self.local_identity.signing_key_id == peer.signing_key_id
                    || self.local_identity.public_key_fingerprint_sha256
                        == peer.public_key_fingerprint_sha256
                {
                    return Err(BootstrapJournalError::Corrupt(
                        "admission identities are not distinct",
                    ));
                }
            }
            BootstrapOperation::DeviceRestriction => {
                if self.role != BootstrapRole::Issuer
                    || self.peer_identity.is_none()
                    || self.transport_association.is_none()
                    || self.restriction_target.is_none()
                    || self.restriction_reason.is_none()
                    || self.cutoff_seq.is_none()
                    || self.cutoff_content_hash.is_none()
                    || self.last_authorized_seq.is_some()
                    || self.last_authorized_content_hash.is_some()
                    || self.agreement_hash.is_some()
                    || self.previous_message_hash.is_some()
                {
                    return Err(BootstrapJournalError::Corrupt(
                        "restriction snapshot fields do not match operation",
                    ));
                }
                validate_identity(self.peer_identity.as_ref().expect("checked above"))?;
                validate_transport(self.transport_association.as_ref().expect("checked above"))?;
                validate_restriction_fields(
                    self.restriction_target.as_deref().expect("checked above"),
                    self.cutoff_seq.as_deref().expect("checked above"),
                    self.cutoff_content_hash.as_deref().expect("checked above"),
                )?;
                if !matches!(
                    self.restriction_reason,
                    Some(RestrictionReason::Retired | RestrictionReason::Compromised)
                ) {
                    return Err(BootstrapJournalError::Corrupt("restriction reason invalid"));
                }
                if self.restriction_target.as_deref().expect("checked above")
                    != self
                        .peer_identity
                        .as_ref()
                        .expect("checked above")
                        .device_id
                {
                    return Err(BootstrapJournalError::Corrupt(
                        "restriction target differs from peer identity",
                    ));
                }
            }
            BootstrapOperation::SelfRetirement => {
                if self.role != BootstrapRole::Issuer
                    || self.peer_identity.is_some()
                    || self.transport_association.is_some()
                    || self.restriction_target.is_some()
                    || self.restriction_reason.is_some()
                    || self.cutoff_seq.is_some()
                    || self.cutoff_content_hash.is_some()
                    || self.last_authorized_seq.is_none()
                    || self.last_authorized_content_hash.is_none()
                    || self.agreement_hash.is_some()
                    || self.previous_message_hash.is_some()
                {
                    return Err(BootstrapJournalError::Corrupt(
                        "self-retirement snapshot fields do not match operation",
                    ));
                }
                validate_content_boundary(
                    self.last_authorized_seq.as_deref().expect("checked above"),
                    self.last_authorized_content_hash
                        .as_deref()
                        .expect("checked above"),
                    "self-retirement boundary",
                )?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapState {
    OfferCreated,
    OfferReceived,
    ProofReceived,
    ProofCreated,
    PeerApprovalReceived,
    LocalApprovalSigned,
    TranscriptReady,
    OwnerPending,
    OwnerApproved,
    AppendUnknown,
    Anchored,
    Cancelled,
    CancelledAfterRelease,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppendAttempt {
    pub reserved_sequence: u64,
    pub predecessor_hash: String,
    pub candidate_content_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingBootstrapTransaction {
    pub snapshot: ApprovalSnapshot,
    pub approval_request_digest: String,
    /// Exact UTF-8 signed message bytes. These remain public protocol data.
    pub transcript_messages: Vec<String>,
    pub transcript_hash: Option<String>,
    pub state: BootstrapState,
    pub local_approval_signature_generated: bool,
    pub local_approval_signature_released: bool,
    pub stable_event_id: String,
    pub append_attempts: Vec<AppendAttempt>,
}

impl PendingBootstrapTransaction {
    pub fn new(
        snapshot: ApprovalSnapshot,
        stable_event_id: String,
    ) -> Result<Self, BootstrapJournalError> {
        snapshot.validate_shape()?;
        validate_canonical_uuid(&stable_event_id, "event ID")?;
        let state = match (snapshot.operation, snapshot.role) {
            (BootstrapOperation::PeerAdmission, BootstrapRole::Initiator) => {
                BootstrapState::OfferCreated
            }
            (BootstrapOperation::PeerAdmission, BootstrapRole::Responder) => {
                BootstrapState::OfferReceived
            }
            (BootstrapOperation::PeerAdmission, _) => {
                return Err(BootstrapJournalError::Corrupt(
                    "peer admission snapshot has issuer role",
                ));
            }
            (
                BootstrapOperation::DeviceRestriction | BootstrapOperation::SelfRetirement,
                BootstrapRole::Issuer,
            ) => BootstrapState::OwnerPending,
            _ => {
                return Err(BootstrapJournalError::Corrupt(
                    "owner operation requires issuer role",
                ));
            }
        };
        Ok(Self {
            approval_request_digest: snapshot.digest()?,
            snapshot,
            transcript_messages: Vec::new(),
            transcript_hash: None,
            state,
            local_approval_signature_generated: false,
            local_approval_signature_released: false,
            stable_event_id,
            append_attempts: Vec::new(),
        })
    }

    /// Persist the exact authenticated prefix that was used to derive this
    /// transaction's approval snapshot. Replaying identical bytes is
    /// idempotent; a different prefix for the same transaction is a conflict.
    pub fn record_verified_approval_prefix(
        &mut self,
        request: &cryptex_events::VerifiedBootstrapApprovalRequest,
        raw_messages: &[Vec<u8>],
    ) -> Result<(), BootstrapJournalError> {
        let role = match request.role() {
            cryptex_events::BootstrapApprovalRole::Initiator => BootstrapRole::Initiator,
            cryptex_events::BootstrapApprovalRole::Responder => BootstrapRole::Responder,
        };
        let refs = raw_messages.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let verified = cryptex_events::verify_bootstrap_approval_request(&refs, request.role())
            .map_err(|_| BootstrapJournalError::Corrupt("approval prefix verification failed"))?;
        let derived_snapshot = ApprovalSnapshot::from_verified_bootstrap_request(&verified)?;
        if &verified != request || derived_snapshot != self.snapshot {
            return Err(BootstrapJournalError::Conflict(
                "approval prefix differs from transaction snapshot",
            ));
        }
        let messages = raw_messages
            .iter()
            .map(|message| {
                std::str::from_utf8(message)
                    .map(str::to_owned)
                    .map_err(|_| BootstrapJournalError::Corrupt("approval prefix is not UTF-8"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let target_state = match role {
            BootstrapRole::Initiator => BootstrapState::ProofReceived,
            BootstrapRole::Responder => BootstrapState::PeerApprovalReceived,
            BootstrapRole::Issuer => {
                return Err(BootstrapJournalError::Corrupt(
                    "peer admission approval has issuer role",
                ));
            }
        };
        if self.transcript_messages.len() >= messages.len()
            && self.transcript_messages[..messages.len()] == messages
            && !matches!(
                self.state,
                BootstrapState::Cancelled | BootstrapState::CancelledAfterRelease
            )
        {
            return self.validate();
        }
        let initial_state = match role {
            BootstrapRole::Initiator => BootstrapState::OfferCreated,
            BootstrapRole::Responder => BootstrapState::OfferReceived,
            BootstrapRole::Issuer => unreachable!("issuer role rejected above"),
        };
        if self.state != initial_state || !self.transcript_messages.is_empty() {
            return Err(BootstrapJournalError::Conflict(
                "transaction already contains a different approval prefix",
            ));
        }

        let mut candidate = self.clone();
        candidate.transcript_messages = messages;
        if role == BootstrapRole::Responder {
            candidate.transition(BootstrapState::ProofCreated)?;
        }
        candidate.transition(target_state)?;
        *self = candidate;
        Ok(())
    }

    /// Record the locally signed approval in the journal before returning it
    /// to the IPC caller. This method verifies the new message against the
    /// pinned transcript and records release conservatively in the same
    /// atomic journal replacement.
    pub fn record_local_approval_signature(
        &mut self,
        request: &cryptex_events::VerifiedBootstrapApprovalRequest,
        signed_message: &[u8],
    ) -> Result<(), BootstrapJournalError> {
        let expected_snapshot = ApprovalSnapshot::from_verified_bootstrap_request(request)?;
        if expected_snapshot != self.snapshot {
            return Err(BootstrapJournalError::Conflict(
                "approval signature request differs from transaction snapshot",
            ));
        }
        let signed_message = std::str::from_utf8(signed_message)
            .map_err(|_| BootstrapJournalError::Corrupt("signed approval is not UTF-8"))?;
        if signed_message.len() > 64 * 1024 {
            return Err(BootstrapJournalError::Limit("signed approval bytes"));
        }
        let expected_state = match self.snapshot.role {
            BootstrapRole::Initiator => BootstrapState::ProofReceived,
            BootstrapRole::Responder => BootstrapState::PeerApprovalReceived,
            BootstrapRole::Issuer => {
                return Err(BootstrapJournalError::Corrupt(
                    "peer admission approval has issuer role",
                ));
            }
        };
        if self.local_approval_signature_generated {
            if self.local_approval_signature_released
                && self.transcript_messages.last().map(String::as_str) == Some(signed_message)
            {
                return self.validate();
            }
            return Err(BootstrapJournalError::Conflict(
                "transaction already has a different local approval signature",
            ));
        }
        if self.state != expected_state {
            return Err(BootstrapJournalError::Conflict(
                "transaction is not ready for local approval signature",
            ));
        }
        let mut candidate = self.clone();
        candidate
            .transcript_messages
            .push(signed_message.to_owned());
        candidate.local_approval_signature_generated = true;
        candidate.local_approval_signature_released = true;
        if self.snapshot.role == BootstrapRole::Initiator {
            let refs = candidate
                .transcript_messages
                .iter()
                .map(String::as_bytes)
                .collect::<Vec<_>>();
            let responder_request = cryptex_events::verify_bootstrap_approval_request(
                &refs,
                cryptex_events::BootstrapApprovalRole::Responder,
            )
            .map_err(|_| BootstrapJournalError::Corrupt("signed initiator approval is invalid"))?;
            if responder_request.transaction_id() != request.transaction_id()
                || responder_request.brain_id() != request.brain_id()
                || responder_request.agreement_hash() != request.agreement_hash()
                || responder_request.local_identity() != request.peer_identity()
                || responder_request.peer_identity() != request.local_identity()
                || responder_request.local_transport_device_id()
                    != request.peer_transport_device_id()
                || responder_request.peer_transport_device_id()
                    != request.local_transport_device_id()
            {
                return Err(BootstrapJournalError::Conflict(
                    "signed initiator approval differs from pinned prefix",
                ));
            }
        } else {
            let refs = candidate
                .transcript_messages
                .iter()
                .map(String::as_bytes)
                .collect::<Vec<_>>();
            let transcript = cryptex_events::verify_bootstrap_transcript(&refs)
                .map_err(|_| BootstrapJournalError::Corrupt("bootstrap transcript is invalid"))?;
            candidate.transcript_hash = Some(transcript.transcript_hash().to_owned());
        }
        candidate.transition(BootstrapState::LocalApprovalSigned)?;
        if self.snapshot.role == BootstrapRole::Responder {
            candidate.transition(BootstrapState::TranscriptReady)?;
        }
        *self = candidate;
        Ok(())
    }

    /// Attach the exact completed bilateral transcript after the missing peer
    /// approval arrives. The already journaled prefix and local signature must
    /// remain byte-identical; this transition never signs or appends anything.
    pub fn record_verified_complete_transcript(
        &mut self,
        raw_messages: &[Vec<u8>],
    ) -> Result<(), BootstrapJournalError> {
        if self.snapshot.operation != BootstrapOperation::PeerAdmission
            || !self.local_approval_signature_generated
            || !self.local_approval_signature_released
        {
            return Err(BootstrapJournalError::Conflict(
                "completed transcript requires a released local admission signature",
            ));
        }
        let refs = raw_messages.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let verified = verify_bootstrap_transcript(&refs)
            .map_err(|_| BootstrapJournalError::Corrupt("completed transcript is invalid"))?;
        let (local, peer, local_transport, peer_transport) = match self.snapshot.role {
            BootstrapRole::Initiator => (
                verified.initiator(),
                verified.responder(),
                verified.initiator_transport_device_id(),
                verified.responder_transport_device_id(),
            ),
            BootstrapRole::Responder => (
                verified.responder(),
                verified.initiator(),
                verified.responder_transport_device_id(),
                verified.initiator_transport_device_id(),
            ),
            BootstrapRole::Issuer => unreachable!("issuer rejected above"),
        };
        let peer_snapshot =
            self.snapshot
                .peer_identity
                .as_ref()
                .ok_or(BootstrapJournalError::Corrupt(
                    "admission peer identity missing",
                ))?;
        let association =
            self.snapshot
                .transport_association
                .as_ref()
                .ok_or(BootstrapJournalError::Corrupt(
                    "admission transport association missing",
                ))?;
        let expected_previous_message_hash = match self.snapshot.role {
            BootstrapRole::Initiator => verified.proof_hash(),
            BootstrapRole::Responder => verified.initiator_approval_hash(),
            BootstrapRole::Issuer => unreachable!("issuer rejected by admission shape"),
        };
        if verified.transaction_id() != self.snapshot.transaction_id
            || verified.brain_id() != self.snapshot.brain_id
            || verified.agreement_hash()
                != self.snapshot.agreement_hash.as_deref().unwrap_or_default()
            || self.snapshot.previous_message_hash.as_deref()
                != Some(expected_previous_message_hash)
            || !identity_matches(&self.snapshot.local_identity, local)
            || !identity_matches(peer_snapshot, peer)
            || association.local_device_id != *local_transport
            || association.peer_device_id != *peer_transport
        {
            return Err(BootstrapJournalError::Conflict(
                "completed transcript differs from the frozen approval snapshot",
            ));
        }
        if let Some(existing_hash) = self.transcript_hash.as_deref() {
            if existing_hash == verified.transcript_hash()
                && self.transcript_messages.len() == raw_messages.len()
                && self
                    .transcript_messages
                    .iter()
                    .zip(raw_messages)
                    .all(|(saved, incoming)| saved.as_bytes() == incoming)
            {
                return self.validate();
            }
            return Err(BootstrapJournalError::Conflict(
                "transaction already has a different completed transcript",
            ));
        }
        let prefix_len = self.transcript_messages.len();
        if !matches!(prefix_len, 3 | 4)
            || prefix_len > raw_messages.len()
            || !self
                .transcript_messages
                .iter()
                .zip(raw_messages)
                .all(|(saved, incoming)| saved.as_bytes() == incoming)
        {
            return Err(BootstrapJournalError::Conflict(
                "completed transcript does not extend the journaled signed prefix",
            ));
        }
        if self.snapshot.role == BootstrapRole::Responder && prefix_len != 4 {
            return Err(BootstrapJournalError::Conflict(
                "responder transcript is not already complete",
            ));
        }
        let mut candidate = self.clone();
        candidate.transcript_messages = raw_messages
            .iter()
            .map(|message| {
                std::str::from_utf8(message)
                    .map(str::to_owned)
                    .map_err(|_| BootstrapJournalError::Corrupt("transcript is not UTF-8"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        candidate.transcript_hash = Some(verified.transcript_hash().to_owned());
        if candidate.state == BootstrapState::LocalApprovalSigned {
            candidate.transition(BootstrapState::TranscriptReady)?;
        } else if candidate.state != BootstrapState::TranscriptReady {
            return Err(BootstrapJournalError::Conflict(
                "transaction is not awaiting a complete transcript",
            ));
        }
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Persist the exact candidate append before the canonical writer attempts
    /// I/O. The caller must hold the canonical inventory and device locks and
    /// must have verified the complete local stream immediately before calling
    /// this method. The journal remains recovery metadata, never authority.
    #[allow(dead_code)] // Wired only after canonical reconciliation is implemented.
    pub(crate) fn record_append_attempt(
        &mut self,
        attempt: AppendAttempt,
    ) -> Result<(), BootstrapJournalError> {
        self.validate()?;
        let is_peer_admission = self.snapshot.operation == BootstrapOperation::PeerAdmission;
        let is_confirmed_owner_operation = !is_peer_admission
            && self.snapshot.role == BootstrapRole::Issuer
            && self.state == BootstrapState::OwnerApproved
            && self.transcript_hash.is_none()
            && self.transcript_messages.is_empty();
        let is_ready_admission = is_peer_admission
            && self.transcript_hash.is_some()
            && matches!(
                self.state,
                BootstrapState::TranscriptReady | BootstrapState::AppendUnknown
            );
        if !is_confirmed_owner_operation && !is_ready_admission {
            return Err(BootstrapJournalError::Conflict(
                "append attempt lacks a confirmed, frozen operation; append refused",
            ));
        }
        if attempt.reserved_sequence == 0 {
            return Err(BootstrapJournalError::Corrupt(
                "append attempt reserved sequence is zero",
            ));
        }
        validate_lower_hex(&attempt.predecessor_hash, 64, "append predecessor hash")?;
        let candidate_hash =
            attempt
                .candidate_content_hash
                .as_deref()
                .ok_or(BootstrapJournalError::Corrupt(
                    "append attempt candidate hash is missing",
                ))?;
        validate_lower_hex(candidate_hash, 64, "append candidate content hash")?;
        if self
            .append_attempts
            .iter()
            .any(|existing| existing.reserved_sequence == attempt.reserved_sequence)
        {
            return Err(BootstrapJournalError::Conflict(
                "append attempt reuses a reserved sequence",
            ));
        }
        if self.append_attempts.len() >= MAX_APPEND_ATTEMPTS {
            return Err(BootstrapJournalError::Limit("append attempt count"));
        }
        let mut candidate = self.clone();
        candidate.append_attempts.push(attempt);
        if matches!(
            candidate.state,
            BootstrapState::TranscriptReady | BootstrapState::OwnerApproved
        ) {
            candidate.transition(BootstrapState::AppendUnknown)?;
        }
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Record owner confirmation of an immutable non-admission security
    /// operation. The IPC caller must consume the connection-bound capability
    /// before invoking this transition.
    fn record_owner_confirmation(
        &mut self,
        approval_request_digest: &str,
    ) -> Result<(), BootstrapJournalError> {
        self.validate()?;
        if self.snapshot.operation == BootstrapOperation::PeerAdmission
            || self.snapshot.digest()? != approval_request_digest
            || self.state != BootstrapState::OwnerPending
        {
            return Err(BootstrapJournalError::Conflict(
                "owner confirmation differs from a pending security operation",
            ));
        }
        self.transition(BootstrapState::OwnerApproved)
    }

    /// Record an anchored cache state only after the caller has re-read the
    /// canonical stream under the same inventory/device locks and matched the
    /// exact transaction, event, transcript, signer, sequence, and content
    /// hash. Startup must independently reconcile this cache with canonical
    /// history before trusting it.
    #[allow(dead_code)] // Must be called only with a lock-scoped canonical proof.
    pub(crate) fn record_verified_anchor(
        &mut self,
        reserved_sequence: u64,
        content_hash: &str,
    ) -> Result<(), BootstrapJournalError> {
        self.validate()?;
        validate_lower_hex(content_hash, 64, "verified anchor content hash")?;
        if self.state != BootstrapState::AppendUnknown {
            return Err(BootstrapJournalError::Conflict(
                "anchor confirmation requires an uncertain append",
            ));
        }
        let attempt = self
            .append_attempts
            .iter()
            .find(|attempt| attempt.reserved_sequence == reserved_sequence)
            .ok_or(BootstrapJournalError::Conflict(
                "verified anchor has no matching append attempt",
            ))?;
        if attempt.candidate_content_hash.as_deref() != Some(content_hash) {
            return Err(BootstrapJournalError::Conflict(
                "verified anchor differs from the recorded append candidate",
            ));
        }
        let mut candidate = self.clone();
        candidate.state = BootstrapState::Anchored;
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    pub fn transition(&mut self, next: BootstrapState) -> Result<(), BootstrapJournalError> {
        let allowed = match (self.snapshot.role, self.state, next) {
            (
                BootstrapRole::Initiator,
                BootstrapState::OfferCreated,
                BootstrapState::ProofReceived,
            )
            | (
                BootstrapRole::Initiator,
                BootstrapState::ProofReceived,
                BootstrapState::LocalApprovalSigned,
            )
            | (
                BootstrapRole::Initiator,
                BootstrapState::LocalApprovalSigned,
                BootstrapState::TranscriptReady,
            )
            | (
                BootstrapRole::Responder,
                BootstrapState::OfferReceived,
                BootstrapState::ProofCreated,
            )
            | (
                BootstrapRole::Responder,
                BootstrapState::ProofCreated,
                BootstrapState::PeerApprovalReceived,
            )
            | (
                BootstrapRole::Responder,
                BootstrapState::PeerApprovalReceived,
                BootstrapState::LocalApprovalSigned,
            )
            | (
                BootstrapRole::Responder,
                BootstrapState::LocalApprovalSigned,
                BootstrapState::TranscriptReady,
            )
            | (
                BootstrapRole::Issuer,
                BootstrapState::OwnerPending,
                BootstrapState::OwnerApproved,
            )
            | (
                BootstrapRole::Issuer,
                BootstrapState::OwnerApproved,
                BootstrapState::AppendUnknown,
            )
            | (_, BootstrapState::TranscriptReady, BootstrapState::AppendUnknown)
            | (_, BootstrapState::AppendUnknown, BootstrapState::TranscriptReady) => true,
            (_, current, BootstrapState::Cancelled)
                if !matches!(
                    current,
                    BootstrapState::Anchored | BootstrapState::CancelledAfterRelease
                ) =>
            {
                true
            }
            (_, current, BootstrapState::CancelledAfterRelease)
                if self.local_approval_signature_released
                    && !matches!(
                        current,
                        BootstrapState::Anchored | BootstrapState::Cancelled
                    ) =>
            {
                true
            }
            _ => false,
        };
        if !allowed {
            return Err(BootstrapJournalError::Conflict(
                "illegal transaction state transition",
            ));
        }
        let previous = self.state;
        self.state = next;
        if let Err(error) = self.validate() {
            self.state = previous;
            return Err(error);
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), BootstrapJournalError> {
        self.snapshot.validate_shape()?;
        validate_canonical_uuid(&self.stable_event_id, "event ID")?;
        validate_lower_hex(&self.approval_request_digest, 64, "approval request digest")?;
        if self.snapshot.digest()? != self.approval_request_digest {
            return Err(BootstrapJournalError::Corrupt(
                "approval snapshot digest differs",
            ));
        }
        if self.transcript_messages.len() > MAX_TRANSCRIPT_MESSAGES {
            return Err(BootstrapJournalError::Limit("transcript message count"));
        }
        for message in &self.transcript_messages {
            if message.len() > 64 * 1024 {
                return Err(BootstrapJournalError::Limit("bootstrap message bytes"));
            }
            let parsed = parse_bootstrap_message(message.as_bytes())
                .map_err(|_| BootstrapJournalError::Corrupt("transcript message invalid"))?;
            if parsed.get("brain_id").and_then(Value::as_str) != Some(&self.snapshot.brain_id)
                || parsed.get("transaction_id").and_then(Value::as_str)
                    != Some(&self.snapshot.transaction_id)
            {
                return Err(BootstrapJournalError::Conflict(
                    "transcript binding differs",
                ));
            }
        }
        match (&self.transcript_hash, self.transcript_messages.len()) {
            (Some(hash), 4) => {
                validate_lower_hex(hash, 64, "transcript hash")?;
                let refs = self
                    .transcript_messages
                    .iter()
                    .map(String::as_bytes)
                    .collect::<Vec<_>>();
                let verified = verify_bootstrap_transcript(&refs)
                    .map_err(|_| BootstrapJournalError::Corrupt("complete transcript invalid"))?;
                if verified.transcript_hash() != hash {
                    return Err(BootstrapJournalError::Corrupt("transcript digest differs"));
                }
                if self.snapshot.operation == BootstrapOperation::PeerAdmission {
                    let (local, peer, local_transport, peer_transport, previous_hash) =
                        match self.snapshot.role {
                            BootstrapRole::Initiator => (
                                verified.initiator(),
                                verified.responder(),
                                verified.initiator_transport_device_id(),
                                verified.responder_transport_device_id(),
                                verified.proof_hash(),
                            ),
                            BootstrapRole::Responder => (
                                verified.responder(),
                                verified.initiator(),
                                verified.responder_transport_device_id(),
                                verified.initiator_transport_device_id(),
                                verified.initiator_approval_hash(),
                            ),
                            BootstrapRole::Issuer => {
                                return Err(BootstrapJournalError::Corrupt(
                                    "admission snapshot has issuer role",
                                ));
                            }
                        };
                    let snapshot_peer = self.snapshot.peer_identity.as_ref().ok_or(
                        BootstrapJournalError::Corrupt("admission peer identity missing"),
                    )?;
                    let association = self.snapshot.transport_association.as_ref().ok_or(
                        BootstrapJournalError::Corrupt("admission transport missing"),
                    )?;
                    if verified.transaction_id() != self.snapshot.transaction_id
                        || verified.brain_id() != self.snapshot.brain_id
                        || verified.agreement_hash()
                            != self.snapshot.agreement_hash.as_deref().unwrap_or_default()
                        || previous_hash
                            != self
                                .snapshot
                                .previous_message_hash
                                .as_deref()
                                .unwrap_or_default()
                        || !identity_matches(&self.snapshot.local_identity, local)
                        || !identity_matches(snapshot_peer, peer)
                        || association.local_device_id != *local_transport
                        || association.peer_device_id != *peer_transport
                    {
                        return Err(BootstrapJournalError::Conflict(
                            "verified transcript differs from approval snapshot",
                        ));
                    }
                }
            }
            (None, 0..=3) => {}
            _ => {
                return Err(BootstrapJournalError::Corrupt(
                    "transcript hash/state differs",
                ));
            }
        }
        if self.local_approval_signature_released && !self.local_approval_signature_generated {
            return Err(BootstrapJournalError::Corrupt(
                "released approval signature was not generated locally",
            ));
        }
        if matches!(
            self.state,
            BootstrapState::LocalApprovalSigned
                | BootstrapState::TranscriptReady
                | BootstrapState::AppendUnknown
                | BootstrapState::Anchored
        ) && !self.local_approval_signature_generated
            && self.snapshot.operation == BootstrapOperation::PeerAdmission
        {
            return Err(BootstrapJournalError::Corrupt(
                "approval state reached before local signature generation",
            ));
        }
        if matches!(
            self.state,
            BootstrapState::OwnerPending | BootstrapState::OwnerApproved
        ) && (self.snapshot.operation == BootstrapOperation::PeerAdmission
            || self.snapshot.role != BootstrapRole::Issuer
            || self.transcript_hash.is_some()
            || !self.transcript_messages.is_empty())
        {
            return Err(BootstrapJournalError::Corrupt(
                "owner-operation state has admission or transcript data",
            ));
        }
        if self.append_attempts.len() > MAX_APPEND_ATTEMPTS {
            return Err(BootstrapJournalError::Limit("append attempt count"));
        }
        let mut seen_sequences = std::collections::BTreeSet::new();
        for attempt in &self.append_attempts {
            if attempt.reserved_sequence == 0 || !seen_sequences.insert(attempt.reserved_sequence) {
                return Err(BootstrapJournalError::Corrupt(
                    "append attempts contain a zero or repeated sequence",
                ));
            }
            validate_lower_hex(&attempt.predecessor_hash, 64, "append predecessor hash")?;
            if let Some(hash) = &attempt.candidate_content_hash {
                validate_lower_hex(hash, 64, "append candidate content hash")?;
            }
        }
        match self.state {
            BootstrapState::TranscriptReady
            | BootstrapState::AppendUnknown
            | BootstrapState::Anchored
                if self.snapshot.operation == BootstrapOperation::PeerAdmission
                    && self.transcript_hash.is_none() =>
            {
                return Err(BootstrapJournalError::Corrupt(
                    "admission advanced without a verified transcript",
                ));
            }
            BootstrapState::Anchored
                if self.transcript_hash.is_none()
                    && self.snapshot.operation == BootstrapOperation::PeerAdmission =>
            {
                return Err(BootstrapJournalError::Corrupt(
                    "anchored admission has no transcript",
                ));
            }
            BootstrapState::AppendUnknown
                if self.append_attempts.is_empty()
                    || (self.snapshot.operation == BootstrapOperation::PeerAdmission
                        && self.transcript_hash.is_none())
                    || (self.snapshot.operation != BootstrapOperation::PeerAdmission
                        && self.snapshot.role != BootstrapRole::Issuer) =>
            {
                return Err(BootstrapJournalError::Corrupt(
                    "append uncertainty lacks attempt data",
                ));
            }
            BootstrapState::Anchored
                if self.append_attempts.is_empty()
                    || !self
                        .append_attempts
                        .iter()
                        .any(|attempt| attempt.candidate_content_hash.is_some()) =>
            {
                return Err(BootstrapJournalError::Corrupt(
                    "anchored transaction lacks a candidate append record",
                ));
            }
            BootstrapState::OwnerApproved
                if self.snapshot.operation == BootstrapOperation::PeerAdmission
                    || self.snapshot.role != BootstrapRole::Issuer =>
            {
                return Err(BootstrapJournalError::Corrupt(
                    "owner-approved state is only valid for issuer operations",
                ));
            }
            _ => {}
        }
        Ok(())
    }
}

/// Durable §2 exchange state before an owner-review transaction is promoted
/// into the approval/anchor state machine below. Messages are public protocol
/// bytes only; this record never grants trust.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingBootstrapExchange {
    pub transaction_id: String,
    pub brain_id: String,
    pub role: BootstrapExchangeRole,
    pub local_identity: ApprovalIdentity,
    pub local_transport_device_id: String,
    pub peer_identity: Option<ApprovalIdentity>,
    pub peer_transport_device_id: Option<String>,
    pub transcript_messages: Vec<String>,
    pub state: BootstrapState,
    pub stable_event_id: String,
}

impl PendingBootstrapExchange {
    pub fn initiator_offer(
        raw_offer: &[u8],
        stable_event_id: String,
    ) -> Result<Self, BootstrapJournalError> {
        let offer = cryptex_events::verify_bootstrap_offer(raw_offer)
            .map_err(|_| BootstrapJournalError::Corrupt("signed offer invalid"))?;
        let message = std::str::from_utf8(raw_offer)
            .map_err(|_| BootstrapJournalError::Corrupt("offer is not UTF-8"))?;
        let exchange = Self {
            transaction_id: offer.transaction_id().to_owned(),
            brain_id: offer.brain_id().to_owned(),
            role: BootstrapExchangeRole::Initiator,
            local_identity: approval_identity_from_bootstrap(offer.initiator()),
            local_transport_device_id: offer.initiator_transport_device_id().to_owned(),
            peer_identity: None,
            peer_transport_device_id: None,
            transcript_messages: vec![message.to_owned()],
            state: BootstrapState::OfferCreated,
            stable_event_id,
        };
        exchange.validate()?;
        Ok(exchange)
    }

    pub fn responder_offer(
        raw_offer: &[u8],
        local_identity: ApprovalIdentity,
        local_transport_device_id: String,
        stable_event_id: String,
    ) -> Result<Self, BootstrapJournalError> {
        let offer = cryptex_events::verify_bootstrap_offer(raw_offer)
            .map_err(|_| BootstrapJournalError::Corrupt("signed offer invalid"))?;
        let message = std::str::from_utf8(raw_offer)
            .map_err(|_| BootstrapJournalError::Corrupt("offer is not UTF-8"))?;
        let exchange = Self {
            transaction_id: offer.transaction_id().to_owned(),
            brain_id: offer.brain_id().to_owned(),
            role: BootstrapExchangeRole::Responder,
            local_identity,
            local_transport_device_id,
            peer_identity: Some(approval_identity_from_bootstrap(offer.initiator())),
            peer_transport_device_id: Some(offer.initiator_transport_device_id().to_owned()),
            transcript_messages: vec![message.to_owned()],
            state: BootstrapState::OfferReceived,
            stable_event_id,
        };
        exchange.validate()?;
        Ok(exchange)
    }

    pub fn record_proof(&mut self, raw_proof: &[u8]) -> Result<(), BootstrapJournalError> {
        self.validate()?;
        let expected_state = match self.role {
            BootstrapExchangeRole::Initiator => BootstrapState::OfferCreated,
            BootstrapExchangeRole::Responder => BootstrapState::OfferReceived,
        };
        if self.state != expected_state || self.transcript_messages.len() != 1 {
            if self.transcript_messages.get(1).is_some_and(|message| {
                message.as_bytes() == raw_proof
                    && matches!(
                        (self.role, self.state),
                        (
                            BootstrapExchangeRole::Initiator,
                            BootstrapState::ProofReceived
                        ) | (
                            BootstrapExchangeRole::Responder,
                            BootstrapState::ProofCreated
                        )
                    )
            }) {
                return Ok(());
            }
            return Err(BootstrapJournalError::Conflict(
                "bootstrap exchange already has a different proof",
            ));
        }
        let offer = self.transcript_messages[0].as_bytes();
        let verified = cryptex_events::verify_bootstrap_approval_request(
            &[offer, raw_proof],
            cryptex_events::BootstrapApprovalRole::Initiator,
        )
        .map_err(|_| BootstrapJournalError::Corrupt("proof does not match signed offer"))?;
        if verified.transaction_id() != self.transaction_id || verified.brain_id() != self.brain_id
        {
            return Err(BootstrapJournalError::Conflict(
                "proof transaction or Brain differs from durable offer",
            ));
        }
        match self.role {
            BootstrapExchangeRole::Initiator => {
                if !identity_matches(&self.local_identity, verified.local_identity())
                    || self.local_transport_device_id != verified.local_transport_device_id()
                {
                    return Err(BootstrapJournalError::Conflict(
                        "proof initiator differs from durable local offer",
                    ));
                }
                self.peer_identity =
                    Some(approval_identity_from_bootstrap(verified.peer_identity()));
                self.peer_transport_device_id =
                    Some(verified.peer_transport_device_id().to_owned());
                self.state = BootstrapState::ProofReceived;
            }
            BootstrapExchangeRole::Responder => {
                if !identity_matches(&self.local_identity, verified.peer_identity())
                    || self.local_transport_device_id != verified.peer_transport_device_id()
                    || self
                        .peer_identity
                        .as_ref()
                        .is_none_or(|peer| !identity_matches(peer, verified.local_identity()))
                    || self.peer_transport_device_id.as_deref()
                        != Some(verified.local_transport_device_id())
                {
                    return Err(BootstrapJournalError::Conflict(
                        "proof responder differs from durable local exchange",
                    ));
                }
                self.state = BootstrapState::ProofCreated;
            }
        }
        self.transcript_messages.push(
            std::str::from_utf8(raw_proof)
                .map_err(|_| BootstrapJournalError::Corrupt("proof is not UTF-8"))?
                .to_owned(),
        );
        self.validate()
    }

    pub fn validate(&self) -> Result<(), BootstrapJournalError> {
        validate_lower_hex(&self.transaction_id, 64, "transaction ID")?;
        validate_canonical_uuid(&self.brain_id, "Brain ID")?;
        validate_identity(&self.local_identity)?;
        validate_transport_id(&self.local_transport_device_id)?;
        validate_canonical_uuid(&self.stable_event_id, "event ID")?;
        if self.transcript_messages.is_empty() || self.transcript_messages.len() > 2 {
            return Err(BootstrapJournalError::Corrupt(
                "exchange transcript must contain an offer and optional proof",
            ));
        }
        for message in &self.transcript_messages {
            if message.len() > 64 * 1024 {
                return Err(BootstrapJournalError::Limit("bootstrap message bytes"));
            }
        }
        let offer = cryptex_events::verify_bootstrap_offer(self.transcript_messages[0].as_bytes())
            .map_err(|_| BootstrapJournalError::Corrupt("exchange offer invalid"))?;
        if offer.transaction_id() != self.transaction_id || offer.brain_id() != self.brain_id {
            return Err(BootstrapJournalError::Conflict(
                "exchange offer differs from transaction binding",
            ));
        }
        match self.role {
            BootstrapExchangeRole::Initiator => {
                if self.transcript_messages.len() == 1 {
                    if self.state != BootstrapState::OfferCreated
                        || !identity_matches(&self.local_identity, offer.initiator())
                        || self.local_transport_device_id != offer.initiator_transport_device_id()
                        || self.peer_identity.is_some()
                        || self.peer_transport_device_id.is_some()
                    {
                        return Err(BootstrapJournalError::Corrupt(
                            "initiator offer state differs from its signed message",
                        ));
                    }
                } else {
                    if self.state != BootstrapState::ProofReceived {
                        return Err(BootstrapJournalError::Corrupt(
                            "initiator exchange state does not match proof",
                        ));
                    }
                    self.clone().validate_proof_binding()?;
                }
            }
            BootstrapExchangeRole::Responder => {
                let peer = self
                    .peer_identity
                    .as_ref()
                    .ok_or(BootstrapJournalError::Corrupt(
                        "responder exchange peer identity missing",
                    ))?;
                let peer_transport = self.peer_transport_device_id.as_deref().ok_or(
                    BootstrapJournalError::Corrupt("responder exchange peer transport missing"),
                )?;
                if !identity_matches(peer, offer.initiator())
                    || peer_transport != offer.initiator_transport_device_id()
                {
                    return Err(BootstrapJournalError::Conflict(
                        "responder peer differs from signed offer",
                    ));
                }
                match self.transcript_messages.len() {
                    1 if self.state == BootstrapState::OfferReceived => {}
                    2 if self.state == BootstrapState::ProofCreated => {
                        self.clone().validate_proof_binding()?;
                    }
                    _ => {
                        return Err(BootstrapJournalError::Corrupt(
                            "responder exchange state does not match its messages",
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_proof_binding(&self) -> Result<(), BootstrapJournalError> {
        let request = cryptex_events::verify_bootstrap_approval_request(
            &[
                self.transcript_messages[0].as_bytes(),
                self.transcript_messages[1].as_bytes(),
            ],
            cryptex_events::BootstrapApprovalRole::Initiator,
        )
        .map_err(|_| BootstrapJournalError::Corrupt("exchange proof invalid"))?;
        let (local, peer, local_transport, peer_transport) = match self.role {
            BootstrapExchangeRole::Initiator => (
                request.local_identity(),
                request.peer_identity(),
                request.local_transport_device_id(),
                request.peer_transport_device_id(),
            ),
            BootstrapExchangeRole::Responder => (
                request.peer_identity(),
                request.local_identity(),
                request.peer_transport_device_id(),
                request.local_transport_device_id(),
            ),
        };
        if !identity_matches(&self.local_identity, local)
            || self.local_transport_device_id != local_transport
            || self
                .peer_identity
                .as_ref()
                .is_none_or(|identity| !identity_matches(identity, peer))
            || self.peer_transport_device_id.as_deref() != Some(peer_transport)
        {
            return Err(BootstrapJournalError::Conflict(
                "exchange proof identity or transport binding differs",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingBootstrapJournal {
    pub schema_version: u32,
    pub transactions: Vec<PendingBootstrapTransaction>,
    pub exchanges: Vec<PendingBootstrapExchange>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PendingBootstrapJournalV1 {
    schema_version: u32,
    transactions: Vec<PendingBootstrapTransaction>,
}

impl Default for PendingBootstrapJournal {
    fn default() -> Self {
        Self {
            schema_version: JOURNAL_SCHEMA_VERSION,
            transactions: Vec::new(),
            exchanges: Vec::new(),
        }
    }
}

impl PendingBootstrapJournal {
    pub fn validate(&self) -> Result<(), BootstrapJournalError> {
        if self.schema_version != JOURNAL_SCHEMA_VERSION {
            return Err(BootstrapJournalError::Corrupt("unsupported journal schema"));
        }
        if self.transactions.len() > MAX_PENDING_TRANSACTIONS {
            return Err(BootstrapJournalError::Limit("pending transaction count"));
        }
        if self.exchanges.len() > MAX_PENDING_TRANSACTIONS {
            return Err(BootstrapJournalError::Limit("pending exchange count"));
        }
        if self.transactions.len() + self.exchanges.len() > MAX_PENDING_TRANSACTIONS {
            return Err(BootstrapJournalError::Limit("pending transaction count"));
        }
        let mut seen = std::collections::BTreeSet::new();
        let mut seen_event_ids = std::collections::BTreeSet::new();
        for transaction in &self.transactions {
            transaction.validate()?;
            if !seen.insert(transaction.snapshot.transaction_id.as_str()) {
                return Err(BootstrapJournalError::Conflict("duplicate transaction ID"));
            }
            if !seen_event_ids.insert(transaction.stable_event_id.as_str()) {
                return Err(BootstrapJournalError::Conflict("duplicate stable event ID"));
            }
        }
        for exchange in &self.exchanges {
            exchange.validate()?;
            if !seen.insert(exchange.transaction_id.as_str()) {
                return Err(BootstrapJournalError::Conflict("duplicate transaction ID"));
            }
            if !seen_event_ids.insert(exchange.stable_event_id.as_str()) {
                return Err(BootstrapJournalError::Conflict("duplicate stable event ID"));
            }
        }
        let bytes = serde_json::to_vec(self)
            .map_err(|_| BootstrapJournalError::Corrupt("journal serialization failed"))?;
        if bytes.len() > MAX_JOURNAL_BYTES {
            return Err(BootstrapJournalError::Limit("journal payload bytes"));
        }
        Ok(())
    }

    pub fn insert(
        &mut self,
        transaction: PendingBootstrapTransaction,
    ) -> Result<(), BootstrapJournalError> {
        transaction.validate()?;
        if self.transactions.iter().any(|existing| {
            existing.snapshot.transaction_id == transaction.snapshot.transaction_id
                || existing.stable_event_id == transaction.stable_event_id
        }) || self.exchanges.iter().any(|existing| {
            existing.transaction_id == transaction.snapshot.transaction_id
                || existing.stable_event_id == transaction.stable_event_id
        }) {
            return Err(BootstrapJournalError::Conflict(
                "transaction or event ID already exists",
            ));
        }
        if self.transactions.len() + self.exchanges.len() >= MAX_PENDING_TRANSACTIONS {
            return Err(BootstrapJournalError::Limit("pending transaction count"));
        }
        self.transactions.push(transaction);
        self.transactions.sort_by(|left, right| {
            left.snapshot
                .transaction_id
                .as_bytes()
                .cmp(right.snapshot.transaction_id.as_bytes())
        });
        self.validate()
    }

    pub fn insert_exchange(
        &mut self,
        exchange: PendingBootstrapExchange,
    ) -> Result<(), BootstrapJournalError> {
        exchange.validate()?;
        if self.transactions.iter().any(|existing| {
            existing.snapshot.transaction_id == exchange.transaction_id
                || existing.stable_event_id == exchange.stable_event_id
        }) || self.exchanges.iter().any(|existing| {
            existing.transaction_id == exchange.transaction_id
                || existing.stable_event_id == exchange.stable_event_id
        }) {
            return Err(BootstrapJournalError::Conflict(
                "transaction or event ID already exists",
            ));
        }
        if self.transactions.len() + self.exchanges.len() >= MAX_PENDING_TRANSACTIONS {
            return Err(BootstrapJournalError::Limit("pending transaction count"));
        }
        self.exchanges.push(exchange);
        self.exchanges.sort_by(|left, right| {
            left.transaction_id
                .as_bytes()
                .cmp(right.transaction_id.as_bytes())
        });
        self.validate()
    }
}

#[derive(Debug, Clone)]
pub struct BootstrapJournalStore {
    root: PathBuf,
}

impl BootstrapJournalStore {
    pub fn new(runtime_root: &Path) -> Result<Self, BootstrapJournalError> {
        assert_outside_repository(runtime_root)
            .map_err(|error| BootstrapJournalError::Io(error.to_string()))?;
        let root = runtime_root.join(JOURNAL_DIR);
        Ok(Self { root })
    }

    pub fn load(&self) -> Result<PendingBootstrapJournal, BootstrapJournalError> {
        self.with_lock(|| self.load_unlocked())
    }

    /// Persist a validated candidate under a cross-process lock, after every
    /// caller-visible state transition and before any response/signature release.
    pub fn update<T>(
        &self,
        update: impl FnOnce(&mut PendingBootstrapJournal) -> Result<T, BootstrapJournalError>,
    ) -> Result<T, BootstrapJournalError> {
        self.with_lock(|| {
            let mut journal = self.load_unlocked()?;
            let result = update(&mut journal)?;
            journal.validate()?;
            let bytes = serde_json::to_vec(&journal)
                .map_err(|_| BootstrapJournalError::Corrupt("journal serialization failed"))?;
            if bytes.len() > MAX_JOURNAL_BYTES {
                return Err(BootstrapJournalError::Limit("journal payload bytes"));
            }
            self.write_unlocked(&bytes)?;
            Ok(result)
        })
    }

    /// Persist a newly generated offer before IPC releases its bytes to Desktop.
    pub fn record_initiator_offer(
        &self,
        raw_offer: &[u8],
        stable_event_id: &str,
    ) -> Result<(), BootstrapJournalError> {
        let exchange =
            PendingBootstrapExchange::initiator_offer(raw_offer, stable_event_id.to_owned())?;
        self.update(|journal| {
            if let Some(existing) = journal
                .exchanges
                .iter()
                .find(|item| item.transaction_id == exchange.transaction_id)
            {
                return if existing == &exchange {
                    Ok(())
                } else {
                    Err(BootstrapJournalError::Conflict(
                        "transaction ID already pins different offer bytes",
                    ))
                };
            }
            journal.insert_exchange(exchange)
        })
    }

    /// Persist an incoming offer before generating or releasing the responder
    /// proof. A transaction ID can never be rebound to a different offer.
    pub fn record_responder_offer(
        &self,
        raw_offer: &[u8],
        local_identity: ApprovalIdentity,
        local_transport_device_id: String,
        stable_event_id: &str,
    ) -> Result<(), BootstrapJournalError> {
        let exchange = PendingBootstrapExchange::responder_offer(
            raw_offer,
            local_identity,
            local_transport_device_id,
            stable_event_id.to_owned(),
        )?;
        self.update(|journal| {
            if let Some(existing) = journal
                .exchanges
                .iter()
                .find(|item| item.transaction_id == exchange.transaction_id)
            {
                return if existing.role == BootstrapExchangeRole::Responder
                    && existing.brain_id == exchange.brain_id
                    && existing.transcript_messages == exchange.transcript_messages
                    && existing.local_identity == exchange.local_identity
                    && existing.local_transport_device_id == exchange.local_transport_device_id
                {
                    Ok(())
                } else {
                    Err(BootstrapJournalError::Conflict(
                        "transaction ID already pins another bootstrap exchange",
                    ))
                };
            }
            journal.insert_exchange(exchange)
        })
    }

    /// Persist a generated or received proof before it is exposed to the next
    /// participant or used to open an owner-approval session.
    pub fn record_exchange_proof(
        &self,
        transaction_id: &str,
        raw_proof: &[u8],
    ) -> Result<(), BootstrapJournalError> {
        self.update(|journal| {
            let exchange = journal
                .exchanges
                .iter_mut()
                .find(|item| item.transaction_id == transaction_id)
                .ok_or(BootstrapJournalError::Conflict(
                    "bootstrap offer is not durably journaled",
                ))?;
            exchange.record_proof(raw_proof)
        })
    }

    pub fn cancel_pending_approval(
        &self,
        transaction_id: &str,
    ) -> Result<(), BootstrapJournalError> {
        self.update(|journal| {
            let transaction = journal
                .transactions
                .iter_mut()
                .find(|transaction| transaction.snapshot.transaction_id == transaction_id)
                .ok_or(BootstrapJournalError::Conflict(
                    "approval transaction is not journaled",
                ))?;
            if transaction.local_approval_signature_released {
                return Err(BootstrapJournalError::Conflict(
                    "released approval signature cannot be cancelled as unreleased",
                ));
            }
            transaction.transition(BootstrapState::Cancelled)
        })
    }

    /// Persist an authenticated prefix before the caller issues an owner
    /// confirmation capability or releases any locally signed message.
    pub fn record_verified_approval_prefix(
        &self,
        request: &cryptex_events::VerifiedBootstrapApprovalRequest,
        raw_messages: &[Vec<u8>],
    ) -> Result<(), BootstrapJournalError> {
        self.update(|journal| {
            let transaction = journal
                .transactions
                .iter_mut()
                .find(|transaction| transaction.snapshot.transaction_id == request.transaction_id())
                .ok_or(BootstrapJournalError::Conflict(
                    "approval transaction is not journaled",
                ))?;
            transaction.record_verified_approval_prefix(request, raw_messages)
        })
    }

    /// Atomically create a pending transaction and persist its authenticated
    /// approval prefix, or attach an exact replay to the existing transaction.
    pub fn begin_verified_approval_prefix(
        &self,
        request: &cryptex_events::VerifiedBootstrapApprovalRequest,
        raw_messages: &[Vec<u8>],
        stable_event_id: &str,
    ) -> Result<(), BootstrapJournalError> {
        self.update(|journal| {
            if let Some(existing) = journal
                .transactions
                .iter_mut()
                .find(|transaction| transaction.snapshot.transaction_id == request.transaction_id())
            {
                return existing.record_verified_approval_prefix(request, raw_messages);
            }
            let snapshot = ApprovalSnapshot::from_verified_bootstrap_request(request)?;
            let exchange_index = journal
                .exchanges
                .iter()
                .position(|exchange| exchange.transaction_id == request.transaction_id());
            let stable_event_id = if let Some(index) = exchange_index {
                let exchange = &journal.exchanges[index];
                let expected_prefix_len = match request.role() {
                    cryptex_events::BootstrapApprovalRole::Initiator => 2,
                    cryptex_events::BootstrapApprovalRole::Responder => 2,
                };
                let role_matches = matches!(
                    (exchange.role, request.role()),
                    (
                        BootstrapExchangeRole::Initiator,
                        cryptex_events::BootstrapApprovalRole::Initiator
                    ) | (
                        BootstrapExchangeRole::Responder,
                        cryptex_events::BootstrapApprovalRole::Responder
                    )
                );
                let state_matches = matches!(
                    (exchange.role, exchange.state),
                    (
                        BootstrapExchangeRole::Initiator,
                        BootstrapState::ProofReceived
                    ) | (
                        BootstrapExchangeRole::Responder,
                        BootstrapState::ProofCreated
                    )
                );
                if !role_matches
                    || !state_matches
                    || exchange.transcript_messages.len() != expected_prefix_len
                    || raw_messages.len() < expected_prefix_len
                    || exchange
                        .transcript_messages
                        .iter()
                        .zip(raw_messages)
                        .any(|(saved, incoming)| saved.as_bytes() != incoming.as_slice())
                {
                    return Err(BootstrapJournalError::Conflict(
                        "approval prefix differs from durable bootstrap exchange",
                    ));
                }
                exchange.stable_event_id.clone()
            } else {
                stable_event_id.to_owned()
            };
            let mut transaction = PendingBootstrapTransaction::new(snapshot, stable_event_id)?;
            transaction.record_verified_approval_prefix(request, raw_messages)?;
            if let Some(index) = exchange_index {
                journal.exchanges.remove(index);
            }
            journal.insert(transaction)
        })
    }

    /// Persist the exact frozen non-admission operation before issuing its
    /// one-use owner-confirmation capability. Exact retries reuse the original
    /// event ID and snapshot; rebinding a transaction is a conflict.
    pub fn begin_owner_operation(
        &self,
        snapshot: ApprovalSnapshot,
        stable_event_id: &str,
    ) -> Result<(), BootstrapJournalError> {
        if snapshot.operation == BootstrapOperation::PeerAdmission {
            return Err(BootstrapJournalError::Conflict(
                "owner-operation journal entry cannot be peer admission",
            ));
        }
        let transaction = PendingBootstrapTransaction::new(snapshot, stable_event_id.to_owned())?;
        self.update(|journal| {
            if let Some(existing) = journal.transactions.iter().find(|existing| {
                existing.snapshot.transaction_id == transaction.snapshot.transaction_id
            }) {
                return if existing == &transaction {
                    Ok(())
                } else {
                    Err(BootstrapJournalError::Conflict(
                        "security transaction ID already pins another owner operation",
                    ))
                };
            }
            journal.insert(transaction)
        })
    }

    /// Persist confirmation only after the authenticated IPC handler consumes
    /// the same connection-bound approval capability.
    pub fn confirm_owner_operation(
        &self,
        transaction_id: &str,
        approval_request_digest: &str,
    ) -> Result<(), BootstrapJournalError> {
        self.update(|journal| {
            let transaction = journal
                .transactions
                .iter_mut()
                .find(|transaction| transaction.snapshot.transaction_id == transaction_id)
                .ok_or(BootstrapJournalError::Conflict(
                    "owner security operation is not journaled",
                ))?;
            transaction.record_owner_confirmation(approval_request_digest)
        })
    }

    pub fn record_local_approval_signature(
        &self,
        request: &cryptex_events::VerifiedBootstrapApprovalRequest,
        signed_message: &[u8],
    ) -> Result<(), BootstrapJournalError> {
        self.update(|journal| {
            let transaction = journal
                .transactions
                .iter_mut()
                .find(|transaction| transaction.snapshot.transaction_id == request.transaction_id())
                .ok_or(BootstrapJournalError::Conflict(
                    "approval transaction is not journaled",
                ))?;
            transaction.record_local_approval_signature(request, signed_message)
        })
    }

    pub fn record_verified_complete_transcript(
        &self,
        transaction_id: &str,
        raw_messages: &[Vec<u8>],
    ) -> Result<(), BootstrapJournalError> {
        self.update(|journal| {
            let transaction = journal
                .transactions
                .iter_mut()
                .find(|transaction| transaction.snapshot.transaction_id == transaction_id)
                .ok_or(BootstrapJournalError::Conflict(
                    "bootstrap transaction is not journaled",
                ))?;
            transaction.record_verified_complete_transcript(raw_messages)
        })
    }

    #[allow(dead_code)] // Wired only after canonical reconciliation is implemented.
    pub(crate) fn record_append_attempt(
        &self,
        transaction_id: &str,
        attempt: AppendAttempt,
    ) -> Result<(), BootstrapJournalError> {
        self.update(|journal| {
            let transaction = journal
                .transactions
                .iter_mut()
                .find(|transaction| transaction.snapshot.transaction_id == transaction_id)
                .ok_or(BootstrapJournalError::Conflict(
                    "bootstrap transaction is not journaled",
                ))?;
            transaction.record_append_attempt(attempt)
        })
    }

    #[allow(dead_code)] // Must be called only with a lock-scoped canonical proof.
    pub(crate) fn record_verified_anchor(
        &self,
        transaction_id: &str,
        reserved_sequence: u64,
        content_hash: &str,
    ) -> Result<(), BootstrapJournalError> {
        self.update(|journal| {
            let transaction = journal
                .transactions
                .iter_mut()
                .find(|transaction| transaction.snapshot.transaction_id == transaction_id)
                .ok_or(BootstrapJournalError::Conflict(
                    "bootstrap transaction is not journaled",
                ))?;
            transaction.record_verified_anchor(reserved_sequence, content_hash)
        })
    }

    fn with_lock<T>(
        &self,
        operation: impl FnOnce() -> Result<T, BootstrapJournalError>,
    ) -> Result<T, BootstrapJournalError> {
        fs::create_dir_all(&self.root).map_err(io_error)?;
        let root_metadata = fs::symlink_metadata(&self.root).map_err(io_error)?;
        if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
            return Err(BootstrapJournalError::Conflict(
                "bootstrap journal directory is linked or not a directory",
            ));
        }
        let lock_path = self.root.join(JOURNAL_LOCK);
        let lock = open_or_create_lock(&lock_path).map_err(io_error)?;
        lock.lock_exclusive().map_err(io_error)?;
        let result = operation();
        FileExt::unlock(&lock).map_err(io_error)?;
        result
    }

    fn load_unlocked(&self) -> Result<PendingBootstrapJournal, BootstrapJournalError> {
        for entry in fs::read_dir(&self.root).map_err(io_error)? {
            let entry = entry.map_err(io_error)?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name != JOURNAL_FILE && name != JOURNAL_LOCK {
                // A crash may leave an uncommitted temp. Preserve it and hold
                // for review instead of silently treating it as disposable.
                return Err(BootstrapJournalError::Conflict(
                    "unexpected bootstrap journal entry; evidence preserved",
                ));
            }
            let metadata = fs::symlink_metadata(entry.path()).map_err(io_error)?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(BootstrapJournalError::Conflict(
                    "bootstrap journal entry is not an ordinary file",
                ));
            }
        }
        let path = self.root.join(JOURNAL_FILE);
        let mut file = match open_no_follow_regular(&path, false) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(PendingBootstrapJournal::default());
            }
            Err(error) => return Err(io_error(error)),
        };
        let metadata = file.metadata().map_err(io_error)?;
        if metadata.len() > MAX_JOURNAL_BYTES as u64 {
            return Err(BootstrapJournalError::Limit("journal payload bytes"));
        }
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        file.read_to_end(&mut bytes).map_err(io_error)?;
        if bytes.len() > MAX_JOURNAL_BYTES {
            return Err(BootstrapJournalError::Limit("journal payload bytes"));
        }
        let raw = std::str::from_utf8(&bytes)
            .map_err(|_| BootstrapJournalError::Corrupt("journal is not UTF-8"))?;
        reject_duplicate_json_keys(raw)
            .map_err(|_| BootstrapJournalError::Corrupt("duplicate JSON keys"))?;
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|_| BootstrapJournalError::Corrupt("journal JSON/schema invalid"))?;
        let version = value.get("schema_version").and_then(Value::as_u64).ok_or(
            BootstrapJournalError::Corrupt("journal schema version missing"),
        )?;
        let journal = if version == u64::from(LEGACY_JOURNAL_SCHEMA_VERSION) {
            let legacy: PendingBootstrapJournalV1 = serde_json::from_value(value)
                .map_err(|_| BootstrapJournalError::Corrupt("legacy journal schema invalid"))?;
            if legacy.schema_version != LEGACY_JOURNAL_SCHEMA_VERSION {
                return Err(BootstrapJournalError::Corrupt(
                    "legacy journal schema version mismatch",
                ));
            }
            PendingBootstrapJournal {
                schema_version: JOURNAL_SCHEMA_VERSION,
                transactions: legacy.transactions,
                exchanges: Vec::new(),
            }
        } else if version == u64::from(JOURNAL_SCHEMA_VERSION) {
            serde_json::from_value(value)
                .map_err(|_| BootstrapJournalError::Corrupt("journal JSON/schema invalid"))?
        } else {
            return Err(BootstrapJournalError::Corrupt(
                "unsupported journal schema version",
            ));
        };
        journal.validate()?;
        Ok(journal)
    }

    fn write_unlocked(&self, bytes: &[u8]) -> Result<(), BootstrapJournalError> {
        let path = self.root.join(JOURNAL_FILE);
        let tmp = self.unique_temp_path()?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(io_error)?;
        if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
            drop(file);
            cleanup_temp(&tmp, &error).map_err(|cleanup| {
                BootstrapJournalError::Durability(format!(
                    "journal write failed: {error}; temporary cleanup failed: {cleanup}"
                ))
            })?;
            return Err(io_error(error));
        }
        drop(file);
        replace_atomic(&tmp, &path).map_err(|error| {
            let cleanup = cleanup_temp(&tmp, &error);
            match cleanup {
                Ok(()) => io_error(error),
                Err(cleanup_error) => BootstrapJournalError::Durability(format!(
                    "replace failed: {error}; temporary cleanup failed: {cleanup_error}"
                )),
            }
        })?;
        let replaced = open_no_follow_regular(&path, true).map_err(io_error)?;
        replaced.sync_all().map_err(|error| {
            BootstrapJournalError::Durability(format!("reopen/sync replaced journal: {error}"))
        })?;
        #[cfg(unix)]
        File::open(&self.root)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| {
                BootstrapJournalError::Durability(format!("sync journal directory: {error}"))
            })?;
        Ok(())
    }

    fn unique_temp_path(&self) -> Result<PathBuf, BootstrapJournalError> {
        let mut random = [0u8; 16];
        getrandom::getrandom(&mut random)
            .map_err(|_| BootstrapJournalError::Durability("secure random unavailable".into()))?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        Ok(self.root.join(format!(
            ".pending.tmp-{}-{}-{}",
            std::process::id(),
            timestamp,
            hex_lower(&random)
        )))
    }
}

fn replace_atomic(tmp: &Path, destination: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        fs::rename(tmp, destination)
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
        };
        let from = tmp
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let to = destination
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        // SAFETY: both pointers reference owned, null-terminated UTF-16 paths.
        if unsafe {
            MoveFileExW(
                from.as_ptr(),
                to.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (tmp, destination);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "atomic bootstrap journal replacement unsupported",
        ))
    }
}

fn open_or_create_lock(path: &Path) -> io::Result<File> {
    match open_no_follow_regular(path, true) {
        Ok(file) => Ok(file),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            match OpenOptions::new().write(true).create_new(true).open(path) {
                Ok(file) => drop(file),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
            open_no_follow_regular(path, true)
        }
        Err(error) => Err(error),
    }
}

fn open_no_follow_regular(path: &Path, writable: bool) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(writable);
    #[cfg(windows)]
    {
        use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
        use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
        let file = options.open(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_file()
            || metadata.file_attributes()
                & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
                != 0
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "journal file is not an ordinary file",
            ));
        }
        Ok(file)
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        let file = options.open(path)?;
        if !file.metadata()?.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "journal file is not an ordinary file",
            ));
        }
        Ok(file)
    }
    #[cfg(not(any(windows, unix)))]
    {
        let file = options.open(path)?;
        if !file.metadata()?.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "journal file is not an ordinary file",
            ));
        }
        Ok(file)
    }
}

fn cleanup_temp(path: &Path, primary: &io::Error) -> Result<(), io::Error> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(cleanup) => Err(io::Error::new(
            cleanup.kind(),
            format!("{primary}; cleanup failed: {cleanup}"),
        )),
    }
}

fn io_error(error: io::Error) -> BootstrapJournalError {
    BootstrapJournalError::Io(error.to_string())
}

fn validate_identity(identity: &ApprovalIdentity) -> Result<(), BootstrapJournalError> {
    if identity.device_id.len() > 64
        || DeviceId::new(identity.device_id.clone())
            .map(|device| device.as_str() != identity.device_id)
            .unwrap_or(true)
    {
        return Err(BootstrapJournalError::Corrupt("identity DeviceId invalid"));
    }
    validate_lower_hex(&identity.signing_key_id, 32, "signing key ID")?;
    validate_lower_hex(
        &identity.public_key_fingerprint_sha256,
        64,
        "public key fingerprint",
    )
}

fn identity_matches(
    snapshot: &ApprovalIdentity,
    verified: &cryptex_events::BootstrapIdentity,
) -> bool {
    snapshot.device_id == verified.device_id.as_str()
        && snapshot.signing_key_id == verified.signing_key_id.as_str()
        && snapshot.public_key_fingerprint_sha256 == hex_lower(&verified.fingerprint_sha256)
}

fn approval_identity_from_bootstrap(
    identity: &cryptex_events::BootstrapIdentity,
) -> ApprovalIdentity {
    ApprovalIdentity {
        device_id: identity.device_id.as_str().to_owned(),
        signing_key_id: identity.signing_key_id.as_str().to_owned(),
        public_key_fingerprint_sha256: hex_lower(&identity.fingerprint_sha256),
    }
}

fn validate_transport(association: &TransportAssociation) -> Result<(), BootstrapJournalError> {
    validate_transport_id(&association.local_device_id)?;
    validate_transport_id(&association.peer_device_id)?;
    if association.local_device_id == association.peer_device_id {
        return Err(BootstrapJournalError::Corrupt(
            "transport identities are equal",
        ));
    }
    Ok(())
}

fn validate_transport_id(value: &str) -> Result<(), BootstrapJournalError> {
    if value.len() != 63
        || value.split('-').count() != 8
        || !value.split('-').all(|group| {
            group.len() == 7
                && group
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase() || (b'2'..=b'7').contains(&byte))
        })
    {
        return Err(BootstrapJournalError::Corrupt(
            "transport association invalid",
        ));
    }
    Ok(())
}

fn validate_restriction_fields(
    target_device_id: &str,
    cutoff_seq: &str,
    cutoff_content_hash: &str,
) -> Result<(), BootstrapJournalError> {
    if target_device_id.len() > 64
        || DeviceId::new(target_device_id.to_owned())
            .map(|device| device.as_str() != target_device_id)
            .unwrap_or(true)
    {
        return Err(BootstrapJournalError::Corrupt("restriction target invalid"));
    }
    validate_content_boundary(cutoff_seq, cutoff_content_hash, "restriction cutoff")
}

fn validate_content_boundary(
    sequence: &str,
    content_hash: &str,
    label: &'static str,
) -> Result<(), BootstrapJournalError> {
    if sequence.is_empty()
        || sequence.len() > 20
        || !sequence.bytes().all(|byte| byte.is_ascii_digit())
        || (sequence.len() > 1 && sequence.starts_with('0'))
        || sequence.parse::<u64>().is_err()
    {
        return Err(BootstrapJournalError::Corrupt(label));
    }
    if sequence == "0" && content_hash != "00".repeat(32) {
        return Err(BootstrapJournalError::Corrupt(label));
    }
    validate_lower_hex(content_hash, 64, label)
}

fn validate_canonical_uuid(value: &str, label: &'static str) -> Result<(), BootstrapJournalError> {
    let parsed = parse_uuid_str(value).map_err(|_| BootstrapJournalError::Corrupt(label))?;
    if parsed.to_ascii_lowercase() != value {
        return Err(BootstrapJournalError::Corrupt(label));
    }
    Ok(())
}

fn validate_lower_hex(
    value: &str,
    length: usize,
    label: &'static str,
) -> Result<(), BootstrapJournalError> {
    if value.len() != length
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(BootstrapJournalError::Corrupt(label));
    }
    Ok(())
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;

    const BRAIN: &str = "11111111-1111-4111-8111-111111111111";
    const LOCAL_TRANSPORT: &str = "A234567-A234567-A234567-A234567-A234567-A234567-A234567-A234567";
    const PEER_TRANSPORT: &str = "B234567-B234567-B234567-B234567-B234567-B234567-B234567-B234567";

    fn snapshot(role: BootstrapRole) -> ApprovalSnapshot {
        ApprovalSnapshot {
            brain_id: "11111111-1111-4111-8111-111111111111".into(),
            transaction_id: "aa".repeat(32),
            role,
            operation: BootstrapOperation::PeerAdmission,
            agreement_hash: Some("bb".repeat(32)),
            previous_message_hash: Some("cc".repeat(32)),
            local_identity: identity("local-device", 1),
            peer_identity: Some(identity("peer-device", 2)),
            transport_association: Some(TransportAssociation {
                local_device_id: "A234567-A234567-A234567-A234567-A234567-A234567-A234567-A234567"
                    .into(),
                peer_device_id: "B234567-B234567-B234567-B234567-B234567-B234567-B234567-B234567"
                    .into(),
            }),
            restriction_target: None,
            restriction_reason: None,
            cutoff_seq: None,
            cutoff_content_hash: None,
            last_authorized_seq: None,
            last_authorized_content_hash: None,
        }
    }

    fn self_retirement_snapshot() -> ApprovalSnapshot {
        ApprovalSnapshot {
            brain_id: BRAIN.into(),
            transaction_id: "dd".repeat(32),
            role: BootstrapRole::Issuer,
            operation: BootstrapOperation::SelfRetirement,
            agreement_hash: None,
            previous_message_hash: None,
            local_identity: identity("local-device", 1),
            peer_identity: None,
            transport_association: None,
            restriction_target: None,
            restriction_reason: None,
            cutoff_seq: None,
            cutoff_content_hash: None,
            last_authorized_seq: Some("1".into()),
            last_authorized_content_hash: Some("ab".repeat(32)),
        }
    }

    fn identity(device_id: &str, seed: u8) -> ApprovalIdentity {
        use cryptex_events::signing_key_id_from_verifying_key;
        use sha2::{Digest, Sha256};
        let key = SigningKey::from_bytes(&[seed; 32]).verifying_key();
        let key_id = signing_key_id_from_verifying_key(&key).unwrap();
        let fingerprint = Sha256::digest(key.as_bytes());
        ApprovalIdentity {
            device_id: device_id.into(),
            signing_key_id: key_id.as_str().to_owned(),
            public_key_fingerprint_sha256: hex_lower(&fingerprint),
        }
    }

    fn transaction(role: BootstrapRole) -> PendingBootstrapTransaction {
        PendingBootstrapTransaction::new(
            snapshot(role),
            "22222222-2222-4222-8222-222222222222".into(),
        )
        .unwrap()
    }

    fn signed_offer() -> (Vec<u8>, SigningKey, SigningKey) {
        let initiator = SigningKey::from_bytes(&[0x11; 32]);
        let responder = SigningKey::from_bytes(&[0x22; 32]);
        let offer = cryptex_events::create_bootstrap_offer(
            BRAIN,
            &"ab".repeat(32),
            &DeviceId::new("local-device").unwrap(),
            LOCAL_TRANSPORT,
            &[0x33; 32],
            &initiator,
        )
        .unwrap();
        (offer, initiator, responder)
    }

    #[test]
    fn durable_offer_proof_promotes_exact_prefix_into_owner_approval_transaction() {
        let root = crate::test_temp::exclusive_temp_dir("m8-bootstrap-exchange-promote");
        let store = BootstrapJournalStore::new(&root).unwrap();
        let (offer, initiator_key, responder_key) = signed_offer();
        let event_id = "22222222-2222-4222-8222-222222222222";
        store.record_initiator_offer(&offer, event_id).unwrap();
        let exchange = store.load().unwrap().exchanges.remove(0);
        assert_eq!(exchange.state, BootstrapState::OfferCreated);
        assert_eq!(exchange.transcript_messages.len(), 1);

        let proof = cryptex_events::create_bootstrap_proof(
            &offer,
            &DeviceId::new("peer-device").unwrap(),
            PEER_TRANSPORT,
            &[0x44; 32],
            &responder_key,
        )
        .unwrap();
        let verified = cryptex_events::verify_bootstrap_approval_request(
            &[offer.as_slice(), proof.as_slice()],
            cryptex_events::BootstrapApprovalRole::Initiator,
        )
        .unwrap();
        store
            .record_exchange_proof(verified.transaction_id(), &proof)
            .unwrap();
        store
            .record_exchange_proof(verified.transaction_id(), &proof)
            .unwrap();
        let replayed = store.load().unwrap().exchanges.remove(0);
        assert_eq!(replayed.state, BootstrapState::ProofReceived);
        assert_eq!(replayed.transcript_messages.len(), 2);

        let approval = cryptex_events::sign_bootstrap_approval(&verified, &initiator_key).unwrap();
        let mut prefix = vec![offer.clone(), proof.clone(), approval.clone()];
        // Initiator approval preparation consumes the signed two-message prefix.
        prefix.truncate(2);
        let refs = prefix.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let verified = cryptex_events::verify_bootstrap_approval_request(
            &refs,
            cryptex_events::BootstrapApprovalRole::Initiator,
        )
        .unwrap();
        store
            .begin_verified_approval_prefix(
                &verified,
                &prefix,
                "33333333-3333-4333-8333-333333333333",
            )
            .unwrap();
        let journal = store.load().unwrap();
        assert!(journal.exchanges.is_empty());
        assert_eq!(journal.transactions.len(), 1);
        assert_eq!(journal.transactions[0].stable_event_id, event_id);
        assert_eq!(journal.transactions[0].state, BootstrapState::ProofReceived);
        assert_eq!(journal.transactions[0].transcript_messages.len(), 2);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn legacy_bootstrap_journal_reads_and_upgrades_without_losing_transactions() {
        let root = crate::test_temp::exclusive_temp_dir("m8-bootstrap-journal-v1-upgrade");
        let store = BootstrapJournalStore::new(&root).unwrap();
        let directory = root.join(JOURNAL_DIR);
        fs::create_dir_all(&directory).unwrap();
        let mut legacy_transaction = transaction(BootstrapRole::Initiator);
        legacy_transaction.snapshot.transaction_id = "aa".repeat(32);
        legacy_transaction.approval_request_digest = legacy_transaction.snapshot.digest().unwrap();
        let legacy = PendingBootstrapJournalV1 {
            schema_version: LEGACY_JOURNAL_SCHEMA_VERSION,
            transactions: vec![legacy_transaction.clone()],
        };
        fs::write(
            directory.join(JOURNAL_FILE),
            serde_json::to_vec(&legacy).unwrap(),
        )
        .unwrap();

        let loaded = store.load().unwrap();
        assert_eq!(loaded.schema_version, JOURNAL_SCHEMA_VERSION);
        assert_eq!(loaded.transactions, vec![legacy_transaction]);
        assert!(loaded.exchanges.is_empty());
        store.update(|_| Ok(())).unwrap();
        let persisted: Value =
            serde_json::from_slice(&fs::read(directory.join(JOURNAL_FILE)).unwrap()).unwrap();
        assert_eq!(persisted["schema_version"], JOURNAL_SCHEMA_VERSION);
        assert_eq!(persisted["transactions"].as_array().unwrap().len(), 1);
        assert!(persisted["exchanges"].as_array().unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn digest_binds_closed_snapshot_and_journal_caps_pending_transactions() {
        let mut journal = PendingBootstrapJournal::default();
        for index in 0..MAX_PENDING_TRANSACTIONS {
            let mut item = transaction(BootstrapRole::Initiator);
            item.snapshot.transaction_id = format!("{:064x}", index + 1);
            item.approval_request_digest = item.snapshot.digest().unwrap();
            item.stable_event_id = format!("00000000-0000-4000-8000-{index:012x}");
            journal.insert(item).unwrap();
        }
        let mut extra = transaction(BootstrapRole::Initiator);
        extra.snapshot.transaction_id = "ff".repeat(32);
        extra.approval_request_digest = extra.snapshot.digest().unwrap();
        assert!(matches!(
            journal.insert(extra),
            Err(BootstrapJournalError::Limit(_))
        ));
        journal.validate().unwrap();
    }

    #[test]
    fn state_machine_rejects_skips_and_cancel_is_terminal() {
        let mut item = transaction(BootstrapRole::Initiator);
        assert!(item.transition(BootstrapState::TranscriptReady).is_err());
        item.transition(BootstrapState::ProofReceived).unwrap();
        assert!(
            item.transition(BootstrapState::LocalApprovalSigned)
                .is_err()
        );
        assert_eq!(item.state, BootstrapState::ProofReceived);
        item.transition(BootstrapState::Cancelled).unwrap();
        assert_eq!(
            item.transition(BootstrapState::OfferReceived),
            Err(BootstrapJournalError::Conflict(
                "illegal transaction state transition"
            ))
        );
    }

    #[test]
    fn altered_owner_snapshot_digest_fails_closed() {
        let mut item = transaction(BootstrapRole::Initiator);
        item.snapshot.peer_identity.as_mut().unwrap().device_id = "other-peer".into();
        assert!(matches!(
            item.validate(),
            Err(BootstrapJournalError::Corrupt(_))
        ));
    }

    #[test]
    fn owner_snapshot_serializes_flat_adr19_restriction_fields() {
        let candidate = snapshot(BootstrapRole::Initiator);
        let value = serde_json::to_value(&candidate).unwrap();
        let object = value.as_object().unwrap();
        assert!(!object.contains_key("restriction"));
        assert_eq!(object.get("restriction_target"), Some(&Value::Null));
        assert_eq!(object.get("cutoff_seq"), Some(&Value::Null));
        assert_eq!(object.get("cutoff_content_hash"), Some(&Value::Null));
        let canonical = serde_json_canonicalizer::to_vec(&value).unwrap();
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"cryptex-owner-approval-request-v1");
        hasher.update(&[0]);
        hasher.update(&canonical);
        assert_eq!(
            candidate.digest().unwrap(),
            hex_lower(hasher.finalize().as_bytes())
        );
    }

    #[test]
    fn append_attempts_reject_zero_or_reused_reserved_sequences() {
        let mut item = transaction(BootstrapRole::Initiator);
        item.append_attempts = vec![
            AppendAttempt {
                reserved_sequence: 7,
                predecessor_hash: "ab".repeat(32),
                candidate_content_hash: None,
            },
            AppendAttempt {
                reserved_sequence: 7,
                predecessor_hash: "cd".repeat(32),
                candidate_content_hash: None,
            },
        ];
        assert!(matches!(
            item.validate(),
            Err(BootstrapJournalError::Corrupt(_))
        ));
        item.append_attempts[1].reserved_sequence = 0;
        assert!(matches!(
            item.validate(),
            Err(BootstrapJournalError::Corrupt(_))
        ));
    }

    #[test]
    fn append_and_anchor_states_cannot_be_entered_without_verified_ready_state() {
        let mut item = transaction(BootstrapRole::Initiator);
        let attempt = AppendAttempt {
            reserved_sequence: 1,
            predecessor_hash: "00".repeat(32),
            candidate_content_hash: Some("11".repeat(32)),
        };
        assert!(matches!(
            item.record_append_attempt(attempt),
            Err(BootstrapJournalError::Conflict(_))
        ));
        assert!(matches!(
            item.transition(BootstrapState::AppendUnknown),
            Err(BootstrapJournalError::Conflict(_))
        ));
        assert!(matches!(
            item.transition(BootstrapState::Anchored),
            Err(BootstrapJournalError::Conflict(_))
        ));
        assert!(matches!(
            item.record_verified_anchor(1, &"11".repeat(32)),
            Err(BootstrapJournalError::Conflict(_))
        ));
        assert_eq!(item.state, BootstrapState::OfferCreated);
        assert!(item.append_attempts.is_empty());
    }

    #[test]
    fn owner_security_operation_requires_exact_persisted_confirmation_before_append() {
        let root = crate::test_temp::exclusive_temp_dir("m8-owner-security-operation");
        let store = BootstrapJournalStore::new(&root).unwrap();
        let snapshot = self_retirement_snapshot();
        let digest = snapshot.digest().unwrap();
        let transaction_id = snapshot.transaction_id.clone();
        let event_id = "33333333-3333-4333-8333-333333333333";
        store
            .begin_owner_operation(snapshot.clone(), event_id)
            .unwrap();
        store.begin_owner_operation(snapshot, event_id).unwrap();
        assert_eq!(
            store.load().unwrap().transactions[0].state,
            BootstrapState::OwnerPending
        );
        assert!(
            store
                .confirm_owner_operation(&transaction_id, &"ff".repeat(32))
                .is_err()
        );
        store
            .confirm_owner_operation(&transaction_id, &digest)
            .unwrap();
        let journal = store.load().unwrap();
        assert_eq!(journal.transactions[0].state, BootstrapState::OwnerApproved);
        assert!(journal.transactions[0].transcript_messages.is_empty());
        store
            .record_append_attempt(
                &transaction_id,
                AppendAttempt {
                    reserved_sequence: 1,
                    predecessor_hash: "00".repeat(32),
                    candidate_content_hash: Some("44".repeat(32)),
                },
            )
            .unwrap();
        let journal = store.load().unwrap();
        assert_eq!(journal.transactions[0].state, BootstrapState::AppendUnknown);
        assert_eq!(journal.transactions[0].append_attempts.len(), 1);
        assert!(journal.transactions[0].validate().is_ok());
        assert!(
            store
                .record_append_attempt(
                    &transaction_id,
                    AppendAttempt {
                        reserved_sequence: 2,
                        predecessor_hash: "44".repeat(32),
                        candidate_content_hash: Some("55".repeat(32)),
                    },
                )
                .is_err()
        );
        store
            .record_verified_anchor(&transaction_id, 1, &"44".repeat(32))
            .unwrap();
        assert_eq!(
            store.load().unwrap().transactions[0].state,
            BootstrapState::Anchored
        );

        let mut restriction = self_retirement_snapshot();
        restriction.transaction_id = "ee".repeat(32);
        restriction.operation = BootstrapOperation::DeviceRestriction;
        restriction.peer_identity = Some(identity("peer-device", 2));
        restriction.transport_association = Some(TransportAssociation {
            local_device_id: LOCAL_TRANSPORT.into(),
            peer_device_id: PEER_TRANSPORT.into(),
        });
        restriction.restriction_target = Some("peer-device".into());
        restriction.restriction_reason = Some(RestrictionReason::Compromised);
        restriction.cutoff_seq = Some("0".into());
        restriction.cutoff_content_hash = Some("00".repeat(32));
        restriction.last_authorized_seq = None;
        restriction.last_authorized_content_hash = None;
        let restriction_digest = restriction.digest().unwrap();
        let mut missing_reason = restriction.clone();
        missing_reason.restriction_reason = None;
        assert!(missing_reason.digest().is_err());
        store
            .begin_owner_operation(restriction.clone(), "44444444-4444-4444-8444-444444444444")
            .unwrap();
        let mut changed_reason = restriction;
        changed_reason.restriction_reason = Some(RestrictionReason::Retired);
        assert_ne!(changed_reason.digest().unwrap(), restriction_digest);
        let mut changed_boundary = self_retirement_snapshot();
        changed_boundary.last_authorized_content_hash = Some("cd".repeat(32));
        assert_ne!(
            changed_boundary.digest().unwrap(),
            self_retirement_snapshot().digest().unwrap()
        );
        let transactions = store.load().unwrap().transactions;
        assert_eq!(transactions.len(), 2);
        let restriction_transaction = transactions
            .iter()
            .find(|transaction| {
                transaction.snapshot.operation == BootstrapOperation::DeviceRestriction
            })
            .expect("restriction owner operation persisted");
        assert_eq!(restriction_transaction.state, BootstrapState::OwnerPending);
        assert_eq!(
            restriction_transaction.approval_request_digest,
            restriction_digest
        );
    }

    #[test]
    fn owner_operation_boundaries_require_exact_empty_prefix_hash() {
        let mut retirement = self_retirement_snapshot();
        retirement.last_authorized_seq = Some("0".into());
        retirement.last_authorized_content_hash = Some("00".repeat(32));
        assert!(retirement.digest().is_ok());
        retirement.last_authorized_content_hash = Some("11".repeat(32));
        assert!(retirement.digest().is_err());
    }

    #[test]
    fn journal_store_round_trips_and_rejects_duplicate_keys() {
        let root = crate::test_temp::exclusive_temp_dir("m8-bootstrap-journal");
        let store = BootstrapJournalStore::new(&root).unwrap();
        let mut journal = PendingBootstrapJournal::default();
        journal
            .insert(transaction(BootstrapRole::Initiator))
            .unwrap();
        store
            .update(|stored| {
                *stored = journal.clone();
                Ok(())
            })
            .unwrap();
        assert_eq!(store.load().unwrap(), journal);

        let path = root.join(JOURNAL_DIR).join(JOURNAL_FILE);
        fs::write(
            &path,
            br#"{"schema_version":1,"schema_version":1,"transactions":[]}"#,
        )
        .unwrap();
        assert_eq!(
            store.load(),
            Err(BootstrapJournalError::Corrupt("duplicate JSON keys"))
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn journal_store_serializes_updates_and_preserves_rejected_candidate() {
        let root = crate::test_temp::exclusive_temp_dir("m8-bootstrap-journal-update");
        let store = BootstrapJournalStore::new(&root).unwrap();
        let mut initial = PendingBootstrapJournal::default();
        initial
            .insert(transaction(BootstrapRole::Initiator))
            .unwrap();
        store
            .update(|stored| {
                *stored = initial.clone();
                Ok(())
            })
            .unwrap();

        let rejected = store.update(|stored| {
            stored.transactions.clear();
            Err::<(), _>(BootstrapJournalError::Conflict(
                "injected candidate rejection",
            ))
        });
        assert_eq!(
            rejected,
            Err(BootstrapJournalError::Conflict(
                "injected candidate rejection"
            ))
        );
        assert_eq!(store.load().unwrap(), initial);

        let mut workers = Vec::new();
        for index in 0..4 {
            let store = store.clone();
            workers.push(std::thread::spawn(move || {
                store
                    .update(|journal| {
                        let mut item = transaction(BootstrapRole::Initiator);
                        item.snapshot.transaction_id = format!("{:064x}", index + 100);
                        item.approval_request_digest = item.snapshot.digest()?;
                        item.stable_event_id = format!("00000000-0000-4000-8000-{index:012x}");
                        journal.insert(item)
                    })
                    .unwrap();
            }));
        }
        for worker in workers {
            worker.join().unwrap();
        }
        assert_eq!(store.load().unwrap().transactions.len(), 5);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unexpected_journal_temp_is_preserved_and_holds() {
        let root = crate::test_temp::exclusive_temp_dir("m8-bootstrap-journal-temp");
        let store = BootstrapJournalStore::new(&root).unwrap();
        let directory = root.join(JOURNAL_DIR);
        fs::create_dir_all(&directory).unwrap();
        let leftover = directory.join(".pending.tmp-interrupted-write");
        fs::write(&leftover, b"partial candidate").unwrap();
        assert!(matches!(
            store.load(),
            Err(BootstrapJournalError::Conflict(_))
        ));
        assert_eq!(fs::read(&leftover).unwrap(), b"partial candidate");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn transaction_and_event_ids_are_unique() {
        let mut journal = PendingBootstrapJournal::default();
        journal
            .insert(transaction(BootstrapRole::Initiator))
            .unwrap();
        let mut duplicate = transaction(BootstrapRole::Initiator);
        duplicate.snapshot.transaction_id = "dd".repeat(32);
        duplicate.approval_request_digest = duplicate.snapshot.digest().unwrap();
        assert!(matches!(
            journal.insert(duplicate),
            Err(BootstrapJournalError::Conflict(_))
        ));
    }
}
