//! Read-only M8 diagnostics, not a peer trust resolver or transport controller.
//!
//! Pairing records are declarations from the M1 foundation. Their presence must
//! never imply authenticated peers, running replication, or protected memory.

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path};

use cryptex_core::{BrainIdentity, PairingRecord, PairingState};
use serde::Serialize;

use crate::identity_runtime::brain_identity_path;
use crate::local_device_identity::{LocalDeviceIdentity, local_device_identity_path};
use crate::pairing_store::pairing_dir;
use crate::paths::assert_outside_repository;

// Diagnostic resource bounds; these are not canonical protocol/device limits.
const MAX_METADATA_BYTES: u64 = 64 * 1024;
const MAX_INVENTORY_ENTRIES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataPresence {
    Missing,
    Present,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplicationProtection {
    NotConfigured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImplementationStatus {
    NotImplemented,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct PairingRecordCounts {
    pub pending: usize,
    /// Counts persisted declarations, not authenticated or accepted peers.
    pub declared_trusted: usize,
    pub retired: usize,
    pub compromised: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReplicationStatus {
    /// Diagnostic output schema; unrelated to canonical event schemas.
    pub schema_version: u32,
    pub protection: ReplicationProtection,
    pub transport_controller: ImplementationStatus,
    pub peer_authentication: ImplementationStatus,
    pub incoming_replication_pipeline: ImplementationStatus,
    pub brain_metadata: MetadataPresence,
    /// Metadata presence does not verify private-key availability.
    pub local_device_metadata: MetadataPresence,
    pub pairing_records: PairingRecordCounts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataKind {
    Brain,
    LocalDevice,
    Pairing,
}

impl MetadataKind {
    fn label(self) -> &'static str {
        match self {
            Self::Brain => "Brain",
            Self::LocalDevice => "local device",
            Self::Pairing => "pairing",
        }
    }
}

/// Redacted diagnostics never include metadata bodies, identifiers, or paths.
#[derive(Debug, PartialEq, Eq)]
pub enum ReplicationStatusError {
    UnsafeRuntimePath,
    Io {
        metadata: MetadataKind,
        kind: io::ErrorKind,
    },
    InvalidMetadata(MetadataKind),
    MetadataTooLarge(MetadataKind),
    MissingBrain(MetadataKind),
    BrainMismatch(MetadataKind),
    PairingFilenameMismatch,
    PairingRecordDisappeared,
    InventoryLimitExceeded,
}

impl std::fmt::Display for ReplicationStatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsafeRuntimePath => f.write_str("replication status requires an absolute app-data path outside the repository without parent traversal"),
            Self::Io { metadata, kind } => write!(f, "{} metadata I/O failed ({kind:?})", metadata.label()),
            Self::InvalidMetadata(kind) => write!(f, "{} metadata is invalid or unsupported", kind.label()),
            Self::MetadataTooLarge(kind) => write!(f, "{} metadata exceeds the diagnostic size bound", kind.label()),
            Self::MissingBrain(kind) => write!(f, "{} metadata exists without Brain metadata", kind.label()),
            Self::BrainMismatch(kind) => write!(f, "{} metadata belongs to another Brain", kind.label()),
            Self::PairingFilenameMismatch => f.write_str("pairing filename does not match its peer identity"),
            Self::PairingRecordDisappeared => f.write_str("pairing inventory changed during inspection; retry status"),
            Self::InventoryLimitExceeded => f.write_str("pairing inventory exceeds the diagnostic entry bound"),
        }
    }
}

impl std::error::Error for ReplicationStatusError {}

/// Inspect public metadata without creating directories, locks, identities, keys,
/// or network connections. This non-atomic inventory is diagnostic only and must
/// never authorize event acceptance. It does not verify canonical history.
pub fn inspect_replication_status(
    root: &Path,
) -> Result<ReplicationStatus, ReplicationStatusError> {
    if !root.is_absolute()
        || root
            .components()
            .any(|part| matches!(part, Component::ParentDir))
        || assert_outside_repository(root).is_err()
    {
        return Err(ReplicationStatusError::UnsafeRuntimePath);
    }

    let brain = read_metadata(&brain_identity_path(root), MetadataKind::Brain)?
        .map(|bytes| {
            let text = std::str::from_utf8(&bytes)
                .map_err(|_| ReplicationStatusError::InvalidMetadata(MetadataKind::Brain))?;
            BrainIdentity::from_json(text)
                .map_err(|_| ReplicationStatusError::InvalidMetadata(MetadataKind::Brain))
        })
        .transpose()?;
    let local = read_metadata(&local_device_identity_path(root), MetadataKind::LocalDevice)?
        .map(|bytes| {
            let identity: LocalDeviceIdentity = serde_json::from_slice(&bytes)
                .map_err(|_| ReplicationStatusError::InvalidMetadata(MetadataKind::LocalDevice))?;
            identity
                .validate()
                .map_err(|_| ReplicationStatusError::InvalidMetadata(MetadataKind::LocalDevice))?;
            require_brain(
                brain.as_ref(),
                &identity.brain_id,
                MetadataKind::LocalDevice,
            )?;
            Ok::<_, ReplicationStatusError>(identity)
        })
        .transpose()?;
    let pairing_records = inspect_pairing_records(root, brain.as_ref(), MAX_INVENTORY_ENTRIES)?;

    Ok(ReplicationStatus {
        schema_version: 1,
        protection: ReplicationProtection::NotConfigured,
        transport_controller: ImplementationStatus::NotImplemented,
        peer_authentication: ImplementationStatus::NotImplemented,
        incoming_replication_pipeline: ImplementationStatus::NotImplemented,
        brain_metadata: if brain.is_some() {
            MetadataPresence::Present
        } else {
            MetadataPresence::Missing
        },
        local_device_metadata: if local.is_some() {
            MetadataPresence::Present
        } else {
            MetadataPresence::Missing
        },
        pairing_records,
    })
}

fn read_metadata(
    path: &Path,
    metadata: MetadataKind,
) -> Result<Option<Vec<u8>>, ReplicationStatusError> {
    let attributes = match fs::symlink_metadata(path) {
        Ok(attributes) => attributes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(ReplicationStatusError::Io {
                metadata,
                kind: error.kind(),
            });
        }
    };
    if !attributes.is_file() {
        return Err(ReplicationStatusError::InvalidMetadata(metadata));
    }
    if attributes.len() > MAX_METADATA_BYTES {
        return Err(ReplicationStatusError::MetadataTooLarge(metadata));
    }
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(ReplicationStatusError::Io {
                metadata,
                kind: error.kind(),
            });
        }
    };
    let mut bytes = Vec::new();
    file.take(MAX_METADATA_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| ReplicationStatusError::Io {
            metadata,
            kind: error.kind(),
        })?;
    if bytes.len() as u64 > MAX_METADATA_BYTES {
        return Err(ReplicationStatusError::MetadataTooLarge(metadata));
    }
    Ok(Some(bytes))
}

fn require_brain(
    brain: Option<&BrainIdentity>,
    expected: &str,
    metadata: MetadataKind,
) -> Result<(), ReplicationStatusError> {
    let brain = brain.ok_or(ReplicationStatusError::MissingBrain(metadata))?;
    if brain.brain_id != expected {
        return Err(ReplicationStatusError::BrainMismatch(metadata));
    }
    Ok(())
}

fn inspect_pairing_records(
    root: &Path,
    brain: Option<&BrainIdentity>,
    max_entries: usize,
) -> Result<PairingRecordCounts, ReplicationStatusError> {
    let io_error = |error: io::Error| ReplicationStatusError::Io {
        metadata: MetadataKind::Pairing,
        kind: error.kind(),
    };
    let entries = match fs::read_dir(pairing_dir(root)) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(PairingRecordCounts::default());
        }
        Err(error) => return Err(io_error(error)),
    };
    let mut counts = PairingRecordCounts::default();
    for (index, entry) in entries.enumerate() {
        if index >= max_entries {
            return Err(ReplicationStatusError::InventoryLimitExceeded);
        }
        let entry = entry.map_err(io_error)?;
        let path = entry.path();
        if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        let bytes = read_metadata(&path, MetadataKind::Pairing)?
            .ok_or(ReplicationStatusError::PairingRecordDisappeared)?;
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| ReplicationStatusError::InvalidMetadata(MetadataKind::Pairing))?;
        let record = PairingRecord::from_json(text)
            .map_err(|_| ReplicationStatusError::InvalidMetadata(MetadataKind::Pairing))?;
        require_brain(brain, &record.local_brain_id, MetadataKind::Pairing)?;
        let expected_name = format!("{}.json", record.peer_device_id.as_filename_stem());
        if entry.file_name() != std::ffi::OsStr::new(&expected_name) {
            return Err(ReplicationStatusError::PairingFilenameMismatch);
        }
        match record.state {
            PairingState::Pending => counts.pending += 1,
            PairingState::Trusted => counts.declared_trusted += 1,
            PairingState::Retired => counts.retired += 1,
            PairingState::Compromised => counts.compromised += 1,
        }
    }
    Ok(counts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity_runtime::write_brain_identity;
    use crate::pairing_store::write_pairing_record;
    use cryptex_core::DeviceId;

    const BRAIN: &str = "11111111-1111-4111-8111-111111111111";
    const OTHER_BRAIN: &str = "22222222-2222-4222-8222-222222222222";

    fn root() -> std::path::PathBuf {
        crate::test_temp::exclusive_temp_dir("cryptex-m8-status")
    }
    fn seed_brain(root: &Path) {
        write_brain_identity(
            root,
            &BrainIdentity::new(BRAIN, "2026-10-19T00:00:00Z").unwrap(),
        )
        .unwrap();
    }
    fn seed_pairing(root: &Path, peer: &str, state: PairingState) -> std::path::PathBuf {
        let peer = DeviceId::new(peer).unwrap();
        let record =
            PairingRecord::new(peer.clone(), state, BRAIN, "2026-10-19T00:00:00Z").unwrap();
        write_pairing_record(root, &record).unwrap();
        pairing_dir(root).join(format!("{}.json", peer.as_filename_stem()))
    }
    fn seed_local_metadata(root: &Path, brain: &str) {
        fs::create_dir_all(root.join("identity")).unwrap();
        let identity = LocalDeviceIdentity {
            schema_version: 1,
            brain_id: brain.into(),
            device_id: "local-device".into(),
        };
        fs::write(
            local_device_identity_path(root),
            serde_json::to_vec(&identity).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn missing_runtime_is_reported_without_creating_it() {
        let parent = root();
        let missing = parent.join("missing");
        let status = inspect_replication_status(&missing).unwrap();
        assert_eq!(status.brain_metadata, MetadataPresence::Missing);
        assert_eq!(status.local_device_metadata, MetadataPresence::Missing);
        assert_eq!(status.protection, ReplicationProtection::NotConfigured);
        assert!(!missing.exists());
        fs::remove_dir(parent).unwrap();
    }

    #[test]
    fn declarations_never_imply_protection_or_peer_authentication() {
        let root = root();
        seed_brain(&root);
        seed_local_metadata(&root, BRAIN);
        let states = [
            PairingState::Pending,
            PairingState::Trusted,
            PairingState::Retired,
            PairingState::Compromised,
        ];
        let mut before = Vec::new();
        for (index, state) in states.into_iter().enumerate() {
            let path = seed_pairing(&root, &format!("peer-{index}"), state);
            before.push((path.clone(), fs::read(path).unwrap()));
        }
        let brain_bytes = fs::read(brain_identity_path(&root)).unwrap();
        let local_bytes = fs::read(local_device_identity_path(&root)).unwrap();
        fs::write(root.join("sentinel"), b"unchanged").unwrap();
        let status = inspect_replication_status(&root).unwrap();
        assert_eq!(
            status.pairing_records,
            PairingRecordCounts {
                pending: 1,
                declared_trusted: 1,
                retired: 1,
                compromised: 1
            }
        );
        assert_eq!(status.protection, ReplicationProtection::NotConfigured);
        assert_eq!(
            status.peer_authentication,
            ImplementationStatus::NotImplemented
        );
        assert_eq!(
            status.transport_controller,
            ImplementationStatus::NotImplemented
        );
        assert_eq!(status.local_device_metadata, MetadataPresence::Present);
        assert_eq!(fs::read(brain_identity_path(&root)).unwrap(), brain_bytes);
        assert_eq!(
            fs::read(local_device_identity_path(&root)).unwrap(),
            local_bytes
        );
        for (path, bytes) in before {
            assert_eq!(fs::read(path).unwrap(), bytes);
        }
        assert_eq!(fs::read(root.join("sentinel")).unwrap(), b"unchanged");
        assert!(!root.join("daemon.lock").exists());
        assert!(!root.join("lifecycle.status").exists());
        assert!(!root.join("canonical").exists());
        let json = serde_json::to_string(&status).unwrap();
        assert!(!json.contains(BRAIN));
        assert!(!json.contains("local-device"));
        assert!(!json.contains("peer-1"));
        assert!(!json.contains(&root.to_string_lossy().to_string()));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn corrupt_metadata_returns_a_redacted_typed_error() {
        let root = root();
        fs::create_dir(root.join("identity")).unwrap();
        fs::write(brain_identity_path(&root), b"private-marker-not-json").unwrap();
        let error = inspect_replication_status(&root).unwrap_err();
        assert_eq!(
            error,
            ReplicationStatusError::InvalidMetadata(MetadataKind::Brain)
        );
        assert!(!error.to_string().contains("private-marker"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_metadata_requires_a_matching_brain() {
        let root = root();
        seed_local_metadata(&root, BRAIN);
        assert_eq!(
            inspect_replication_status(&root).unwrap_err(),
            ReplicationStatusError::MissingBrain(MetadataKind::LocalDevice)
        );
        seed_brain(&root);
        seed_local_metadata(&root, OTHER_BRAIN);
        assert_eq!(
            inspect_replication_status(&root).unwrap_err(),
            ReplicationStatusError::BrainMismatch(MetadataKind::LocalDevice)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn future_local_metadata_is_not_reported_as_present() {
        let root = root();
        seed_brain(&root);
        seed_local_metadata(&root, BRAIN);
        let mut identity: LocalDeviceIdentity =
            serde_json::from_slice(&fs::read(local_device_identity_path(&root)).unwrap()).unwrap();
        identity.schema_version = 2;
        fs::write(
            local_device_identity_path(&root),
            serde_json::to_vec(&identity).unwrap(),
        )
        .unwrap();
        assert_eq!(
            inspect_replication_status(&root).unwrap_err(),
            ReplicationStatusError::InvalidMetadata(MetadataKind::LocalDevice)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn malformed_pairing_is_not_skipped() {
        let root = root();
        seed_brain(&root);
        let path = seed_pairing(&root, "peer", PairingState::Trusted);
        fs::write(path, b"not-json").unwrap();
        assert_eq!(
            inspect_replication_status(&root).unwrap_err(),
            ReplicationStatusError::InvalidMetadata(MetadataKind::Pairing)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn foreign_brain_pairing_is_rejected() {
        let root = root();
        seed_brain(&root);
        let path = seed_pairing(&root, "peer", PairingState::Trusted);
        let mut record = PairingRecord::from_json(&fs::read_to_string(&path).unwrap()).unwrap();
        record.local_brain_id = OTHER_BRAIN.into();
        fs::write(path, serde_json::to_vec(&record).unwrap()).unwrap();
        assert_eq!(
            inspect_replication_status(&root).unwrap_err(),
            ReplicationStatusError::BrainMismatch(MetadataKind::Pairing)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn copied_pairing_record_cannot_be_counted_twice() {
        let root = root();
        seed_brain(&root);
        let path = seed_pairing(&root, "peer", PairingState::Trusted);
        fs::copy(path, pairing_dir(&root).join("duplicate.json")).unwrap();
        assert_eq!(
            inspect_replication_status(&root).unwrap_err(),
            ReplicationStatusError::PairingFilenameMismatch
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn oversized_public_metadata_is_bounded() {
        let root = root();
        seed_brain(&root);
        fs::write(
            brain_identity_path(&root),
            vec![b' '; MAX_METADATA_BYTES as usize + 1],
        )
        .unwrap();
        assert_eq!(
            inspect_replication_status(&root).unwrap_err(),
            ReplicationStatusError::MetadataTooLarge(MetadataKind::Brain)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn diagnostic_inventory_has_an_entry_bound() {
        let root = root();
        seed_brain(&root);
        seed_pairing(&root, "peer-one", PairingState::Pending);
        seed_pairing(&root, "peer-two", PairingState::Pending);
        let brain = BrainIdentity::new(BRAIN, "2026-10-19T00:00:00Z").unwrap();
        assert_eq!(
            inspect_pairing_records(&root, Some(&brain), 1).unwrap_err(),
            ReplicationStatusError::InventoryLimitExceeded
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn case_distinct_peer_ids_remain_distinct() {
        let root = root();
        seed_brain(&root);
        seed_pairing(&root, "Peer-A", PairingState::Pending);
        seed_pairing(&root, "peer-a", PairingState::Pending);
        assert_eq!(
            inspect_replication_status(&root)
                .unwrap()
                .pairing_records
                .pending,
            2
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn relative_and_parent_traversal_paths_are_refused() {
        assert_eq!(
            inspect_replication_status(Path::new("relative")).unwrap_err(),
            ReplicationStatusError::UnsafeRuntimePath
        );
        let root = root();
        assert_eq!(
            inspect_replication_status(&root.join("..").join("elsewhere")).unwrap_err(),
            ReplicationStatusError::UnsafeRuntimePath
        );
        fs::remove_dir(root).unwrap();
    }
}
