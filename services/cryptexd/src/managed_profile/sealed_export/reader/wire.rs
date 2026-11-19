use cryptex_events::{ContentHashHex, SignatureHex};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};

use super::Error;

// A signature for this fixed probe cannot authorize an event, pairing or owner action.
const DOMAIN: &[u8] = b"cryptex/m9/attachment.link/reader-proof/v1\0";
pub(super) const LIMIT: u64 = 4096;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Route {
    pub brain: String,
    pub sender: String,
    pub receiver: String,
    pub sender_transport: String,
    pub receiver_transport: String,
}

impl Route {
    pub fn reversed(&self) -> Self {
        Self {
            brain: self.brain.clone(),
            sender: self.receiver.clone(),
            receiver: self.sender.clone(),
            sender_transport: self.receiver_transport.clone(),
            receiver_transport: self.sender_transport.clone(),
        }
    }
    pub fn slot(&self) -> Result<String, Error> {
        use sha2::{Digest, Sha256};
        Ok(format!("{:x}.control", Sha256::digest(encode(self)?)))
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Body {
    pub route: Route,
    pub nonce: ContentHashHex,
    pub echo: Option<ContentHashHex>,
    pub daemon_sha256: ContentHashHex,
    pub resolved_blob: ContentHashHex,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Message {
    body: Body,
    signature: SignatureHex,
}

fn encode(value: &impl Serialize) -> Result<Vec<u8>, Error> {
    serde_json_canonicalizer::to_vec(value).map_err(|_| Error::Message)
}

fn preimage(body: &Body) -> Result<Vec<u8>, Error> {
    Ok([DOMAIN, encode(body)?.as_slice()].concat())
}

pub(super) fn sign(body: Body, key: &SigningKey) -> Result<Vec<u8>, Error> {
    let signature = SignatureHex::from_bytes(&key.sign(&preimage(&body)?).to_bytes())
        .map_err(|_| Error::Message)?;
    encode(&Message { body, signature })
}

pub(super) fn authenticate(bytes: &[u8], route: &Route, key: &[u8; 32]) -> Result<Body, Error> {
    if bytes.len() as u64 > LIMIT {
        return Err(Error::Message);
    }
    let msg: Message = serde_json::from_slice(bytes).map_err(|_| Error::Message)?;
    if &msg.body.route != route || encode(&msg)? != bytes {
        return Err(Error::Message);
    }
    let signature = Signature::from_bytes(&msg.signature.to_bytes().map_err(|_| Error::Message)?);
    let signed = preimage(&msg.body)?;
    VerifyingKey::from_bytes(key)
        .and_then(|key| key.verify_strict(&signed, &signature))
        .map_err(|_| Error::Message)?;
    Ok(msg.body)
}
