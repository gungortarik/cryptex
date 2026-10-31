use serde::{Deserialize, Serialize};

use crate::error::{CoreError, CoreResult};
use crate::ids::DeviceId;
use crate::validate::{parse_rfc3339_str, parse_uuid_str, require_exact_schema};

/// Schema version for durable pairing records (fail-closed on load).
pub const PAIRING_RECORD_SCHEMA_VERSION: u32 = 1;

/// Pairing record state enum (data only -- product flows are M8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PairingState {
    Pending,
    Trusted,
    Retired,
    Compromised,
}

/// Durable pairing **record** shape (ADR-0015 section 2.4). Not a pairing UX.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingRecord {
    pub schema_version: u32,
    pub peer_device_id: DeviceId,
    pub state: PairingState,
    pub local_brain_id: String,
    /// Display only -- never security authority.
    pub updated_at: String,
}

impl PairingRecord {
    pub fn new(
        peer_device_id: DeviceId,
        state: PairingState,
        local_brain_id: impl Into<String>,
        updated_at: impl Into<String>,
    ) -> CoreResult<Self> {
        let record = Self {
            schema_version: PAIRING_RECORD_SCHEMA_VERSION,
            peer_device_id,
            state,
            local_brain_id: local_brain_id.into(),
            updated_at: updated_at.into(),
        };
        record.validate()?;
        Ok(record)
    }

    pub fn validate(&self) -> CoreResult<()> {
        require_exact_schema(
            "pairing record",
            self.schema_version,
            PAIRING_RECORD_SCHEMA_VERSION,
        )?;
        parse_uuid_str(&self.local_brain_id)?;
        parse_rfc3339_str(&self.updated_at)?;
        Ok(())
    }

    pub fn from_json(text: &str) -> CoreResult<Self> {
        let parsed: Self = serde_json::from_str(text)
            .map_err(|e| CoreError::Io(format!("parse pairing record: {e}")))?;
        parsed.validate()?;
        Ok(parsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_pairing_record() {
        let peer = DeviceId::new("peer-device-aabb").unwrap();
        let record = PairingRecord::new(
            peer,
            PairingState::Trusted,
            "11111111-1111-4111-8111-111111111111",
            "2026-10-06T00:00:00Z",
        )
        .unwrap();
        assert_eq!(record.schema_version, PAIRING_RECORD_SCHEMA_VERSION);
    }

    #[test]
    fn rejects_malformed_brain_id_and_timestamp() {
        let peer = DeviceId::new("peer-device-aabb").unwrap();
        assert!(
            PairingRecord::new(
                peer.clone(),
                PairingState::Pending,
                "brain-1",
                "2026-10-06T00:00:00Z",
            )
            .is_err()
        );
        assert!(
            PairingRecord::new(
                peer,
                PairingState::Pending,
                "11111111-1111-4111-8111-111111111111",
                "not-a-timestamp",
            )
            .is_err()
        );
    }

    #[test]
    fn fails_closed_on_future_schema_version() {
        let json = r#"{
  "schema_version": 7,
  "peer_device_id": "peer-device-aabb",
  "state": "trusted",
  "local_brain_id": "11111111-1111-4111-8111-111111111111",
  "updated_at": "2026-10-06T00:00:00Z"
}"#;
        let err = PairingRecord::from_json(json).unwrap_err();
        assert!(matches!(
            err,
            CoreError::UnsupportedSchema {
                found: 7,
                expected: 1,
                ..
            }
        ));
    }

    #[test]
    fn rejects_whitespace_wrapped_local_brain_id() {
        let peer = DeviceId::new("peer-device-aabb").unwrap();
        assert!(
            PairingRecord::new(
                peer,
                PairingState::Pending,
                " 11111111-1111-4111-8111-111111111111",
                "2026-10-06T00:00:00Z",
            )
            .is_err()
        );
        let json = r#"{
  "schema_version": 1,
  "peer_device_id": "peer-device-aabb",
  "state": "pending",
  "local_brain_id": "11111111-1111-4111-8111-111111111111 ",
  "updated_at": "2026-10-06T00:00:00Z"
}"#;
        assert!(PairingRecord::from_json(json).is_err());
    }
}
