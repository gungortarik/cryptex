use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{LifecycleError, LifecycleResult};

/// Spike namespace under the Cryptex-Dev app-data root.
///
/// Intentionally isolated from P0-1 Syncthing paths (`syncthing-p0-1*`).
pub const P0_2_NAMESPACE: &str = "p0-2";

/// Resolve the default development runtime root for P0-2.
///
/// - macOS: `~/Library/Application Support/Cryptex-Dev/p0-2`
/// - Windows: `%LOCALAPPDATA%\Cryptex-Dev\p0-2`
pub fn default_p0_2_runtime_root() -> LifecycleResult<PathBuf> {
    let root = if cfg!(target_os = "macos") {
        let home = env::var_os("HOME").ok_or_else(|| {
            LifecycleError::Io("HOME is not set; cannot resolve Cryptex-Dev runtime root".into())
        })?;
        PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("Cryptex-Dev")
            .join(P0_2_NAMESPACE)
    } else if cfg!(target_os = "windows") {
        let local = env::var_os("LOCALAPPDATA").ok_or_else(|| {
            LifecycleError::Io(
                "LOCALAPPDATA is not set; cannot resolve Cryptex-Dev runtime root".into(),
            )
        })?;
        PathBuf::from(local)
            .join("Cryptex-Dev")
            .join(P0_2_NAMESPACE)
    } else {
        // Non-first-class platforms still keep runtime outside the repository.
        env::temp_dir().join("Cryptex-Dev").join(P0_2_NAMESPACE)
    };

    assert_outside_repository(&root)?;
    Ok(root)
}

pub fn ensure_runtime_root(root: &Path) -> LifecycleResult<()> {
    assert_outside_repository(root)?;
    fs::create_dir_all(root)?;
    Ok(())
}

pub fn lock_path(runtime_root: &Path) -> PathBuf {
    runtime_root.join("daemon.lock")
}

pub fn status_path(runtime_root: &Path) -> PathBuf {
    runtime_root.join("lifecycle.status")
}

pub fn marker_path(runtime_root: &Path) -> PathBuf {
    runtime_root.join("synthetic-canonical.marker")
}

/// Refuse runtime roots that resolve inside the source repository.
pub fn assert_outside_repository(path: &Path) -> LifecycleResult<()> {
    let Some(repo_root) = workspace_root_hint() else {
        return Ok(());
    };

    let Ok(repo) = fs::canonicalize(&repo_root) else {
        return Ok(());
    };

    let candidate = resolve_for_comparison(path);
    if candidate.starts_with(&repo) {
        return Err(LifecycleError::UnsafeRuntimePath(format!(
            "refusing runtime path inside source repository: {}",
            path.display()
        )));
    }

    Ok(())
}

fn workspace_root_hint() -> Option<PathBuf> {
    let manifest_dir = env::var_os("CARGO_MANIFEST_DIR")?;
    // services/cryptexd -> repository root
    Some(PathBuf::from(manifest_dir).join("../.."))
}

fn resolve_for_comparison(path: &Path) -> PathBuf {
    if let Ok(canonical) = fs::canonicalize(path) {
        return canonical;
    }
    if let Some(parent) = path.parent()
        && let Ok(canonical_parent) = fs::canonicalize(parent)
    {
        return canonical_parent.join(path.file_name().unwrap_or_default());
    }
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_root_uses_p0_2_namespace() {
        let root = default_p0_2_runtime_root().expect("runtime root");
        assert!(root.ends_with(P0_2_NAMESPACE));
        assert!(root.to_string_lossy().contains("Cryptex-Dev"));
        assert!(!root.to_string_lossy().contains("syncthing-p0-1"));
    }

    #[test]
    fn rejects_path_inside_repository() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let inside = manifest.join("would-be-runtime");
        let err = assert_outside_repository(&inside).expect_err("must reject");
        match err {
            LifecycleError::UnsafeRuntimePath(_) => {}
            other => panic!("unexpected error: {other}"),
        }
    }
}
