//! Event integrity primitives (ADR-0016 §§4–5).
//!
//! Uses RFC 8785 JCS, BLAKE3-256, and Ed25519 with fixed domain-separation
//! labels. No custom cryptographic constructions.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};
use sha2::{Digest, Sha256};
use std::fmt;

use crate::envelope::{AcceptedEvent, SignableEvent};
use crate::hex::{ContentHashHex, SignatureHex, SigningKeyId, ZERO_CONTENT_HASH_HEX};
use crate::{EventError, EventResult};

/// Domain label for content-hash preimages (schema_version 1).
pub const CONTENT_HASH_DOMAIN: &[u8] = b"cryptex-event-content-v1";

/// Domain label for signature preimages (schema_version 1).
pub const SIGNATURE_DOMAIN: &[u8] = b"cryptex-event-sig-v1";

const DOMAIN_SEP: u8 = 0x00;

/// RFC 8785 JCS bytes of the signable object.
pub fn canonicalize_signable(event: &SignableEvent) -> EventResult<Vec<u8>> {
    event.validate()?;
    serde_json_canonicalizer::to_vec(event)
        .map_err(|err| EventError::Integrity(format!("JCS canonicalization failed: {err}")))
}

/// BLAKE3-256 content hash over domain-separated JCS bytes.
pub fn content_hash_raw(event: &SignableEvent) -> EventResult<[u8; 32]> {
    let j = canonicalize_signable(event)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(CONTENT_HASH_DOMAIN);
    hasher.update(&[DOMAIN_SEP]);
    hasher.update(&j);
    Ok(*hasher.finalize().as_bytes())
}

/// Lowercase-hex content hash for a signable event.
pub fn content_hash(event: &SignableEvent) -> EventResult<ContentHashHex> {
    ContentHashHex::from_bytes(&content_hash_raw(event)?)
}

fn signature_preimage(content_hash_raw: &[u8; 32]) -> Vec<u8> {
    let mut preimage = Vec::with_capacity(SIGNATURE_DOMAIN.len() + 1 + 32);
    preimage.extend_from_slice(SIGNATURE_DOMAIN);
    preimage.push(DOMAIN_SEP);
    preimage.extend_from_slice(content_hash_raw);
    preimage
}

/// Derive `signing_key_id` as first 16 bytes of SHA-256(Ed25519 public key), hex.
pub fn signing_key_id_from_verifying_key(
    verifying_key: &VerifyingKey,
) -> EventResult<SigningKeyId> {
    let digest = Sha256::digest(verifying_key.as_bytes());
    SigningKeyId::from_bytes(&digest[..16])
}

/// Sign the domain-separated content-hash preimage with an Ed25519 signing key.
pub fn sign_content_hash(
    signing_key: &SigningKey,
    content_hash_raw: &[u8; 32],
) -> EventResult<SignatureHex> {
    let preimage = signature_preimage(content_hash_raw);
    let signature = signing_key.sign(&preimage);
    SignatureHex::from_bytes(&signature.to_bytes())
}

/// Verify Ed25519 signature over the domain-separated content-hash preimage.
pub fn verify_content_hash_signature(
    verifying_key: &VerifyingKey,
    content_hash_raw: &[u8; 32],
    signature: &SignatureHex,
) -> EventResult<()> {
    let preimage = signature_preimage(content_hash_raw);
    let sig = Signature::from_bytes(&signature.to_bytes()?);
    verifying_key
        .verify(&preimage, &sig)
        .map_err(|_| EventError::Integrity("Ed25519 signature verification failed".into()))
}

/// Hash and sign a validated signable event into an accepted record.
pub fn accept_event(
    signable: SignableEvent,
    signing_key: &SigningKey,
) -> EventResult<AcceptedEvent> {
    signable.validate()?;
    let expected_key_id = signing_key_id_from_verifying_key(&signing_key.verifying_key())?;
    if signable.signing_key_id != expected_key_id {
        return Err(EventError::Integrity(format!(
            "signing_key_id {} does not match key {}",
            signable.signing_key_id.as_str(),
            expected_key_id.as_str()
        )));
    }
    let hash_raw = content_hash_raw(&signable)?;
    let content_hash = ContentHashHex::from_bytes(&hash_raw)?;
    let signature = sign_content_hash(signing_key, &hash_raw)?;
    AcceptedEvent::from_parts(signable, content_hash, signature)
}

/// Recompute hash and verify signature for an accepted event.
pub fn verify_accepted_event(
    accepted: &AcceptedEvent,
    verifying_key: &VerifyingKey,
) -> EventResult<()> {
    accepted.validate()?;
    let expected_key_id = signing_key_id_from_verifying_key(verifying_key)?;
    if accepted.signing_key_id != expected_key_id {
        return Err(EventError::Integrity(format!(
            "signing_key_id {} does not match verifying key {}",
            accepted.signing_key_id.as_str(),
            expected_key_id.as_str()
        )));
    }

    let signable = SignableEvent {
        schema_version: accepted.schema_version,
        event_id: accepted.event_id.clone(),
        brain_id: accepted.brain_id.clone(),
        device_id: accepted.device_id.clone(),
        device_seq: accepted.device_seq.clone(),
        created_at: accepted.created_at.clone(),
        event_type: accepted.event_type,
        classification: accepted.classification,
        signing_key_id: accepted.signing_key_id.clone(),
        payload: accepted.payload.clone(),
        prev_content_hash: accepted.prev_content_hash.clone(),
    };

    let computed = content_hash(&signable)?;
    if computed != accepted.content_hash {
        return Err(EventError::Integrity(format!(
            "content_hash mismatch: computed {} recorded {}",
            computed.as_str(),
            accepted.content_hash.as_str()
        )));
    }

    let hash_raw = accepted.content_hash.to_bytes()?;
    verify_content_hash_signature(verifying_key, &hash_raw, &accepted.signature)
}

/// First event on a device must use the zero `prev_content_hash` sentinel.
pub fn check_first_event_prev_hash(prev_content_hash: &ContentHashHex) -> EventResult<()> {
    if prev_content_hash.as_str() != ZERO_CONTENT_HASH_HEX {
        return Err(EventError::Integrity(
            "first event prev_content_hash must be 64 zero nibbles".into(),
        ));
    }
    Ok(())
}

/// Subsequent events must link `prev_content_hash` to the prior accepted hash.
pub fn check_chain_link(
    prior_content_hash: &ContentHashHex,
    prev_content_hash: &ContentHashHex,
) -> EventResult<()> {
    if prior_content_hash != prev_content_hash {
        return Err(EventError::Integrity(format!(
            "integrity chain broken: expected prev_content_hash {} found {}",
            prior_content_hash.as_str(),
            prev_content_hash.as_str()
        )));
    }
    Ok(())
}

/// Reject JSON text that contains duplicate object member names (ADR-0016 §4.1).
pub fn reject_duplicate_json_keys(raw: &str) -> EventResult<()> {
    let mut de = serde_json::Deserializer::from_str(raw);
    de.deserialize_any(DupKeyGuard)
        .map_err(|err| EventError::Integrity(format!("JSON duplicate-key check failed: {err}")))?;
    de.end()
        .map_err(|err| EventError::Integrity(format!("JSON trailing input: {err}")))?;
    Ok(())
}

struct DupKeyGuard;

impl<'de> Visitor<'de> for DupKeyGuard {
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "JSON value without duplicate object keys")
    }

    fn visit_bool<E: de::Error>(self, _: bool) -> Result<(), E> {
        Ok(())
    }
    fn visit_i64<E: de::Error>(self, _: i64) -> Result<(), E> {
        Ok(())
    }
    fn visit_u64<E: de::Error>(self, _: u64) -> Result<(), E> {
        Ok(())
    }
    fn visit_f64<E: de::Error>(self, _: f64) -> Result<(), E> {
        Ok(())
    }
    fn visit_str<E: de::Error>(self, _: &str) -> Result<(), E> {
        Ok(())
    }
    fn visit_string<E: de::Error>(self, _: String) -> Result<(), E> {
        Ok(())
    }
    fn visit_none<E: de::Error>(self) -> Result<(), E> {
        Ok(())
    }
    fn visit_unit<E: de::Error>(self) -> Result<(), E> {
        Ok(())
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while seq.next_element_seed(DupKeyGuard)?.is_some() {}
        Ok(())
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        let mut seen = std::collections::BTreeSet::<String>::new();
        while let Some(key) = map.next_key::<String>()? {
            if !seen.insert(key.clone()) {
                return Err(de::Error::custom(format!(
                    "duplicate object member name: {key}"
                )));
            }
            map.next_value_seed(DupKeyGuard)?;
        }
        Ok(())
    }
}

impl<'de> de::DeserializeSeed<'de> for DupKeyGuard {
    type Value = ();
    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_any(DupKeyGuard)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device_seq::DeviceSeq;
    use crate::envelope::{
        Classification, EVENT_SCHEMA_VERSION, EventPayload, EventType, MemoryNotePayload,
    };

    fn sample_key() -> SigningKey {
        SigningKey::from_bytes(&[7u8; 32])
    }

    fn sample_signable(key: &SigningKey) -> SignableEvent {
        let key_id = signing_key_id_from_verifying_key(&key.verifying_key()).unwrap();
        SignableEvent {
            schema_version: EVENT_SCHEMA_VERSION,
            event_id: "11111111-1111-4111-8111-111111111111".into(),
            brain_id: "22222222-2222-4222-8222-222222222222".into(),
            device_id: "device-a".into(),
            device_seq: DeviceSeq::from_u64(1).unwrap(),
            created_at: "2026-10-07T00:00:00Z".into(),
            event_type: EventType::MemoryNote,
            classification: Classification::Internal,
            signing_key_id: key_id,
            payload: EventPayload::MemoryNote(MemoryNotePayload::new("hello").unwrap()),
            prev_content_hash: ContentHashHex::zero(),
        }
    }

    #[test]
    fn accept_and_verify_round_trip() {
        let key = sample_key();
        let signable = sample_signable(&key);
        let accepted = accept_event(signable.clone(), &key).unwrap();
        verify_accepted_event(&accepted, &key.verifying_key()).unwrap();
        assert_eq!(accepted.content_hash, content_hash(&signable).unwrap());
    }

    #[test]
    fn content_hash_is_stable_for_same_signable() {
        let key = sample_key();
        let signable = sample_signable(&key);
        let a = content_hash(&signable).unwrap();
        let b = content_hash(&signable).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.as_str().len(), 64);
    }

    #[test]
    fn rejects_tampered_payload_on_verify() {
        let key = sample_key();
        let mut accepted = accept_event(sample_signable(&key), &key).unwrap();
        accepted.payload = EventPayload::MemoryNote(MemoryNotePayload::new("tampered").unwrap());
        let err = verify_accepted_event(&accepted, &key.verifying_key()).unwrap_err();
        assert!(matches!(err, EventError::Integrity(_)));
    }

    #[test]
    fn rejects_wrong_verifying_key() {
        let key = sample_key();
        let other = SigningKey::from_bytes(&[9u8; 32]);
        let accepted = accept_event(sample_signable(&key), &key).unwrap();
        let err = verify_accepted_event(&accepted, &other.verifying_key()).unwrap_err();
        assert!(matches!(err, EventError::Integrity(_)));
    }

    #[test]
    fn chain_link_rules() {
        check_first_event_prev_hash(&ContentHashHex::zero()).unwrap();
        assert!(
            check_first_event_prev_hash(&ContentHashHex::parse(&"ab".repeat(32)).unwrap()).is_err()
        );

        let prior = ContentHashHex::parse(&"cd".repeat(32)).unwrap();
        check_chain_link(&prior, &prior).unwrap();
        assert!(check_chain_link(&prior, &ContentHashHex::zero()).is_err());
    }

    #[test]
    fn jcs_is_compact_without_insignificant_whitespace() {
        let key = sample_key();
        let j = canonicalize_signable(&sample_signable(&key)).unwrap();
        let s = String::from_utf8(j).unwrap();
        assert!(!s.contains('\n'));
        assert!(!s.contains(" : "));
        assert!(s.starts_with('{'));
    }

    #[test]
    fn rejects_duplicate_json_object_keys() {
        let err = reject_duplicate_json_keys(r#"{"a":1,"a":2}"#).unwrap_err();
        assert!(
            err.to_string().contains("duplicate"),
            "unexpected error: {err}"
        );
        reject_duplicate_json_keys(r#"{"a":1,"b":{"c":1,"c":2}}"#).unwrap_err();
        reject_duplicate_json_keys(r#"{"a":1,"b":2}"#).unwrap();
    }

    #[test]
    fn signing_key_id_is_32_lowercase_hex() {
        let key = sample_key();
        let id = signing_key_id_from_verifying_key(&key.verifying_key()).unwrap();
        assert_eq!(id.as_str().len(), 32);
        assert!(
            id.as_str()
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        );
    }
}
