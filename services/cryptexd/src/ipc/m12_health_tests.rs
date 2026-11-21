use super::*;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader};
use std::process::Command;

fn isolated_listener() -> (
    tempfile::TempDir,
    PathBuf,
    interprocess::local_socket::Listener,
) {
    #[cfg(target_os = "macos")]
    let workspace = tempfile::Builder::new()
        .prefix("cpx-m12-")
        .tempdir_in("/tmp")
        .unwrap();
    #[cfg(windows)]
    let workspace = tempfile::tempdir().unwrap();
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
    (workspace, endpoint, listener)
}

/// Explicit CI rehearsal: the SDK test client needs the locked local-mcp npm dependencies.
#[test]
#[ignore = "run with the local MCP SDK installed"]
fn native_stdio_bridge() {
    let (workspace, endpoint, listener) = isolated_listener();
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

struct Bridge {
    child: Arc<Mutex<process::Child>>,
    input: process::ChildStdin,
    output: BufReader<process::ChildStdout>,
}

impl Bridge {
    fn start(endpoint: &Path) -> Self {
        let entry = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/local-mcp/test/fixture-stdio.mjs");
        let mut child = Command::new("node")
            .arg(entry)
            .arg(endpoint)
            .stdin(process::Stdio::piped())
            .stdout(process::Stdio::piped())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        let mut bridge = Self {
            input,
            output,
            child: Arc::new(Mutex::new(child)),
        };
        bridge.send(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
            "protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"native-test","version":"1"}
        }}));
        assert_eq!(
            bridge.receive()["result"]["serverInfo"]["name"],
            "cryptex-local"
        );
        bridge.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
        bridge
    }

    fn send(&mut self, value: Value) {
        writeln!(self.input, "{value}").unwrap();
        self.input.flush().unwrap();
    }

    fn receive(&mut self) -> Value {
        let mut line = String::new();
        self.output.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }

    fn search(
        &mut self,
        listener: &interprocess::local_socket::Listener,
        ctx: &SearchContext,
    ) -> Value {
        self.send(
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{
                "name":"cryptex_search_public_notes","arguments":{"phrase":"synthetic"}
            }}),
        );
        let mut stream = listener.accept().unwrap();
        handle_connection(&mut stream, Some(ctx), None, None, None).unwrap();
        self.receive()["result"].clone()
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        let mut child = self.child.lock().unwrap();
        let _ = child.kill();
        child.wait().unwrap();
    }
}

#[test]
#[ignore = "run with the local MCP SDK installed"]
fn approved_bridge_reads_only_visible_public_notes() {
    use crate::canonical_writer::{AppendMemoryNoteArgs, append_memory_note};
    use cryptex_events::TrustedDaemonKeyResolver;
    let (_workspace, endpoint, listener) = isolated_listener();
    let mut fixture = tests::prepared_search_fixture("public-mcp");
    let instance = crate::instance::try_acquire_instance(&fixture.root).unwrap();
    fixture.context.admission = instance.admission();
    let env = fixture.context.environment;
    let store = fixture.store.as_ref();
    let append = |classification, text: &str| {
        append_memory_note(
            store,
            env,
            AppendMemoryNoteArgs {
                runtime_root: &fixture.root,
                brain_id: &fixture.brain_id,
                device_id: &fixture.device,
                classification,
                text,
                event_id: None,
                config: &CanonicalWriterConfig::default(),
            },
        )
        .unwrap()
        .accepted
    };
    append(Classification::Private, "synthetic private");
    append(Classification::Restricted, "synthetic restricted");
    let visible = append(Classification::Public, "synthetic visible");
    let target = append(Classification::Public, "synthetic forget");
    let resolver =
        LocalDeviceKeyResolver::load(&fixture.root, &fixture.brain_id, store, env).unwrap();
    let mut db = ProjectionDb::open_or_create(&fixture.root).unwrap();
    crate::knowledge_materialization::rebuild_knowledge_from_local_approval_history(
        &fixture.root,
        &fixture.brain_id,
        &resolver.local_trust_anchor().unwrap(),
        &mut db,
    )
    .unwrap();
    drop(db);
    let mut bridge = Bridge::start(&endpoint);
    let denied = json!({"isError":true,"content":[{"type":"text","text":"public_read_denied"}]});
    assert_eq!(bridge.search(&listener, &fixture.context), denied);
    fixture.context.public_reader = Some(bridge.child.clone());
    let mut other = Bridge::start(&endpoint);
    assert_eq!(other.search(&listener, &fixture.context), denied);
    let hits = bridge.search(&listener, &fixture.context);
    let visible_note = json!({"event_id":visible.event_id,"text":"synthetic visible"});
    let forgotten_note = json!({"event_id":target.event_id,"text":"synthetic forget"});
    let notes = hits["structuredContent"]["notes"].as_array().unwrap();
    assert_eq!(notes.len(), 2);
    assert!(notes.contains(&visible_note) && notes.contains(&forgotten_note));
    let key = load_existing_event_signing_key(store, env, &fixture.root, &fixture.device).unwrap();
    let forget = crate::test_temp::sign_v2_fixture(
        json!({
            "schema_version":2, "event_id":"88888888-1111-4111-8111-000000000001",
            "device_id":fixture.device.as_str(),"brain_id":fixture.brain_id,
            "device_seq": (target.device_seq.to_u64().unwrap()+1).to_string(),
            "created_at":target.created_at, "event_type":"memory.forget","payload_schema_version":1,
            "classification":target.classification,"signing_key_id":target.signing_key_id,
            "prev_content_hash": target.content_hash, "payload":{"note_event_id":target.event_id,
            "note_content_hash":target.content_hash,"prior_state_hash":target.content_hash}
        }),
        &key.signing_key,
    );
    cryptex_storage::append_frame(&fixture.root, &fixture.device, 0, &forget).unwrap();
    let before = crate::test_temp::event_hashes(&fixture.root);
    let after_forget = bridge.search(&listener, &fixture.context);
    let expected = json!({"notes":[visible_note]});
    assert_eq!(after_forget["structuredContent"], expected);
    assert_eq!(after_forget["content"][0]["text"], expected.to_string());
    assert_eq!(crate::test_temp::event_hashes(&fixture.root), before);
    cryptex_storage::append_frame(&fixture.root, &fixture.device, 0, b"broken record").unwrap();
    assert_eq!(
        bridge.search(&listener, &fixture.context),
        json!({"isError":true,"content":[{"type":"text","text":"public_read_unavailable"}]})
    );
    let root = fixture.root.clone();
    drop((fixture, instance));
    std::fs::remove_dir_all(root).unwrap();
}
