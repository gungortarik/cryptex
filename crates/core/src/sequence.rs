//! Sequence-related shared types (ADR-0015).
//!
//! Durable counter allocation lives only in `cryptexd` (`sequence_store`),
//! because ADR-0015 requires local cryptexd to be the sole allocator.

use serde::{Deserialize, Serialize};

use crate::ids::DeviceId;

/// Placeholder cutoff record for later milestones.
///
/// M1 stores the shape only. Cutoff enforcement semantics are defined in M8,
/// not here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceSequenceCutoff {
    pub device_id: DeviceId,
    pub cutoff: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[test]
    fn cutoff_round_trips() {
        let cutoff = DeviceSequenceCutoff {
            device_id: DeviceId::new("device-cutoff-1").unwrap(),
            cutoff: 9,
        };
        let json = serde_json::to_string(&cutoff).unwrap();
        let parsed: DeviceSequenceCutoff = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, cutoff);
    }

    #[test]
    fn nested_cutoff_deserialize_rejects_unsafe_device_id() {
        #[derive(Debug, Deserialize)]
        struct Wrap {
            #[allow(dead_code)]
            cutoff: DeviceSequenceCutoff,
        }

        let err = serde_json::from_str::<Wrap>(
            r#"{
  "cutoff": {
    "device_id": "../sneaky",
    "cutoff": 7
  }
}"#,
        );
        assert!(err.is_err());
    }
}
