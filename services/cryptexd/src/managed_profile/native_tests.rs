use super::*;
use crate::secure_store::{FakeSecureStore, SecureStoreEnvironment, SecureStoreError};

const SECRET: &str = "SYNTHETIC_NATIVE_BOOTSTRAP_KEY_0123456789abcdef";
const ID: &str = "AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA";

pub(crate) fn synthetic_xml() -> String {
    format!(
        r#"<configuration version="52"><device id="{ID}" introducer="false"><autoAcceptFolders>false</autoAcceptFolders></device><gui enabled="true" tls="false"><address>127.0.0.1:8384</address><metricsWithoutAuth>false</metricsWithoutAuth><apikey>{SECRET}</apikey></gui><ldap></ldap><options><listenAddress>default</listenAddress><globalAnnounceEnabled>true</globalAnnounceEnabled><localAnnounceEnabled>true</localAnnounceEnabled><relaysEnabled>true</relaysEnabled><natEnabled>true</natEnabled><startBrowser>true</startBrowser><autoUpgradeIntervalH>12</autoUpgradeIntervalH><upgradeToPreReleases>false</upgradeToPreReleases><urAccepted>0</urAccepted><crashReportingEnabled>true</crashReportingEnabled><auditEnabled>false</auditEnabled></options><defaults><device><autoAcceptFolders>false</autoAcceptFolders></device></defaults></configuration>"#
    )
}

struct Fixture(std::path::PathBuf, std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = crate::test_temp::exclusive_temp_dir("cryptex-m8-native-profile");
        let mut bytes = [0; 16];
        getrandom::getrandom(&mut bytes).unwrap();
        let leaf: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let path = root.join(leaf);
        Self(root, path)
    }
    fn create(&self) -> ManagedProfile {
        ManagedProfile::create(&self.1, SecureStoreEnvironment::Dev).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let parent = std::fs::canonicalize(std::env::temp_dir()).unwrap();
        let target = std::fs::canonicalize(&self.0).unwrap();
        assert!(target.starts_with(&parent) && target != parent);
        assert!(
            target
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("cryptex-m8-native-profile-")
        );
        std::fs::remove_dir_all(target).unwrap();
    }
}

fn generate(home: &Arc<PrivateDirectory>) -> Result<(), NativeConfigError> {
    for (name, bytes) in [
        ("config.xml", synthetic_xml().into_bytes()),
        (
            "cert.pem",
            b"-----BEGIN CERTIFICATE-----\nSYNTHETIC_NOT_A_CERT\n".to_vec(),
        ),
        (
            "key.pem",
            b"-----BEGIN PRIVATE KEY-----\nSYNTHETIC_NOT_A_KEY\n".to_vec(),
        ),
    ] {
        ensure_same_or_new(home, name, &bytes)?;
    }
    Ok(())
}
fn ports() -> NativePorts {
    NativePorts::new(18384, 22001).unwrap()
}

#[test]
fn fresh_prepare_reopen_keeps_identity_secret_and_raw_config() {
    let fixture = Fixture::new();
    let mut profile = fixture.create();
    let store = FakeSecureStore::new();
    let prepared = profile
        .prepare_with_generator(&store, ports(), generate)
        .unwrap();
    prepared.validate(&store).unwrap();
    assert!(!format!("{prepared:?}").contains(SECRET));
    drop(prepared);
    assert_eq!(store.put_count(), 1);
    assert_eq!(
        std::fs::read(fixture.1.join("generation/config.xml")).unwrap(),
        synthetic_xml().as_bytes()
    );
    drop(profile);
    let mut profile = ManagedProfile::open(&fixture.1, SecureStoreEnvironment::Dev).unwrap();
    drop(
        profile
            .prepare_with_generator(&store, ports(), |_| {
                panic!("ready profile must not regenerate")
            })
            .unwrap(),
    );
    assert_eq!(store.put_count(), 1);
}

#[test]
fn failed_store_enrollment_resumes_only_the_same_staged_bytes() {
    let fixture = Fixture::new();
    let mut profile = fixture.create();
    let store = FakeSecureStore::new();
    store.fail_next_put_with(SecureStoreError::Platform {
        op: "synthetic-denied",
        code: 5,
    });
    assert_eq!(
        profile
            .prepare_with_generator(&store, ports(), generate)
            .unwrap_err(),
        NativeConfigError::Profile(ProfileError::CredentialUnavailable)
    );
    assert!(!fixture.1.join("native-ready.json").exists());
    let before = std::fs::read(fixture.1.join("native/config.xml")).unwrap();
    drop(
        profile
            .prepare_with_generator(&store, ports(), |_| {
                panic!("complete generation must resume")
            })
            .unwrap(),
    );
    assert_eq!(
        std::fs::read(fixture.1.join("native/config.xml")).unwrap(),
        before
    );
}

#[test]
fn worker_entry() {
    let Some(path) = std::env::var_os("CRYPTEX_TEST_NATIVE_PROFILE_CRASH") else {
        return;
    };
    let mut profile =
        ManagedProfile::create(std::path::Path::new(&path), SecureStoreEnvironment::Dev).unwrap();
    let store = FakeSecureStore::new();
    store.fail_next_put_with(SecureStoreError::Platform {
        op: "synthetic-failed-enroll",
        code: 5,
    });
    assert!(
        profile
            .prepare_with_generator(&store, ports(), generate)
            .is_err()
    );
    // Real abrupt process exit, no root-profile Drop. This is a session-lock
    // recovery probe, not a simulated power-loss/flush guarantee.
    std::process::exit(97);
}

#[test]
fn abrupt_exit_after_staging_releases_locks_and_resumes_identical_identity() {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    let fixture = Fixture::new();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "managed_profile::native::tests::worker_entry"])
        .env("CRYPTEX_TEST_NATIVE_PROFILE_CRASH", &fixture.1)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
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
            panic!("synthetic worker deadline");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let before = std::fs::read(fixture.1.join("native/key.pem")).unwrap();
    let mut profile = ManagedProfile::open(&fixture.1, SecureStoreEnvironment::Dev).unwrap();
    let store = FakeSecureStore::new();
    drop(
        profile
            .prepare_with_generator(&store, ports(), |_| panic!("must resume staged bytes"))
            .unwrap(),
    );
    assert_eq!(
        before,
        std::fs::read(fixture.1.join("native/key.pem")).unwrap()
    );
    assert_eq!(store.put_count(), 1);
}

#[test]
fn interrupted_generation_does_not_regenerate_or_delete_partial_identity() {
    let fixture = Fixture::new();
    let mut profile = fixture.create();
    let store = FakeSecureStore::new();
    assert_eq!(
        profile
            .prepare_with_generator(&store, ports(), |home| {
                ensure_same_or_new(home, "key.pem", b"partial synthetic identity")?;
                Err(NativeConfigError::GenerationFailed)
            })
            .unwrap_err(),
        NativeConfigError::GenerationFailed
    );
    assert_eq!(
        profile
            .prepare_with_generator(&store, ports(), |_| panic!("must not regenerate"))
            .unwrap_err(),
        NativeConfigError::IncompleteGeneration
    );
    assert_eq!(
        std::fs::read(fixture.1.join("generation/key.pem")).unwrap(),
        b"partial synthetic identity"
    );
    assert_eq!(store.put_count(), 0);
}

#[test]
fn files_alone_do_not_prove_a_failed_generation_completed() {
    let fixture = Fixture::new();
    let mut profile = fixture.create();
    let store = FakeSecureStore::new();
    assert!(
        profile
            .prepare_with_generator(&store, ports(), |home| {
                generate(home)?;
                Err(NativeConfigError::GenerationFailed)
            })
            .is_err()
    );
    assert_eq!(
        profile
            .prepare_with_generator(&store, ports(), |_| panic!("must not regenerate"))
            .unwrap_err(),
        NativeConfigError::IncompleteGeneration
    );
    assert_eq!(store.put_count(), 0);
}

#[test]
fn root_and_native_session_locks_survive_preparation_and_aliases_refuse() {
    let fixture = Fixture::new();
    let mut profile = fixture.create();
    let store = FakeSecureStore::new();
    let prepared = profile
        .prepare_with_generator(&store, ports(), generate)
        .unwrap();
    assert!(matches!(
        ManagedProfile::open(&fixture.1, SecureStoreEnvironment::Dev),
        Err(ProfileError::Busy)
    ));
    assert!(matches!(
        PrivateDirectory::acquire(&fixture.1.join("native"), false),
        Err(ProfileError::Busy)
    ));
    drop(prepared);
    std::fs::hard_link(
        fixture.1.join("native/config.xml"),
        fixture.0.join("synthetic-alias"),
    )
    .unwrap();
    assert_eq!(
        profile
            .prepare_with_generator(&store, ports(), |_| panic!("no regeneration"))
            .unwrap_err(),
        NativeConfigError::Profile(ProfileError::UnsafeObject)
    );
    assert_eq!(store.put_count(), 1);
}

#[test]
fn active_or_unresolved_session_cannot_launch_a_replacement() {
    let fixture = Fixture::new();
    let mut profile = fixture.create();
    let store = FakeSecureStore::new();
    let mut prepared = profile
        .prepare_with_generator(&store, ports(), generate)
        .unwrap();
    prepared.require_idle().unwrap();
    prepared.in_use = true;
    assert_eq!(
        prepared.require_idle().unwrap_err(),
        NativeConfigError::Profile(ProfileError::Busy)
    );
    prepared.cleanup_unresolved = true;
    assert_eq!(
        prepared.require_idle().unwrap_err(),
        NativeConfigError::UnresolvedCleanup
    );
}

#[test]
fn ready_missing_secret_changed_ports_config_and_identity_fail_closed() {
    for mutation in [
        "missing-secret",
        "ports",
        "network",
        "key",
        "receipt",
        "api-key",
        "completion",
    ] {
        let fixture = Fixture::new();
        let mut profile = fixture.create();
        let store = FakeSecureStore::new();
        drop(
            profile
                .prepare_with_generator(&store, ports(), generate)
                .unwrap(),
        );
        let mut requested_ports = ports();
        match mutation {
            "missing-secret" => {
                let (service, account) = profile.labels().unwrap();
                store.delete(&service, &account).unwrap();
            }
            "ports" => requested_ports = NativePorts::new(18385, 22002).unwrap(),
            "network" => {
                let file = fixture.1.join("native/config.xml");
                let s = std::fs::read_to_string(&file).unwrap();
                std::fs::write(
                    file,
                    s.replace(
                        "<natEnabled>false</natEnabled>",
                        "<natEnabled>true</natEnabled>",
                    ),
                )
                .unwrap();
            }
            "key" => {
                std::fs::write(fixture.1.join("native/key.pem"), b"changed synthetic key").unwrap();
            }
            "receipt" => {
                std::fs::write(
                    fixture.1.join("native-ready.json"),
                    b"{\"schema\":1,\"schema\":1}",
                )
                .unwrap();
            }
            "api-key" => {
                let file = fixture.1.join("native/config.xml");
                let text = std::fs::read_to_string(&file).unwrap();
                std::fs::write(
                    file,
                    text.replace(SECRET, "SYNTHETIC_CHANGED_NATIVE_API_KEY_0123456789abcdef"),
                )
                .unwrap();
            }
            "completion" => {
                std::fs::remove_file(fixture.1.join("native-generation-complete")).unwrap();
            }
            _ => unreachable!(),
        }
        assert!(
            profile
                .prepare_with_generator(&store, requested_ports, |_| panic!(
                    "ready must not regenerate"
                ))
                .is_err()
        );
        assert_eq!(store.put_count(), 1);
    }
}

#[test]
fn corruption_of_partial_output_never_gets_replaced() {
    let fixture = Fixture::new();
    let mut profile = fixture.create();
    let store = FakeSecureStore::new();
    store.fail_next_put_with(SecureStoreError::Platform {
        op: "synthetic",
        code: 5,
    });
    assert!(
        profile
            .prepare_with_generator(&store, ports(), generate)
            .is_err()
    );
    let path = fixture.1.join("native/config.xml");
    std::fs::write(&path, b"corrupt synthetic output").unwrap();
    assert!(
        profile
            .prepare_with_generator(&store, ports(), |_| panic!("must not regenerate"))
            .is_err()
    );
    assert_eq!(std::fs::read(path).unwrap(), b"corrupt synthetic output");
    assert_eq!(store.put_count(), 0);
}

#[test]
#[ignore = "explicit retained synthetic native package fixture required"]
fn real_pinned_profile_provision_reopen_and_collision_recovery() {
    use crate::secure_store::{SecureStore, WindowsCredentialStore};
    use crate::syncthing_owned_rest::{EndpointError, OwnedRestError, OwnedSyncthingRest};
    use crate::syncthing_rest::ApiCredential;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};
    let fixture = PathBuf::from(
        std::env::var_os("CRYPTEX_M8_NATIVE_FIXTURE").expect("explicit synthetic fixture required"),
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
    let specification: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.join("fixture.json")).unwrap()).unwrap();
    assert_eq!(specification["scope"], "m8-owned-native-synthetic");
    let package_root =
        crate::test_temp::exclusive_temp_dir_in(&fixture, "native-provision-package");
    fs::copy(
        specification["binary"].as_str().unwrap(),
        package_root.join("syncthing.exe"),
    )
    .unwrap();
    for name in ["LICENSE.txt", "AUTHORS.txt"] {
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../third_party/syncthing/2.1.5")
                .join(name),
            package_root.join(name),
        )
        .unwrap();
    }
    let package = PinnedSyncthingPackage::open(&package_root).unwrap();
    let mut bytes = [0; 16];
    getrandom::getrandom(&mut bytes).unwrap();
    let leaf: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let profile_path = fixture.join(leaf);
    let mut profile = ManagedProfile::create(&profile_path, SecureStoreEnvironment::Dev).unwrap();
    let store = WindowsCredentialStore::new();
    let (service, account) = profile.labels().unwrap();
    struct Cleanup<'a>(&'a WindowsCredentialStore, String, String);
    impl Drop for Cleanup<'_> {
        fn drop(&mut self) {
            let _ = self.0.delete(&self.1, &self.2);
        }
    }
    let _cleanup = Cleanup(&store, service.clone(), account.clone());
    let port = || {
        let l = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        l.local_addr().unwrap().port()
    };
    let api = port();
    let mut sync = port();
    while api == sync {
        sync = port();
    }
    let selected = NativePorts::new(api, sync).unwrap();
    let mut pids = Vec::new();
    let mut auth_rejections = Vec::new();
    let mut identity = None;
    let mut collision_outcome = None;
    for run in 0..3 {
        let mut prepared = profile.prepare_native(&package, &store, selected).unwrap();
        if let Some(id) = &identity {
            assert_eq!(id, &prepared.receipt.native_id);
        }
        identity = Some(prepared.receipt.native_id.clone());
        if run == 2 {
            use std::io::Read;
            let foreign = std::net::TcpListener::bind(selected.api_endpoint()).unwrap();
            foreign.set_nonblocking(true).unwrap();
            match prepared.start(&package, &store) {
                Err(NativeConfigError::StartExited) => {
                    collision_outcome = Some("owned_root_exited")
                }
                Err(NativeConfigError::Rest(OwnedRestError::Endpoint(
                    EndpointError::PeerNotOwned | EndpointError::OwnerExited,
                ))) => collision_outcome = Some("foreign_endpoint_refused"),
                Err(error) => panic!("unexpected collision result: {error}"),
                Ok(session) => {
                    drop(session);
                    panic!("foreign occupied API port became ready");
                }
            }
            loop {
                match foreign.accept() {
                    Ok((mut stream, _)) => {
                        stream
                            .set_read_timeout(Some(Duration::from_millis(100)))
                            .unwrap();
                        let mut bytes = [0; 1];
                        match stream.read(&mut bytes) {
                            Ok(0) => {}
                            Err(error)
                                if matches!(
                                    error.kind(),
                                    std::io::ErrorKind::WouldBlock
                                        | std::io::ErrorKind::TimedOut
                                        | std::io::ErrorKind::ConnectionReset
                                        | std::io::ErrorKind::ConnectionAborted
                                ) => {}
                            _ => panic!("application byte sent to foreign API listener"),
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(error) => panic!("synthetic foreign listener: {error}"),
                }
            }
            drop(foreign);
            assert!(!prepared.in_use && !prepared.cleanup_unresolved);
            // A new explicit start after removing the collision must preserve
            // identity/key. No automatic retry/adoption was allowed above.
        }
        let config_key = native_xml::inspect(
            &read_required(&prepared.home, "config.xml", 256 * 1024).unwrap(),
            selected,
            false,
        )
        .unwrap()
        .key;
        let mut running = prepared.start(&package, &store).unwrap();
        let deadline = Instant::now() + Duration::from_secs(12);
        loop {
            assert!(
                running.try_exit().unwrap().is_none(),
                "native root exited before readiness"
            );
            match running.observe(&store) {
                Ok(_) => break,
                Err(NativeConfigError::Rest(
                    OwnedRestError::Endpoint(EndpointError::Unavailable) | OwnedRestError::Deadline,
                )) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
                Err(error) => panic!("native readiness rejected: {error}"),
            }
        }
        let live: serde_json::Value =
            serde_json::from_slice(&running.rest(&store).unwrap().test_read_config().unwrap())
                .unwrap();
        assert!(live["folders"].as_array().is_none_or(|a| a.is_empty()));
        assert_eq!(live["devices"].as_array().unwrap().len(), 1);
        for field in [
            "globalAnnounceEnabled",
            "localAnnounceEnabled",
            "relaysEnabled",
            "natEnabled",
            "startBrowser",
            "crashReportingEnabled",
            "auditEnabled",
        ] {
            assert_eq!(
                live["options"][field].as_bool(),
                Some(false),
                "isolation field {field}"
            );
        }
        assert_eq!(live["options"]["autoUpgradeIntervalH"].as_i64(), Some(0));
        assert_eq!(
            live["options"]["listenAddresses"],
            serde_json::json!([format!("tcp://127.0.0.1:{}", selected.sync)])
        );
        assert_eq!(live["options"]["urAccepted"].as_i64(), Some(-1));
        assert_eq!(
            live["gui"]["address"].as_str(),
            Some(selected.api_endpoint().to_string().as_str())
        );
        assert_eq!(live["gui"]["insecureAdminAccess"].as_bool(), Some(false));
        assert_eq!(live["gui"]["metricsWithoutAuth"].as_bool(), Some(false));
        let rejection = OwnedSyncthingRest::new(
            running.owned.process(),
            selected.api_endpoint(),
            identity.as_ref().unwrap(),
            ApiCredential::from_bytes(b"wrong-synthetic-key").unwrap(),
        )
        .unwrap()
        .observe()
        .unwrap_err();
        match rejection {
            OwnedRestError::Unauthorized(code @ (401 | 403)) => auth_rejections.push(code),
            other => panic!("native auth rejection expected: {other}"),
        }
        pids.push(running.pid());
        assert!(running.stop(&store).unwrap().is_clean());
        drop(running);
        assert!(!prepared.in_use && !prepared.cleanup_unresolved);
        let log = read_required(&prepared.home, "native.log", 1024 * 1024).unwrap();
        assert!(
            !log.windows(config_key.len())
                .any(|window| window == config_key)
        );
        drop(prepared);
        // Reopen root/profile/private native home between real starts.
        drop(profile);
        profile = ManagedProfile::open(&profile_path, SecureStoreEnvironment::Dev).unwrap();
    }
    let mut controller_pids = Vec::new();
    for _ in 0..2 {
        use super::super::controller::{Control, Outcome, Phase};
        let mut prepared = profile.prepare_native(&package, &store, selected).unwrap();
        let result = prepared.supervise(&package, &store, |status| {
            if status.phase == Phase::LocalReady {
                controller_pids.push(status.pid.unwrap());
                Control::Stop
            } else {
                Control::Continue
            }
        });
        assert_eq!(result, Ok(Outcome::CleanShutdown));
        assert!(!prepared.in_use && !prepared.cleanup_unresolved);
    }
    assert_eq!(controller_pids.len(), 2);
    use super::super::controller::{Control, ControllerError, Outcome, Phase};
    let mut restart_pids = Vec::new();
    let mut cancellation_pids = Vec::new();
    let mut exhausted_pids = Vec::new();
    let restart_started = Instant::now();
    {
        let mut prepared = profile.prepare_native(&package, &store, selected).unwrap();
        let result = prepared.supervise_with_test_crashes(&package, &store, 1, |status| {
            if status.phase == Phase::LocalReady {
                restart_pids.push(status.pid.unwrap());
                if status.attempt == 2 {
                    return Control::Stop;
                }
            }
            Control::Continue
        });
        assert_eq!(result, Ok(Outcome::CleanShutdown));
        assert_eq!(restart_pids.len(), 2);
        assert_ne!(restart_pids[0], restart_pids[1]);
        assert!(!prepared.in_use && !prepared.cleanup_unresolved);
    }
    let restart_elapsed = restart_started.elapsed();
    assert!(restart_elapsed >= Duration::from_secs(1));
    {
        let mut prepared = profile.prepare_native(&package, &store, selected).unwrap();
        let result = prepared.supervise_with_test_crashes(&package, &store, 1, |status| {
            if status.phase == Phase::LocalReady {
                cancellation_pids.push(status.pid.unwrap());
            }
            if status.phase == Phase::RestartWaiting {
                Control::Stop
            } else {
                Control::Continue
            }
        });
        assert_eq!(result, Ok(Outcome::Cancelled));
        assert_eq!(cancellation_pids.len(), 1);
        assert!(!prepared.in_use && !prepared.cleanup_unresolved);
    }
    let exhaustion_started = Instant::now();
    {
        let mut prepared = profile.prepare_native(&package, &store, selected).unwrap();
        let mut terminal = None;
        let result = prepared.supervise_with_test_crashes(&package, &store, 4, |status| {
            if status.phase == Phase::LocalReady {
                exhausted_pids.push(status.pid.unwrap());
            }
            if status.phase == Phase::Failed {
                terminal = Some(status);
            }
            Control::Continue
        });
        assert_eq!(result, Err(ControllerError::RestartLimit));
        assert_eq!(exhausted_pids.len(), 4);
        assert_eq!(terminal.unwrap().attempt, 4);
        assert_eq!(terminal.unwrap().pid, None);
        assert!(!prepared.in_use && !prepared.cleanup_unresolved);
    }
    let exhaustion_elapsed = exhaustion_started.elapsed();
    assert!(exhaustion_elapsed >= Duration::from_secs(7));
    // Crash exhaustion never replaces the completed native identity or key.
    let prepared = profile.prepare_native(&package, &store, selected).unwrap();
    assert_eq!(identity.as_ref().unwrap(), &prepared.receipt.native_id);
    drop(prepared);
    drop(profile);
    use super::super::worker::{ManagedTransportWorker, WorkerError, WorkerState};
    let mut worker = ManagedTransportWorker::spawn_existing(
        profile_path.clone(),
        package_root.to_path_buf(),
        SecureStoreEnvironment::Dev,
        selected,
    )
    .unwrap();
    let worker_status = worker.status_reader();
    let deadline = Instant::now() + Duration::from_secs(18);
    let worker_pid = loop {
        let snapshot = worker.snapshot().unwrap();
        assert_ne!(snapshot.state, WorkerState::Failed, "{snapshot:?}");
        if let Some(status) = snapshot.controller
            && status.phase == Phase::LocalReady
        {
            assert_eq!(
                worker_status.try_transport_device_id().unwrap(),
                identity.as_deref().expect("prepared adapter identity")
            );
            break status.pid.unwrap();
        }
        assert!(Instant::now() < deadline, "worker readiness deadline");
        std::thread::sleep(Duration::from_millis(10));
    };
    // A second worker must refuse the already owned private profile, without
    // requesting cancellation before its actual opening error is observed.
    let mut duplicate = ManagedTransportWorker::spawn_existing(
        profile_path.clone(),
        package_root.to_path_buf(),
        SecureStoreEnvironment::Dev,
        selected,
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while duplicate.snapshot().unwrap().state != WorkerState::Failed {
        assert!(
            Instant::now() < deadline,
            "duplicate ownership refusal deadline"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        duplicate.shutdown(Duration::from_secs(1)),
        Err(WorkerError::Profile(ProfileError::Busy))
    );
    assert_eq!(
        worker.shutdown(Duration::from_secs(40)),
        Ok(Outcome::CleanShutdown)
    );
    assert_eq!(
        worker.shutdown(Duration::from_secs(1)),
        Ok(Outcome::CleanShutdown)
    );
    assert_eq!(worker.snapshot().unwrap().state, WorkerState::Finished);
    assert_eq!(
        worker_status.try_transport_device_id(),
        Err(WorkerError::StatusUnavailable)
    );
    drop(worker);
    // Exercise the actual daemon startup/stop call sites while no profile owner
    // remains. CLI parent intentionally receives health-only IPC, not Desktop.
    let daemon_binary = PathBuf::from(
        std::env::var_os("CRYPTEX_M8_DAEMON_BINARY")
            .expect("explicit freshly built daemon required"),
    );
    assert!(daemon_binary.is_absolute() && daemon_binary.is_file());
    crate::paths::assert_outside_repository(&daemon_binary).unwrap();
    let daemon_runtime =
        crate::test_temp::exclusive_temp_dir_in(&fixture, "managed-daemon-runtime");
    let ready = daemon_runtime.join("ready");
    let stop = daemon_runtime.join("stop");
    struct ChildGuard(std::process::Child);
    impl std::ops::Deref for ChildGuard {
        type Target = std::process::Child;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
    impl std::ops::DerefMut for ChildGuard {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }
    impl Drop for ChildGuard {
        fn drop(&mut self) {
            if !matches!(self.0.try_wait(), Ok(Some(_))) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
    }
    let mut daemon = ChildGuard(
        std::process::Command::new(&daemon_binary)
            .args(["lifecycle", "hold", "--runtime-root"])
            .arg(daemon_runtime.as_path())
            .arg("--ready-file")
            .arg(&ready)
            .arg("--stop-file")
            .arg(&stop)
            .arg("--managed-profile")
            .arg(&profile_path)
            .arg("--managed-package")
            .arg(package_root.as_path())
            .arg("--managed-api-port")
            .arg(api.to_string())
            .arg("--managed-sync-port")
            .arg(sync.to_string())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    let daemon_pid = daemon.id();
    let deadline = Instant::now() + Duration::from_secs(25);
    while !ready.exists() {
        assert!(
            daemon.try_wait().unwrap().is_none(),
            "daemon exited before managed readiness"
        );
        if Instant::now() >= deadline {
            daemon.kill().unwrap();
            daemon.wait().unwrap();
            panic!("managed daemon readiness deadline");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(matches!(
        ManagedProfile::open(&profile_path, SecureStoreEnvironment::Dev),
        Err(ProfileError::Busy)
    ));
    fs::write(&stop, b"stop\n").unwrap();
    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        if let Some(exit) = daemon.try_wait().unwrap() {
            assert!(exit.success());
            break;
        }
        if Instant::now() >= deadline {
            daemon.kill().unwrap();
            daemon.wait().unwrap();
            panic!("managed daemon shutdown deadline");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let lifecycle = fs::read_to_string(daemon_runtime.join("lifecycle.status")).unwrap();
    assert!(lifecycle.contains("note=clean shutdown released instance lock"));
    assert!(crate::instance::try_acquire_instance(&daemon_runtime).is_ok());
    profile = ManagedProfile::open(&profile_path, SecureStoreEnvironment::Dev).unwrap();
    // Reopen-only refuses a missing receipt and never rewrites it or generates.
    let receipt_path = profile.directory.path().join("native-ready.json");
    let receipt_bytes = fs::read(&receipt_path).unwrap();
    fs::remove_file(&receipt_path).unwrap();
    assert!(matches!(
        profile.reopen_native(&package, &store, selected),
        Err(NativeConfigError::InvalidState)
    ));
    assert!(!receipt_path.exists());
    profile
        .write_new("native-ready.json", &receipt_bytes)
        .unwrap();
    store.delete(&service, &account).unwrap();
    assert!(matches!(
        store.get(&service, &account),
        Err(SecureStoreError::Missing)
    ));
    assert!(profile.prepare_native(&package, &store, selected).is_err());
    fs::write(
        fixture.join("native-provision-result.json"),
        serde_json::to_vec(&serde_json::json!({
            "schema":1,"scope":"synthetic-pinned-provision-controller-faults-only","pids":pids,"auth_rejections":auth_rejections,
                "direct_and_requested_stop_cycles_clean":true,"windows_credential_manager":true,"synthetic_credential_deleted":true,
            "identity_preserved":true,"live_loopback_isolation_checked":true,
                "raw_generation_home_never_served":true,
                "api_port_collision_refused_before_api_bytes":true,
                "explicit_retry_after_collision_clean":true,
                "collision_outcome":collision_outcome,
                "controller_clean_start_stop_pids":controller_pids,
                "controller_crash_then_clean_stop_pids":restart_pids,
                "controller_backoff_cancel_pids":cancellation_pids,
                "controller_restart_exhaustion_pids":exhausted_pids,
                "controller_restart_elapsed_ms":restart_elapsed.as_millis(),
                "controller_exhaustion_elapsed_ms":exhaustion_elapsed.as_millis(),
                "controller_identity_preserved_after_faults":true,
                "worker_clean_stop_pid":worker_pid,
                "worker_duplicate_profile_refused":true,
                "worker_missing_receipt_not_recreated":true,
                "daemon_managed_start_stop_pid":daemon_pid,
                "daemon_instance_lock_released_after_worker_join":true,
                "fault_scope":"test-only-owned-job-termination-after-healthy-observation"
        }))
        .unwrap(),
    )
    .unwrap();
}
