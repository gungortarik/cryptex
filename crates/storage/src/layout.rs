//! Path layout for canonical packs and quarantine (ADR-0016).
//!
//! Paths are relative to an app-data / runtime root supplied by the caller.
//! Callers must keep that root outside the source repository.

use std::path::{Path, PathBuf};

use cryptex_core::DeviceId;

use crate::{StorageError, StorageResult};

/// Segment file kind for a per-device pack index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentKind {
    /// Active appendable pack (`*.open`).
    Open,
    /// Sealed immutable pack (`*.seg`).
    Sealed,
}

impl SegmentKind {
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Sealed => "seg",
        }
    }
}

/// `<app-data-root>/canonical/events/<device_id_hex>`
pub fn canonical_events_device_dir(app_data_root: &Path, device_id: &DeviceId) -> PathBuf {
    app_data_root
        .join("canonical")
        .join("events")
        .join(device_id.as_filename_stem())
}

/// `<app-data-root>/quarantine/canonical-events`
pub fn quarantine_events_root(app_data_root: &Path) -> PathBuf {
    app_data_root.join("quarantine").join("canonical-events")
}

/// Active `.open` pack path for `segment_index`.
pub fn active_segment_path(
    app_data_root: &Path,
    device_id: &DeviceId,
    segment_index: u64,
) -> PathBuf {
    segment_path(app_data_root, device_id, segment_index, SegmentKind::Open)
}

/// Sealed `.seg` pack path for `segment_index`.
pub fn sealed_segment_path(
    app_data_root: &Path,
    device_id: &DeviceId,
    segment_index: u64,
) -> PathBuf {
    segment_path(app_data_root, device_id, segment_index, SegmentKind::Sealed)
}

fn segment_path(
    app_data_root: &Path,
    device_id: &DeviceId,
    segment_index: u64,
    kind: SegmentKind,
) -> PathBuf {
    canonical_events_device_dir(app_data_root, device_id)
        .join(format!("{segment_index:020}.{}", kind.extension()))
}

/// Parse a validated `device_id` string into [`DeviceId`].
pub fn device_id_from_str(value: &str) -> StorageResult<DeviceId> {
    DeviceId::new(value).map_err(StorageError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn builds_per_device_open_and_sealed_paths() {
        let device = DeviceId::new("device-a").unwrap();
        let root = Path::new("C:/Cryptex-Dev/runtime-fixture");
        let open = active_segment_path(root, &device, 0);
        let sealed = sealed_segment_path(root, &device, 0);
        let stem = device.as_filename_stem();

        assert!(open.ends_with(format!("canonical/events/{stem}/00000000000000000000.open")));
        assert!(sealed.ends_with(format!("canonical/events/{stem}/00000000000000000000.seg")));
        assert_ne!(open, sealed);
    }

    #[test]
    fn quarantine_root_is_under_app_data_not_canonical_events() {
        let root = Path::new("/var/tmp/cryptex-app-data");
        let q = quarantine_events_root(root);
        assert!(q.ends_with("quarantine/canonical-events"));
        assert!(!q.components().any(|c| c.as_os_str() == "canonical"));
    }

    #[test]
    fn rejects_path_unsafe_device_ids() {
        let err = device_id_from_str("../etc-passwd").unwrap_err();
        assert!(matches!(err, StorageError::InvalidDeviceId(_)));
    }
}
