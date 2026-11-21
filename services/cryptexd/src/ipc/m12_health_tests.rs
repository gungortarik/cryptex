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

    fn request(
        &mut self,
        listener: &interprocess::local_socket::Listener,
        ctx: &SearchContext,
    ) -> String {
        self.send(
            json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{
                "name":"cryptex_request_public_read","arguments":{}
            }}),
        );
        let mut stream = listener.accept().unwrap();
        let ctx = ctx.clone();
        thread::spawn(move || {
            handle_connection(&mut stream, Some(&ctx), None, None, None).unwrap()
        });
        let reply = self.receive();
        let ticket = reply["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .split_whitespace()
            .last()
            .unwrap();
        cryptex_core::validate::parse_uuid_str(ticket).unwrap();
        ticket.to_string()
    }

    fn search(&mut self) -> Value {
        self.send(
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{
                "name":"cryptex_search_public_notes","arguments":{"phrase":"synthetic"}
            }}),
        );
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
    use public_consent::Action;
    let decide = |ticket: String, allow| {
        let action = if allow {
            Action::Allow { ticket }
        } else {
            Action::Deny { ticket }
        };
        fixture.context.public_consent.decide(action).unwrap();
    };
    assert_eq!(bridge.search(), denied);
    let ticket = bridge.request(&listener, &fixture.context);
    let claimed_owner = serde_json::to_vec(&json!({
        "protocol_version": 1, "request_id": new_bootstrap_event_id().unwrap(),
        "operation": "context.public.review", "client_role": "desktop",
        "capabilities": ["context.public.owner"], "action": {"kind": "allow", "ticket": ticket}
    }))
    .unwrap();
    assert!(matches!(
        public_consent::review_frame(&claimed_owner, Some(&fixture.context), false).result,
        Err(public_consent::Error::Unauthorized)
    ));
    decide(ticket, false);
    assert_eq!(bridge.search(), denied);
    let ticket = bridge.request(&listener, &fixture.context);
    assert!(
        fixture
            .context
            .public_consent
            .decide(Action::Allow {
                ticket: "stale".into()
            })
            .is_err()
    );
    let mut other = Bridge::start(&endpoint);
    decide(ticket, true);
    assert_eq!(other.search(), denied);
    let hits = bridge.search();
    assert_eq!(bridge.search(), denied);
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
    decide(bridge.request(&listener, &fixture.context), true);
    let after_forget = bridge.search();
    let expected = json!({"notes":[visible_note]});
    assert_eq!(after_forget["structuredContent"], expected);
    assert_eq!(after_forget["content"][0]["text"], expected.to_string());
    assert_eq!(crate::test_temp::event_hashes(&fixture.root), before);
    cryptex_storage::append_frame(&fixture.root, &fixture.device, 0, b"broken record").unwrap();
    decide(bridge.request(&listener, &fixture.context), true);
    assert_eq!(
        bridge.search(),
        json!({"isError":true,"content":[{"type":"text","text":"public_read_unavailable"}]})
    );
    let root = fixture.root.clone();
    drop((fixture, instance));
    std::fs::remove_dir_all(root).unwrap();
}

/// The hosted VM has no personal daemon. The parent represents the Desktop process;
/// its child recognizes the parent's OS identity on the real product socket.
#[test]
#[ignore = "requires an installed npm package, built Desktop and an empty hosted runner"]
fn installed_client_owner_consent() {
    assert_eq!(
        std::env::var("RUNNER_ENVIRONMENT").as_deref(),
        Ok("github-hosted")
    );
    assert_eq!(std::env::var("GITHUB_ACTIONS").as_deref(), Ok("true"));
    if std::env::var_os("CRYPTEX_M12_CONSENT_FIXTURE").is_some() {
        serve_installed_consent_fixture();
        return;
    }
    let mut daemon = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "ipc::m12_health_tests::installed_client_owner_consent",
            "--ignored",
            "--nocapture",
        ])
        .env("CRYPTEX_M12_CONSENT_FIXTURE", "1")
        .stdin(process::Stdio::piped())
        .stdout(process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut daemon_output = BufReader::new(daemon.stdout.take().unwrap());
    let ready = (&mut daemon_output)
        .lines()
        .map(Result::unwrap)
        .any(|line| line.ends_with("consent-fixture-ready"));
    assert!(ready, "disposable daemon startup");
    let binary = std::env::var_os("M12_CODEX_BIN").expect("installed Codex executable");
    let sdk_passed = review_in_browser(None);
    let codex_passed = review_in_browser(Some(&binary));
    drop(daemon.stdin.take());
    assert!(daemon.wait().unwrap().success(), "daemon integrity check");
    assert!(sdk_passed, "SDK client / Desktop review");
    assert!(codex_passed, "Codex client / Desktop review");
    eprintln!(
        "SDK and Codex each passed public-note approval, denial, Escape and grant reuse checks."
    );
}

fn review_in_browser(codex: Option<&std::ffi::OsStr>) -> bool {
    let mut command = Command::new("node");
    if let Some(binary) = codex {
        command.env("M12_CODEX_BIN", binary);
    } else {
        command.env_remove("M12_CODEX_BIN");
    }
    let mut browser = command
        .arg(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../apps/desktop/scripts/public-consent.test.mjs"),
        )
        .stdin(process::Stdio::piped())
        .stdout(process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut replies = browser.stdin.take().unwrap();
    for line in BufReader::new(browser.stdout.take().unwrap()).lines() {
        let action: public_consent::Action = serde_json::from_str(&line.unwrap()).unwrap();
        let result = public_consent::desktop(action);
        writeln!(replies, "{}", serde_json::to_string(&result).unwrap()).unwrap();
        replies.flush().unwrap();
    }
    browser.wait().unwrap().success()
}

fn serve_installed_consent_fixture() {
    use crate::canonical_writer::{AppendMemoryNoteArgs, append_memory_note};
    use cryptex_events::TrustedDaemonKeyResolver;
    let mut fixture = tests::prepared_search_fixture("installed-m12-review");
    let instance = crate::instance::try_acquire_instance(&fixture.root).unwrap();
    fixture.context.admission = instance.admission();
    append_memory_note(
        fixture.store.as_ref(),
        fixture.context.environment,
        AppendMemoryNoteArgs {
            runtime_root: &fixture.root,
            brain_id: &fixture.brain_id,
            device_id: &fixture.device,
            classification: Classification::Public,
            text: "synthetic installed public",
            event_id: None,
            config: &CanonicalWriterConfig::default(),
        },
    )
    .unwrap();
    let resolver = LocalDeviceKeyResolver::load(
        &fixture.root,
        &fixture.brain_id,
        fixture.store.as_ref(),
        fixture.context.environment,
    )
    .unwrap();
    let mut db = ProjectionDb::open_or_create(&fixture.root).unwrap();
    crate::knowledge_materialization::rebuild_knowledge_from_local_approval_history(
        &fixture.root,
        &fixture.brain_id,
        &resolver.local_trust_anchor().unwrap(),
        &mut db,
    )
    .unwrap();
    drop(db);
    let before = crate::test_temp::event_hashes(&fixture.root);
    let listener = create_listener(false).unwrap();
    let ctx = fixture.context.clone();
    thread::spawn(move || {
        while let Ok(mut stream) = listener.accept() {
            let ctx = ctx.clone();
            thread::spawn(move || {
                handle_connection(&mut stream, Some(&ctx), None, None, None).unwrap()
            });
        }
    });
    println!("consent-fixture-ready");
    io::stdout().flush().unwrap();
    let mut end = String::new();
    io::stdin().read_line(&mut end).unwrap();
    assert_eq!(crate::test_temp::event_hashes(&fixture.root), before);
    let root = fixture.root.clone();
    drop((instance, fixture));
    std::fs::remove_dir_all(root).unwrap();
}
