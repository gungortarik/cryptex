//! Bounded parser and signature verifier for the proposed M8 peer-bootstrap
//! exchange. A verified transcript is evidence of two signatures only; it does
//! not by itself establish local owner approval or a durable trust anchor.

use std::collections::HashSet;
use std::fmt;

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::Deserialize;
use serde_json::{Map, Number, Value};
use sha2::{Digest, Sha256};

use cryptex_core::ids::DeviceId;
use cryptex_core::validate::parse_uuid_str;

use crate::hex::SigningKeyId;
use crate::integrity::signing_key_id_from_verifying_key;

const MAX_MESSAGE_BYTES: usize = 64 * 1024;
const MAX_TRANSACTION_MESSAGES: usize = 16;
const MAX_TRANSACTION_BYTES: usize = 1024 * 1024;
const MAX_DEPTH: u8 = 4;
const MAX_OBJECT_MEMBERS: usize = 24;
const MAX_ARRAY_ITEMS: usize = 16;
const MAX_STRING_BYTES: usize = 16 * 1024;
const ZERO_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const MESSAGE_HASH_DOMAIN: &[u8] = b"cryptex-peer-bootstrap-message-v1";
const AGREEMENT_HASH_DOMAIN: &[u8] = b"cryptex-peer-bootstrap-agreement-v1";
const TRANSCRIPT_HASH_DOMAIN: &[u8] = b"cryptex-peer-bootstrap-transcript-v1";
const SIGNATURE_DOMAIN: &[u8] = b"cryptex-peer-bootstrap-sig-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapError(&'static str);

impl fmt::Display for BootstrapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for BootstrapError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapIdentity {
    pub device_id: DeviceId,
    pub signing_key_id: SigningKeyId,
    pub public_key: [u8; 32],
    pub fingerprint_sha256: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// A transcript whose complete signed message chain has been verified.
///
/// Fields are private so callers cannot construct verification evidence by
/// copying or inventing identity, transport, or transcript-hash values.
///
/// ```compile_fail
/// use cryptex_events::VerifiedBootstrapTranscript;
/// let _ = VerifiedBootstrapTranscript {
///     transaction_id: String::new(),
///     brain_id: String::new(),
///     initiator: unimplemented!(),
///     responder: unimplemented!(),
///     initiator_transport_device_id: String::new(),
///     responder_transport_device_id: String::new(),
///     offer_hash: String::new(),
///     proof_hash: String::new(),
///     initiator_approval_hash: String::new(),
///     responder_approval_hash: String::new(),
///     agreement_hash: String::new(),
///     transcript_hash: String::new(),
/// };
/// ```
pub struct VerifiedBootstrapTranscript {
    transaction_id: String,
    brain_id: String,
    initiator: BootstrapIdentity,
    responder: BootstrapIdentity,
    initiator_transport_device_id: String,
    responder_transport_device_id: String,
    offer_hash: String,
    proof_hash: String,
    initiator_approval_hash: String,
    responder_approval_hash: String,
    agreement_hash: String,
    transcript_hash: String,
    transcript: Value,
}

/// A single signature-verified initiator offer, before a peer proof exists.
/// The private fields keep journal/IPC code from treating parsed JSON as a
/// verified bootstrap identity or transaction binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedBootstrapOffer {
    transaction_id: String,
    brain_id: String,
    initiator: BootstrapIdentity,
    initiator_transport_device_id: String,
    offer_hash: String,
    message: Value,
}

impl VerifiedBootstrapOffer {
    #[must_use]
    pub fn transaction_id(&self) -> &str {
        &self.transaction_id
    }

    #[must_use]
    pub fn brain_id(&self) -> &str {
        &self.brain_id
    }

    #[must_use]
    pub const fn initiator(&self) -> &BootstrapIdentity {
        &self.initiator
    }

    #[must_use]
    pub fn initiator_transport_device_id(&self) -> &str {
        &self.initiator_transport_device_id
    }

    #[must_use]
    pub fn offer_hash(&self) -> &str {
        &self.offer_hash
    }

    /// Exact canonical signed offer value for persistence or explicit copying.
    #[must_use]
    pub const fn message(&self) -> &Value {
        &self.message
    }
}

impl VerifiedBootstrapTranscript {
    #[must_use]
    pub fn transaction_id(&self) -> &str {
        &self.transaction_id
    }

    #[must_use]
    pub fn brain_id(&self) -> &str {
        &self.brain_id
    }

    #[must_use]
    pub const fn initiator(&self) -> &BootstrapIdentity {
        &self.initiator
    }

    #[must_use]
    pub const fn responder(&self) -> &BootstrapIdentity {
        &self.responder
    }

    #[must_use]
    pub fn initiator_transport_device_id(&self) -> &str {
        &self.initiator_transport_device_id
    }

    #[must_use]
    pub fn responder_transport_device_id(&self) -> &str {
        &self.responder_transport_device_id
    }

    #[must_use]
    pub fn offer_hash(&self) -> &str {
        &self.offer_hash
    }

    #[must_use]
    pub fn proof_hash(&self) -> &str {
        &self.proof_hash
    }

    #[must_use]
    pub fn initiator_approval_hash(&self) -> &str {
        &self.initiator_approval_hash
    }

    #[must_use]
    pub fn responder_approval_hash(&self) -> &str {
        &self.responder_approval_hash
    }

    #[must_use]
    pub fn agreement_hash(&self) -> &str {
        &self.agreement_hash
    }

    #[must_use]
    pub fn transcript_hash(&self) -> &str {
        &self.transcript_hash
    }

    /// Complete canonical signed message set verified for this transcript.
    /// The value is immutable verification output, not caller-provided state.
    #[must_use]
    pub const fn transcript(&self) -> &Value {
        &self.transcript
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapApprovalRole {
    Initiator,
    Responder,
}

/// An authenticated bootstrap prefix that is ready for one local owner
/// approval. Private fields prevent callers from constructing an unsigned
/// approval context from unverified request data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedBootstrapApprovalRequest {
    role: BootstrapApprovalRole,
    transaction_id: String,
    brain_id: String,
    local_identity: BootstrapIdentity,
    peer_identity: BootstrapIdentity,
    local_transport_device_id: String,
    peer_transport_device_id: String,
    agreement_hash: String,
    previous_message_hash: String,
}

impl VerifiedBootstrapApprovalRequest {
    pub fn role(&self) -> BootstrapApprovalRole {
        self.role
    }

    pub fn transaction_id(&self) -> &str {
        &self.transaction_id
    }

    pub fn brain_id(&self) -> &str {
        &self.brain_id
    }

    pub fn local_identity(&self) -> &BootstrapIdentity {
        &self.local_identity
    }

    pub fn peer_identity(&self) -> &BootstrapIdentity {
        &self.peer_identity
    }

    pub fn local_transport_device_id(&self) -> &str {
        &self.local_transport_device_id
    }

    pub fn peer_transport_device_id(&self) -> &str {
        &self.peer_transport_device_id
    }

    pub fn agreement_hash(&self) -> &str {
        &self.agreement_hash
    }

    pub fn previous_message_hash(&self) -> &str {
        &self.previous_message_hash
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MessageType {
    Offer,
    Proof,
    Approval,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireMessage {
    protocol_version: u64,
    message_type: String,
    transaction_id: String,
    brain_id: String,
    sender_device_id: String,
    sender_signing_key_id: String,
    sender_public_key: String,
    sender_transport_device_id: String,
    peer_device_id: Option<String>,
    peer_signing_key_id: Option<String>,
    peer_public_key: Option<String>,
    peer_transport_device_id: Option<String>,
    previous_hash: String,
    payload: Value,
    signature: String,
}

#[derive(Debug)]
struct ParsedMessage {
    wire: WireMessage,
    value: Value,
    message_type: MessageType,
    sender: BootstrapIdentity,
    sender_transport_id: String,
    peer: Option<BootstrapIdentity>,
    peer_transport_id: Option<String>,
    payload_hash: Option<String>,
    approval_role: Option<&'static str>,
    agreement_hash: Option<String>,
    hash: String,
}

struct ParsedPayload {
    hash: Option<String>,
    approval_role: Option<&'static str>,
    agreement_hash: Option<String>,
}

struct BoundedJsonParser<'a> {
    raw: &'a [u8],
    cursor: usize,
}

impl BoundedJsonParser<'_> {
    fn parse_value(
        &mut self,
        depth: u8,
        protocol_version_allowed: bool,
    ) -> Result<Value, BootstrapError> {
        self.skip_whitespace();
        match self.peek() {
            Some(b'{') => self.parse_object(depth),
            Some(b'[') => self.parse_array(depth),
            Some(b'"') => self.parse_string(false).map(Value::String),
            Some(b't') => {
                self.parse_literal(b"true")?;
                Ok(Value::Bool(true))
            }
            Some(b'f') => {
                self.parse_literal(b"false")?;
                Ok(Value::Bool(false))
            }
            Some(b'n') => {
                self.parse_literal(b"null")?;
                Ok(Value::Null)
            }
            Some(b'1') if protocol_version_allowed => {
                self.cursor += 1;
                if self
                    .peek()
                    .is_some_and(|byte| !is_json_value_delimiter(byte))
                {
                    return Err(BootstrapError(
                        "protocol_version must be the exact integer 1",
                    ));
                }
                Ok(Value::Number(Number::from(1)))
            }
            Some(b'-' | b'0'..=b'9') => Err(BootstrapError(
                "only protocol_version integer 1 is permitted in bootstrap JSON",
            )),
            _ => Err(BootstrapError("bootstrap JSON value is malformed")),
        }
    }

    fn parse_object(&mut self, depth: u8) -> Result<Value, BootstrapError> {
        self.require_depth(depth)?;
        self.cursor += 1;
        self.skip_whitespace();
        let mut values = Map::new();
        let mut seen = HashSet::new();
        if self.consume_if(b'}') {
            return Ok(Value::Object(values));
        }
        loop {
            if values.len() >= MAX_OBJECT_MEMBERS {
                return Err(BootstrapError("bootstrap object member limit exceeded"));
            }
            self.skip_whitespace();
            if self.peek() != Some(b'"') {
                return Err(BootstrapError("bootstrap object key must be a string"));
            }
            let key = self.parse_string(true)?;
            if !seen.insert(key.clone()) {
                return Err(BootstrapError("duplicate decoded bootstrap object key"));
            }
            self.skip_whitespace();
            if !self.consume_if(b':') {
                return Err(BootstrapError("bootstrap object key is missing a colon"));
            }
            let protocol_version_allowed = depth == 1 && key == "protocol_version";
            let value = self.parse_value(depth.saturating_add(1), protocol_version_allowed)?;
            values.insert(key, value);
            self.skip_whitespace();
            if self.consume_if(b'}') {
                return Ok(Value::Object(values));
            }
            if !self.consume_if(b',') {
                return Err(BootstrapError("bootstrap object separator is invalid"));
            }
        }
    }

    fn parse_array(&mut self, depth: u8) -> Result<Value, BootstrapError> {
        self.require_depth(depth)?;
        self.cursor += 1;
        self.skip_whitespace();
        let mut values = Vec::new();
        if self.consume_if(b']') {
            return Ok(Value::Array(values));
        }
        loop {
            if values.len() >= MAX_ARRAY_ITEMS {
                return Err(BootstrapError("bootstrap array item limit exceeded"));
            }
            values.push(self.parse_value(depth.saturating_add(1), false)?);
            self.skip_whitespace();
            if self.consume_if(b']') {
                return Ok(Value::Array(values));
            }
            if !self.consume_if(b',') {
                return Err(BootstrapError("bootstrap array separator is invalid"));
            }
        }
    }

    fn parse_string(&mut self, object_key: bool) -> Result<String, BootstrapError> {
        if !self.consume_if(b'"') {
            return Err(BootstrapError("bootstrap string is malformed"));
        }
        let mut output = String::new();
        loop {
            let Some(byte) = self.peek() else {
                return Err(BootstrapError("bootstrap string is unterminated"));
            };
            match byte {
                b'"' => {
                    self.cursor += 1;
                    return Ok(output);
                }
                b'\\' => {
                    self.cursor += 1;
                    let escaped = match self.peek() {
                        Some(b'"') => '"',
                        Some(b'\\') => '\\',
                        Some(b'/') => '/',
                        Some(b'b') => '\u{0008}',
                        Some(b'f') => '\u{000c}',
                        Some(b'n') => '\n',
                        Some(b'r') => '\r',
                        Some(b't') => '\t',
                        Some(b'u') => {
                            let character = self.parse_unicode_escape()?;
                            self.push_char(&mut output, character, object_key)?;
                            continue;
                        }
                        _ => return Err(BootstrapError("bootstrap string escape is invalid")),
                    };
                    self.cursor += 1;
                    self.push_char(&mut output, escaped, object_key)?;
                }
                0x00..=0x1f => {
                    return Err(BootstrapError("unescaped control byte in bootstrap string"));
                }
                0x20..=0x7f => {
                    self.cursor += 1;
                    self.push_char(&mut output, byte as char, object_key)?;
                }
                _ => {
                    let width = match byte {
                        0xc2..=0xdf => 2,
                        0xe0..=0xef => 3,
                        0xf0..=0xf4 => 4,
                        _ => return Err(BootstrapError("bootstrap string contains invalid UTF-8")),
                    };
                    let end = self.cursor + width;
                    let chunk = self
                        .raw
                        .get(self.cursor..end)
                        .ok_or(BootstrapError("bootstrap string contains invalid UTF-8"))?;
                    let text = std::str::from_utf8(chunk)
                        .map_err(|_| BootstrapError("bootstrap string contains invalid UTF-8"))?;
                    let character = text
                        .chars()
                        .next()
                        .ok_or(BootstrapError("bootstrap string contains invalid UTF-8"))?;
                    self.cursor = end;
                    self.push_char(&mut output, character, object_key)?;
                }
            }
        }
    }

    fn parse_unicode_escape(&mut self) -> Result<char, BootstrapError> {
        self.cursor += 1; // consume `u`
        let first = self.parse_hex_quad()?;
        let scalar = match first {
            0xd800..=0xdbff => {
                if self.raw.get(self.cursor..self.cursor + 2) != Some(b"\\u") {
                    return Err(BootstrapError("unpaired bootstrap Unicode surrogate"));
                }
                self.cursor += 2;
                let second = self.parse_hex_quad()?;
                if !(0xdc00..=0xdfff).contains(&second) {
                    return Err(BootstrapError("unpaired bootstrap Unicode surrogate"));
                }
                0x10000 + ((u32::from(first) - 0xd800) << 10) + (u32::from(second) - 0xdc00)
            }
            0xdc00..=0xdfff => {
                return Err(BootstrapError("unpaired bootstrap Unicode surrogate"));
            }
            value => u32::from(value),
        };
        char::from_u32(scalar).ok_or(BootstrapError("bootstrap Unicode scalar is invalid"))
    }

    fn parse_hex_quad(&mut self) -> Result<u16, BootstrapError> {
        let mut value = 0u16;
        for _ in 0..4 {
            let byte = self
                .peek()
                .ok_or(BootstrapError("bootstrap Unicode escape is incomplete"))?;
            self.cursor += 1;
            let nibble = hex_nibble(byte)
                .or_else(|| match byte {
                    b'A'..=b'F' => Some(byte - b'A' + 10),
                    _ => None,
                })
                .ok_or(BootstrapError("bootstrap Unicode escape is invalid"))?;
            value = (value << 4) | u16::from(nibble);
        }
        Ok(value)
    }

    fn parse_literal(&mut self, literal: &[u8]) -> Result<(), BootstrapError> {
        if self.raw.get(self.cursor..self.cursor + literal.len()) != Some(literal) {
            return Err(BootstrapError("bootstrap JSON literal is malformed"));
        }
        self.cursor += literal.len();
        if self
            .peek()
            .is_some_and(|byte| !is_json_value_delimiter(byte))
        {
            return Err(BootstrapError("bootstrap JSON literal boundary is invalid"));
        }
        Ok(())
    }

    fn push_char(
        &self,
        output: &mut String,
        character: char,
        object_key: bool,
    ) -> Result<(), BootstrapError> {
        if object_key && !character.is_ascii() {
            return Err(BootstrapError("bootstrap object keys must be ASCII"));
        }
        if output.len() + character.len_utf8() > MAX_STRING_BYTES {
            return Err(BootstrapError(
                "bootstrap string exceeds the decoded-byte limit",
            ));
        }
        output.push(character);
        Ok(())
    }

    fn require_depth(&self, depth: u8) -> Result<(), BootstrapError> {
        if depth > MAX_DEPTH {
            return Err(BootstrapError("bootstrap JSON nesting limit exceeded"));
        }
        Ok(())
    }

    fn skip_whitespace(&mut self) {
        while self
            .peek()
            .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
        {
            self.cursor += 1;
        }
    }

    fn consume_if(&mut self, expected: u8) -> bool {
        if self.peek() == Some(expected) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<u8> {
        self.raw.get(self.cursor).copied()
    }
}

fn is_json_value_delimiter(byte: u8) -> bool {
    matches!(byte, b',' | b'}' | b']' | b' ' | b'\t' | b'\r' | b'\n')
}

/// Verify one signed initiator offer before a proof has been received.
pub fn verify_bootstrap_offer(raw: &[u8]) -> Result<VerifiedBootstrapOffer, BootstrapError> {
    let parsed = parse_and_verify_message(raw)?;
    if parsed.message_type != MessageType::Offer {
        return Err(BootstrapError("bootstrap message is not an offer"));
    }
    Ok(VerifiedBootstrapOffer {
        transaction_id: parsed.wire.transaction_id,
        brain_id: parsed.wire.brain_id,
        initiator: parsed.sender,
        initiator_transport_device_id: parsed.sender_transport_id,
        offer_hash: parsed.hash,
        message: parsed.value,
    })
}

/// Parse one bootstrap message after applying raw-byte and incremental JSON
/// limits. Each string's decoded-byte bound is checked as its scalars are read.
pub fn parse_bootstrap_message(raw: &[u8]) -> Result<Value, BootstrapError> {
    let value = parse_bounded_json(raw)?;
    if !value.is_object() {
        return Err(BootstrapError("bootstrap message root must be an object"));
    }
    require_exact_keys(
        value.as_object().expect("root object checked"),
        &[
            "protocol_version",
            "message_type",
            "transaction_id",
            "brain_id",
            "sender_device_id",
            "sender_signing_key_id",
            "sender_public_key",
            "sender_transport_device_id",
            "peer_device_id",
            "peer_signing_key_id",
            "peer_public_key",
            "peer_transport_device_id",
            "previous_hash",
            "payload",
            "signature",
        ],
    )?;
    Ok(value)
}

fn parse_bounded_json(raw: &[u8]) -> Result<Value, BootstrapError> {
    if raw.len() > MAX_MESSAGE_BYTES {
        return Err(BootstrapError("bootstrap message exceeds 64 KiB"));
    }
    std::str::from_utf8(raw).map_err(|_| BootstrapError("bootstrap message is not UTF-8"))?;
    let mut parser = BoundedJsonParser { raw, cursor: 0 };
    parser.skip_whitespace();
    let value = parser.parse_value(1, false)?;
    parser.skip_whitespace();
    if parser.cursor != raw.len() {
        return Err(BootstrapError("bootstrap message has trailing JSON"));
    }
    Ok(value)
}

/// Verify the exact four-message offer/proof/two-approval exchange.
pub fn verify_bootstrap_transcript(
    raw_messages: &[&[u8]],
) -> Result<VerifiedBootstrapTranscript, BootstrapError> {
    if raw_messages.len() > MAX_TRANSACTION_MESSAGES {
        return Err(BootstrapError(
            "bootstrap transaction message limit exceeded",
        ));
    }
    let total_bytes = raw_messages
        .iter()
        .try_fold(0usize, |sum, message| sum.checked_add(message.len()));
    if total_bytes.is_none_or(|total| total > MAX_TRANSACTION_BYTES) {
        return Err(BootstrapError("bootstrap transaction byte limit exceeded"));
    }
    if raw_messages.len() != 4 {
        return Err(BootstrapError(
            "bootstrap transcript must contain four messages",
        ));
    }

    let mut messages = Vec::with_capacity(4);
    for raw in raw_messages {
        messages.push(parse_and_verify_message(raw)?);
    }
    let [offer, proof, initiator_approval, responder_approval] = messages.as_slice() else {
        unreachable!("exact length checked above")
    };
    if offer.message_type != MessageType::Offer
        || proof.message_type != MessageType::Proof
        || initiator_approval.message_type != MessageType::Approval
        || responder_approval.message_type != MessageType::Approval
    {
        return Err(BootstrapError(
            "bootstrap transcript message order is invalid",
        ));
    }
    for message in &messages[1..] {
        if message.wire.transaction_id != offer.wire.transaction_id
            || message.wire.brain_id != offer.wire.brain_id
        {
            return Err(BootstrapError(
                "bootstrap transaction or Brain binding differs",
            ));
        }
    }
    if offer.wire.previous_hash != ZERO_HASH {
        return Err(BootstrapError("bootstrap offer previous_hash must be zero"));
    }
    if proof.wire.previous_hash != offer.hash
        || proof.payload_hash.as_deref() != Some(offer.hash.as_str())
    {
        return Err(BootstrapError(
            "bootstrap proof does not bind the exact offer",
        ));
    }

    let initiator = &offer.sender;
    let responder = &proof.sender;
    require_same_peer(proof, initiator, &offer.sender_transport_id)?;
    require_same_peer(initiator_approval, responder, &proof.sender_transport_id)?;
    require_same_sender(initiator_approval, initiator, &offer.sender_transport_id)?;
    require_same_peer(responder_approval, initiator, &offer.sender_transport_id)?;
    require_same_sender(responder_approval, responder, &proof.sender_transport_id)?;

    if initiator_approval.approval_role != Some("initiator")
        || responder_approval.approval_role != Some("responder")
    {
        return Err(BootstrapError("bootstrap approval roles are invalid"));
    }
    if initiator_approval.wire.previous_hash != proof.hash
        || responder_approval.wire.previous_hash != initiator_approval.hash
    {
        return Err(BootstrapError("bootstrap approval hash chain is invalid"));
    }

    let proof_hash = proof.hash.clone();
    let initiator_approval_hash = initiator_approval.hash.clone();
    let responder_approval_hash = responder_approval.hash.clone();
    let agreement_value = serde_json::json!({
        "offer_hash": offer.hash,
        "proof_hash": proof_hash,
        "brain_id": offer.wire.brain_id,
        "initiator_identity": identity_value(initiator),
        "responder_identity": identity_value(responder),
        "transport_association": {
            "initiator_transport_device_id": offer.sender_transport_id,
            "responder_transport_device_id": proof.sender_transport_id
        }
    });
    let agreement_hash = domain_hash(AGREEMENT_HASH_DOMAIN, &agreement_value)?;
    if initiator_approval.agreement_hash.as_deref() != Some(agreement_hash.as_str())
        || responder_approval.agreement_hash.as_deref() != Some(agreement_hash.as_str())
    {
        return Err(BootstrapError("bootstrap approval agreement hash differs"));
    }

    let transcript = Value::Array(
        messages
            .iter()
            .map(|message| message.value.clone())
            .collect(),
    );
    let transcript_hash = domain_hash(TRANSCRIPT_HASH_DOMAIN, &transcript)?;
    Ok(VerifiedBootstrapTranscript {
        transaction_id: offer.wire.transaction_id.clone(),
        brain_id: offer.wire.brain_id.clone(),
        initiator: initiator.clone(),
        responder: responder.clone(),
        initiator_transport_device_id: offer.sender_transport_id.clone(),
        responder_transport_device_id: proof.sender_transport_id.clone(),
        offer_hash: offer.hash.clone(),
        proof_hash,
        initiator_approval_hash,
        responder_approval_hash,
        agreement_hash,
        transcript_hash,
        transcript,
    })
}

/// Authenticate the available offer/proof prefix and derive the immutable
/// local approval display context. Initiator approvals use the two-message
/// prefix; responder approvals additionally require the signed initiator
/// approval. This function never creates trust or writes canonical data.
pub fn verify_bootstrap_approval_request(
    raw_messages: &[&[u8]],
    role: BootstrapApprovalRole,
) -> Result<VerifiedBootstrapApprovalRequest, BootstrapError> {
    let expected_len = match role {
        BootstrapApprovalRole::Initiator => 2,
        BootstrapApprovalRole::Responder => 3,
    };
    if raw_messages.len() != expected_len {
        return Err(BootstrapError(
            "bootstrap approval prefix has the wrong number of messages",
        ));
    }
    let total_bytes = raw_messages
        .iter()
        .try_fold(0usize, |sum, message| sum.checked_add(message.len()));
    if total_bytes.is_none_or(|total| total > MAX_TRANSACTION_BYTES) {
        return Err(BootstrapError("bootstrap transaction byte limit exceeded"));
    }
    let mut messages = Vec::with_capacity(expected_len);
    for raw in raw_messages {
        messages.push(parse_and_verify_message(raw)?);
    }
    let offer = &messages[0];
    let proof = &messages[1];
    if offer.message_type != MessageType::Offer
        || proof.message_type != MessageType::Proof
        || offer.wire.previous_hash != ZERO_HASH
        || proof.wire.transaction_id != offer.wire.transaction_id
        || proof.wire.brain_id != offer.wire.brain_id
        || proof.wire.previous_hash != offer.hash
        || proof.payload_hash.as_deref() != Some(offer.hash.as_str())
    {
        return Err(BootstrapError(
            "bootstrap offer/proof prefix binding is invalid",
        ));
    }
    require_same_peer(proof, &offer.sender, &offer.sender_transport_id)?;

    let agreement_hash = agreement_hash(offer, proof)?;
    let previous_message_hash = match role {
        BootstrapApprovalRole::Initiator => proof.hash.clone(),
        BootstrapApprovalRole::Responder => {
            let approval = &messages[2];
            if approval.message_type != MessageType::Approval
                || approval.wire.transaction_id != offer.wire.transaction_id
                || approval.wire.brain_id != offer.wire.brain_id
                || approval.approval_role != Some("initiator")
                || approval.wire.previous_hash != proof.hash
                || approval.agreement_hash.as_deref() != Some(agreement_hash.as_str())
            {
                return Err(BootstrapError(
                    "initiator approval does not bind the verified prefix",
                ));
            }
            require_same_peer(approval, &proof.sender, &proof.sender_transport_id)?;
            require_same_sender(approval, &offer.sender, &offer.sender_transport_id)?;
            approval.hash.clone()
        }
    };

    let (local, peer, local_transport, peer_transport) = match role {
        BootstrapApprovalRole::Initiator => (
            &offer.sender,
            &proof.sender,
            &offer.sender_transport_id,
            &proof.sender_transport_id,
        ),
        BootstrapApprovalRole::Responder => (
            &proof.sender,
            &offer.sender,
            &proof.sender_transport_id,
            &offer.sender_transport_id,
        ),
    };
    Ok(VerifiedBootstrapApprovalRequest {
        role,
        transaction_id: offer.wire.transaction_id.clone(),
        brain_id: offer.wire.brain_id.clone(),
        local_identity: local.clone(),
        peer_identity: peer.clone(),
        local_transport_device_id: local_transport.clone(),
        peer_transport_device_id: peer_transport.clone(),
        agreement_hash,
        previous_message_hash,
    })
}

/// Create the local signed approval message for a request derived by
/// `verify_bootstrap_approval_request`. The caller must first consume its
/// connection-bound owner-approval capability; this function does not itself
/// establish that a human approved the displayed snapshot.
pub fn sign_bootstrap_approval(
    request: &VerifiedBootstrapApprovalRequest,
    signing_key: &SigningKey,
) -> Result<Vec<u8>, BootstrapError> {
    let verifying_key = signing_key.verifying_key();
    let public_key = verifying_key.to_bytes();
    if public_key != request.local_identity.public_key
        || signing_key_id_from_verifying_key(&verifying_key)
            .map_err(|_| BootstrapError("bootstrap approval key ID derivation failed"))?
            != request.local_identity.signing_key_id
    {
        return Err(BootstrapError(
            "bootstrap approval key does not match local identity",
        ));
    }
    let role = match request.role {
        BootstrapApprovalRole::Initiator => "initiator",
        BootstrapApprovalRole::Responder => "responder",
    };
    let mut message = serde_json::json!({
        "protocol_version": 1,
        "message_type": "approval",
        "transaction_id": request.transaction_id,
        "brain_id": request.brain_id,
        "sender_device_id": request.local_identity.device_id.as_str(),
        "sender_signing_key_id": request.local_identity.signing_key_id.as_str(),
        "sender_public_key": encode_lower_hex(&request.local_identity.public_key),
        "sender_transport_device_id": request.local_transport_device_id,
        "peer_device_id": request.peer_identity.device_id.as_str(),
        "peer_signing_key_id": request.peer_identity.signing_key_id.as_str(),
        "peer_public_key": encode_lower_hex(&request.peer_identity.public_key),
        "peer_transport_device_id": request.peer_transport_device_id,
        "previous_hash": request.previous_message_hash,
        "payload": {
            "agreement_hash": request.agreement_hash,
            "role": role
        }
    });
    let canonical_signable = serde_json_canonicalizer::to_vec(&message)
        .map_err(|_| BootstrapError("bootstrap approval JCS failed"))?;
    let mut signature_preimage =
        Vec::with_capacity(SIGNATURE_DOMAIN.len() + 1 + canonical_signable.len());
    signature_preimage.extend_from_slice(SIGNATURE_DOMAIN);
    signature_preimage.push(0);
    signature_preimage.extend_from_slice(&canonical_signable);
    let signature = signing_key.sign(&signature_preimage);
    message
        .as_object_mut()
        .expect("bootstrap approval root is an object")
        .insert(
            "signature".into(),
            Value::String(encode_lower_hex(&signature.to_bytes())),
        );
    serde_json_canonicalizer::to_vec(&message)
        .map_err(|_| BootstrapError("bootstrap approval serialization failed"))
}

/// Create the initiator's first signed bootstrap message. The caller supplies
/// fresh transaction and nonce bytes from the operating-system CSPRNG; this
/// function validates their wire encoding and binds the message to the exact
/// local DeviceId, transport association, Brain, and signing key.
pub fn create_bootstrap_offer(
    brain_id: &str,
    transaction_id: &str,
    sender_device_id: &DeviceId,
    sender_transport_device_id: &str,
    initiator_nonce: &[u8; 32],
    signing_key: &SigningKey,
) -> Result<Vec<u8>, BootstrapError> {
    validate_canonical_brain_id(brain_id)?;
    validate_lower_hex(transaction_id, 64)?;
    validate_transport_id(sender_transport_device_id)?;
    let local = bootstrap_identity_from_key(sender_device_id.clone(), signing_key)?;
    let message = serde_json::json!({
        "protocol_version": 1,
        "message_type": "offer",
        "transaction_id": transaction_id,
        "brain_id": brain_id,
        "sender_device_id": local.device_id.as_str(),
        "sender_signing_key_id": local.signing_key_id.as_str(),
        "sender_public_key": encode_lower_hex(&local.public_key),
        "sender_transport_device_id": sender_transport_device_id,
        "peer_device_id": null,
        "peer_signing_key_id": null,
        "peer_public_key": null,
        "peer_transport_device_id": null,
        "previous_hash": ZERO_HASH,
        "payload": { "initiator_nonce": encode_lower_hex(initiator_nonce) },
        "signature": ""
    });
    sign_and_validate_bootstrap_message(message, signing_key, MessageType::Offer)
}

/// Create a responder proof only from a valid signed offer. The offer's exact
/// identity, Brain, transaction, transport binding, and message hash are
/// copied into the proof; no corresponding values are trusted from the caller.
pub fn create_bootstrap_proof(
    offer: &[u8],
    responder_device_id: &DeviceId,
    responder_transport_device_id: &str,
    responder_nonce: &[u8; 32],
    signing_key: &SigningKey,
) -> Result<Vec<u8>, BootstrapError> {
    let offer = parse_and_verify_message(offer)?;
    if offer.message_type != MessageType::Offer {
        return Err(BootstrapError("bootstrap proof requires a signed offer"));
    }
    validate_transport_id(responder_transport_device_id)?;
    let responder = bootstrap_identity_from_key(responder_device_id.clone(), signing_key)?;
    if responder.device_id == offer.sender.device_id
        || responder.signing_key_id == offer.sender.signing_key_id
    {
        return Err(BootstrapError(
            "bootstrap cannot pair an identity with itself",
        ));
    }
    let message = serde_json::json!({
        "protocol_version": 1,
        "message_type": "proof",
        "transaction_id": offer.wire.transaction_id,
        "brain_id": offer.wire.brain_id,
        "sender_device_id": responder.device_id.as_str(),
        "sender_signing_key_id": responder.signing_key_id.as_str(),
        "sender_public_key": encode_lower_hex(&responder.public_key),
        "sender_transport_device_id": responder_transport_device_id,
        "peer_device_id": offer.sender.device_id.as_str(),
        "peer_signing_key_id": offer.sender.signing_key_id.as_str(),
        "peer_public_key": encode_lower_hex(&offer.sender.public_key),
        "peer_transport_device_id": offer.sender_transport_id,
        "previous_hash": offer.hash,
        "payload": {
            "offer_hash": offer.hash,
            "responder_nonce": encode_lower_hex(responder_nonce)
        },
        "signature": ""
    });
    sign_and_validate_bootstrap_message(message, signing_key, MessageType::Proof)
}

fn bootstrap_identity_from_key(
    device_id: DeviceId,
    signing_key: &SigningKey,
) -> Result<BootstrapIdentity, BootstrapError> {
    let verifying_key = signing_key.verifying_key();
    let signing_key_id = signing_key_id_from_verifying_key(&verifying_key)
        .map_err(|_| BootstrapError("bootstrap signing key ID derivation failed"))?;
    Ok(BootstrapIdentity {
        device_id,
        signing_key_id,
        public_key: verifying_key.to_bytes(),
        fingerprint_sha256: Sha256::digest(verifying_key.as_bytes()).into(),
    })
}

fn sign_and_validate_bootstrap_message(
    mut message: Value,
    signing_key: &SigningKey,
    expected_type: MessageType,
) -> Result<Vec<u8>, BootstrapError> {
    let object = message
        .as_object_mut()
        .ok_or(BootstrapError("bootstrap message root must be an object"))?;
    object.remove("signature");
    let canonical_signable = serde_json_canonicalizer::to_vec(&message)
        .map_err(|_| BootstrapError("bootstrap message JCS failed"))?;
    let mut signature_preimage =
        Vec::with_capacity(SIGNATURE_DOMAIN.len() + 1 + canonical_signable.len());
    signature_preimage.extend_from_slice(SIGNATURE_DOMAIN);
    signature_preimage.push(0);
    signature_preimage.extend_from_slice(&canonical_signable);
    let signature = signing_key.sign(&signature_preimage);
    message
        .as_object_mut()
        .expect("bootstrap message root is an object")
        .insert(
            "signature".into(),
            Value::String(encode_lower_hex(&signature.to_bytes())),
        );
    let encoded = serde_json_canonicalizer::to_vec(&message)
        .map_err(|_| BootstrapError("bootstrap message serialization failed"))?;
    let parsed = parse_and_verify_message(&encoded)?;
    if parsed.message_type != expected_type {
        return Err(BootstrapError("generated bootstrap message type mismatch"));
    }
    Ok(encoded)
}

fn agreement_hash(offer: &ParsedMessage, proof: &ParsedMessage) -> Result<String, BootstrapError> {
    let value = serde_json::json!({
        "offer_hash": offer.hash,
        "proof_hash": proof.hash,
        "brain_id": offer.wire.brain_id,
        "initiator_identity": identity_value(&offer.sender),
        "responder_identity": identity_value(&proof.sender),
        "transport_association": {
            "initiator_transport_device_id": offer.sender_transport_id,
            "responder_transport_device_id": proof.sender_transport_id
        }
    });
    domain_hash(AGREEMENT_HASH_DOMAIN, &value)
}

/// Verify the canonical transcript object stored in an authenticated v3 event.
/// The event parser has already bounded and duplicate-checked the enclosing
/// JSON; each message is nevertheless serialized and re-parsed through the
/// stricter bootstrap limits before any trust derivation.
pub fn verify_bootstrap_transcript_values(
    messages: &[Value],
) -> Result<VerifiedBootstrapTranscript, BootstrapError> {
    if messages.len() > MAX_TRANSACTION_MESSAGES {
        return Err(BootstrapError(
            "bootstrap transaction message limit exceeded",
        ));
    }
    let mut encoded = Vec::with_capacity(messages.len());
    let mut total = 0usize;
    for message in messages {
        let bytes = serde_json_canonicalizer::to_vec(message)
            .map_err(|_| BootstrapError("embedded bootstrap message JCS failed"))?;
        total = total
            .checked_add(bytes.len())
            .ok_or(BootstrapError("bootstrap transaction byte limit exceeded"))?;
        if total > MAX_TRANSACTION_BYTES {
            return Err(BootstrapError("bootstrap transaction byte limit exceeded"));
        }
        encoded.push(bytes);
    }
    let refs: Vec<_> = encoded.iter().map(Vec::as_slice).collect();
    verify_bootstrap_transcript(&refs)
}

fn parse_and_verify_message(raw: &[u8]) -> Result<ParsedMessage, BootstrapError> {
    let value = parse_bootstrap_message(raw)?;
    let wire: WireMessage = serde_json::from_value(value.clone())
        .map_err(|_| BootstrapError("bootstrap message has invalid or unknown fields"))?;
    if wire.protocol_version != 1 {
        return Err(BootstrapError("unsupported bootstrap protocol version"));
    }
    let message_type = match wire.message_type.as_str() {
        "offer" => MessageType::Offer,
        "proof" => MessageType::Proof,
        "approval" => MessageType::Approval,
        _ => return Err(BootstrapError("unknown bootstrap message type")),
    };
    validate_lower_hex(&wire.transaction_id, 64)?;
    validate_canonical_brain_id(&wire.brain_id)?;
    validate_lower_hex(&wire.previous_hash, 64)?;
    let sender = parse_identity(
        &wire.sender_device_id,
        &wire.sender_signing_key_id,
        &wire.sender_public_key,
    )?;
    validate_transport_id(&wire.sender_transport_device_id)?;

    let peer_fields = [
        wire.peer_device_id.as_ref(),
        wire.peer_signing_key_id.as_ref(),
        wire.peer_public_key.as_ref(),
        wire.peer_transport_device_id.as_ref(),
    ];
    let peer = if peer_fields.iter().all(Option::is_none) {
        None
    } else if peer_fields.iter().all(Option::is_some) {
        let peer = parse_identity(
            wire.peer_device_id
                .as_deref()
                .expect("all peer fields present"),
            wire.peer_signing_key_id
                .as_deref()
                .expect("all peer fields present"),
            wire.peer_public_key
                .as_deref()
                .expect("all peer fields present"),
        )?;
        validate_transport_id(
            wire.peer_transport_device_id
                .as_deref()
                .expect("all peer fields present"),
        )?;
        Some(peer)
    } else {
        return Err(BootstrapError(
            "bootstrap peer identity fields are incomplete",
        ));
    };
    if message_type == MessageType::Offer && peer.is_some()
        || message_type != MessageType::Offer && peer.is_none()
    {
        return Err(BootstrapError(
            "bootstrap peer fields do not match message type",
        ));
    }
    if message_type == MessageType::Offer && wire.previous_hash != ZERO_HASH {
        return Err(BootstrapError("bootstrap offer previous_hash must be zero"));
    }

    let ParsedPayload {
        hash: payload_hash,
        approval_role,
        agreement_hash,
    } = parse_payload(message_type, &wire.payload)?;
    let signature_bytes = decode_lower_hex(&wire.signature, 128)?;
    let signature = Signature::from_slice(&signature_bytes)
        .map_err(|_| BootstrapError("bootstrap signature has invalid length"))?;
    let sender_key = VerifyingKey::from_bytes(&sender.public_key)
        .map_err(|_| BootstrapError("bootstrap sender public key is invalid"))?;
    if signing_key_id_from_verifying_key(&sender_key)
        .map_err(|_| BootstrapError("bootstrap sender key ID derivation failed"))?
        != sender.signing_key_id
    {
        return Err(BootstrapError(
            "bootstrap sender key ID does not match public key",
        ));
    }
    let mut signable = value.clone();
    signable
        .as_object_mut()
        .expect("bootstrap root object checked")
        .remove("signature");
    let canonical_signable = serde_json_canonicalizer::to_vec(&signable)
        .map_err(|_| BootstrapError("bootstrap message JCS failed"))?;
    let mut signature_preimage =
        Vec::with_capacity(SIGNATURE_DOMAIN.len() + 1 + canonical_signable.len());
    signature_preimage.extend_from_slice(SIGNATURE_DOMAIN);
    signature_preimage.push(0);
    signature_preimage.extend_from_slice(&canonical_signable);
    sender_key
        .verify(&signature_preimage, &signature)
        .map_err(|_| BootstrapError("bootstrap message signature is invalid"))?;
    let hash = domain_hash(MESSAGE_HASH_DOMAIN, &value)?;
    let sender_transport_id = wire.sender_transport_device_id.clone();
    let peer_transport_id = wire.peer_transport_device_id.clone();
    Ok(ParsedMessage {
        wire,
        value,
        message_type,
        sender,
        sender_transport_id,
        peer,
        peer_transport_id,
        payload_hash,
        approval_role,
        agreement_hash,
        hash,
    })
}

fn parse_payload(
    message_type: MessageType,
    payload: &Value,
) -> Result<ParsedPayload, BootstrapError> {
    let object = payload
        .as_object()
        .ok_or(BootstrapError("bootstrap payload must be an object"))?;
    match message_type {
        MessageType::Offer => {
            require_exact_keys(object, &["initiator_nonce"])?;
            let nonce = string_field(object, "initiator_nonce")?;
            validate_lower_hex(&nonce, 64)?;
            let _ = nonce;
            Ok(ParsedPayload {
                hash: None,
                approval_role: None,
                agreement_hash: None,
            })
        }
        MessageType::Proof => {
            require_exact_keys(object, &["offer_hash", "responder_nonce"])?;
            let hash = string_field(object, "offer_hash")?;
            let nonce = string_field(object, "responder_nonce")?;
            validate_lower_hex(&hash, 64)?;
            validate_lower_hex(&nonce, 64)?;
            let _ = nonce;
            Ok(ParsedPayload {
                hash: Some(hash),
                approval_role: None,
                agreement_hash: None,
            })
        }
        MessageType::Approval => {
            require_exact_keys(object, &["agreement_hash", "role"])?;
            let hash = string_field(object, "agreement_hash")?;
            validate_lower_hex(&hash, 64)?;
            let role = match string_field(object, "role")?.as_str() {
                "initiator" => "initiator",
                "responder" => "responder",
                _ => return Err(BootstrapError("bootstrap approval role is invalid")),
            };
            Ok(ParsedPayload {
                hash: None,
                approval_role: Some(role),
                agreement_hash: Some(hash),
            })
        }
    }
}

fn parse_identity(
    device_id: &str,
    signing_key_id: &str,
    public_key: &str,
) -> Result<BootstrapIdentity, BootstrapError> {
    let device_id = DeviceId::new(device_id.to_owned())
        .map_err(|_| BootstrapError("bootstrap DeviceId is invalid"))?;
    let signing_key_id = SigningKeyId::parse(signing_key_id)
        .map_err(|_| BootstrapError("bootstrap signing key ID is invalid"))?;
    let public_key_bytes = decode_lower_hex(public_key, 64)?;
    let public_key: [u8; 32] = public_key_bytes
        .try_into()
        .map_err(|_| BootstrapError("bootstrap public key must be 32 bytes"))?;
    let key = VerifyingKey::from_bytes(&public_key)
        .map_err(|_| BootstrapError("bootstrap public key is invalid"))?;
    if signing_key_id_from_verifying_key(&key)
        .map_err(|_| BootstrapError("bootstrap key ID derivation failed"))?
        != signing_key_id
    {
        return Err(BootstrapError(
            "bootstrap signing key ID does not match public key",
        ));
    }
    let fingerprint_sha256: [u8; 32] = Sha256::digest(public_key).into();
    Ok(BootstrapIdentity {
        device_id,
        signing_key_id,
        public_key,
        fingerprint_sha256,
    })
}

fn require_same_sender(
    message: &ParsedMessage,
    expected: &BootstrapIdentity,
    transport_id: &str,
) -> Result<(), BootstrapError> {
    if &message.sender != expected || message.sender_transport_id != transport_id {
        return Err(BootstrapError(
            "bootstrap sender identity/transport binding differs",
        ));
    }
    Ok(())
}

fn require_same_peer(
    message: &ParsedMessage,
    expected: &BootstrapIdentity,
    transport_id: &str,
) -> Result<(), BootstrapError> {
    if message.peer.as_ref() != Some(expected)
        || message.peer_transport_id.as_deref() != Some(transport_id)
    {
        return Err(BootstrapError(
            "bootstrap peer identity/transport binding differs",
        ));
    }
    Ok(())
}

fn identity_value(identity: &BootstrapIdentity) -> Value {
    serde_json::json!({
        "device_id": identity.device_id.as_str(),
        "signing_key_id": identity.signing_key_id.as_str(),
        "public_key": encode_lower_hex(&identity.public_key),
    })
}

fn domain_hash(domain: &[u8], value: &Value) -> Result<String, BootstrapError> {
    let canonical = serde_json_canonicalizer::to_vec(value)
        .map_err(|_| BootstrapError("bootstrap JCS encoding failed"))?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(&[0]);
    hasher.update(&canonical);
    Ok(encode_lower_hex(hasher.finalize().as_bytes()))
}

fn validate_canonical_brain_id(value: &str) -> Result<(), BootstrapError> {
    if value.len() != 36
        || value.bytes().any(|byte| byte.is_ascii_uppercase())
        || parse_uuid_str(value).is_err()
    {
        return Err(BootstrapError(
            "bootstrap Brain ID must be canonical lowercase UUID",
        ));
    }
    Ok(())
}

fn validate_transport_id(value: &str) -> Result<(), BootstrapError> {
    if value.len() != 63
        || value.split('-').count() != 8
        || !value.split('-').all(|group| {
            group.len() == 7
                && group
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase() || (b'2'..=b'7').contains(&byte))
        })
    {
        return Err(BootstrapError(
            "bootstrap transport DeviceId format is invalid",
        ));
    }
    Ok(())
}

fn validate_lower_hex(value: &str, expected_len: usize) -> Result<(), BootstrapError> {
    if value.len() != expected_len
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(BootstrapError(
            "bootstrap hex field is not fixed-width lowercase hex",
        ));
    }
    Ok(())
}

fn decode_lower_hex(value: &str, expected_len: usize) -> Result<Vec<u8>, BootstrapError> {
    validate_lower_hex(value, expected_len)?;
    let mut bytes = Vec::with_capacity(expected_len / 2);
    let (pairs, remainder) = value.as_bytes().as_chunks::<2>();
    debug_assert!(remainder.is_empty(), "validated hex length is even");
    for pair in pairs {
        let high = hex_nibble(pair[0]).ok_or(BootstrapError("invalid lowercase hex"))?;
        let low = hex_nibble(pair[1]).ok_or(BootstrapError("invalid lowercase hex"))?;
        bytes.push((high << 4) | low);
    }
    Ok(bytes)
}

fn encode_lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn require_exact_keys(
    object: &Map<String, Value>,
    expected: &[&str],
) -> Result<(), BootstrapError> {
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err(BootstrapError(
            "bootstrap payload fields are invalid or unknown",
        ));
    }
    Ok(())
}

fn string_field(object: &Map<String, Value>, key: &str) -> Result<String, BootstrapError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or(BootstrapError("bootstrap payload field has the wrong type"))
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};

    use super::*;

    const BRAIN: &str = "22222222-2222-4222-8222-222222222222";
    const INITIATOR_TRANSPORT: &str =
        "AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA";
    const RESPONDER_TRANSPORT: &str =
        "BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB";

    fn key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    fn hex(bytes: &[u8]) -> String {
        encode_lower_hex(bytes)
    }

    #[allow(clippy::too_many_arguments)]
    fn common(
        kind: &str,
        tx: &str,
        sender_id: &str,
        sender_key: &SigningKey,
        sender_transport: &str,
        peer: Option<(&str, &SigningKey, &str)>,
        previous: &str,
        payload: Value,
    ) -> Value {
        let sender_public = sender_key.verifying_key().to_bytes();
        let sender_key_id = signing_key_id_from_verifying_key(&sender_key.verifying_key()).unwrap();
        let (peer_id, peer_key_id, peer_public, peer_transport) = match peer {
            Some((id, key, transport)) => {
                let public = key.verifying_key().to_bytes();
                let key_id = signing_key_id_from_verifying_key(&key.verifying_key()).unwrap();
                (
                    Value::String(id.into()),
                    Value::String(key_id.as_str().into()),
                    Value::String(hex(&public)),
                    Value::String(transport.into()),
                )
            }
            None => (Value::Null, Value::Null, Value::Null, Value::Null),
        };
        serde_json::json!({
            "protocol_version": 1,
            "message_type": kind,
            "transaction_id": tx,
            "brain_id": BRAIN,
            "sender_device_id": sender_id,
            "sender_signing_key_id": sender_key_id.as_str(),
            "sender_public_key": hex(&sender_public),
            "sender_transport_device_id": sender_transport,
            "peer_device_id": peer_id,
            "peer_signing_key_id": peer_key_id,
            "peer_public_key": peer_public,
            "peer_transport_device_id": peer_transport,
            "previous_hash": previous,
            "payload": payload,
            "signature": "00".repeat(64)
        })
    }

    fn sign(mut value: Value, key: &SigningKey) -> Vec<u8> {
        let object = value.as_object_mut().unwrap();
        object.remove("signature");
        let canonical = serde_json_canonicalizer::to_vec(&value).unwrap();
        let mut preimage = Vec::new();
        preimage.extend_from_slice(SIGNATURE_DOMAIN);
        preimage.push(0);
        preimage.extend_from_slice(&canonical);
        let signature = key.sign(&preimage);
        value.as_object_mut().unwrap().insert(
            "signature".into(),
            Value::String(hex(&signature.to_bytes())),
        );
        serde_json_canonicalizer::to_vec(&value).unwrap()
    }

    fn transcript() -> Vec<Vec<u8>> {
        let initiator_key = key(0x11);
        let responder_key = key(0x22);
        let tx = "ab".repeat(32);
        let offer = sign(
            common(
                "offer",
                &tx,
                "device-initiator",
                &initiator_key,
                INITIATOR_TRANSPORT,
                None,
                ZERO_HASH,
                serde_json::json!({"initiator_nonce":"11".repeat(32)}),
            ),
            &initiator_key,
        );
        let offer_value = parse_bootstrap_message(&offer).unwrap();
        let offer_hash = domain_hash(MESSAGE_HASH_DOMAIN, &offer_value).unwrap();
        let proof = sign(
            common(
                "proof",
                &tx,
                "device-responder",
                &responder_key,
                RESPONDER_TRANSPORT,
                Some(("device-initiator", &initiator_key, INITIATOR_TRANSPORT)),
                &offer_hash,
                serde_json::json!({"offer_hash":offer_hash,"responder_nonce":"22".repeat(32)}),
            ),
            &responder_key,
        );
        let proof_value = parse_bootstrap_message(&proof).unwrap();
        let proof_hash = domain_hash(MESSAGE_HASH_DOMAIN, &proof_value).unwrap();
        let initiator = parse_identity_from_key("device-initiator", &initiator_key);
        let responder = parse_identity_from_key("device-responder", &responder_key);
        let agreement = domain_hash(
            AGREEMENT_HASH_DOMAIN,
            &serde_json::json!({
                "offer_hash":offer_hash,
                "proof_hash":proof_hash,
                "brain_id":BRAIN,
                "initiator_identity":identity_value(&initiator),
                "responder_identity":identity_value(&responder),
                "transport_association":{
                    "initiator_transport_device_id":INITIATOR_TRANSPORT,
                    "responder_transport_device_id":RESPONDER_TRANSPORT
                }
            }),
        )
        .unwrap();
        let initiator_approval = sign(
            common(
                "approval",
                &tx,
                "device-initiator",
                &initiator_key,
                INITIATOR_TRANSPORT,
                Some(("device-responder", &responder_key, RESPONDER_TRANSPORT)),
                &proof_hash,
                serde_json::json!({"agreement_hash":agreement,"role":"initiator"}),
            ),
            &initiator_key,
        );
        let init_value = parse_bootstrap_message(&initiator_approval).unwrap();
        let init_hash = domain_hash(MESSAGE_HASH_DOMAIN, &init_value).unwrap();
        let responder_approval = sign(
            common(
                "approval",
                &tx,
                "device-responder",
                &responder_key,
                RESPONDER_TRANSPORT,
                Some(("device-initiator", &initiator_key, INITIATOR_TRANSPORT)),
                &init_hash,
                serde_json::json!({"agreement_hash":agreement,"role":"responder"}),
            ),
            &responder_key,
        );
        vec![offer, proof, initiator_approval, responder_approval]
    }

    #[test]
    fn bootstrap_offer_and_proof_builders_bind_signed_exchange() {
        let initiator_key = key(0x11);
        let responder_key = key(0x22);
        let initiator_id = DeviceId::new("device-initiator").unwrap();
        let responder_id = DeviceId::new("device-responder").unwrap();
        let offer = create_bootstrap_offer(
            BRAIN,
            &"ab".repeat(32),
            &initiator_id,
            INITIATOR_TRANSPORT,
            &[0x33; 32],
            &initiator_key,
        )
        .unwrap();
        let parsed_offer = parse_and_verify_message(&offer).unwrap();
        assert_eq!(parsed_offer.message_type, MessageType::Offer);
        assert_eq!(parsed_offer.sender.device_id, initiator_id);
        assert_eq!(parsed_offer.sender_transport_id, INITIATOR_TRANSPORT);
        assert_eq!(parsed_offer.wire.previous_hash, ZERO_HASH);

        let proof = create_bootstrap_proof(
            &offer,
            &responder_id,
            RESPONDER_TRANSPORT,
            &[0x44; 32],
            &responder_key,
        )
        .unwrap();
        let parsed_proof = parse_and_verify_message(&proof).unwrap();
        assert_eq!(parsed_proof.message_type, MessageType::Proof);
        assert_eq!(parsed_proof.sender.device_id, responder_id);
        assert_eq!(parsed_proof.sender_transport_id, RESPONDER_TRANSPORT);
        assert_eq!(parsed_proof.peer.as_ref(), Some(&parsed_offer.sender));
        assert_eq!(
            parsed_proof.peer_transport_id.as_deref(),
            Some(INITIATOR_TRANSPORT)
        );
        assert_eq!(
            parsed_proof.wire.transaction_id,
            parsed_offer.wire.transaction_id
        );
        assert_eq!(parsed_proof.wire.brain_id, parsed_offer.wire.brain_id);
        assert_eq!(parsed_proof.wire.previous_hash, parsed_offer.hash);

        let request = verify_bootstrap_approval_request(
            &[offer.as_slice(), proof.as_slice()],
            BootstrapApprovalRole::Initiator,
        )
        .unwrap();
        let initiator_approval = sign_bootstrap_approval(&request, &initiator_key).unwrap();
        let responder_request = verify_bootstrap_approval_request(
            &[
                offer.as_slice(),
                proof.as_slice(),
                initiator_approval.as_slice(),
            ],
            BootstrapApprovalRole::Responder,
        )
        .unwrap();
        let responder_approval =
            sign_bootstrap_approval(&responder_request, &responder_key).unwrap();
        verify_bootstrap_transcript(&[
            offer.as_slice(),
            proof.as_slice(),
            initiator_approval.as_slice(),
            responder_approval.as_slice(),
        ])
        .unwrap();
    }

    #[test]
    fn bootstrap_proof_builder_rejects_invalid_or_non_offer_input() {
        let responder_id = DeviceId::new("device-responder").unwrap();
        let key = key(0x22);
        let messages = transcript();
        assert!(
            create_bootstrap_proof(
                &messages[1],
                &responder_id,
                RESPONDER_TRANSPORT,
                &[0x44; 32],
                &key,
            )
            .is_err()
        );
        assert!(
            create_bootstrap_proof(
                b"{\"not\":\"an offer\"}",
                &responder_id,
                RESPONDER_TRANSPORT,
                &[0x44; 32],
                &key,
            )
            .is_err()
        );
    }

    fn parse_identity_from_key(id: &str, key: &SigningKey) -> BootstrapIdentity {
        parse_identity(
            id,
            signing_key_id_from_verifying_key(&key.verifying_key())
                .unwrap()
                .as_str(),
            &hex(&key.verifying_key().to_bytes()),
        )
        .unwrap()
    }

    #[test]
    fn verifies_complete_offer_proof_and_bilateral_approval_transcript() {
        let messages = transcript();
        let refs: Vec<_> = messages.iter().map(Vec::as_slice).collect();
        let verified = verify_bootstrap_transcript(&refs).unwrap();
        assert_eq!(verified.transaction_id, "ab".repeat(32));
        assert_eq!(verified.initiator.device_id.as_str(), "device-initiator");
        assert_eq!(verified.responder.device_id.as_str(), "device-responder");
        assert_eq!(verified.initiator_transport_device_id, INITIATOR_TRANSPORT);
        assert_eq!(verified.responder_transport_device_id, RESPONDER_TRANSPORT);
        let initiator_approval: Value =
            serde_json::from_slice(&messages[2]).expect("initiator approval JSON");
        let responder_approval: Value =
            serde_json::from_slice(&messages[3]).expect("responder approval JSON");
        assert_eq!(
            verified.initiator_approval_hash,
            domain_hash(MESSAGE_HASH_DOMAIN, &initiator_approval).unwrap()
        );
        assert_eq!(
            verified.responder_approval_hash,
            domain_hash(MESSAGE_HASH_DOMAIN, &responder_approval).unwrap()
        );
        assert_eq!(verified.transcript_hash.len(), 64);
        let values: Vec<_> = messages
            .iter()
            .map(|message| serde_json::from_slice(message).unwrap())
            .collect();
        let embedded = verify_bootstrap_transcript_values(&values).unwrap();
        assert_eq!(embedded, verified);
    }

    #[test]
    fn verified_bootstrap_transcript_signs_exact_local_security_anchor() {
        let messages = transcript();
        let refs: Vec<_> = messages.iter().map(Vec::as_slice).collect();
        let verified = verify_bootstrap_transcript(&refs).unwrap();
        let initiator_key = key(0x11);
        let signing_key_id =
            signing_key_id_from_verifying_key(&initiator_key.verifying_key()).unwrap();
        let bytes = crate::security_v3::sign_peer_approved_event(
            &verified,
            "66666666-6666-4666-8666-666666666666",
            crate::device_seq::DeviceSeq::from_u64(1).unwrap(),
            "2026-10-29T12:00:00Z",
            &signing_key_id,
            &crate::hex::ContentHashHex::zero(),
            &initiator_key,
        )
        .unwrap();
        let authenticated =
            crate::security_v3::read_security_v3_event(&bytes, &initiator_key.verifying_key())
                .unwrap();
        assert_eq!(authenticated.device_id().as_str(), "device-initiator");
        assert_eq!(authenticated.brain_id(), BRAIN);
        assert_eq!(
            authenticated.event_id(),
            "66666666-6666-4666-8666-666666666666"
        );
        assert!(matches!(
            authenticated.payload(),
            crate::security_v3::SecurityPayloadV3::PeerApproved { transaction_id, transcript }
                if transaction_id == verified.transaction_id()
                    && transcript == verified.transcript()
        ));
        assert!(
            crate::security_v3::sign_peer_approved_event(
                &verified,
                "66666666-6666-4666-8666-666666666666",
                crate::device_seq::DeviceSeq::from_u64(1).unwrap(),
                "2026-10-29T12:00:00Z",
                &signing_key_id,
                &crate::hex::ContentHashHex::zero(),
                &key(0x33),
            )
            .is_err()
        );
    }

    #[test]
    fn approval_signing_requires_verified_prefix_and_matching_local_key() {
        let messages = transcript();
        let initiator_request = verify_bootstrap_approval_request(
            &[messages[0].as_slice(), messages[1].as_slice()],
            BootstrapApprovalRole::Initiator,
        )
        .unwrap();
        assert_eq!(initiator_request.previous_message_hash(), {
            let value: Value = serde_json::from_slice(&messages[1]).unwrap();
            domain_hash(MESSAGE_HASH_DOMAIN, &value).unwrap()
        });
        let initiator_approval = sign_bootstrap_approval(&initiator_request, &key(0x11)).unwrap();
        assert_eq!(initiator_approval, messages[2]);

        let responder_request = verify_bootstrap_approval_request(
            &[
                messages[0].as_slice(),
                messages[1].as_slice(),
                initiator_approval.as_slice(),
            ],
            BootstrapApprovalRole::Responder,
        )
        .unwrap();
        let responder_approval = sign_bootstrap_approval(&responder_request, &key(0x22)).unwrap();
        assert_eq!(responder_approval, messages[3]);
        let complete = [
            messages[0].as_slice(),
            messages[1].as_slice(),
            initiator_approval.as_slice(),
            responder_approval.as_slice(),
        ];
        verify_bootstrap_transcript(&complete).unwrap();

        assert!(sign_bootstrap_approval(&initiator_request, &key(0x33)).is_err());
        assert!(
            verify_bootstrap_approval_request(
                &[messages[0].as_slice(), messages[1].as_slice()],
                BootstrapApprovalRole::Responder
            )
            .is_err()
        );
        let mut tampered = messages[1].clone();
        let mut value: Value = serde_json::from_slice(&tampered).unwrap();
        value["payload"]["offer_hash"] = Value::String("99".repeat(32));
        tampered = sign(value, &key(0x22));
        assert!(
            verify_bootstrap_approval_request(
                &[messages[0].as_slice(), tampered.as_slice()],
                BootstrapApprovalRole::Initiator
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_changed_prefix_role_transaction_and_signature() {
        let mut messages = transcript();
        let mut changed: Value = serde_json::from_slice(&messages[2]).unwrap();
        changed["previous_hash"] = Value::String("33".repeat(32));
        messages[2] = sign(changed, &key(0x11));
        let refs: Vec<_> = messages.iter().map(Vec::as_slice).collect();
        assert!(verify_bootstrap_transcript(&refs).is_err());

        let mut messages = transcript();
        let mut changed: Value = serde_json::from_slice(&messages[3]).unwrap();
        changed["payload"]["role"] = Value::String("initiator".into());
        messages[3] = sign(changed, &key(0x22));
        let refs: Vec<_> = messages.iter().map(Vec::as_slice).collect();
        assert!(verify_bootstrap_transcript(&refs).is_err());

        let mut messages = transcript();
        let mut changed: Value = serde_json::from_slice(&messages[3]).unwrap();
        changed["transaction_id"] = Value::String("cd".repeat(32));
        messages[3] = sign(changed, &key(0x22));
        let refs: Vec<_> = messages.iter().map(Vec::as_slice).collect();
        assert!(verify_bootstrap_transcript(&refs).is_err());

        let mut messages = transcript();
        let mut changed: Value = serde_json::from_slice(&messages[1]).unwrap();
        changed["signature"] = Value::String("00".repeat(64));
        messages[1] = serde_json_canonicalizer::to_vec(&changed).unwrap();
        let refs: Vec<_> = messages.iter().map(Vec::as_slice).collect();
        assert!(verify_bootstrap_transcript(&refs).is_err());
    }

    #[test]
    fn bounded_parser_rejects_duplicate_decoded_keys_depth_counts_and_numbers() {
        assert!(parse_bootstrap_message(br#"{"a":1,"\u0061":2}"#).is_err());
        assert!(parse_bootstrap_message(br#"{"a":{"b":{"c":{"d":{"e":null}}}}}"#).is_err());
        assert!(parse_bootstrap_message(br#"{"protocol_version":1.0}"#).is_err());
        assert!(parse_bootstrap_message(br#"{"protocol_version":1e0}"#).is_err());
        assert!(parse_bootstrap_message(br#"{"protocol_version":-0}"#).is_err());
        assert!(parse_bootstrap_message(br#"{"protocol_version":2}"#).is_err());
        assert!(parse_bootstrap_message(br#"{"\u0061":1,"a":2}"#).is_err());
        assert!(parse_bootstrap_message(br#"{"a":"\ud800"}"#).is_err());
        assert!(parse_bootstrap_message(r#"{"é":null}"#.as_bytes()).is_err());
        assert!(parse_bootstrap_message(&vec![b' '; MAX_MESSAGE_BYTES + 1]).is_err());
        assert!(parse_bounded_json(br#"{"a":{"b":{"c":{"d":null}}}}"#).is_ok());

        let too_many_members = format!(
            "{{{}}}",
            (0..25)
                .map(|index| format!("\"k{index}\":null"))
                .collect::<Vec<_>>()
                .join(",")
        );
        assert!(parse_bounded_json(too_many_members.as_bytes()).is_err());
        let too_many_items = format!("[{}]", vec!["null"; 17].join(","));
        assert!(parse_bounded_json(too_many_items.as_bytes()).is_err());
        let max_items = format!("[{}]", vec!["null"; 16].join(","));
        assert!(parse_bounded_json(max_items.as_bytes()).is_ok());
        let max_decoded_string = format!("{{\"a\":\"{}\"}}", "a".repeat(16 * 1024));
        assert!(parse_bounded_json(max_decoded_string.as_bytes()).is_ok());
        let too_long_decoded_string = format!("{{\"a\":\"{}\"}}", "a".repeat(16 * 1024 + 1));
        assert!(parse_bounded_json(too_long_decoded_string.as_bytes()).is_err());
    }

    #[test]
    fn rejects_bad_transport_identity_and_brain_spelling() {
        let messages = transcript();
        let mut offer: Value = serde_json::from_slice(&messages[0]).unwrap();
        offer["sender_transport_device_id"] = Value::String("a".repeat(63));
        let bytes = serde_json_canonicalizer::to_vec(&offer).unwrap();
        assert!(parse_and_verify_message(&bytes).is_err());
        offer["sender_transport_device_id"] = Value::String(INITIATOR_TRANSPORT.into());
        offer["brain_id"] =
            Value::String("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".to_ascii_uppercase());
        let bytes = serde_json_canonicalizer::to_vec(&offer).unwrap();
        assert!(parse_and_verify_message(&bytes).is_err());
    }
}
