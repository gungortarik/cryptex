//! Install identity metadata (ADR-0015 section 2.2).
//!
//! `install_id` binds this Cryptex installation / local instance. Durable writes
//! are performed only by `cryptexd` (`install_runtime`).

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, CoreResult};
use crate::ids::InstallId;
use crate::validate::require_exact_schema;

/// Schema version for durable install identity metadata.
pub const INSTALL_IDENTITY_SCHEMA_VERSION: u32 = 1;

/// Public install identity metadata. Daemon-written; stable for the lifetime of an install.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallIdentity {
    pub schema_version: u32,
    pub install_id: InstallId,
}

impl InstallIdentity {
    pub fn new(install_id: InstallId) -> CoreResult<Self> {
        let identity = Self {
            schema_version: INSTALL_IDENTITY_SCHEMA_VERSION,
            install_id,
        };
        identity.validate()?;
        Ok(identity)
    }

    pub fn validate(&self) -> CoreResult<()> {
        require_exact_schema(
            "install identity",
            self.schema_version,
            INSTALL_IDENTITY_SCHEMA_VERSION,
        )?;
        // InstallId already validated on construct/deserialize; re-check via Display/as_str path.
        InstallId::new(self.install_id.as_str())?;
        Ok(())
    }

    pub fn to_json_lf(&self) -> CoreResult<String> {
        self.validate()?;
        let mut json = serde_json::to_string_pretty(self)
            .map_err(|e| CoreError::Io(format!("serialize install identity: {e}")))?;
        json = json.replace("\r\n", "\n");
        if !json.ends_with('\n') {
            json.push('\n');
        }
        Ok(json)
    }

    pub fn from_json(text: &str) -> CoreResult<Self> {
        let parsed: Self = serde_json::from_str(text)
            .map_err(|e| CoreError::Io(format!("parse install identity: {e}")))?;
        parsed.validate()?;
        Ok(parsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_utf8_lf_json() {
        let id = InstallId::new("install-local-001").unwrap();
        let identity = InstallIdentity::new(id).unwrap();
        let json = identity.to_json_lf().unwrap();
        assert!(json.ends_with('\n'));
        assert!(!json.contains('\r'));
        assert_eq!(InstallIdentity::from_json(&json).unwrap(), identity);
    }

    #[test]
    fn fails_closed_on_future_schema_version() {
        let json = r#"{
  "schema_version": 9,
  "install_id": "install-local-001"
}"#;
        let err = InstallIdentity::from_json(json).unwrap_err();
        assert!(matches!(
            err,
            CoreError::UnsupportedSchema {
                found: 9,
                expected: 1,
                ..
            }
        ));
    }

    #[test]
    fn rejects_path_unsafe_install_id() {
        let json = r#"{
  "schema_version": 1,
  "install_id": "../sneaky"
}"#;
        assert!(InstallIdentity::from_json(json).is_err());
    }
}
