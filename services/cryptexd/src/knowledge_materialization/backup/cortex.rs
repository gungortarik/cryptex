//! Carry the signed Cortex selection evidence inside the authenticated payload.
use super::{Error, VerifiedCanonicalSnapshot};
use crate::cortex_evidence::{self as evidence, BatchDecision, Choice, Receipt, Source};
use crate::import_staging::{export_fs::Directory, open_recovery_source, source_snapshot};
use cryptex_events::{AuthenticatedSecurityRecord, EventPayload, TrustedDaemonKeyResolver};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, io::Read, path::Path};

fn read(root: &Path, name: &str) -> Result<Vec<u8>, Error> {
    let (mut file, before) =
        open_recovery_source(&root.join(name)).map_err(|_| Error::CortexEvidence)?;
    let mut bytes = Vec::new();
    (&mut file)
        .take(evidence::MAX_RECEIPT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::CortexEvidence)?;
    if bytes.len() as u64 != before.length
        || before.length > evidence::MAX_RECEIPT
        || source_snapshot(&file).map_err(|_| Error::CortexEvidence)? != before
    {
        return Err(Error::CortexEvidence);
    }
    Ok(bytes)
}

fn verify<T: Serialize + DeserializeOwned>(bytes: &[u8], key: &VerifyingKey) -> Result<T, Error> {
    let receipt: Receipt<T> = serde_json::from_slice(bytes).map_err(|_| Error::CortexEvidence)?;
    let signed =
        serde_json_canonicalizer::to_vec(&receipt.source).map_err(|_| Error::CortexEvidence)?;
    let raw = receipt
        .signature
        .to_bytes()
        .map_err(|_| Error::CortexEvidence)?;
    key.verify_strict(&signed, &Signature::from_bytes(&raw))
        .map_err(|_| Error::CortexEvidence)?;
    // Disallow unsigned trailing data or a second encoding carried inside a backup.
    if serde_json_canonicalizer::to_vec(&receipt).map_err(|_| Error::CortexEvidence)? != bytes {
        return Err(Error::CortexEvidence);
    }
    Ok(receipt.source)
}

pub(in crate::knowledge_materialization) fn verified_members(
    root: &Path,
    brain: &str,
    snapshot: &VerifiedCanonicalSnapshot,
) -> Result<BTreeMap<String, Vec<u8>>, Error> {
    let directory = Directory::open(root).map_err(|_| Error::CortexEvidence)?;
    let mut members = BTreeMap::new();
    let present = |name| directory.exists(name).map_err(|_| Error::CortexEvidence);
    if present("cortex-pending-source.json")? {
        return Err(Error::CortexEvidence);
    }
    if !present(evidence::DIRECTORY)? && !present(evidence::DECISION_FILE)? {
        return Ok(members);
    }
    let receipts = directory
        .child(evidence::DIRECTORY, false)
        .map_err(|_| Error::CortexEvidence)?;
    let decision_bytes = read(root, evidence::DECISION_FILE)?;
    let (history, _) = snapshot
        .verified_trust_context()
        .ok_or(Error::Credentials)?;
    let mut keys = history.key_resolver();
    let notes: BTreeMap<_, _> = snapshot
        .security_history
        .records()
        .iter()
        .filter_map(|record| match record {
            AuthenticatedSecurityRecord::V1(event) => Some((event.event_id.as_str(), event)),
            _ => None,
        })
        .collect();
    let mut selected = BTreeMap::new();
    let mut signer = None;
    let mut total = decision_bytes.len();
    for entry in fs::read_dir(root.join(evidence::DIRECTORY)).map_err(|_| Error::CortexEvidence)? {
        let file = entry
            .map_err(|_| Error::CortexEvidence)?
            .file_name()
            .into_string()
            .map_err(|_| Error::CortexEvidence)?;
        let name = format!("{}/{file}", evidence::DIRECTORY);
        if !evidence::backup_member(&name) {
            return Err(Error::CortexEvidence);
        }
        let event = notes
            .get(file.strip_suffix(".json").ok_or(Error::CortexEvidence)?)
            .ok_or(Error::CortexEvidence)?;
        let key = keys
            .trusted_daemon_key(&event.device_id, event.signing_key_id.as_str())
            .ok_or(Error::CortexEvidence)?;
        let bytes = read(root, &name)?;
        total += bytes.len();
        if total > 64 * 1024 * 1024 {
            return Err(Error::CortexEvidence);
        }
        let source: Source = verify(&bytes, &key)?;
        let EventPayload::MemoryNote(note) = &event.payload;
        if source.schema != evidence::SCHEMA
            || source.brain_id != brain
            || source.device_id != event.device_id
            || source.event_id != event.event_id
            || source.content_hash != event.content_hash.as_str()
            || source.approval.classification != event.classification
            || source.approval.source_sha256
                != format!("{:x}", Sha256::digest(note.text.as_bytes()))
        {
            return Err(Error::CortexEvidence);
        }
        let identity = (&event.device_id, key);
        if signer.as_ref().is_some_and(|prior| prior != &identity)
            || selected
                .insert(source.approval.capture_id.clone(), source)
                .is_some()
        {
            return Err(Error::CortexEvidence);
        }
        signer = Some(identity);
        members.insert(name, bytes);
    }
    receipts.validate().map_err(|_| Error::CortexEvidence)?;
    directory.validate().map_err(|_| Error::CortexEvidence)?;
    let (device, key) = signer.ok_or(Error::CortexEvidence)?;
    let decision: BatchDecision = verify(&decision_bytes, &key)?;
    let sources: Vec<_> = selected.into_values().collect();
    let report = evidence::comparison_report(brain, &sources, &[], &[], &[]);
    if decision.schema != evidence::DECISION_SCHEMA
        || decision.choice != Choice::Accepted
        || decision.brain_id != brain
        || &decision.device_id != device
        || decision.review_sha256
            != evidence::review_digest(&report).map_err(|_| Error::CortexEvidence)?
    {
        return Err(Error::CortexEvidence);
    }
    members.insert(evidence::DECISION_FILE.into(), decision_bytes);
    Ok(members)
}
