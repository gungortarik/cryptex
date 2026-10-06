//! Recovery Kit **public** metadata persistence (ADR-0015). Offline secrets stay out of runtime.

use std::fs;
use std::path::{Path, PathBuf};

use cryptex_core::RecoveryKitPublicMetadata;

use crate::error::LifecycleError;
use crate::identity_runtime::IdentityRuntimeError;
use crate::paths::{assert_outside_repository, ensure_runtime_root};
use crate::secure_store::payload_bytes_recoverable;

const RECOVERY_DIR: &str = "recovery";
const PUBLIC_META_FILE: &str = "recovery_kit_public.json";

pub fn recovery_dir(runtime_root: &Path) -> PathBuf {
    runtime_root.join(RECOVERY_DIR)
}

pub fn recovery_public_metadata_path(runtime_root: &Path) -> PathBuf {
    recovery_dir(runtime_root).join(PUBLIC_META_FILE)
}

pub fn write_recovery_public_metadata(
    runtime_root: &Path,
    meta: &RecoveryKitPublicMetadata,
) -> Result<(), IdentityRuntimeError> {
    assert_outside_repository(runtime_root)?;
    ensure_runtime_root(runtime_root)?;
    let dir = recovery_dir(runtime_root);
    fs::create_dir_all(&dir)?;
    let path = recovery_public_metadata_path(runtime_root);
    let mut json = serde_json::to_string_pretty(meta).map_err(|e| {
        IdentityRuntimeError::Lifecycle(LifecycleError::Io(format!("serialize recovery meta: {e}")))
    })?;
    json = json.replace("\r\n", "\n");
    if !json.ends_with('\n') {
        json.push('\n');
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json.as_bytes())?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

pub fn load_recovery_public_metadata(
    runtime_root: &Path,
) -> Result<RecoveryKitPublicMetadata, IdentityRuntimeError> {
    let path = recovery_public_metadata_path(runtime_root);
    let text = fs::read_to_string(&path)?;
    let meta: RecoveryKitPublicMetadata = serde_json::from_str(&text).map_err(|e| {
        IdentityRuntimeError::Lifecycle(LifecycleError::Io(format!("parse recovery meta: {e}")))
    })?;
    Ok(meta)
}

/// Guard helper for tests/callers: public metadata must not recover a given secret.
pub fn public_metadata_excludes_secret(
    runtime_root: &Path,
    secret: &[u8],
) -> Result<bool, IdentityRuntimeError> {
    let text = fs::read_to_string(recovery_public_metadata_path(runtime_root))?;
    Ok(!payload_bytes_recoverable(secret, &text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cryptex_core::RecoveryPurpose;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("cryptexd-recovery-{}", nanos));
        let _ = fs::remove_dir_all(&root);
        root
    }

    #[test]
    fn write_load_public_metadata_round_trip() {
        let root = temp_root();
        let meta = RecoveryKitPublicMetadata::new(
            "kit-1",
            "brain-1",
            vec![
                RecoveryPurpose::BackupDecrypt,
                RecoveryPurpose::TrustBootstrap,
            ],
        );
        write_recovery_public_metadata(&root, &meta).expect("write");
        let loaded = load_recovery_public_metadata(&root).expect("load");
        assert_eq!(loaded, meta);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn public_metadata_excludes_offline_secret_bytes() {
        let root = temp_root();
        let meta = RecoveryKitPublicMetadata::new(
            "kit-2",
            "brain-2",
            vec![RecoveryPurpose::TrustBootstrap],
        );
        write_recovery_public_metadata(&root, &meta).expect("write");
        let offline_secret = b"SYNTHETIC_OFFLINE_RECOVERY_SECRET_NOT_IN_RUNTIME";
        assert!(public_metadata_excludes_secret(&root, offline_secret).expect("check"));
        let text = fs::read_to_string(recovery_public_metadata_path(&root)).unwrap();
        assert!(!text.contains("SYNTHETIC_OFFLINE_RECOVERY_SECRET"));
        let _ = fs::remove_dir_all(&root);
    }
}
