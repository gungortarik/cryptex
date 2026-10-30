//! Order-independent verification of an explicit multi-device security history.
//!
//! The caller must supply an exhaustive snapshot of the canonical records it
//! intends to evaluate and a key resolver backed by the local trust policy.
//! This module authenticates supported records and verifies each device chain;
//! it cannot prove that the input snapshot itself is exhaustive or authorize
//! retrieval/publication.

use std::collections::{BTreeMap, HashMap};

use serde_json::Value;

use cryptex_core::ids::DeviceId;
use cryptex_core::validate::parse_uuid_str;

use crate::envelope::AcceptedEvent;
use crate::hex::{ContentHashHex, SigningKeyId};
use crate::security_v3::SecurityEventV3;
use crate::validate_record::{ValidationFailure, ValidationReasonCode};
use crate::versioned_reader::{
    TrustedDaemonKeyResolver, VerifiedUnsupportedV2, VerifiedV2History,
    VerifiedV2KnowledgeAssertion, VersionedChainVerifier, VersionedRecord, read_versioned_record,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CancellableSecurityHistoryError {
    InvalidRecord(ValidationFailure),
    Cancelled,
}

impl From<ValidationFailure> for CancellableSecurityHistoryError {
    fn from(error: ValidationFailure) -> Self {
        Self::InvalidRecord(error)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AuthenticatedSecurityRecord {
    V1(AcceptedEvent),
    V2(VerifiedV2KnowledgeAssertion),
    SecurityV3(SecurityEventV3),
}

impl AuthenticatedSecurityRecord {
    pub fn event_id(&self) -> &str {
        match self {
            Self::V1(event) => &event.event_id,
            Self::V2(event) => &event.event().event_id,
            Self::SecurityV3(event) => event.event_id(),
        }
    }

    pub fn brain_id(&self) -> &str {
        match self {
            Self::V1(event) => &event.brain_id,
            Self::V2(event) => &event.event().brain_id,
            Self::SecurityV3(event) => event.brain_id(),
        }
    }

    pub fn device_id(&self) -> &str {
        match self {
            Self::V1(event) => &event.device_id,
            Self::V2(event) => &event.event().device_id,
            Self::SecurityV3(event) => event.device_id().as_str(),
        }
    }

    pub fn device_seq(&self) -> u64 {
        match self {
            Self::V1(event) => event.device_seq.to_u64().expect("validated v1 sequence"),
            Self::V2(event) => event
                .event()
                .device_seq
                .to_u64()
                .expect("validated v2 sequence"),
            Self::SecurityV3(event) => event.device_seq().to_u64().expect("validated v3 sequence"),
        }
    }

    pub fn content_hash(&self) -> &ContentHashHex {
        match self {
            Self::V1(event) => &event.content_hash,
            Self::V2(event) => &event.event().content_hash,
            Self::SecurityV3(event) => event.content_hash(),
        }
    }

    pub fn signing_key_id(&self) -> &SigningKeyId {
        match self {
            Self::V1(event) => &event.signing_key_id,
            Self::V2(event) => &event.event().signing_key_id,
            Self::SecurityV3(event) => event.signing_key_id(),
        }
    }
}

/// Fully authenticated supported records, sorted by device ID bytes then
/// sequence. This type proves per-device integrity for the supplied set only;
/// source-level completeness remains a caller responsibility.
#[derive(Debug, Clone, PartialEq)]
pub struct VerifiedSecurityHistory {
    records: Vec<AuthenticatedSecurityRecord>,
    expected_brain_id: String,
}

impl VerifiedSecurityHistory {
    #[must_use]
    pub fn records(&self) -> &[AuthenticatedSecurityRecord] {
        &self.records
    }

    #[must_use]
    pub fn expected_brain_id(&self) -> &str {
        &self.expected_brain_id
    }

    /// Select authenticated v1/v2 content records at or before each device's
    /// effective cutoff. Security v3 records are policy metadata and are never
    /// returned as content. An absent device cutoff excludes that device.
    ///
    /// This is a filtering helper only: callers must obtain the cutoff map from
    /// the local trust reducer and must keep the complete verified history's
    /// lock scope alive through any content read.
    pub fn content_records_through_cutoffs<'a>(
        &'a self,
        effective_cutoffs: &'a BTreeMap<String, u64>,
    ) -> impl Iterator<Item = &'a AuthenticatedSecurityRecord> + 'a {
        self.records.iter().filter(move |record| {
            if matches!(record, AuthenticatedSecurityRecord::SecurityV3(_)) {
                return false;
            }
            effective_cutoffs
                .get(record.device_id())
                .is_some_and(|cutoff| record.device_seq() <= *cutoff)
        })
    }

    /// Build the legacy content-history view from records admitted by the
    /// caller's effective per-device cutoffs. Security policy records are
    /// excluded. The cutoff map must come from local-anchor trust reduction;
    /// this conversion does not establish trust or snapshot completeness.
    #[must_use]
    pub fn content_history_through_cutoffs(
        &self,
        effective_cutoffs: &BTreeMap<String, u64>,
    ) -> VerifiedV2History {
        self.content_history_through_cutoffs_cancellable(effective_cutoffs, || false)
            .expect("non-cancellable content-history projection cannot be cancelled")
    }

    /// Cancellable variant for bounded retrieval/materialization callers.
    /// Cancellation returns no partial content history.
    pub fn content_history_through_cutoffs_cancellable<C>(
        &self,
        effective_cutoffs: &BTreeMap<String, u64>,
        mut cancelled: C,
    ) -> Result<VerifiedV2History, CancellableSecurityHistoryError>
    where
        C: FnMut() -> bool,
    {
        let mut records = Vec::new();
        let mut assertions = Vec::new();
        for record in self.content_records_through_cutoffs(effective_cutoffs) {
            if cancelled() {
                return Err(CancellableSecurityHistoryError::Cancelled);
            }
            match record {
                AuthenticatedSecurityRecord::V1(event) => {
                    records.push(VersionedRecord::VerifiedV1(event.clone()));
                }
                AuthenticatedSecurityRecord::V2(assertion) => {
                    records.push(VersionedRecord::VerifiedV2Supported(assertion.clone()));
                    assertions.push(assertion.clone());
                }
                AuthenticatedSecurityRecord::SecurityV3(_) => {
                    unreachable!("content_records_through_cutoffs excludes policy records")
                }
            }
        }
        if cancelled() {
            return Err(CancellableSecurityHistoryError::Cancelled);
        }
        Ok(VerifiedV2History::from_authenticated_content(
            records, assertions,
        ))
    }
}

/// Verify all supported records in one supplied Brain snapshot.
///
/// Exact duplicate record bytes are deduplicated. Conflicting bytes for one
/// event ID, duplicate sequence positions, unknown event semantics, unknown
/// schema versions, unresolved keys, or any broken v1/v2/v3 chain reject the
/// entire result. Sequence gaps are permitted because durable reservations
/// may be consumed without an appended event.
pub fn verify_security_history<R: TrustedDaemonKeyResolver>(
    raw_records: &[&[u8]],
    expected_brain_id: &str,
    key_resolver: R,
) -> Result<VerifiedSecurityHistory, ValidationFailure> {
    match verify_security_history_cancellable(raw_records, expected_brain_id, key_resolver, || {
        false
    }) {
        Ok(history) => Ok(history),
        Err(CancellableSecurityHistoryError::InvalidRecord(error)) => Err(error),
        Err(CancellableSecurityHistoryError::Cancelled) => {
            unreachable!("the non-cancellable security-history verifier never cancels")
        }
    }
}

/// Verify a supplied security-history snapshot while allowing bounded callers
/// to stop between records and chain checks. Cancellation never returns a
/// partial verified history.
pub fn verify_security_history_cancellable<R, C>(
    raw_records: &[&[u8]],
    expected_brain_id: &str,
    mut key_resolver: R,
    mut cancelled: C,
) -> Result<VerifiedSecurityHistory, CancellableSecurityHistoryError>
where
    R: TrustedDaemonKeyResolver,
    C: FnMut() -> bool,
{
    let expected_brain_id = parse_uuid_str(expected_brain_id)
        .map_err(|_| {
            failure(
                ValidationReasonCode::InvalidRecord,
                "expected Brain ID is invalid",
            )
        })?
        .to_ascii_lowercase();
    let mut seen_event_bytes = HashMap::<String, Vec<u8>>::new();
    let mut records = Vec::<AuthenticatedSecurityRecord>::with_capacity(raw_records.len());

    for bytes in raw_records {
        if cancelled() {
            return Err(CancellableSecurityHistoryError::Cancelled);
        }
        let value: Value = serde_json::from_slice(bytes).map_err(|_| {
            failure(
                ValidationReasonCode::InvalidRecord,
                "record JSON is malformed",
            )
        })?;
        let event_id = value
            .get("event_id")
            .and_then(Value::as_str)
            .and_then(|id| parse_uuid_str(id).ok())
            .map(|id| id.to_ascii_lowercase())
            .ok_or_else(|| {
                failure(
                    ValidationReasonCode::InvalidRecord,
                    "record event_id is invalid",
                )
            })?;
        if let Some(previous) = seen_event_bytes.get(&event_id) {
            if previous.as_slice() == *bytes {
                continue;
            }
            return Err(failure(
                ValidationReasonCode::IntegrityFailure,
                "different record bytes share one event_id",
            )
            .into());
        }
        let device_id = value
            .get("device_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                failure(
                    ValidationReasonCode::InvalidRecord,
                    "record device_id is unavailable",
                )
            })?;
        let key_id = value
            .get("signing_key_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                failure(
                    ValidationReasonCode::InvalidRecord,
                    "record signing_key_id is unavailable",
                )
            })?;
        let verifying_key = key_resolver
            .trusted_daemon_key(device_id, key_id)
            .ok_or_else(|| {
                failure(
                    ValidationReasonCode::IntegrityFailure,
                    "trusted record key is unavailable",
                )
            })?;
        let version = value.get("schema_version").and_then(Value::as_u64);
        let authenticated = if version == Some(3) {
            AuthenticatedSecurityRecord::SecurityV3(crate::security_v3::read_security_v3_event(
                bytes,
                &verifying_key,
            )?)
        } else {
            match read_versioned_record(bytes, &verifying_key)? {
                VersionedRecord::VerifiedV1(event) => AuthenticatedSecurityRecord::V1(event),
                VersionedRecord::VerifiedV2Supported(event) => {
                    AuthenticatedSecurityRecord::V2(event)
                }
                VersionedRecord::VerifiedUnsupported(VerifiedUnsupportedV2 { .. }) => {
                    return Err(failure(
                        ValidationReasonCode::InvalidRecord,
                        "unsupported v2 event semantics block security-history verification",
                    )
                    .into());
                }
                VersionedRecord::VerifiedSecurityV3(event) => {
                    AuthenticatedSecurityRecord::SecurityV3(event)
                }
                VersionedRecord::Unverifiable(_) => {
                    return Err(failure(
                        ValidationReasonCode::InvalidRecord,
                        "unverifiable record blocks security-history verification",
                    )
                    .into());
                }
            }
        };
        if authenticated.device_id() != device_id
            || !authenticated
                .brain_id()
                .eq_ignore_ascii_case(&expected_brain_id)
        {
            return Err(failure(
                ValidationReasonCode::IntegrityFailure,
                "verified record identity does not match its selected Brain/device",
            )
            .into());
        }
        seen_event_bytes.insert(event_id, bytes.to_vec());
        records.push(authenticated);
    }

    records.sort_by(|left, right| {
        left.device_id()
            .as_bytes()
            .cmp(right.device_id().as_bytes())
            .then_with(|| left.device_seq().cmp(&right.device_seq()))
            .then_with(|| left.event_id().as_bytes().cmp(right.event_id().as_bytes()))
    });

    let mut chain_verifiers = BTreeMap::<DeviceId, VersionedChainVerifier>::new();
    for record in &records {
        if cancelled() {
            return Err(CancellableSecurityHistoryError::Cancelled);
        }
        let device = DeviceId::new(record.device_id().to_owned()).map_err(|_| {
            failure(
                ValidationReasonCode::InvalidRecord,
                "verified device_id is invalid",
            )
        })?;
        let verifier = chain_verifiers
            .entry(device.clone())
            .or_insert_with(|| VersionedChainVerifier::new(device.as_str()));
        match record {
            AuthenticatedSecurityRecord::SecurityV3(event) => {
                verifier.verify_security_v3_next(event)?;
            }
            legacy => {
                let dispatched = match legacy {
                    AuthenticatedSecurityRecord::V1(event) => {
                        VersionedRecord::VerifiedV1(event.clone())
                    }
                    AuthenticatedSecurityRecord::V2(event) => {
                        VersionedRecord::VerifiedV2Supported(event.clone())
                    }
                    AuthenticatedSecurityRecord::SecurityV3(_) => unreachable!(),
                };
                verifier.verify_next(&dispatched)?;
            }
        }
    }

    Ok(VerifiedSecurityHistory {
        records,
        expected_brain_id,
    })
}

fn failure(reason: ValidationReasonCode, detail: &'static str) -> ValidationFailure {
    ValidationFailure {
        reason,
        detail: detail.to_owned(),
        schema_version_if_parsed: None,
    }
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
    use serde_json::Value;

    use super::*;
    use crate::device_seq::DeviceSeq;
    use crate::envelope::{
        Classification, EventPayload, EventType, MemoryNotePayload, SignableEvent,
    };
    use crate::hex::{ContentHashHex, SignatureHex, ZERO_CONTENT_HASH_HEX};
    use crate::integrity::{accept_event, signing_key_id_from_verifying_key};
    use crate::security_v3::{SECURITY_V3_CONTENT_HASH_DOMAIN, SECURITY_V3_SIGNATURE_DOMAIN};
    use crate::versioned_reader::{V2_CONTENT_HASH_DOMAIN, V2_SIGNATURE_DOMAIN};

    const BRAIN: &str = "22222222-2222-4222-8222-222222222222";
    const DEVICE: &str = "device-history-test";
    const SEED: [u8; 32] = [0x51; 32];

    struct Resolver(VerifyingKey);

    impl TrustedDaemonKeyResolver for Resolver {
        fn trusted_daemon_key(&mut self, device_id: &str, key_id: &str) -> Option<VerifyingKey> {
            let expected_key_id = signing_key_id_from_verifying_key(&self.0).ok()?;
            (device_id == DEVICE && key_id == expected_key_id.as_str()).then_some(self.0)
        }
    }

    fn make_v1(seq: u64, prev_hash: &str, event_id: &str, key: &SigningKey) -> AcceptedEvent {
        accept_event(
            SignableEvent {
                schema_version: 1,
                event_id: event_id.into(),
                brain_id: BRAIN.into(),
                device_id: DEVICE.into(),
                device_seq: DeviceSeq::from_u64(seq).unwrap(),
                created_at: "2026-10-29T00:00:00Z".into(),
                event_type: EventType::MemoryNote,
                classification: Classification::Internal,
                signing_key_id: signing_key_id_from_verifying_key(&key.verifying_key()).unwrap(),
                payload: EventPayload::MemoryNote(MemoryNotePayload::new("v1 fixture").unwrap()),
                prev_content_hash: ContentHashHex::parse(prev_hash).unwrap(),
            },
            key,
        )
        .unwrap()
    }

    fn make_v2(seq: u64, prev_hash: &str, event_id: &str, key: &SigningKey) -> Vec<u8> {
        let key_id = signing_key_id_from_verifying_key(&key.verifying_key()).unwrap();
        let mut record = serde_json::json!({
            "schema_version": 2,
            "event_id": event_id,
            "brain_id": BRAIN,
            "device_id": DEVICE,
            "device_seq": seq.to_string(),
            "created_at": "2026-10-29T00:00:01Z",
            "event_type": "knowledge.assertion",
            "payload_schema_version": 1,
            "classification": "internal",
            "signing_key_id": key_id.as_str(),
            "payload": {
                "kind": "entity.declare",
                "entity_id": "33333333-3333-4333-8333-333333333333",
                "entity_kind": "person",
                "label": "Synthetic",
                "publication_authorization": {
                    "basis": "direct_user",
                    "receipt_id": "44444444-4444-4444-8444-444444444444"
                }
            },
            "prev_content_hash": prev_hash,
            "content_hash": "00".repeat(32),
            "signature": "00".repeat(64)
        });
        record.as_object_mut().unwrap().remove("content_hash");
        record.as_object_mut().unwrap().remove("signature");
        let signable = serde_json_canonicalizer::to_vec(&record).unwrap();
        let mut hasher = blake3::Hasher::new();
        hasher.update(V2_CONTENT_HASH_DOMAIN);
        hasher.update(&[0]);
        hasher.update(&signable);
        let hash = *hasher.finalize().as_bytes();
        let signature = key.sign(&[V2_SIGNATURE_DOMAIN, &[0], &hash].concat());
        let object = record.as_object_mut().unwrap();
        object.insert(
            "content_hash".into(),
            Value::String(ContentHashHex::from_bytes(&hash).unwrap().as_str().into()),
        );
        object.insert(
            "signature".into(),
            Value::String(
                SignatureHex::from_bytes(&signature.to_bytes())
                    .unwrap()
                    .as_str()
                    .into(),
            ),
        );
        serde_json_canonicalizer::to_vec(&record).unwrap()
    }

    fn make_v3(seq: u64, prev_hash: &str, key: &SigningKey) -> Vec<u8> {
        let key_id = signing_key_id_from_verifying_key(&key.verifying_key()).unwrap();
        let mut record = serde_json::json!({
            "schema_version": 3,
            "event_id": "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
            "brain_id": BRAIN,
            "device_id": DEVICE,
            "device_seq": seq.to_string(),
            "created_at": "2026-10-29T00:00:02Z",
            "event_type": "security.device_self_retired",
            "classification": "internal",
            "signing_key_id": key_id.as_str(),
            "payload": {
                "transaction_id": "ab".repeat(32),
                "last_authorized_seq": "3",
                "last_authorized_content_hash": prev_hash
            },
            "prev_content_hash": prev_hash,
            "content_hash": "00".repeat(32),
            "signature": "00".repeat(64)
        });
        record.as_object_mut().unwrap().remove("content_hash");
        record.as_object_mut().unwrap().remove("signature");
        let signable = serde_json_canonicalizer::to_vec(&record).unwrap();
        let mut hasher = blake3::Hasher::new();
        hasher.update(SECURITY_V3_CONTENT_HASH_DOMAIN);
        hasher.update(&[0]);
        hasher.update(&signable);
        let hash = *hasher.finalize().as_bytes();
        let signature = key.sign(&[SECURITY_V3_SIGNATURE_DOMAIN, &[0], &hash].concat());
        let object = record.as_object_mut().unwrap();
        object.insert(
            "content_hash".into(),
            Value::String(ContentHashHex::from_bytes(&hash).unwrap().as_str().into()),
        );
        object.insert(
            "signature".into(),
            Value::String(
                SignatureHex::from_bytes(&signature.to_bytes())
                    .unwrap()
                    .as_str()
                    .into(),
            ),
        );
        serde_json_canonicalizer::to_vec(&record).unwrap()
    }

    #[test]
    fn sorts_arrival_order_and_verifies_v1_v2_v3_chain() {
        let key = SigningKey::from_bytes(&SEED);
        let v1 = make_v1(
            1,
            ZERO_CONTENT_HASH_HEX,
            "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            &key,
        );
        let v1_hash = v1.content_hash.as_str();
        let v2 = make_v2(3, v1_hash, "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb", &key);
        let v2_hash = serde_json::from_slice::<Value>(&v2).unwrap()["content_hash"]
            .as_str()
            .unwrap()
            .to_owned();
        let v3 = make_v3(5, &v2_hash, &key);
        let v1_bytes = serde_json_canonicalizer::to_vec(&v1).unwrap();
        let inputs = [v3.as_slice(), v2.as_slice(), v1_bytes.as_slice()];
        let history =
            verify_security_history(&inputs, BRAIN, Resolver(key.verifying_key())).unwrap();
        assert_eq!(
            history
                .records()
                .iter()
                .map(AuthenticatedSecurityRecord::device_seq)
                .collect::<Vec<_>>(),
            vec![1, 3, 5]
        );
        assert_eq!(history.expected_brain_id(), BRAIN);
    }

    #[test]
    fn content_view_omits_security_records_and_applies_each_device_cutoff() {
        let key = SigningKey::from_bytes(&SEED);
        let v1 = make_v1(
            1,
            ZERO_CONTENT_HASH_HEX,
            "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            &key,
        );
        let v2 = make_v2(
            3,
            v1.content_hash.as_str(),
            "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
            &key,
        );
        let v2_hash = serde_json::from_slice::<Value>(&v2).unwrap()["content_hash"]
            .as_str()
            .unwrap()
            .to_owned();
        let v3 = make_v3(5, &v2_hash, &key);
        let v1_bytes = serde_json_canonicalizer::to_vec(&v1).unwrap();
        let history = verify_security_history(
            &[v1_bytes.as_slice(), v2.as_slice(), v3.as_slice()],
            BRAIN,
            Resolver(key.verifying_key()),
        )
        .unwrap();

        let mut cutoffs = BTreeMap::from([(DEVICE.to_owned(), 3)]);
        let content = history
            .content_records_through_cutoffs(&cutoffs)
            .map(AuthenticatedSecurityRecord::device_seq)
            .collect::<Vec<_>>();
        assert_eq!(content, vec![1, 3]);
        let reduced_view = history.content_history_through_cutoffs(&cutoffs);
        assert_eq!(reduced_view.records().len(), 2);
        assert!(matches!(
            reduced_view.records()[0],
            VersionedRecord::VerifiedV1(_)
        ));
        assert!(matches!(
            reduced_view.records()[1],
            VersionedRecord::VerifiedV2Supported(_)
        ));
        assert_eq!(reduced_view.assertions().len(), 1);

        cutoffs.insert(DEVICE.to_owned(), 1);
        let content = history
            .content_records_through_cutoffs(&cutoffs)
            .map(AuthenticatedSecurityRecord::device_seq)
            .collect::<Vec<_>>();
        assert_eq!(content, vec![1]);

        cutoffs.remove(DEVICE);
        assert_eq!(history.content_records_through_cutoffs(&cutoffs).count(), 0);
        assert!(
            history
                .content_history_through_cutoffs(&cutoffs)
                .records()
                .is_empty()
        );
        assert_eq!(
            history.content_history_through_cutoffs_cancellable(&cutoffs, || true),
            Err(CancellableSecurityHistoryError::Cancelled)
        );
    }

    #[test]
    fn rejects_competing_sequence_positions() {
        let key = SigningKey::from_bytes(&SEED);
        let first = make_v1(
            1,
            ZERO_CONTENT_HASH_HEX,
            "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            &key,
        );
        let fork = make_v1(
            1,
            ZERO_CONTENT_HASH_HEX,
            "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
            &key,
        );
        let first_bytes = serde_json_canonicalizer::to_vec(&first).unwrap();
        let fork_bytes = serde_json_canonicalizer::to_vec(&fork).unwrap();
        assert!(
            verify_security_history(
                &[first_bytes.as_slice(), fork_bytes.as_slice()],
                BRAIN,
                Resolver(key.verifying_key()),
            )
            .is_err()
        );
    }

    #[test]
    fn cancellation_never_returns_partial_history() {
        let key = SigningKey::from_bytes(&SEED);
        let event = make_v1(
            1,
            ZERO_CONTENT_HASH_HEX,
            "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            &key,
        );
        let bytes = serde_json_canonicalizer::to_vec(&event).unwrap();
        assert_eq!(
            verify_security_history_cancellable(
                &[bytes.as_slice()],
                BRAIN,
                Resolver(key.verifying_key()),
                || true,
            ),
            Err(CancellableSecurityHistoryError::Cancelled)
        );
    }
}
