//! Ephemeral, connection-bound single-use owner approval capabilities.
//!
//! The Windows IPC path binds these single-use capabilities to one
//! authenticated stream and journals the owner-visible snapshot before signing.
//! macOS completion remains fail-closed until peer process-instance binding is
//! implemented. Capabilities themselves are never persisted.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use crate::bootstrap_journal::ApprovalSnapshot;

const MAX_ACTIVE_SESSIONS: usize = 64;
const MAX_SESSIONS_PER_CONNECTION: usize = 8;
const APPROVAL_SESSION_LIFETIME: Duration = Duration::from_secs(5 * 60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalSessionError {
    RandomUnavailable,
    RegistryPoisoned,
    SessionLimit,
    InvalidCapability,
    UnknownSession,
    WrongConnection,
    SnapshotMismatch,
    SnapshotInvalid,
    ApprovalSigningFailed,
    ApprovalJournalFailed,
}

impl fmt::Display for ApprovalSessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RandomUnavailable => f.write_str("approval capability generation failed"),
            Self::RegistryPoisoned => f.write_str("approval session registry unavailable"),
            Self::SessionLimit => f.write_str("approval session limit reached"),
            Self::InvalidCapability => f.write_str("approval capability format invalid"),
            Self::UnknownSession => f.write_str("approval session is no longer pending"),
            Self::WrongConnection => f.write_str("approval session belongs to another connection"),
            Self::SnapshotMismatch => f.write_str("approval request snapshot changed"),
            Self::SnapshotInvalid => f.write_str("approval request snapshot invalid"),
            Self::ApprovalSigningFailed => f.write_str("bootstrap approval signing failed"),
            Self::ApprovalJournalFailed => f.write_str("bootstrap approval journal update failed"),
        }
    }
}

impl std::error::Error for ApprovalSessionError {}

/// An opaque identity created once for an authenticated IPC connection.
/// Fields are private so callers cannot bind a grant to a caller-chosen value.
pub struct ApprovalConnection {
    id: [u8; 32],
    registry: Weak<Mutex<HashMap<[u8; 32], PendingSession>>>,
}

impl Drop for ApprovalConnection {
    fn drop(&mut self) {
        if let Some(registry) = self.registry.upgrade()
            && let Ok(mut pending) = registry.lock()
        {
            pending.retain(|_, session| session.connection_id != self.id);
        }
    }
}

impl fmt::Debug for ApprovalConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApprovalConnection([redacted])")
    }
}

/// Wire-safe response fields. The bearer capability is deliberately omitted
/// from Debug output and is never written to the durable bootstrap journal.
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalSessionGrant {
    pub approval_request_digest: String,
    pub capability: String,
}

impl fmt::Debug for ApprovalSessionGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApprovalSessionGrant")
            .field("approval_request_digest", &self.approval_request_digest)
            .field("capability", &"[redacted]")
            .finish()
    }
}

struct PendingSession {
    connection_id: [u8; 32],
    approval_request_digest: String,
    snapshot: ApprovalSnapshot,
    expires_at: Instant,
}

/// Process-local registry: restart discards every capability by construction.
pub struct ApprovalSessionRegistry {
    pending: Arc<Mutex<HashMap<[u8; 32], PendingSession>>>,
}

impl Default for ApprovalSessionRegistry {
    fn default() -> Self {
        Self {
            pending: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl ApprovalSessionRegistry {
    /// Creates the unforgeable identity that must be retained for exactly one
    /// authenticated IPC stream. Dropping it automatically invalidates grants.
    pub fn open_connection(&self) -> Result<ApprovalConnection, ApprovalSessionError> {
        let mut id = [0_u8; 32];
        getrandom::getrandom(&mut id).map_err(|_| ApprovalSessionError::RandomUnavailable)?;
        Ok(ApprovalConnection {
            id,
            registry: Arc::downgrade(&self.pending),
        })
    }
}

impl ApprovalSessionRegistry {
    pub fn issue(
        &self,
        connection: &ApprovalConnection,
        snapshot: ApprovalSnapshot,
    ) -> Result<ApprovalSessionGrant, ApprovalSessionError> {
        self.issue_with_lifetime(connection, snapshot, APPROVAL_SESSION_LIFETIME)
    }

    fn issue_with_lifetime(
        &self,
        connection: &ApprovalConnection,
        snapshot: ApprovalSnapshot,
        lifetime: Duration,
    ) -> Result<ApprovalSessionGrant, ApprovalSessionError> {
        let digest = snapshot
            .digest()
            .map_err(|_| ApprovalSessionError::SnapshotInvalid)?;
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| ApprovalSessionError::RegistryPoisoned)?;
        let now = Instant::now();
        pending.retain(|_, session| session.expires_at > now);
        let on_connection = pending
            .values()
            .filter(|session| session.connection_id == connection.id)
            .count();
        if pending.len() >= MAX_ACTIVE_SESSIONS || on_connection >= MAX_SESSIONS_PER_CONNECTION {
            return Err(ApprovalSessionError::SessionLimit);
        }

        let mut capability = [0_u8; 32];
        let mut unique = false;
        for _ in 0..4 {
            getrandom::getrandom(&mut capability)
                .map_err(|_| ApprovalSessionError::RandomUnavailable)?;
            if !pending.contains_key(&capability) {
                unique = true;
                break;
            }
        }
        if !unique {
            return Err(ApprovalSessionError::RandomUnavailable);
        }
        pending.insert(
            capability,
            PendingSession {
                connection_id: connection.id,
                approval_request_digest: digest.clone(),
                snapshot,
                expires_at: now + lifetime,
            },
        );
        Ok(ApprovalSessionGrant {
            approval_request_digest: digest,
            capability: encode_hex(&capability),
        })
    }

    /// Atomically validate and consume. The returned frozen snapshot is the
    /// only input the caller may use for the confirmed operation.
    pub fn consume(
        &self,
        connection: &ApprovalConnection,
        capability: &str,
        approval_request_digest: &str,
    ) -> Result<ApprovalSnapshot, ApprovalSessionError> {
        let capability = decode_hex(capability)?;
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| ApprovalSessionError::RegistryPoisoned)?;
        let session = pending
            .get(&capability)
            .ok_or(ApprovalSessionError::UnknownSession)?;
        if session.connection_id != connection.id {
            return Err(ApprovalSessionError::WrongConnection);
        }
        if session.expires_at <= Instant::now() {
            pending.remove(&capability);
            return Err(ApprovalSessionError::UnknownSession);
        }
        if session.approval_request_digest != approval_request_digest {
            return Err(ApprovalSessionError::SnapshotMismatch);
        }
        let session = pending
            .remove(&capability)
            .expect("session remains present while registry lock is held");
        Ok(session.snapshot)
    }

    /// Consume an exact bootstrap approval grant before signing. The grant is
    /// burned on every confirmation attempt from its owning connection; later
    /// validation/signing failures require a newly displayed confirmation.
    pub fn confirm_bootstrap_approval(
        &self,
        connection: &ApprovalConnection,
        capability: &str,
        approval_request_digest: &str,
        request: &cryptex_events::VerifiedBootstrapApprovalRequest,
        signing_key: &ed25519_dalek::SigningKey,
    ) -> Result<Vec<u8>, ApprovalSessionError> {
        self.confirm_bootstrap_approval_with_signer(
            connection,
            capability,
            approval_request_digest,
            request,
            |verified| {
                cryptex_events::sign_bootstrap_approval(verified, signing_key)
                    .map_err(|_| ApprovalSessionError::ApprovalSigningFailed)
            },
        )
    }

    /// Consume an exact owner grant before loading protected signing material
    /// or invoking a signer. This lets IPC perform its read-only SecureStore
    /// access strictly after the one-time capability has been claimed.
    pub fn confirm_bootstrap_approval_with_signer(
        &self,
        connection: &ApprovalConnection,
        capability: &str,
        approval_request_digest: &str,
        request: &cryptex_events::VerifiedBootstrapApprovalRequest,
        signer: impl FnOnce(
            &cryptex_events::VerifiedBootstrapApprovalRequest,
        ) -> Result<Vec<u8>, ApprovalSessionError>,
    ) -> Result<Vec<u8>, ApprovalSessionError> {
        let capability = decode_hex(capability)?;
        {
            let mut pending = self
                .pending
                .lock()
                .map_err(|_| ApprovalSessionError::RegistryPoisoned)?;
            let session = pending
                .get(&capability)
                .ok_or(ApprovalSessionError::UnknownSession)?;
            if session.connection_id != connection.id {
                return Err(ApprovalSessionError::WrongConnection);
            }
            if session.expires_at <= Instant::now() {
                pending.remove(&capability);
                return Err(ApprovalSessionError::UnknownSession);
            }
            let session = pending
                .remove(&capability)
                .expect("session remains present while registry lock is held");
            if session.approval_request_digest != approval_request_digest
                || !session.snapshot.matches_bootstrap_request(request)
            {
                return Err(ApprovalSessionError::SnapshotMismatch);
            }
            let snapshot = session.snapshot;
            debug_assert_eq!(
                snapshot.digest().ok().as_deref(),
                Some(approval_request_digest)
            );
        }
        signer(request)
    }

    /// Consume the owner grant, sign, and durably record the signed message
    /// before returning bytes to IPC. Any failure burns the grant and releases
    /// no message to the peer.
    pub fn confirm_bootstrap_approval_journaled(
        &self,
        connection: &ApprovalConnection,
        capability: &str,
        approval_request_digest: &str,
        request: &cryptex_events::VerifiedBootstrapApprovalRequest,
        journal: &crate::bootstrap_journal::BootstrapJournalStore,
        signer: impl FnOnce(
            &cryptex_events::VerifiedBootstrapApprovalRequest,
        ) -> Result<Vec<u8>, ApprovalSessionError>,
    ) -> Result<Vec<u8>, ApprovalSessionError> {
        self.confirm_bootstrap_approval_with_signer(
            connection,
            capability,
            approval_request_digest,
            request,
            |verified| {
                let signed = signer(verified)?;
                journal
                    .record_local_approval_signature(verified, &signed)
                    .map_err(|_| ApprovalSessionError::ApprovalJournalFailed)?;
                Ok(signed)
            },
        )
    }

    /// Cancellation only succeeds on the exact connection that received the
    /// capability. A cancelled capability cannot be restored or replayed.
    pub fn cancel(
        &self,
        connection: &ApprovalConnection,
        capability: &str,
    ) -> Result<(), ApprovalSessionError> {
        let capability = decode_hex(capability)?;
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| ApprovalSessionError::RegistryPoisoned)?;
        let session = pending
            .get(&capability)
            .ok_or(ApprovalSessionError::UnknownSession)?;
        if session.connection_id != connection.id {
            return Err(ApprovalSessionError::WrongConnection);
        }
        if session.expires_at <= Instant::now() {
            pending.remove(&capability);
            return Err(ApprovalSessionError::UnknownSession);
        }
        pending.remove(&capability);
        Ok(())
    }

    /// Called when an authenticated IPC stream closes. All its outstanding
    /// capabilities are invalidated before that connection is discarded.
    pub fn invalidate_connection(
        &self,
        connection: &ApprovalConnection,
    ) -> Result<usize, ApprovalSessionError> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| ApprovalSessionError::RegistryPoisoned)?;
        let before = pending.len();
        pending.retain(|_, session| session.connection_id != connection.id);
        Ok(before - pending.len())
    }
}

fn decode_hex(value: &str) -> Result<[u8; 32], ApprovalSessionError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ApprovalSessionError::InvalidCapability);
    }
    let mut result = [0_u8; 32];
    for (index, byte) in result.iter_mut().enumerate() {
        let offset = index * 2;
        *byte = (hex_nibble(value.as_bytes()[offset])? << 4)
            | hex_nibble(value.as_bytes()[offset + 1])?;
    }
    Ok(result)
}

fn hex_nibble(byte: u8) -> Result<u8, ApprovalSessionError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(ApprovalSessionError::InvalidCapability),
    }
}

fn encode_hex(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap_journal::{
        ApprovalIdentity, BootstrapJournalError, BootstrapJournalStore, BootstrapOperation,
        BootstrapRole, BootstrapState, PendingBootstrapTransaction, TransportAssociation,
    };
    use ed25519_dalek::{Signer, SigningKey};
    use serde_json::{Value, json};

    const BRAIN: &str = "22222222-2222-4222-8222-222222222222";
    const TX: &str = "abababababababababababababababababababababababababababababababab";
    const LOCAL_TRANSPORT: &str = "AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA";
    const PEER_TRANSPORT: &str = "BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB";

    fn hex(bytes: &[u8]) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut result = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            result.push(HEX[(byte >> 4) as usize] as char);
            result.push(HEX[(byte & 0x0f) as usize] as char);
        }
        result
    }

    fn sign_message(mut value: Value, key: &SigningKey) -> Vec<u8> {
        let canonical = serde_json_canonicalizer::to_vec(&value).unwrap();
        let mut preimage = b"cryptex-peer-bootstrap-sig-v1\0".to_vec();
        preimage.extend_from_slice(&canonical);
        let signature = key.sign(&preimage);
        value.as_object_mut().unwrap().insert(
            "signature".into(),
            Value::String(hex(&signature.to_bytes())),
        );
        serde_json_canonicalizer::to_vec(&value).unwrap()
    }

    fn message_hash(bytes: &[u8]) -> String {
        let value: Value = serde_json::from_slice(bytes).unwrap();
        let canonical = serde_json_canonicalizer::to_vec(&value).unwrap();
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"cryptex-peer-bootstrap-message-v1");
        hasher.update(&[0]);
        hasher.update(&canonical);
        hex(hasher.finalize().as_bytes())
    }

    fn signed_prefix() -> (Vec<u8>, Vec<u8>, SigningKey, SigningKey) {
        let local_key = SigningKey::from_bytes(&[0x11; 32]);
        let peer_key = SigningKey::from_bytes(&[0x22; 32]);
        let (offer, proof) = signed_prefix_for(
            &local_key,
            &peer_key,
            "local-device",
            "peer-device",
            BRAIN,
            TX,
            LOCAL_TRANSPORT,
            PEER_TRANSPORT,
        );
        (offer, proof, local_key, peer_key)
    }

    #[allow(clippy::too_many_arguments)]
    fn signed_prefix_for(
        local_key: &SigningKey,
        peer_key: &SigningKey,
        local_device: &str,
        peer_device: &str,
        brain_id: &str,
        transaction_id: &str,
        local_transport: &str,
        peer_transport: &str,
    ) -> (Vec<u8>, Vec<u8>) {
        let local_key_id =
            cryptex_events::signing_key_id_from_verifying_key(&local_key.verifying_key()).unwrap();
        let peer_key_id =
            cryptex_events::signing_key_id_from_verifying_key(&peer_key.verifying_key()).unwrap();
        let offer = sign_message(
            json!({
                "protocol_version": 1,
                "message_type": "offer",
                "transaction_id": transaction_id,
                "brain_id": brain_id,
                "sender_device_id": local_device,
                "sender_signing_key_id": local_key_id.as_str(),
                "sender_public_key": hex(&local_key.verifying_key().to_bytes()),
                "sender_transport_device_id": local_transport,
                "peer_device_id": null,
                "peer_signing_key_id": null,
                "peer_public_key": null,
                "peer_transport_device_id": null,
                "previous_hash": "00".repeat(32),
                "payload": {"initiator_nonce": "11".repeat(32)}
            }),
            local_key,
        );
        let offer_hash = message_hash(&offer);
        let proof = sign_message(
            json!({
                "protocol_version": 1,
                "message_type": "proof",
                "transaction_id": transaction_id,
                "brain_id": brain_id,
                "sender_device_id": peer_device,
                "sender_signing_key_id": peer_key_id.as_str(),
                "sender_public_key": hex(&peer_key.verifying_key().to_bytes()),
                "sender_transport_device_id": peer_transport,
                "peer_device_id": local_device,
                "peer_signing_key_id": local_key_id.as_str(),
                "peer_public_key": hex(&local_key.verifying_key().to_bytes()),
                "peer_transport_device_id": local_transport,
                "previous_hash": offer_hash,
                "payload": {
                    "offer_hash": offer_hash,
                    "responder_nonce": "22".repeat(32)
                }
            }),
            peer_key,
        );
        (offer, proof)
    }

    fn snapshot() -> ApprovalSnapshot {
        ApprovalSnapshot {
            brain_id: "00000000-0000-4000-8000-000000000001".into(),
            transaction_id: "a".repeat(64),
            role: BootstrapRole::Initiator,
            operation: BootstrapOperation::PeerAdmission,
            agreement_hash: Some("b".repeat(64)),
            previous_message_hash: Some("c".repeat(64)),
            local_identity: ApprovalIdentity {
                device_id: "local-device".into(),
                signing_key_id: "1".repeat(32),
                public_key_fingerprint_sha256: "2".repeat(64),
            },
            peer_identity: Some(ApprovalIdentity {
                device_id: "peer-device".into(),
                signing_key_id: "3".repeat(32),
                public_key_fingerprint_sha256: "4".repeat(64),
            }),
            transport_association: Some(TransportAssociation {
                local_device_id: std::iter::repeat_n("AAAAAAA", 8)
                    .collect::<Vec<_>>()
                    .join("-"),
                peer_device_id: std::iter::repeat_n("BBBBBBB", 8)
                    .collect::<Vec<_>>()
                    .join("-"),
            }),
            restriction_target: None,
            restriction_reason: None,
            cutoff_seq: None,
            cutoff_content_hash: None,
            last_authorized_seq: None,
            last_authorized_content_hash: None,
        }
    }

    #[test]
    fn capability_is_connection_bound_and_single_use() {
        let registry = ApprovalSessionRegistry::default();
        let owner_connection = registry.open_connection().unwrap();
        let other_connection = registry.open_connection().unwrap();
        let expected = snapshot();
        let grant = registry.issue(&owner_connection, expected.clone()).unwrap();

        assert!(matches!(
            registry.consume(
                &other_connection,
                &grant.capability,
                &grant.approval_request_digest
            ),
            Err(ApprovalSessionError::WrongConnection)
        ));
        assert_eq!(
            registry
                .consume(
                    &owner_connection,
                    &grant.capability,
                    &grant.approval_request_digest
                )
                .unwrap(),
            expected
        );
        assert!(matches!(
            registry.consume(
                &owner_connection,
                &grant.capability,
                &grant.approval_request_digest
            ),
            Err(ApprovalSessionError::UnknownSession)
        ));
    }

    #[test]
    fn expired_capability_is_removed_and_cannot_be_consumed() {
        let registry = ApprovalSessionRegistry::default();
        let connection = registry.open_connection().unwrap();
        let grant = registry
            .issue_with_lifetime(&connection, snapshot(), Duration::ZERO)
            .unwrap();

        assert!(matches!(
            registry.consume(
                &connection,
                &grant.capability,
                &grant.approval_request_digest
            ),
            Err(ApprovalSessionError::UnknownSession)
        ));
        assert!(matches!(
            registry.consume(
                &connection,
                &grant.capability,
                &grant.approval_request_digest
            ),
            Err(ApprovalSessionError::UnknownSession)
        ));

        let (offer, proof, local_key, _) = signed_prefix();
        let prefix = [offer.as_slice(), proof.as_slice()];
        let request = cryptex_events::verify_bootstrap_approval_request(
            &prefix,
            cryptex_events::BootstrapApprovalRole::Initiator,
        )
        .unwrap();
        let grant = registry
            .issue_with_lifetime(
                &connection,
                ApprovalSnapshot::from_verified_bootstrap_request(&request).unwrap(),
                Duration::ZERO,
            )
            .unwrap();
        assert!(matches!(
            registry.confirm_bootstrap_approval(
                &connection,
                &grant.capability,
                &grant.approval_request_digest,
                &request,
                &local_key
            ),
            Err(ApprovalSessionError::UnknownSession)
        ));
    }

    #[test]
    fn journal_records_verified_prefix_idempotently_and_rejects_changed_bytes() {
        let (offer, proof, _, _) = signed_prefix();
        let messages = vec![offer.clone(), proof.clone()];
        let refs = messages.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let request = cryptex_events::verify_bootstrap_approval_request(
            &refs,
            cryptex_events::BootstrapApprovalRole::Initiator,
        )
        .unwrap();
        let snapshot = ApprovalSnapshot::from_verified_bootstrap_request(&request).unwrap();
        let transaction = PendingBootstrapTransaction::new(
            snapshot,
            "33333333-3333-4333-8333-333333333333".into(),
        )
        .unwrap();
        let root = crate::test_temp::exclusive_temp_dir("m8-approval-prefix");
        let journal = BootstrapJournalStore::new(&root).unwrap();
        journal
            .update(|stored| stored.insert(transaction.clone()))
            .unwrap();

        journal
            .record_verified_approval_prefix(&request, &messages)
            .unwrap();
        let persisted = journal.load().unwrap();
        assert_eq!(
            persisted.transactions[0].state,
            BootstrapState::ProofReceived
        );
        assert_eq!(persisted.transactions[0].transcript_messages.len(), 2);
        journal
            .record_verified_approval_prefix(&request, &messages)
            .unwrap();

        let mut changed = messages;
        changed[0].push(b' ');
        assert!(matches!(
            journal.record_verified_approval_prefix(&request, &changed),
            Err(BootstrapJournalError::Conflict(_))
        ));
        assert_eq!(
            journal.load().unwrap().transactions[0].state,
            BootstrapState::ProofReceived
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn responder_journal_prefix_reaches_peer_approval_received() {
        let (offer, proof, local_key, peer_key) = signed_prefix();
        let initiator_prefix = [offer.as_slice(), proof.as_slice()];
        let initiator_request = cryptex_events::verify_bootstrap_approval_request(
            &initiator_prefix,
            cryptex_events::BootstrapApprovalRole::Initiator,
        )
        .unwrap();
        let initiator_approval =
            cryptex_events::sign_bootstrap_approval(&initiator_request, &local_key).unwrap();
        let messages = vec![offer, proof, initiator_approval];
        let refs = messages.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let request = cryptex_events::verify_bootstrap_approval_request(
            &refs,
            cryptex_events::BootstrapApprovalRole::Responder,
        )
        .unwrap();
        assert_eq!(
            request.local_identity().public_key,
            peer_key.verifying_key().to_bytes()
        );
        let snapshot = ApprovalSnapshot::from_verified_bootstrap_request(&request).unwrap();
        let mut transaction = PendingBootstrapTransaction::new(
            snapshot,
            "44444444-4444-4444-8444-444444444444".into(),
        )
        .unwrap();

        transaction
            .record_verified_approval_prefix(&request, &messages)
            .unwrap();
        assert_eq!(transaction.state, BootstrapState::PeerApprovalReceived);
        assert_eq!(transaction.transcript_messages.len(), 3);
        transaction.validate().unwrap();
    }

    #[test]
    fn journal_persists_local_approval_before_releasing_signed_message() {
        let (offer, proof, local_key, peer_key) = signed_prefix();
        let prefix = [offer.as_slice(), proof.as_slice()];
        let request = cryptex_events::verify_bootstrap_approval_request(
            &prefix,
            cryptex_events::BootstrapApprovalRole::Initiator,
        )
        .unwrap();
        let messages = vec![offer.clone(), proof.clone()];
        let root = crate::test_temp::exclusive_temp_dir("m8-approval-signature");
        let journal = BootstrapJournalStore::new(&root).unwrap();
        journal
            .begin_verified_approval_prefix(
                &request,
                &messages,
                "55555555-5555-4555-8555-555555555555",
            )
            .unwrap();

        let registry = ApprovalSessionRegistry::default();
        let connection = registry.open_connection().unwrap();
        let grant = registry
            .issue(
                &connection,
                ApprovalSnapshot::from_verified_bootstrap_request(&request).unwrap(),
            )
            .unwrap();
        let approval = registry
            .confirm_bootstrap_approval_journaled(
                &connection,
                &grant.capability,
                &grant.approval_request_digest,
                &request,
                &journal,
                |verified| {
                    cryptex_events::sign_bootstrap_approval(verified, &local_key)
                        .map_err(|_| ApprovalSessionError::ApprovalSigningFailed)
                },
            )
            .unwrap();
        journal
            .record_local_approval_signature(&request, &approval)
            .unwrap();
        let persisted = journal.load().unwrap();
        assert_eq!(
            persisted.transactions[0].state,
            BootstrapState::LocalApprovalSigned
        );
        assert!(persisted.transactions[0].local_approval_signature_generated);
        assert!(persisted.transactions[0].local_approval_signature_released);
        assert_eq!(persisted.transactions[0].transcript_messages.len(), 3);
        assert_eq!(persisted.transactions[0].transcript_hash, None);
        journal
            .begin_verified_approval_prefix(
                &request,
                &messages,
                "77777777-7777-4777-8777-777777777777",
            )
            .unwrap();
        journal
            .record_local_approval_signature(&request, &approval)
            .unwrap();
        let mut changed = approval;
        changed.push(b' ');
        assert!(matches!(
            journal.record_local_approval_signature(&request, &changed),
            Err(BootstrapJournalError::Conflict(_))
        ));

        let initiator_request = request;
        let initiator_approval = persisted.transactions[0]
            .transcript_messages
            .last()
            .unwrap()
            .as_bytes()
            .to_vec();
        let responder_prefix = [
            offer.as_slice(),
            proof.as_slice(),
            initiator_approval.as_slice(),
        ];
        let responder_request = cryptex_events::verify_bootstrap_approval_request(
            &responder_prefix,
            cryptex_events::BootstrapApprovalRole::Responder,
        )
        .unwrap();
        assert_ne!(initiator_request.role(), responder_request.role());
        let responder_root = crate::test_temp::exclusive_temp_dir("m8-responder-signature");
        let responder_journal = BootstrapJournalStore::new(&responder_root).unwrap();
        let responder_snapshot =
            ApprovalSnapshot::from_verified_bootstrap_request(&responder_request).unwrap();
        let responder_transaction = PendingBootstrapTransaction::new(
            responder_snapshot,
            "66666666-6666-4666-8666-666666666666".into(),
        )
        .unwrap();
        responder_journal
            .update(|stored| stored.insert(responder_transaction))
            .unwrap();
        responder_journal
            .record_verified_approval_prefix(
                &responder_request,
                &responder_prefix.map(<[u8]>::to_vec),
            )
            .unwrap();
        let responder_approval =
            cryptex_events::sign_bootstrap_approval(&responder_request, &peer_key).unwrap();
        responder_journal
            .record_local_approval_signature(&responder_request, &responder_approval)
            .unwrap();
        let responder_saved = responder_journal.load().unwrap();
        assert_eq!(
            responder_saved.transactions[0].state,
            BootstrapState::TranscriptReady
        );
        assert!(responder_saved.transactions[0].transcript_hash.is_some());
        cryptex_events::verify_bootstrap_transcript(
            &responder_saved.transactions[0]
                .transcript_messages
                .iter()
                .map(String::as_bytes)
                .collect::<Vec<_>>(),
        )
        .unwrap();

        let complete_transcript = vec![offer, proof, initiator_approval, responder_approval];
        journal
            .record_verified_complete_transcript(TX, &complete_transcript)
            .unwrap();
        let complete_saved = journal.load().unwrap();
        assert_eq!(
            complete_saved.transactions[0].state,
            BootstrapState::TranscriptReady
        );
        assert_eq!(complete_saved.transactions[0].transcript_messages.len(), 4);
        assert!(complete_saved.transactions[0].transcript_hash.is_some());

        journal
            .record_append_attempt(
                TX,
                crate::bootstrap_journal::AppendAttempt {
                    reserved_sequence: 9,
                    predecessor_hash: "77".repeat(32),
                    candidate_content_hash: Some("88".repeat(32)),
                },
            )
            .unwrap();
        let append_pending = journal.load().unwrap();
        assert_eq!(
            append_pending.transactions[0].state,
            BootstrapState::AppendUnknown
        );
        assert_eq!(append_pending.transactions[0].append_attempts.len(), 1);
        assert!(matches!(
            journal.record_verified_anchor(TX, 9, &"99".repeat(32)),
            Err(BootstrapJournalError::Conflict(_))
        ));
        assert_eq!(
            journal.load().unwrap().transactions[0].state,
            BootstrapState::AppendUnknown
        );
        std::fs::remove_dir_all(root).unwrap();
        std::fs::remove_dir_all(responder_root).unwrap();
    }

    #[test]
    fn local_peer_approval_anchor_appends_and_replays_idempotently() {
        use cryptex_core::BrainIdentity;
        use cryptex_events::Classification;

        use crate::canonical_writer::{
            AppendMemoryNoteArgs, CanonicalWriterConfig, append_memory_note,
        };
        use crate::identity_runtime::write_brain_identity;
        use crate::local_device_identity::enroll_local_device;
        use crate::secure_store::{FakeSecureStore, SecureStoreEnvironment};

        let root = crate::test_temp::exclusive_temp_dir("m8-peer-anchor-append");
        let store = FakeSecureStore::new();
        let brain = BrainIdentity::new(BRAIN, "2026-10-29T00:00:00Z").unwrap();
        write_brain_identity(&root, &brain).unwrap();
        let enrollment = enroll_local_device(&root, &store, SecureStoreEnvironment::Dev).unwrap();
        let local_device = enrollment.device_id;
        let local_key = crate::event_signing_key::load_existing_event_signing_key(
            &store,
            SecureStoreEnvironment::Dev,
            &root,
            &cryptex_core::DeviceId::new(local_device.clone()).unwrap(),
        )
        .unwrap();
        let peer_key = SigningKey::from_bytes(&[0x62; 32]);
        let (offer, proof) = signed_prefix_for(
            &local_key.signing_key,
            &peer_key,
            &local_device,
            "peer-device-m8-test",
            BRAIN,
            TX,
            LOCAL_TRANSPORT,
            PEER_TRANSPORT,
        );
        let prefix = [offer.as_slice(), proof.as_slice()];
        let request = cryptex_events::verify_bootstrap_approval_request(
            &prefix,
            cryptex_events::BootstrapApprovalRole::Initiator,
        )
        .unwrap();
        let first_messages = vec![offer.clone(), proof.clone()];
        let journal = BootstrapJournalStore::new(&root).unwrap();
        journal
            .begin_verified_approval_prefix(
                &request,
                &first_messages,
                "77777777-7777-4777-8777-777777777777",
            )
            .unwrap();
        let local_approval =
            cryptex_events::sign_bootstrap_approval(&request, &local_key.signing_key).unwrap();
        journal
            .record_local_approval_signature(&request, &local_approval)
            .unwrap();
        let responder_prefix = [
            offer.as_slice(),
            proof.as_slice(),
            local_approval.as_slice(),
        ];
        let responder_request = cryptex_events::verify_bootstrap_approval_request(
            &responder_prefix,
            cryptex_events::BootstrapApprovalRole::Responder,
        )
        .unwrap();
        let responder_approval =
            cryptex_events::sign_bootstrap_approval(&responder_request, &peer_key).unwrap();
        let complete_transcript = vec![offer, proof, local_approval, responder_approval];
        journal
            .record_verified_complete_transcript(TX, &complete_transcript)
            .unwrap();

        let local_device = cryptex_core::DeviceId::new(local_device).unwrap();
        let prior_note = append_memory_note(
            &store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: BRAIN,
                device_id: &local_device,
                classification: Classification::Internal,
                text: "synthetic canonical prefix for M8 anchor test",
                event_id: None,
                config: &CanonicalWriterConfig::default(),
            },
        )
        .unwrap();
        let abandoned_sequence =
            crate::sequence_runtime::allocate_device_sequence(&root, &local_device).unwrap();
        assert_eq!(abandoned_sequence, 2);
        journal
            .record_append_attempt(
                TX,
                crate::bootstrap_journal::AppendAttempt {
                    reserved_sequence: abandoned_sequence,
                    predecessor_hash: prior_note.accepted.content_hash.as_str().to_owned(),
                    candidate_content_hash: Some("77".repeat(32)),
                },
            )
            .unwrap();
        assert!(
            crate::knowledge_materialization::reconcile_peer_approval_anchors_before_startup(
                &root,
                &store,
                SecureStoreEnvironment::Dev,
            )
            .is_err()
        );
        assert_eq!(
            journal.load().unwrap().transactions[0].state,
            BootstrapState::AppendUnknown
        );

        let first = crate::knowledge_materialization::append_local_peer_approval_anchor(
            &root,
            BRAIN,
            &store,
            SecureStoreEnvironment::Dev,
            TX,
            &complete_transcript,
        )
        .unwrap();
        assert_eq!(first.device_seq, 3);
        assert_eq!(
            journal.load().unwrap().transactions[0].state,
            BootstrapState::Anchored
        );

        let replay = crate::knowledge_materialization::append_local_peer_approval_anchor(
            &root,
            BRAIN,
            &store,
            SecureStoreEnvironment::Dev,
            TX,
            &complete_transcript,
        )
        .unwrap();
        assert_eq!(replay, first);
        crate::knowledge_materialization::reconcile_peer_approval_anchors_before_startup(
            &root,
            &store,
            SecureStoreEnvironment::Dev,
        )
        .unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn owner_confirmation_consumes_matching_snapshot_before_bootstrap_signature() {
        let (offer, proof, local_key, peer_key) = signed_prefix();
        let prefix = [offer.as_slice(), proof.as_slice()];
        let request = cryptex_events::verify_bootstrap_approval_request(
            &prefix,
            cryptex_events::BootstrapApprovalRole::Initiator,
        )
        .unwrap();
        let snapshot = ApprovalSnapshot::from_verified_bootstrap_request(&request).unwrap();
        assert!(snapshot.matches_bootstrap_request(&request));

        let registry = ApprovalSessionRegistry::default();
        let connection = registry.open_connection().unwrap();
        let grant = registry.issue(&connection, snapshot).unwrap();
        let initiator_approval = registry
            .confirm_bootstrap_approval(
                &connection,
                &grant.capability,
                &grant.approval_request_digest,
                &request,
                &local_key,
            )
            .unwrap();
        assert!(matches!(
            registry.confirm_bootstrap_approval(
                &connection,
                &grant.capability,
                &grant.approval_request_digest,
                &request,
                &local_key
            ),
            Err(ApprovalSessionError::UnknownSession)
        ));

        let responder_prefix = [
            offer.as_slice(),
            proof.as_slice(),
            initiator_approval.as_slice(),
        ];
        let responder_request = cryptex_events::verify_bootstrap_approval_request(
            &responder_prefix,
            cryptex_events::BootstrapApprovalRole::Responder,
        )
        .unwrap();
        let responder_approval =
            cryptex_events::sign_bootstrap_approval(&responder_request, &peer_key).unwrap();
        let transcript = [
            offer.as_slice(),
            proof.as_slice(),
            initiator_approval.as_slice(),
            responder_approval.as_slice(),
        ];
        cryptex_events::verify_bootstrap_transcript(&transcript).unwrap();
    }

    #[test]
    fn signer_callback_runs_only_after_matching_capability_is_consumed() {
        let (offer, proof, _, _) = signed_prefix();
        let prefix = [offer.as_slice(), proof.as_slice()];
        let request = cryptex_events::verify_bootstrap_approval_request(
            &prefix,
            cryptex_events::BootstrapApprovalRole::Initiator,
        )
        .unwrap();
        let registry = ApprovalSessionRegistry::default();
        let connection = registry.open_connection().unwrap();
        let snapshot = ApprovalSnapshot::from_verified_bootstrap_request(&request).unwrap();
        let grant = registry.issue(&connection, snapshot).unwrap();
        let invoked = std::cell::Cell::new(false);

        let result = registry
            .confirm_bootstrap_approval_with_signer(
                &connection,
                &grant.capability,
                &grant.approval_request_digest,
                &request,
                |_| {
                    invoked.set(true);
                    Ok(b"signed approval".to_vec())
                },
            )
            .unwrap();
        assert!(invoked.get());
        assert_eq!(result, b"signed approval");

        invoked.set(false);
        assert!(matches!(
            registry.confirm_bootstrap_approval_with_signer(
                &connection,
                &grant.capability,
                &grant.approval_request_digest,
                &request,
                |_| {
                    invoked.set(true);
                    Ok(Vec::new())
                }
            ),
            Err(ApprovalSessionError::UnknownSession)
        ));
        assert!(!invoked.get());
    }

    #[test]
    fn changed_snapshot_or_wrong_key_burns_confirmation_capability() {
        let (offer, proof, local_key, peer_key) = signed_prefix();
        let prefix = [offer.as_slice(), proof.as_slice()];
        let request = cryptex_events::verify_bootstrap_approval_request(
            &prefix,
            cryptex_events::BootstrapApprovalRole::Initiator,
        )
        .unwrap();
        let registry = ApprovalSessionRegistry::default();
        let connection = registry.open_connection().unwrap();

        let mut changed = ApprovalSnapshot::from_verified_bootstrap_request(&request).unwrap();
        changed
            .transport_association
            .as_mut()
            .unwrap()
            .peer_device_id = PEER_TRANSPORT.replace('B', "C");
        let changed_grant = registry.issue(&connection, changed).unwrap();
        assert!(matches!(
            registry.confirm_bootstrap_approval(
                &connection,
                &changed_grant.capability,
                &changed_grant.approval_request_digest,
                &request,
                &local_key
            ),
            Err(ApprovalSessionError::SnapshotMismatch)
        ));

        let grant = registry
            .issue(
                &connection,
                ApprovalSnapshot::from_verified_bootstrap_request(&request).unwrap(),
            )
            .unwrap();
        assert!(matches!(
            registry.confirm_bootstrap_approval(
                &connection,
                &grant.capability,
                &grant.approval_request_digest,
                &request,
                &peer_key
            ),
            Err(ApprovalSessionError::ApprovalSigningFailed)
        ));
        assert!(matches!(
            registry.confirm_bootstrap_approval(
                &connection,
                &grant.capability,
                &grant.approval_request_digest,
                &request,
                &local_key
            ),
            Err(ApprovalSessionError::UnknownSession)
        ));
    }

    #[test]
    fn mismatch_and_cancel_do_not_authorize_or_restore_capability() {
        let registry = ApprovalSessionRegistry::default();
        let connection = registry.open_connection().unwrap();
        let grant = registry.issue(&connection, snapshot()).unwrap();

        assert!(matches!(
            registry.consume(&connection, &grant.capability, &"0".repeat(64)),
            Err(ApprovalSessionError::SnapshotMismatch)
        ));
        registry.cancel(&connection, &grant.capability).unwrap();
        assert!(matches!(
            registry.consume(
                &connection,
                &grant.capability,
                &grant.approval_request_digest
            ),
            Err(ApprovalSessionError::UnknownSession)
        ));
    }

    #[test]
    fn concurrent_confirmations_consume_at_most_once() {
        let registry = std::sync::Arc::new(ApprovalSessionRegistry::default());
        let connection = std::sync::Arc::new(registry.open_connection().unwrap());
        let grant = registry.issue(&connection, snapshot()).unwrap();
        let mut workers = Vec::new();
        for _ in 0..2 {
            let registry = std::sync::Arc::clone(&registry);
            let connection = std::sync::Arc::clone(&connection);
            let capability = grant.capability.clone();
            let digest = grant.approval_request_digest.clone();
            workers.push(std::thread::spawn(move || {
                registry.consume(&connection, &capability, &digest).is_ok()
            }));
        }
        let successes = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .filter(|success| *success)
            .count();
        assert_eq!(successes, 1);
    }

    #[test]
    fn disconnect_invalidates_only_that_connections_sessions() {
        let registry = ApprovalSessionRegistry::default();
        let first = registry.open_connection().unwrap();
        let second = registry.open_connection().unwrap();
        let first_grant = registry.issue(&first, snapshot()).unwrap();
        let second_grant = registry.issue(&second, snapshot()).unwrap();

        drop(first);
        assert!(matches!(
            registry.consume(
                &second,
                &first_grant.capability,
                &first_grant.approval_request_digest
            ),
            Err(ApprovalSessionError::UnknownSession)
        ));
        assert!(
            registry
                .consume(
                    &second,
                    &second_grant.capability,
                    &second_grant.approval_request_digest
                )
                .is_ok()
        );
    }

    #[test]
    fn capability_wire_format_is_strict_and_debug_redacts_it() {
        assert_eq!(decode_hex(&"a5".repeat(32)).unwrap(), [0xa5; 32]);
        assert!(matches!(
            decode_hex(&"A5".repeat(32)),
            Err(ApprovalSessionError::InvalidCapability)
        ));
        assert!(matches!(
            decode_hex("short"),
            Err(ApprovalSessionError::InvalidCapability)
        ));

        let registry = ApprovalSessionRegistry::default();
        let connection = registry.open_connection().unwrap();
        let grant = registry.issue(&connection, snapshot()).unwrap();
        assert!(!format!("{grant:?}").contains(&grant.capability));
    }
}
