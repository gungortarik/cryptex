//! Accepted-record validation for schema_version 1 (ADR-0016).
//!
//! M2 uses an exact schema window: `schema_version` must equal 1. Unknown fields,
//! duplicate keys, and integrity failures are typed and visible.

use ed25519_dalek::VerifyingKey;
use serde_json::Value;

use crate::envelope::{AcceptedEvent, EVENT_SCHEMA_VERSION};
use crate::integrity::{reject_duplicate_json_keys, verify_accepted_event};
use crate::limits::{
    MAX_ACCEPTED_RECORD_BYTES, MAX_JSON_CONTAINER_ELEMENTS, MAX_JSON_NESTING_DEPTH,
    MAX_JSON_STRING_BYTES_PER_RECORD,
};
use crate::{EventError, EventResult};

/// Machine-oriented reason codes for quarantine / diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationReasonCode {
    UnsupportedSchema,
    DuplicateJsonKey,
    UnknownField,
    LimitExceeded,
    IntegrityFailure,
    InvalidRecord,
}

impl ValidationReasonCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedSchema => "unsupported_schema",
            Self::DuplicateJsonKey => "duplicate_json_key",
            Self::UnknownField => "unknown_field",
            Self::LimitExceeded => "limit_exceeded",
            Self::IntegrityFailure => "integrity_failure",
            Self::InvalidRecord => "invalid_record",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationFailure {
    pub reason: ValidationReasonCode,
    pub detail: String,
    pub schema_version_if_parsed: Option<u32>,
}

impl std::fmt::Display for ValidationFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.reason.as_str(), self.detail)
    }
}

/// Validate UTF-8 accepted-event JSON bytes for M2 exact schema_version == 1.
pub fn validate_accepted_record_bytes(
    bytes: &[u8],
    verifying_key: &VerifyingKey,
) -> Result<AcceptedEvent, ValidationFailure> {
    if bytes.len() > MAX_ACCEPTED_RECORD_BYTES as usize {
        return Err(ValidationFailure {
            reason: ValidationReasonCode::LimitExceeded,
            detail: format!("record exceeds max {} bytes", MAX_ACCEPTED_RECORD_BYTES),
            schema_version_if_parsed: None,
        });
    }
    let text = std::str::from_utf8(bytes).map_err(|err| ValidationFailure {
        reason: ValidationReasonCode::InvalidRecord,
        detail: format!("record is not valid UTF-8: {err}"),
        schema_version_if_parsed: None,
    })?;

    reject_duplicate_json_keys(text).map_err(|err| ValidationFailure {
        reason: ValidationReasonCode::DuplicateJsonKey,
        detail: err.to_string(),
        schema_version_if_parsed: None,
    })?;

    let value: Value = serde_json::from_str(text).map_err(|err| ValidationFailure {
        reason: ValidationReasonCode::InvalidRecord,
        detail: format!("JSON parse failed: {err}"),
        schema_version_if_parsed: None,
    })?;

    enforce_json_limits(&value, 0).map_err(|err| match err {
        EventError::LimitExceeded(detail) => ValidationFailure {
            reason: ValidationReasonCode::LimitExceeded,
            detail,
            schema_version_if_parsed: peek_schema_version(&value),
        },
        other => ValidationFailure {
            reason: ValidationReasonCode::InvalidRecord,
            detail: other.to_string(),
            schema_version_if_parsed: peek_schema_version(&value),
        },
    })?;

    let schema_version = peek_schema_version(&value);
    if let Some(found) = schema_version {
        if found != EVENT_SCHEMA_VERSION {
            return Err(ValidationFailure {
                reason: ValidationReasonCode::UnsupportedSchema,
                detail: format!(
                    "unsupported event schema_version {found} (expected {EVENT_SCHEMA_VERSION})"
                ),
                schema_version_if_parsed: Some(found),
            });
        }
    }

    let accepted: AcceptedEvent = serde_json::from_value(value.clone()).map_err(|err| {
        let msg = err.to_string();
        let reason = if msg.contains("unknown") || msg.contains("does not match") {
            ValidationReasonCode::UnknownField
        } else {
            ValidationReasonCode::InvalidRecord
        };
        ValidationFailure {
            reason,
            detail: msg,
            schema_version_if_parsed: schema_version,
        }
    })?;

    accepted.validate().map_err(|err| match err {
        EventError::UnsupportedSchema { found, expected } => ValidationFailure {
            reason: ValidationReasonCode::UnsupportedSchema,
            detail: format!("unsupported event schema_version {found} (expected {expected})"),
            schema_version_if_parsed: Some(found),
        },
        EventError::LimitExceeded(detail) => ValidationFailure {
            reason: ValidationReasonCode::LimitExceeded,
            detail,
            schema_version_if_parsed: Some(accepted.schema_version),
        },
        other => ValidationFailure {
            reason: ValidationReasonCode::InvalidRecord,
            detail: other.to_string(),
            schema_version_if_parsed: Some(accepted.schema_version),
        },
    })?;

    verify_accepted_event(&accepted, verifying_key).map_err(|err| ValidationFailure {
        reason: ValidationReasonCode::IntegrityFailure,
        detail: err.to_string(),
        schema_version_if_parsed: Some(accepted.schema_version),
    })?;

    Ok(accepted)
}

fn peek_schema_version(value: &Value) -> Option<u32> {
    value
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .and_then(|n| u32::try_from(n).ok())
}

fn enforce_json_limits(value: &Value, depth: usize) -> EventResult<()> {
    if depth > MAX_JSON_NESTING_DEPTH {
        return Err(EventError::LimitExceeded(format!(
            "JSON nesting exceeds max {MAX_JSON_NESTING_DEPTH}"
        )));
    }
    match value {
        Value::Object(map) => {
            if map.len() > MAX_JSON_CONTAINER_ELEMENTS {
                return Err(EventError::LimitExceeded(format!(
                    "JSON object exceeds max {MAX_JSON_CONTAINER_ELEMENTS} members"
                )));
            }
            let mut string_bytes = 0usize;
            for (k, v) in map {
                string_bytes = string_bytes.saturating_add(k.len());
                if string_bytes > MAX_JSON_STRING_BYTES_PER_RECORD {
                    return Err(EventError::LimitExceeded(format!(
                        "JSON string bytes exceed max {MAX_JSON_STRING_BYTES_PER_RECORD}"
                    )));
                }
                enforce_json_limits(v, depth + 1)?;
                if let Value::String(s) = v {
                    string_bytes = string_bytes.saturating_add(s.len());
                    if string_bytes > MAX_JSON_STRING_BYTES_PER_RECORD {
                        return Err(EventError::LimitExceeded(format!(
                            "JSON string bytes exceed max {MAX_JSON_STRING_BYTES_PER_RECORD}"
                        )));
                    }
                }
            }
            Ok(())
        }
        Value::Array(items) => {
            if items.len() > MAX_JSON_CONTAINER_ELEMENTS {
                return Err(EventError::LimitExceeded(format!(
                    "JSON array exceeds max {MAX_JSON_CONTAINER_ELEMENTS} elements"
                )));
            }
            for item in items {
                enforce_json_limits(item, depth + 1)?;
            }
            Ok(())
        }
        Value::String(s) => {
            if s.len() > MAX_JSON_STRING_BYTES_PER_RECORD {
                return Err(EventError::LimitExceeded(format!(
                    "JSON string exceeds max {MAX_JSON_STRING_BYTES_PER_RECORD} bytes"
                )));
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device_seq::DeviceSeq;
    use crate::envelope::{
        Classification, EventPayload, EventType, MemoryNotePayload, SignableEvent,
    };
    use crate::hex::ContentHashHex;
    use crate::integrity::{accept_event, signing_key_id_from_verifying_key};
    use ed25519_dalek::SigningKey;

    fn sample_accepted() -> (SigningKey, AcceptedEvent) {
        let key = SigningKey::from_bytes(&[0x21u8; 32]);
        let key_id = signing_key_id_from_verifying_key(&key.verifying_key()).unwrap();
        let signable = SignableEvent {
            schema_version: EVENT_SCHEMA_VERSION,
            event_id: "11111111-1111-4111-8111-111111111111".into(),
            brain_id: "22222222-2222-4222-8222-222222222222".into(),
            device_id: "device-a".into(),
            device_seq: DeviceSeq::from_u64(1).unwrap(),
            created_at: "2026-10-07T00:00:00Z".into(),
            event_type: EventType::MemoryNote,
            classification: Classification::Internal,
            signing_key_id: key_id,
            payload: EventPayload::MemoryNote(MemoryNotePayload::new("hello").unwrap()),
            prev_content_hash: ContentHashHex::zero(),
        };
        (key.clone(), accept_event(signable, &key).unwrap())
    }

    #[test]
    fn accepts_valid_signed_record() {
        let (key, accepted) = sample_accepted();
        let bytes = serde_json_canonicalizer::to_vec(&accepted).unwrap();
        let parsed = validate_accepted_record_bytes(&bytes, &key.verifying_key()).unwrap();
        assert_eq!(parsed.content_hash, accepted.content_hash);
    }

    #[test]
    fn rejects_unsupported_schema_version_with_reason_code() {
        let (key, mut accepted) = sample_accepted();
        accepted.schema_version = 2;
        // Bypass accept_event re-check by serializing mutated struct directly.
        let bytes = serde_json::to_vec(&accepted).unwrap();
        let err = validate_accepted_record_bytes(&bytes, &key.verifying_key()).unwrap_err();
        assert_eq!(err.reason, ValidationReasonCode::UnsupportedSchema);
        assert_eq!(err.schema_version_if_parsed, Some(2));
        assert_eq!(err.reason.as_str(), "unsupported_schema");
    }

    #[test]
    fn rejects_unknown_fields() {
        let (key, accepted) = sample_accepted();
        let mut value = serde_json::to_value(&accepted).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("extra".into(), serde_json::json!(1));
        let bytes = serde_json::to_vec(&value).unwrap();
        let err = validate_accepted_record_bytes(&bytes, &key.verifying_key()).unwrap_err();
        assert_eq!(err.reason, ValidationReasonCode::UnknownField);
    }

    #[test]
    fn rejects_duplicate_keys() {
        let (key, _) = sample_accepted();
        let raw = r#"{"schema_version":1,"schema_version":1}"#;
        let err = validate_accepted_record_bytes(raw.as_bytes(), &key.verifying_key()).unwrap_err();
        assert_eq!(err.reason, ValidationReasonCode::DuplicateJsonKey);
    }

    #[test]
    fn rejects_integrity_failure() {
        let (_key, accepted) = sample_accepted();
        let other = SigningKey::from_bytes(&[0x22u8; 32]);
        let bytes = serde_json_canonicalizer::to_vec(&accepted).unwrap();
        let err = validate_accepted_record_bytes(&bytes, &other.verifying_key()).unwrap_err();
        assert_eq!(err.reason, ValidationReasonCode::IntegrityFailure);
    }

    #[test]
    fn rejects_excessive_nesting() {
        let (key, _) = sample_accepted();
        let mut v = serde_json::json!("leaf");
        for _ in 0..(MAX_JSON_NESTING_DEPTH + 2) {
            v = serde_json::json!({ "n": v });
        }
        // Provide minimal sibling fields so parse gets further if needed — nesting checked first.
        let bytes = serde_json::to_vec(&v).unwrap();
        let err = validate_accepted_record_bytes(&bytes, &key.verifying_key()).unwrap_err();
        assert_eq!(err.reason, ValidationReasonCode::LimitExceeded);
    }
}
