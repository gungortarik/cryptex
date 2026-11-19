//! Browser harness for the normal Desktop clients and the authenticated IPC dispatcher.
//! Both helper processes are test executables. The server creates its own root.
use super::tests::prepared_search_fixture;
use super::*;
use cryptex_events::Classification;
use sha2::Digest;
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
    if enabled {
        fixture.context.purge_reviews = Some(Arc::default());
    }
    start_projection_sync_worker(&mut fixture.context);
    let workers = Arc::new(AtomicUsize::new(0));
    println!("fixture-ready");
    io::stdout().flush().unwrap();
    for command in io::stdin().lock().lines() {
        let command = command.unwrap();
        if command == "finish" {
            break;
        }
        if command == "approval_inventory" {
            let ledger = root.join("canonical").join(purge_review::receipt::FOLDER);
            let mut inventory = Vec::new();
            if ledger.try_exists().unwrap() {
                for entry in fs::read_dir(ledger).unwrap() {
                    let path = entry.unwrap().path();
                    inventory.push((
                        path.file_name().unwrap().to_str().unwrap().to_owned(),
                        format!("{:x}", sha2::Sha256::digest(fs::read(&path).unwrap())),
                    ));
                }
            }
            inventory.sort();
            println!(
                "approval-inventory {}",
                serde_json::to_string(&inventory).unwrap()
            );
            io::stdout().flush().unwrap();
            continue;
        }
        let before = (command == "review").then(|| crate::test_temp::event_hashes(root));
        let mut connection = listener.accept().unwrap();
        handle_connection(
            &mut connection,
            Some(&fixture.context),
            None,
            Some(&workers),
            None,
        )
        .unwrap();
        if let Some(before) = before {
            assert!(!before.is_empty());
            assert_eq!(crate::test_temp::event_hashes(root), before);
        }
    }
    instance.drain_runtime(Duration::from_secs(5)).unwrap();
    drop(fixture.context);
    drop(instance);
    let original = crate::instance::try_acquire_instance(root).unwrap();
    let mut resolver = LocalDeviceKeyResolver::load(
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
    let mut pairs = Vec::new();
    let digest = <[u8; 32]>::from(sha2::Sha256::digest(TEST_BYTES));
    for record in chain {
        let cryptex_events::AuthenticatedSecurityRecord::Attachment(link, fields) = record else {
            continue;
        };
        let note = chain
            .iter()
            .find_map(|record| match record {
                cryptex_events::AuthenticatedSecurityRecord::V1(note)
                    if note.event_id == fields.note_event_id() =>
                {
                    Some(note)
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(note.classification, Classification::Internal);
        assert_eq!(fields.bind(note).unwrap().blob_sha256, digest);
        pairs.push((note.event_id.clone(), link.event_id.clone()));
    }
    let approved_note = pairs.first().map(|(id, _)| {
        let note = chain.iter().find(|record| record.event_id() == id).unwrap();
        (id.clone(), note.content_hash().as_str().to_owned())
    });
    drop(snapshot);
    let blobs = root.join("canonical/blobs");
    if enabled {
        let [first, second] = pairs.as_slice() else {
            panic!("expected one signed link per note, including after retry");
        };
        assert_ne!(first.0, second.0);
        assert_ne!(first.1, second.1);
        let db = ProjectionDb::open_or_create(root).unwrap();
        let gate = crate::knowledge_materialization::evaluate_retrieval_safety(
            root,
            &fixture.brain_id,
            std::slice::from_ref(&fixture.device),
            &mut resolver,
            &db,
        );
        for (own, other) in [(first, second), (second, first)] {
            assert!(matches!(
                gate.open_local_attachment(&own.0, &other.1),
                Err(crate::attachment_blobs::LocalLinkError::Reference(
                    cryptex_events::attachment_link::LinkError::MissingLink
                ))
            ));
            let mut bytes = Vec::new();
            gate.open_local_attachment(&own.0, &own.1)
                .unwrap()
                .read_to_end(&mut bytes)
                .unwrap();
            assert_eq!(bytes, TEST_BYTES);
        }
        let stored = fs::read_dir(blobs)
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(stored.len(), 1);
        assert!(stored[0].file_type().unwrap().is_file());
        let expected_name = format!("{:x}", sha2::Sha256::digest(TEST_BYTES));
        assert_eq!(stored[0].file_name().to_str(), Some(expected_name.as_str()));
        println!(
            "shared_attachment=verified notes=2 signed_links=2 canonical_files=1 reopened_reads=2"
        );
    } else {
        assert!(pairs.is_empty());
        assert!(!blobs.exists());
    }
    drop((original, listener));
    if let Some((id, hash)) = approved_note {
        // Reopen from another process with the public trust anchor only.
        let checked = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "ipc::attachment_flow::reopen_approval",
                "--ignored",
                "--nocapture",
            ])
            .env(
                "CRYPTEX_TRIAL_REOPEN",
                serde_json::to_string(&(
                    root,
                    &fixture.brain_id,
                    fixture.device.as_str(),
                    id,
                    hash,
                    anchor.public_key(),
                ))
                .unwrap(),
            )
            .output()
            .unwrap();
        assert!(
            checked.status.success(),
            "{}",
            String::from_utf8_lossy(&checked.stderr)
        );
        assert!(
            String::from_utf8_lossy(&checked.stdout).contains("approval-reopened-and-verified")
        );
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "fresh process spawned after the disposable profile closes"]
fn reopen_approval() {
    let (root, brain, device, note, hash, public): (
        PathBuf,
        String,
        String,
        String,
        String,
        [u8; 32],
    ) = serde_json::from_str(&std::env::var("CRYPTEX_TRIAL_REOPEN").unwrap()).unwrap();
    let _instance = crate::instance::try_acquire_instance(&root).unwrap();
    purge_review::receipt::verify_reopened(&root, &brain, &device, &note, &hash, &public).unwrap();
    assert!(matches!(
        purge_review::receipt::verify_reopened(
            &root,
            &brain,
            &device,
            &note,
            &"0".repeat(64),
            &public
        ),
        Err(purge_review::ReviewError::ReceiptIntegrity)
    ));
    let file = root
        .join("canonical")
        .join(purge_review::receipt::FOLDER)
        .join(format!("{note}.json"));
    let stored: serde_json::Value = serde_json::from_slice(&fs::read(file).unwrap()).unwrap();
    assert_eq!(stored.as_object().unwrap().len(), 2);
    assert_eq!(stored["signature"].as_str().unwrap().len(), 128);
    assert_eq!(
        stored["approval"],
        serde_json::json!({
            "purpose": "cryptex.purge-approval.v1", "brain_id": brain,
            "device_id": device, "note_id": note, "note_hash": hash
        })
    );
    let wrong_key = ed25519_dalek::SigningKey::from_bytes(&[17; 32]).verifying_key();
    assert!(matches!(
        purge_review::receipt::verify_reopened(
            &root,
            &brain,
            &device,
            &note,
            &hash,
            wrong_key.as_bytes()
        ),
        Err(purge_review::ReviewError::ReceiptIntegrity)
    ));
    println!("approval-reopened-and-verified");
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
    for input in io::stdin().lock().lines() {
        let request: serde_json::Value = serde_json::from_str(&input.unwrap()).unwrap();
        let args = &request["args"];
        let command = request["command"].as_str().unwrap();
        if command == "approval_inventory" {
            writeln!(child.stdin.as_mut().unwrap(), "approval_inventory").unwrap();
            line.clear();
            server_log.read_line(&mut line).unwrap();
            let inventory = line.trim().strip_prefix("approval-inventory ").unwrap();
            println!("browser-reply {inventory}");
            io::stdout().flush().unwrap();
            continue;
        }
        let check = if command == "m10_purge_review" {
            "review"
        } else {
            "serve"
        };
        writeln!(child.stdin.as_mut().unwrap(), "{check}").unwrap();
        let result = match command {
            "m10_purge_review" => serde_json::to_value(purge_review::desktop(
                serde_json::from_value(args["action"].clone()).unwrap(),
            ))
            .unwrap(),
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
