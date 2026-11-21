use super::*;
use std::process::Command;

/// Explicit CI rehearsal: the SDK test client needs the locked local-mcp npm dependencies.
#[test]
#[ignore = "run with the local MCP SDK installed"]
fn native_stdio_bridge() {
    #[cfg(target_os = "macos")]
    let workspace = tempfile::Builder::new()
        .prefix("cpx-m12-")
        .tempdir_in("/tmp")
        .unwrap();
    #[cfg(windows)]
    let workspace = tempfile::tempdir().unwrap();
    let root = workspace.path().join("profile");
    let _instance = crate::instance::try_acquire_instance(&root).unwrap();
    let canary = root.join("synthetic-canary");
    std::fs::write(&canary, b"PRIVATE-CANARY").unwrap();
    let entries = || {
        std::fs::read_dir(&root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<std::collections::BTreeSet<_>>()
    };
    let before = entries();
    #[cfg(target_os = "macos")]
    let (endpoint, name) = {
        use interprocess::local_socket::GenericFilePath;
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(workspace.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = workspace.path().join("s");
        (path.clone(), path.to_fs_name::<GenericFilePath>().unwrap())
    };
    #[cfg(windows)]
    let (endpoint, name) = {
        let id = format!(
            "cryptex-m12-{}",
            workspace.path().file_name().unwrap().to_str().unwrap()
        );
        (
            PathBuf::from(format!(r"\\.\pipe\{id}")),
            id.to_ns_name::<GenericNamespaced>().unwrap(),
        )
    };
    let listener = ListenerOptions::new().name(name).create_sync().unwrap();
    let worker = thread::spawn(move || {
        let mut stream = listener.accept().unwrap();
        drop(listener);
        handle_connection(&mut stream, None, None, None, None).unwrap();
    });
    let tests =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/local-mcp/test/bridge.test.mjs");
    let status = Command::new("node")
        .arg("--test")
        .arg(tests)
        .env("CRYPTEX_M12_TEST_ENDPOINT", endpoint)
        .status()
        .unwrap();
    assert!(status.success(), "MCP client tests failed");
    worker.join().unwrap();
    assert_eq!(entries(), before);
    assert_eq!(std::fs::read(canary).unwrap(), b"PRIVATE-CANARY");
}
