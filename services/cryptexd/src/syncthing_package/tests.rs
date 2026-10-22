use super::*;
use std::fs;
use std::time::Duration;

#[test]
fn missing_wrong_and_repository_packages_refuse_without_launch() {
    assert_eq!(
        PinnedSyncthingPackage::open(Path::new("relative")).unwrap_err(),
        PackageError::InvalidPath
    );
    let root = crate::test_temp::exclusive_temp_dir("cryptex-m8-package");
    assert!(PinnedSyncthingPackage::open(&root).is_err());
    fs::write(root.join("syncthing.exe"), b"not executable").unwrap();
    assert_eq!(
        PinnedSyncthingPackage::open(&root).unwrap_err(),
        PackageError::SizeMismatch
    );
    fs::create_dir(root.join(".git")).unwrap();
    assert_eq!(
        PinnedSyncthingPackage::open(&root).unwrap_err(),
        PackageError::InvalidPath
    );
    fs::remove_dir(root.join(".git")).unwrap();
    fs::remove_file(root.join("syncthing.exe")).unwrap();
    fs::remove_dir(root).unwrap();
}

#[test]
#[ignore = "explicit retained synthetic native fixture required; no download"]
fn pinned_package_owned_generation_fixture() {
    use crate::managed_profile::ManagedProfile;
    use crate::secure_store::SecureStoreEnvironment;
    let fixture = PathBuf::from(
        std::env::var_os("CRYPTEX_M8_NATIVE_FIXTURE").expect("explicit fixture required"),
    );
    assert!(fixture.is_absolute());
    assert!(
        fixture
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("m8-owned-native-")
    );
    crate::paths::assert_outside_repository(&fixture).unwrap();
    let fixture_json: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.join("fixture.json")).unwrap()).unwrap();
    assert_eq!(fixture_json["scope"], "m8-owned-native-synthetic");
    let binary = PathBuf::from(fixture_json["binary"].as_str().unwrap());
    let root = crate::test_temp::exclusive_temp_dir_in(&fixture, "pinned-package");
    fs::copy(binary, root.join("syncthing.exe")).unwrap();
    for name in ["LICENSE.txt", "AUTHORS.txt"] {
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../third_party/syncthing/2.1.5")
                .join(name),
            root.join(name),
        )
        .unwrap();
    }
    let package = PinnedSyncthingPackage::open(&root).unwrap();
    let executable = root.join("syncthing.exe");
    assert!(
        fs::OpenOptions::new()
            .write(true)
            .open(&executable)
            .is_err()
    );
    assert!(fs::rename(&root, root.with_extension("renamed")).is_err());
    assert!(fs::remove_file(root.join("LICENSE.txt")).is_err());
    let mut random = [0; 16];
    getrandom::getrandom(&mut random).unwrap();
    let leaf: String = random.iter().map(|b| format!("{b:02x}")).collect();
    let profile_path = fixture.join(leaf);
    let profile = ManagedProfile::create(&profile_path, SecureStoreEnvironment::Dev).unwrap();
    let mut generated = package.spawn_generate(&profile_path).unwrap();
    let pid = generated.process().pid();
    drop(package); // running owner must retain every package pin independently
    assert!(
        fs::OpenOptions::new()
            .write(true)
            .open(&executable)
            .is_err()
    );
    let exit = generated.wait_for_exit(Duration::from_secs(15)).unwrap();
    assert!(exit.is_clean());
    assert!(
        fs::OpenOptions::new()
            .write(true)
            .open(&executable)
            .is_err()
    );
    for name in ["config.xml", "cert.pem", "key.pem"] {
        assert!(fs::metadata(profile_path.join(name)).unwrap().len() > 0);
    }
    // Generation has no server; do not start the default config or claim its
    // network isolation. No API enrollment or peer authorization occurs here.
    drop(generated);
    fs::OpenOptions::new()
        .write(true)
        .open(&executable)
        .unwrap();
    // Exercise abandonment with a retained OS observer, not PID/name killing.
    // Drop must finish owned-job cleanup before releasing the package pins.
    let mut random = [0; 16];
    getrandom::getrandom(&mut random).unwrap();
    let leaf: String = random.iter().map(|b| format!("{b:02x}")).collect();
    let abandoned_path = fixture.join(leaf);
    let abandoned_profile =
        ManagedProfile::create(&abandoned_path, SecureStoreEnvironment::Dev).unwrap();
    let package = PinnedSyncthingPackage::open(&root).unwrap();
    let abandoned = package.spawn_generate(&abandoned_path).unwrap();
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
    };
    let observer = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, abandoned.process().pid()) };
    assert!(!observer.is_null());
    drop(package);
    drop(abandoned);
    let observed = unsafe { WaitForSingleObject(observer, 0) };
    unsafe {
        CloseHandle(observer);
    }
    assert_eq!(observed, WAIT_OBJECT_0);
    fs::OpenOptions::new()
        .write(true)
        .open(&executable)
        .unwrap();
    drop(abandoned_profile);
    let notice_path = root.join("AUTHORS.txt");
    let notice = fs::read(&notice_path).unwrap();
    let mut altered = notice.clone();
    altered[0] ^= 1;
    fs::write(&notice_path, &altered).unwrap();
    assert_eq!(
        PinnedSyncthingPackage::open(&root).unwrap_err(),
        PackageError::DigestMismatch
    );
    fs::write(&notice_path, &notice).unwrap();
    fs::rename(&notice_path, root.join("withheld-notice")).unwrap();
    assert_eq!(
        PinnedSyncthingPackage::open(&root).unwrap_err(),
        PackageError::Io
    );
    fs::rename(root.join("withheld-notice"), &notice_path).unwrap();
    use std::io::{Read, Seek, SeekFrom, Write};
    let mut executable_file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&executable)
        .unwrap();
    let mut original = [0; 1];
    executable_file.read_exact(&mut original).unwrap();
    executable_file.seek(SeekFrom::Start(0)).unwrap();
    executable_file.write_all(&[original[0] ^ 1]).unwrap();
    drop(executable_file);
    assert_eq!(
        PinnedSyncthingPackage::open(&root).unwrap_err(),
        PackageError::DigestMismatch
    );
    let mut executable_file = fs::OpenOptions::new()
        .write(true)
        .open(&executable)
        .unwrap();
    executable_file.write_all(&original).unwrap();
    drop(executable_file);
    PinnedSyncthingPackage::open(&root).unwrap();
    drop(profile);
    // Retain synthetic profile/config and native keys as private proof; never
    // remove arbitrary trees and never print key/config bodies.
    let result = serde_json::to_vec(&serde_json::json!({
            "schema":1, "scope":"synthetic-package-owned-generation-only", "clean":exit.is_clean(), "pid":pid,
            "package_pin":WINDOWS_AMD64_SHA256, "config_server_not_started":true,
            "package_mutation_blocked_while_owned":true,
            "same_size_binary_and_notice_tampering_rejected":true,
            "missing_notice_rejected":true,
            "abandoned_owned_generation_cleanup_observed":true
        })).unwrap();
    fs::write(root.join("generation-result.json"), &result).unwrap();
    fs::write(fixture.join("native-pinned-package-result.json"), &result).unwrap();
}
