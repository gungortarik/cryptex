//! Daemon-owned install identity persistence (ADR-0015 section 2.2 / section 4).
//!
//! New install => new `install_id`. Existing durable identity is loaded and never
//! silently rewritten in place. First-create uses exclusive no-replace placement
//! so concurrent ensure/load_or_create callers cannot overwrite the winner.

use std::fs::{self, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use cryptex_core::{InstallId, InstallIdentity};

use crate::identity_runtime::IdentityRuntimeError;
use crate::paths::{assert_outside_repository, ensure_runtime_root};
use crate::sequence_runtime::open_sequence_store;

const IDENTITY_DIR: &str = "identity";
const INSTALL_FILE: &str = "install_identity.json";

static TMP_SEQ: AtomicU64 = AtomicU64::new(1);

pub fn install_identity_path(runtime_root: &Path) -> PathBuf {
    runtime_root.join(IDENTITY_DIR).join(INSTALL_FILE)
}

fn unique_install_tmp(path: &Path) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    path.with_extension(format!("json.tmp.{}-{}-{}", std::process::id(), seq, nanos))
}

/// Place `tmp` at `dest` only if `dest` does not already exist.
///
/// - Windows: `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING` (Rust's
///   `fs::rename` replaces on Windows and must not be used here).
/// - Unix: `hard_link` fails when the destination exists; then remove `tmp`.
fn place_new_file_no_replace(tmp: &Path, dest: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;

        // Do not set MOVEFILE_REPLACE_EXISTING — losing creators must fail.
        const MOVEFILE_WRITE_THROUGH: u32 = 0x8;

        let from: Vec<u16> = tmp.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = dest.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: pointers are null-terminated wide strings from OsStr encoding.
        let ok = unsafe {
            windows_sys::Win32::Storage::FileSystem::MoveFileExW(
                from.as_ptr(),
                to.as_ptr(),
                MOVEFILE_WRITE_THROUGH,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    #[cfg(unix)]
    {
        match fs::hard_link(tmp, dest) {
            Ok(()) => {
                let _ = fs::remove_file(tmp);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = (tmp, dest);
        Err(io::Error::new(
            ErrorKind::Unsupported,
            "exclusive install-identity create is unsupported on this platform",
        ))
    }
}

/// Persist a new install identity. Fails if one already exists (no replace).
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
    let tmp = unique_install_tmp(&path);
    {
        let mut file = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
    }

    match place_new_file_no_replace(&tmp, &path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            let _ = fs::remove_file(&tmp);
            Err(IdentityRuntimeError::AlreadyExists(
                path.display().to_string(),
            ))
        }
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e.into())
        }
    }
}

pub fn load_install_identity(runtime_root: &Path) -> Result<InstallIdentity, IdentityRuntimeError> {
    let path = install_identity_path(runtime_root);
    let text = fs::read_to_string(&path)?;
    Ok(InstallIdentity::from_json(&text)?)
}

/// Load existing install identity, or create and persist a new one when absent.
///
/// Concurrent first-create losers observe [`IdentityRuntimeError::AlreadyExists`]
/// from exclusive placement and load the persisted winner instead of overwriting.
pub fn load_or_create_install_identity(
    runtime_root: &Path,
    new_install_id: InstallId,
) -> Result<InstallIdentity, IdentityRuntimeError> {
    let path = install_identity_path(runtime_root);
    if path.exists() {
        return load_install_identity(runtime_root);
    }
    let identity = InstallIdentity::new(new_install_id)?;
    match write_install_identity(runtime_root, &identity) {
        Ok(()) => Ok(identity),
        Err(IdentityRuntimeError::AlreadyExists(_)) => load_install_identity(runtime_root),
        Err(e) => Err(e),
    }
}

fn generate_install_id() -> Result<InstallId, IdentityRuntimeError> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    // Path-safe synthetic id for a new local installation (ADR-0015 section 2.2).
    let raw = format!("install-{}-{}", std::process::id(), nanos);
    InstallId::new(raw).map_err(IdentityRuntimeError::from)
}

/// Ensure a durable install identity exists for this runtime root (create-once).
pub fn ensure_install_identity(
    runtime_root: &Path,
) -> Result<InstallIdentity, IdentityRuntimeError> {
    if install_identity_path(runtime_root).exists() {
        return load_install_identity(runtime_root);
    }
    let id = generate_install_id()?;
    load_or_create_install_identity(runtime_root, id)
}

/// Live-daemon M1 foundation bind for a runtime root under cryptexd authority.
///
/// - Persists/loads install identity (`new install => new install_id`).
/// - Opens the daemon-owned sequence store directory (no allocate; allocation is
///   for enrolled `device_id`s when a later milestone issues events).
pub fn prepare_m1_runtime_foundation(
    runtime_root: &Path,
) -> Result<InstallIdentity, IdentityRuntimeError> {
    let install = ensure_install_identity(runtime_root)?;
    let _store = open_sequence_store(runtime_root)?;
    Ok(install)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sequence_runtime::sequence_dir;
    use cryptex_core::CoreError;
    use std::sync::{Arc, Barrier};
    use std::thread;

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
        let before = fs::read(install_identity_path(&root)).expect("read before");
        let err = write_install_identity(&root, &identity).expect_err("dup");
        assert!(matches!(err, IdentityRuntimeError::AlreadyExists(_)));
        let after = fs::read(install_identity_path(&root)).expect("read after");
        assert_eq!(
            before, after,
            "duplicate write must not change persisted bytes"
        );
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

    #[test]
    fn prepare_m1_runtime_foundation_binds_install_and_sequence_dir() {
        let root = temp_root();
        let first = prepare_m1_runtime_foundation(&root).expect("prepare");
        assert!(install_identity_path(&root).is_file());
        assert!(sequence_dir(&root).is_dir());
        let second = prepare_m1_runtime_foundation(&root).expect("reprepare");
        assert_eq!(first.install_id, second.install_id);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn ensure_install_identity_is_stable_across_calls() {
        let root = temp_root();
        let a = ensure_install_identity(&root).expect("a");
        let b = ensure_install_identity(&root).expect("b");
        assert_eq!(a.install_id, b.install_id);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn concurrent_first_create_all_observe_same_install_id() {
        let root = Arc::new(temp_root());
        let n = 16;
        let barrier = Arc::new(Barrier::new(n));
        let mut handles = Vec::with_capacity(n);
        for i in 0..n {
            let root = Arc::clone(&root);
            let barrier = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier.wait();
                // Mix ensure and load_or_create with distinct candidate ids.
                if i % 2 == 0 {
                    ensure_install_identity(&root)
                } else {
                    let candidate =
                        InstallId::new(format!("install-candidate-{i}-{}", std::process::id()))
                            .expect("candidate id");
                    load_or_create_install_identity(&root, candidate)
                }
            }));
        }

        let mut observed = Vec::with_capacity(n);
        for handle in handles {
            let identity = handle.join().expect("thread").expect("create-or-load");
            observed.push(identity.install_id);
        }

        let winner = observed[0].clone();
        for id in &observed {
            assert_eq!(
                id, &winner,
                "every successful concurrent caller must observe the same install_id"
            );
        }

        let path = install_identity_path(&root);
        let persisted_bytes = fs::read(&path).expect("read persisted");
        let persisted = load_install_identity(&root).expect("load persisted");
        assert_eq!(persisted.install_id, winner);
        assert_eq!(
            fs::read(&path).expect("re-read persisted"),
            persisted_bytes,
            "persisted install identity file must remain unchanged"
        );

        // Explicit duplicate write still fails and must not mutate the winner.
        let rogue =
            InstallIdentity::new(InstallId::new("install-rogue-overwrite").unwrap()).unwrap();
        let err = write_install_identity(&root, &rogue).expect_err("dup write");
        assert!(matches!(err, IdentityRuntimeError::AlreadyExists(_)));
        assert_eq!(
            fs::read(&path).expect("read after refused write"),
            persisted_bytes,
            "refused duplicate write must leave persisted bytes unchanged"
        );

        let _ = fs::remove_dir_all(&*root);
    }
}
