//! Local confirmation evidence. Its signature covers a fixed, content-free schema.
use super::ReviewError;
#[cfg(test)]
use super::{Outcome, SearchContext};
use crate::import_staging::export_fs::{self, Directory};
#[cfg(test)]
use crate::local_device_identity::LocalDeviceKeyResolver;
#[cfg(test)]
use cryptex_events::TrustedDaemonKeyResolver;
use cryptex_events::{ContentHashHex, SignatureHex};
#[cfg(test)]
use ed25519_dalek::Signer;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::io::Read;
#[cfg(test)]
use std::io::Write;
#[cfg(test)]
use std::time::{Duration, Instant};

const PURPOSE: &str = "cryptex.purge-approval.v1";
pub(crate) const FOLDER: &str = "purge-approvals";
pub(crate) const LIMIT: u64 = 1024;

#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Approval {
    purpose: String,
    brain_id: String,
    device_id: String,
    note_id: String,
    note_hash: ContentHashHex,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Signed {
    approval: Approval,
    signature: SignatureHex,
}

fn encode(value: &impl Serialize) -> Result<Vec<u8>, ReviewError> {
    serde_json_canonicalizer::to_vec(value).map_err(|_| ReviewError::ReceiptIntegrity)
}

impl Approval {
    fn new(brain: &str, device: &str, note: &str, hash: &str) -> Result<Self, ReviewError> {
        Ok(Self {
            purpose: PURPOSE.into(),
            brain_id: brain.into(),
            device_id: device.into(),
            note_id: cryptex_core::validate::parse_uuid_str(note)
                .map_err(|_| ReviewError::TargetUnavailable)?,
            note_hash: ContentHashHex::parse(hash).map_err(|_| ReviewError::TargetUnavailable)?,
        })
    }
}

impl From<export_fs::Error> for ReviewError {
    fn from(err: export_fs::Error) -> Self {
        match err {
            export_fs::Error::Unsafe => Self::ReceiptIntegrity,
            export_fs::Error::Io(_, _) => Self::ReceiptStorage,
        }
    }
}

fn authenticate(
    mut source: impl Read,
    expected: &Approval,
    key: &VerifyingKey,
) -> Result<Vec<u8>, ReviewError> {
    let mut bytes = Vec::new();
    source
        .by_ref()
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ReviewError::ReceiptStorage)?;
    if bytes.len() as u64 > LIMIT {
        return Err(ReviewError::ReceiptIntegrity);
    }
    let signed: Signed =
        serde_json::from_slice(&bytes).map_err(|_| ReviewError::ReceiptIntegrity)?;
    if &signed.approval != expected || encode(&signed)? != bytes {
        return Err(ReviewError::ReceiptIntegrity);
    }
    let sig = signed
        .signature
        .to_bytes()
        .map_err(|_| ReviewError::ReceiptIntegrity)?;
    key.verify_strict(&encode(expected)?, &Signature::from_bytes(&sig))
        .map_err(|_| ReviewError::ReceiptIntegrity)?;
    Ok(bytes)
}

#[cfg(test)]
pub(super) fn persist(
    ctx: &SearchContext,
    note_id: String,
    note_hash: String,
    expires: Instant,
) -> Result<Outcome, ReviewError> {
    let (note, hash) = (note_id.as_str(), note_hash.as_str());
    let root = &ctx.runtime_root;
    let brain =
        crate::identity_runtime::load_brain_identity(root).map_err(|_| ReviewError::Unavailable)?;
    let resolver =
        LocalDeviceKeyResolver::load(root, &brain.brain_id, ctx.store.as_ref(), ctx.environment)
            .map_err(|_| ReviewError::Unavailable)?;
    let anchor = resolver
        .local_trust_anchor()
        .ok_or(ReviewError::Unavailable)?;
    let snapshot = crate::knowledge_materialization::lock_visible_purge_target(
        root,
        &brain.brain_id,
        &anchor,
        note,
        hash,
        expires.min(Instant::now() + Duration::from_secs(2)),
    )
    .map_err(|_| ReviewError::TargetUnavailable)?;
    let key = crate::event_signing_key::load_existing_event_signing_key(
        ctx.store.as_ref(),
        ctx.environment,
        root,
        anchor.device_id(),
    )
    .map_err(|_| ReviewError::Unavailable)?
    .signing_key;
    if key.verifying_key().as_bytes() != anchor.public_key() {
        return Err(ReviewError::ReceiptIntegrity);
    }
    let expected = Approval::new(&brain.brain_id, anchor.device_id().as_str(), note, hash)?;
    let signature = SignatureHex::from_bytes(&key.sign(&encode(&expected)?).to_bytes())
        .map_err(|_| ReviewError::ReceiptIntegrity)?;
    let bytes = encode(&Signed {
        approval: expected.clone(),
        signature,
    })?;
    if !snapshot.locks_runtime(root) {
        return Err(ReviewError::Unavailable);
    }
    store_receipt(root, &expected, &key.verifying_key(), &bytes, expires)?;
    let outcome = if super::physical::enabled(root) {
        verify_reopened(
            root,
            &brain.brain_id,
            anchor.device_id().as_str(),
            note,
            hash,
            anchor.public_key(),
        )?;
        let compact = snapshot.compact_approved_note(&anchor, note, hash, &key, expires);
        drop(snapshot);
        match compact.and_then(|()| super::physical::finish_cleanup(root, &brain.brain_id, &anchor))
        {
            Ok(()) => Outcome::Purged { note_id, note_hash },
            Err(reason) => Outcome::CleanupIncomplete {
                note_id,
                note_hash,
                reason,
            },
        }
    } else {
        Outcome::Confirmed { note_id, note_hash }
    };
    ctx.admission
        .check_namespace()
        .map_err(|_| ReviewError::Unavailable)?;
    Ok(outcome)
}

// Caller holds the inventory and device locks throughout publication.
#[cfg(test)]
fn store_receipt(
    root: &std::path::Path,
    expected: &Approval,
    public: &VerifyingKey,
    bytes: &[u8],
    expires: Instant,
) -> Result<(), ReviewError> {
    authenticate(bytes, expected, public)?;
    let profile = Directory::open(root)?;
    let canonical = profile.child("canonical", false)?;
    let ledger = canonical.child(FOLDER, true)?;
    canonical.sync()?;
    let name = format!("{}.json", expected.note_id);
    if ledger.exists(&name)? {
        let file = ledger.open_file(&name, false)?;
        authenticate(&file, expected, public)?;
        export_fs::flush(&file).map_err(|_| ReviewError::ReceiptStorage)?;
        ledger.sync()?;
    } else {
        // The canonical inventory lock serializes writers and orphan staging cleanup.
        let staging = profile.child("purge-staging", true)?;
        profile.sync()?;
        if staging.exists("segment.tmp")? {
            staging.discard_scratch(staging.open_file("segment.tmp", false)?)?;
        }
        let mut file = staging.open_file("segment.tmp", true)?;
        file.write_all(bytes)
            .map_err(|_| ReviewError::ReceiptStorage)?;
        if Instant::now() >= expires {
            return Err(ReviewError::ReviewExpired);
        }
        staging.promote("segment.tmp", &file, &ledger, &name)?;
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn bridge_body(
    note: &cryptex_events::AcceptedEvent,
    needs_original_class: bool,
) -> serde_json::Value {
    let mut payload = serde_json::json!({"original_hash": note.content_hash});
    if needs_original_class {
        payload["original_classification"] = serde_json::json!(note.classification);
    }
    serde_json::json!({
        "schema_version": 2, "event_id": note.event_id, "brain_id": note.brain_id,
        "device_id": note.device_id, "device_seq": note.device_seq,
        "prev_content_hash": note.prev_content_hash, "created_at": note.created_at,
        "signing_key_id": note.signing_key_id, "classification": "restricted",
        "event_type": "memory.purge.bridge", "payload_schema_version": 1,
        "payload": payload
    })
}

/// Admit the owner's completed deletion only against this receiver's verified
/// copy of that exact note and the key in its own signed pairing history.
#[cfg(test)]
pub(crate) fn receive(
    root: &std::path::Path,
    brain: &str,
    local: &cryptex_events::LocalTrustAnchor,
    bridge: &[u8],
    approval: &[u8],
) -> Result<(), ReviewError> {
    use cryptex_events::{AuthenticatedSecurityRecord, VersionedRecord, read_versioned_record};
    if !super::physical::paired(root) {
        return Err(ReviewError::Disabled);
    }
    if approval.len() as u64 > LIMIT
        || bridge.len() > cryptex_events::MAX_ACCEPTED_RECORD_BYTES as usize
    {
        return Err(ReviewError::ReceiptIntegrity);
    }
    let expires = Instant::now() + Duration::from_secs(5);
    let snapshot =
        crate::knowledge_materialization::scan_verified_canonical_from_local_anchor_until(
            root,
            brain,
            local,
            Some(expires),
        )
        .map_err(|_| ReviewError::Unavailable)?;
    let (history, trust) = snapshot
        .verified_trust_context()
        .ok_or(ReviewError::Unauthorized)?;
    let signed: Signed =
        serde_json::from_slice(approval).map_err(|_| ReviewError::ReceiptIntegrity)?;
    let device = cryptex_core::ids::DeviceId::new(signed.approval.device_id)
        .map_err(|_| ReviewError::ReceiptIntegrity)?;
    let binding = history
        .approved_peer_binding(&device)
        .ok_or(ReviewError::Unauthorized)?;
    if history.restricted(trust, &device) || trust.effective_cutoffs.get(&device) != Some(&u64::MAX)
    {
        return Err(ReviewError::Unauthorized);
    }
    let key =
        VerifyingKey::from_bytes(&binding.public_key).map_err(|_| ReviewError::ReceiptIntegrity)?;
    let note = snapshot
        .reader_device_chain(device.as_str())
        .iter()
        .find_map(|r| match r {
            AuthenticatedSecurityRecord::V1(note) if note.event_id == signed.approval.note_id => {
                Some(note)
            }
            _ => None,
        })
        .ok_or(ReviewError::TargetUnavailable)?;
    let expected = Approval::new(
        brain,
        device.as_str(),
        &note.event_id,
        note.content_hash.as_str(),
    )?;
    let Ok(VersionedRecord::VerifiedUnsupported(_)) = read_versioned_record(bridge, &key) else {
        return Err(ReviewError::ReceiptIntegrity);
    };
    let mut envelope: serde_json::Map<String, serde_json::Value> =
        serde_json::from_slice(bridge).map_err(|_| ReviewError::ReceiptIntegrity)?;
    if encode(&envelope)? != bridge {
        return Err(ReviewError::ReceiptIntegrity);
    }
    envelope.remove("content_hash");
    envelope.remove("signature");
    // The sender may already know about a link or transition still in transit.
    // Bind the supplied class to the receiver's signed original before storing it.
    let needs_original_class = snapshot.purge_requires_original_class(&note.event_id)
        || envelope
            .get("payload")
            .and_then(|payload| payload.get("original_classification"))
            .is_some();
    let expected_bridge = bridge_body(note, needs_original_class);
    if serde_json::Value::Object(envelope) != expected_bridge {
        return Err(ReviewError::ReceiptIntegrity);
    }
    store_receipt(root, &expected, &key, approval, expires)?;
    snapshot.replace_approved_frame(local, &device, &note.event_id, bridge, expires)
}

#[cfg(test)]
pub(crate) fn verify_reopened(
    root: &std::path::Path,
    brain: &str,
    device: &str,
    note: &str,
    hash: &str,
    public: &[u8; 32],
) -> Result<(), ReviewError> {
    let ledger = Directory::open(root)?
        .child("canonical", false)?
        .child(FOLDER, false)?;
    verified_bytes(&ledger, brain, device, note, hash, public).map(drop)
}

/// Return the same bounded file bytes whose owner signature was verified.
pub(crate) fn verified_bytes(
    ledger: &Directory,
    brain: &str,
    device: &str,
    note: &str,
    hash: &str,
    public: &[u8; 32],
) -> Result<Vec<u8>, ReviewError> {
    let expected = Approval::new(brain, device, note, hash)?;
    let file = ledger.open_file(&format!("{}.json", expected.note_id), false)?;
    let key = VerifyingKey::from_bytes(public).map_err(|_| ReviewError::ReceiptIntegrity)?;
    authenticate(file, &expected, &key)
}
