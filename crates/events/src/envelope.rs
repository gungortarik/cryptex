//! Production event envelope types (ADR-0016 Â§3).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cryptex_core::ids::DeviceId;
use cryptex_core::validate::{parse_rfc3339_str, parse_uuid_str, require_exact_schema};

use crate::device_seq::DeviceSeq;
use crate::hex::{ContentHashHex, SignatureHex, SigningKeyId};
use crate::limits::MAX_MEMORY_NOTE_TEXT_BYTES;
use crate::{EventError, EventResult};

/// Event schema version emitted by M2 writers.
pub const EVENT_SCHEMA_VERSION: u32 = 1;

/// Production classification values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Classification {
    Public,
    Internal,
    Private,
    Restricted,
}

impl Classification {
    /// Wire / fingerprint form (snake_case).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Internal => "internal",
            Self::Private => "private",
            Self::Restricted => "restricted",
        }
    }
}

/// Production `event_type` values for M2 (fixture-only types are not included).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum EventType {
    #[serde(rename = "memory.note")]
    #[schemars(rename = "memory.note")]
    MemoryNote,
}

impl EventType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MemoryNote => "memory.note",
        }
    }
}

/// Payload for `memory.note`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MemoryNotePayload {
    pub text: String,
}

impl MemoryNotePayload {
    pub fn new(text: impl Into<String>) -> EventResult<Self> {
        let text = text.into();
        if text.len() > MAX_MEMORY_NOTE_TEXT_BYTES {
            return Err(EventError::LimitExceeded(format!(
                "memory.note text exceeds max {MAX_MEMORY_NOTE_TEXT_BYTES} bytes"
            )));
        }
        Ok(Self { text })
    }
}

/// Type-specific payload object. Production M2 includes only `memory.note`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum EventPayload {
    MemoryNote(MemoryNotePayload),
}

/// Signable event fields (excludes `content_hash` and `signature`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SignableEvent {
    #[schemars(range(min = 1, max = 1))]
    pub schema_version: u32,
    pub event_id: String,
    pub brain_id: String,
    pub device_id: String,
    pub device_seq: DeviceSeq,
    pub created_at: String,
    pub event_type: EventType,
    pub classification: Classification,
    pub signing_key_id: SigningKeyId,
    pub payload: EventPayload,
    pub prev_content_hash: ContentHashHex,
}

/// Accepted on-disk record: signable fields plus integrity fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AcceptedEvent {
    #[schemars(range(min = 1, max = 1))]
    pub schema_version: u32,
    pub event_id: String,
    pub brain_id: String,
    pub device_id: String,
    pub device_seq: DeviceSeq,
    pub created_at: String,
    pub event_type: EventType,
    pub classification: Classification,
    pub signing_key_id: SigningKeyId,
    pub payload: EventPayload,
    pub prev_content_hash: ContentHashHex,
    pub content_hash: ContentHashHex,
    pub signature: SignatureHex,
}

impl SignableEvent {
    pub fn validate(&self) -> EventResult<()> {
        require_exact_schema("event", self.schema_version, EVENT_SCHEMA_VERSION)?;
        parse_uuid_str(&self.event_id)?;
        parse_uuid_str(&self.brain_id)?;
        DeviceId::new(self.device_id.clone())?;
        let _ = self.device_seq.to_u64()?;
        parse_rfc3339_str(&self.created_at)?;
        match (&self.event_type, &self.payload) {
            (EventType::MemoryNote, EventPayload::MemoryNote(payload)) => {
                MemoryNotePayload::new(payload.text.clone())?;
            }
        }
        Ok(())
    }
}

impl AcceptedEvent {
    pub fn validate(&self) -> EventResult<()> {
        let signable = SignableEvent {
            schema_version: self.schema_version,
            event_id: self.event_id.clone(),
            brain_id: self.brain_id.clone(),
            device_id: self.device_id.clone(),
            device_seq: self.device_seq.clone(),
            created_at: self.created_at.clone(),
            event_type: self.event_type,
            classification: self.classification,
            signing_key_id: self.signing_key_id.clone(),
            payload: self.payload.clone(),
            prev_content_hash: self.prev_content_hash.clone(),
        };
        signable.validate()?;
        Ok(())
    }

    pub fn from_parts(
        signable: SignableEvent,
        content_hash: ContentHashHex,
        signature: SignatureHex,
    ) -> EventResult<Self> {
        signable.validate()?;
        let accepted = Self {
            schema_version: signable.schema_version,
            event_id: signable.event_id,
            brain_id: signable.brain_id,
            device_id: signable.device_id,
            device_seq: signable.device_seq,
            created_at: signable.created_at,
            event_type: signable.event_type,
            classification: signable.classification,
            signing_key_id: signable.signing_key_id,
            payload: signable.payload,
            prev_content_hash: signable.prev_content_hash,
            content_hash,
            signature,
        };
        accepted.validate()?;
        Ok(accepted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_signable() -> SignableEvent {
        SignableEvent {
            schema_version: EVENT_SCHEMA_VERSION,
            event_id: "11111111-1111-4111-8111-111111111111".into(),
            brain_id: "22222222-2222-4222-8222-222222222222".into(),
            device_id: "device-a".into(),
            device_seq: DeviceSeq::from_u64(1).unwrap(),
            created_at: "2026-10-07T00:00:00Z".into(),
            event_type: EventType::MemoryNote,
            classification: Classification::Internal,
            signing_key_id: SigningKeyId::parse(&"ab".repeat(16)).unwrap(),
            payload: EventPayload::MemoryNote(MemoryNotePayload::new("hello").unwrap()),
            prev_content_hash: ContentHashHex::zero(),
        }
    }

    #[test]
    fn validates_production_memory_note_envelope() {
        sample_signable().validate().unwrap();
    }

    #[test]
    fn rejects_unsupported_schema_version() {
        let mut event = sample_signable();
        event.schema_version = 99;
        assert!(matches!(
            event.validate(),
            Err(EventError::UnsupportedSchema {
                found: 99,
                expected: 1
            })
        ));
    }

    #[test]
    fn serde_uses_memory_note_event_type_string() {
        let json = serde_json::to_value(sample_signable()).unwrap();
        assert_eq!(json["event_type"], "memory.note");
        assert_eq!(json["device_seq"], "1");
        assert!(json["device_seq"].is_string());
        assert!(json.get("content_hash").is_none());
    }

    #[test]
    fn accepted_event_includes_integrity_fields() {
        let accepted = AcceptedEvent::from_parts(
            sample_signable(),
            ContentHashHex::parse(&"11".repeat(32)).unwrap(),
            SignatureHex::parse(&"22".repeat(64)).unwrap(),
        )
        .unwrap();
        let json = serde_json::to_value(&accepted).unwrap();
        assert!(json["content_hash"].is_string());
        assert!(json["signature"].is_string());
    }

    #[test]
    fn serde_rejects_unknown_fields_on_accepted_event() {
        let mut value = serde_json::to_value(
            AcceptedEvent::from_parts(
                sample_signable(),
                ContentHashHex::parse(&"11".repeat(32)).unwrap(),
                SignatureHex::parse(&"22".repeat(64)).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("unexpected".into(), serde_json::json!("x"));
        let err = serde_json::from_value::<AcceptedEvent>(value).unwrap_err();
        assert!(
            err.to_string().contains("unexpected") || err.to_string().contains("unknown"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn serde_rejects_unknown_fields_on_memory_note_payload() {
        let err =
            serde_json::from_str::<MemoryNotePayload>(r#"{"text":"hi","extra":1}"#).unwrap_err();
        assert!(
            err.to_string().contains("extra") || err.to_string().contains("unknown"),
            "unexpected error: {err}"
        );
    }
}
