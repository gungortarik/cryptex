//! Owner-directed scheduling roles. A fresh target has a new immutable identity.
//! Each signer durably relinquishes its local authority before releasing a grant.
use crate::event_signing_key::load_existing_event_signing_key;
use crate::import_staging::export_fs::Directory;
use crate::knowledge_materialization::scan_verified_canonical_from_local_anchor_until;
use crate::secure_store::{
    AccountLabel, SecureStore, SecureStoreEnvironment, SecureStoreError, ServiceLabel,
};
use cryptex_core::DeviceId;
use cryptex_events::LocalTrustAnchor;
use ed25519_dalek::{Signature, Signer, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::Path;

const MARKER: &str = "backup-target.id";
const DOMAIN: &[u8] = b"cryptex/backup/writer-handoff/v1\0";
// The daemon owns one profile instance. Its workers can see the same target
// through different locally synchronized directories with different file locks.
static ROLE_CHANGES: std::sync::Mutex<()> = std::sync::Mutex::new(());
type Hash = [u8; 32];

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Target,
    Authority,
    Store,
    Signature,
    History,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Writer {
    device: DeviceId,
    key: Hash,
}

impl From<&LocalTrustAnchor> for Writer {
    fn from(anchor: &LocalTrustAnchor) -> Self {
        Self {
            device: anchor.device_id().clone(),
            key: *anchor.public_key(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Origin {
    version: u8,
    brain: String,
    nonce: Hash,
    writer: Writer,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    prior: Hash,
    next: Writer,
    challenge: Hash,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Grant {
    request: Request,
    signature: Vec<u8>,
}

impl Grant {
    fn head(&self) -> Result<Hash, Error> {
        Ok(Sha256::new()
            .chain_update(message(&self.request)?)
            .chain_update(&self.signature)
            .finalize()
            .into())
    }
}

/// Public, portable proof rooted in the independently selected target ID.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    origin: Origin,
    grants: Vec<Grant>,
}

fn encoded(value: &impl Serialize) -> Result<Vec<u8>, Error> {
    serde_json_canonicalizer::to_vec(value).map_err(|_| Error::History)
}

fn message(request: &Request) -> Result<Vec<u8>, Error> {
    Ok([DOMAIN, encoded(request)?.as_slice()].concat())
}

impl Assignment {
    fn id(&self) -> Result<Hash, Error> {
        if self.origin.version != 1 {
            return Err(Error::History);
        }
        Ok(Sha256::digest(encoded(&self.origin)?).into())
    }

    fn verify(&self, id: Hash) -> Result<(Hash, &Writer), Error> {
        if self.id()? != id {
            return Err(Error::Target);
        }
        let mut current = &self.origin.writer;
        let mut previous = id;
        for grant in &self.grants {
            if current.device == grant.request.next.device || current.key == grant.request.next.key
            {
                return Err(Error::History);
            }
            if grant.request.prior != previous {
                return Err(Error::History);
            }
            let bytes = message(&grant.request)?;
            let signature =
                Signature::from_slice(&grant.signature).map_err(|_| Error::Signature)?;
            VerifyingKey::from_bytes(&current.key)
                .map_err(|_| Error::Signature)?
                .verify_strict(&bytes, &signature)
                .map_err(|_| Error::Signature)?;
            previous = grant.head()?;
            current = &grant.request.next;
        }
        Ok((previous, current))
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
enum LocalRole {
    Active(Hash),
    Released(Request),
    Awaiting(Request),
}

pub struct DeviceRole<'a> {
    pub store: &'a dyn SecureStore,
    pub env: SecureStoreEnvironment,
    pub local: &'a LocalTrustAnchor,
}

fn marker(dir: &Directory) -> Result<Hash, Error> {
    let file = dir.open_file(MARKER, false).map_err(|_| Error::Target)?;
    let mut bytes = Vec::new();
    file.take(33)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Target)?;
    bytes.try_into().map_err(|_| Error::Target)
}

impl DeviceRole<'_> {
    fn changes(&self) -> Result<std::sync::MutexGuard<'static, ()>, Error> {
        ROLE_CHANGES.lock().map_err(|_| Error::Store)
    }

    #[cfg(test)] // The scheduler and retention runner currently operate only on disposable profiles.
    pub(crate) fn guard_writer(
        &self,
        target: &Path,
        directory: &Directory,
    ) -> Result<(std::sync::MutexGuard<'static, ()>, Hash), Error> {
        let guard = self.changes()?;
        let head = self.active(directory)?;
        if self.assigned_head(target)? != head {
            return Err(Error::Authority);
        }
        Ok((guard, head))
    }
    fn labels(&self, target: Hash) -> Result<(ServiceLabel, AccountLabel), Error> {
        let key = Sha256::digest(encoded(&(target, Writer::from(self.local)))?);
        Ok((
            ServiceLabel::device_credentials(self.env),
            AccountLabel::backup_role(&format!("{key:x}")),
        ))
    }

    fn load(&self, target: Hash) -> Result<Option<LocalRole>, Error> {
        let (service, account) = self.labels(target)?;
        match self.store.get(service.as_str(), account.as_str()) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|_| Error::Store),
            Err(SecureStoreError::Missing) => Ok(None),
            Err(_) => Err(Error::Store),
        }
    }

    fn save(&self, target: Hash, state: &LocalRole) -> Result<(), Error> {
        let (service, account) = self.labels(target)?;
        self.store
            .put(service.as_str(), account.as_str(), &encoded(state)?)
            .map_err(|_| Error::Store)
    }

    // Store one public proof per credential item. A long history would exceed
    // a Windows credential's size limit if stored as a single JSON value.
    fn proof_account(&self, target: Hash, head: Hash) -> Result<AccountLabel, Error> {
        let key = Sha256::digest(encoded(&("proof", target, head, Writer::from(self.local)))?);
        Ok(AccountLabel::backup_role(&format!("{key:x}")))
    }

    fn remember(&self, id: Hash, head: Hash, record: &impl Serialize) -> Result<(), Error> {
        let service = ServiceLabel::device_credentials(self.env);
        let account = self.proof_account(id, head)?;
        let bytes = encoded(record)?;
        match self.store.get(service.as_str(), account.as_str()) {
            Ok(stored) if stored == bytes => Ok(()),
            Ok(_) => Err(Error::History),
            Err(SecureStoreError::Missing) => self
                .store
                .put(service.as_str(), account.as_str(), &bytes)
                .map_err(|_| Error::Store),
            Err(_) => Err(Error::Store),
        }
    }

    fn remember_assignment(&self, id: Hash, assignment: &Assignment) -> Result<(), Error> {
        self.remember(id, id, &assignment.origin)?;
        for grant in &assignment.grants {
            self.remember(id, grant.head()?, grant)?;
        }
        Ok(())
    }

    /// Reopen the exact proof behind the local role, including the predecessor
    /// necessary to reissue an interrupted handoff after durable relinquishment.
    pub fn reopen(&self, target: &Path) -> Result<Assignment, Error> {
        let dir = Directory::open(target).map_err(|_| Error::Target)?;
        let id = marker(&dir)?;
        let _change = self.changes()?;
        let expected = match self.load(id)?.ok_or(Error::Authority)? {
            LocalRole::Active(head) => head,
            LocalRole::Awaiting(request) | LocalRole::Released(request) => request.prior,
        };
        let service = ServiceLabel::device_credentials(self.env);
        let read = |head| {
            self.store
                .get(service.as_str(), self.proof_account(id, head)?.as_str())
                .map_err(|_| Error::Store)
        };
        let mut cursor = expected;
        let mut grants = Vec::new();
        while cursor != id {
            let grant: Grant =
                serde_json::from_slice(&read(cursor)?).map_err(|_| Error::History)?;
            if grant.head()? != cursor {
                return Err(Error::History);
            }
            cursor = grant.request.prior;
            grants.push(grant);
        }
        grants.reverse();
        let origin = serde_json::from_slice(&read(id)?).map_err(|_| Error::History)?;
        let assignment = Assignment { origin, grants };
        if assignment.verify(id)?.0 != expected {
            return Err(Error::History);
        }
        Ok(assignment)
    }

    fn path_account(&self, path: &Path) -> Result<AccountLabel, Error> {
        let digest = Sha256::digest(encoded(&("path", path, Writer::from(self.local)))?);
        Ok(AccountLabel::backup_role(&format!("{digest:x}")))
    }

    fn bind(&self, path: &Path, id: Hash) -> Result<(), Error> {
        let service = ServiceLabel::device_credentials(self.env);
        self.store
            .put(service.as_str(), self.path_account(path)?.as_str(), &id)
            .map_err(|_| Error::Store)
    }

    pub fn assigned_head(&self, path: &Path) -> Result<Hash, Error> {
        let service = ServiceLabel::device_credentials(self.env);
        let bytes = self
            .store
            .get(service.as_str(), self.path_account(path)?.as_str())
            .map_err(|_| Error::Store)?;
        match self.load(bytes.try_into().map_err(|_| Error::Store)?)? {
            Some(LocalRole::Active(head)) => Ok(head),
            _ => Err(Error::Authority),
        }
    }

    /// The native owner selects a new child directory. Existing directories,
    /// including disconnected old targets remounted at the same path, cannot be adopted.
    pub fn create(&self, profile: &Path, parent: &Path, name: &str) -> Result<Assignment, Error> {
        let brain = crate::identity_runtime::load_brain_identity(profile)
            .map_err(|_| Error::History)?
            .brain_id;
        let _snapshot = self.check_authority(profile, &brain, None)?;
        let mut nonce = [0; 32];
        getrandom::getrandom(&mut nonce).map_err(|_| Error::Store)?;
        let initial = Assignment {
            origin: Origin {
                version: 1,
                brain,
                nonce,
                writer: self.local.into(),
            },
            grants: Vec::new(),
        };
        let id = initial.id()?;
        let path = parent.join(name);
        let parent = Directory::open(parent).map_err(|_| Error::Target)?;
        let dir = parent
            .create_child_exclusive(name)
            .map_err(|_| Error::Target)?;
        let mut file = dir.open_file(MARKER, true).map_err(|_| Error::Target)?;
        file.write_all(&id)
            .and_then(|()| file.sync_all())
            .map_err(|_| Error::Target)?;
        dir.sync()
            .and_then(|()| parent.sync())
            .map_err(|_| Error::Target)?;
        self.remember(id, id, &initial.origin)?;
        self.save(id, &LocalRole::Active(id))?;
        self.bind(&path, id)?;
        Ok(initial)
    }

    /// Called while the scheduler holds this target's writer lock, including
    /// after a wait for reconnection and immediately before removing generations.
    #[cfg(test)]
    fn active(&self, dir: &Directory) -> Result<Hash, Error> {
        let state = self.load(marker(dir)?)?.ok_or(Error::Authority)?;
        match state {
            LocalRole::Active(head) => Ok(head),
            _ => Err(Error::Authority),
        }
    }

    fn check_authority(
        &self,
        profile: &Path,
        brain: &str,
        peer: Option<&Writer>,
    ) -> Result<crate::knowledge_materialization::VerifiedCanonicalSnapshot, Error> {
        let enrolled = crate::local_device_identity::load_local_device_identity(profile, brain)
            .map_err(|_| Error::Authority)?;
        if enrolled.device_id != self.local.device_id().as_str() {
            return Err(Error::Authority);
        }
        let snapshot =
            scan_verified_canonical_from_local_anchor_until(profile, brain, self.local, None)
                .map_err(|_| Error::History)?;
        let (history, trust) = snapshot.verified_trust_context().ok_or(Error::Authority)?;
        if history.local_device != *self.local.device_id()
            || history.local_public_key != *self.local.public_key()
            || !trust.trusted_devices.contains(self.local.device_id())
            || history.restricted(trust, self.local.device_id())
            || peer.is_some_and(|peer| {
                !trust.trusted_devices.contains(&peer.device)
                    || history.restricted(trust, &peer.device)
                    || !history
                        .approved_peer_binding(&peer.device)
                        .is_some_and(|bound| bound.public_key == peer.key)
            })
        {
            return Err(Error::Authority);
        }
        Ok(snapshot)
    }

    pub fn request(
        &self,
        profile: &Path,
        target: &Path,
        assignment: &Assignment,
    ) -> Result<Request, Error> {
        let dir = Directory::open(target).map_err(|_| Error::Target)?;
        let _lock = dir
            .try_lock_exclusive("backup-writer.lock")
            .map_err(|_| Error::Target)?;
        let id = marker(&dir)?;
        let (prior, current) = assignment.verify(id)?;
        let _change = self.changes()?;
        if current == &Writer::from(self.local) {
            return Err(Error::Authority);
        }
        let _snapshot = self.check_authority(profile, &assignment.origin.brain, Some(current))?;
        match self.load(id)? {
            None => {}
            Some(LocalRole::Awaiting(request)) if request.prior == prior => return Ok(request),
            Some(LocalRole::Released(old))
                if assignment.grants.iter().any(|g| g.request == old) => {}
            _ => return Err(Error::Authority),
        }
        let mut challenge = [0; 32];
        getrandom::getrandom(&mut challenge).map_err(|_| Error::Store)?;
        let request = Request {
            prior,
            next: self.local.into(),
            challenge,
        };
        self.remember_assignment(id, assignment)?;
        self.save(id, &LocalRole::Awaiting(request.clone()))?;
        Ok(request)
    }

    pub fn release(
        &self,
        profile: &Path,
        target: &Path,
        assignment: &Assignment,
        next: &LocalTrustAnchor,
        request: &Request,
    ) -> Result<Assignment, Error> {
        let dir = Directory::open(target).map_err(|_| Error::Target)?;
        let _lock = dir
            .try_lock_exclusive("backup-writer.lock")
            .map_err(|_| Error::Target)?;
        let id = marker(&dir)?;
        let (head, current) = assignment.verify(id)?;
        let _change = self.changes()?;
        let next = Writer::from(next);
        if *current != Writer::from(self.local)
            || current.device == next.device
            || current.key == next.key
            || request.next != next
            || request.prior != head
        {
            return Err(Error::Authority);
        }
        let _snapshot = self.check_authority(profile, &assignment.origin.brain, Some(&next))?;
        match self.load(id)? {
            Some(LocalRole::Active(active)) if active == head => {}
            Some(LocalRole::Released(old)) if old == *request => {}
            _ => return Err(Error::Authority),
        }
        let key =
            load_existing_event_signing_key(self.store, self.env, profile, self.local.device_id())
                .map_err(|_| Error::Authority)?;
        if key.signing_key.verifying_key().as_bytes() != &current.key {
            return Err(Error::Authority);
        }
        let bytes = message(request)?;
        self.save(id, &LocalRole::Released(request.clone()))?;
        let signature = key.signing_key.sign(&bytes).to_bytes().to_vec();
        let mut result = assignment.clone();
        result.grants.push(Grant {
            request: request.clone(),
            signature,
        });
        Ok(result)
    }

    pub fn accept(
        &self,
        profile: &Path,
        target: &Path,
        assignment: &Assignment,
    ) -> Result<(), Error> {
        let dir = Directory::open(target).map_err(|_| Error::Target)?;
        let _lock = dir
            .try_lock_exclusive("backup-writer.lock")
            .map_err(|_| Error::Target)?;
        let id = marker(&dir)?;
        let (head, current) = assignment.verify(id)?;
        let _change = self.changes()?;
        let grant = assignment.grants.last().ok_or(Error::Authority)?;
        if *current != Writer::from(self.local) {
            return Err(Error::Authority);
        }
        let previous = assignment
            .grants
            .iter()
            .rev()
            .nth(1)
            .map_or(&assignment.origin.writer, |g| &g.request.next);
        let _snapshot = self.check_authority(profile, &assignment.origin.brain, Some(previous))?;
        match self.load(id)? {
            Some(LocalRole::Active(active)) if active == head => self.bind(target, id),
            Some(LocalRole::Awaiting(request)) if request == grant.request => {
                self.remember(id, head, grant)?;
                self.save(id, &LocalRole::Active(head))?;
                self.bind(target, id)
            }
            _ => Err(Error::Authority),
        }
    }
}
