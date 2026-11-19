//! Browser harness for the normal Desktop clients and the authenticated IPC dispatcher.
//! Both helper processes are test executables. The server creates its own root.
use super::tests::prepared_search_fixture;
use super::*;
use cryptex_events::Classification;
use std::fs;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

const SERVER: &str = "ipc::attachment_flow::isolated_server";
const TEST_BYTES: &[u8] = &[0, 255, 13, 10, 65, 128];

fn fixture_name(token: &str) -> interprocess::local_socket::Name<'static> {
    cryptex_core::validate::parse_uuid_str(token).unwrap();
    #[cfg(windows)]
    {
        std::ffi::OsString::from(format!("cryptex-file-{token}"))
            .to_ns_name::<GenericNamespaced>()
            .unwrap()
    }
    #[cfg(unix)]
    {
        PathBuf::from("/tmp")
            .join(format!("cpx-file-{token}"))
            .to_fs_name::<interprocess::local_socket::GenericFilePath>()
            .unwrap()
    }
}

#[test]
#[ignore = "started by the isolated browser client"]
fn isolated_server() {
    let token = std::env::var("CRYPTEX_FILE_FIXTURE_SOCKET").unwrap();
    let enabled = std::env::var("CRYPTEX_FILE_FIXTURE_MODE").unwrap() == "allowed";
    let listener = ListenerOptions::new()
        .name(fixture_name(&token))
        .create_sync()
        .unwrap();
    let mut fixture = prepared_search_fixture("attachment");
    let root = &fixture.root;
    drop(crate::instance::try_acquire_instance(root).unwrap());
    crate::instance::migration::migrate_profile(root).unwrap();
    let instance = crate::instance::try_acquire_instance(root).unwrap();
    fixture.context.admission = instance.admission();
    fixture.context.isolated_attachment_writer = enabled;
    start_projection_sync_worker(&mut fixture.context);
    let workers = Arc::new(AtomicUsize::new(0));
    println!("fixture-ready");
    io::stdout().flush().unwrap();
    for command in io::stdin().lock().lines() {
        if command.unwrap() == "finish" {
            break;
        }
        let mut connection = listener.accept().unwrap();
        handle_connection(
            &mut connection,
            Some(&fixture.context),
            None,
            Some(&workers),
            None,
        )
        .unwrap();
    }
    instance.drain_runtime(Duration::from_secs(5)).unwrap();
    drop(fixture.context);
    drop(instance);
    let original = crate::instance::try_acquire_instance(root).unwrap();
    let resolver = LocalDeviceKeyResolver::load(
        root,
        &fixture.brain_id,
        fixture.store.as_ref(),
        SecureStoreEnvironment::Dev,
    )
    .unwrap();
    let anchor = resolver.local_trust_anchor().unwrap();
    let snapshot =
        crate::knowledge_materialization::scan_verified_canonical_from_local_anchor_until(
            root,
            &fixture.brain_id,
            &anchor,
            None,
        )
        .unwrap();
    let chain = snapshot.reader_device_chain(fixture.device.as_str());
    let links: Vec<_> = chain
        .iter()
        .filter_map(|record| match record {
            cryptex_events::AuthenticatedSecurityRecord::Attachment(_, fields) => Some(fields),
            _ => None,
        })
        .collect();
    assert_eq!(links.len(), usize::from(enabled));
    if enabled {
        use sha2::{Digest, Sha256};
        let note = chain
            .iter()
            .find_map(|record| match record {
                cryptex_events::AuthenticatedSecurityRecord::V1(note)
                    if note.event_id == links[0].note_event_id() =>
                {
                    Some(note)
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(note.classification, Classification::Internal);
        let binding = links[0].bind(note).unwrap();
        assert_eq!(
            binding.blob_sha256,
            <[u8; 32]>::from(Sha256::digest(TEST_BYTES))
        );
        crate::attachment_blobs::verify_stored(root, &binding.blob_sha256).unwrap();
        assert_eq!(
            fs::read_dir(root.join("canonical/blobs")).unwrap().count(),
            1
        );
    } else {
        assert!(!root.join("canonical/blobs").exists());
    }
    drop((snapshot, original, listener));
    fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "launched from Playwright with stdin and stdout pipes"]
fn browser_client() {
    let token = new_bootstrap_event_id().unwrap();
    FIXTURE_SOCKET.with(|slot| slot.set(fixture_name(&token)).unwrap());
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", SERVER, "--ignored", "--nocapture"])
        .env("CRYPTEX_FILE_FIXTURE_SOCKET", token)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut server_log = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    loop {
        line.clear();
        assert_ne!(
            server_log.read_line(&mut line).unwrap(),
            0,
            "server exited before ready"
        );
        if line.contains("fixture-ready") {
            break;
        }
    }
    println!("browser-ready");
    io::stdout().flush().unwrap();
    let timeout = Duration::from_secs(2);
    for line in io::stdin().lock().lines() {
        let request: serde_json::Value = serde_json::from_str(&line.unwrap()).unwrap();
        let args = &request["args"];
        writeln!(child.stdin.as_mut().unwrap(), "serve").unwrap();
        let result = match request["command"].as_str().unwrap() {
            "p0_5_guarded_timeline" => serde_json::to_value(
                desktop_timeline(
                    serde_json::from_value(args["query"].clone()).unwrap(),
                    timeout,
                )
                .unwrap(),
            )
            .unwrap(),
            "p0_5_guarded_event_detail" => serde_json::to_value(
                desktop_event_detail(args["eventId"].as_str().unwrap().to_owned(), timeout)
                    .unwrap(),
            )
            .unwrap(),
            "p0_5_capture_memory" => serde_json::to_value(
                desktop_capture(
                    serde_json::from_value(args["body"].clone()).unwrap(),
                    timeout,
                )
                .unwrap(),
            )
            .unwrap(),
            "m9_attach_file" => serde_json::to_value(attachment::desktop_attach(
                serde_json::from_value(args["file"].clone()).unwrap(),
            ))
            .unwrap(),
            _ => panic!("unexpected browser command"),
        };
        println!("browser-reply {result}");
        io::stdout().flush().unwrap();
    }
    writeln!(child.stdin.take().unwrap(), "finish").unwrap();
    io::copy(&mut server_log, &mut io::stderr()).unwrap();
    assert!(child.wait().unwrap().success());
}
