//! Canonical decimal-string `device_seq` (ADR-0016 §3.1.1).

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{EventError, EventResult};

/// Per-device sequence encoded as a canonical decimal JSON string covering full `u64`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, JsonSchema)]
#[schemars(description = "Canonical decimal string of a u64 device sequence in [1, 2^64-1]")]
pub struct DeviceSeq(String);

impl DeviceSeq {
    /// Construct from an allocated ADR-0015 sequence value (`>= 1`).
    pub fn from_u64(value: u64) -> EventResult<Self> {
        if value == 0 {
            return Err(EventError::Invalid(
                "device_seq must be >= 1 (0 is reserved for none issued)".into(),
            ));
        }
        Ok(Self(value.to_string()))
    }

    /// Strict parse of a canonical decimal digit string into `u64` then `DeviceSeq`.
    pub fn parse(raw: &str) -> EventResult<Self> {
        if raw.is_empty() || !raw.bytes().all(|b| b.is_ascii_digit()) {
            return Err(EventError::Invalid(
                "device_seq must be a canonical decimal digit string".into(),
            ));
        }
        if raw.as_bytes()[0] == b'0' {
            return Err(EventError::Invalid(
                "device_seq must not have leading zeros and must be >= 1".into(),
            ));
        }
        let value: u64 = raw
            .parse()
            .map_err(|_| EventError::Invalid("device_seq does not fit in u64".into()))?;
        Self::from_u64(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn to_u64(&self) -> EventResult<u64> {
        self.0
            .parse()
            .map_err(|_| EventError::Invalid("device_seq does not fit in u64".into()))
    }
}

impl Serialize for DeviceSeq {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for DeviceSeq {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        DeviceSeq::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for DeviceSeq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_full_u64_range_as_decimal_string() {
        let max = DeviceSeq::from_u64(u64::MAX).unwrap();
        assert_eq!(max.as_str(), "18446744073709551615");
        assert_eq!(max.to_u64().unwrap(), u64::MAX);
        let parsed = DeviceSeq::parse("18446744073709551615").unwrap();
        assert_eq!(parsed, max);
    }

    #[test]
    fn rejects_zero_leading_zeros_and_json_number_shape() {
        assert!(DeviceSeq::from_u64(0).is_err());
        assert!(DeviceSeq::parse("0").is_err());
        assert!(DeviceSeq::parse("01").is_err());
        assert!(DeviceSeq::parse("-1").is_err());
        assert!(DeviceSeq::parse("1.0").is_err());
        assert!(DeviceSeq::parse("18446744073709551616").is_err()); // u64::MAX+1
    }

    #[test]
    fn serde_round_trip_is_string_not_number() {
        let seq = DeviceSeq::from_u64(42).unwrap();
        let json = serde_json::to_string(&seq).unwrap();
        assert_eq!(json, "\"42\"");
        let parsed: DeviceSeq = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, seq);
        assert!(serde_json::from_str::<DeviceSeq>("42").is_err());
    }
}
