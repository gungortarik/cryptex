//! Local signed source receipts for a single disposable batch import.
use super::{Approval, DEV, LifecycleError, fault};
use cryptex_core::DeviceId;
use cryptex_events::{EventPayload, SignatureHex};
use cryptex_storage::durability::sync_directory;
use cryptexd::{
    SecureStore,
    canonical_writer::load_verified_events,
    capture::{CaptureCommit, CaptureOutcome, resolve_capture},
    event_signing_key::load_existing_event_signing_key,
    identity_runtime::load_brain_identity,
};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{Read, Write},
    path::Path,
};

const DIRECTORY: &str = "cortex-source-receipts";
const SCHEMA: &str = "cryptex.m15.disposable-source.v1";
const PENDING: &str = "cortex-pending-source.json";
const INTENT_SCHEMA: &str = "cryptex.m15.disposable-source-intent.v1";
const MAX_RECEIPT: u64 = 32 * 1024;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    schema: String,
    brain_id: String,
    device_id: String,
    event_id: String,
    content_hash: String,
    approval: Approval,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt<T> {
    source: T,
    signature: SignatureHex,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Intent {
    schema: String,
    brain_id: String,
    device_id: String,
    approval: Approval,
}

fn encode(source: &impl Serialize) -> Result<Vec<u8>, LifecycleError> {
    serde_json_canonicalizer::to_vec(source).map_err(|_| fault("source_receipt_encoding"))
}

pub(super) struct Batch {
    key: SigningKey,
    brain_id: String,
    device_id: String,
    pending_capture: Option<String>,
    decisions: HashMap<String, Approval>,
    paths: HashSet<String>,
    digests: HashSet<String>,
}

impl Batch {
    pub(super) fn create(
        root: &Path,
        device: &DeviceId,
        store: &impl SecureStore,
    ) -> Result<Self, LifecycleError> {
        let key = load_existing_event_signing_key(store, DEV, root, device)
            .map_err(|_| fault("trial_key_unavailable"))?
            .signing_key;
        fs::create_dir(root.join(DIRECTORY))?;
        sync_directory(root).map_err(|_| fault("source_receipt_durability"))?;
        Ok(Self {
            key,
            brain_id: load_brain_identity(root)
                .map_err(|_| fault("trial_brain_missing"))?
                .brain_id,
            device_id: device.as_str().to_owned(),
            pending_capture: None,
            decisions: HashMap::new(),
            paths: HashSet::new(),
            digests: HashSet::new(),
        })
    }

    pub(super) fn check(&self, approval: &Approval) -> Option<bool> {
        if let Some(stored) = self.decisions.get(&approval.capture_id) {
            return Some(stored == approval);
        }
        if self.paths.contains(&approval.source_path)
            || self.digests.contains(&approval.source_sha256)
        {
            return Some(false);
        }
        None
    }

    pub(super) fn reserve(
        &mut self,
        root: &Path,
        approval: &Approval,
    ) -> Result<(), LifecycleError> {
        let intent = Intent {
            schema: INTENT_SCHEMA.into(),
            brain_id: self.brain_id.clone(),
            device_id: self.device_id.clone(),
            approval: approval.clone(),
        };
        publish(root, root, PENDING, &intent, &self.key)?;
        self.pending_capture = Some(approval.capture_id.clone());
        self.paths.insert(approval.source_path.clone());
        self.digests.insert(approval.source_sha256.clone());
        self.decisions
            .insert(approval.capture_id.clone(), approval.clone());
        Ok(())
    }

    pub(super) fn record(
        &self,
        root: &Path,
        approval: &Approval,
        commit: &CaptureCommit,
    ) -> Result<(), LifecycleError> {
        let source = Source::from_commit(approval, commit);
        publish(
            root,
            &root.join(DIRECTORY),
            &format!("{}.json", commit.event_id),
            &source,
            &self.key,
        )
    }

    pub(super) fn finish(&mut self, root: &Path, capture_id: &str) -> Result<(), LifecycleError> {
        if self.pending_capture.as_deref() == Some(capture_id) {
            clear_pending(root)?;
            self.pending_capture = None;
        }
        Ok(())
    }
}

impl Source {
    fn from_commit(approval: &Approval, commit: &CaptureCommit) -> Self {
        Self {
            schema: SCHEMA.into(),
            brain_id: commit.brain_id.clone(),
            device_id: commit.device_id.clone(),
            event_id: commit.event_id.clone(),
            content_hash: commit.content_hash.clone(),
            approval: approval.clone(),
        }
    }
}

fn publish(
    root: &Path,
    directory: &Path,
    name: &str,
    source: &impl Serialize,
    key: &SigningKey,
) -> Result<(), LifecycleError> {
    let signature = SignatureHex::from_bytes(&key.sign(&encode(source)?).to_bytes())
        .map_err(|_| fault("source_receipt_signature"))?;
    let bytes = encode(&Receipt { source, signature })?;
    let path = directory.join(name);
    let mut pending = tempfile::NamedTempFile::new_in(root)?;
    pending.write_all(&bytes)?;
    pending.as_file().sync_all()?;
    match pending.persist_noclobber(&path) {
        Ok(_) => (),
        Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {
            if read_bytes(&path)? != bytes {
                return Err(fault("source_receipt_conflict"));
            }
        }
        Err(e) => return Err(e.error.into()),
    }
    sync_directory(directory).map_err(|_| fault("source_receipt_durability"))
}

fn clear_pending(root: &Path) -> Result<(), LifecycleError> {
    fs::remove_file(root.join(PENDING))?;
    sync_directory(root).map_err(|_| fault("source_receipt_durability"))
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, LifecycleError> {
    if !fs::symlink_metadata(path).is_ok_and(|m| m.is_file() && m.len() <= MAX_RECEIPT) {
        return Err(fault("source_receipt_missing_or_invalid"));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(MAX_RECEIPT + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_RECEIPT {
        return Err(fault("source_receipt_oversize"));
    }
    Ok(bytes)
}

fn read_signed<T: serde::de::DeserializeOwned + Serialize>(
    path: &Path,
    key: &VerifyingKey,
) -> Result<T, LifecycleError> {
    let receipt: Receipt<T> =
        serde_json::from_slice(&read_bytes(path)?).map_err(|_| fault("source_receipt_invalid"))?;
    let raw = receipt
        .signature
        .to_bytes()
        .map_err(|_| fault("source_receipt_signature"))?;
    key.verify_strict(&encode(&receipt.source)?, &Signature::from_bytes(&raw))
        .map_err(|_| fault("source_receipt_signature"))?;
    Ok(receipt.source)
}

fn exists(path: &Path) -> Result<bool, LifecycleError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

fn pending_source(
    root: &Path,
    device: &DeviceId,
    store: &impl SecureStore,
    key: &VerifyingKey,
) -> Result<Option<Source>, LifecycleError> {
    let path = root.join(PENDING);
    if !exists(&path)? {
        return Ok(None);
    }
    let intent: Intent = read_signed(&path, key)?;
    if intent.schema != INTENT_SCHEMA || intent.device_id != device.as_str() {
        return Err(fault("source_intent_mismatch"));
    }
    let CaptureOutcome::Committed(commit) =
        resolve_capture(store, DEV, root, &intent.approval.capture_id)
    else {
        return Err(fault("source_commit_unverified"));
    };
    if commit.brain_id != intent.brain_id
        || commit.device_id != intent.device_id
        || commit.capture_id != intent.approval.capture_id
    {
        return Err(fault("source_intent_mismatch"));
    }
    Ok(Some(Source::from_commit(&intent.approval, &commit)))
}

pub(super) fn report(
    root: &Path,
    device: &DeviceId,
    store: &impl SecureStore,
    key: &VerifyingKey,
) -> Result<Value, LifecycleError> {
    let directory = root.join(DIRECTORY);
    if !fs::symlink_metadata(&directory).is_ok_and(|m| m.is_dir()) {
        return Err(fault("source_receipts_unavailable"));
    }
    let events = load_verified_events(store, DEV, root, device)
        .map_err(|_| fault("signed_readback_failed"))?;
    let pending = pending_source(root, device, store, key)?;
    let mut missing = false;
    let mut sources = Vec::with_capacity(events.len());
    for event in events {
        let path = directory.join(format!("{}.json", event.event_id));
        let source: Source = match &pending {
            Some(expected) if expected.event_id == event.event_id && !exists(&path)? => {
                missing = true;
                expected.clone()
            }
            _ => read_signed(&path, key)?,
        };
        let EventPayload::MemoryNote(note) = event.payload;
        if source.schema != SCHEMA
            || source.brain_id != event.brain_id
            || source.device_id != event.device_id
            || source.event_id != event.event_id
            || source.content_hash != event.content_hash.as_str()
            || source.approval.classification != event.classification
            || source.approval.source_sha256
                != format!("{:x}", Sha256::digest(note.text.as_bytes()))
        {
            return Err(fault("source_receipt_mismatch"));
        }
        sources.push(source);
    }
    let count =
        fs::read_dir(&directory)?.try_fold(0usize, |count, entry| entry.map(|_| count + 1))?;
    if count + usize::from(missing) != sources.len() {
        return Err(fault("source_receipt_inventory_mismatch"));
    }
    if let Some(expected) = pending {
        if !sources.contains(&expected) {
            return Err(fault("source_intent_mismatch"));
        }
        if missing {
            let signing = load_existing_event_signing_key(store, DEV, root, device)
                .map_err(|_| fault("trial_key_unavailable"))?;
            publish(
                root,
                &directory,
                &format!("{}.json", expected.event_id),
                &expected,
                &signing.signing_key,
            )?;
        } else {
            // A prior process may have exited just after installing the receipt.
            sync_directory(&directory).map_err(|_| fault("source_receipt_durability"))?;
        }
        clear_pending(root)?;
    }
    Ok(json!({"schema":"cryptex.cortex.provenance.v1", "sources":sources}))
}
