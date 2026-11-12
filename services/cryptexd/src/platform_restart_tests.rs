//! Isolated OS-store restart prerequisite. The native owner route is tested separately.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::{AccountLabel, MacosKeychainStore, SecureStore, SecureStoreEnvironment, ServiceLabel};
use cryptex_core::DeviceId;

fn identity(root: &Path) -> DeviceId {
    let brain = crate::identity_runtime::load_brain_identity(root).unwrap();
    let local =
        crate::local_device_identity::load_local_device_identity(root, &brain.brain_id).unwrap();
    DeviceId::new(local.device_id).unwrap()
}

fn delete_key(root: &Path) -> crate::SecureStoreResult<()> {
    MacosKeychainStore::new().delete(
        ServiceLabel::device_credentials(SecureStoreEnvironment::Dev).as_str(),
        AccountLabel::device_event_signing_seed(identity(root).as_str()).as_str(),
    )
}

struct Runtime(PathBuf);
impl Drop for Runtime {
    fn drop(&mut self) {
        if crate::identity_runtime::brain_identity_path(&self.0).exists() {
            match delete_key(&self.0) {
                Ok(()) | Err(crate::SecureStoreError::Missing) => {}
                Err(_) => eprintln!("platform_restart_key_cleanup_failed"),
            }
        }
        if fs::remove_dir_all(&self.0).is_err() {
            eprintln!("platform_restart_runtime_cleanup_failed");
        }
    }
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn collect(root: &Path, path: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            assert!(!entry.file_type().unwrap().is_symlink());
            if entry.file_type().unwrap().is_dir() {
                collect(root, &entry.path(), files);
            } else {
                files.insert(
                    entry.path().strip_prefix(root).unwrap().into(),
                    fs::read(entry.path()).unwrap(),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    collect(root, root, &mut files);
    files
}

fn worker(root: &Path, phase: &str) {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "platform_restart_tests::restart_worker",
            "--nocapture",
        ])
        .env("CRYPTEX_OS_RESTART_ROOT", root)
        .env("CRYPTEX_OS_RESTART_PHASE", phase)
        .stdin(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "platform restart worker failed");
            return;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("platform restart worker timed out");
        }
        std::thread::sleep(Duration::from_millis(30));
    }
}

#[test]
fn restart_worker() {
    let Some(root) = std::env::var_os("CRYPTEX_OS_RESTART_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    crate::paths::assert_outside_repository(&root).unwrap();
    let phase = std::env::var("CRYPTEX_OS_RESTART_PHASE").unwrap();
    let store = MacosKeychainStore::new();
    if phase == "seed" {
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
        crate::signed_transfer_probe::create(
            &root,
            &store,
            "aa5d7ae0-5321-4f53-a91e-36a411d88591",
            "d9885c8e-6a01-48bc-8488-cd52c4c1f725",
        )
        .unwrap();
        return;
    }
    let device = identity(&root);
    let key = crate::event_signing_key::load_existing_event_signing_key(
        &store,
        SecureStoreEnvironment::Dev,
        &root,
        &device,
    );
    if phase == "missing" {
        assert!(matches!(
            key,
            Err(crate::event_signing_key::EventSigningKeyError::SecureStore(
                crate::SecureStoreError::Missing
            ))
        ));
        return;
    }
    assert_eq!(phase, "verify");
    let key = key.unwrap();
    let brain = crate::identity_runtime::load_brain_identity(&root).unwrap();
    let policy = crate::knowledge_materialization::scan_verified_trust_policy(
        &root,
        &brain.brain_id,
        &device,
        &cryptex_events::SigningKeyId::parse(&key.public.signing_key_id).unwrap(),
        &key.signing_key.verifying_key().to_bytes(),
        std::slice::from_ref(&device),
    )
    .unwrap();
    assert_eq!(policy.history.records().len(), 1);
}

#[test]
#[ignore = "uses isolated real Keychain credentials across processes"]
fn os_key_reloads_after_process_exit_and_missing_key_never_reenrolls() {
    let root = Runtime(crate::test_temp::exclusive_temp_dir("cryptex-os-restart"));
    worker(&root.0, "seed");
    let original = snapshot(&root.0);
    worker(&root.0, "verify");
    worker(&root.0, "verify");
    assert_eq!(snapshot(&root.0), original);
    delete_key(&root.0).unwrap();
    worker(&root.0, "missing");
    assert_eq!(snapshot(&root.0), original);
    assert!(matches!(
        delete_key(&root.0),
        Err(crate::SecureStoreError::Missing)
    ));
}
