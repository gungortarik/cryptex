//! Version/update compatibility fixtures for P0-2.
//!
//! Exact updater orchestration remains unresolved. These fixtures prove that
//! incompatible Desktop/daemon protocol pairs fail explicitly and that a
//! synthetic "canonical" marker is not damaged by a failed update simulation.

use std::fs;
use std::path::Path;

use crate::error::{LifecycleError, LifecycleResult};
use crate::paths::marker_path;

/// Spike protocol version for cryptexd lifecycle fixtures.
pub const DAEMON_PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentVersions {
    pub desktop_protocol: u32,
    pub daemon_protocol: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatDecision {
    Compatible,
    Incompatible,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompatError {
    pub message: String,
}

pub fn evaluate_compat(versions: ComponentVersions) -> Result<CompatDecision, CompatError> {
    if versions.desktop_protocol == 0 || versions.daemon_protocol == 0 {
        return Err(CompatError {
            message: "protocol version 0 is invalid".into(),
        });
    }
    if versions.desktop_protocol != versions.daemon_protocol {
        return Ok(CompatDecision::Incompatible);
    }
    Ok(CompatDecision::Compatible)
}

/// Reject an update candidate whose synthetic Desktop/daemon protocol pair is
/// incompatible before the update harness stages or activates its daemon.
pub(crate) fn require_compatible(versions: ComponentVersions) -> LifecycleResult<()> {
    match evaluate_compat(versions).map_err(|error| LifecycleError::Incompatible(error.message))? {
        CompatDecision::Compatible => Ok(()),
        CompatDecision::Incompatible => Err(LifecycleError::Incompatible(format!(
            "desktop protocol {} is incompatible with daemon protocol {}",
            versions.desktop_protocol, versions.daemon_protocol
        ))),
    }
}

/// Write a synthetic canonical marker used only for lifecycle/update safety proofs.
pub fn write_synthetic_marker(runtime_root: &Path, contents: &str) -> LifecycleResult<()> {
    fs::create_dir_all(runtime_root)?;
    fs::write(marker_path(runtime_root), contents)?;
    Ok(())
}

pub fn read_synthetic_marker(runtime_root: &Path) -> LifecycleResult<String> {
    Ok(fs::read_to_string(marker_path(runtime_root))?)
}

/// Simulate a failed Desktop+daemon update that must not mutate the marker.
pub fn simulate_failed_update(runtime_root: &Path) -> LifecycleResult<()> {
    let staging = runtime_root.join("update-staging.tmp");
    fs::write(&staging, "partial-update-bytes")?;
    // Controlled error path: refuse to replace marker; remove staging only.
    let marker_before = read_synthetic_marker(runtime_root)?;
    if marker_before.is_empty() {
        return Err(LifecycleError::Attention(
            "synthetic marker missing before failed-update simulation".into(),
        ));
    }
    let _ = fs::remove_file(&staging);
    let marker_after = read_synthetic_marker(runtime_root)?;
    if marker_after != marker_before {
        return Err(LifecycleError::Attention(
            "canonical marker changed during failed-update simulation".into(),
        ));
    }
    Ok(())
}

/// Simulate rollback by restoring marker from an explicit backup copy.
pub fn simulate_rollback(runtime_root: &Path, backup_contents: &str) -> LifecycleResult<()> {
    let backup = runtime_root.join("synthetic-canonical.marker.bak");
    fs::write(&backup, backup_contents)?;
    fs::write(marker_path(runtime_root), backup_contents)?;
    let restored = read_synthetic_marker(runtime_root)?;
    if restored != backup_contents {
        return Err(LifecycleError::Attention(
            "rollback did not restore synthetic marker".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_runtime() -> std::path::PathBuf {
        crate::test_temp::exclusive_temp_dir("cryptexd-compat")
    }

    #[test]
    fn matching_protocols_are_compatible() {
        let decision = evaluate_compat(ComponentVersions {
            desktop_protocol: 1,
            daemon_protocol: 1,
        })
        .expect("compat");
        assert_eq!(decision, CompatDecision::Compatible);
    }

    #[test]
    fn mismatched_protocols_are_incompatible() {
        let decision = evaluate_compat(ComponentVersions {
            desktop_protocol: 2,
            daemon_protocol: 1,
        })
        .expect("compat");
        assert_eq!(decision, CompatDecision::Incompatible);
    }

    #[test]
    fn incompatible_component_pair_is_rejected_explicitly() {
        let error = require_compatible(ComponentVersions {
            desktop_protocol: 2,
            daemon_protocol: 1,
        })
        .expect_err("mismatched candidate must be rejected");
        assert!(matches!(error, LifecycleError::Incompatible(_)));
    }

    #[test]
    fn invalid_zero_protocol_is_rejected_explicitly() {
        let error = require_compatible(ComponentVersions {
            desktop_protocol: 0,
            daemon_protocol: 1,
        })
        .expect_err("zero protocol must be rejected");
        assert!(matches!(error, LifecycleError::Incompatible(_)));
    }

    #[test]
    fn failed_update_preserves_marker() {
        let root = temp_runtime();
        write_synthetic_marker(&root, "canonical-v1").expect("marker");
        simulate_failed_update(&root).expect("failed update");
        assert_eq!(read_synthetic_marker(&root).expect("read"), "canonical-v1");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rollback_restores_marker() {
        let root = temp_runtime();
        write_synthetic_marker(&root, "damaged").expect("marker");
        simulate_rollback(&root, "canonical-v1").expect("rollback");
        assert_eq!(read_synthetic_marker(&root).expect("read"), "canonical-v1");
        let _ = fs::remove_dir_all(&root);
    }
}
