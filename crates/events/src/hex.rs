//! Hex-encoded integrity field helpers (ADR-0016).

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{EventError, EventResult};

pub const CONTENT_HASH_HEX_LEN: usize = 64;
pub const SIGNING_KEY_ID_HEX_LEN: usize = 32;
pub const SIGNATURE_HEX_LEN: usize = 128;

pub const CONTENT_HASH_HEX_PATTERN: &str = r"^[0-9a-f]{64}$";
pub const SIGNING_KEY_ID_HEX_PATTERN: &str = r"^[0-9a-f]{32}$";
pub const SIGNATURE_HEX_PATTERN: &str = r"^[0-9a-f]{128}$";

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

/// 64-char lowercase hex content hash.
#[derive(Debug, Clone, PartialEq, Eq, Hash, JsonSchema)]
#[schemars(transparent)]
pub struct ContentHashHex(
    #[schemars(
        regex(pattern = CONTENT_HASH_HEX_PATTERN),
        length(equal = 64)
    )]
    String,
);

/// 32-char lowercase hex signing key id.
#[derive(Debug, Clone, PartialEq, Eq, Hash, JsonSchema)]
#[schemars(transparent)]
pub struct SigningKeyId(
    #[schemars(
        regex(pattern = SIGNING_KEY_ID_HEX_PATTERN),
        length(equal = 32)
    )]
    String,
);

/// 128-char lowercase hex Ed25519 signature.
#[derive(Debug, Clone, PartialEq, Eq, Hash, JsonSchema)]
#[schemars(transparent)]
pub struct SignatureHex(
    #[schemars(
        regex(pattern = SIGNATURE_HEX_PATTERN),
        length(equal = 128)
    )]
    String,
);

impl ContentHashHex {
    pub fn parse(raw: &str) -> EventResult<Self> {
        Ok(Self(parse_lowercase_hex(
            raw,
            CONTENT_HASH_HEX_LEN,
            "content hash",
        )?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn zero() -> Self {
        Self::parse(ZERO_CONTENT_HASH_HEX).expect("zero hash constant is valid")
    }
}

impl SigningKeyId {
    pub fn parse(raw: &str) -> EventResult<Self> {
        Ok(Self(parse_lowercase_hex(
            raw,
            SIGNING_KEY_ID_HEX_LEN,
            "signing_key_id",
        )?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl SignatureHex {
    pub fn parse(raw: &str) -> EventResult<Self> {
        Ok(Self(parse_lowercase_hex(
            raw,
            SIGNATURE_HEX_LEN,
            "signature",
        )?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

macro_rules! impl_serde_hex {
    ($name:ident) => {
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

impl_serde_hex!(ContentHashHex);
impl_serde_hex!(SigningKeyId);
impl_serde_hex!(SignatureHex);

/// First-event `prev_content_hash` sentinel (64 zero nibbles).
pub const ZERO_CONTENT_HASH_HEX: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

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
