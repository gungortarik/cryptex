use super::*;
use crate::secure_store::{FakeSecureStore, WindowsCredentialStore};
use std::fs;
use std::path::PathBuf;

const KEY: &[u8] = b"SYNTHETIC_NATIVE_API_KEY_0123456789abcdef";
const OTHER_KEY: &[u8] = b"SYNTHETIC_OTHER_API_KEY_0123456789abcdef";

struct Fixture {
    parent: PathBuf,
    path: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let parent = crate::test_temp::exclusive_temp_dir("cryptex-m8-profile");
        let mut id = [0u8; 16];
        getrandom::getrandom(&mut id).unwrap();
        let name = id.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let path = parent.join(name);
        Self { parent, path }
    }
    fn create(&self) -> ManagedProfile {
        ManagedProfile::create(&self.path, SecureStoreEnvironment::Dev).unwrap()
    }
    fn open(&self) -> ManagedProfile {
        ManagedProfile::open(&self.path, SecureStoreEnvironment::Dev).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Only this fixture's exclusive test root; no computed shared cleanup.
        let allowed = fs::canonicalize(std::env::temp_dir()).unwrap();
        let target = fs::canonicalize(&self.parent).unwrap();
        assert!(target.starts_with(&allowed) && target != allowed);
        assert!(
            target
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("cryptex-m8-profile-")
        );
        fs::remove_dir_all(target).unwrap();
    }
}

#[test]
fn explicit_create_private_files_and_reopen_credential() {
    let fixture = Fixture::new();
    let store = FakeSecureStore::new();
    let mut profile = fixture.create();
    assert!(profile.load_api_credential(&store).is_err());
    profile.enroll_native_api_key(&store, KEY).unwrap();
    assert_eq!(store.put_count(), 1);
    profile.enroll_native_api_key(&store, KEY).unwrap();
    assert_eq!(store.put_count(), 1);
    assert!(!format!("{profile:?}").contains(fixture.path.to_str().unwrap()));
    let binding = fs::read(fixture.path.join("api-binding.json")).unwrap();
    assert!(!String::from_utf8_lossy(&binding).contains(std::str::from_utf8(KEY).unwrap()));
    drop(profile);
    fixture.open().load_api_credential(&store).unwrap();
}

#[test]
fn existing_unprotected_directory_refused_without_acl_repair() {
    let fixture = Fixture::new();
    fs::create_dir(&fixture.path).unwrap();
    fs::write(fixture.path.join("owner-sentinel"), b"preserve").unwrap();
    assert_eq!(
        ManagedProfile::create(&fixture.path, SecureStoreEnvironment::Dev).unwrap_err(),
        ProfileError::AlreadyExists
    );
    assert_eq!(
        ManagedProfile::open(&fixture.path, SecureStoreEnvironment::Dev).unwrap_err(),
        ProfileError::UnsafePermissions
    );
    assert_eq!(
        fs::read(fixture.path.join("owner-sentinel")).unwrap(),
        b"preserve"
    );
}

#[test]
fn session_exclusive_and_root_cannot_be_renamed_while_held() {
    let fixture = Fixture::new();
    let profile = fixture.create();
    assert_eq!(
        ManagedProfile::open(&fixture.path, SecureStoreEnvironment::Dev).unwrap_err(),
        ProfileError::Busy
    );
    assert!(fs::rename(&fixture.path, fixture.parent.join("replacement")).is_err());
    assert!(fs::rename(&fixture.parent, fixture.parent.with_extension("renamed")).is_err());
    drop(profile);
    drop(fixture.open());
}

#[test]
fn namespace_and_corrupt_identity_fail_closed() {
    let fixture = Fixture::new();
    drop(fixture.create());
    assert_eq!(
        ManagedProfile::open(&fixture.path, SecureStoreEnvironment::Prod).unwrap_err(),
        ProfileError::InvalidState
    );
    fs::write(
        fixture.path.join("profile.json"),
        b"{\"schema\":1,\"schema\":1}",
    )
    .unwrap();
    assert_eq!(
        ManagedProfile::open(&fixture.path, SecureStoreEnvironment::Dev).unwrap_err(),
        ProfileError::InvalidState
    );
}

#[test]
fn binding_written_before_store_failure_retry_same_key_only() {
    let fixture = Fixture::new();
    let store = FakeSecureStore::new();
    let mut profile = fixture.create();
    store.fail_next_put_with(SecureStoreError::Platform {
        op: "synthetic",
        code: 5,
    });
    assert_eq!(
        profile.enroll_native_api_key(&store, KEY).unwrap_err(),
        ProfileError::CredentialUnavailable
    );
    assert!(fixture.path.join("api-binding.json").exists());
    assert!(!fixture.path.join("api-ready").exists());
    drop(profile);
    let mut profile = fixture.open();
    assert_eq!(
        profile
            .enroll_native_api_key(&store, OTHER_KEY)
            .unwrap_err(),
        ProfileError::CredentialConflict
    );
    profile.enroll_native_api_key(&store, KEY).unwrap();
    assert_eq!(store.put_count(), 1);
}

#[test]
fn store_write_before_readiness_can_complete_without_replacement() {
    let fixture = Fixture::new();
    let store = FakeSecureStore::new();
    let mut profile = fixture.create();
    profile.enroll_native_api_key(&store, KEY).unwrap();
    fs::remove_file(fixture.path.join("api-ready")).unwrap();
    drop(profile);
    fixture.open().enroll_native_api_key(&store, KEY).unwrap();
    assert_eq!(store.put_count(), 1);
}

#[test]
fn missing_ready_credential_never_regenerated() {
    let fixture = Fixture::new();
    let store = FakeSecureStore::new();
    let mut profile = fixture.create();
    profile.enroll_native_api_key(&store, KEY).unwrap();
    let (service, account) = profile.labels().unwrap();
    store.delete(&service, &account).unwrap();
    assert_eq!(
        profile.load_api_credential(&store).unwrap_err(),
        ProfileError::CredentialUnavailable
    );
    assert_eq!(
        profile.enroll_native_api_key(&store, KEY).unwrap_err(),
        ProfileError::CredentialUnavailable
    );
    assert_eq!(store.put_count(), 1);
}

#[test]
fn occupied_or_replaced_store_entry_is_not_overwritten() {
    let fixture = Fixture::new();
    let store = FakeSecureStore::new();
    let mut profile = fixture.create();
    let (service, account) = profile.labels().unwrap();
    store.put(&service, &account, OTHER_KEY).unwrap();
    assert_eq!(
        profile.enroll_native_api_key(&store, KEY).unwrap_err(),
        ProfileError::CredentialConflict
    );
    assert!(!fixture.path.join("api-binding.json").exists());
    assert_eq!(store.get(&service, &account).unwrap(), OTHER_KEY);
    assert_eq!(store.put_count(), 1);
}

#[test]
fn corrupt_partial_or_oversized_state_never_replaced() {
    for name in ["api-binding.json", "api-ready"] {
        for bytes in [b"{".to_vec(), vec![b'x'; 4097]] {
            let fixture = Fixture::new();
            let store = FakeSecureStore::new();
            let mut profile = fixture.create();
            fs::write(fixture.path.join(name), &bytes).unwrap();
            assert_eq!(
                profile.enroll_native_api_key(&store, KEY).unwrap_err(),
                ProfileError::InvalidState
            );
            assert_eq!(fs::read(fixture.path.join(name)).unwrap(), bytes);
            assert_eq!(store.put_count(), 0);
        }
    }
}

#[test]
fn hard_linked_metadata_refused() {
    let fixture = Fixture::new();
    drop(fixture.create());
    fs::hard_link(
        fixture.path.join("profile.json"),
        fixture.parent.join("alias"),
    )
    .unwrap();
    assert_eq!(
        ManagedProfile::open(&fixture.path, SecureStoreEnvironment::Dev).unwrap_err(),
        ProfileError::UnsafeObject
    );
}

#[test]
fn invalid_id_short_secret_and_repo_path_refused() {
    let fixture = Fixture::new();
    assert_eq!(
        ManagedProfile::create(
            &fixture.parent.join("not-an-id"),
            SecureStoreEnvironment::Dev
        )
        .unwrap_err(),
        ProfileError::InvalidPath
    );
    let mut profile = fixture.create();
    let store = FakeSecureStore::new();
    assert_eq!(
        profile.enroll_native_api_key(&store, b"short").unwrap_err(),
        ProfileError::InvalidState
    );
    let repo_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("00000000000000000000000000000000");
    assert_eq!(
        ManagedProfile::create(&repo_path, SecureStoreEnvironment::Dev).unwrap_err(),
        ProfileError::InvalidPath
    );
    assert_eq!(store.put_count(), 0);
}

#[test]
fn actual_windows_credential_store_synthetic_profile_roundtrip_and_loss() {
    let fixture = Fixture::new();
    let store = WindowsCredentialStore::new();
    let mut profile = fixture.create();
    let (service, account) = profile.labels().unwrap();
    // Unpredictable exclusive profile ID, DEV namespace, never a real account.
    assert_eq!(
        store.get(&service, &account).unwrap_err(),
        SecureStoreError::Missing
    );
    struct Cleanup<'a>(&'a WindowsCredentialStore, String, String);
    impl Drop for Cleanup<'_> {
        fn drop(&mut self) {
            let _ = self.0.delete(&self.1, &self.2);
        }
    }
    let _cleanup = Cleanup(&store, service.clone(), account.clone());
    profile.enroll_native_api_key(&store, KEY).unwrap();
    drop(profile);
    let mut profile = fixture.open();
    profile.load_api_credential(&store).unwrap();
    assert_eq!(
        profile
            .enroll_native_api_key(&store, OTHER_KEY)
            .unwrap_err(),
        ProfileError::CredentialConflict
    );
    assert_eq!(store.get(&service, &account).unwrap(), KEY);
    store.delete(&service, &account).unwrap();
    assert_eq!(
        profile.enroll_native_api_key(&store, KEY).unwrap_err(),
        ProfileError::CredentialUnavailable
    );
    assert_eq!(
        store.get(&service, &account).unwrap_err(),
        SecureStoreError::Missing
    );
}

#[test]
fn worker_entry() {
    let Some(path) = std::env::var_os("CRYPTEX_TEST_PROFILE_CRASH_PATH") else {
        return;
    };
    let mut profile =
        ManagedProfile::create(Path::new(&path), SecureStoreEnvironment::Dev).unwrap();
    match std::env::var("CRYPTEX_TEST_PROFILE_CRASH_PHASE")
        .unwrap()
        .as_str()
    {
        "binding" => {
            let store = FakeSecureStore::new();
            store.fail_next_put_with(SecureStoreError::Platform {
                op: "synthetic",
                code: 5,
            });
            assert_eq!(
                profile.enroll_native_api_key(&store, KEY).unwrap_err(),
                ProfileError::CredentialUnavailable
            );
        }
        "ready" => {
            profile
                .enroll_native_api_key(&WindowsCredentialStore::new(), KEY)
                .unwrap();
        }
        _ => panic!("invalid synthetic phase"),
    }
    // Abrupt process termination without Rust Drop: kernel must release session
    // and directory pins. Not a power-loss/disk-flush durability simulation.
    std::process::exit(97);
}

fn run_crash_worker(fixture: &Fixture, phase: &str) {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "managed_profile::tests::worker_entry",
            "--nocapture",
        ])
        .env("CRYPTEX_TEST_PROFILE_CRASH_PATH", &fixture.path)
        .env("CRYPTEX_TEST_PROFILE_CRASH_PHASE", phase)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        // Worker errors contain typed profile/store diagnostics only. Retain
        // those on failure instead of reducing every panic to exit code 101.
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert_eq!(status.code(), Some(97));
            break;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("synthetic profile worker deadline");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn abrupt_exit_after_binding_reopens_and_completes_only_same_key() {
    let fixture = Fixture::new();
    run_crash_worker(&fixture, "binding");
    let mut profile = fixture.open();
    let store = FakeSecureStore::new();
    assert_eq!(
        profile
            .enroll_native_api_key(&store, OTHER_KEY)
            .unwrap_err(),
        ProfileError::CredentialConflict
    );
    profile.enroll_native_api_key(&store, KEY).unwrap();
    assert_eq!(store.put_count(), 1);
}

#[test]
fn abrupt_exit_ready_windows_store_reopens_without_replacement() {
    let fixture = Fixture::new();
    let store = WindowsCredentialStore::new();
    run_crash_worker(&fixture, "ready");
    let mut profile = fixture.open();
    let (_, account) = profile.labels().unwrap();
    struct Cleanup<'a>(&'a WindowsCredentialStore, String);
    impl Drop for Cleanup<'_> {
        fn drop(&mut self) {
            let _ = self.0.delete("Cryptex-Dev-ManagedSyncthing", &self.1);
        }
    }
    let _cleanup = Cleanup(&store, account.clone());
    profile.load_api_credential(&store).unwrap();
    assert_eq!(
        profile
            .enroll_native_api_key(&store, OTHER_KEY)
            .unwrap_err(),
        ProfileError::CredentialConflict
    );
    assert_eq!(
        store.get("Cryptex-Dev-ManagedSyncthing", &account).unwrap(),
        KEY
    );
    store
        .delete("Cryptex-Dev-ManagedSyncthing", &account)
        .unwrap();
    assert_eq!(
        store
            .get("Cryptex-Dev-ManagedSyncthing", &account)
            .unwrap_err(),
        SecureStoreError::Missing
    );
}

#[test]
fn replaced_ready_secret_is_rejected_without_repair() {
    let fixture = Fixture::new();
    let store = FakeSecureStore::new();
    let mut profile = fixture.create();
    profile.enroll_native_api_key(&store, KEY).unwrap();
    let (service, account) = profile.labels().unwrap();
    store.put(&service, &account, OTHER_KEY).unwrap();
    assert_eq!(
        profile.load_api_credential(&store).unwrap_err(),
        ProfileError::CredentialConflict
    );
    assert_eq!(
        profile.enroll_native_api_key(&store, KEY).unwrap_err(),
        ProfileError::CredentialConflict
    );
    assert_eq!(store.get(&service, &account).unwrap(), OTHER_KEY);
    assert_eq!(store.put_count(), 2);
}

#[test]
fn duplicate_binding_fields_are_corruption() {
    let fixture = Fixture::new();
    let store = FakeSecureStore::new();
    let mut profile = fixture.create();
    let digest = blake3::hash(KEY).to_hex().to_string();
    profile
        .write_new(
            "api-binding.json",
            format!("{{\"schema\":1,\"schema\":1,\"api_key_blake3\":\"{digest}\"}}").as_bytes(),
        )
        .unwrap();
    assert_eq!(
        profile.enroll_native_api_key(&store, KEY).unwrap_err(),
        ProfileError::InvalidState
    );
    assert_eq!(store.put_count(), 0);
}

#[test]
fn identical_folder_names_have_distinct_credential_namespaces() {
    let first = Fixture::new();
    let mut second = Fixture::new();
    second.path = second.parent.join(first.path.file_name().unwrap());
    let mut one = first.create();
    let mut two = second.create();
    let labels_one = one.labels().unwrap();
    let labels_two = two.labels().unwrap();
    assert_ne!(labels_one, labels_two);
    let store = FakeSecureStore::new();
    std::thread::scope(|scope| {
        let store_ref = &store;
        scope.spawn(move || {
            one.enroll_native_api_key(store_ref, KEY).unwrap();
        });
        scope.spawn(move || {
            two.enroll_native_api_key(store_ref, OTHER_KEY).unwrap();
        });
    });
    assert_eq!(store.len(), 2);
    assert_eq!(store.get(&labels_one.0, &labels_one.1).unwrap(), KEY);
    assert_eq!(store.get(&labels_two.0, &labels_two.1).unwrap(), OTHER_KEY);
    first.open().load_api_credential(&store).unwrap();
    second.open().load_api_credential(&store).unwrap();
}

#[test]
fn denied_store_read_never_becomes_missing_or_causes_write() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Denied(AtomicUsize);
    impl SecureStore for Denied {
        fn get(&self, _: &str, _: &str) -> Result<Vec<u8>, SecureStoreError> {
            Err(SecureStoreError::Platform {
                op: "synthetic-denied",
                code: 5,
            })
        }
        fn put(&self, _: &str, _: &str, _: &[u8]) -> Result<(), SecureStoreError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn delete(&self, _: &str, _: &str) -> Result<(), SecureStoreError> {
            unreachable!()
        }
    }
    let fixture = Fixture::new();
    let store = Denied(AtomicUsize::new(0));
    let mut profile = fixture.create();
    assert_eq!(
        profile.enroll_native_api_key(&store, KEY).unwrap_err(),
        ProfileError::CredentialUnavailable
    );
    assert_eq!(store.0.load(Ordering::SeqCst), 0);
    assert!(!fixture.path.join("api-binding.json").exists());
}

fn junction(link: &Path, target: &Path) {
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
    };
    use windows_sys::Win32::System::IO::DeviceIoControl;

    fs::create_dir(link).unwrap();
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(link)
        .unwrap();
    let substitute = format!(r"\??\{}", target.display())
        .encode_utf16()
        .collect::<Vec<_>>();
    let print = target.as_os_str().encode_wide().collect::<Vec<_>>();
    let data_length = 8 + (substitute.len() + 1 + print.len() + 1) * 2;
    assert!(data_length < 16376);
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&0xA0000003u32.to_le_bytes()); // IO_REPARSE_TAG_MOUNT_POINT
    bytes.extend_from_slice(&(data_length as u16).to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&((substitute.len() * 2) as u16).to_le_bytes());
    bytes.extend_from_slice(&(((substitute.len() + 1) * 2) as u16).to_le_bytes());
    bytes.extend_from_slice(&((print.len() * 2) as u16).to_le_bytes());
    for unit in substitute
        .into_iter()
        .chain(Some(0))
        .chain(print)
        .chain(Some(0))
    {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    let mut returned = 0;
    // Fresh test-owned empty directory -> test-owned target. Junction creation
    // uses the supported Windows FSCTL directly, with no shell or symlink privilege.
    assert_ne!(
        unsafe {
            DeviceIoControl(
                file.as_raw_handle(),
                0x000900A4,
                bytes.as_ptr().cast(),
                bytes.len() as u32,
                std::ptr::null_mut(),
                0,
                &mut returned,
                std::ptr::null_mut(),
            )
        },
        0,
        "{}",
        std::io::Error::last_os_error()
    );
}

#[test]
fn root_and_ancestor_junctions_refused_before_any_profile_write() {
    let fixture = Fixture::new();
    let outside = fixture.parent.join("outside-synthetic");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("sentinel"), b"untouched").unwrap();
    junction(&fixture.path, &outside);
    assert_eq!(
        ManagedProfile::open(&fixture.path, SecureStoreEnvironment::Dev).unwrap_err(),
        ProfileError::UnsafeObject
    );
    let descendant = fixture.path.join("00000000000000000000000000000000");
    assert_eq!(
        ManagedProfile::create(&descendant, SecureStoreEnvironment::Dev).unwrap_err(),
        ProfileError::UnsafeObject
    );
    assert_eq!(fs::read(outside.join("sentinel")).unwrap(), b"untouched");
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
    fs::remove_dir(&fixture.path).unwrap(); // remove only the junction, not its target
}
