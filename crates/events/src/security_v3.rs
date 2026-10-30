//! Strict authenticated reader for the proposed M8 security-event envelope.
//!
//! This reader verifies only the record envelope, payload shape, content hash,
//! and signature. It does not verify bootstrap transcripts, chain completeness,
//! cutoff boundary hashes, issuer trust, or authorize any mutation. The
//! version-dispatched reader uses this strict envelope verifier; separate
//! verified-history and trust-reducer layers validate policy semantics.

use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use cryptex_core::ids::DeviceId;
use cryptex_core::validate::{parse_rfc3339_str, parse_uuid_str};

use crate::device_seq::DeviceSeq;
use crate::hex::{ContentHashHex, SignatureHex, SigningKeyId, ZERO_CONTENT_HASH_HEX};
use crate::integrity::signing_key_id_from_verifying_key;
use crate::limits::{
    MAX_ACCEPTED_RECORD_BYTES, MAX_JSON_CONTAINER_ELEMENTS, MAX_JSON_NESTING_DEPTH,
    MAX_JSON_STRING_BYTES_PER_RECORD,
};
use crate::validate_record::ValidationFailure;

pub const SECURITY_V3_SCHEMA_VERSION: u32 = 3;
pub const SECURITY_V3_CONTENT_HASH_DOMAIN: &[u8] = b"cryptex-security-content-v1";
pub const SECURITY_V3_SIGNATURE_DOMAIN: &[u8] = b"cryptex-security-sign-v1";

#[derive(Debug, Clone, PartialEq)]
pub struct SecurityEventV3 {
    event_id: String,
    brain_id: String,
    device_id: DeviceId,
    device_seq: DeviceSeq,
    created_at: String,
    event_type: SecurityEventTypeV3,
    signing_key_id: SigningKeyId,
    payload: SecurityPayloadV3,
    operation_digest: String,
    prev_content_hash: ContentHashHex,
    content_hash: ContentHashHex,
    raw_record: Vec<u8>,
}

impl SecurityEventV3 {
    #[must_use]
    pub fn event_id(&self) -> &str {
        &self.event_id
    }

    #[must_use]
    pub fn brain_id(&self) -> &str {
        &self.brain_id
    }

    #[must_use]
    pub fn created_at(&self) -> &str {
        &self.created_at
    }

    #[must_use]
    pub const fn device_id(&self) -> &DeviceId {
        &self.device_id
    }

    #[must_use]
    pub const fn device_seq(&self) -> &DeviceSeq {
        &self.device_seq
    }

    #[must_use]
    pub const fn event_type(&self) -> SecurityEventTypeV3 {
        self.event_type
    }

    #[must_use]
    pub const fn signing_key_id(&self) -> &SigningKeyId {
        &self.signing_key_id
    }

    #[must_use]
    pub const fn payload(&self) -> &SecurityPayloadV3 {
        &self.payload
    }

    #[must_use]
    pub const fn prev_content_hash(&self) -> &ContentHashHex {
        &self.prev_content_hash
    }

    #[must_use]
    pub const fn content_hash(&self) -> &ContentHashHex {
        &self.content_hash
    }

    #[must_use]
    pub fn raw_record(&self) -> &[u8] {
        &self.raw_record
    }

    /// Stable digest of the security operation, excluding append-position
    /// fields so an uncertain retry can be compared at a new reserved seq.
    #[must_use]
    pub fn operation_digest(&self) -> &str {
        &self.operation_digest
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityEventTypeV3 {
    PeerApproved,
    DeviceRestricted,
    DeviceSelfRetired,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SecurityPayloadV3 {
    /// The outer event is authenticated; transcript message signatures and
    /// agreement bindings still require the separate bootstrap verifier.
    PeerApproved {
        transaction_id: String,
        transcript: Value,
    },
    DeviceRestricted {
        transaction_id: String,
        target_device_id: DeviceId,
        target_signing_key_id: SigningKeyId,
        target_public_key: String,
        reason: RestrictionReasonV3,
        cutoff_seq: u64,
        cutoff_content_hash: ContentHashHex,
    },
    DeviceSelfRetired {
        transaction_id: String,
        last_authorized_seq: u64,
        last_authorized_content_hash: ContentHashHex,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestrictionReasonV3 {
    Retired,
    Compromised,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SecurityV3Envelope {
    schema_version: u32,
    event_id: String,
    brain_id: String,
    device_id: String,
    device_seq: DeviceSeq,
    created_at: String,
    event_type: String,
    classification: String,
    signing_key_id: SigningKeyId,
    payload: Value,
    prev_content_hash: ContentHashHex,
    content_hash: ContentHashHex,
    signature: SignatureHex,
}

#[derive(Serialize)]
struct SignableSecurityV3Envelope<'a> {
    schema_version: u32,
    event_id: &'a str,
    brain_id: &'a str,
    device_id: &'a str,
    device_seq: &'a DeviceSeq,
    created_at: &'a str,
    event_type: &'static str,
    classification: &'static str,
    signing_key_id: &'a SigningKeyId,
    payload: Value,
    prev_content_hash: &'a ContentHashHex,
}

/// Sign the local v3 `security.peer_approved` anchor from a fully verified
/// four-message bootstrap transcript. The signing identity must be exactly
/// one participant in that transcript. This only creates record bytes; the
/// daemon writer must still serialize, journal, append idempotently, and
/// reconcile before local trust becomes active.
pub fn sign_peer_approved_event(
    transcript: &crate::bootstrap::VerifiedBootstrapTranscript,
    event_id: &str,
    device_seq: DeviceSeq,
    created_at: &str,
    signing_key_id: &SigningKeyId,
    prev_content_hash: &ContentHashHex,
    signing_key: &SigningKey,
) -> Result<Vec<u8>, ValidationFailure> {
    let local_key_id = signing_key_id_from_verifying_key(&signing_key.verifying_key())
        .map_err(|_| failure("integrity_failure", "v3 signing key id derivation failed"))?;
    let is_initiator = transcript.initiator().signing_key_id == local_key_id
        && transcript.initiator().public_key == signing_key.verifying_key().to_bytes();
    let is_responder = transcript.responder().signing_key_id == local_key_id
        && transcript.responder().public_key == signing_key.verifying_key().to_bytes();
    if signing_key_id != &local_key_id || !(is_initiator || is_responder) {
        return Err(failure(
            "integrity_failure",
            "v3 local approval signer is not a verified transcript participant",
        ));
    }
    let local = if is_initiator {
        transcript.initiator()
    } else {
        transcript.responder()
    };
    sign_security_event(
        event_id,
        transcript.brain_id(),
        &local.device_id,
        device_seq,
        created_at,
        "security.peer_approved",
        signing_key_id,
        serde_json::json!({
            "transaction_id": transcript.transaction_id(),
            "transcript": transcript.transcript(),
        }),
        prev_content_hash,
        signing_key,
    )
}

/// Sign a locally owner-approved device restriction. This creates canonical
/// event bytes only; the daemon must validate the exact approval snapshot and
/// cutoff boundary while holding the canonical locks before appending.
#[allow(clippy::too_many_arguments)]
pub fn sign_device_restricted_event(
    event_id: &str,
    brain_id: &str,
    author_device_id: &DeviceId,
    device_seq: DeviceSeq,
    created_at: &str,
    author_signing_key_id: &SigningKeyId,
    prev_content_hash: &ContentHashHex,
    transaction_id: &str,
    target_device_id: &DeviceId,
    target_signing_key_id: &SigningKeyId,
    target_public_key: &[u8; 32],
    reason: RestrictionReasonV3,
    cutoff_seq: u64,
    cutoff_content_hash: &ContentHashHex,
    signing_key: &SigningKey,
) -> Result<Vec<u8>, ValidationFailure> {
    let target_key = VerifyingKey::from_bytes(target_public_key)
        .map_err(|_| failure("invalid_record", "target public key is invalid"))?;
    let derived_target_key_id = signing_key_id_from_verifying_key(&target_key)
        .map_err(|_| failure("invalid_record", "target key id derivation failed"))?;
    if target_signing_key_id != &derived_target_key_id {
        return Err(failure(
            "integrity_failure",
            "target signing key id does not match target public key",
        ));
    }
    let reason = match reason {
        RestrictionReasonV3::Retired => "retired",
        RestrictionReasonV3::Compromised => "compromised",
    };
    sign_security_event(
        event_id,
        brain_id,
        author_device_id,
        device_seq,
        created_at,
        "security.device_restricted",
        author_signing_key_id,
        serde_json::json!({
            "transaction_id": transaction_id,
            "target_device_id": target_device_id.as_str(),
            "target_signing_key_id": target_signing_key_id.as_str(),
            "target_public_key": encode_hex(target_public_key),
            "reason": reason,
            "cutoff_seq": cutoff_seq.to_string(),
            "cutoff_content_hash": cutoff_content_hash.as_str(),
        }),
        prev_content_hash,
        signing_key,
    )
}

/// Sign a local terminal self-retirement record. The caller must supply the
/// exact immediately preceding verified chain boundary; this function checks
/// the sequence ordering and closed event payload, not the history itself.
#[allow(clippy::too_many_arguments)]
pub fn sign_device_self_retired_event(
    event_id: &str,
    brain_id: &str,
    author_device_id: &DeviceId,
    device_seq: DeviceSeq,
    created_at: &str,
    author_signing_key_id: &SigningKeyId,
    prev_content_hash: &ContentHashHex,
    transaction_id: &str,
    last_authorized_seq: u64,
    last_authorized_content_hash: &ContentHashHex,
    signing_key: &SigningKey,
) -> Result<Vec<u8>, ValidationFailure> {
    let current_seq = device_seq
        .to_u64()
        .map_err(|_| failure("invalid_record", "self-retirement sequence is invalid"))?;
    if last_authorized_seq >= current_seq {
        return Err(failure(
            "invalid_record",
            "self-retirement cutoff must precede its control event",
        ));
    }
    sign_security_event(
        event_id,
        brain_id,
        author_device_id,
        device_seq,
        created_at,
        "security.device_self_retired",
        author_signing_key_id,
        serde_json::json!({
            "transaction_id": transaction_id,
            "last_authorized_seq": last_authorized_seq.to_string(),
            "last_authorized_content_hash": last_authorized_content_hash.as_str(),
        }),
        prev_content_hash,
        signing_key,
    )
}

#[allow(clippy::too_many_arguments)]
fn sign_security_event(
    event_id: &str,
    brain_id: &str,
    device_id: &DeviceId,
    device_seq: DeviceSeq,
    created_at: &str,
    event_type: &'static str,
    signing_key_id: &SigningKeyId,
    payload: Value,
    prev_content_hash: &ContentHashHex,
    signing_key: &SigningKey,
) -> Result<Vec<u8>, ValidationFailure> {
    let local_key_id = signing_key_id_from_verifying_key(&signing_key.verifying_key())
        .map_err(|_| failure("integrity_failure", "v3 signing key id derivation failed"))?;
    if signing_key_id != &local_key_id {
        return Err(failure(
            "integrity_failure",
            "v3 signing key id does not match signing key",
        ));
    }
    let signable = SignableSecurityV3Envelope {
        schema_version: SECURITY_V3_SCHEMA_VERSION,
        event_id,
        brain_id,
        device_id: device_id.as_str(),
        device_seq: &device_seq,
        created_at,
        event_type,
        classification: "internal",
        signing_key_id,
        payload,
        prev_content_hash,
    };
    let signable_value = serde_json::to_value(&signable)
        .map_err(|_| failure("invalid_record", "v3 signable serialization failed"))?;
    let canonical_signable = serde_json_canonicalizer::to_vec(&signable_value)
        .map_err(|_| failure("integrity_failure", "v3 JCS canonicalization failed"))?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(SECURITY_V3_CONTENT_HASH_DOMAIN);
    hasher.update(&[0]);
    hasher.update(&canonical_signable);
    let content_hash = *hasher.finalize().as_bytes();
    let mut signature_preimage = Vec::with_capacity(SECURITY_V3_SIGNATURE_DOMAIN.len() + 33);
    signature_preimage.extend_from_slice(SECURITY_V3_SIGNATURE_DOMAIN);
    signature_preimage.push(0);
    signature_preimage.extend_from_slice(&content_hash);
    let signature = signing_key.sign(&signature_preimage);
    let mut record_value = signable_value;
    let object = record_value
        .as_object_mut()
        .expect("serialized v3 envelope is an object");
    object.insert(
        "content_hash".into(),
        serde_json::json!(encode_hex(&content_hash)),
    );
    object.insert(
        "signature".into(),
        serde_json::json!(encode_hex(&signature.to_bytes())),
    );
    let bytes = serde_json_canonicalizer::to_vec(&record_value)
        .map_err(|_| failure("integrity_failure", "v3 record JCS canonicalization failed"))?;
    read_security_v3_event(&bytes, &signing_key.verifying_key())?;
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

/// Parse and authenticate one candidate M8 v3 security event.
pub fn read_security_v3_event(
    bytes: &[u8],
    verifying_key: &VerifyingKey,
) -> Result<SecurityEventV3, ValidationFailure> {
    if bytes.len() > MAX_ACCEPTED_RECORD_BYTES as usize {
        return Err(failure(
            "limit_exceeded",
            "record exceeds the ADR body limit",
        ));
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| failure("invalid_record", "record is not valid UTF-8"))?;
    crate::integrity::reject_duplicate_json_keys(text)
        .map_err(|_| failure("duplicate_json_key", "record contains duplicate JSON keys"))?;
    let value: Value = serde_json::from_str(text)
        .map_err(|_| failure("invalid_record", "record JSON is malformed"))?;
    enforce_limits(&value, 0)
        .map_err(|_| failure("limit_exceeded", "record exceeds JSON limits"))?;
    let envelope: SecurityV3Envelope = serde_json::from_value(value.clone()).map_err(|_| {
        failure(
            "invalid_record",
            "v3 envelope fields are malformed or unknown",
        )
    })?;

    if envelope.schema_version != SECURITY_V3_SCHEMA_VERSION {
        return Err(failure(
            "invalid_record",
            "v3 schema discriminator mismatch",
        ));
    }
    require_lowercase_uuid(&envelope.event_id)
        .and_then(|()| require_lowercase_uuid(&envelope.brain_id))
        .map_err(|_| failure("invalid_record", "v3 UUID field is not canonical lowercase"))?;
    let device_id = DeviceId::new(envelope.device_id.clone())
        .map_err(|_| failure("invalid_record", "v3 device_id is invalid"))?;
    envelope
        .device_seq
        .to_u64()
        .map_err(|_| failure("invalid_record", "v3 device_seq is invalid"))?;
    parse_rfc3339_str(&envelope.created_at)
        .map_err(|_| failure("invalid_record", "v3 created_at is invalid"))?;
    if envelope.classification != "internal" {
        return Err(failure(
            "invalid_record",
            "security v3 classification must be internal",
        ));
    }

    let event_type = match envelope.event_type.as_str() {
        "security.peer_approved" => SecurityEventTypeV3::PeerApproved,
        "security.device_restricted" => SecurityEventTypeV3::DeviceRestricted,
        "security.device_self_retired" => SecurityEventTypeV3::DeviceSelfRetired,
        _ => return Err(failure("invalid_record", "unknown v3 security event type")),
    };
    let payload = parse_payload(event_type, envelope.payload)?;
    if let SecurityPayloadV3::DeviceSelfRetired {
        last_authorized_seq,
        ..
    } = &payload
        && *last_authorized_seq
            >= envelope
                .device_seq
                .to_u64()
                .map_err(|_| failure("invalid_record", "v3 device_seq is invalid"))?
    {
        return Err(failure(
            "invalid_record",
            "self-retirement cutoff must precede its control event",
        ));
    }

    let expected_key_id = signing_key_id_from_verifying_key(verifying_key)
        .map_err(|_| failure("integrity_failure", "v3 signing key id derivation failed"))?;
    if envelope.signing_key_id != expected_key_id {
        return Err(failure(
            "integrity_failure",
            "v3 signing_key_id does not match device key",
        ));
    }

    let mut signable = value.clone();
    let object = signable
        .as_object_mut()
        .expect("strict v3 envelope must be a JSON object");
    object.remove("content_hash");
    object.remove("signature");
    let canonical_signable = serde_json_canonicalizer::to_vec(&signable)
        .map_err(|_| failure("integrity_failure", "v3 JCS canonicalization failed"))?;
    let canonical_record = serde_json_canonicalizer::to_vec(&value)
        .map_err(|_| failure("integrity_failure", "v3 record JCS canonicalization failed"))?;
    if canonical_record.as_slice() != bytes {
        return Err(failure("invalid_record", "v3 body is not canonical JCS"));
    }

    let mut hasher = blake3::Hasher::new();
    hasher.update(SECURITY_V3_CONTENT_HASH_DOMAIN);
    hasher.update(&[0]);
    hasher.update(&canonical_signable);
    let computed_hash = *hasher.finalize().as_bytes();
    if envelope
        .content_hash
        .to_bytes()
        .map_err(|_| failure("integrity_failure", "v3 content_hash encoding is invalid"))?
        != computed_hash
    {
        return Err(failure("integrity_failure", "v3 content_hash mismatch"));
    }
    let mut signature_preimage = Vec::with_capacity(SECURITY_V3_SIGNATURE_DOMAIN.len() + 33);
    signature_preimage.extend_from_slice(SECURITY_V3_SIGNATURE_DOMAIN);
    signature_preimage.push(0);
    signature_preimage.extend_from_slice(&computed_hash);
    let signature = ed25519_dalek::Signature::from_bytes(
        &envelope
            .signature
            .to_bytes()
            .map_err(|_| failure("integrity_failure", "v3 signature encoding is invalid"))?,
    );
    verifying_key
        .verify_strict(&signature_preimage, &signature)
        .map_err(|_| failure("integrity_failure", "v3 signature verification failed"))?;

    let operation_digest = operation_digest_from_record(&value)?;

    Ok(SecurityEventV3 {
        event_id: envelope.event_id,
        brain_id: envelope.brain_id,
        device_id,
        device_seq: envelope.device_seq,
        created_at: envelope.created_at,
        event_type,
        signing_key_id: envelope.signing_key_id,
        payload,
        operation_digest,
        prev_content_hash: envelope.prev_content_hash,
        content_hash: envelope.content_hash,
        raw_record: bytes.to_vec(),
    })
}

fn operation_digest_from_record(value: &Value) -> Result<String, ValidationFailure> {
    let object = value
        .as_object()
        .ok_or_else(|| failure("invalid_record", "security record must be an object"))?;
    let mut operation = serde_json::Map::new();
    for field in [
        "schema_version",
        "brain_id",
        "device_id",
        "event_type",
        "classification",
        "signing_key_id",
        "payload",
    ] {
        let value = object
            .get(field)
            .ok_or_else(|| failure("invalid_record", "security operation field is missing"))?;
        operation.insert(field.to_owned(), value.clone());
    }
    let canonical = serde_json_canonicalizer::to_vec(&Value::Object(operation))
        .map_err(|_| failure("integrity_failure", "security operation JCS failed"))?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"cryptex-security-operation-v1");
    hasher.update(&[0]);
    hasher.update(&canonical);
    Ok(encode_hex(hasher.finalize().as_bytes()))
}

/// Compute the ADR-0019 stable operation digest for a peer-approval anchor.
/// Append-position fields are absent by construction, allowing exact retries
/// to compare the same operation after a permanently consumed sequence gap.
pub fn peer_approval_operation_digest(
    brain_id: &str,
    device_id: &str,
    signing_key_id: &SigningKeyId,
    transaction_id: &str,
    transcript: &Value,
) -> Result<String, ValidationFailure> {
    require_lowercase_uuid(brain_id)
        .map_err(|_| failure("invalid_record", "Brain ID is not canonical lowercase UUID"))?;
    DeviceId::new(device_id.to_owned())
        .map_err(|_| failure("invalid_record", "device_id is invalid"))?;
    validate_transaction_id(transaction_id)?;
    if !transcript
        .as_array()
        .is_some_and(|messages| messages.len() == 4 && messages.iter().all(Value::is_object))
    {
        return Err(failure(
            "invalid_record",
            "peer_approved transcript must contain four messages",
        ));
    }
    let operation = serde_json::json!({
        "schema_version": SECURITY_V3_SCHEMA_VERSION,
        "brain_id": brain_id,
        "device_id": device_id,
        "event_type": "security.peer_approved",
        "classification": "internal",
        "signing_key_id": signing_key_id,
        "payload": {
            "transaction_id": transaction_id,
            "transcript": transcript
        }
    });
    operation_digest_from_record(&operation)
}

fn parse_payload(
    event_type: SecurityEventTypeV3,
    payload: Value,
) -> Result<SecurityPayloadV3, ValidationFailure> {
    match event_type {
        SecurityEventTypeV3::PeerApproved => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Payload {
                transaction_id: String,
                transcript: Value,
            }
            let payload: Payload = serde_json::from_value(payload)
                .map_err(|_| failure("invalid_record", "peer_approved payload is malformed"))?;
            validate_transaction_id(&payload.transaction_id)?;
            if !payload.transcript.as_array().is_some_and(|messages| {
                messages.len() == 4 && messages.iter().all(Value::is_object)
            }) {
                return Err(failure(
                    "invalid_record",
                    "peer_approved transcript must contain four ordered messages",
                ));
            }
            Ok(SecurityPayloadV3::PeerApproved {
                transaction_id: payload.transaction_id,
                transcript: payload.transcript,
            })
        }
        SecurityEventTypeV3::DeviceRestricted => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Payload {
                transaction_id: String,
                target_device_id: String,
                target_signing_key_id: SigningKeyId,
                target_public_key: String,
                reason: String,
                cutoff_seq: String,
                cutoff_content_hash: ContentHashHex,
            }
            let payload: Payload = serde_json::from_value(payload)
                .map_err(|_| failure("invalid_record", "device_restricted payload is malformed"))?;
            validate_transaction_id(&payload.transaction_id)?;
            let target_device_id = DeviceId::new(payload.target_device_id)
                .map_err(|_| failure("invalid_record", "target_device_id is invalid"))?;
            let public_key_bytes =
                decode_lowercase_hex_32(&payload.target_public_key).ok_or_else(|| {
                    failure(
                        "invalid_record",
                        "target_public_key must be 32-byte lowercase hex",
                    )
                })?;
            let target_verifying_key =
                VerifyingKey::from_bytes(&public_key_bytes).map_err(|_| {
                    failure(
                        "invalid_record",
                        "target_public_key is not a valid Ed25519 key",
                    )
                })?;
            if signing_key_id_from_verifying_key(&target_verifying_key)
                .map_err(|_| failure("invalid_record", "target key id derivation failed"))?
                != payload.target_signing_key_id
            {
                return Err(failure(
                    "invalid_record",
                    "target key id does not match the full target public key",
                ));
            }
            let reason = match payload.reason.as_str() {
                "retired" => RestrictionReasonV3::Retired,
                "compromised" => RestrictionReasonV3::Compromised,
                _ => return Err(failure("invalid_record", "restriction reason is invalid")),
            };
            let cutoff_seq = parse_cutoff_seq(&payload.cutoff_seq)?;
            validate_cutoff_hash(cutoff_seq, &payload.cutoff_content_hash)?;
            Ok(SecurityPayloadV3::DeviceRestricted {
                transaction_id: payload.transaction_id,
                target_device_id,
                target_signing_key_id: payload.target_signing_key_id,
                target_public_key: payload.target_public_key,
                reason,
                cutoff_seq,
                cutoff_content_hash: payload.cutoff_content_hash,
            })
        }
        SecurityEventTypeV3::DeviceSelfRetired => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Payload {
                transaction_id: String,
                last_authorized_seq: String,
                last_authorized_content_hash: ContentHashHex,
            }
            let payload: Payload = serde_json::from_value(payload).map_err(|_| {
                failure("invalid_record", "device_self_retired payload is malformed")
            })?;
            validate_transaction_id(&payload.transaction_id)?;
            let last_authorized_seq = parse_cutoff_seq(&payload.last_authorized_seq)?;
            validate_cutoff_hash(last_authorized_seq, &payload.last_authorized_content_hash)?;
            Ok(SecurityPayloadV3::DeviceSelfRetired {
                transaction_id: payload.transaction_id,
                last_authorized_seq,
                last_authorized_content_hash: payload.last_authorized_content_hash,
            })
        }
    }
}

fn parse_cutoff_seq(raw: &str) -> Result<u64, ValidationFailure> {
    if raw == "0" {
        return Ok(0);
    }
    if raw.is_empty() || raw.starts_with('0') || !raw.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(failure(
            "invalid_record",
            "cutoff sequence is not canonical decimal",
        ));
    }
    raw.parse::<u64>()
        .map_err(|_| failure("invalid_record", "cutoff sequence exceeds u64"))
}

fn validate_cutoff_hash(seq: u64, hash: &ContentHashHex) -> Result<(), ValidationFailure> {
    if (seq == 0) != (hash.as_str() == ZERO_CONTENT_HASH_HEX) {
        return Err(failure(
            "invalid_record",
            "zero cutoff sequence and zero cutoff hash must appear together",
        ));
    }
    Ok(())
}

fn validate_transaction_id(raw: &str) -> Result<(), ValidationFailure> {
    if is_lowercase_hex(raw, 64) {
        Ok(())
    } else {
        Err(failure(
            "invalid_record",
            "transaction_id must be 32-byte lowercase hex",
        ))
    }
}

fn require_lowercase_uuid(raw: &str) -> Result<(), ()> {
    parse_uuid_str(raw).map_err(|_| ())?;
    if raw.bytes().all(|byte| !byte.is_ascii_uppercase()) {
        Ok(())
    } else {
        Err(())
    }
}

fn is_lowercase_hex(raw: &str, len: usize) -> bool {
    raw.len() == len
        && raw
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn decode_lowercase_hex_32(raw: &str) -> Option<[u8; 32]> {
    if !is_lowercase_hex(raw, 64) {
        return None;
    }
    let mut decoded = [0u8; 32];
    for (index, byte) in decoded.iter_mut().enumerate() {
        let high = decode_hex_nibble(raw.as_bytes()[index * 2])?;
        let low = decode_hex_nibble(raw.as_bytes()[index * 2 + 1])?;
        *byte = high << 4 | low;
    }
    Some(decoded)
}

fn decode_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn enforce_limits(value: &Value, depth: usize) -> Result<(), ()> {
    if depth > MAX_JSON_NESTING_DEPTH {
        return Err(());
    }
    match value {
        Value::Array(items) => {
            if items.len() > MAX_JSON_CONTAINER_ELEMENTS {
                return Err(());
            }
            for item in items {
                enforce_limits(item, depth + 1)?;
            }
        }
        Value::Object(fields) => {
            if fields.len() > MAX_JSON_CONTAINER_ELEMENTS {
                return Err(());
            }
            for item in fields.values() {
                enforce_limits(item, depth + 1)?;
            }
        }
        Value::String(text) if text.len() > MAX_JSON_STRING_BYTES_PER_RECORD => return Err(()),
        _ => {}
    }
    Ok(())
}

fn failure(code: &'static str, message: &'static str) -> ValidationFailure {
    use crate::validate_record::ValidationReasonCode;
    let reason = match code {
        "limit_exceeded" => ValidationReasonCode::LimitExceeded,
        "duplicate_json_key" => ValidationReasonCode::DuplicateJsonKey,
        "integrity_failure" => ValidationReasonCode::IntegrityFailure,
        _ => ValidationReasonCode::InvalidRecord,
    };
    ValidationFailure {
        reason,
        detail: message.to_string(),
        schema_version_if_parsed: Some(SECURITY_V3_SCHEMA_VERSION),
    }
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};

    use super::*;
    use crate::hex::{ContentHashHex, SignatureHex};
    use crate::integrity::signing_key_id_from_verifying_key;

    const SEED: [u8; 32] = [0x37; 32];

    fn signed_record(event_type: &str, payload: Value) -> (Vec<u8>, SigningKey) {
        signed_record_at(event_type, payload, 1, ZERO_CONTENT_HASH_HEX)
    }

    fn signed_record_at(
        event_type: &str,
        payload: Value,
        device_seq: u64,
        prev_content_hash: &str,
    ) -> (Vec<u8>, SigningKey) {
        let key = SigningKey::from_bytes(&SEED);
        let key_id = signing_key_id_from_verifying_key(&key.verifying_key()).unwrap();
        let mut record = serde_json::json!({
            "schema_version": 3,
            "event_id": "11111111-1111-4111-8111-111111111111",
            "brain_id": "22222222-2222-4222-8222-222222222222",
            "device_id": "device-security-test",
            "device_seq": device_seq.to_string(),
            "created_at": "2026-10-29T00:00:00Z",
            "event_type": event_type,
            "classification": "internal",
            "signing_key_id": key_id.as_str(),
            "payload": payload,
            "prev_content_hash": prev_content_hash,
            "content_hash": "00".repeat(32),
            "signature": "00".repeat(64),
        });
        let object = record.as_object_mut().unwrap();
        object.remove("content_hash");
        object.remove("signature");
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
        (serde_json_canonicalizer::to_vec(&record).unwrap(), key)
    }

    fn transaction_id() -> String {
        "ab".repeat(32)
    }

    #[test]
    fn signs_and_reparses_owner_restriction_event() {
        let author = SigningKey::from_bytes(&SEED);
        let target = SigningKey::from_bytes(&[0x11; 32]);
        let author_id = signing_key_id_from_verifying_key(&author.verifying_key()).unwrap();
        let target_id = signing_key_id_from_verifying_key(&target.verifying_key()).unwrap();
        let author_device = DeviceId::new("device-security-test").unwrap();
        let target_device = DeviceId::new("device-target").unwrap();
        let bytes = sign_device_restricted_event(
            "11111111-1111-4111-8111-111111111111",
            "22222222-2222-4222-8222-222222222222",
            &author_device,
            DeviceSeq::from_u64(2).unwrap(),
            "2026-10-29T00:00:00Z",
            &author_id,
            &ContentHashHex::zero(),
            &transaction_id(),
            &target_device,
            &target_id,
            &target.verifying_key().to_bytes(),
            RestrictionReasonV3::Compromised,
            0,
            &ContentHashHex::zero(),
            &author,
        )
        .unwrap();
        let event = read_security_v3_event(&bytes, &author.verifying_key()).unwrap();
        assert_eq!(event.event_type(), SecurityEventTypeV3::DeviceRestricted);
        assert!(matches!(
            event.payload(),
            SecurityPayloadV3::DeviceRestricted {
                target_device_id,
                reason: RestrictionReasonV3::Compromised,
                cutoff_seq: 0,
                ..
            } if target_device_id == &DeviceId::new("device-target").unwrap()
        ));
    }

    #[test]
    fn signs_self_retirement_and_rejects_nonpreceding_cutoff() {
        let author = SigningKey::from_bytes(&SEED);
        let author_id = signing_key_id_from_verifying_key(&author.verifying_key()).unwrap();
        let author_device = DeviceId::new("device-security-test").unwrap();
        let event_id = "11111111-1111-4111-8111-111111111111";
        let brain_id = "22222222-2222-4222-8222-222222222222";
        let created_at = "2026-10-29T00:00:00Z";
        let transaction_id = transaction_id();
        let bytes = sign_device_self_retired_event(
            event_id,
            brain_id,
            &author_device,
            DeviceSeq::from_u64(3).unwrap(),
            created_at,
            &author_id,
            &ContentHashHex::zero(),
            &transaction_id,
            1,
            &ContentHashHex::parse(&"44".repeat(32)).unwrap(),
            &author,
        )
        .unwrap();
        let event = read_security_v3_event(&bytes, &author.verifying_key()).unwrap();
        assert_eq!(event.event_type(), SecurityEventTypeV3::DeviceSelfRetired);
        assert!(
            sign_device_self_retired_event(
                event_id,
                brain_id,
                &author_device,
                DeviceSeq::from_u64(3).unwrap(),
                created_at,
                &author_id,
                &ContentHashHex::zero(),
                &transaction_id,
                3,
                &ContentHashHex::parse(&"44".repeat(32)).unwrap(),
                &author,
            )
            .is_err()
        );
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn verifies_strict_self_retirement_event() {
        let (bytes, key) = signed_record(
            "security.device_self_retired",
            serde_json::json!({
                "transaction_id": transaction_id(),
                "last_authorized_seq": "0",
                "last_authorized_content_hash": ZERO_CONTENT_HASH_HEX
            }),
        );
        let event = read_security_v3_event(&bytes, &key.verifying_key()).unwrap();
        assert_eq!(event.device_seq.to_u64().unwrap(), 1);
        assert!(matches!(
            event.payload(),
            SecurityPayloadV3::DeviceSelfRetired {
                last_authorized_seq: 0,
                ..
            }
        ));
    }

    #[test]
    fn operation_digest_is_stable_across_append_position_but_binds_payload() {
        let target = SigningKey::from_bytes(&[0x22; 32]);
        let target_key_id = signing_key_id_from_verifying_key(&target.verifying_key()).unwrap();
        let payload = serde_json::json!({
            "transaction_id": transaction_id(),
            "target_device_id": "device-target",
            "target_signing_key_id": target_key_id.as_str(),
            "target_public_key": hex(&target.verifying_key().to_bytes()),
            "reason": "retired",
            "cutoff_seq": "0",
            "cutoff_content_hash": ZERO_CONTENT_HASH_HEX
        });
        let (first, key) = signed_record_at(
            "security.device_restricted",
            payload.clone(),
            1,
            ZERO_CONTENT_HASH_HEX,
        );
        let (retry, _) =
            signed_record_at("security.device_restricted", payload, 7, &"44".repeat(32));
        let first = read_security_v3_event(&first, &key.verifying_key()).unwrap();
        let retry = read_security_v3_event(&retry, &key.verifying_key()).unwrap();
        assert_eq!(first.operation_digest(), retry.operation_digest());

        let (changed, _) = signed_record(
            "security.device_restricted",
            serde_json::json!({
                "transaction_id": transaction_id(),
                "target_device_id": "device-target",
                "target_signing_key_id": target_key_id.as_str(),
                "target_public_key": hex(&target.verifying_key().to_bytes()),
                "reason": "compromised",
                "cutoff_seq": "0",
                "cutoff_content_hash": ZERO_CONTENT_HASH_HEX
            }),
        );
        let changed = read_security_v3_event(&changed, &key.verifying_key()).unwrap();
        assert_ne!(first.operation_digest(), changed.operation_digest());
    }

    #[test]
    fn verifies_restriction_payload_shape_and_cutoff_pair() {
        let target = SigningKey::from_bytes(&[0x11; 32]);
        let target_verifying_key = target.verifying_key();
        let target_key_id = signing_key_id_from_verifying_key(&target_verifying_key).unwrap();
        let (bytes, key) = signed_record(
            "security.device_restricted",
            serde_json::json!({
                "transaction_id": transaction_id(),
                "target_device_id": "device-target",
                "target_signing_key_id": target_key_id.as_str(),
                "target_public_key": hex(&target_verifying_key.to_bytes()),
                "reason": "retired",
                "cutoff_seq": "7",
                "cutoff_content_hash": "33".repeat(32)
            }),
        );
        assert!(matches!(
            read_security_v3_event(&bytes, &key.verifying_key())
                .unwrap()
                .payload(),
            SecurityPayloadV3::DeviceRestricted {
                cutoff_seq: 7,
                reason: RestrictionReasonV3::Retired,
                ..
            }
        ));
    }

    #[test]
    fn peer_approval_requires_four_message_transcript_container() {
        let (bytes, key) = signed_record(
            "security.peer_approved",
            serde_json::json!({
                "transaction_id": transaction_id(),
                "transcript": [{}, {}, {}, {}]
            }),
        );
        assert!(matches!(
            read_security_v3_event(&bytes, &key.verifying_key())
                .unwrap()
                .payload(),
            SecurityPayloadV3::PeerApproved { .. }
        ));
        let (bad, key) = signed_record(
            "security.peer_approved",
            serde_json::json!({"transaction_id": transaction_id(), "transcript": [{}, {}, {}]}),
        );
        assert!(read_security_v3_event(&bad, &key.verifying_key()).is_err());
    }

    #[test]
    fn rejects_wrong_key_and_noncanonical_bytes() {
        let (bytes, key) = signed_record(
            "security.device_self_retired",
            serde_json::json!({
                "transaction_id": transaction_id(),
                "last_authorized_seq": "0",
                "last_authorized_content_hash": ZERO_CONTENT_HASH_HEX
            }),
        );
        let wrong_key = SigningKey::from_bytes(&[0x38; 32]);
        assert!(read_security_v3_event(&bytes, &wrong_key.verifying_key()).is_err());
        assert!(read_security_v3_event(&bytes, &key.verifying_key()).is_ok());
        let noncanonical = [b" ".as_slice(), bytes.as_slice()].concat();
        assert!(read_security_v3_event(&noncanonical, &key.verifying_key()).is_err());
    }

    #[test]
    fn rejects_tampered_content_hash_and_signature() {
        let (bytes, key) = signed_record(
            "security.device_self_retired",
            serde_json::json!({
                "transaction_id": transaction_id(),
                "last_authorized_seq": "0",
                "last_authorized_content_hash": ZERO_CONTENT_HASH_HEX
            }),
        );
        let mut bad_hash: Value = serde_json::from_slice(&bytes).unwrap();
        bad_hash
            .as_object_mut()
            .unwrap()
            .insert("content_hash".into(), Value::String("aa".repeat(32)));
        let bad_hash = serde_json_canonicalizer::to_vec(&bad_hash).unwrap();
        assert!(read_security_v3_event(&bad_hash, &key.verifying_key()).is_err());

        let mut bad_signature: Value = serde_json::from_slice(&bytes).unwrap();
        bad_signature
            .as_object_mut()
            .unwrap()
            .insert("signature".into(), Value::String("00".repeat(64)));
        let bad_signature = serde_json_canonicalizer::to_vec(&bad_signature).unwrap();
        assert!(read_security_v3_event(&bad_signature, &key.verifying_key()).is_err());
    }

    #[test]
    fn rejects_unknown_fields_and_event_types() {
        let (bytes, key) = signed_record(
            "security.device_self_retired",
            serde_json::json!({
                "transaction_id": transaction_id(),
                "last_authorized_seq": "0",
                "last_authorized_content_hash": ZERO_CONTENT_HASH_HEX,
                "extra": true
            }),
        );
        assert!(read_security_v3_event(&bytes, &key.verifying_key()).is_err());
        let (bytes, key) = signed_record(
            "security.future_policy",
            serde_json::json!({"transaction_id": transaction_id()}),
        );
        assert!(read_security_v3_event(&bytes, &key.verifying_key()).is_err());
    }

    #[test]
    fn rejects_noncanonical_uuid_and_transaction_id() {
        let (bytes, key) = signed_record(
            "security.device_self_retired",
            serde_json::json!({
                "transaction_id": transaction_id(),
                "last_authorized_seq": "0",
                "last_authorized_content_hash": ZERO_CONTENT_HASH_HEX
            }),
        );
        let mut uppercase_uuid: Value = serde_json::from_slice(&bytes).unwrap();
        uppercase_uuid.as_object_mut().unwrap().insert(
            "event_id".into(),
            Value::String("AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA".into()),
        );
        let uppercase_uuid = serde_json_canonicalizer::to_vec(&uppercase_uuid).unwrap();
        assert!(read_security_v3_event(&uppercase_uuid, &key.verifying_key()).is_err());

        let (bad_transaction, key) = signed_record(
            "security.device_self_retired",
            serde_json::json!({
                "transaction_id": "AB".repeat(32),
                "last_authorized_seq": "0",
                "last_authorized_content_hash": ZERO_CONTENT_HASH_HEX
            }),
        );
        assert!(read_security_v3_event(&bad_transaction, &key.verifying_key()).is_err());
    }

    #[test]
    fn self_retirement_boundary_must_precede_control_event() {
        let (bytes, key) = signed_record(
            "security.device_self_retired",
            serde_json::json!({
                "transaction_id": transaction_id(),
                "last_authorized_seq": "1",
                "last_authorized_content_hash": "55".repeat(32)
            }),
        );
        assert!(read_security_v3_event(&bytes, &key.verifying_key()).is_err());
    }

    #[test]
    fn rejects_duplicate_keys_and_nonzero_hash_for_zero_cutoff() {
        let (bytes, key) = signed_record(
            "security.device_self_retired",
            serde_json::json!({
                "transaction_id": transaction_id(),
                "last_authorized_seq": "0",
                "last_authorized_content_hash": "44".repeat(32)
            }),
        );
        assert!(read_security_v3_event(&bytes, &key.verifying_key()).is_err());
        let duplicate = br#"{"schema_version":3,"schema_version":3}"#;
        assert!(read_security_v3_event(duplicate, &key.verifying_key()).is_err());
    }

    #[test]
    fn versioned_dispatch_authenticates_v3_and_chain_path_checks_identity() {
        let (bytes, key) = signed_record(
            "security.device_self_retired",
            serde_json::json!({
                "transaction_id": transaction_id(),
                "last_authorized_seq": "0",
                "last_authorized_content_hash": ZERO_CONTENT_HASH_HEX
            }),
        );
        let dispatched =
            crate::versioned_reader::read_versioned_record(&bytes, &key.verifying_key()).unwrap();
        let crate::versioned_reader::VersionedRecord::VerifiedSecurityV3(authenticated) =
            dispatched
        else {
            panic!("schema v3 should dispatch to its strict authenticated parser");
        };
        let mut chain =
            crate::versioned_reader::VersionedChainVerifier::new("device-security-test");
        chain
            .verify_next(
                &crate::versioned_reader::VersionedRecord::VerifiedSecurityV3(
                    authenticated.clone(),
                ),
            )
            .unwrap();
        assert!(
            crate::versioned_reader::VersionedChainVerifier::new("another-device")
                .verify_security_v3_next(&authenticated)
                .is_err()
        );
    }
}
