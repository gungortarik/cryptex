//! Local signed source receipts for a single disposable batch import.
use super::{Approval, DEV, LifecycleError, fault};
use cryptex_core::DeviceId;
use cryptex_events::{EventPayload, SignatureHex};
use cryptex_storage::durability::sync_directory;
use cryptexd::{
    SecureStore, canonical_writer::load_verified_events, capture::CaptureCommit,
    event_signing_key::load_existing_event_signing_key,
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
const MAX_RECEIPT: u64 = 32 * 1024;

#[derive(Serialize, Deserialize)]
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
struct Receipt {
    source: Source,
    signature: SignatureHex,
}

fn encode(source: &impl Serialize) -> Result<Vec<u8>, LifecycleError> {
    serde_json_canonicalizer::to_vec(source).map_err(|_| fault("source_receipt_encoding"))
}

pub(super) struct Batch {
    key: SigningKey,
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

    pub(super) fn reserve(&mut self, approval: &Approval) {
        self.paths.insert(approval.source_path.clone());
        self.digests.insert(approval.source_sha256.clone());
        self.decisions
            .insert(approval.capture_id.clone(), approval.clone());
    }

    pub(super) fn record(
        &self,
        root: &Path,
        approval: &Approval,
        commit: &CaptureCommit,
    ) -> Result<(), LifecycleError> {
        let source = Source {
            schema: SCHEMA.into(),
            brain_id: commit.brain_id.clone(),
            device_id: commit.device_id.clone(),
            event_id: commit.event_id.clone(),
            content_hash: commit.content_hash.clone(),
            approval: approval.clone(),
        };
        let signature = SignatureHex::from_bytes(&self.key.sign(&encode(&source)?).to_bytes())
            .map_err(|_| fault("source_receipt_signature"))?;
        let bytes = encode(&Receipt { source, signature })?;
        let directory = root.join(DIRECTORY);
        let path = directory.join(format!("{}.json", commit.event_id));
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
        sync_directory(&directory).map_err(|_| fault("source_receipt_durability"))
    }
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
    let mut sources = Vec::with_capacity(events.len());
    for event in events {
        let bytes = read_bytes(&directory.join(format!("{}.json", event.event_id)))?;
        let receipt: Receipt =
            serde_json::from_slice(&bytes).map_err(|_| fault("source_receipt_invalid"))?;
        let raw = receipt
            .signature
            .to_bytes()
            .map_err(|_| fault("source_receipt_signature"))?;
        let source = receipt.source;
        key.verify_strict(&encode(&source)?, &Signature::from_bytes(&raw))
            .map_err(|_| fault("source_receipt_signature"))?;
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
        fs::read_dir(directory)?.try_fold(0usize, |count, entry| entry.map(|_| count + 1))?;
    if count != sources.len() {
        return Err(fault("source_receipt_inventory_mismatch"));
    }
    Ok(json!({"schema":"cryptex.cortex.provenance.v1", "sources":sources}))
}
