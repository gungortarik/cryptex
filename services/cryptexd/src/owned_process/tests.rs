use super::*;

use std::fs;
use std::process::{Command, Stdio};

use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, WaitForSingleObject,
};

const WORKER: &str = "owned_process::tests::worker_entry";
const TEST_TIMEOUT: Duration = Duration::from_secs(10);

#[path = "tests_http.rs"]
mod http;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        Self(crate::test_temp::exclusive_temp_dir(
            "cryptex-m8-owned-process",
        ))
    }

    fn spec(&self, mode: &str) -> LaunchSpec {
        fixture_spec(&self.0, mode)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let parent = fs::canonicalize(std::env::temp_dir()).unwrap();
        let target = fs::canonicalize(&self.0).unwrap();
        assert!(target.starts_with(&parent) && target != parent);
        assert!(
            target
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("cryptex-m8-owned-process-")
        );
        // Windows may briefly retain a terminated child's directory/image
        // handles after job membership reaches zero. Only fixture cleanup may
        // retry sharing violations; all process/ownership assertions above stay
        // unchanged, and a persistent lock still fails the test.
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match fs::remove_dir_all(&target) {
                Ok(()) => break,
                Err(error)
                    if matches!(error.raw_os_error(), Some(32 | 33))
                        && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Err(error) => panic!("exclusive owned fixture cleanup failed: {error}"),
            }
        }
    }
}

fn fixture_spec(root: &Path, mode: &str) -> LaunchSpec {
    let mut environment = minimal_system_environment();
    environment.push((
        "CRYPTEX_TEST_OWNED_ROOT".into(),
        root.as_os_str().to_owned(),
    ));
    environment.push(("CRYPTEX_TEST_OWNED_MODE".into(), mode.into()));
    LaunchSpec::validate(
        &std::env::current_exe().unwrap(),
        root,
        vec!["--exact".into(), WORKER.into(), "--nocapture".into()],
        environment,
    )
    .unwrap()
}

fn wait_file(path: &Path) {
    let deadline = Instant::now() + TEST_TIMEOUT;
    while !path.is_file() {
        assert!(
            Instant::now() < deadline,
            "synthetic worker did not become ready"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn publish(root: &Path, name: &str, bytes: &[u8]) {
    // Presence must not race a partially written PID/ready file under the full
    // parallel suite. This is fixture publication, not a durability claim.
    let staging = root.join(format!("{name}.tmp"));
    fs::write(&staging, bytes).unwrap();
    fs::rename(staging, root.join(name)).unwrap();
}

fn process_is_running(pid: u32) -> bool {
    let handle = unsafe {
        OpenProcess(
            PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
            0,
            pid,
        )
    };
    if handle.is_null() {
        return false;
    }
    let result = unsafe { WaitForSingleObject(handle, 0) };
    unsafe {
        CloseHandle(handle);
    }
    match result {
        WAIT_TIMEOUT => true,
        WAIT_OBJECT_0 => false,
        _ => panic!("synthetic process observation failed"),
    }
}

fn wait_stopped(pid: u32) {
    let deadline = Instant::now() + TEST_TIMEOUT;
    while process_is_running(pid) {
        assert!(
            Instant::now() < deadline,
            "owned synthetic process remained alive"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn worker_entry() {
    let Some(root) = std::env::var_os("CRYPTEX_TEST_OWNED_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    let mode = std::env::var("CRYPTEX_TEST_OWNED_MODE").unwrap();
    publish(&root, "ready", std::process::id().to_string().as_bytes());

    if mode == "args" {
        let arguments: Vec<_> = std::env::args_os()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        fs::write(
            root.join("args.json"),
            serde_json::to_vec(&arguments).unwrap(),
        )
        .unwrap();
        let names: Vec<_> = std::env::vars_os()
            .map(|(name, _)| name.to_string_lossy().into_owned())
            .collect();
        fs::write(
            root.join("environment-names.json"),
            serde_json::to_vec(&names).unwrap(),
        )
        .unwrap();
        return;
    }
    if let Some(code) = mode.strip_prefix("exit-")
        && let Ok(code) = code.parse::<i32>()
    {
        std::process::exit(code);
    }
    if mode == "http" {
        http::http_worker(&root);
        return;
    }
    if mode == "http-tree" || mode == "http-root-exit" {
        use std::os::windows::process::CommandExt;
        let child_root = root.join("http-child");
        fs::create_dir(&child_root).unwrap();
        fs::copy(
            root.join("http-plan.json"),
            child_root.join("http-plan.json"),
        )
        .unwrap();
        let spec = fixture_spec(&child_root, "http");
        let mut child = Command::new(&spec.program)
            .args(&spec.arguments)
            .current_dir(&spec.working_directory)
            .env_clear()
            .envs(spec.environment)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW)
            .spawn()
            .unwrap();
        wait_file(&child_root.join("server.addr"));
        publish(
            &root,
            "server.addr",
            &fs::read(child_root.join("server.addr")).unwrap(),
        );
        if mode == "http-root-exit" {
            std::process::exit(0);
        }
        assert!(child.wait().unwrap().success());
        return;
    }
    if mode == "owner-crash" || mode == "owner-hold" {
        let nested_root = root.join("nested-owner");
        fs::create_dir(&nested_root).unwrap();
        let owned = OwnedProcess::spawn(&fixture_spec(&nested_root, "hold")).unwrap();
        wait_file(&nested_root.join("ready"));
        publish(&root, "descendant.pid", owned.pid().to_string().as_bytes());
        // Deliberately bypass Rust destructors. Windows must close the last
        // non-inherited job handle and terminate the nested tree itself.
        if mode == "owner-crash" {
            std::process::exit(97);
        }
        wait_for_stop(&root);
        return;
    }
    if mode == "tree" || mode == "exit-with-tree" {
        let descendant_root = root.join("descendant");
        fs::create_dir(&descendant_root).unwrap();
        let spec = fixture_spec(&descendant_root, "hold");
        use std::os::windows::process::CommandExt;
        let mut child = Command::new(&spec.program)
            .args(&spec.arguments)
            .current_dir(&spec.working_directory)
            .env_clear()
            .envs(spec.environment)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW)
            .spawn()
            .unwrap();
        wait_file(&descendant_root.join("ready"));
        publish(&root, "descendant.pid", child.id().to_string().as_bytes());
        if mode == "exit-with-tree" {
            // This scenario intentionally exits while its descendant remains.
            // The owning job outside this worker is responsible for cleanup.
            std::process::exit(0);
        }
        wait_for_stop(&root);
        fs::write(descendant_root.join("stop"), b"stop").unwrap();
        assert!(child.wait().unwrap().success());
        return;
    }
    assert_eq!(mode, "hold");
    wait_for_stop(&root);
}

fn wait_for_stop(root: &Path) {
    // Every test worker has a self-exit deadline even if its parent fails.
    let deadline = Instant::now() + Duration::from_secs(20);
    while !root.join("stop").is_file() {
        if Instant::now() >= deadline {
            std::process::exit(98);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn clean_exit_is_observed_and_repeat_observation_is_stable() {
    let fixture = Fixture::new();
    let mut owned = OwnedProcess::spawn(&fixture.spec("exit-0")).unwrap();
    let exit = owned.wait_for_exit(TEST_TIMEOUT).unwrap();
    assert!(exit.is_clean());
    assert_eq!(owned.try_exit().unwrap(), Some(exit));
    assert!(!process_is_running(owned.pid()));
}

#[test]
fn nonzero_exit_is_not_clean_and_259_is_not_mistaken_for_still_active() {
    for code in [7, 97, 259] {
        let fixture = Fixture::new();
        let mut owned = OwnedProcess::spawn(&fixture.spec(&format!("exit-{code}"))).unwrap();
        let exit = owned.wait_for_exit(TEST_TIMEOUT).unwrap();
        assert_eq!(exit.exit_code, Some(code));
        assert!(!exit.is_clean());
        assert!(!exit.forced);
    }
}

#[test]
fn timeout_does_not_kill_or_claim_a_clean_shutdown() {
    let fixture = Fixture::new();
    let mut owned = OwnedProcess::spawn(&fixture.spec("hold")).unwrap();
    wait_file(&fixture.0.join("ready"));
    assert_eq!(
        owned.wait_for_exit(Duration::ZERO),
        Err(OwnedProcessError::Deadline)
    );
    assert!(process_is_running(owned.pid()));
    fs::write(fixture.0.join("stop"), b"stop").unwrap();
    assert!(owned.wait_for_exit(TEST_TIMEOUT).unwrap().is_clean());
}

#[test]
fn forced_cleanup_is_explicit_even_if_the_root_already_exited() {
    let fixture = Fixture::new();
    let mut owned = OwnedProcess::spawn(&fixture.spec("exit-0")).unwrap();
    assert!(owned.wait_for_exit(TEST_TIMEOUT).unwrap().is_clean());
    let forced = owned.force_cleanup(TEST_TIMEOUT).unwrap();
    assert!(forced.forced);
    assert!(!forced.is_clean());
}

#[test]
fn forced_cleanup_kills_only_the_owned_tree_and_not_an_unrelated_job() {
    let first = Fixture::new();
    let second = Fixture::new();
    let mut owned = OwnedProcess::spawn(&first.spec("tree")).unwrap();
    let mut unrelated = OwnedProcess::spawn(&second.spec("hold")).unwrap();
    wait_file(&first.0.join("descendant.pid"));
    wait_file(&second.0.join("ready"));
    let descendant: u32 = fs::read_to_string(first.0.join("descendant.pid"))
        .unwrap()
        .parse()
        .unwrap();
    assert!(process_is_running(descendant));
    let exit = owned.force_cleanup(TEST_TIMEOUT).unwrap();
    assert!(exit.forced && !exit.is_clean());
    wait_stopped(descendant);
    assert!(process_is_running(unrelated.pid()));
    fs::write(second.0.join("stop"), b"stop").unwrap();
    assert!(unrelated.wait_for_exit(TEST_TIMEOUT).unwrap().is_clean());
}

#[test]
fn drop_closes_the_job_and_stops_root_and_descendants() {
    let fixture = Fixture::new();
    let owned = OwnedProcess::spawn(&fixture.spec("tree")).unwrap();
    wait_file(&fixture.0.join("descendant.pid"));
    let root_pid = owned.pid();
    let descendant: u32 = fs::read_to_string(fixture.0.join("descendant.pid"))
        .unwrap()
        .parse()
        .unwrap();
    drop(owned);
    wait_stopped(root_pid);
    wait_stopped(descendant);
}

#[test]
fn exited_root_with_live_descendant_is_not_reported_as_clean() {
    let fixture = Fixture::new();
    let mut owned = OwnedProcess::spawn(&fixture.spec("exit-with-tree")).unwrap();
    wait_file(&fixture.0.join("descendant.pid"));
    wait_stopped(owned.pid());
    assert_eq!(owned.try_exit().unwrap(), None);
    assert_eq!(
        owned.wait_for_exit(Duration::ZERO),
        Err(OwnedProcessError::Deadline)
    );
    let descendant: u32 = fs::read_to_string(fixture.0.join("descendant.pid"))
        .unwrap()
        .parse()
        .unwrap();
    assert!(owned.force_cleanup(TEST_TIMEOUT).unwrap().forced);
    wait_stopped(descendant);
}

#[test]
fn abrupt_owner_exit_without_destructors_still_stops_its_nested_job() {
    let fixture = Fixture::new();
    let mut owner = OwnedProcess::spawn(&fixture.spec("owner-crash")).unwrap();
    let exit = owner.wait_for_exit(TEST_TIMEOUT).unwrap();
    assert_eq!(exit.exit_code, Some(97));
    assert!(!exit.is_clean());
    let child: u32 = fs::read_to_string(fixture.0.join("descendant.pid"))
        .unwrap()
        .parse()
        .unwrap();
    assert!(!process_is_running(child));
}

#[test]
fn externally_terminated_owner_still_closes_its_non_inherited_nested_job() {
    let fixture = Fixture::new();
    let mut owner = OwnedProcess::spawn(&fixture.spec("owner-hold")).unwrap();
    wait_file(&fixture.0.join("descendant.pid"));
    let child: u32 = fs::read_to_string(fixture.0.join("descendant.pid"))
        .unwrap()
        .parse()
        .unwrap();
    assert!(process_is_running(child));
    owner.native.terminate_root_only_for_test().unwrap();
    let exit = owner.wait_for_exit(TEST_TIMEOUT).unwrap();
    assert_eq!(exit.exit_code, Some(97));
    assert!(!exit.is_clean());
    assert!(!process_is_running(child));
}

#[test]
fn command_line_preserves_spaces_quotes_backslashes_unicode_and_empty_arguments() {
    let fixture = Fixture::new();
    let mut spec = fixture.spec("args");
    let samples = [
        "",
        "two words",
        "quote\"here",
        "C:\\tail\\",
        "back\\\"slash",
        "🧠",
        "semi;$(literal)",
    ];
    spec.arguments.push("--".into());
    spec.arguments.extend(samples.iter().map(OsString::from));
    let mut owned = OwnedProcess::spawn(&spec).unwrap();
    assert!(owned.wait_for_exit(TEST_TIMEOUT).unwrap().is_clean());
    let arguments: Vec<String> =
        serde_json::from_slice(&fs::read(fixture.0.join("args.json")).unwrap()).unwrap();
    assert_eq!(&arguments[arguments.len() - samples.len()..], samples);
    let environment: Vec<String> =
        serde_json::from_slice(&fs::read(fixture.0.join("environment-names.json")).unwrap())
            .unwrap();
    assert!(!environment.iter().any(|name| matches!(
        name.to_ascii_uppercase().as_str(),
        "PATH" | "HTTP_PROXY" | "HTTPS_PROXY" | "ALL_PROXY" | "CARGO_MANIFEST_DIR" | "RUST_LOG"
    )));
}

#[test]
fn launch_errors_are_typed_and_do_not_echo_paths_or_arguments() {
    let fixture = Fixture::new();
    let mut spec = fixture.spec("hold");
    spec.program = fixture.0.join("PRIVATE_MISSING_EXECUTABLE.exe");
    let error = OwnedProcess::spawn(&spec).unwrap_err();
    assert!(matches!(
        error,
        OwnedProcessError::Platform {
            operation: "process_create",
            ..
        }
    ));
    assert!(!format!("{error} {error:?}").contains("PRIVATE_MISSING_EXECUTABLE"));
    assert!(!format!("{spec:?}").contains(&fixture.0.to_string_lossy().to_string()));
}

#[test]
fn no_shell_extension_relative_path_or_nul_argument_is_executed() {
    let fixture = Fixture::new();
    let mut spec = fixture.spec("hold");
    spec.program = fixture.0.join("launch.cmd");
    assert_eq!(
        OwnedProcess::spawn(&spec).unwrap_err(),
        OwnedProcessError::InvalidLaunch
    );
    let mut spec = fixture.spec("hold");
    spec.arguments.push("PRIVATE\0ARGUMENT".into());
    assert_eq!(
        OwnedProcess::spawn(&spec).unwrap_err(),
        OwnedProcessError::InvalidLaunch
    );
    assert!(LaunchSpec::validate(Path::new("syncthing.exe"), &fixture.0, vec![], vec![]).is_err());
    assert!(
        LaunchSpec::validate(
            &std::env::current_exe().unwrap(),
            Path::new("relative"),
            vec![],
            vec![]
        )
        .is_err()
    );
    assert!(!fixture.0.join("ready").exists());
}

#[test]
fn duplicate_environment_keys_and_nul_values_fail_before_execution() {
    let fixture = Fixture::new();
    for extra in [
        vec![
            ("SAMPLE".into(), "one".into()),
            ("sample".into(), "two".into()),
        ],
        vec![("SAMPLE".into(), "PRIVATE\0VALUE".into())],
    ] {
        let mut spec = fixture.spec("hold");
        spec.environment.extend(extra);
        assert_eq!(
            OwnedProcess::spawn(&spec).unwrap_err(),
            OwnedProcessError::InvalidLaunch
        );
    }
    assert!(!fixture.0.join("ready").exists());
}

#[test]
fn repository_runtime_and_unsafe_bounds_are_rejected_without_writes() {
    let fixture = Fixture::new();
    let program = std::env::current_exe().unwrap();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        LaunchSpec::validate(&program, &repo, vec![], vec![]).unwrap_err(),
        OwnedProcessError::UnsafeWorkingDirectory
    );
    assert_eq!(
        LaunchSpec::validate(&program, &fixture.0, vec!["x".into(); 33], vec![]).unwrap_err(),
        OwnedProcessError::InvalidLaunch
    );
    let mut spec = fixture.spec("hold");
    spec.arguments.push("x".repeat(32768).into());
    assert_eq!(
        OwnedProcess::spawn(&spec).unwrap_err(),
        OwnedProcessError::InvalidLaunch
    );
    assert!(!fixture.0.join("ready").exists());
}

#[test]
fn prepared_syncthing_role_has_fixed_private_profile_flags_and_minimal_environment() {
    let fixture = Fixture::new();
    let program = fixture.0.join("syncthing.exe");
    let spec = LaunchSpec::syncthing(&program, &fixture.0).unwrap();
    let arguments: Vec<_> = spec
        .arguments
        .iter()
        .map(|arg| arg.to_string_lossy())
        .collect();
    for flag in [
        "serve",
        "--home",
        "--no-browser",
        "--no-restart",
        "--no-upgrade",
        "--no-port-probing",
        "--log-file",
    ] {
        assert!(arguments.iter().any(|arg| arg == flag));
    }
    assert!(
        !spec
            .environment
            .iter()
            .any(|(name, _)| name == "PATH" || name == "STGUIADDRESS" || name == "STAPIKEY")
    );
    assert!(
        spec.environment
            .iter()
            .any(|(name, value)| name == "STHOMEDIR" && value == fixture.0.as_os_str())
    );
    assert!(
        spec.environment
            .iter()
            .any(|(name, value)| name == "STNODEFAULTFOLDER" && value == "1")
    );
    // Preparing this role creates no profile/config and launches nothing.
    assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 0);
}

#[test]
#[ignore = "requires the explicit fresh, protected, synthetic native fixture from the Windows batch runner"]
fn native_syncthing_lifecycle_fixture() {
    run_native_syncthing_lifecycle_fixture(false);
}

#[test]
#[ignore = "requires the explicit protected native fixture; uses a fresh DEV-only Windows credential account"]
fn native_syncthing_private_profile_and_os_credential_fixture() {
    run_native_syncthing_lifecycle_fixture(true);
}

#[cfg(windows)]
fn run_native_syncthing_lifecycle_fixture(use_managed_profile: bool) {
    use crate::managed_profile::ManagedProfile;
    use crate::secure_store::{
        FakeSecureStore, SecureStore, SecureStoreEnvironment, WindowsCredentialStore,
    };
    use crate::syncthing_owned_rest::{EndpointError, OwnedRestError, OwnedSyncthingRest};
    use crate::syncthing_rest::{ApiCredential, SyncthingRest, SyncthingRestError};
    use serde::{Deserialize, Serialize};
    use sha2::{Digest, Sha256};
    use std::io::Read;

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct NativeFixture {
        schema_version: u32,
        scope: String,
        binary: PathBuf,
        api_port: u16,
        native_id: String,
    }
    #[derive(Serialize)]
    struct NativeRun {
        pid: u32,
        exit_code: Option<i64>,
        forced: bool,
        auth_rejection: u16,
    }

    let root = PathBuf::from(
        std::env::var_os("CRYPTEX_M8_NATIVE_FIXTURE").expect("explicit native fixture required"),
    );
    assert!(root.is_absolute());
    assert!(
        root.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("m8-owned-native-")
    );
    assert_outside_repository(&root).unwrap();
    let mut fixture_bytes = Vec::new();
    fs::File::open(root.join("fixture.json"))
        .unwrap()
        .take(65537)
        .read_to_end(&mut fixture_bytes)
        .unwrap();
    assert!(fixture_bytes.len() <= 65536);
    let fixture: NativeFixture = serde_json::from_slice(&fixture_bytes).unwrap();
    assert_eq!(fixture.schema_version, 1);
    assert_eq!(fixture.scope, "m8-owned-native-synthetic");
    assert!(fixture.binary.is_absolute());
    let mut binary = fs::File::open(&fixture.binary).unwrap();
    assert!(binary.metadata().unwrap().len() < 128 * 1024 * 1024);
    let mut hasher = Sha256::new();
    let mut block = [0; 16384];
    loop {
        let count = binary.read(&mut block).unwrap();
        if count == 0 {
            break;
        }
        hasher.update(&block[..count]);
    }
    assert_eq!(
        format!("{:x}", hasher.finalize()),
        "36a0f7bc372f64fa7cc4f5654fa324c0dd9f7fef2e07565e00c6e1cf73f50344"
    );
    let original_profile = root.join("profile");
    let config_bytes = fs::read(original_profile.join("config.xml")).unwrap();
    assert!(config_bytes.len() < 256 * 1024);
    let config = std::str::from_utf8(&config_bytes).unwrap();
    // Test-only read of the fresh native-generated fixture, never a personal or
    // production profile. The product config reader is not implemented here.
    assert_eq!(config.matches("<apikey>").count(), 1);
    let key = config
        .split_once("<apikey>")
        .unwrap()
        .1
        .split_once("</apikey>")
        .unwrap()
        .0;
    for name in ["generate.stdout.txt", "generate.stderr.txt"] {
        let generated_log = fs::read(root.join(name)).unwrap();
        assert!(
            !String::from_utf8_lossy(&generated_log).contains(key),
            "native API key appeared in generation logs"
        );
    }
    let store: Box<dyn SecureStore> = if use_managed_profile {
        Box::new(WindowsCredentialStore::new())
    } else {
        Box::new(FakeSecureStore::new())
    };
    // The real-key variant copies ONLY the native-generated synthetic fixture
    // into a newly product-created private profile. No user/global profile,
    // fixture identity generation or product XML provisioning is claimed.
    let mut managed = None;
    let profile_id = if use_managed_profile {
        let mut bytes = [0u8; 16];
        getrandom::getrandom(&mut bytes).unwrap();
        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
    } else {
        "native-fixture".to_owned()
    };
    let profile = if use_managed_profile {
        let path = root.join(&profile_id);
        let owner = ManagedProfile::create(&path, SecureStoreEnvironment::Dev).unwrap();
        for name in ["config.xml", "cert.pem", "key.pem"] {
            fs::copy(original_profile.join(name), path.join(name)).unwrap();
        }
        managed = Some(owner);
        path
    } else {
        store
            .put(
                "Cryptex-Dev-ManagedSyncthing",
                "api:native-fixture",
                key.as_bytes(),
            )
            .unwrap();
        original_profile
    };
    struct CredentialCleanup<'a> {
        store: &'a dyn SecureStore,
        account: String,
    }
    impl Drop for CredentialCleanup<'_> {
        fn drop(&mut self) {
            let _ = self
                .store
                .delete("Cryptex-Dev-ManagedSyncthing", &self.account);
        }
    }
    let _credential_cleanup = CredentialCleanup {
        store: store.as_ref(),
        account: format!(
            "api:{}",
            managed
                .as_ref()
                .map_or(profile_id.as_str(), |owner| owner.credential_profile_id())
        ),
    };
    if let Some(owner) = &mut managed {
        assert_eq!(
            store
                .get("Cryptex-Dev-ManagedSyncthing", &_credential_cleanup.account)
                .unwrap_err(),
            crate::secure_store::SecureStoreError::Missing
        );
        owner
            .enroll_native_api_key(store.as_ref(), key.as_bytes())
            .unwrap();
    }
    let endpoint = format!("127.0.0.1:{}", fixture.api_port).parse().unwrap();
    let rest = SyncthingRest::for_owned_profile(
        endpoint,
        &fixture.native_id,
        if let Some(owner) = &managed {
            owner.load_api_credential(store.as_ref()).unwrap()
        } else {
            ApiCredential::load_existing(store.as_ref(), SecureStoreEnvironment::Dev, &profile_id)
                .unwrap()
        },
    )
    .unwrap();
    let mut runs = Vec::new();
    for _ in 0..2 {
        let mut owned =
            OwnedProcess::spawn(&LaunchSpec::syncthing(&fixture.binary, &profile).unwrap())
                .unwrap();
        let deadline = Instant::now() + Duration::from_secs(12);
        loop {
            assert!(
                owned.try_exit().unwrap().is_none(),
                "native transport exited before readiness"
            );
            if let Some(profile_owner) = &managed {
                let result = OwnedSyncthingRest::new(
                    &owned,
                    endpoint,
                    &fixture.native_id,
                    profile_owner.load_api_credential(store.as_ref()).unwrap(),
                )
                .unwrap()
                .observe();
                match result {
                    Ok(_) => break,
                    Err(
                        OwnedRestError::Endpoint(EndpointError::Unavailable)
                        | OwnedRestError::Deadline,
                    ) if Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(100));
                        continue;
                    }
                    Err(error) => panic!("owned native readiness: {error}"),
                }
            }
            match rest.observe() {
                Ok(_) => break,
                Err(SyncthingRestError::Unavailable | SyncthingRestError::Deadline)
                    if Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(error) => panic!("native readiness: {error}"),
            }
        }
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap();
        let status_url = format!("http://{endpoint}/rest/system/status");
        let owned_rest = managed.as_ref().map(|profile_owner| {
            OwnedSyncthingRest::new(
                &owned,
                endpoint,
                &fixture.native_id,
                profile_owner.load_api_credential(store.as_ref()).unwrap(),
            )
            .unwrap()
        });
        let rejected = if owned_rest.is_some() {
            let error = OwnedSyncthingRest::new(
                &owned,
                endpoint,
                &fixture.native_id,
                ApiCredential::from_bytes(b"incorrect-synthetic-key").unwrap(),
            )
            .unwrap()
            .observe()
            .unwrap_err();
            match error {
                OwnedRestError::Unauthorized(code @ (401 | 403)) => code,
                error => panic!("expected native auth rejection: {error}"),
            }
        } else {
            client
                .get(&status_url)
                .header("X-API-Key", "incorrect-synthetic-key")
                .send()
                .unwrap()
                .status()
                .as_u16()
        };
        assert!(matches!(rejected, 401 | 403));
        let mut api_header = reqwest::header::HeaderValue::from_bytes(key.as_bytes()).unwrap();
        api_header.set_sensitive(true);
        let mut live_config = Vec::new();
        if let Some(owned_rest) = &owned_rest {
            live_config = owned_rest.test_read_config().unwrap();
        } else {
            let config_response = client
                .get(format!("http://{endpoint}/rest/config"))
                .header("X-API-Key", api_header.clone())
                .send()
                .unwrap();
            assert_eq!(config_response.status().as_u16(), 200);
            config_response
                .take(262145)
                .read_to_end(&mut live_config)
                .unwrap();
        }
        assert!(live_config.len() <= 262144);
        let live: serde_json::Value = serde_json::from_slice(&live_config).unwrap();
        assert!(
            live["folders"]
                .as_array()
                .is_some_and(|folders| folders.is_empty())
        );
        for option in [
            "globalAnnounceEnabled",
            "localAnnounceEnabled",
            "relaysEnabled",
            "natEnabled",
        ] {
            assert!(
                live["options"][option].as_bool() == Some(false),
                "native external transport option was enabled"
            );
        }
        assert!(live["options"]["autoUpgradeIntervalH"].as_u64() == Some(0));
        assert!(live["options"]["urAccepted"].as_i64() == Some(-1));
        assert!(live["gui"]["address"].as_str() == Some(endpoint.to_string().as_str()));
        assert!(live["gui"]["insecureAdminAccess"].as_bool() == Some(false));
        let listeners = live["options"]["listenAddresses"].as_array().unwrap();
        assert!(
            listeners.len() == 1
                && listeners[0]
                    .as_str()
                    .is_some_and(|listener| listener.starts_with("tcp://127.0.0.1:"))
        );
        if let Some(owned_rest) = &owned_rest {
            owned_rest.request_shutdown().unwrap();
        } else {
            let stop = client
                .post(format!("http://{endpoint}/rest/system/shutdown"))
                .header("X-API-Key", api_header)
                .send()
                .unwrap();
            assert_eq!(stop.status().as_u16(), 200);
        }
        drop(owned_rest);
        let exit = owned.wait_for_exit(Duration::from_secs(25)).unwrap();
        assert!(exit.is_clean(), "native transport did not cleanly stop");
        assert!(!process_is_running(owned.pid()));
        let log = fs::read(profile.join("native.log")).unwrap();
        assert!(log.len() < 4 * 1024 * 1024);
        assert!(!String::from_utf8_lossy(&log).contains("Database failed to stop within 10s"));
        assert!(
            !String::from_utf8_lossy(&log).contains(key),
            "native API key appeared in transport logs"
        );
        runs.push(NativeRun {
            pid: owned.pid(),
            exit_code: exit.exit_code,
            forced: exit.forced,
            auth_rejection: rejected,
        });
    }
    let proof = if use_managed_profile {
        store
            .delete("Cryptex-Dev-ManagedSyncthing", &_credential_cleanup.account)
            .unwrap();
        assert_eq!(
            store
                .get("Cryptex-Dev-ManagedSyncthing", &_credential_cleanup.account)
                .unwrap_err(),
            crate::secure_store::SecureStoreError::Missing
        );
        serde_json::to_vec_pretty(&serde_json::json!({
            "credential_store": "windows_credential_manager",
            "established_endpoint_attested": true,
            "synthetic_credential_deleted": true,
            "runs": runs,
        }))
        .unwrap()
    } else {
        serde_json::to_vec_pretty(&runs).unwrap()
    };
    fs::write(
        root.join(if use_managed_profile {
            "native-private-profile-result.json"
        } else {
            "native-result.json"
        }),
        proof,
    )
    .unwrap();
}
