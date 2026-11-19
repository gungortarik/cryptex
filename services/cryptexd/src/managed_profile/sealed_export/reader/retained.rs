//! Local evidence of an earlier challenge. Never a live reply or a writer permit.
use super::{Body, ContentHashHex, Error, Route, VerifiedCanonicalSnapshot, wire};
use crate::secure_store::{SecureStore, SecureStoreEnvironment, SecureStoreError};
use cryptex_events::SignatureHex;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const DOMAIN: &[u8] = b"cryptex/m9/local-reader-observation/v1\0";
// Fits a Windows generic credential, including both signatures.
const LIMIT: usize = 2560;

pub(super) fn context(
    snapshot: &VerifiedCanonicalSnapshot,
    lock: &std::fs::File,
    build: &ContentHashHex,
) -> Result<ContentHashHex, Error> {
    let profile = crate::import_staging::source_snapshot(lock).map_err(|_| Error::Storage)?;
    // Exact native IDs can exceed the integer precision of canonical JSON.
    // The header ends at the JSON array boundary; each following hash has
    // exactly 64 lowercase hex characters in verified device/sequence order.
    let (volume, high, low) = profile.identity;
    let header = serde_json_canonicalizer::to_vec(&(
        snapshot.brain_id(),
        build,
        [volume.to_string(), high.to_string(), low.to_string()],
    ))
    .map_err(|_| Error::Message)?;
    let mut digest = Sha256::new();
    digest.update(header);
    for hash in snapshot.security_event_hashes() {
        digest.update(hash.as_str().as_bytes());
    }
    ContentHashHex::from_bytes(&digest.finalize()).map_err(|_| Error::Message)
}

// Boundaries stay in the local signed observation, outside the LAN messages.
#[derive(Serialize, Deserialize)]
struct ChainTip(cryptex_events::DeviceSeq, ContentHashHex);

impl ChainTip {
    fn from_chain(
        snapshot: &VerifiedCanonicalSnapshot,
        device: &str,
    ) -> Result<Option<Self>, Error> {
        snapshot
            .reader_device_chain(device)
            .last()
            .map(|r| {
                Ok(Self(
                    cryptex_events::DeviceSeq::from_u64(r.device_seq())
                        .map_err(|_| Error::Authority)?,
                    r.content_hash().clone(),
                ))
            })
            .transpose()
    }

    fn present(&self, snapshot: &VerifiedCanonicalSnapshot, device: &str) -> bool {
        let chain = snapshot.reader_device_chain(device);
        let Ok(seq) = self.0.to_u64() else {
            return false;
        };
        chain
            .binary_search_by_key(&seq, |r| r.device_seq())
            .is_ok_and(|i| chain[i].content_hash() == &self.1)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Observation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    boundaries: Option<[Option<ChainTip>; 2]>,
    context: ContentHashHex,
    challenge: ContentHashHex,
    signed_reply: String,
}

impl Observation {
    pub(super) fn covers_history(
        &self,
        snapshot: &VerifiedCanonicalSnapshot,
        reverse: &Route,
    ) -> bool {
        self.boundaries.as_ref().is_none_or(|tips| {
            tips.iter()
                .zip([&reverse.receiver, &reverse.sender])
                .all(|(tip, device)| tip.as_ref().is_none_or(|tip| tip.present(snapshot, device)))
        })
    }

    fn preimage(&self) -> Result<Vec<u8>, Error> {
        let bytes = serde_json_canonicalizer::to_vec(self).map_err(|_| Error::Message)?;
        Ok([DOMAIN, bytes.as_slice()].concat())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    observation: Observation,
    signature: SignatureHex,
}

#[cfg(test)]
pub(super) mod tests;

pub(super) struct Slot<'a> {
    store: &'a dyn SecureStore,
    service: String,
    account: String,
}

impl<'a> Slot<'a> {
    pub(super) fn new(
        credentials: (&'a dyn SecureStore, SecureStoreEnvironment),
        route: &Route,
    ) -> Result<Self, Error> {
        Ok(Self {
            store: credentials.0,
            service: format!("{}-M9ReaderObservations", credentials.1.service_prefix()),
            account: route.slot()?,
        })
    }

    pub(super) fn save(
        &self,
        context: &ContentHashHex,
        challenge: &ContentHashHex,
        reply: &[u8],
        signer: &SigningKey,
        history: Option<(&VerifiedCanonicalSnapshot, &Route)>,
    ) -> Result<(), Error> {
        let boundaries = match history {
            Some((snapshot, reverse)) => Some([
                ChainTip::from_chain(snapshot, &reverse.receiver)?,
                ChainTip::from_chain(snapshot, &reverse.sender)?,
            ]),
            None => None,
        };
        let observation = Observation {
            boundaries,
            context: context.clone(),
            challenge: challenge.clone(),
            signed_reply: std::str::from_utf8(reply)
                .map_err(|_| Error::Message)?
                .into(),
        };
        let signature = SignatureHex::from_bytes(&signer.sign(&observation.preimage()?).to_bytes())
            .map_err(|_| Error::Message)?;
        let bytes = serde_json::to_vec(&Record {
            observation,
            signature,
        })
        .map_err(|_| Error::Message)?;
        if bytes.len() > LIMIT {
            return Err(Error::Message);
        }
        self.store
            .put(&self.service, &self.account, &bytes)
            .map_err(|_| Error::Storage)
    }

    pub(super) fn read_observation(
        &self,
        local_key: &[u8; 32],
    ) -> Result<Option<Observation>, Error> {
        let bytes = match self.store.get(&self.service, &self.account) {
            Ok(bytes) if bytes.len() <= LIMIT => bytes,
            Ok(_) => return Err(Error::Message),
            Err(SecureStoreError::Missing) => return Ok(None),
            Err(_) => return Err(Error::Storage),
        };
        let record: Record = serde_json::from_slice(&bytes).map_err(|_| Error::Message)?;
        let signature =
            Signature::from_bytes(&record.signature.to_bytes().map_err(|_| Error::Message)?);
        let obs = record.observation;
        let preimage = obs.preimage()?;
        VerifyingKey::from_bytes(local_key)
            .and_then(|key| key.verify_strict(&preimage, &signature))
            .map_err(|_| Error::Message)?;
        Ok(Some(obs))
    }

    #[cfg(test)]
    pub(super) fn load(
        &self,
        context: &ContentHashHex,
        route: &Route,
        local_key: &[u8; 32],
        peer_key: &[u8; 32],
        blob: &ContentHashHex,
        observed: Option<&Body>,
    ) -> Result<Option<Body>, Error> {
        let Some(obs) = self.read_observation(local_key)? else {
            return Ok(None);
        };
        self.match_reply(&obs, context, route, peer_key, blob, observed)
    }

    pub(super) fn match_reply(
        &self,
        obs: &Observation,
        context: &ContentHashHex,
        route: &Route,
        peer_key: &[u8; 32],
        blob: &ContentHashHex,
        observed: Option<&Body>,
    ) -> Result<Option<Body>, Error> {
        if &obs.context != context {
            return Ok(None);
        }
        let body = wire::authenticate(obs.signed_reply.as_bytes(), route, peer_key)?;
        if body.echo.as_ref() != Some(&obs.challenge) || &body.resolved_blob != blob {
            return Err(Error::Message);
        }
        if observed.is_some_and(|new| {
            new.daemon_sha256 != body.daemon_sha256 || new.resolved_blob != body.resolved_blob
        }) {
            self.store
                .delete(&self.service, &self.account)
                .map_err(|_| Error::Storage)?;
            return Ok(None);
        }
        Ok(Some(body))
    }
}
