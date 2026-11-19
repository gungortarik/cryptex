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
    let deadline = Instant::now() + Duration::from_secs(200);
    loop {
        if let Some(left) = exchanged(&source)
            && Some(left) == exchanged(&receiver)
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "both live signed replies must cross the native connection"
        );
        std::thread::sleep(Duration::from_millis(500));
    }
    // Allow two normal export ticks to authenticate the last delivered echo.
    std::thread::sleep(Duration::from_secs(12));
    for owner in [&mut owner_a, &mut owner_b] {
        assert_eq!(owner.handle().request(None).unwrap().error, None);
        owner.shutdown().unwrap();
    }
    drop((owner_a, owner_b, instance_a, instance_b));
    for root in [source, receiver, negative] {
        fs::remove_dir_all(root).unwrap();
    }
}
