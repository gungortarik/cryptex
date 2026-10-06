//! Daemon-owned install identity persistence (ADR-0015 section 2.2 / section 4).
//!
//! New install ⇒ new `install_id`. Existing durable identity is loaded and never
//! silently rewritten in place.

use std::fs;
use std::path::{Path, PathBuf};

use cryptex_core::{InstallId, InstallIdentity};

use crate::identity_runtime::IdentityRuntimeError;
use crate::paths::{assert_outside_repository, ensure_runtime_root};

const IDENTITY_DIR: &str = "identity";
const INSTALL_FILE: &str = "install_identity.json";

pub fn install_identity_path(runtime_root: &Path) -> PathBuf {
    runtime_root.join(IDENTITY_DIR).join(INSTALL_FILE)
}

/// Persist a new install identity. Fails if one already exists.
pub fn write_install_identity(
    runtime_root: &Path,
    identity: &InstallIdentity,
) -> Result<(), IdentityRuntimeError> {
    identity.validate().map_err(IdentityRuntimeError::from)?;
    assert_outside_repository(runtime_root)?;
    ensure_runtime_root(runtime_root)?;
    let dir = runtime_root.join(IDENTITY_DIR);
    fs::create_dir_all(&dir)?;
    let path = install_identity_path(runtime_root);
    if path.exists() {
        return Err(IdentityRuntimeError::AlreadyExists(
            path.display().to_string(),
        ));
    }
    let json = identity.to_json_lf()?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json.as_bytes())?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

pub fn load_install_identity(runtime_root: &Path) -> Result<InstallIdentity, IdentityRuntimeError> {
    let path = install_identity_path(runtime_root);
    let text = fs::read_to_string(&path)?;
    Ok(InstallIdentity::from_json(&text)?)
}

/// Load existing install identity, or create and persist a new one when absent.
pub fn load_or_create_install_identity(
    runtime_root: &Path,
    new_install_id: InstallId,
) -> Result<InstallIdentity, IdentityRuntimeError> {
    let path = install_identity_path(runtime_root);
    if path.exists() {
        return load_install_identity(runtime_root);
    }
    let identity = InstallIdentity::new(new_install_id)?;
    write_install_identity(runtime_root, &identity)?;
    Ok(identity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cryptex_core::CoreError;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("cryptexd-install-{}", nanos));
        let _ = fs::remove_dir_all(&root);
        root
    }

    #[test]
    fn create_load_round_trip_and_stable_across_reload() {
        let root = temp_root();
        let id = InstallId::new("install-daemon-001").unwrap();
        let created = load_or_create_install_identity(&root, id.clone()).expect("create");
        assert_eq!(created.install_id, id);
        let loaded = load_install_identity(&root).expect("load");
        assert_eq!(loaded, created);
        // Second create-or-load must not replace the durable id.
        let other = InstallId::new("install-daemon-OTHER").unwrap();
        let again = load_or_create_install_identity(&root, other).expect("reload");
        assert_eq!(again.install_id, id);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn write_refuses_duplicate_install_identity() {
        let root = temp_root();
        let identity = InstallIdentity::new(InstallId::new("install-dup-1").unwrap()).unwrap();
        write_install_identity(&root, &identity).expect("first");
        let err = write_install_identity(&root, &identity).expect_err("dup");
        assert!(matches!(err, IdentityRuntimeError::AlreadyExists(_)));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn load_fails_closed_on_future_schema() {
        let root = temp_root();
        fs::create_dir_all(root.join(IDENTITY_DIR)).unwrap();
        fs::write(
            install_identity_path(&root),
            r#"{
  "schema_version": 3,
  "install_id": "install-daemon-001"
}
"#,
        )
        .unwrap();
        let err = load_install_identity(&root).unwrap_err();
        assert!(matches!(
            err,
            IdentityRuntimeError::Core(CoreError::UnsupportedSchema {
                found: 3,
                expected: 1,
                ..
            })
        ));
        let _ = fs::remove_dir_all(&root);
    }
}
