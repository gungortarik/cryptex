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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) brain_identity_sha256: Option<[u8; 32]>,
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
    #[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
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
            || !matches!(
                (
                    self.expected.operation.as_str(),
                    self.expected.brain_identity_sha256
                ),
                ("cryptex.m11.trial.authorize-first-device.v1", None)
                    | ("cryptex.m11.trial.authorize-first-device.v2", Some(_))
            )
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
        let approval = self.checked_receipt()?;
        Self::archive_reader(&approval)
    }

    pub(super) fn checked_receipt(&self) -> Result<Approval, Denial> {
        let approval: Approval = serde_json::from_slice(&read_metadata(&self.receipt, 16 * 1024)?)
            .map_err(|_| Denial::Receipt)?;
        self.verify(&approval)?;
        Ok(approval)
    }

    pub(super) fn archive_reader(approval: &Approval) -> Result<ArchiveReader, Denial> {
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

fn is_link_or_reparse(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
        meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}

fn plain_directory(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir() && !is_link_or_reparse(&meta))
}

fn read_metadata(path: &Path, limit: u64) -> Result<Vec<u8>, Denial> {
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
        options.custom_flags(windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options.open(path).map_err(|_| Denial::Receipt)?;
    let meta = file.metadata().map_err(|_| Denial::Receipt)?;
    if !meta.is_file() || is_link_or_reparse(&meta) {
        return Err(Denial::Receipt);
    }
    let mut data = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut data)
        .map_err(|_| Denial::Receipt)?;
    if data.len() > limit as usize {
        return Err(Denial::Receipt);
    }
    Ok(data)
}

// Ordinary enrolled devices can have M1 public Kit metadata in this same
// directory. It identifies the Brain but supplies no recovery authority.
fn check_unbound_directory(root: &Path, brain: &str) -> Result<(), SearchHold> {
    use crate::recovery_store::{recovery_dir, recovery_public_metadata_path};
    let directory = recovery_dir(root);
    match fs::symlink_metadata(&directory) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Ok(meta) if meta.is_dir() && !is_link_or_reparse(&meta) => {}
        _ => return Err(SearchHold::Approval),
    }
    let path = recovery_public_metadata_path(root);
    let bytes = read_metadata(&path, 16 * 1024).map_err(|_| SearchHold::Approval)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| SearchHold::Approval)?;
    let public = RecoveryKitPublicMetadata::from_json(text).map_err(|_| SearchHold::Approval)?;
    let lock = path.with_extension("json.lock");
    let mut found = false;
    for entry in fs::read_dir(&directory).map_err(|_| SearchHold::Approval)? {
        let entry = entry.map_err(|_| SearchHold::Approval)?.path();
        if entry == path {
            found = true;
        } else if entry != lock || read_metadata(&entry, 0).is_err() {
            return Err(SearchHold::Approval);
        }
    }
    if !found || public.brain_id != brain || !plain_directory(&directory) {
        return Err(SearchHold::Approval);
    }
    Ok(())
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
            check_unbound_directory(root, brain)?;
            return Ok(None);
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

#[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
pub(crate) fn verify_backup_reconstruction(
    root: &Path,
    expected: &crate::backup_inventory::Checkpoint<'_>,
) -> Result<(), crate::backup_inventory::Error> {
    use crate::backup_inventory::Error;
    use crate::import_staging::export_fs::Directory;
    let directory = Directory::open(&root.join("identity")).map_err(|_| Error::Archive)?;
    let mut identity = directory
        .open_existing_brain_identity()
        .map_err(|_| Error::Archive)?;
    let mut bytes = Vec::new();
    (&mut identity)
        .take(4097)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Archive)?;
    if bytes.len() > 4096
        || <[u8; 32]>::from(Sha256::digest(&bytes)) != expected.brain_identity_sha256
    {
        return Err(Error::Checkpoint);
    }
    let snapshot = super::backup::snapshot(root, expected.brain_id, expected.writer)?;
    if history_digest(&snapshot) != expected.history_sha256 {
        return Err(Error::Checkpoint);
    }
    // An intact blob name and digest alone do not authorize restoring its
    // bytes. A previous attachment may have lost its final link to a purge.
    let digests = super::backup::blob_digests(&snapshot)?;
    let canonical = Directory::open(&root.join("canonical")).map_err(|_| Error::Archive)?;
    if canonical.exists("blobs").map_err(|_| Error::Attachment)? {
        let directory = canonical
            .child("blobs", false)
            .map_err(|_| Error::Attachment)?;
        let mut expected: BTreeSet<_> = digests
            .iter()
            .map(|&value| format!("{:x}", sha2::digest::Output::<Sha256>::from(value)))
            .collect();
        for entry in fs::read_dir(root.join("canonical/blobs")).map_err(|_| Error::Attachment)? {
            let name = entry.map_err(|_| Error::Attachment)?.file_name();
            if !name.to_str().is_some_and(|n| expected.remove(n)) {
                return Err(Error::Attachment);
            }
        }
        directory.validate().map_err(|_| Error::Attachment)?;
        if !expected.is_empty() {
            return Err(Error::Attachment);
        }
    }
    for digest in digests {
        crate::attachment_blobs::open_verified(root, &digest).map_err(|_| Error::Attachment)?;
    }
    Ok(())
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

// Only a replacement bound to a signed first-device receipt can read the
// archive before its first local event. Hold both canonical locks through the
// query. A missing stream, a pack (even empty), or prior capture evidence
// cannot be interpreted as a new device.
fn pristine_snapshot(
    root: &Path,
    brain: &str,
    device: &DeviceId,
    local: &mut LocalDeviceKeyResolver,
    db: &ProjectionDb,
    deadline: Option<Instant>,
) -> Result<VerifiedCanonicalSnapshot, SearchHold> {
    let directory = canonical_events_device_dir(root, device);
    if !plain_directory(&directory) || validate_schema_v1_read_only(db.connection()).is_err() {
        return Err(SearchHold::Replacement);
    }
    let snapshot =
        scan_verified_canonical_until(root, brain, std::slice::from_ref(device), local, deadline)
            .map_err(|error| match error {
            KnowledgeMaterializationError::DeadlineExceeded => SearchHold::Deadline,
            _ => SearchHold::Replacement,
        })?;
    if snapshot._locks.len() != 1 || !snapshot.security_history.records().is_empty() {
        return Err(SearchHold::Replacement);
    }
    let mut entries = fs::read_dir(directory).map_err(|_| SearchHold::Replacement)?;
    let first = entries
        .next()
        .transpose()
        .map_err(|_| SearchHold::Replacement)?;
    if !first.is_some_and(|entry| {
        entry.file_name() == "writer.lock" && entry.file_type().is_ok_and(|t| t.is_file())
    }) || entries.next().is_some()
    {
        return Err(SearchHold::Replacement);
    }
    match fs::symlink_metadata(crate::capture_receipt::capture_receipts_root(root)) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        _ => return Err(SearchHold::Replacement),
    }
    let used: bool = db
        .connection()
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM events) OR EXISTS(SELECT 1 FROM device_checkpoint)",
            [],
            |row| row.get(0),
        )
        .map_err(|_| SearchHold::Replacement)?;
    if used {
        return Err(SearchHold::Replacement);
    }
    Ok(snapshot)
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
    if let Some(expected) = ceremony.expected.brain_identity_sha256 {
        for (root, hold) in [
            (&old as &Path, SearchHold::ArchiveCheckpoint),
            (new, SearchHold::Replacement),
        ] {
            if !plain_directory(&root.join("identity"))
                || !read_metadata(&crate::identity_runtime::brain_identity_path(root), 4096)
                    .is_ok_and(|bytes| <[u8; 32]>::from(Sha256::digest(bytes)) == expected)
            {
                return Err(hold);
            }
        }
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
    let fresh;
    let new_snapshot = match new_guard.snapshot.as_ref() {
        Some(snapshot) => snapshot,
        None if new_guard.hold == Some(RetrievalSafetyHold::CanonicalVerificationUnavailable) => {
            fresh = pristine_snapshot(
                new,
                &ceremony.expected.brain_id,
                key.device_id(),
                local,
                &new_db,
                deadline,
            )?;
            &fresh
        }
        None if new_guard.hold == Some(RetrievalSafetyHold::OperationDeadlineExceeded) => {
            return Err(SearchHold::Deadline);
        }
        None => return Err(SearchHold::Replacement),
    };
    // The first-device trial admits a single fresh device's note stream. New
    // trust or lifecycle controls require reduction across both histories.
    if !disjoint_histories(old_snapshot, new_snapshot, &ceremony.expected.new_device) {
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

pub(super) fn disjoint_histories(
    old: &VerifiedCanonicalSnapshot,
    new: &VerifiedCanonicalSnapshot,
    device: &DeviceId,
) -> bool {
    let ids: BTreeSet<_> = old
        .security_history
        .records()
        .iter()
        .map(|e| e.event_id())
        .collect();
    !old.locked_devices().contains(device)
        && new.locked_devices() == std::slice::from_ref(device)
        && new
            .security_history
            .records()
            .iter()
            .all(|e| matches!(e, AuthenticatedSecurityRecord::V1(_)) && !ids.contains(e.event_id()))
}

#[cfg(test)]
#[test]
fn only_a_pristine_replacement_stream_can_open_an_archive_without_local_records() {
    let stage = tempfile::tempdir().unwrap();
    let root = stage.path();
    let brain = "99999999-9999-4999-8999-999999999999";
    let store = crate::secure_store::FakeSecureStore::new();
    let env = SecureStoreEnvironment::Dev;
    let raw = cryptex_core::BrainIdentity::new(brain, "2026-10-08T00:00:00Z")
        .unwrap()
        .to_json_lf()
        .unwrap();
    crate::local_device_identity::join_existing_brain(root, &store, env, &raw, brain).unwrap();
    let mut local = LocalDeviceKeyResolver::load(root, brain, &store, env).unwrap();
    let anchor = local.local_trust_anchor().unwrap();
    let device = anchor.device_id();
    let (db, _) = crate::projection::open_projection_db(root).unwrap();
    assert!(pristine_snapshot(root, brain, device, &mut local, &db, None).is_err());
    drop(crate::canonical_writer::acquire_device_canonical_lock(root, device).unwrap());
    drop(pristine_snapshot(root, brain, device, &mut local, &db, None).unwrap());
    let pack = active_segment_path(root, device, 0);
    fs::write(&pack, []).unwrap();
    assert!(pristine_snapshot(root, brain, device, &mut local, &db, None).is_err());
    fs::remove_file(pack).unwrap();
    fs::create_dir(crate::capture_receipt::capture_receipts_root(root)).unwrap();
    assert!(pristine_snapshot(root, brain, device, &mut local, &db, None).is_err());
}
