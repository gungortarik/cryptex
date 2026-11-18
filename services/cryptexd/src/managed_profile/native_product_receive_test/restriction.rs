//! Three disposable identities, genuine native delivery, and daemon-written
//! approval/restriction records. No canonical or inbox copies between peers.
use super::*;
use cryptex_core::DeviceId;
use cryptex_events::BootstrapApprovalRole;

const BRAIN: &str = "88888888-8888-4888-8888-888888888888";

fn pair(roots: [&Path; 2], transports: [&str; 2], store: &Stores, index: usize) {
    let identities = roots
        .map(|root| crate::local_device_identity::load_local_device_identity(root, BRAIN).unwrap());
    let devices = identities.map(|identity| DeviceId::new(identity.device_id).unwrap());
    let keys = [0, 1].map(|side| {
        crate::event_signing_key::load_existing_event_signing_key(
            store,
            ENV,
            roots[side],
            &devices[side],
        )
        .unwrap()
    });
    let transaction = format!("{index:064x}");
    let offer = cryptex_events::create_bootstrap_offer(
        BRAIN,
        &transaction,
        &devices[0],
        transports[0],
        &[11; 32],
        &keys[0].signing_key,
    )
    .unwrap();
    let proof = cryptex_events::create_bootstrap_proof(
        &offer,
        &devices[1],
        transports[1],
        &[12; 32],
        &keys[1].signing_key,
    )
    .unwrap();
    let request_a = cryptex_events::verify_bootstrap_approval_request(
        &[&offer, &proof],
        BootstrapApprovalRole::Initiator,
    )
    .unwrap();
    let approval_a =
        cryptex_events::sign_bootstrap_approval(&request_a, &keys[0].signing_key).unwrap();
    let request_b = cryptex_events::verify_bootstrap_approval_request(
        &[&offer, &proof, &approval_a],
        BootstrapApprovalRole::Responder,
    )
    .unwrap();
    let approval_b =
        cryptex_events::sign_bootstrap_approval(&request_b, &keys[1].signing_key).unwrap();
    let messages = vec![offer, proof, approval_a, approval_b];
    for (side, request, prefix) in [(0, request_a, 2), (1, request_b, 3)] {
        let journal = crate::bootstrap_journal::BootstrapJournalStore::new(roots[side]).unwrap();
        let event_id = format!("aaaaaaaa-aaaa-4aaa-8aaa-{:012x}", index * 2 + side);
        journal
            .begin_verified_approval_prefix(&request, &messages[..prefix], &event_id)
            .unwrap();
        // Synthetic owner confirmation seam; not a Desktop approval assertion.
        journal
            .record_local_approval_signature(&request, &messages[prefix])
            .unwrap();
        journal
            .record_verified_complete_transcript(&transaction, &messages)
            .unwrap();
        crate::knowledge_materialization::append_local_peer_approval_anchor(
            roots[side],
            BRAIN,
            store,
            ENV,
            &transaction,
            &messages,
        )
        .unwrap();
    }
}

fn configure(owner: &Owner, ip: Ipv4Addr, natives: &[Native; 3]) -> Update {
    let view = owner.handle().request(None).unwrap();
    assert_eq!(view.error, None);
    assert_eq!(view.devices.len(), 2);
    let update = Update {
        expected_revision: view.revision,
        paused: false,
        listen_address: Some(ip),
        peers: view
            .devices
            .into_iter()
            .map(|device| {
                let native = natives
                    .iter()
                    .find(|native| native.identity == device.transport_id)
                    .unwrap();
                PeerAddress {
                    device_id: device.device_id,
                    endpoint: SocketAddrV4::new(ip, native.ports.sync),
                }
            })
            .collect(),
    };
    let view = owner.handle().request(Some(update.clone())).unwrap();
    assert_eq!(view.error, None);
    assert_eq!(view.state, State::ReadyLocally);
    update
}

fn retained_peer(owner: &Owner, expected: &DeviceId) -> bool {
    let view = owner.handle().request(None).unwrap();
    assert_eq!(
        view.error, None,
        "remaining authorized exchange must stay available"
    );
    view.state == State::ReadyLocally
        && view.devices.len() == 1
        && view.devices[0].device_id == expected.as_str()
}

#[test]
#[ignore = "requires explicit pinned package, private CI interface and disposable OS credentials"]
fn restriction_propagates_to_remaining_peer_and_survives_restart() {
    let package_path = PathBuf::from(
        std::env::var_os("CRYPTEX_EXCHANGE_NATIVE_PACKAGE").expect("explicit pinned package"),
    );
    let package = crate::syncthing_package::PinnedSyncthingPackage::open(&package_path).unwrap();
    let route = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).unwrap();
    route.connect("192.0.2.1:9").unwrap();
    let std::net::IpAddr::V4(ip) = route.local_addr().unwrap().ip() else {
        panic!("IPv4 required")
    };
    assert!(ip.is_private());
    let natives = [
        Native::new(&package, ip),
        Native::new(&package, ip),
        Native::new(&package, ip),
    ];
    let store = Arc::new(Stores {
        canonical: crate::FakeSecureStore::new(),
        transport: natives.iter().map(|native| native.labels.clone()).collect(),
    });
    let roots = ["restriction-a", "restriction-b", "restriction-c"]
        .map(crate::test_temp::exclusive_temp_dir);
    let devices = roots.each_ref().map(|root| {
        crate::identity_runtime::write_brain_identity(
            root,
            &cryptex_core::BrainIdentity::new(BRAIN, "2026-11-17T00:00:00Z").unwrap(),
        )
        .unwrap();
        let identity =
            crate::local_device_identity::enroll_local_device(root, store.as_ref(), ENV).unwrap();
        note(root, &store, "retained pre-restriction note");
        DeviceId::new(identity.device_id).unwrap()
    });
    for (index, a, b) in [(1, 0, 1), (2, 0, 2), (3, 1, 2)] {
        pair(
            [&roots[a], &roots[b]],
            [&natives[a].identity, &natives[b].identity],
            &store,
            index,
        );
    }
    for (index, root) in roots.iter().enumerate() {
        note(root, &store, &format!("original peer {index}"));
    }
    let mut owners = Vec::new();
    let mut instances = Vec::new();
    for (native, root) in natives.iter().zip(&roots) {
        let (owner, instance) = native.start(root, &package_path, store.clone());
        owners.push(owner);
        instances.push(instance);
    }
    let stale = owners
        .iter()
        .map(|owner| configure(owner, ip, &natives))
        .collect::<Vec<_>>();
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        if roots.iter().zip(&instances).all(|(root, instance)| {
            (0..3).all(|index| found(root, instance, &store, &format!("original peer {index}")))
        }) {
            break;
        }
        for owner in &owners {
            assert_eq!(owner.handle().request(None).unwrap().error, None);
        }
        assert!(
            Instant::now() < deadline,
            "three peers did not converge before restriction"
        );
        std::thread::sleep(Duration::from_millis(500));
    }
    for owner in &owners {
        pause(owner, true);
    }
    for owner in &mut owners {
        owner.shutdown().unwrap();
    }
    drop((owners, instances));
    let before = canonical(&roots[0]);
    assert_eq!(before, canonical(&roots[1]));
    assert_eq!(before, canonical(&roots[2]));

    // B is offline. Its approved prefix is already on A/C. Only A's daemon
    // creates the new decision; C must learn it through normal network import.
    let (mut a, ia) = natives[0].start(&roots[0], &package_path, store.clone());
    let (mut c, ic) = natives[2].start(&roots[2], &package_path, store.clone());
    pause(&a, false);
    pause(&c, false);
    security_change_for(&roots[0], &ia, &store, false, Some(devices[1].clone()));
    let deadline = Instant::now() + Duration::from_secs(180);
    while !retained_peer(&a, &devices[2]) || !retained_peer(&c, &devices[0]) {
        assert!(
            Instant::now() < deadline,
            "restriction did not reach remaining peer"
        );
        std::thread::sleep(Duration::from_millis(500));
    }
    // Preserve current revision, but deliberately submit the old roster: this
    // must fail on authority, not just on an obsolete settings revision.
    let mut old_roster = stale[2].clone();
    old_roster.expected_revision = c.handle().request(None).unwrap().revision;
    assert!(matches!(
        c.handle().request(Some(old_roster)),
        Err(connection_settings::Error::UnknownDevice)
    ));
    assert!(retained_peer(&c, &devices[0]));
    assert!(found(&roots[2], &ic, &store, "original peer 1"));
    note(&roots[0], &store, "authorized traffic after restriction");
    let deadline = Instant::now() + Duration::from_secs(180);
    while !found(
        &roots[2],
        &ic,
        &store,
        "authorized traffic after restriction",
    ) {
        assert!(retained_peer(&a, &devices[2]));
        assert!(retained_peer(&c, &devices[0]));
        assert!(
            Instant::now() < deadline,
            "remaining peers stopped exchanging after restriction"
        );
        std::thread::sleep(Duration::from_millis(500));
    }
    pause(&a, true);
    pause(&c, true);
    a.shutdown().unwrap();
    c.shutdown().unwrap();
    drop((a, c, ia, ic));
    let after = canonical(&roots[0]);
    assert_eq!(after, canonical(&roots[2]));
    for previous in &before {
        assert!(after.contains(previous), "accepted history changed");
    }
    assert_eq!(
        before,
        canonical(&roots[1]),
        "offline peer was not modified by fixture"
    );
    let (mut a, ia) = natives[0].start(&roots[0], &package_path, store.clone());
    let (mut c, ic) = natives[2].start(&roots[2], &package_path, store.clone());
    for (owner, expected) in [(&a, &devices[2]), (&c, &devices[0])] {
        let view = owner.handle().request(None).unwrap();
        assert_eq!(view.state, State::Paused);
        assert_eq!(view.error, None);
        assert_eq!(view.devices.len(), 1);
        assert_eq!(view.devices[0].device_id, expected.as_str());
    }
    assert!(found(&roots[2], &ic, &store, "original peer 1"));
    assert!(found(
        &roots[2],
        &ic,
        &store,
        "authorized traffic after restriction"
    ));
    assert_eq!(after, canonical(&roots[0]));
    assert_eq!(after, canonical(&roots[2]));
    a.shutdown().unwrap();
    c.shutdown().unwrap();
    drop((a, c, ia, ic, store));
    for root in roots {
        std::fs::remove_dir_all(root).unwrap();
    }
}
