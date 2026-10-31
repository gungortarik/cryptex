//! Recovery Kit **public** metadata persistence (ADR-0015 section 2.5).
//!
//! Offline signing / recovery secrets must never appear in these files.

use std::fs;
use std::path::{Path, PathBuf};

use cryptex_core::RecoveryKitPublicMetadata;

use crate::error::LifecycleError;
use crate::identity_runtime::IdentityRuntimeError;
use crate::metadata_persistence::write_public_metadata;
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
    meta.validate().map_err(IdentityRuntimeError::from)?;
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
    write_public_metadata(&path, json.as_bytes())
}

pub fn load_recovery_public_metadata(
    runtime_root: &Path,
) -> Result<RecoveryKitPublicMetadata, IdentityRuntimeError> {
    let path = recovery_public_metadata_path(runtime_root);
    let text = fs::read_to_string(&path)?;
    Ok(RecoveryKitPublicMetadata::from_json(&text)?)
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
    use cryptex_core::{CoreError, RecoveryPurpose};

    fn temp_root() -> PathBuf {
        crate::test_temp::exclusive_temp_dir("cryptexd-recovery")
    }

    #[test]
    fn write_load_public_metadata_round_trip() {
        let root = temp_root();
        let meta = RecoveryKitPublicMetadata::new(
            "kit-1",
            "11111111-1111-4111-8111-111111111111",
            vec![
                RecoveryPurpose::BackupDecrypt,
                RecoveryPurpose::TrustBootstrap,
            ],
        )
        .expect("meta");
        write_recovery_public_metadata(&root, &meta).expect("write");
        let loaded = load_recovery_public_metadata(&root).expect("load");
        assert_eq!(loaded, meta);
        let mut updated = meta.clone();
        updated.kit_id = "kit-replacement".into();
        write_recovery_public_metadata(&root, &updated).expect("replace existing metadata");
        assert_eq!(load_recovery_public_metadata(&root).unwrap(), updated);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn public_metadata_excludes_offline_secret_bytes() {
        let root = temp_root();
        let meta = RecoveryKitPublicMetadata::new(
            "kit-2",
            "22222222-2222-4222-8222-222222222222",
            vec![RecoveryPurpose::TrustBootstrap],
        )
        .expect("meta");
        write_recovery_public_metadata(&root, &meta).expect("write");
        let offline_secret = b"SYNTHETIC_OFFLINE_RECOVERY_SECRET_NOT_IN_RUNTIME";
        assert!(public_metadata_excludes_secret(&root, offline_secret).expect("check"));
        let text = fs::read_to_string(recovery_public_metadata_path(&root)).unwrap();
        assert!(!text.contains("SYNTHETIC_OFFLINE_RECOVERY_SECRET"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn load_fails_closed_on_future_schema_and_malformed_brain_id() {
        let root = temp_root();
        fs::create_dir_all(recovery_dir(&root)).unwrap();
        let path = recovery_public_metadata_path(&root);
        fs::write(
            &path,
            r#"{
  "schema_version": 5,
  "kit_id": "kit-x",
  "brain_id": "11111111-1111-4111-8111-111111111111",
  "purposes": ["backup_decrypt"]
}
"#,
        )
        .unwrap();
        let err = load_recovery_public_metadata(&root).unwrap_err();
        assert!(matches!(
            err,
            IdentityRuntimeError::Core(CoreError::UnsupportedSchema {
                found: 5,
                expected: 1,
                ..
            })
        ));

        fs::write(
            &path,
            r#"{
  "schema_version": 1,
  "kit_id": "kit-x",
  "brain_id": "not-uuid",
  "purposes": ["backup_decrypt"]
}
"#,
        )
        .unwrap();
        assert!(load_recovery_public_metadata(&root).is_err());
        let _ = fs::remove_dir_all(&root);
    }
}
