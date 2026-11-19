//! Local confirmation evidence. Its signature covers a fixed, content-free schema.
use super::{ReviewError, SearchContext};
use crate::import_staging::export_fs::{self, Directory};
use crate::local_device_identity::LocalDeviceKeyResolver;
use cryptex_events::{ContentHashHex, SignatureHex, TrustedDaemonKeyResolver};
use ed25519_dalek::{Signature, Signer, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::time::{Duration, Instant};

const PURPOSE: &str = "cryptex.purge-approval.v1";
pub(in crate::ipc) const FOLDER: &str = "purge-approvals";
const LIMIT: u64 = 1024;

#[derive(Serialize, Deserialize, PartialEq)]
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
) -> Result<(), ReviewError> {
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
        .map_err(|_| ReviewError::ReceiptIntegrity)
}

pub(super) fn persist(
    ctx: &SearchContext,
    note: &str,
    hash: &str,
    expires: Instant,
) -> Result<(), ReviewError> {
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
    let profile = Directory::open(root)?;
    let canonical = profile.child("canonical", false)?;
    let ledger = canonical.child(FOLDER, true)?;
    canonical.sync()?;
    let name = format!("{}.json", expected.note_id);
    if ledger.exists(&name)? {
        let file = ledger.open_file(&name, false)?;
        authenticate(&file, &expected, &key.verifying_key())?;
        export_fs::flush(&file).map_err(|_| ReviewError::ReceiptStorage)?;
        ledger.sync()?;
    } else {
        // The canonical inventory lock serializes writers and orphan staging cleanup.
        let staging = profile.child("purge-staging", true)?;
        profile.sync()?;
        if staging.exists("segment.tmp")? {
            staging.discard_scratch(staging.open_file("segment.tmp", false)?)?;
        }
        let signature = SignatureHex::from_bytes(&key.sign(&encode(&expected)?).to_bytes())
            .map_err(|_| ReviewError::ReceiptIntegrity)?;
        let bytes = encode(&Signed {
            approval: expected,
            signature,
        })?;
        let mut file = staging.open_file("segment.tmp", true)?;
        file.write_all(&bytes)
            .map_err(|_| ReviewError::ReceiptStorage)?;
        if Instant::now() >= expires {
            return Err(ReviewError::ReviewExpired);
        }
        if !snapshot.locks_runtime(root) {
            return Err(ReviewError::Unavailable);
        }
        staging.promote("segment.tmp", &file, &ledger, &name)?;
    }
    if super::physical::enabled(root) {
        verify_reopened(
            root,
            &brain.brain_id,
            anchor.device_id().as_str(),
            note,
            hash,
            anchor.public_key(),
        )?;
        snapshot.compact_approved_note(&anchor, note, hash, &key, expires)?;
        drop(snapshot);
        super::physical::finish_cleanup(ctx)?;
    }
    ctx.admission
        .check_namespace()
        .map_err(|_| ReviewError::Unavailable)
}

pub(crate) fn verify_reopened(
    root: &std::path::Path,
    brain: &str,
    device: &str,
    note: &str,
    hash: &str,
    public: &[u8; 32],
) -> Result<(), ReviewError> {
    let expected = Approval::new(brain, device, note, hash)?;
    let ledger = Directory::open(root)?
        .child("canonical", false)?
        .child(FOLDER, false)?;
    let file = ledger.open_file(&format!("{}.json", expected.note_id), false)?;
    let key = VerifyingKey::from_bytes(public).map_err(|_| ReviewError::ReceiptIntegrity)?;
    authenticate(file, &expected, &key)
}
