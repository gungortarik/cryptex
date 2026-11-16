use super::*;
use cryptex_events::TrustedDaemonKeyResolver;

const LOCAL: &str = "U2SYIGU-YWZX757-2WPZGCR-DHD7MLH-ZKV7HLJ-EYO6CXE-K3DU3DU-SDFXYAE";
const REMOTE: &str = "XAAPCAD-GA4SF4J-BNQL3FS-T456WKV-FGLXUKO-CDWHVDW-EYWNRIG-SM5GUAZ";
const BOOTSTRAP: &[u8] = include_bytes!("bootstrap-test.xml");

fn ports() -> NativePorts {
    NativePorts::new(18384, 22001).unwrap()
}
fn listen() -> SocketAddrV4 {
    "192.168.77.1:22001".parse().unwrap()
}
fn endpoints() -> BTreeMap<String, SocketAddrV4> {
    BTreeMap::from([(REMOTE.into(), "192.168.77.2:22002".parse().unwrap())])
}

fn with_authority(f: impl FnOnce(&ExchangePeers<'_>, &[u8])) {
    let (source, receiver, negative, store) =
        crate::trusted_import_probe::tests::pair_with_transport(LOCAL, REMOTE);
    {
        let brain = crate::identity_runtime::load_brain_identity(&source).unwrap();
        let resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
            &source,
            &brain.brain_id,
            &store,
            crate::SecureStoreEnvironment::Dev,
        )
        .unwrap();
        let snapshot =
            crate::knowledge_materialization::scan_verified_canonical_from_local_anchor_until(
                &source,
                &brain.brain_id,
                &resolver.local_trust_anchor().unwrap(),
                None,
            )
            .unwrap();
        let authority = ExchangePeers::from_snapshot(&snapshot, &source, LOCAL).unwrap();
        let bootstrap = inspect(BOOTSTRAP, ports(), true).unwrap();
        f(&authority, &bootstrap.bytes);
    }
    for root in [source, receiver, negative] {
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn signed_authority_builds_one_exact_exchange_and_rechecks_derived_binding() {
    with_authority(|authority, bootstrap| {
        let policy = ExchangeConfigPolicy::new(authority, ports(), listen(), endpoints()).unwrap();
        let bytes = policy.prepare_bootstrap(bootstrap).unwrap();
        let config = policy.inspect_exchange(&bytes).unwrap();
        assert_eq!(config.native_id, LOCAL);
        assert_eq!(config.key, inspect(bootstrap, ports(), false).unwrap().key);
        let tree = parse(&bytes).unwrap();
        let folder = tree.one("folder").unwrap();
        assert_eq!(folder.attrs["type"], "sendreceive");
        assert_eq!(
            folder.attrs["path"],
            authority
                .runtime_root()
                .join("transport/incoming")
                .to_str()
                .unwrap()
        );
        assert_eq!(
            tree.children.iter().filter(|n| n.name == "device").count(),
            2
        );
        let binding = policy.binding_bytes(&bytes).unwrap();
        policy.verify_binding(&bytes, &binding).unwrap();
        // Bootstrap remains strict; this capability is needed for active config.
        assert!(inspect(&bytes, ports(), false).is_err());
        assert!(policy.prepare_bootstrap(&bytes).is_err());
        let reordered = {
            let mut tree = tree.clone();
            tree.children.reverse();
            let mut writer = Writer::new(Vec::new());
            tree.write(&mut writer).unwrap();
            writer.into_inner()
        };
        policy.verify_binding(&reordered, &binding).unwrap();
        let reconstructed =
            ExchangeConfigPolicy::new(authority, ports(), listen(), endpoints()).unwrap();
        reconstructed.verify_binding(&bytes, &binding).unwrap();
        let mut changed_endpoints = endpoints();
        changed_endpoints.insert(REMOTE.into(), "192.168.77.3:22002".parse().unwrap());
        let different =
            ExchangeConfigPolicy::new(authority, ports(), listen(), changed_endpoints).unwrap();
        assert!(different.verify_binding(&bytes, &binding).is_err());
    });
}

#[test]
fn fresh_snapshot_reopens_same_binding_but_signed_retirement_refuses_old_plan() {
    use crate::bootstrap_journal::BootstrapJournalStore;
    let (source, receiver, negative, store) =
        crate::trusted_import_probe::tests::pair_with_transport(LOCAL, REMOTE);
    let brain = crate::identity_runtime::load_brain_identity(&source).unwrap();
    let resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
        &source,
        &brain.brain_id,
        &store,
        crate::SecureStoreEnvironment::Dev,
    )
    .unwrap();
    let anchor = resolver.local_trust_anchor().unwrap();
    let mut saved: Option<(Vec<u8>, Vec<u8>)> = None;
    // Drop all previous policy/snapshot locks between opens, then reconstruct
    // authority from canonical bytes. The receipt is merely an equality check.
    for _ in 0..2 {
        let snapshot =
            crate::knowledge_materialization::scan_verified_canonical_from_local_anchor_until(
                &source,
                &brain.brain_id,
                &anchor,
                None,
            )
            .unwrap();
        let authority = ExchangePeers::from_snapshot(&snapshot, &source, LOCAL).unwrap();
        let plan = ExchangeConfigPolicy::new(&authority, ports(), listen(), endpoints()).unwrap();
        if let Some((config, binding)) = &saved {
            plan.verify_binding(config, binding).unwrap();
        } else {
            let bootstrap = inspect(BOOTSTRAP, ports(), true).unwrap();
            let config = plan.prepare_bootstrap(&bootstrap.bytes).unwrap();
            let binding = plan.binding_bytes(&config).unwrap();
            saved = Some((config, binding));
        }
    }
    let snapshot =
        crate::knowledge_materialization::scan_verified_canonical_from_local_anchor_until(
            &source,
            &brain.brain_id,
            &anchor,
            None,
        )
        .unwrap();
    let (history, _) = snapshot.verified_trust_context().unwrap();
    let local = history.local_device.clone();
    let peers: Vec<_> = history.directly_approved_peers.iter().cloned().collect();
    let key_id = history.local_signing_key_id.clone();
    let public_key = history.local_public_key;
    drop(snapshot);
    let mut devices = peers;
    devices.push(local.clone());
    let security = crate::knowledge_materialization::scan_verified_trust_policy(
        &source,
        &brain.brain_id,
        &local,
        &key_id,
        &public_key,
        &devices,
    )
    .unwrap();
    let transaction = "ce".repeat(32);
    let approval = security
        .self_retirement_approval_snapshot(transaction.clone())
        .unwrap();
    drop(security);
    let journal = BootstrapJournalStore::new(&source).unwrap();
    journal
        .begin_owner_operation(approval.clone(), "99999999-9999-4999-8999-999999999995")
        .unwrap();
    journal
        .confirm_owner_operation(&transaction, &approval.digest().unwrap())
        .unwrap();
    crate::knowledge_materialization::append_confirmed_owner_security_operation(
        &source,
        &brain.brain_id,
        &store,
        crate::SecureStoreEnvironment::Dev,
        &transaction,
    )
    .unwrap();
    let snapshot =
        crate::knowledge_materialization::scan_verified_canonical_from_local_anchor_until(
            &source,
            &brain.brain_id,
            &anchor,
            None,
        )
        .unwrap();
    assert!(saved.is_some());
    assert!(matches!(
        ExchangePeers::from_snapshot(&snapshot, &source, LOCAL),
        Err(crate::managed_profile::exchange_policy::ExchangePolicyError::LocalRestricted)
    ));
    // No plan constructor takes a receipt instead of this required authority.
    drop(snapshot);
    drop(journal);
    for root in [source, receiver, negative] {
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn extra_missing_duplicate_and_modified_active_configuration_refuses() {
    with_authority(|authority, bootstrap| {
        let policy = ExchangeConfigPolicy::new(authority, ports(), listen(), endpoints()).unwrap();
        let bytes = policy.prepare_bootstrap(bootstrap).unwrap();
        let base = parse(&bytes).unwrap();
        let binding = policy.binding_bytes(&bytes).unwrap();
        let check = |tree: &Node| {
            let changed = canonical_bytes(tree).unwrap();
            assert!(policy.inspect_exchange(&changed).is_err());
            assert!(policy.verify_binding(&changed, &binding).is_err());
        };
        for name in ["folder", "device"] {
            let mut tree = base.clone();
            tree.children.push(
                tree.children
                    .iter()
                    .find(|n| n.name == name)
                    .unwrap()
                    .clone(),
            );
            check(&tree);
        }
        let mut tree = base.clone();
        tree.children
            .retain(|n| n.name != "device" || n.attrs.get("id").is_none_or(|id| id != REMOTE));
        check(&tree);
        for (key, value) in [
            ("type", "receiveonly"),
            ("path", "/canonical/events"),
            ("fsWatcherEnabled", "true"),
            ("introducer", "true"),
        ] {
            let mut tree = base.clone();
            tree.one_mut("folder")
                .unwrap()
                .attrs
                .insert(key.into(), value.into());
            check(&tree);
        }
        for (name, value) in [
            ("versioning", "unsafe"),
            ("filesystemType", "fake"),
            ("disableFsync", "true"),
            ("junctionsAsDirs", "true"),
        ] {
            let mut tree = base.clone();
            let child = tree.one_mut("folder").unwrap().one_mut(name).unwrap();
            child.children.clear();
            child.text = value.into();
            check(&tree);
        }
        for (key, value) in [
            ("id", LOCAL),
            ("introducer", "true"),
            ("introducedBy", LOCAL),
            ("unrecognized", "true"),
        ] {
            let mut tree = base.clone();
            tree.children
                .iter_mut()
                .find(|n| n.name == "device" && n.attrs.get("id").is_some_and(|id| id == REMOTE))
                .unwrap()
                .attrs
                .insert(key.into(), value.into());
            check(&tree);
        }
        for (option, value) in [
            ("listenAddress", "tcp://0.0.0.0:22001"),
            ("globalAnnounceEnabled", "true"),
            ("localAnnounceEnabled", "true"),
            ("relaysEnabled", "true"),
            ("natEnabled", "true"),
            ("overwriteRemoteDeviceNamesOnConnect", "true"),
        ] {
            let mut tree = base.clone();
            tree.one_mut("options").unwrap().set(option, value).unwrap();
            check(&tree);
        }
    });
}

#[test]
fn mutable_defaults_are_never_promoted_and_receipt_drift_is_not_adopted() {
    with_authority(|authority, bootstrap| {
        let policy = ExchangeConfigPolicy::new(authority, ports(), listen(), endpoints()).unwrap();
        let mut base = parse(bootstrap).unwrap();
        let defaults = base.one_mut("defaults").unwrap();
        defaults
            .one_mut("folder")
            .unwrap()
            .attrs
            .insert("path".into(), "/private/unsafe".into());
        defaults
            .one_mut("folder")
            .unwrap()
            .one_mut("versioning")
            .unwrap()
            .attrs
            .insert("type".into(), "external".into());
        let bytes = policy
            .prepare_bootstrap(&canonical_bytes(&base).unwrap())
            .unwrap();
        assert_eq!(
            canonical_bytes(parse(&bytes).unwrap().one("folder").unwrap()).unwrap(),
            canonical_bytes(&policy.folder).unwrap()
        );
        let binding = policy.binding_bytes(&bytes).unwrap();
        for field in [
            "schema",
            "brain_id",
            "local_transport",
            "incoming_path",
            "folder_id",
            "engine_version",
            "config_sha256",
        ] {
            let mut value: serde_json::Value = serde_json::from_slice(&binding).unwrap();
            value[field] = if field == "schema" {
                2.into()
            } else {
                "changed".into()
            };
            assert!(
                policy
                    .verify_binding(&bytes, &serde_json::to_vec(&value).unwrap())
                    .is_err()
            );
        }
        let duplicate =
            String::from_utf8(binding.clone())
                .unwrap()
                .replacen('{', "{\"schema\":1,", 1);
        assert!(policy.verify_binding(&bytes, duplicate.as_bytes()).is_err());
        let mut unknown: serde_json::Value = serde_json::from_slice(&binding).unwrap();
        unknown["extra"] = true.into();
        assert!(
            policy
                .verify_binding(&bytes, &serde_json::to_vec(&unknown).unwrap())
                .is_err()
        );
        assert!(
            policy
                .verify_binding(&bytes, &vec![b' '; MAX_BINDING + 1])
                .is_err()
        );
        let mut changed = parse(&bytes).unwrap();
        changed
            .one_mut("defaults")
            .unwrap()
            .one_mut("folder")
            .unwrap()
            .attrs
            .insert("label".into(), "changed inactive template".into());
        let changed = canonical_bytes(&changed).unwrap();
        policy.inspect_exchange(&changed).unwrap();
        assert!(policy.verify_binding(&changed, &binding).is_err());
    });
}

#[test]
fn endpoint_set_requires_exact_trusted_peers_and_explicit_private_ipv4() {
    with_authority(|authority, _| {
        for address in [
            "0.0.0.0:22001",
            "127.0.0.1:22001",
            "8.8.8.8:22001",
            "192.168.77.1:0",
            "192.168.77.1:22002",
        ] {
            assert!(
                ExchangeConfigPolicy::new(
                    authority,
                    ports(),
                    address.parse().unwrap(),
                    endpoints()
                )
                .is_err()
            );
        }
        assert!(ExchangeConfigPolicy::new(authority, ports(), listen(), BTreeMap::new()).is_err());
        let mut extra = endpoints();
        extra.insert(LOCAL.into(), "192.168.77.3:22003".parse().unwrap());
        assert!(ExchangeConfigPolicy::new(authority, ports(), listen(), extra).is_err());
        for address in [
            "0.0.0.0:22002",
            "127.0.0.1:22002",
            "8.8.8.8:22002",
            "192.168.77.2:0",
            "192.168.77.1:22001",
        ] {
            assert!(
                ExchangeConfigPolicy::new(
                    authority,
                    ports(),
                    listen(),
                    BTreeMap::from([(REMOTE.into(), address.parse().unwrap())])
                )
                .is_err()
            );
        }
    });
}

/// Real pinned serializer/startup check, with an empty disposable exchange and
/// loopback-only endpoints. This does not claim real peer transfer. No product
/// constructor admits loopback; the private field adjustment exists only here.
#[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
#[test]
#[ignore = "requires explicit pinned native package; opens isolated loopback listeners"]
fn pinned_native_engine_preserves_exchange_config_on_two_starts() {
    use crate::syncthing_owned_rest::{EndpointError, OwnedRestError, OwnedSyncthingRest};
    use crate::syncthing_package::PinnedSyncthingPackage;
    use std::net::{Ipv4Addr, TcpListener};
    use std::time::{Duration, Instant};
    let package = PinnedSyncthingPackage::open(&std::path::PathBuf::from(
        std::env::var_os("CRYPTEX_EXCHANGE_NATIVE_PACKAGE")
            .expect("explicit pinned package required"),
    ))
    .unwrap();
    let home = crate::test_temp::exclusive_temp_dir("exchange-native-schema");
    let mut generation = package.spawn_generate(&home).unwrap();
    assert!(
        generation
            .wait_for_exit(Duration::from_secs(15))
            .unwrap()
            .is_clean()
    );
    drop(generation);
    let api = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let sync = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let peer_listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let ports = NativePorts::new(
        api.local_addr().unwrap().port(),
        sync.local_addr().unwrap().port(),
    )
    .unwrap();
    let bootstrap = inspect(
        &std::fs::read(home.join("config.xml")).unwrap(),
        ports,
        true,
    )
    .unwrap();
    let (source, receiver, negative, store) =
        crate::trusted_import_probe::tests::pair_with_transport(&bootstrap.native_id, REMOTE);
    let brain = crate::identity_runtime::load_brain_identity(&source).unwrap();
    let resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
        &source,
        &brain.brain_id,
        &store,
        crate::SecureStoreEnvironment::Dev,
    )
    .unwrap();
    let anchor = resolver.local_trust_anchor().unwrap();
    std::fs::create_dir_all(source.join("transport/incoming")).unwrap();
    drop(api);
    drop(sync);
    let mut saved: Option<Vec<u8>> = None;
    for attempt in 0..2 {
        let snapshot =
            crate::knowledge_materialization::scan_verified_canonical_from_local_anchor_until(
                &source,
                &brain.brain_id,
                &anchor,
                None,
            )
            .unwrap();
        let authority =
            ExchangePeers::from_snapshot(&snapshot, &source, &bootstrap.native_id).unwrap();
        let mut policy = ExchangeConfigPolicy::new(
            &authority,
            ports,
            SocketAddrV4::new("192.168.77.1".parse().unwrap(), ports.sync),
            endpoints(),
        )
        .unwrap();
        policy.listen = SocketAddrV4::new(Ipv4Addr::LOCALHOST, ports.sync);
        let peer_endpoint = SocketAddrV4::new(
            Ipv4Addr::LOCALHOST,
            peer_listener.local_addr().unwrap().port(),
        );
        policy.endpoints.insert(REMOTE.into(), peer_endpoint);
        policy.peers[0]
            .set("address", &format!("tcp://{peer_endpoint}"))
            .unwrap();
        if attempt == 0 {
            let config = policy.prepare_bootstrap(&bootstrap.bytes).unwrap();
            saved = Some(policy.binding_bytes(&config).unwrap());
            std::fs::write(home.join("config.xml"), config).unwrap();
        }
        policy
            .verify_binding(
                &std::fs::read(home.join("config.xml")).unwrap(),
                saved.as_ref().unwrap(),
            )
            .unwrap();
        let mut process = package.spawn_serve(&home).unwrap();
        {
            let rest = OwnedSyncthingRest::new(
                process.process(),
                ports.api_endpoint(),
                &bootstrap.native_id,
                ApiCredential::from_bytes(&bootstrap.key).unwrap(),
            )
            .unwrap();
            let deadline = Instant::now() + Duration::from_secs(12);
            loop {
                match rest.observe() {
                    Ok(_) => break,
                    Err(
                        OwnedRestError::Endpoint(EndpointError::Unavailable)
                        | OwnedRestError::Unavailable
                        | OwnedRestError::Deadline,
                    ) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
                    Err(error) => panic!("owned native readiness failed: {error}"),
                }
            }
            policy
                .verify_binding(
                    &std::fs::read(home.join("config.xml")).unwrap(),
                    saved.as_ref().unwrap(),
                )
                .unwrap();
            rest.request_shutdown().unwrap();
        }
        assert!(
            process
                .wait_for_exit(Duration::from_secs(10))
                .unwrap()
                .is_clean()
        );
        drop(process);
        policy
            .verify_binding(
                &std::fs::read(home.join("config.xml")).unwrap(),
                saved.as_ref().unwrap(),
            )
            .unwrap();
    }
    drop(peer_listener);
    for root in [home, source, receiver, negative] {
        std::fs::remove_dir_all(root).unwrap();
    }
}
