//! Authenticated, read-only Desktop search for a recovered profile.
use super::*;
use crate::local_device_identity::LocalDeviceKeyResolver;
use crate::secure_store::{AccountLabel, SecureStoreError, ServiceLabel};
use cryptex_core::{RecoveryKitPublicMetadata, RecoveryPurpose};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::PathBuf;

pub(super) const RECOVERY: &str = "recovery";
pub(super) const RECEIPT: &str = "first-device.json";
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Request {
    pub(super) operation: String,
    pub(super) kit_id: String,
    pub(super) brain_id: String,
    pub(super) challenge: [u8; 32],
    pub(super) backup_sha256: [u8; 32],
    pub(super) new_device: DeviceId,
    pub(super) new_key: [u8; 32],
    pub(super) archive_device: DeviceId,
    pub(super) archive_key: [u8; 32],
    pub(super) archive_history_sha256: [u8; 32],
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Approval {
    pub(super) request: Request,
    pub(super) signature: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Denial {
    Context,
    Signature,
    #[cfg(test)]
    AlreadyAuthorized,
    Receipt,
}

pub(crate) struct Ceremony {
    pub(super) expected: Request,
    pub(super) kit: RecoveryKitPublicMetadata,
    pub(super) trusted_root: VerifyingKey,
    pub(super) receipt: PathBuf,
}

impl Ceremony {
    pub(super) fn verify(&self, approval: &Approval) -> Result<(), Denial> {
        if self.kit.validate().is_err()
            || !self.kit.purposes.contains(&RecoveryPurpose::TrustBootstrap)
            || self.kit.brain_id != self.expected.brain_id
            || self.kit.kit_id != self.expected.kit_id
            || self.expected.operation != "cryptex.m11.trial.authorize-first-device.v1"
            || self.expected != approval.request
        {
            return Err(Denial::Context);
        }
        let bytes =
            serde_json_canonicalizer::to_vec(&approval.request).map_err(|_| Denial::Receipt)?;
        let signature =
            Signature::from_slice(&approval.signature).map_err(|_| Denial::Signature)?;
        self.trusted_root
            .verify_strict(&bytes, &signature)
            .map_err(|_| Denial::Signature)
    }

    pub(super) fn reopen(&self) -> Result<ArchiveReader, Denial> {
        let mut options = fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.custom_flags(
                windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT,
            );
        }
        let reader = options.open(&self.receipt).map_err(|_| Denial::Receipt)?;
        let meta = reader.metadata().map_err(|_| Denial::Receipt)?;
        if !meta.is_file() || crate::import_staging::is_link_or_reparse(&meta) {
            return Err(Denial::Receipt);
        }
        let approval: Approval =
            serde_json::from_slice(&read_receipt(reader)?).map_err(|_| Denial::Receipt)?;
        self.verify(&approval)?;
        let history = &approval.request;
        let key = VerifyingKey::from_bytes(&history.archive_key).map_err(|_| Denial::Receipt)?;
        let id =
            cryptex_events::signing_key_id_from_verifying_key(&key).map_err(|_| Denial::Receipt)?;
        Ok(ArchiveReader(LocalTrustAnchor::new(
            history.archive_device.clone(),
            id,
            key.to_bytes(),
        )))
    }
}
// This resolver is confined to historical reads. The replacement signs with
// its newly enrolled key in its own device stream.
pub(super) struct ArchiveReader(LocalTrustAnchor);
impl TrustedDaemonKeyResolver for ArchiveReader {
    fn local_trust_anchor(&self) -> Option<LocalTrustAnchor> {
        Some(self.0.clone())
    }
    fn trusted_daemon_key(&mut self, device: &str, key: &str) -> Option<VerifyingKey> {
        if self.0.device_id().as_str() != device || self.0.signing_key_id().as_str() != key {
            return None;
        }
        VerifyingKey::from_bytes(self.0.public_key()).ok()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Binding {
    pub(super) request: Request,
    pub(super) kit: RecoveryKitPublicMetadata,
    pub(super) pinned: [u8; 32],
}

fn plain_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|meta| meta.is_dir() && !crate::import_staging::is_link_or_reparse(&meta))
}

fn read_receipt(file: fs::File) -> Result<Vec<u8>, Denial> {
    const MAX_RECEIPT: u64 = 16 * 1024;
    let mut data = Vec::new();
    file.take(MAX_RECEIPT + 1)
        .read_to_end(&mut data)
        .map_err(|_| Denial::Receipt)?;
    if data.len() > MAX_RECEIPT as usize {
        return Err(Denial::Receipt);
    }
    Ok(data)
}

// Only an authenticated restore may provision this independent device credential.
// Missing or damaged recovery state must never silently downgrade to local-only hits.
pub(crate) fn for_profile(
    root: &Path,
    brain: &str,
    device: &DeviceId,
    store: &dyn SecureStore,
    environment: SecureStoreEnvironment,
) -> Result<Option<Ceremony>, SearchHold> {
    let folder = root.join(RECOVERY);
    let service = ServiceLabel::device_credentials(environment);
    let account = AccountLabel::recovery_search(device.as_str());
    let bytes = match store.get(service.as_str(), account.as_str()) {
        Ok(bytes) => bytes,
        Err(SecureStoreError::Missing) => {
            return match fs::symlink_metadata(folder) {
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
                _ => Err(SearchHold::Approval),
            };
        }
        Err(_) => return Err(SearchHold::Approval),
    };
    let binding: Binding = serde_json::from_slice(&bytes).map_err(|_| SearchHold::Approval)?;
    if !plain_directory(&folder)
        || binding.request.brain_id != brain
        || &binding.request.new_device != device
    {
        return Err(SearchHold::Approval);
    }
    Ok(Some(Ceremony {
        expected: binding.request,
        kit: binding.kit,
        trusted_root: VerifyingKey::from_bytes(&binding.pinned)
            .map_err(|_| SearchHold::Approval)?,
        receipt: folder.join(RECEIPT),
    }))
}
pub(super) fn history_digest(snapshot: &VerifiedCanonicalSnapshot) -> [u8; 32] {
    // security_history is ordered by device and sequence; each content hash is
    // fixed-width. Distinguish a purge bridge from an original record with the
    // same chain hash.
    let mut digest = Sha256::new();
    for event in snapshot.security_history.records() {
        digest.update([u8::from(matches!(
            event,
            AuthenticatedSecurityRecord::PurgeBridge(..)
        ))]);
        digest.update(event.content_hash().as_str().as_bytes());
    }
    digest.finalize().into()
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SearchHold {
    Approval,
    Archive,
    ArchiveCheckpoint,
    Replacement,
    Overlap,
    Projection,
    Deadline,
    InvalidQuery,
}

pub(crate) fn search(
    new: &Path,
    ceremony: &Ceremony,
    local: &mut LocalDeviceKeyResolver,
    query: &SearchQuery,
    deadline: Option<Instant>,
) -> Result<Vec<SearchHit>, SearchHold> {
    let mut archived = ceremony.reopen().map_err(|_| SearchHold::Approval)?;
    let key = local.local_trust_anchor().ok_or(SearchHold::Approval)?;
    if key.device_id() != &ceremony.expected.new_device
        || key.public_key() != &ceremony.expected.new_key
    {
        return Err(SearchHold::Approval);
    }
    let old = new.join(RECOVERY).join("archive");
    if !plain_directory(&old) {
        return Err(SearchHold::Archive);
    }
    let old_db = ProjectionDb::open_existing_read_only(&old).map_err(|_| SearchHold::Archive)?;
    let new_db = ProjectionDb::open_existing_read_only(new).map_err(|_| SearchHold::Replacement)?;
    let old_guard = evaluate_retrieval_safety_until(
        &old,
        &ceremony.expected.brain_id,
        &[],
        &mut archived,
        &old_db,
        deadline,
        NoteLifecycleRead::Apply,
    );
    let old_snapshot = old_guard.snapshot.as_ref().ok_or(SearchHold::Archive)?;
    if history_digest(old_snapshot) != ceremony.expected.archive_history_sha256 {
        return Err(SearchHold::ArchiveCheckpoint);
    }
    let new_guard = evaluate_retrieval_safety_until(
        new,
        &ceremony.expected.brain_id,
        &[],
        local,
        &new_db,
        deadline,
        NoteLifecycleRead::Apply,
    );
    let new_snapshot = new_guard.snapshot.as_ref().ok_or(SearchHold::Replacement)?;
    // The first-device trial admits a single fresh device's note stream. New
    // trust or lifecycle controls require reduction across both histories.
    let device = &ceremony.expected.new_device;
    if old_snapshot.locked_devices().contains(device)
        || new_snapshot.locked_devices() != std::slice::from_ref(device)
        || new_snapshot
            .security_history
            .records()
            .iter()
            .any(|r| !matches!(r, AuthenticatedSecurityRecord::V1(_)))
    {
        return Err(SearchHold::Overlap);
    }
    let ids: BTreeSet<_> = old_snapshot
        .security_history
        .records()
        .iter()
        .map(|e| e.event_id())
        .collect();
    if new_snapshot
        .security_history
        .records()
        .iter()
        .any(|e| ids.contains(e.event_id()))
    {
        return Err(SearchHold::Overlap);
    }
    // Both canonical locks stay held through the single FTS query. Project
    // only policy-admitted note bodies into this short-lived read model.
    let union = ProjectionDb::open_in_memory().map_err(|_| SearchHold::Projection)?;
    for snapshot in [old_snapshot, new_snapshot] {
        for record in snapshot.verified_records() {
            if deadline.is_some_and(|until| std::time::Instant::now() >= until) {
                return Err(SearchHold::Deadline);
            }
            if let VersionedRecord::VerifiedV1(note) = record
                && !snapshot.forgotten.contains(&note.event_id)
            {
                cryptex_projection::project_memory_note(union.connection(), note)
                    .map_err(|_| SearchHold::Projection)?;
            }
        }
    }
    let scoped = SearchQuery {
        brain_id: Some(ceremony.expected.brain_id.clone()),
        ..query.clone()
    };
    cryptex_projection::search::search_visible_events(
        union.connection(),
        &scoped,
        &BTreeSet::new(),
        deadline,
    )
    .map_err(|e| match e {
        ProjectionError::OperationDeadlineExceeded => SearchHold::Deadline,
        ProjectionError::InvalidQuery { .. } => SearchHold::InvalidQuery,
        _ => SearchHold::Projection,
    })
}
