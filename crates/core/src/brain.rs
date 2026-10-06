use serde::{Deserialize, Serialize};

use crate::error::{CoreError, CoreResult};

/// Schema version for immutable brain identity metadata (ADR-0015 §2.1).
pub const BRAIN_IDENTITY_SCHEMA_VERSION: u32 = 1;

/// Canonical public brain identity metadata. Daemon-written; immutable `brain_id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrainIdentity {
    pub schema_version: u32,
    pub brain_id: String,
    /// Display/timeline only — never security authority.
    pub created_at: String,
}

impl BrainIdentity {
    pub fn new(brain_id: impl Into<String>, created_at: impl Into<String>) -> CoreResult<Self> {
        let brain_id = brain_id.into().trim().to_string();
        let created_at = created_at.into().trim().to_string();
        if brain_id.is_empty() {
            return Err(CoreError::InvalidId(
                "brain_id must be non-empty".to_string(),
            ));
        }
        if created_at.is_empty() {
            return Err(CoreError::InvalidId(
                "created_at must be non-empty display timestamp".to_string(),
            ));
        }
        Ok(Self {
            schema_version: BRAIN_IDENTITY_SCHEMA_VERSION,
            brain_id,
            created_at,
        })
    }

    pub fn to_json_lf(&self) -> CoreResult<String> {
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
        if parsed.schema_version == 0 {
            return Err(CoreError::InvalidId(
                "brain identity schema_version must be positive".to_string(),
            ));
        }
        if parsed.brain_id.trim().is_empty() {
            return Err(CoreError::InvalidId(
                "brain_id must be non-empty".to_string(),
            ));
        }
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
}
