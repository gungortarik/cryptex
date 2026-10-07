//! Hex-encoded integrity field helpers (ADR-0016).

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{EventError, EventResult};

fn parse_lowercase_hex(raw: &str, expected_chars: usize, label: &str) -> EventResult<String> {
    if raw.len() != expected_chars {
        return Err(EventError::Invalid(format!(
            "{label} must be {expected_chars} lowercase hex characters"
        )));
    }
    if !raw
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(EventError::Invalid(format!(
            "{label} must be lowercase hexadecimal"
        )));
    }
    Ok(raw.to_string())
}

macro_rules! hex_newtype {
    ($name:ident, $chars:expr, $label:expr) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, JsonSchema)]
        pub struct $name(String);

        impl $name {
            pub fn parse(raw: &str) -> EventResult<Self> {
                Ok(Self(parse_lowercase_hex(raw, $chars, $label)?))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let raw = String::deserialize(deserializer)?;
                $name::parse(&raw).map_err(serde::de::Error::custom)
            }
        }
    };
}

hex_newtype!(ContentHashHex, 64, "content hash");
hex_newtype!(SigningKeyId, 32, "signing_key_id");
hex_newtype!(SignatureHex, 128, "signature");

/// First-event `prev_content_hash` sentinel (64 zero nibbles).
pub const ZERO_CONTENT_HASH_HEX: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

impl ContentHashHex {
    pub fn zero() -> Self {
        Self::parse(ZERO_CONTENT_HASH_HEX).expect("zero hash constant is valid")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_lowercase_hex_and_rejects_uppercase() {
        let h = "a".repeat(64);
        ContentHashHex::parse(&h).unwrap();
        assert!(ContentHashHex::parse(&"A".repeat(64)).is_err());
        assert!(SigningKeyId::parse(&"ab".repeat(16)).is_ok());
        assert!(SignatureHex::parse(&"cd".repeat(64)).is_ok());
        assert_eq!(ContentHashHex::zero().as_str().len(), 64);
    }
}
