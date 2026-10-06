use serde::{Deserialize, Serialize};

use crate::ids::DeviceId;

/// Pairing record state enum (data only — product flows are M8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PairingState {
    Pending,
    Trusted,
    Retired,
    Compromised,
}

/// Durable pairing **record** shape (ADR-0015 §2.4). Not a pairing UX.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingRecord {
    pub peer_device_id: DeviceId,
    pub state: PairingState,
    pub local_brain_id: String,
    /// Display only — never security authority.
    pub updated_at: String,
}

impl PairingRecord {
    pub fn new(
        peer_device_id: DeviceId,
        state: PairingState,
        local_brain_id: impl Into<String>,
        updated_at: impl Into<String>,
    ) -> Self {
        Self {
            peer_device_id,
            state,
            local_brain_id: local_brain_id.into(),
            updated_at: updated_at.into(),
        }
    }
}
