use serde::{Deserialize, Serialize};

use crate::error::{CoreError, CoreResult};
use crate::validate::{parse_uuid_str, require_exact_schema};

/// Schema version for Recovery Kit **public** metadata (ADR-0015 section 2.5).
pub const RECOVERY_KIT_PUBLIC_SCHEMA_VERSION: u32 = 1;

/// Declared Recovery Kit purposes -- not embedded secrets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryPurpose {
    BackupDecrypt,
    TrustBootstrap,
}

/// Public, non-secret Recovery Kit metadata. Offline signing secrets must not appear here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryKitPublicMetadata {
    pub schema_version: u32,
    pub kit_id: String,
    pub brain_id: String,
    pub purposes: Vec<RecoveryPurpose>,
}

impl RecoveryKitPublicMetadata {
    pub fn new(
        kit_id: impl Into<String>,
        brain_id: impl Into<String>,
        purposes: Vec<RecoveryPurpose>,
    ) -> CoreResult<Self> {
        let kit_id = kit_id.into().trim().to_string();
        if kit_id.is_empty() {
            return Err(CoreError::InvalidId("kit_id must be non-empty".into()));
        }
        let brain_id = parse_uuid_str(&brain_id.into())?;
        let meta = Self {
            schema_version: RECOVERY_KIT_PUBLIC_SCHEMA_VERSION,
            kit_id,
            brain_id,
            purposes,
        };
        meta.validate()?;
        Ok(meta)
    }

    pub fn validate(&self) -> CoreResult<()> {
        require_exact_schema(
            "recovery kit public metadata",
            self.schema_version,
            RECOVERY_KIT_PUBLIC_SCHEMA_VERSION,
        )?;
        if self.kit_id.trim().is_empty() {
            return Err(CoreError::InvalidId("kit_id must be non-empty".into()));
        }
        parse_uuid_str(&self.brain_id)?;
        Ok(())
    }

    pub fn from_json(text: &str) -> CoreResult<Self> {
        let parsed: Self = serde_json::from_str(text)
            .map_err(|e| CoreError::Io(format!("parse recovery kit public metadata: {e}")))?;
        parsed.validate()?;
        Ok(parsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_public_metadata() {
        let meta = RecoveryKitPublicMetadata::new(
            "kit-1",
            "11111111-1111-4111-8111-111111111111",
            vec![RecoveryPurpose::BackupDecrypt],
        )
        .unwrap();
        assert_eq!(meta.schema_version, RECOVERY_KIT_PUBLIC_SCHEMA_VERSION);
    }

    #[test]
    fn rejects_malformed_brain_id() {
        assert!(
            RecoveryKitPublicMetadata::new(
                "kit-1",
                "brain-1",
                vec![RecoveryPurpose::TrustBootstrap],
            )
            .is_err()
        );
    }

    #[test]
    fn fails_closed_on_future_schema_version() {
        let json = r#"{
  "schema_version": 42,
  "kit_id": "kit-1",
  "brain_id": "11111111-1111-4111-8111-111111111111",
  "purposes": ["trust_bootstrap"]
}"#;
        let err = RecoveryKitPublicMetadata::from_json(json).unwrap_err();
        assert!(matches!(
            err,
            CoreError::UnsupportedSchema {
                found: 42,
                expected: 1,
                ..
            }
        ));
    }

    #[test]
    fn rejects_whitespace_wrapped_brain_id() {
        assert!(
            RecoveryKitPublicMetadata::new(
                "kit-1",
                " 11111111-1111-4111-8111-111111111111",
                vec![RecoveryPurpose::TrustBootstrap],
            )
            .is_err()
        );
        let json = r#"{
  "schema_version": 1,
  "kit_id": "kit-1",
  "brain_id": "11111111-1111-4111-8111-111111111111 ",
  "purposes": ["trust_bootstrap"]
}"#;
        assert!(RecoveryKitPublicMetadata::from_json(json).is_err());
    }
}
