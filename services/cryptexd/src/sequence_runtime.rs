//! Daemon-owned sequence allocation under the app-data runtime root (ADR-0015).
//!
//! Product callers must use these helpers (or other `cryptexd` modules). Durable
//! counters are not allocated through `cryptex-core`.

use std::path::{Path, PathBuf};

use cryptex_core::DeviceId;

use crate::identity_runtime::IdentityRuntimeResult;
use crate::paths::{assert_outside_repository, ensure_runtime_root};
use crate::sequence_store::SequenceStore;

const SEQUENCE_DIR: &str = "sequence";

pub use crate::sequence_store::COUNTER_SCHEMA_VERSION;
pub use crate::sequence_store::SequenceStore as DaemonSequenceStore;

pub fn sequence_dir(runtime_root: &Path) -> PathBuf {
    runtime_root.join(SEQUENCE_DIR)
}

/// Open the durable sequence store under `runtime_root/sequence/`.
pub fn open_sequence_store(runtime_root: &Path) -> IdentityRuntimeResult<SequenceStore> {
    assert_outside_repository(runtime_root)?;
    ensure_runtime_root(runtime_root)?;
    let dir = sequence_dir(runtime_root);
    Ok(SequenceStore::open(dir)?)
}

pub fn initialize_device_sequence(
    runtime_root: &Path,
    device_id: &DeviceId,
) -> IdentityRuntimeResult<()> {
    let store = open_sequence_store(runtime_root)?;
    Ok(store.initialize(device_id)?)
}

pub fn allocate_device_sequence(
    runtime_root: &Path,
    device_id: &DeviceId,
) -> IdentityRuntimeResult<u64> {
    let store = open_sequence_store(runtime_root)?;
    Ok(store.allocate(device_id)?)
}

pub fn last_issued_device_sequence(
    runtime_root: &Path,
    device_id: &DeviceId,
) -> IdentityRuntimeResult<u64> {
    let store = open_sequence_store(runtime_root)?;
    Ok(store.last_issued(device_id)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity_runtime::IdentityRuntimeError;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("cryptexd-seq-rt-{}", nanos));
        let _ = fs::remove_dir_all(&root);
        root
    }

    #[test]
    fn daemon_runtime_initialize_then_first_allocate_is_one() {
        let root = temp_root();
        let device = DeviceId::new("seq-rt-device-1").unwrap();
        initialize_device_sequence(&root, &device).expect("init");
        let n = allocate_device_sequence(&root, &device).expect("alloc");
        assert_eq!(n, 1);
        assert_eq!(last_issued_device_sequence(&root, &device).unwrap(), 1);
        assert_eq!(COUNTER_SCHEMA_VERSION, 1);
        assert!(sequence_dir(&root).exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn allocate_without_initialize_fails_closed_via_daemon_api() {
        let root = temp_root();
        let device = DeviceId::new("seq-rt-device-missing").unwrap();
        let err = allocate_device_sequence(&root, &device).expect_err("missing");
        assert!(matches!(
            err,
            IdentityRuntimeError::Core(cryptex_core::CoreError::SequenceStateMissing { .. })
        ));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn refuses_runtime_root_inside_repository() {
        let device = DeviceId::new("seq-rt-device-repo").unwrap();
        // Workspace-relative path that assert_outside_repository should reject when it
        // resolves inside the checkout (same guard as other identity writers).
        let repoish = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let err = initialize_device_sequence(&repoish, &device).expect_err("repo");
        assert!(matches!(err, IdentityRuntimeError::Lifecycle(_)));
    }
}
