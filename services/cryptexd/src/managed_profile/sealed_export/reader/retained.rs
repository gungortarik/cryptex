//! Local evidence of an earlier challenge. Never a live reply or a writer permit.
use super::{Body, ContentHashHex, Error, Route, VerifiedCanonicalSnapshot, wire};
use crate::secure_store::{SecureStore, SecureStoreEnvironment, SecureStoreError};
use cryptex_events::{SignatureHex, VersionedRecord};
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
    let mut security = Vec::new();
    for record in snapshot.verified_records() {
        if let VersionedRecord::VerifiedSecurityV3(event) = record {
            security.push(event.content_hash().as_str());
        }
    }
    security.sort_unstable();
    let bytes =
        serde_json_canonicalizer::to_vec(&(snapshot.brain_id(), profile.identity, build, security))
            .map_err(|_| Error::Message)?;
    ContentHashHex::from_bytes(&Sha256::digest(bytes)).map_err(|_| Error::Message)
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    context: ContentHashHex,
    challenge: ContentHashHex,
    signed_reply: String,
}

impl Observation {
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
#[test]
fn previous_observation_requires_both_keys_context_route_and_same_observed_build() {
    use crate::FakeSecureStore;
    let local = SigningKey::from_bytes(&[12; 32]);
    let remote = SigningKey::from_bytes(&[31; 32]);
    let route = Route {
        brain: "brain".into(),
        sender: "peer".into(),
        receiver: "local".into(),
        sender_transport: "peer transport".into(),
        receiver_transport: "local transport".into(),
    };
    let hash = |n| ContentHashHex::from_bytes(&[n; 32]).unwrap();
    let reply = Body {
        route: route.clone(),
        nonce: hash(1),
        echo: Some(hash(2)),
        daemon_sha256: hash(3),
        resolved_blob: hash(4),
    };
    let store = FakeSecureStore::new();
    let slot = Slot::new((&store, SecureStoreEnvironment::Dev), &route).unwrap();
    let signed = wire::sign(reply.clone(), &remote).unwrap();
    let local_key = local.verifying_key().to_bytes();
    let peer_key = remote.verifying_key().to_bytes();
    let read = |observed: Option<&Body>| {
        slot.load(&hash(5), &route, &local_key, &peer_key, &hash(4), observed)
    };
    assert_eq!(read(None), Ok(None));
    slot.save(&hash(5), &hash(2), &signed, &local).unwrap();
    assert_eq!(read(None), Ok(Some(hash(3))));
    assert_eq!(
        slot.load(&hash(6), &route, &local_key, &peer_key, &hash(4), None),
        Ok(None)
    );
    assert_eq!(
        slot.load(
            &hash(5),
            &route.reversed(),
            &local_key,
            &peer_key,
            &hash(4),
            None
        ),
        Err(Error::Message)
    );
    assert_eq!(
        slot.load(&hash(5), &route, &peer_key, &peer_key, &hash(4), None),
        Err(Error::Message)
    );
    assert_eq!(
        slot.load(&hash(5), &route, &local_key, &local_key, &hash(4), None),
        Err(Error::Message)
    );
    slot.save(&hash(5), &hash(7), &signed, &local).unwrap();
    assert_eq!(read(None), Err(Error::Message));
    slot.save(&hash(5), &hash(2), &signed, &local).unwrap();
    let changed = Body {
        daemon_sha256: hash(8),
        ..reply.clone()
    };
    assert_eq!(read(Some(&changed)), Ok(None));
    assert_eq!(read(None), Ok(None));
    slot.save(&hash(5), &hash(2), &signed, &local).unwrap();
    let changed = Body {
        resolved_blob: hash(9),
        ..reply
    };
    assert_eq!(read(Some(&changed)), Ok(None));
    assert_eq!(read(None), Ok(None));
}

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
    ) -> Result<(), Error> {
        let observation = Observation {
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

    pub(super) fn load(
        &self,
        context: &ContentHashHex,
        route: &Route,
        local_key: &[u8; 32],
        peer_key: &[u8; 32],
        blob: &ContentHashHex,
        observed: Option<&Body>,
    ) -> Result<Option<ContentHashHex>, Error> {
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
        Ok(Some(body.daemon_sha256))
    }
}
