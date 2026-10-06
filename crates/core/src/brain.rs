use serde::{Deserialize, Serialize};

use crate::error::{CoreError, CoreResult};
use crate::validate::{parse_rfc3339_str, parse_uuid_str, require_exact_schema};

/// Schema version for immutable brain identity metadata (ADR-0015 section 2.1).
pub const BRAIN_IDENTITY_SCHEMA_VERSION: u32 = 1;

/// Canonical public brain identity metadata. Daemon-written; immutable `brain_id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrainIdentity {
    pub schema_version: u32,
    pub brain_id: String,
    /// Display/timeline only -- never security authority.
    pub created_at: String,
}

impl BrainIdentity {
    pub fn new(brain_id: impl Into<String>, created_at: impl Into<String>) -> CoreResult<Self> {
        let brain_id = parse_uuid_str(&brain_id.into())?;
        let created_at = parse_rfc3339_str(&created_at.into())?;
        Ok(Self {
            schema_version: BRAIN_IDENTITY_SCHEMA_VERSION,
            brain_id,
            created_at,
        })
    }

    pub fn validate(&self) -> CoreResult<()> {
        require_exact_schema(
            "brain identity",
            self.schema_version,
            BRAIN_IDENTITY_SCHEMA_VERSION,
        )?;
        parse_uuid_str(&self.brain_id)?;
        parse_rfc3339_str(&self.created_at)?;
        Ok(())
    }

    pub fn to_json_lf(&self) -> CoreResult<String> {
        self.validate()?;
        let mut json = serde_json::to_string_pretty(self)
            .map_err(|e| CoreError::Io(format!("serialize brain identity: {e}")))?;
        // Canonical text is UTF-8 with LF (no CRLF).
        json = json.replace("\r\n", "\n");
        if !json.ends_with('\n') {
            json.push('\n');
        }
        Ok(json)
    }

    pub fn from_json(text: &str) -> CoreResult<Self> {
        let parsed: Self = serde_json::from_str(text)
            .map_err(|e| CoreError::Io(format!("parse brain identity: {e}")))?;
        parsed.validate()?;
        Ok(parsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_utf8_lf_json() {
        let identity = BrainIdentity::new(
            "11111111-1111-4111-8111-111111111111",
            "2026-10-05T00:00:00Z",
        )
        .expect("identity");
        let json = identity.to_json_lf().expect("json");
        assert!(json.ends_with('\n'));
        assert!(!json.contains('\r'));
        let parsed = BrainIdentity::from_json(&json).expect("parse");
        assert_eq!(parsed, identity);
    }

    #[test]
    fn rejects_non_uuid_brain_id() {
        assert!(BrainIdentity::new("not-a-uuid", "2026-10-05T00:00:00Z").is_err());
        let json = r#"{
  "schema_version": 1,
  "brain_id": "brain-1",
  "created_at": "2026-10-05T00:00:00Z"
}"#;
        assert!(BrainIdentity::from_json(json).is_err());
    }

    #[test]
    fn rejects_non_rfc3339_created_at() {
        assert!(BrainIdentity::new("11111111-1111-4111-8111-111111111111", "2026-10-05").is_err());
        let json = r#"{
  "schema_version": 1,
  "brain_id": "11111111-1111-4111-8111-111111111111",
  "created_at": "yesterday"
}"#;
        assert!(BrainIdentity::from_json(json).is_err());
    }

    #[test]
    fn fails_closed_on_future_schema_version() {
        let json = r#"{
  "schema_version": 99,
  "brain_id": "11111111-1111-4111-8111-111111111111",
  "created_at": "2026-10-05T00:00:00Z"
}"#;
        let err = BrainIdentity::from_json(json).unwrap_err();
        assert!(matches!(
            err,
            CoreError::UnsupportedSchema {
                found: 99,
                expected: 1,
                ..
            }
        ));
    }
}
