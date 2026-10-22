//! In-memory M8 review policy. This is not a trust resolver or pairing protocol.
//!
//! A local review receipt proves only that one native review object consumed its
//! exact ticket/snapshot. It does not prove peer key possession, bilateral
//! approval, durable authorization, or permission to accept incoming events.
//! No production IPC, canonical writer or key resolver consumes these types.
//! ADR-0019's bootstrap/security-event activation gates remain mandatory.

use std::fmt;
use std::time::{Duration, Instant};

use cryptex_core::ids::DeviceId;
use cryptex_core::validate::parse_uuid_str;
use cryptex_events::{DeviceSeq, SigningKeyId, signing_key_id_from_verifying_key};
use ed25519_dalek::VerifyingKey;

const REVIEW_LIFETIME: Duration = Duration::from_secs(300);
const MAX_TRANSPORT_ASSOCIATION_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerReviewError {
    InvalidIdentity,
    InvalidKey,
    KeyIdMismatch,
    DifferentBrain,
    SameDevice,
    SharedKey,
    InvalidTransportAssociation,
    RandomUnavailable,
    Closed,
    Expired,
    TicketMismatch,
    SnapshotChanged,
    BindingMismatch,
    InvalidCutoff,
}

impl fmt::Display for PeerReviewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidIdentity => "peer_review_invalid_identity",
            Self::InvalidKey => "peer_review_invalid_key",
            Self::KeyIdMismatch => "peer_review_key_id_mismatch",
            Self::DifferentBrain => "peer_review_different_brain",
            Self::SameDevice => "peer_review_same_device",
            Self::SharedKey => "peer_review_shared_key",
            Self::InvalidTransportAssociation => "peer_review_invalid_transport_association",
            Self::RandomUnavailable => "peer_review_random_unavailable",
            Self::Closed => "peer_review_closed",
            Self::Expired => "peer_review_expired",
            Self::TicketMismatch => "peer_review_ticket_mismatch",
            Self::SnapshotChanged => "peer_review_snapshot_changed",
            Self::BindingMismatch => "peer_review_binding_mismatch",
            Self::InvalidCutoff => "peer_review_invalid_cutoff",
        })
    }
}

impl std::error::Error for PeerReviewError {}

/// Structurally valid public claims, NEVER authenticated identity by itself.
/// No deserializer or trust-resolver implementation is provided.
#[derive(Clone, PartialEq, Eq)]
pub struct PeerBinding {
    brain_id: String,
    device_id: DeviceId,
    public_key: [u8; 32],
    signing_key_id: SigningKeyId,
}

impl PeerBinding {
    pub fn from_public_claims(
        brain_id: &str,
        device_id: &str,
        public_key: [u8; 32],
        claimed_key_id: &str,
    ) -> Result<Self, PeerReviewError> {
        let brain_id = parse_uuid_str(brain_id).map_err(|_| PeerReviewError::InvalidIdentity)?;
        // DeviceId::new accepts owned input: bound before allocating that copy.
        if device_id.len() > 64 {
            return Err(PeerReviewError::InvalidIdentity);
        }
        let device_id = DeviceId::new(device_id).map_err(|_| PeerReviewError::InvalidIdentity)?;
        let verifying_key =
            VerifyingKey::from_bytes(&public_key).map_err(|_| PeerReviewError::InvalidKey)?;
        if verifying_key.is_weak() {
            return Err(PeerReviewError::InvalidKey);
        }
        let signing_key_id =
            SigningKeyId::parse(claimed_key_id).map_err(|_| PeerReviewError::KeyIdMismatch)?;
        let derived = signing_key_id_from_verifying_key(&verifying_key)
            .map_err(|_| PeerReviewError::InvalidKey)?;
        if signing_key_id != derived {
            return Err(PeerReviewError::KeyIdMismatch);
        }
        Ok(Self {
            brain_id,
            device_id,
            public_key,
            signing_key_id,
        })
    }

    /// Explicit UI display only; ordinary Debug/logs are redacted.
    pub fn brain_id(&self) -> &str {
        &self.brain_id
    }

    pub fn device_id(&self) -> &str {
        self.device_id.as_str()
    }

    pub fn signing_key_id(&self) -> &str {
        self.signing_key_id.as_str()
    }

    /// Full public key, not a shortened authentication code or proof of trust.
    pub fn full_public_key_hex(&self) -> String {
        use std::fmt::Write;
        let mut hex = String::with_capacity(64);
        for byte in self.public_key {
            write!(hex, "{byte:02x}").expect("String writes are infallible");
        }
        hex
    }
}

impl fmt::Debug for PeerBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PeerBinding(<public claims redacted; unauthenticated>)")
    }
}

/// Immutable candidate containing both complete bindings and transport choice.
#[derive(Clone, PartialEq, Eq)]
pub struct PairingCandidate {
    local: PeerBinding,
    peer: PeerBinding,
    transport_association: String,
}

impl PairingCandidate {
    pub fn new(
        local: PeerBinding,
        peer: PeerBinding,
        transport_association: &str,
    ) -> Result<Self, PeerReviewError> {
        if local.brain_id != peer.brain_id {
            return Err(PeerReviewError::DifferentBrain);
        }
        if local.device_id == peer.device_id {
            return Err(PeerReviewError::SameDevice);
        }
        if local.public_key == peer.public_key || local.signing_key_id == peer.signing_key_id {
            return Err(PeerReviewError::SharedKey);
        }
        // Opaque association from a future transport adapter, not validation of
        // Syncthing's native DeviceID/checksum or authentication of a process.
        if transport_association.is_empty()
            || transport_association.len() > MAX_TRANSPORT_ASSOCIATION_BYTES
            || !transport_association
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            return Err(PeerReviewError::InvalidTransportAssociation);
        }
        Ok(Self {
            local,
            peer,
            transport_association: transport_association.to_owned(),
        })
    }

    pub fn local(&self) -> &PeerBinding {
        &self.local
    }

    pub fn peer(&self) -> &PeerBinding {
        &self.peer
    }

    pub fn transport_association(&self) -> &str {
        &self.transport_association
    }
}

impl fmt::Debug for PairingCandidate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PairingCandidate(<review snapshot redacted>)")
    }
}

/// Native in-process ticket only. No wire format or IPC authorization implied.
#[derive(Clone, PartialEq, Eq)]
pub struct ReviewTicket([u8; 32]);

impl fmt::Debug for ReviewTicket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ReviewTicket(<redacted>)")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReviewState {
    Pending,
    Closed,
}

/// A local acknowledgement only. Intentionally not Clone/Serialize/Deserialize.
/// It grants no incoming-data or transport capability.
pub struct LocalReviewReceipt {
    candidate: PairingCandidate,
}

impl LocalReviewReceipt {
    pub fn candidate(&self) -> &PairingCandidate {
        &self.candidate
    }
}

impl fmt::Debug for LocalReviewReceipt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LocalReviewReceipt(<local acknowledgement only>)")
    }
}

pub struct PendingReview {
    candidate: PairingCandidate,
    ticket: ReviewTicket,
    started: Instant,
    state: ReviewState,
}

impl PendingReview {
    pub fn start(candidate: PairingCandidate) -> Result<Self, PeerReviewError> {
        Self::start_with_random(candidate, |bytes| {
            getrandom::getrandom(bytes).map_err(|_| PeerReviewError::RandomUnavailable)
        })
    }

    fn start_with_random(
        candidate: PairingCandidate,
        random: impl FnOnce(&mut [u8]) -> Result<(), PeerReviewError>,
    ) -> Result<Self, PeerReviewError> {
        let mut ticket = [0; 32];
        random(&mut ticket)?;
        Ok(Self {
            candidate,
            ticket: ReviewTicket(ticket),
            started: Instant::now(),
            state: ReviewState::Pending,
        })
    }

    pub fn ticket(&self) -> &ReviewTicket {
        &self.ticket
    }

    pub fn candidate(&self) -> &PairingCandidate {
        &self.candidate
    }

    pub fn cancel(&mut self) {
        self.state = ReviewState::Closed;
    }

    /// Caller must obtain the CURRENT candidate from daemon-owned setup state.
    /// Merely echoing the original snapshot cannot detect external state changes.
    /// Snapshot loading, durable commit and bilateral authentication are not
    /// implemented here; callers must not turn this receipt into peer trust.
    pub fn confirm_local(
        &mut self,
        ticket: &ReviewTicket,
        current_candidate: &PairingCandidate,
    ) -> Result<LocalReviewReceipt, PeerReviewError> {
        self.confirm_at(ticket, current_candidate, Instant::now())
    }

    fn confirm_at(
        &mut self,
        ticket: &ReviewTicket,
        current_candidate: &PairingCandidate,
        now: Instant,
    ) -> Result<LocalReviewReceipt, PeerReviewError> {
        if self.state != ReviewState::Pending {
            return Err(PeerReviewError::Closed);
        }
        if now
            .checked_duration_since(self.started)
            .is_none_or(|elapsed| elapsed >= REVIEW_LIFETIME)
        {
            self.cancel();
            return Err(PeerReviewError::Expired);
        }
        if ticket != &self.ticket {
            return Err(PeerReviewError::TicketMismatch);
        }
        if current_candidate != &self.candidate {
            self.cancel();
            return Err(PeerReviewError::SnapshotChanged);
        }
        self.state = ReviewState::Closed;
        Ok(LocalReviewReceipt {
            candidate: self.candidate.clone(),
        })
    }
}

impl fmt::Debug for PendingReview {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PendingReview")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

/// Full-u64 inclusive prefix arithmetic only, NOT authenticated policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LastAcceptedSequence(u64);

impl LastAcceptedSequence {
    pub fn from_u64(value: u64) -> Self {
        Self(value)
    }

    pub fn parse_decimal(value: &str) -> Result<Self, PeerReviewError> {
        if value.is_empty()
            || value.len() > 20
            || !value.bytes().all(|byte| byte.is_ascii_digit())
            || (value.len() > 1 && value.starts_with('0'))
        {
            return Err(PeerReviewError::InvalidCutoff);
        }
        value
            .parse::<u64>()
            .map(Self)
            .map_err(|_| PeerReviewError::InvalidCutoff)
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }

    pub fn to_decimal(self) -> String {
        self.0.to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefixEligibility {
    /// Still requires authentic trust, schema, signature, chain and safety checks.
    WithinPrefix,
    AfterCutoff,
}

/// Pure comparison over an exact Brain/device/full-key binding. Construction
/// does not authenticate the issuer or authorize a revocation. Do not use with
/// unvalidated transport records; ADR-0019's policy admission is still gated.
#[derive(Clone, PartialEq, Eq)]
pub struct CutoffComparison {
    binding: PeerBinding,
    last_accepted: LastAcceptedSequence,
}

impl CutoffComparison {
    pub fn new(binding: PeerBinding, last_accepted: LastAcceptedSequence) -> Self {
        Self {
            binding,
            last_accepted,
        }
    }

    pub fn last_accepted(&self) -> LastAcceptedSequence {
        self.last_accepted
    }

    /// Validated policy inputs must be supplied by a future authority evaluator.
    pub fn tightened_with(&self, other: &Self) -> Result<Self, PeerReviewError> {
        if self.binding != other.binding {
            return Err(PeerReviewError::BindingMismatch);
        }
        Ok(Self::new(
            self.binding.clone(),
            LastAcceptedSequence(self.last_accepted.0.min(other.last_accepted.0)),
        ))
    }

    pub fn compare(
        &self,
        binding: &PeerBinding,
        sequence: &DeviceSeq,
    ) -> Result<PrefixEligibility, PeerReviewError> {
        if &self.binding != binding {
            return Err(PeerReviewError::BindingMismatch);
        }
        let sequence = sequence
            .to_u64()
            .map_err(|_| PeerReviewError::InvalidCutoff)?;
        Ok(if sequence <= self.last_accepted.0 {
            PrefixEligibility::WithinPrefix
        } else {
            PrefixEligibility::AfterCutoff
        })
    }
}

impl fmt::Debug for CutoffComparison {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CutoffComparison(<comparison only; policy not authenticated>)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;

    const BRAIN: &str = "11111111-1111-4111-8111-111111111111";
    const OTHER_BRAIN: &str = "22222222-2222-4222-8222-222222222222";

    fn binding(brain: &str, device: &str, seed: u8) -> PeerBinding {
        let key = SigningKey::from_bytes(&[seed; 32]).verifying_key();
        let id = signing_key_id_from_verifying_key(&key).unwrap();
        PeerBinding::from_public_claims(brain, device, key.to_bytes(), id.as_str()).unwrap()
    }

    fn candidate() -> PairingCandidate {
        PairingCandidate::new(
            binding(BRAIN, "local-device", 1),
            binding(BRAIN, "peer-device", 2),
            "transport-peer-A",
        )
        .unwrap()
    }

    fn review() -> PendingReview {
        PendingReview::start(candidate()).unwrap()
    }

    #[test]
    fn valid_public_claims_expose_full_key_only_through_explicit_display() {
        let peer = binding(BRAIN, "peer-device", 2);
        assert_eq!(peer.brain_id(), BRAIN);
        assert_eq!(peer.device_id(), "peer-device");
        assert_eq!(peer.signing_key_id().len(), 32);
        let hex = peer.full_public_key_hex();
        assert_eq!(hex.len(), 64);
        assert!(
            hex.bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        );
        assert!(!format!("{peer:?}").contains(&hex));
    }

    #[test]
    fn invalid_brain_and_device_are_redacted_typed_errors() {
        let key = SigningKey::from_bytes(&[1; 32]).verifying_key();
        let id = signing_key_id_from_verifying_key(&key).unwrap();
        for (brain, device) in [("PRIVATE_INPUT", "peer"), (BRAIN, "../PRIVATE_INPUT")] {
            let error = PeerBinding::from_public_claims(brain, device, key.to_bytes(), id.as_str())
                .unwrap_err();
            assert_eq!(error, PeerReviewError::InvalidIdentity);
            assert!(!format!("{error} {error:?}").contains("PRIVATE_INPUT"));
        }
    }

    #[test]
    fn weak_public_key_is_rejected() {
        let mut identity_point = [0; 32];
        identity_point[0] = 1;
        assert_eq!(
            PeerBinding::from_public_claims(BRAIN, "peer", identity_point, &"0".repeat(32)),
            Err(PeerReviewError::InvalidKey)
        );
    }

    #[test]
    fn incorrect_and_malformed_claimed_key_ids_are_rejected() {
        let peer = binding(BRAIN, "peer", 2);
        for id in ["0".repeat(32), "PRIVATE_INPUT".into()] {
            assert_eq!(
                PeerBinding::from_public_claims(BRAIN, "peer", peer.public_key, &id),
                Err(PeerReviewError::KeyIdMismatch)
            );
        }
    }

    #[test]
    fn different_brains_cannot_pair() {
        assert_eq!(
            PairingCandidate::new(
                binding(BRAIN, "local", 1),
                binding(OTHER_BRAIN, "peer", 2),
                "T"
            ),
            Err(PeerReviewError::DifferentBrain)
        );
    }

    #[test]
    fn duplicate_device_and_shared_signing_key_are_rejected() {
        assert_eq!(
            PairingCandidate::new(binding(BRAIN, "same", 1), binding(BRAIN, "same", 2), "T"),
            Err(PeerReviewError::SameDevice)
        );
        assert_eq!(
            PairingCandidate::new(binding(BRAIN, "local", 1), binding(BRAIN, "peer", 1), "T"),
            Err(PeerReviewError::SharedKey)
        );
    }

    #[test]
    fn device_ids_remain_case_sensitive() {
        let candidate = PairingCandidate::new(
            binding(BRAIN, "Device", 1),
            binding(BRAIN, "device", 2),
            "T",
        )
        .unwrap();
        assert_ne!(candidate.local().device_id(), candidate.peer().device_id());
    }

    #[test]
    fn transport_association_is_bounded_and_not_path_or_header_input() {
        for transport in [
            "".into(),
            "../peer".into(),
            "peer\r\nsecret".into(),
            "🧠".into(),
            "A".repeat(129),
        ] {
            assert_eq!(
                PairingCandidate::new(
                    binding(BRAIN, "local", 1),
                    binding(BRAIN, "peer", 2),
                    &transport
                ),
                Err(PeerReviewError::InvalidTransportAssociation)
            );
        }
    }

    #[test]
    fn exact_candidate_and_ticket_produce_local_receipt_once() {
        let mut review = review();
        let ticket = review.ticket().clone();
        let current = review.candidate().clone();
        let receipt = review.confirm_local(&ticket, &current).unwrap();
        assert_eq!(receipt.candidate(), &current);
        assert_eq!(
            review.confirm_local(&ticket, &current).unwrap_err(),
            PeerReviewError::Closed
        );
    }

    #[test]
    fn concurrent_callbacks_on_one_review_cannot_produce_two_receipts() {
        use std::sync::{Arc, Barrier, Mutex};
        let review = Arc::new(Mutex::new(review()));
        let ticket = review.lock().unwrap().ticket().clone();
        let barrier = Arc::new(Barrier::new(2));
        let workers: Vec<_> = (0..2)
            .map(|_| {
                let review = Arc::clone(&review);
                let barrier = Arc::clone(&barrier);
                let ticket = ticket.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    review
                        .lock()
                        .unwrap()
                        .confirm_local(&ticket, &candidate())
                        .is_ok()
                })
            })
            .collect();
        let receipts = workers
            .into_iter()
            .map(|worker| usize::from(worker.join().unwrap()))
            .sum::<usize>();
        assert_eq!(receipts, 1);
    }

    #[test]
    fn wrong_ticket_does_not_confirm_or_consume_correct_review() {
        let mut review = review();
        let current = candidate();
        assert_eq!(
            review
                .confirm_local(&ReviewTicket([0; 32]), &current)
                .unwrap_err(),
            PeerReviewError::TicketMismatch
        );
        let ticket = review.ticket().clone();
        assert!(review.confirm_local(&ticket, &current).is_ok());
    }

    #[test]
    fn cancel_is_terminal_and_idempotent() {
        let mut review = review();
        let ticket = review.ticket().clone();
        review.cancel();
        review.cancel();
        assert_eq!(
            review.confirm_local(&ticket, &candidate()).unwrap_err(),
            PeerReviewError::Closed
        );
    }

    #[test]
    fn a_new_review_does_not_accept_the_previous_ticket() {
        let previous = review();
        let mut fresh = review();
        assert_ne!(previous.ticket(), fresh.ticket());
        assert_eq!(
            fresh
                .confirm_local(previous.ticket(), &candidate())
                .unwrap_err(),
            PeerReviewError::TicketMismatch
        );
    }

    #[test]
    fn random_failure_does_not_create_a_review() {
        let error = PendingReview::start_with_random(candidate(), |_| {
            Err(PeerReviewError::RandomUnavailable)
        })
        .unwrap_err();
        assert_eq!(error, PeerReviewError::RandomUnavailable);
    }

    #[test]
    fn expires_at_deadline_without_wall_clock_input() {
        let mut review = review();
        let ticket = review.ticket().clone();
        let deadline = review.started + REVIEW_LIFETIME;
        assert_eq!(
            review
                .confirm_at(&ticket, &candidate(), deadline)
                .unwrap_err(),
            PeerReviewError::Expired
        );
        assert_eq!(
            review
                .confirm_at(&ticket, &candidate(), deadline)
                .unwrap_err(),
            PeerReviewError::Closed
        );
    }

    #[test]
    fn just_before_deadline_can_confirm() {
        let mut review = review();
        let ticket = review.ticket().clone();
        let now = review.started + REVIEW_LIFETIME - Duration::from_millis(1);
        assert!(review.confirm_at(&ticket, &candidate(), now).is_ok());
    }

    #[test]
    fn impossible_monotonic_time_fails_closed() {
        let mut review = review();
        let ticket = review.ticket().clone();
        // Windows Instant uses QPC ticks: a 1 ns subtraction can round back to
        // the same Instant. Construct an observably earlier value on both OSes.
        let earlier = review.started - Duration::from_secs(1);
        assert!(earlier < review.started);
        assert_eq!(
            review
                .confirm_at(&ticket, &candidate(), earlier)
                .unwrap_err(),
            PeerReviewError::Expired
        );
    }

    #[test]
    fn changed_snapshot_invalidates_original_review_permanently() {
        let original = candidate();
        let changes = [
            PairingCandidate::new(
                binding(BRAIN, "new-local", 1),
                original.peer.clone(),
                "transport-peer-A",
            )
            .unwrap(),
            PairingCandidate::new(
                original.local.clone(),
                binding(BRAIN, "new-peer", 2),
                "transport-peer-A",
            )
            .unwrap(),
            PairingCandidate::new(
                original.local.clone(),
                binding(BRAIN, "peer-device", 3),
                "transport-peer-A",
            )
            .unwrap(),
            PairingCandidate::new(
                binding(BRAIN, "local-device", 3),
                original.peer.clone(),
                "transport-peer-A",
            )
            .unwrap(),
            PairingCandidate::new(
                original.local.clone(),
                original.peer.clone(),
                "transport-peer-B",
            )
            .unwrap(),
            PairingCandidate::new(
                binding(OTHER_BRAIN, "local-device", 1),
                binding(OTHER_BRAIN, "peer-device", 2),
                "transport-peer-A",
            )
            .unwrap(),
        ];
        for changed in changes {
            let mut review = review();
            let ticket = review.ticket().clone();
            assert_eq!(
                review.confirm_local(&ticket, &changed).unwrap_err(),
                PeerReviewError::SnapshotChanged
            );
            assert_eq!(
                review.confirm_local(&ticket, &original).unwrap_err(),
                PeerReviewError::Closed
            );
        }
    }

    #[test]
    fn debug_does_not_leak_tickets_or_identity_snapshots() {
        let review = review();
        let debug = format!(
            "{review:?} {:?} {:?} {:?}",
            review.ticket(),
            review.candidate(),
            review.candidate().peer()
        );
        for secret in [
            BRAIN,
            "peer-device",
            "transport-peer-A",
            &format!("{:?}", review.ticket.0),
        ] {
            assert!(!debug.contains(secret));
        }
    }

    fn cutoff(value: u64) -> CutoffComparison {
        CutoffComparison::new(
            binding(BRAIN, "peer-device", 2),
            LastAcceptedSequence::from_u64(value),
        )
    }

    #[test]
    fn zero_cutoff_excludes_every_issued_sequence() {
        let cutoff = cutoff(0);
        for seq in [1, 2, u64::MAX] {
            assert_eq!(
                cutoff
                    .compare(&cutoff.binding, &DeviceSeq::from_u64(seq).unwrap())
                    .unwrap(),
                PrefixEligibility::AfterCutoff
            );
        }
    }

    #[test]
    fn inclusive_cutoff_before_at_and_after() {
        let cutoff = cutoff(7);
        for (seq, expected) in [
            (6, PrefixEligibility::WithinPrefix),
            (7, PrefixEligibility::WithinPrefix),
            (8, PrefixEligibility::AfterCutoff),
        ] {
            assert_eq!(
                cutoff
                    .compare(&cutoff.binding, &DeviceSeq::from_u64(seq).unwrap())
                    .unwrap(),
                expected
            );
        }
    }

    #[test]
    fn maximum_cutoff_does_not_overflow_or_round_above_js_precision() {
        let cutoff = cutoff(u64::MAX);
        let seq = DeviceSeq::parse("18446744073709551615").unwrap();
        assert_eq!(
            cutoff.compare(&cutoff.binding, &seq).unwrap(),
            PrefixEligibility::WithinPrefix
        );
        assert_eq!(
            LastAcceptedSequence::parse_decimal(seq.as_str())
                .unwrap()
                .as_u64(),
            u64::MAX
        );
        let cutoff = CutoffComparison::new(
            cutoff.binding,
            LastAcceptedSequence::parse_decimal("9007199254740992").unwrap(),
        );
        assert_eq!(
            cutoff
                .compare(
                    &cutoff.binding,
                    &DeviceSeq::parse("9007199254740993").unwrap()
                )
                .unwrap(),
            PrefixEligibility::AfterCutoff
        );
    }

    #[test]
    fn cutoff_decimal_parser_is_strict_and_allows_only_zero_sentinel() {
        for input in [
            "",
            "00",
            "01",
            "+1",
            "-1",
            " 1",
            "1\n",
            "1.0",
            "1e2",
            "١",
            "18446744073709551616",
            "000000000000000000000",
        ] {
            assert_eq!(
                LastAcceptedSequence::parse_decimal(input),
                Err(PeerReviewError::InvalidCutoff)
            );
        }
        for (input, value) in [("0", 0), ("1", 1), ("18446744073709551615", u64::MAX)] {
            let parsed = LastAcceptedSequence::parse_decimal(input).unwrap();
            assert_eq!(parsed.as_u64(), value);
            assert_eq!(parsed.to_decimal(), input);
        }
    }

    #[test]
    fn cutoff_cannot_be_applied_or_merged_across_identity_bindings() {
        let original = cutoff(7);
        let changed = [
            binding(OTHER_BRAIN, "peer-device", 2),
            binding(BRAIN, "Peer-device", 2),
            binding(BRAIN, "peer-device", 3),
        ];
        for binding in changed {
            assert_eq!(
                original.compare(&binding, &DeviceSeq::from_u64(1).unwrap()),
                Err(PeerReviewError::BindingMismatch)
            );
            assert_eq!(
                original.tightened_with(&CutoffComparison::new(
                    binding,
                    LastAcceptedSequence::from_u64(0)
                )),
                Err(PeerReviewError::BindingMismatch)
            );
        }
    }

    #[test]
    fn cutoff_tightening_is_associative_commutative_idempotent_and_never_increases() {
        for a in [0, 1, 7, u64::MAX] {
            for b in [0, 1, 7, u64::MAX] {
                let left = cutoff(a);
                let right = cutoff(b);
                let merged = left.tightened_with(&right).unwrap();
                assert_eq!(merged.last_accepted().as_u64(), a.min(b));
                assert_eq!(merged, right.tightened_with(&left).unwrap());
                assert_eq!(merged, merged.tightened_with(&merged).unwrap());
                for c in [0, 3, u64::MAX] {
                    let third = cutoff(c);
                    assert_eq!(
                        merged.tightened_with(&third).unwrap(),
                        left.tightened_with(&right.tightened_with(&third).unwrap())
                            .unwrap()
                    );
                }
            }
        }
    }
}
