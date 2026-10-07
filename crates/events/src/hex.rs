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

mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut out = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            out.push(HEX[(b >> 4) as usize] as char);
            out.push(HEX[(b & 0x0f) as usize] as char);
        }
        out
    }

    pub fn decode(raw: &str) -> crate::EventResult<Vec<u8>> {
        if raw.len() % 2 != 0 {
            return Err(crate::EventError::Invalid(
                "hex string must have even length".into(),
            ));
        }
        let mut out = Vec::with_capacity(raw.len() / 2);
        let bytes = raw.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let hi = from_nibble(bytes[i])?;
            let lo = from_nibble(bytes[i + 1])?;
            out.push((hi << 4) | lo);
            i += 2;
        }
        Ok(out)
    }

    fn from_nibble(b: u8) -> crate::EventResult<u8> {
        match b {
            b'0'..=b'9' => Ok(b - b'0'),
            b'a'..=b'f' => Ok(b - b'a' + 10),
            _ => Err(crate::EventError::Invalid(
                "hex string must be lowercase hexadecimal".into(),
            )),
        }
    }
}

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

    pub fn from_bytes(bytes: &[u8]) -> EventResult<Self> {
        if bytes.len() != 32 {
            return Err(EventError::Invalid(format!(
                "content hash must be 32 bytes, got {}",
                bytes.len()
            )));
        }
        Self::parse(&hex::encode(bytes))
    }

    pub fn to_bytes(&self) -> EventResult<[u8; 32]> {
        let raw = hex::decode(&self.0)?;
        let mut out = [0u8; 32];
        out.copy_from_slice(&raw);
        Ok(out)
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

    pub fn from_bytes(bytes: &[u8]) -> EventResult<Self> {
        if bytes.len() != 16 {
            return Err(EventError::Invalid(format!(
                "signing_key_id must be 16 bytes, got {}",
                bytes.len()
            )));
        }
        Self::parse(&hex::encode(bytes))
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

    pub fn from_bytes(bytes: &[u8]) -> EventResult<Self> {
        if bytes.len() != 64 {
            return Err(EventError::Invalid(format!(
                "signature must be 64 bytes, got {}",
                bytes.len()
            )));
        }
        Self::parse(&hex::encode(bytes))
    }

    pub fn to_bytes(&self) -> EventResult<[u8; 64]> {
        let raw = hex::decode(&self.0)?;
        let mut out = [0u8; 64];
        out.copy_from_slice(&raw);
        Ok(out)
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
