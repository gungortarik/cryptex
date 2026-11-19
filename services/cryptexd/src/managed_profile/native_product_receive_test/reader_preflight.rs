use super::*;
use crate::import_staging::READER_CONTROL_FOLDER;
use std::collections::BTreeMap;
use std::fs;

/// Both inboxes must contain the same two directional messages, each
/// answering the other's live nonce. The CI harness also requires both
/// controllers' independent signature/reader verification diagnostics.
fn exchanged(root: &Path) -> Option<BTreeMap<String, String>> {
    let mut challenge = BTreeMap::new();
    for entry in fs::read_dir(root.join("transport/incoming").join(READER_CONTROL_FOLDER)).ok()? {
        let entry = entry.ok()?;
        if entry.path().extension().is_none_or(|ext| ext != "control") {
            continue;
        }
        let msg: serde_json::Value = serde_json::from_slice(&fs::read(entry.path()).ok()?).ok()?;
        let body = &msg["body"];
        challenge.insert(
            body["nonce"].as_str()?.to_owned(),
            body["echo"].as_str()?.to_owned(),
        );
    }
    (challenge.len() == 2
        && challenge
            .iter()
            .all(|(nonce, echo)| nonce != echo && challenge.get(echo) == Some(nonce)))
    .then_some(challenge)
}

#[test]
#[ignore = "requires a pinned native worker and disposable Windows credentials"]
fn normal_owners_verify_readers_over_the_private_network() {
    let path = PathBuf::from(std::env::var_os("CRYPTEX_EXCHANGE_NATIVE_PACKAGE").unwrap());
    let package = crate::syncthing_package::PinnedSyncthingPackage::open(&path).unwrap();
    let route = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).unwrap();
    route.connect("192.0.2.1:9").unwrap();
    let std::net::IpAddr::V4(ip) = route.local_addr().unwrap().ip() else {
        panic!("IPv4 required")
    };
    assert!(ip.is_private());
    let a = Native::new(&package, ip);
    let b = Native::new(&package, ip);
    let (source, receiver, negative, canonical) =
        crate::trusted_import_probe::tests::pair_with_transport(&a.identity, &b.identity);
    for root in [&source, &receiver] {
        drop(crate::instance::try_acquire_instance(root).unwrap());
        crate::instance::migration::migrate_profile(root).unwrap();
    }
    let store = Arc::new(Stores {
        canonical,
        transport: BTreeSet::from([a.labels.clone(), b.labels.clone()]),
    });
    let (mut owner_a, instance_a) = a.start(&source, &path, store.clone());
    let (mut owner_b, instance_b) = b.start(&receiver, &path, store.clone());
    connect(&owner_a, ip, b.ports.sync);
    connect(&owner_b, ip, a.ports.sync);
    let attempt = std::env::var("CRYPTEX_M9_READER_ATTEMPT").expect("CI reader receipt attempt");
    let receipt = |root: &Path| {
        let file = root
            .join("transport/m9-control-staging")
            .join(format!("{attempt}.receipt"));
        let bytes = fs::read(file).ok()?;
        serde_json::from_slice::<serde_json::Value>(&bytes).ok()
    };
    let wait_for_exchange = |old_nonce: Option<&serde_json::Value>| {
        let deadline = Instant::now() + Duration::from_secs(200);
        loop {
            if let Some(left) = exchanged(&source)
                && Some(left.clone()) == exchanged(&receiver)
                && let (Some(a), Some(b)) = (receipt(&source), receipt(&receiver))
                && let (Some(nonce), Some(echo)) =
                    (a["local_nonce"].as_str(), a["peer_nonce"].as_str())
                && left.get(nonce).is_some_and(|value| value == echo)
                && a["local_nonce"] == b["peer_nonce"]
                && a["peer_nonce"] == b["local_nonce"]
                && Some(&a["local_nonce"]) != old_nonce
            {
                for (local, peer) in [("local", "peer"), ("local_build", "peer_build")] {
                    assert!(a[local].is_string() && b[local].is_string());
                    assert_eq!(a[local], b[peer]);
                    assert_eq!(b[local], a[peer]);
                }
                assert_eq!(a["attempt"], attempt);
                assert_eq!(b["attempt"], attempt);
                break a["local_nonce"].clone();
            }
            assert!(
                Instant::now() < deadline,
                "live controls and receipts must agree"
            );
            std::thread::sleep(Duration::from_millis(500));
        }
    };
    let previous = wait_for_exchange(None);
    owner_a.shutdown().unwrap();
    drop((owner_a, instance_a));
    pause(&owner_b, true);
    // All path changes below are confined to stopped disposable fixtures.
    // Preserve the original beside the staged copy. The enrolled worker
    // reopens the same path and settings it uses on an ordinary restart.
    let restored = crate::test_temp::stage_stopped_m9_profile(&source, &negative, "restore");
    let original = canonical_files(&source, true);
    let parked = negative.join("original");
    fs::rename(&source, &parked).unwrap();
    fs::rename(restored, &source).unwrap();
    const AFTER_BACKUP: &str = "peer note after reader backup";
    note(&receiver, &store, AFTER_BACKUP);
    let (mut owner_a, instance_a) = a.start(&source, &path, store.clone());
    let ready = owner_a.handle().request(None).unwrap();
    assert_eq!(ready.state, State::ReadyLocally);
    assert_eq!(ready.error, None);
    let deadline = Instant::now() + Duration::from_secs(60);
    while receipt(&source).is_some() {
        assert!(
            Instant::now() < deadline,
            "restored receipt survived offline startup"
        );
        std::thread::sleep(Duration::from_millis(250));
    }
    assert!(!found(&source, &instance_a, &store, AFTER_BACKUP));
    pause(&owner_b, false);
    wait_for_exchange(Some(&previous));
    let deadline = Instant::now() + Duration::from_secs(180);
    while !found(&source, &instance_a, &store, AFTER_BACKUP) {
        assert!(
            Instant::now() < deadline,
            "verified import did not catch up"
        );
        std::thread::sleep(Duration::from_millis(500));
    }
    assert_eq!(canonical_files(&parked, true), original);
    for owner in [&mut owner_a, &mut owner_b] {
        assert_eq!(owner.handle().request(None).unwrap().error, None);
        owner.shutdown().unwrap();
    }
    drop((owner_a, owner_b, instance_a, instance_b));
    for root in [source, receiver, negative] {
        fs::remove_dir_all(root).unwrap();
    }
}
