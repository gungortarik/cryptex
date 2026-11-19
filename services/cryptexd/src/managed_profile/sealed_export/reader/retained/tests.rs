use super::*;

/// The CI driver runs this test twice in distinct processes under a disposable
/// account. A random run ID isolates the synthetic route from enrolled devices.
#[test]
#[ignore = "uses a disposable native credential store"]
fn native_observation_survives_process_exit() {
    #[cfg(windows)]
    let store = crate::WindowsCredentialStore::new();
    #[cfg(target_os = "macos")]
    let store = crate::MacosKeychainStore::new();
    let run = std::env::var("CRYPTEX_M9_READER_ATTEMPT").unwrap();
    assert!(run.len() == 32 && run.bytes().all(|b| b.is_ascii_hexdigit()));
    let route = Route {
        brain: format!("native-test-{run}"),
        sender: "synthetic-peer".into(),
        receiver: "synthetic-local".into(),
        sender_transport: "peer-transport".into(),
        receiver_transport: "local-transport".into(),
    };
    let slot = Slot::new((&store, SecureStoreEnvironment::Dev), &route).unwrap();
    let local = SigningKey::from_bytes(&[12; 32]);
    let remote = SigningKey::from_bytes(&[31; 32]);
    let hash = |n| ContentHashHex::from_bytes(&[n; 32]).unwrap();
    let load = || {
        slot.load(
            &hash(5),
            &route,
            &local.verifying_key().to_bytes(),
            &remote.verifying_key().to_bytes(),
            &hash(4),
            None,
        )
    };
    match std::env::var("CRYPTEX_M9_OBSERVATION_PHASE")
        .unwrap()
        .as_str()
    {
        "write" => {
            assert_eq!(load(), Ok(None));
            let reply = Body {
                route: route.clone(),
                nonce: hash(1),
                echo: Some(hash(2)),
                daemon_sha256: hash(3),
                resolved_blob: hash(4),
            };
            slot.save(
                &hash(5),
                &hash(2),
                &wire::sign(reply, &remote).unwrap(),
                &local,
            )
            .unwrap();
        }
        "reopen" => {
            assert_eq!(load(), Ok(Some(hash(3))));
            store.delete(&slot.service, &slot.account).unwrap();
            assert_eq!(load(), Ok(None));
        }
        _ => panic!("unknown observation phase"),
    }
}

#[test]
fn previous_observation_requires_both_keys_context_route_and_same_observed_build() {
    use crate::FakeSecureStore;
    let local = SigningKey::from_bytes(&[12; 32]);
    let remote = SigningKey::from_bytes(&[31; 32]);
    let route = Route {
        brain: "brain".into(),
        sender: "peer".into(),
        receiver: "local".into(),
        sender_transport: "peer transport".into(),
        receiver_transport: "local transport".into(),
    };
    let hash = |n| ContentHashHex::from_bytes(&[n; 32]).unwrap();
    let reply = Body {
        route: route.clone(),
        nonce: hash(1),
        echo: Some(hash(2)),
        daemon_sha256: hash(3),
        resolved_blob: hash(4),
    };
    let store = FakeSecureStore::new();
    let slot = Slot::new((&store, SecureStoreEnvironment::Dev), &route).unwrap();
    let signed = wire::sign(reply.clone(), &remote).unwrap();
    let local_key = local.verifying_key().to_bytes();
    let peer_key = remote.verifying_key().to_bytes();
    let read = |observed: Option<&Body>| {
        slot.load(&hash(5), &route, &local_key, &peer_key, &hash(4), observed)
    };
    assert_eq!(read(None), Ok(None));
    slot.save(&hash(5), &hash(2), &signed, &local).unwrap();
    assert_eq!(read(None), Ok(Some(hash(3))));
    assert_eq!(
        slot.load(&hash(6), &route, &local_key, &peer_key, &hash(4), None),
        Ok(None)
    );
    assert_eq!(
        slot.load(
            &hash(5),
            &route.reversed(),
            &local_key,
            &peer_key,
            &hash(4),
            None
        ),
        Err(Error::Message)
    );
    assert_eq!(
        slot.load(&hash(5), &route, &peer_key, &peer_key, &hash(4), None),
        Err(Error::Message)
    );
    assert_eq!(
        slot.load(&hash(5), &route, &local_key, &local_key, &hash(4), None),
        Err(Error::Message)
    );
    slot.save(&hash(5), &hash(7), &signed, &local).unwrap();
    assert_eq!(read(None), Err(Error::Message));
    slot.save(&hash(5), &hash(2), &signed, &local).unwrap();
    let changed = Body {
        daemon_sha256: hash(8),
        ..reply.clone()
    };
    assert_eq!(read(Some(&changed)), Ok(None));
    assert_eq!(read(None), Ok(None));
    slot.save(&hash(5), &hash(2), &signed, &local).unwrap();
    let changed = Body {
        resolved_blob: hash(9),
        ..reply
    };
    assert_eq!(read(Some(&changed)), Ok(None));
    assert_eq!(read(None), Ok(None));
}
