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
use crate::v2_payload::{KnowledgeAssertionPayload, V2KnowledgeAssertionEvent};
use crate::validate_record::{ValidationFailure, validate_accepted_record_bytes};

pub const EVENT_SCHEMA_VERSION_V2: u32 = 2;
pub const V2_CONTENT_HASH_DOMAIN: &[u8] = b"cryptex-event-content-v2";
pub const V2_SIGNATURE_DOMAIN: &[u8] = b"cryptex-event-sig-v2";

/// A known and fully validated v2 knowledge assertion. M3 consumers still may
/// not apply it; M4 materialization owns its semantics.
#[derive(Debug, Clone, PartialEq)]
pub struct VerifiedV2KnowledgeAssertion {
    event: V2KnowledgeAssertionEvent,
    raw_record: Vec<u8>,
}

impl VerifiedV2KnowledgeAssertion {
    /// Return the typed event that was validated against `raw_record`.
    #[must_use]
    pub const fn event(&self) -> &V2KnowledgeAssertionEvent {
        &self.event
    }

    /// Return the exact canonical JCS bytes that passed hash/signature checks.
    #[must_use]
    pub fn raw_record(&self) -> &[u8] {
        &self.raw_record
    }
}

/// Complete event history whose records passed signature/content verification
/// and per-device cross-version chain verification using keys resolved by a
/// trusted-daemon key resolver.
///
/// A publication receipt UUID is retained as provenance; it does not itself
/// prove human intent. The trusted-daemon key resolver is the trust boundary:
/// it must return a key only when the device/key binding is trusted as a
/// Cryptex daemon attestation under the caller's enrolled-device policy.
#[derive(Debug, Clone, PartialEq)]
pub struct VerifiedV2History {
    records: Vec<VersionedRecord>,
    assertions: Vec<VerifiedV2KnowledgeAssertion>,
}

/// Supplies only public keys trusted to attest canonical events on behalf of
/// an enrolled Cryptex daemon. Implementations must not trust keys merely
/// because they were embedded in or requested by an event.
pub trait TrustedDaemonKeyResolver {
    fn trusted_daemon_key(&mut self, device_id: &str, key_id: &str) -> Option<VerifyingKey>;

    /// Return this installation's locally held identity anchor when the
    /// resolver represents one. Generic trusted-key resolvers remain
    /// compatible and do not enable local-anchor policy scanning by default.
    fn local_trust_anchor(&self) -> Option<LocalTrustAnchor> {
        None
    }
}

/// Local identity material used to derive peer bindings from signed local
/// approval anchors. This carries only a public key and never grants trust on
/// its own; the key ID is checked against the key before policy verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalTrustAnchor {
    device_id: DeviceId,
    signing_key_id: SigningKeyId,
    public_key: [u8; 32],
}

impl LocalTrustAnchor {
    #[must_use]
    pub const fn new(
        device_id: DeviceId,
        signing_key_id: SigningKeyId,
        public_key: [u8; 32],
    ) -> Self {
        Self {
            device_id,
            signing_key_id,
            public_key,
        }
    }

    #[must_use]
    pub const fn device_id(&self) -> &DeviceId {
        &self.device_id
    }

    #[must_use]
    pub const fn signing_key_id(&self) -> &SigningKeyId {
        &self.signing_key_id
    }

    #[must_use]
    pub const fn public_key(&self) -> &[u8; 32] {
        &self.public_key
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CancellableHistoryVerificationError {
    InvalidRecord(ValidationFailure),
    Cancelled,
}

impl From<ValidationFailure> for CancellableHistoryVerificationError {
    fn from(value: ValidationFailure) -> Self {
        Self::InvalidRecord(value)
    }
}

impl VerifiedV2History {
    pub(crate) fn from_authenticated_content(
        records: Vec<VersionedRecord>,
        assertions: Vec<VerifiedV2KnowledgeAssertion>,
    ) -> Self {
        Self {
            records,
            assertions,
        }
    }

    #[must_use]
    pub fn records(&self) -> &[VersionedRecord] {
        &self.records
    }

    #[must_use]
    pub fn assertions(&self) -> &[VerifiedV2KnowledgeAssertion] {
        &self.assertions
    }
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

/// Read classification. Verified payload integrity is not factual truth and
/// is not publication authorization; consumers still apply their own policy.
#[derive(Debug, Clone, PartialEq)]
pub enum VersionedRecord {
    VerifiedV1(AcceptedEvent),
    VerifiedV2Supported(VerifiedV2KnowledgeAssertion),
    VerifiedUnsupported(VerifiedUnsupportedV2),
    /// Authenticated M8 security control event. This is chain data, not a
    /// retrieval fact or publication authorization by itself.
    VerifiedSecurityV3(crate::security_v3::SecurityEventV3),
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
            VersionedRecord::VerifiedV2Supported(record) => (
                record.event.device_id.as_str(),
                record
                    .event
                    .device_seq
                    .to_u64()
                    .map_err(|_| failure("invalid_record", "device_seq is invalid", Some(2)))?,
                &record.event.prev_content_hash,
                &record.event.content_hash,
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
            VersionedRecord::VerifiedSecurityV3(event) => (
                event.device_id().as_str(),
                event
                    .device_seq()
                    .to_u64()
                    .map_err(|_| failure("invalid_record", "v3 device_seq is invalid", Some(3)))?,
                event.prev_content_hash(),
                event.content_hash(),
                3,
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

    /// Verify and advance with a separately authenticated M8 v3 security
    /// record. The normal dispatch reader intentionally does not classify v3
    /// as verified, so callers must explicitly use the strict v3 parser first.
    pub fn verify_security_v3_next(
        &mut self,
        record: &crate::security_v3::SecurityEventV3,
    ) -> Result<(), ValidationFailure> {
        let seq = record.device_seq().to_u64().map_err(|_| {
            failure(
                "invalid_record",
                "v3 device_seq is invalid",
                Some(crate::security_v3::SECURITY_V3_SCHEMA_VERSION.into()),
            )
        })?;
        if record.device_id().as_str() != self.expected_device_id {
            return Err(failure(
                "integrity_failure",
                "v3 event device_id does not match the device chain",
                Some(crate::security_v3::SECURITY_V3_SCHEMA_VERSION.into()),
            ));
        }
        if let Some(prior_seq) = self.last_seq {
            if seq <= prior_seq || record.prev_content_hash() != &self.last_hash {
                return Err(failure(
                    "integrity_failure",
                    "v3 device sequence or cross-version hash-chain link is invalid",
                    Some(crate::security_v3::SECURITY_V3_SCHEMA_VERSION.into()),
                ));
            }
        } else if record.prev_content_hash().as_str() != crate::hex::ZERO_CONTENT_HASH_HEX {
            return Err(failure(
                "integrity_failure",
                "first device event must use the zero previous-hash sentinel",
                Some(crate::security_v3::SECURITY_V3_SCHEMA_VERSION.into()),
            ));
        }
        self.last_seq = Some(seq);
        self.last_hash = record.content_hash().clone();
        Ok(())
    }
}

/// Verify a complete set of canonical record bytes in per-device stream order.
/// The resolver must bind each key to a trusted Cryptex daemon for that device.
/// This establishes daemon attestation only: the receipt UUID remains a
/// reference and is not independent proof that a human performed the action.
/// An unverifiable record, conflicting event ID, or broken link rejects the
/// whole history; verified unsupported records remain present in the result.
pub fn verify_v2_history<R>(
    raw_records: &[&[u8]],
    key_resolver: R,
) -> Result<VerifiedV2History, ValidationFailure>
where
    R: TrustedDaemonKeyResolver,
{
    match verify_v2_history_cancellable(raw_records, key_resolver, || false) {
        Ok(history) => Ok(history),
        Err(CancellableHistoryVerificationError::InvalidRecord(error)) => Err(error),
        Err(CancellableHistoryVerificationError::Cancelled) => {
            unreachable!("the non-cancellable verifier never cancels")
        }
    }
}

/// Verify a complete history while allowing a bounded caller to stop between
/// records. Cancellation is distinct from invalid canonical data and never
/// returns a partial `VerifiedV2History`.
pub fn verify_v2_history_cancellable<R, C>(
    raw_records: &[&[u8]],
    mut key_resolver: R,
    mut cancelled: C,
) -> Result<VerifiedV2History, CancellableHistoryVerificationError>
where
    R: TrustedDaemonKeyResolver,
    C: FnMut() -> bool,
{
    let mut verifiers = std::collections::BTreeMap::<String, VersionedChainVerifier>::new();
    let mut seen_event_bytes = std::collections::BTreeMap::<String, Vec<u8>>::new();
    let mut records = Vec::with_capacity(raw_records.len());
    let mut assertions = Vec::new();
    for bytes in raw_records {
        if cancelled() {
            return Err(CancellableHistoryVerificationError::Cancelled);
        }
        let value: serde_json::Value = serde_json::from_slice(bytes)
            .map_err(|_| failure("invalid_record", "record JSON is malformed", None))?;
        // Event IDs are UUIDs and therefore case-insensitive identifiers.
        // Exact replica bytes may be skipped; differing bytes are compared
        // only after both records have passed signature and chain checks.
        let canonical_event_id = value
            .get("event_id")
            .and_then(serde_json::Value::as_str)
            .map(|event_id| {
                parse_uuid_str(event_id)
                    .map(|uuid| uuid.to_ascii_lowercase())
                    .map_err(|_| {
                        failure(
                            "invalid_record",
                            "record event_id is invalid",
                            peek_version(&value),
                        )
                    })
            })
            .transpose()?;
        if let Some(event_id) = &canonical_event_id
            && seen_event_bytes
                .get(event_id)
                .is_some_and(|previous| previous.as_slice() == *bytes)
        {
            continue;
        }
        let device_id = value
            .get("device_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| failure("invalid_record", "record device_id is unavailable", None))?;
        let key_id = value
            .get("signing_key_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| failure("invalid_record", "record key id is unavailable", None))?;
        let verifying_key = key_resolver
            .trusted_daemon_key(device_id, key_id)
            .ok_or_else(|| {
                failure(
                    "integrity_failure",
                    "trusted daemon signing key is unavailable for the record",
                    peek_version(&value),
                )
            })?;
        let record = read_versioned_record(bytes, &verifying_key)?;
        let actual_device_id = match &record {
            VersionedRecord::VerifiedV1(event) => event.device_id.as_str(),
            VersionedRecord::VerifiedV2Supported(record) => record.event.device_id.as_str(),
            VersionedRecord::VerifiedUnsupported(record) => record.device_id.as_str(),
            VersionedRecord::VerifiedSecurityV3(record) => record.device_id().as_str(),
            VersionedRecord::Unverifiable(_) => {
                return Err(failure(
                    "invalid_record",
                    "unverifiable envelope blocks complete history verification",
                    peek_version(&value),
                )
                .into());
            }
        };
        if actual_device_id != device_id {
            return Err(failure(
                "integrity_failure",
                "record device_id changed during verification",
                peek_version(&value),
            )
            .into());
        }
        let verifier = verifiers
            .entry(actual_device_id.to_owned())
            .or_insert_with(|| VersionedChainVerifier::new(actual_device_id));
        verifier.verify_next(&record)?;
        if let Some(canonical_event_id) = canonical_event_id {
            if seen_event_bytes.contains_key(&canonical_event_id) {
                return Err(failure(
                    "integrity_failure",
                    "different verified records share the same event_id",
                    peek_version(&value),
                )
                .into());
            }
            seen_event_bytes.insert(canonical_event_id, bytes.to_vec());
        }
        if let VersionedRecord::VerifiedV2Supported(assertion) = &record {
            assertions.push(assertion.clone());
        }
        records.push(record);
    }
    Ok(VerifiedV2History {
        records,
        assertions,
    })
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
        Some(v) if v == u64::from(crate::security_v3::SECURITY_V3_SCHEMA_VERSION) => {
            crate::security_v3::read_security_v3_event(bytes, verifying_key)
                .map(VersionedRecord::VerifiedSecurityV3)
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

    if envelope.event_type == "knowledge.assertion" && envelope.payload_schema_version == 1 {
        let payload =
            KnowledgeAssertionPayload::parse_and_validate(envelope.payload).map_err(|_| {
                failure(
                    "invalid_record",
                    "known v2 payload failed strict validation",
                    Some(2),
                )
            })?;
        let event = V2KnowledgeAssertionEvent {
            schema_version: envelope.schema_version,
            event_id: envelope.event_id,
            brain_id: envelope.brain_id,
            device_id: envelope.device_id,
            device_seq: envelope.device_seq,
            created_at: envelope.created_at,
            event_type: crate::v2_payload::KnowledgeAssertionEventType::KnowledgeAssertion,
            payload_schema_version: envelope.payload_schema_version,
            classification: envelope.classification,
            signing_key_id: envelope.signing_key_id,
            payload,
            prev_content_hash: envelope.prev_content_hash,
            content_hash: envelope.content_hash,
            signature: envelope.signature,
        };
        event.validate().map_err(|_| {
            failure(
                "invalid_record",
                "known v2 event failed strict validation",
                Some(2),
            )
        })?;
        return Ok(VersionedRecord::VerifiedV2Supported(
            VerifiedV2KnowledgeAssertion {
                event,
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

    struct TestTrustedDaemonKeyResolver(VerifyingKey);

    impl TrustedDaemonKeyResolver for TestTrustedDaemonKeyResolver {
        fn trusted_daemon_key(&mut self, _device_id: &str, _key_id: &str) -> Option<VerifyingKey> {
            Some(self.0)
        }
    }

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
        make_v2_with_id(
            "11111111-1111-4111-8111-111111111111",
            event_type,
            payload_schema_version,
            device_seq,
            prev_content_hash,
        )
    }

    fn make_v2_with_id(
        event_id: &str,
        event_type: &str,
        payload_schema_version: u32,
        device_seq: u64,
        prev_content_hash: &str,
    ) -> (Vec<u8>, SigningKey) {
        let key = SigningKey::from_bytes(&SEED);
        let key_id = signing_key_id_from_verifying_key(&key.verifying_key()).unwrap();
        let mut record = serde_json::json!({
            "schema_version": 2,
            "event_id": event_id,
            "brain_id": "22222222-2222-4222-8222-222222222222",
            "device_id": "device-reader-test",
            "device_seq": device_seq.to_string(),
            "created_at": "2026-10-08T00:00:00Z",
            "event_type": event_type,
            "payload_schema_version": payload_schema_version,
            "classification": "internal",
            "signing_key_id": key_id.as_str(),
            "payload": if event_type == "knowledge.assertion" && payload_schema_version == 1 {
                serde_json::json!({
                    "kind":"entity.declare",
                    "entity_id":"33333333-3333-4333-8333-333333333333",
                    "entity_kind":"person",
                    "label":"Synthetic",
                    "publication_authorization":{
                        "basis":"direct_user",
                        "receipt_id":"44444444-4444-4444-8444-444444444444"
                    }
                })
            } else {
                serde_json::json!({"kind": "test", "value": "synthetic"})
            },
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

    fn resign_v2_record(record: &mut Value, key: &SigningKey) {
        let object = record.as_object_mut().unwrap();
        object.remove("content_hash");
        object.remove("signature");
        let canonical = serde_json_canonicalizer::to_vec(record).unwrap();
        let mut hasher = blake3::Hasher::new();
        hasher.update(V2_CONTENT_HASH_DOMAIN);
        hasher.update(&[0]);
        hasher.update(&canonical);
        let hash = *hasher.finalize().as_bytes();
        let signature = key.sign(&[V2_SIGNATURE_DOMAIN, &[0], hash.as_slice()].concat());
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
    }

    fn change_v2_device_id(bytes: &[u8], device_id: &str, key: &SigningKey) -> Vec<u8> {
        let mut record: Value = serde_json::from_slice(bytes).unwrap();
        record["device_id"] = Value::String(device_id.to_owned());
        resign_v2_record(&mut record, key);
        serde_json_canonicalizer::to_vec(&record).unwrap()
    }

    #[test]
    fn known_v2_payload_is_strictly_validated_and_preserved() {
        let (bytes, key) = make_v2("knowledge.assertion", 1);
        let result = read_versioned_record(&bytes, &key.verifying_key()).unwrap();
        let VersionedRecord::VerifiedV2Supported(record) = result else {
            panic!("expected validated v2 knowledge assertion");
        };
        assert_eq!(record.raw_record, bytes);
        assert_eq!(
            record.event.event_id,
            "11111111-1111-4111-8111-111111111111"
        );
        assert!(matches!(
            record.event.payload,
            KnowledgeAssertionPayload::EntityDeclare { .. }
        ));
        let typed: V2KnowledgeAssertionEvent = serde_json::from_slice(&bytes).unwrap();
        typed.validate().unwrap();
    }

    #[test]
    fn malformed_known_v2_payload_is_invalid_not_verified_unsupported() {
        let (mut bytes, key) = make_v2("knowledge.assertion", 1);
        let mut record: Value = serde_json::from_slice(&bytes).unwrap();
        record["payload"] = serde_json::json!({
            "kind":"entity.declare",
            "entity_id":"33333333-3333-4333-8333-333333333333",
            "entity_kind":"person",
            "label":"Synthetic",
            "unknown":true,
            "publication_authorization":{
                "basis":"direct_user",
                "receipt_id":"44444444-4444-4444-8444-444444444444"
            }
        });
        resign_v2_record(&mut record, &key);
        bytes = serde_json_canonicalizer::to_vec(&record).unwrap();
        let error = read_versioned_record(&bytes, &key.verifying_key()).unwrap_err();
        assert_eq!(error.reason, crate::ValidationReasonCode::InvalidRecord);
        assert!(!error.detail.contains("Synthetic"));
        assert!(!error.detail.contains("unknown"));
    }

    #[test]
    fn known_v2_event_type_and_payload_schema_are_exact() {
        let (bytes, key) = make_v2("Knowledge.assertion", 1);
        assert!(matches!(
            read_versioned_record(&bytes, &key.verifying_key()).unwrap(),
            VersionedRecord::VerifiedUnsupported(_)
        ));
        let (bytes, key) = make_v2("knowledge.assertion", 0);
        assert!(read_versioned_record(&bytes, &key.verifying_key()).is_err());
        let (bytes, key) = make_v2("knowledge.assertion", 1);
        let mut raw: Value = serde_json::from_slice(&bytes).unwrap();
        raw["payload_schema_version"] = Value::from(2);
        resign_v2_record(&mut raw, &key);
        let future = serde_json_canonicalizer::to_vec(&raw).unwrap();
        assert!(matches!(
            read_versioned_record(&future, &key.verifying_key()).unwrap(),
            VersionedRecord::VerifiedUnsupported(_)
        ));
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

    #[test]
    fn cancelled_history_verification_returns_no_partial_history() {
        let (raw, key) = make_v2("knowledge.assertion", 1);
        let error = verify_v2_history_cancellable(
            &[&raw],
            TestTrustedDaemonKeyResolver(key.verifying_key()),
            || true,
        )
        .expect_err("cancelled verification must not return a partial history");
        assert_eq!(error, CancellableHistoryVerificationError::Cancelled);
    }

    #[test]
    fn verified_history_requires_genesis_link_and_deduplicates_exact_replica_bytes() {
        let (first, key) = make_v2_with(
            "knowledge.assertion",
            1,
            1,
            crate::hex::ZERO_CONTENT_HASH_HEX,
        );
        let first_record = read_versioned_record(&first, &key.verifying_key()).unwrap();
        let VersionedRecord::VerifiedV2Supported(first_event) = first_record else {
            panic!("expected supported first v2 record");
        };
        let (second, _) = make_v2_with_id(
            "55555555-5555-4555-8555-555555555555",
            "knowledge.assertion",
            1,
            3,
            first_event.event.content_hash.as_str(),
        );
        let history = verify_v2_history(
            &[&first, &first, &second],
            TestTrustedDaemonKeyResolver(key.verifying_key()),
        )
        .unwrap();
        assert_eq!(history.records().len(), 2);
        assert_eq!(history.assertions().len(), 2);

        let truncated = verify_v2_history(
            &[&second],
            TestTrustedDaemonKeyResolver(key.verifying_key()),
        );
        assert_eq!(
            truncated.unwrap_err().reason,
            crate::ValidationReasonCode::IntegrityFailure
        );
    }

    #[test]
    fn differing_supported_and_unsupported_v2_records_with_same_id_fail_closed() {
        let (unsupported, key) = make_v2("future.event", 1);
        let (supported, _) = make_v2("knowledge.assertion", 1);
        let supported = change_v2_device_id(&supported, "device-other", &key);

        let error = verify_v2_history(
            &[&unsupported, &supported],
            TestTrustedDaemonKeyResolver(key.verifying_key()),
        )
        .unwrap_err();

        assert_eq!(error.reason, crate::ValidationReasonCode::IntegrityFailure);
        assert!(error.detail.contains("different verified records"));
    }

    #[test]
    fn differing_v1_and_v2_records_with_same_id_fail_before_reducer_filtering() {
        let key = SigningKey::from_bytes(&SEED);
        let key_id = signing_key_id_from_verifying_key(&key.verifying_key()).unwrap();
        let v1 = accept_event(
            SignableEvent {
                schema_version: EVENT_SCHEMA_VERSION,
                event_id: "11111111-1111-4111-8111-111111111111".into(),
                brain_id: "22222222-2222-4222-8222-222222222222".into(),
                device_id: "device-reader-test".into(),
                device_seq: DeviceSeq::from_u64(1).unwrap(),
                created_at: "2026-10-08T00:00:00Z".into(),
                event_type: EventType::MemoryNote,
                classification: Classification::Internal,
                signing_key_id: key_id,
                payload: EventPayload::MemoryNote(MemoryNotePayload::new("synthetic v1").unwrap()),
                prev_content_hash: ContentHashHex::zero(),
            },
            &key,
        )
        .unwrap();
        let v1 = serde_json_canonicalizer::to_vec(&v1).unwrap();
        let (v2, _) = make_v2("knowledge.assertion", 1);
        let v2 = change_v2_device_id(&v2, "device-other", &key);

        let error = verify_v2_history(
            &[&v1, &v2],
            TestTrustedDaemonKeyResolver(key.verifying_key()),
        )
        .unwrap_err();

        assert_eq!(error.reason, crate::ValidationReasonCode::IntegrityFailure);
        assert!(error.detail.contains("different verified records"));
    }

    #[test]
    fn trusted_daemon_signature_does_not_claim_receipt_proves_human_action() {
        let (bytes, key) = make_v2("knowledge.assertion", 1);
        let history =
            verify_v2_history(&[&bytes], TestTrustedDaemonKeyResolver(key.verifying_key()))
                .unwrap();

        assert_eq!(history.assertions().len(), 1);
        let KnowledgeAssertionPayload::EntityDeclare {
            publication_authorization,
            ..
        } = &history.assertions()[0].event().payload
        else {
            panic!("expected synthetic entity declaration");
        };
        assert_eq!(
            publication_authorization.receipt_id,
            "44444444-4444-4444-8444-444444444444"
        );
    }
}
