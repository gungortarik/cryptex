//! Exercise the shipping socket dispatcher on a disposable profile. The fixture
//! supplies peer classification so even a recognized Desktop is tested.
use super::*;
use crate::test_temp::event_hashes;
use serde_json::{Value, json};

fn send(context: &SearchContext, frame: &Value, desktop_peer: bool) -> Value {
    let (name, listener) = isolated_ipc_listener("purge");
    let context = context.clone();
    let server = thread::spawn(move || {
        let mut stream = listener.accept().unwrap();
        handle_connection_for_peer(&mut stream, Some(&context), None, None, None, desktop_peer)
            .unwrap();
    });
    let mut client = Stream::connect(name.borrow()).unwrap();
    let deadline = Instant::now() + IPC_RESPONSE_DEADLINE;
    let mut bytes = serde_json::to_vec(frame).unwrap();
    bytes.push(b'\n');
    write_ipc_all(&mut client, &bytes, deadline).unwrap();
    let reply = read_ipc_frame(&mut client, MAX_SEARCH_RESPONSE_BYTES, deadline).unwrap();
    server.join().unwrap();
    serde_json::from_slice(&reply).unwrap()
}

#[test]
fn unconfirmed_purge_requests_preserve_all_records() {
    let fixture = prepared_search_fixture("unconfirmed-purge");
    let target = append_memory_note(
        fixture.store.as_ref(),
        SecureStoreEnvironment::Dev,
        AppendMemoryNoteArgs {
            runtime_root: &fixture.root,
            brain_id: &fixture.brain_id,
            device_id: &fixture.device,
            classification: Classification::Restricted,
            text: "Disposable purge target",
            event_id: None,
            config: &CanonicalWriterConfig {
                seal_event_threshold: 1,
            },
        },
    )
    .unwrap();
    assert!(target.sealed);
    synchronize_local_derived_state(&fixture.context).unwrap();
    let read = serde_json::to_value(search_request(
        "desktop",
        vec![CAPABILITY_CONTENT_SEARCH.into()],
    ))
    .unwrap();
    let notes = || {
        let reply = send(&fixture.context, &read, true);
        assert_eq!(reply["accepted"], true);
        reply["hits"].as_array().unwrap().clone()
    };
    let before = event_hashes(&fixture.root);
    assert!(!before.is_empty());
    let original = notes();
    assert_eq!(original.len(), 2);
    assert!(
        original
            .iter()
            .any(|n| n["eventId"] == target.accepted.event_id
                && n["contentHash"] == target.accepted.content_hash.as_str())
    );
    let mut direct = read.clone();
    direct["operation"] = json!("memory.purge");
    direct["query"] = json!({"eventId": target.accepted.event_id});
    direct["capabilities"] = json!([CAPABILITY_STATUS_READ]);
    let mut claimed = direct.clone();
    claimed["confirmed"] = json!(true);
    let mut agent = direct.clone();
    agent["client_role"] = json!("mcp");
    agent["capabilities"] = json!(["memory.purge"]);
    let mut owner = serde_json::to_value(approval_request("owner.security.prepare")).unwrap();
    owner["role"] = Value::Null;
    owner["transcriptMessages"] = Value::Null;
    owner["transactionId"] = json!("aa".repeat(32));
    owner["ownerOperation"] = json!({"kind": "purge", "noteEventId": target.accepted.event_id});
    // The protected Mac owner channel has its own decoder in addition to the socket.
    assert!(matches!(
        crate::m8_owner_channel::request::decode_untrusted(&serde_json::to_vec(&owner).unwrap()),
        Err(crate::m8_owner_channel::ChannelError::MalformedRequest)
    ));
    let owner_error = if cfg!(target_os = "macos") {
        "mac_owner_channel_disabled"
    } else {
        "malformed_request"
    };
    for (frame, desktop, error) in [
        (direct, true, "unsupported_operation"),
        (claimed, true, "malformed_request"),
        (agent, false, "unauthorized_capability"),
        (owner, true, owner_error),
    ] {
        let reply = send(&fixture.context, &frame, desktop);
        assert_eq!(reply["accepted"], false);
        assert_eq!(reply["error"], error);
        assert_eq!(event_hashes(&fixture.root), before);
        assert_eq!(notes(), original);
    }
    fs::remove_dir_all(fixture.root).unwrap();
}
