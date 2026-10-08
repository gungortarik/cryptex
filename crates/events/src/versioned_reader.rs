//! Version-dispatched canonical record reader (ADR-0017 §4).
//!
//! This module is read-only. It does not expose event signing or production
//! writing. The v1 path delegates to the frozen ADR-0016 validator unchanged.

use ed25519_dalek::VerifyingKey;
use serde::Deserialize;
use serde_json::Value;

use cryptex_core::ids::DeviceId;
use cryptex_core::validate::{parse_rfc3339_str, parse_uuid_str};

use crate::device_seq::DeviceSeq;
use crate::envelope::{AcceptedEvent, Classification, EVENT_SCHEMA_VERSION};
use crate::hex::{ContentHashHex, SignatureHex, SigningKeyId};
use crate::integrity::{reject_duplicate_json_keys, signing_key_id_from_verifying_key};
use crate::limits::{
    MAX_ACCEPTED_RECORD_BYTES, MAX_JSON_CONTAINER_ELEMENTS, MAX_JSON_NESTING_DEPTH,
    MAX_JSON_STRING_BYTES_PER_RECORD,
};
use crate::validate_record::{ValidationFailure, validate_accepted_record_bytes};

pub const EVENT_SCHEMA_VERSION_V2: u32 = 2;
pub const V2_CONTENT_HASH_DOMAIN: &[u8] = b"cryptex-event-content-v2";
pub const V2_SIGNATURE_DOMAIN: &[u8] = b"cryptex-event-sig-v2";

/// A record whose cryptography is verified but whose known payload validator
/// is deliberately not part of Slice 1 yet. Callers must not materialize it.
#[derive(Debug, Clone, PartialEq)]
pub struct VerifiedV2PayloadPending {
    pub event_id: String,
    pub brain_id: String,
    pub device_id: String,
    pub device_seq: DeviceSeq,
    pub created_at: String,
    pub event_type: String,
    pub payload_schema_version: u32,
    pub classification: Classification,
    pub signing_key_id: SigningKeyId,
    pub payload: Value,
    pub prev_content_hash: ContentHashHex,
    pub content_hash: ContentHashHex,
    pub raw_record: Vec<u8>,
}

/// A known v2 envelope with unknown event type or payload schema. The original
/// bytes are retained exactly; this state is never interpreted as corruption.
#[derive(Debug, Clone, PartialEq)]
pub struct VerifiedUnsupportedV2 {
    pub event_id: String,
    pub device_id: String,
    pub device_seq: DeviceSeq,
    pub signing_key_id: SigningKeyId,
    pub prev_content_hash: ContentHashHex,
    pub content_hash: ContentHashHex,
    pub raw_record: Vec<u8>,
}

/// An envelope version whose verification contract this binary does not know.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnverifiableEnvelope {
    pub schema_version: Option<u64>,
    pub raw_record: Vec<u8>,
}

/// Read classification. `VerifiedV2PayloadPending` is a transitional M4
/// implementation state, not an authorization to apply the payload.
#[derive(Debug, Clone, PartialEq)]
pub enum VersionedRecord {
    VerifiedV1(AcceptedEvent),
    VerifiedV2PayloadPending(VerifiedV2PayloadPending),
    VerifiedUnsupported(VerifiedUnsupportedV2),
    Unverifiable(UnverifiableEnvelope),
}

/// Per-device cross-version chain state. Call `verify_next` before treating a
/// record as a final `verified_*` classification or advancing any watermark.
#[derive(Debug, Clone)]
pub struct VersionedChainVerifier {
    expected_device_id: String,
    last_seq: Option<u64>,
    last_hash: ContentHashHex,
}

impl VersionedChainVerifier {
    pub fn new(device_id: impl Into<String>) -> Self {
        Self {
            expected_device_id: device_id.into(),
            last_seq: None,
            last_hash: ContentHashHex::zero(),
        }
    }

    /// Validate device binding, strictly increasing sequence (gaps allowed),
    /// and the v1/v2 shared hash link before updating the verifier state.
    pub fn verify_next(&mut self, record: &VersionedRecord) -> Result<(), ValidationFailure> {
        let (device_id, seq, prev_hash, content_hash, schema) = match record {
            VersionedRecord::VerifiedV1(event) => (
                event.device_id.as_str(),
                event
                    .device_seq
                    .to_u64()
                    .map_err(|_| failure("invalid_record", "device_seq is invalid", Some(1)))?,
                &event.prev_content_hash,
                &event.content_hash,
                1,
            ),
            VersionedRecord::VerifiedV2PayloadPending(event) => (
                event.device_id.as_str(),
                event
                    .device_seq
                    .to_u64()
                    .map_err(|_| failure("invalid_record", "device_seq is invalid", Some(2)))?,
                &event.prev_content_hash,
                &event.content_hash,
                2,
            ),
            VersionedRecord::VerifiedUnsupported(event) => (
                event.device_id.as_str(),
                event
                    .device_seq
                    .to_u64()
                    .map_err(|_| failure("invalid_record", "device_seq is invalid", Some(2)))?,
                &event.prev_content_hash,
                &event.content_hash,
                2,
            ),
            VersionedRecord::Unverifiable(_) => {
                return Err(failure(
                    "invalid_record",
                    "unverifiable envelope blocks the device chain",
                    None,
                ));
            }
        };

        if device_id != self.expected_device_id {
            return Err(failure(
                "integrity_failure",
                "event device_id does not match the device chain",
                Some(schema),
            ));
        }
        if let Some(prior_seq) = self.last_seq {
            if seq <= prior_seq || prev_hash != &self.last_hash {
                return Err(failure(
                    "integrity_failure",
                    "device sequence or cross-version hash-chain link is invalid",
                    Some(schema),
                ));
            }
        } else if prev_hash.as_str() != crate::hex::ZERO_CONTENT_HASH_HEX {
            return Err(failure(
                "integrity_failure",
                "first device event must use the zero previous-hash sentinel",
                Some(schema),
            ));
        }

        self.last_seq = Some(seq);
        self.last_hash = content_hash.clone();
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct V2Envelope {
    schema_version: u32,
    event_id: String,
    brain_id: String,
    device_id: String,
    device_seq: DeviceSeq,
    created_at: String,
    event_type: String,
    payload_schema_version: u32,
    classification: Classification,
    signing_key_id: SigningKeyId,
    payload: Value,
    prev_content_hash: ContentHashHex,
    content_hash: ContentHashHex,
    signature: SignatureHex,
}

/// Dispatch a canonical JSON event record by `schema_version`.
///
/// Unknown protocol versions are preserved as unverifiable, without pretending
/// they have passed authentication. V1 is passed to the original strict reader.
/// V2 verifies the exact ADR-0017 domains and common envelope before deciding
/// whether its event/payload schema is known to this reader.
pub fn read_versioned_record(
    bytes: &[u8],
    verifying_key: &VerifyingKey,
) -> Result<VersionedRecord, ValidationFailure> {
    if bytes.len() > MAX_ACCEPTED_RECORD_BYTES as usize {
        return Err(failure(
            "limit_exceeded",
            "record exceeds the ADR body limit",
            None,
        ));
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| failure("invalid_record", "record is not valid UTF-8", None))?;
    let value: Value = serde_json::from_str(text)
        .map_err(|_| failure("invalid_record", "record JSON is malformed", None))?;

    let version = peek_version(&value);
    match version {
        Some(v) if v == u64::from(EVENT_SCHEMA_VERSION) => {
            // Preserve the legacy validator as the sole authority for v1.
            validate_accepted_record_bytes(bytes, verifying_key).map(VersionedRecord::VerifiedV1)
        }
        Some(v) if v == u64::from(EVENT_SCHEMA_VERSION_V2) => {
            reject_duplicate_json_keys(text).map_err(|_| {
                failure(
                    "duplicate_json_key",
                    "record contains duplicate JSON keys",
                    Some(2),
                )
            })?;
            enforce_limits(&value, 0)
                .map_err(|_| failure("limit_exceeded", "record exceeds JSON limits", Some(2)))?;
            read_v2(bytes, value, verifying_key)
        }
        Some(v) => Ok(VersionedRecord::Unverifiable(UnverifiableEnvelope {
            schema_version: Some(v),
            raw_record: bytes.to_vec(),
        })),
        None if value.get("schema_version").is_some() => {
            // The version discriminator is present but not interpretable by
            // this binary. Preserve rather than applying v1 corruption rules.
            Ok(VersionedRecord::Unverifiable(UnverifiableEnvelope {
                schema_version: None,
                raw_record: bytes.to_vec(),
            }))
        }
        None => Err(failure(
            "invalid_record",
            "record has no schema_version discriminator",
            None,
        )),
    }
}

fn read_v2(
    bytes: &[u8],
    value: Value,
    verifying_key: &VerifyingKey,
) -> Result<VersionedRecord, ValidationFailure> {
    let envelope: V2Envelope = serde_json::from_value(value.clone()).map_err(|_| {
        failure(
            "invalid_record",
            "v2 envelope fields are malformed or unknown",
            Some(2),
        )
    })?;
    if envelope.schema_version != EVENT_SCHEMA_VERSION_V2 {
        return Err(failure(
            "invalid_record",
            "v2 schema discriminator mismatch",
            Some(2),
        ));
    }
    parse_uuid_str(&envelope.event_id)
        .and_then(|_| parse_uuid_str(&envelope.brain_id))
        .map_err(|_| failure("invalid_record", "v2 UUID field is invalid", Some(2)))?;
    DeviceId::new(envelope.device_id.clone())
        .map_err(|_| failure("invalid_record", "v2 device_id is invalid", Some(2)))?;
    envelope
        .device_seq
        .to_u64()
        .map_err(|_| failure("invalid_record", "v2 device_seq is invalid", Some(2)))?;
    parse_rfc3339_str(&envelope.created_at)
        .map_err(|_| failure("invalid_record", "v2 created_at is invalid", Some(2)))?;
    if envelope.event_type.is_empty()
        || envelope.payload_schema_version == 0
        || envelope.payload_schema_version > i32::MAX as u32
        || !envelope.payload.is_object()
    {
        return Err(failure(
            "invalid_record",
            "v2 event discriminator is invalid",
            Some(2),
        ));
    }

    let expected_key_id = signing_key_id_from_verifying_key(verifying_key).map_err(|_| {
        failure(
            "integrity_failure",
            "v2 signing key id derivation failed",
            Some(2),
        )
    })?;
    if envelope.signing_key_id != expected_key_id {
        return Err(failure(
            "integrity_failure",
            "v2 signing_key_id does not match device key",
            Some(2),
        ));
    }

    let mut signable = value.clone();
    let object = signable.as_object_mut().expect("deserialized v2 object");
    object.remove("content_hash");
    object.remove("signature");
    let canonical = serde_json_canonicalizer::to_vec(&signable).map_err(|_| {
        failure(
            "integrity_failure",
            "v2 JCS canonicalization failed",
            Some(2),
        )
    })?;
    let canonical_record = serde_json_canonicalizer::to_vec(&value).map_err(|_| {
        failure(
            "integrity_failure",
            "v2 record JCS canonicalization failed",
            Some(2),
        )
    })?;
    if canonical_record.as_slice() != bytes {
        return Err(failure(
            "invalid_record",
            "v2 on-disk body is not canonical JCS",
            Some(2),
        ));
    }
    let mut hash = blake3::Hasher::new();
    hash.update(V2_CONTENT_HASH_DOMAIN);
    hash.update(&[0]);
    hash.update(&canonical);
    let computed_hash = *hash.finalize().as_bytes();
    let recorded_hash = envelope.content_hash.to_bytes().map_err(|_| {
        failure(
            "integrity_failure",
            "v2 content_hash encoding is invalid",
            Some(2),
        )
    })?;
    if computed_hash != recorded_hash {
        return Err(failure(
            "integrity_failure",
            "v2 content_hash mismatch",
            Some(2),
        ));
    }
    let mut sig_preimage = Vec::with_capacity(V2_SIGNATURE_DOMAIN.len() + 33);
    sig_preimage.extend_from_slice(V2_SIGNATURE_DOMAIN);
    sig_preimage.push(0);
    sig_preimage.extend_from_slice(&computed_hash);
    let signature =
        ed25519_dalek::Signature::from_bytes(&envelope.signature.to_bytes().map_err(|_| {
            failure(
                "integrity_failure",
                "v2 signature encoding is invalid",
                Some(2),
            )
        })?);
    verifying_key
        .verify_strict(&sig_preimage, &signature)
        .map_err(|_| {
            failure(
                "integrity_failure",
                "v2 signature verification failed",
                Some(2),
            )
        })?;

    // Slice 1 has no typed payload validator. Do not label the known schema as
    // unsupported or apply it; Slice 2 will supply the exact validator.
    if envelope.event_type == "knowledge.assertion" && envelope.payload_schema_version == 1 {
        return Ok(VersionedRecord::VerifiedV2PayloadPending(
            VerifiedV2PayloadPending {
                event_id: envelope.event_id,
                brain_id: envelope.brain_id,
                device_id: envelope.device_id,
                device_seq: envelope.device_seq,
                created_at: envelope.created_at,
                event_type: envelope.event_type,
                payload_schema_version: envelope.payload_schema_version,
                classification: envelope.classification,
                signing_key_id: envelope.signing_key_id,
                payload: envelope.payload,
                prev_content_hash: envelope.prev_content_hash,
                content_hash: envelope.content_hash,
                raw_record: bytes.to_vec(),
            },
        ));
    }

    Ok(VersionedRecord::VerifiedUnsupported(
        VerifiedUnsupportedV2 {
            event_id: envelope.event_id,
            device_id: envelope.device_id,
            device_seq: envelope.device_seq,
            signing_key_id: envelope.signing_key_id,
            prev_content_hash: envelope.prev_content_hash,
            content_hash: envelope.content_hash,
            raw_record: bytes.to_vec(),
        },
    ))
}

fn peek_version(value: &Value) -> Option<u64> {
    value.get("schema_version").and_then(Value::as_u64)
}

fn failure(reason: &str, detail: &str, schema_version_if_parsed: Option<u64>) -> ValidationFailure {
    use crate::validate_record::ValidationReasonCode;
    let reason = match reason {
        "limit_exceeded" => ValidationReasonCode::LimitExceeded,
        "duplicate_json_key" => ValidationReasonCode::DuplicateJsonKey,
        "integrity_failure" => ValidationReasonCode::IntegrityFailure,
        "unknown_field" => ValidationReasonCode::UnknownField,
        _ => ValidationReasonCode::InvalidRecord,
    };
    ValidationFailure {
        reason,
        detail: detail.to_string(),
        schema_version_if_parsed: schema_version_if_parsed.and_then(|v| u32::try_from(v).ok()),
    }
}

fn enforce_limits(value: &Value, depth: usize) -> Result<(), ()> {
    if depth > MAX_JSON_NESTING_DEPTH {
        return Err(());
    }
    match value {
        Value::Object(map) => {
            if map.len() > MAX_JSON_CONTAINER_ELEMENTS {
                return Err(());
            }
            let mut bytes = 0usize;
            for (key, value) in map {
                bytes = bytes.saturating_add(key.len());
                enforce_limits(value, depth + 1)?;
                if let Value::String(text) = value {
                    bytes = bytes.saturating_add(text.len());
                }
                if bytes > MAX_JSON_STRING_BYTES_PER_RECORD {
                    return Err(());
                }
            }
            Ok(())
        }
        Value::Array(items) => {
            if items.len() > MAX_JSON_CONTAINER_ELEMENTS {
                return Err(());
            }
            for item in items {
                enforce_limits(item, depth + 1)?;
            }
            Ok(())
        }
        Value::String(text) if text.len() > MAX_JSON_STRING_BYTES_PER_RECORD => Err(()),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device_seq::DeviceSeq;
    use crate::envelope::{EventPayload, EventType, MemoryNotePayload, SignableEvent};
    use crate::integrity::{accept_event, signing_key_id_from_verifying_key};
    use ed25519_dalek::{Signer, SigningKey};

    const SEED: [u8; 32] = [0x42; 32];

    fn make_v2(event_type: &str, payload_schema_version: u32) -> (Vec<u8>, SigningKey) {
        make_v2_with(
            event_type,
            payload_schema_version,
            1,
            crate::hex::ZERO_CONTENT_HASH_HEX,
        )
    }

    fn make_v2_with(
        event_type: &str,
        payload_schema_version: u32,
        device_seq: u64,
        prev_content_hash: &str,
    ) -> (Vec<u8>, SigningKey) {
        let key = SigningKey::from_bytes(&SEED);
        let key_id = signing_key_id_from_verifying_key(&key.verifying_key()).unwrap();
        let mut record = serde_json::json!({
            "schema_version": 2,
            "event_id": "11111111-1111-4111-8111-111111111111",
            "brain_id": "22222222-2222-4222-8222-222222222222",
            "device_id": "device-reader-test",
            "device_seq": device_seq.to_string(),
            "created_at": "2026-10-08T00:00:00Z",
            "event_type": event_type,
            "payload_schema_version": payload_schema_version,
            "classification": "internal",
            "signing_key_id": key_id.as_str(),
            "payload": {"kind": "test", "value": "synthetic"},
            "prev_content_hash": prev_content_hash,
            "content_hash": "00".repeat(32),
            "signature": "00".repeat(64),
        });
        let mut signable = record.clone();
        signable.as_object_mut().unwrap().remove("content_hash");
        signable.as_object_mut().unwrap().remove("signature");
        let canonical = serde_json_canonicalizer::to_vec(&signable).unwrap();
        let mut hash = blake3::Hasher::new();
        hash.update(V2_CONTENT_HASH_DOMAIN);
        hash.update(&[0]);
        hash.update(&canonical);
        let hash = *hash.finalize().as_bytes();
        let signature = key.sign(&[V2_SIGNATURE_DOMAIN, &[0], hash.as_slice()].concat());
        record.as_object_mut().unwrap().insert(
            "content_hash".into(),
            Value::String(
                ContentHashHex::from_bytes(&hash)
                    .unwrap()
                    .as_str()
                    .to_string(),
            ),
        );
        record.as_object_mut().unwrap().insert(
            "signature".into(),
            Value::String(
                SignatureHex::from_bytes(&signature.to_bytes())
                    .unwrap()
                    .as_str()
                    .to_string(),
            ),
        );
        let bytes = serde_json_canonicalizer::to_vec(&record).unwrap();
        (bytes, key)
    }

    #[test]
    fn known_v2_common_envelope_is_crypto_verified_but_not_yet_materializable() {
        let (bytes, key) = make_v2("knowledge.assertion", 1);
        let result = read_versioned_record(&bytes, &key.verifying_key()).unwrap();
        let VersionedRecord::VerifiedV2PayloadPending(record) = result else {
            panic!("expected typed-payload validation boundary");
        };
        assert_eq!(record.raw_record, bytes);
        assert_eq!(record.event_type, "knowledge.assertion");
    }

    #[test]
    fn unknown_v2_event_or_payload_schema_is_verified_and_preserved() {
        for (event_type, payload_version) in [("future.event", 1), ("knowledge.assertion", 2)] {
            let (bytes, key) = make_v2(event_type, payload_version);
            let result = read_versioned_record(&bytes, &key.verifying_key()).unwrap();
            let VersionedRecord::VerifiedUnsupported(record) = result else {
                panic!("expected verified-unsupported classification");
            };
            assert_eq!(record.raw_record, bytes);
            assert_eq!(record.device_seq.to_u64().unwrap(), 1);
        }
    }

    #[test]
    fn unknown_envelope_version_is_unverifiable_and_preserved_exactly() {
        let bytes = br#"{"schema_version":77,"future":"opaque"}"#.to_vec();
        let result =
            read_versioned_record(&bytes, &SigningKey::from_bytes(&SEED).verifying_key()).unwrap();
        let VersionedRecord::Unverifiable(record) = result else {
            panic!("expected unverifiable envelope");
        };
        assert_eq!(record.schema_version, Some(77));
        assert_eq!(record.raw_record, bytes);
    }

    #[test]
    fn v1_golden_event_still_dispatches_through_frozen_validator() {
        let key = SigningKey::from_bytes(&[0x11; 32]);
        let signable = SignableEvent {
            schema_version: EVENT_SCHEMA_VERSION,
            event_id: "11111111-1111-4111-8111-111111111111".into(),
            brain_id: "22222222-2222-4222-8222-222222222222".into(),
            device_id: "device-a".into(),
            device_seq: DeviceSeq::from_u64(1).unwrap(),
            created_at: "2026-10-07T00:00:00Z".into(),
            event_type: EventType::MemoryNote,
            classification: Classification::Internal,
            signing_key_id: signing_key_id_from_verifying_key(&key.verifying_key()).unwrap(),
            payload: EventPayload::MemoryNote(MemoryNotePayload::new("kat-hello").unwrap()),
            prev_content_hash: ContentHashHex::zero(),
        };
        let accepted = accept_event(signable, &key).unwrap();
        assert_eq!(
            accepted.content_hash.as_str(),
            "45dc225413467451623b09445ace22de02255184faeb114421a2df68c17e1be0"
        );
        assert_eq!(
            accepted.signature.as_str(),
            concat!(
                "955eae9c88d8e1d34a75511a4734fbc18d83fd0fd978e907206cb507767958aa",
                "d95f9352d243de0cc3e0f8804429ca865ef499cbac8cf760058a55644c599900"
            )
        );
        let bytes = serde_json_canonicalizer::to_vec(&accepted).unwrap();
        let dispatched = read_versioned_record(&bytes, &key.verifying_key()).unwrap();
        assert_eq!(dispatched, VersionedRecord::VerifiedV1(accepted));

        let mut with_v2_field: Value = serde_json::from_slice(&bytes).unwrap();
        with_v2_field
            .as_object_mut()
            .unwrap()
            .insert("payload_schema_version".into(), Value::from(1));
        let with_v2_field = serde_json::to_vec(&with_v2_field).unwrap();
        let error = read_versioned_record(&with_v2_field, &key.verifying_key()).unwrap_err();
        assert_eq!(error.reason, crate::ValidationReasonCode::UnknownField);
    }

    #[test]
    fn v2_signature_failure_is_not_downgraded_to_unsupported() {
        let (mut bytes, key) = make_v2("future.event", 99);
        let marker = b"\"signature\":\"";
        let start = bytes
            .windows(marker.len())
            .position(|window| window == marker)
            .unwrap()
            + marker.len();
        bytes[start] = if bytes[start] == b'0' { b'1' } else { b'0' };
        let err = read_versioned_record(&bytes, &key.verifying_key()).unwrap_err();
        assert_eq!(
            err.reason,
            crate::ValidationReasonCode::IntegrityFailure,
            "{err}"
        );
    }

    #[test]
    fn verified_unsupported_v2_advances_cross_version_chain_only_after_link_check() {
        let (first_bytes, key) = make_v2("future.event", 1);
        let first = read_versioned_record(&first_bytes, &key.verifying_key()).unwrap();
        let VersionedRecord::VerifiedUnsupported(first_event) = &first else {
            panic!("expected verified unsupported first event");
        };
        let mut chain = VersionedChainVerifier::new("device-reader-test");
        chain.verify_next(&first).unwrap();

        // ADR-0015 permits sequence gaps after a durable allocate with no append.
        let (second_bytes, _) =
            make_v2_with("future.event", 1, 3, first_event.content_hash.as_str());
        let second = read_versioned_record(&second_bytes, &key.verifying_key()).unwrap();
        chain.verify_next(&second).unwrap();
        assert_eq!(chain.last_seq, Some(3));

        let (bad_bytes, _) = make_v2_with("future.event", 1, 4, crate::hex::ZERO_CONTENT_HASH_HEX);
        let bad = read_versioned_record(&bad_bytes, &key.verifying_key()).unwrap();
        let err = chain.verify_next(&bad).unwrap_err();
        assert_eq!(err.reason, crate::ValidationReasonCode::IntegrityFailure);
        assert_eq!(
            chain.last_seq,
            Some(3),
            "failed link must not advance state"
        );
    }

    #[test]
    fn v1_v2_v1_chain_is_verified_without_changing_v1_contract() {
        let key = SigningKey::from_bytes(&SEED);
        let key_id = signing_key_id_from_verifying_key(&key.verifying_key()).unwrap();
        let first_v1 = accept_event(
            SignableEvent {
                schema_version: EVENT_SCHEMA_VERSION,
                event_id: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".into(),
                brain_id: "22222222-2222-4222-8222-222222222222".into(),
                device_id: "device-reader-test".into(),
                device_seq: DeviceSeq::from_u64(1).unwrap(),
                created_at: "2026-10-08T00:00:00Z".into(),
                event_type: EventType::MemoryNote,
                classification: Classification::Internal,
                signing_key_id: key_id.clone(),
                payload: EventPayload::MemoryNote(MemoryNotePayload::new("synthetic v1").unwrap()),
                prev_content_hash: ContentHashHex::zero(),
            },
            &key,
        )
        .unwrap();
        let first_bytes = serde_json_canonicalizer::to_vec(&first_v1).unwrap();
        let first = read_versioned_record(&first_bytes, &key.verifying_key()).unwrap();
        assert!(matches!(first, VersionedRecord::VerifiedV1(_)));

        let (v2_bytes, _) = make_v2_with("future.event", 1, 3, first_v1.content_hash.as_str());
        let v2 = read_versioned_record(&v2_bytes, &key.verifying_key()).unwrap();
        assert!(matches!(v2, VersionedRecord::VerifiedUnsupported(_)));
        let VersionedRecord::VerifiedUnsupported(v2_record) = &v2 else {
            unreachable!();
        };

        let second_v1 = accept_event(
            SignableEvent {
                schema_version: EVENT_SCHEMA_VERSION,
                event_id: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb".into(),
                brain_id: "22222222-2222-4222-8222-222222222222".into(),
                device_id: "device-reader-test".into(),
                device_seq: DeviceSeq::from_u64(5).unwrap(),
                created_at: "2026-10-08T00:00:01Z".into(),
                event_type: EventType::MemoryNote,
                classification: Classification::Internal,
                signing_key_id: key_id,
                payload: EventPayload::MemoryNote(
                    MemoryNotePayload::new("synthetic v1 after v2").unwrap(),
                ),
                prev_content_hash: v2_record.content_hash.clone(),
            },
            &key,
        )
        .unwrap();
        let second_bytes = serde_json_canonicalizer::to_vec(&second_v1).unwrap();
        let second = read_versioned_record(&second_bytes, &key.verifying_key()).unwrap();

        let mut chain = VersionedChainVerifier::new("device-reader-test");
        chain.verify_next(&first).unwrap();
        chain.verify_next(&v2).unwrap();
        chain.verify_next(&second).unwrap();
        assert_eq!(chain.last_seq, Some(5));
        assert_eq!(chain.last_hash, second_v1.content_hash);
    }
}
