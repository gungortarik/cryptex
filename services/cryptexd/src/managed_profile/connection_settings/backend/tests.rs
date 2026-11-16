use super::*;
const LOCAL: &str = "U2SYIGU-YWZX757-2WPZGCR-DHD7MLH-ZKV7HLJ-EYO6CXE-K3DU3DU-SDFXYAE";
const REMOTE: &str = "XAAPCAD-GA4SF4J-BNQL3FS-T456WKV-FGLXUKO-CDWHVDW-EYWNRIG-SM5GUAZ";

#[test]
fn preferences_require_current_canonical_binding_and_revision() {
    let (source, receiver, negative, store) =
        crate::trusted_import_probe::tests::pair_with_transport(LOCAL, REMOTE);
    let brain = crate::identity_runtime::load_brain_identity(&source).unwrap();
    let resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
        &source,
        &brain.brain_id,
        &store,
        SecureStoreEnvironment::Dev,
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
    let peers = ExchangePeers::from_snapshot(&snapshot, &source, LOCAL).unwrap();
    assert_eq!(peers.bindings().len(), 1);
    let id = peers.bindings()[0].device_id.as_str().to_owned();
    let update = Update {
        expected_revision: "0".into(),
        paused: false,
        listen_address: Some("192.168.77.1".parse().unwrap()),
        peers: vec![PeerAddress {
            device_id: id,
            endpoint: "192.168.77.2:22002".parse().unwrap(),
        }],
    };
    let original = Preferences::default();
    assert!(original.endpoints(&peers).is_none());
    let saved = original.update(&update, &peers, 22001).unwrap();
    assert_eq!(saved.revision, 1);
    assert_eq!(saved.endpoints(&peers).unwrap().len(), 1);
    assert!(matches!(
        saved.update(&update, &peers, 22001),
        Err(Error::StaleRevision)
    ));
    let mut wrong = update.clone();
    wrong.peers[0].device_id = "99999999-9999-4999-8999-999999999999".into();
    assert!(matches!(
        original.update(&wrong, &peers, 22001),
        Err(Error::UnknownDevice)
    ));
    wrong = update.clone();
    wrong.peers[0].endpoint = "192.168.77.1:22001".parse().unwrap();
    assert!(matches!(
        original.update(&wrong, &peers, 22001),
        Err(Error::InvalidEndpoint)
    ));
    let mut changed_association = saved.clone();
    changed_association.peers[0].transport_id = LOCAL.into();
    assert!(changed_association.endpoints(&peers).is_none());
    // Pausing removes network authorization, not signed history or peer rows.
    let mut pause = update.clone();
    pause.expected_revision = "1".into();
    pause.paused = true;
    assert_eq!(
        saved
            .update(&pause, &peers, 22001)
            .unwrap()
            .devices(&peers)
            .len(),
        1
    );
    drop(snapshot);
    let resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
        &receiver,
        &brain.brain_id,
        &store,
        SecureStoreEnvironment::Dev,
    )
    .unwrap();
    let snapshot =
        crate::knowledge_materialization::scan_verified_canonical_from_local_anchor_until(
            &receiver,
            &brain.brain_id,
            &resolver.local_trust_anchor().unwrap(),
            None,
        )
        .unwrap();
    let restricted = ExchangePeers::from_snapshot(&snapshot, &receiver, REMOTE).unwrap();
    assert!(restricted.bindings().is_empty());
    assert!(saved.devices(&restricted).is_empty());
    assert!(
        ExchangeConfigPolicy::local_only(&restricted, NativePorts::new(18384, 22001).unwrap())
            .is_ok()
    );
    drop(snapshot);
    for root in [source, receiver, negative] {
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn persisted_settings_survive_reopen_but_staging_and_corruption_refuse() {
    use std::io::Write;
    let root = crate::test_temp::exclusive_temp_dir("connection-settings-store");
    {
        let settings = SettingsStore::open(&root).unwrap();
        let default = settings.load().unwrap();
        assert!(default.paused);
        assert_eq!(default.revision, 0);
        settings
            .save(&Preferences {
                revision: 7,
                ..default
            })
            .unwrap();
    }
    {
        let settings = SettingsStore::open(&root).unwrap();
        assert_eq!(settings.load().unwrap().revision, 7);
        let mut stage = settings
            .directory
            .create_file(Leaf::Settings.temporary())
            .unwrap();
        stage.write_all(b"{}").unwrap();
        drop(stage);
        assert!(matches!(settings.load(), Err(Error::PendingSettings)));
    }
    assert!(Preferences::decode(br#"{"schema":1,"schema":1}"#).is_err());
    assert!(Preferences::decode(&vec![b' '; MAX_FRAME + 1]).is_err());
    let mut value = serde_json::to_value(Preferences::default()).unwrap();
    value["schema"] = serde_json::json!(2);
    assert!(Preferences::decode(&serde_json::to_vec(&value).unwrap()).is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn exchange_tree_contains_no_session_files_and_keeps_original_namespace() {
    let root = crate::test_temp::exclusive_temp_dir("connection-tree");
    let tree = crate::managed_profile::platform::ensure_exchange_tree(&root).unwrap();
    tree.validate().unwrap();
    assert_eq!(
        std::fs::read_dir(root.join("transport/incoming"))
            .unwrap()
            .count(),
        0
    );
    assert!(!root.join("transport/session.lock").exists());
    #[cfg(windows)]
    assert!(std::fs::rename(root.join("transport"), root.join("old-transport")).is_err());
    drop(tree);
    let second = crate::managed_profile::platform::ensure_exchange_tree(&root).unwrap();
    second.validate().unwrap();
    drop(second);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unenrolled_coordinator_never_opens_a_profile_or_creates_settings() {
    let root = crate::test_temp::exclusive_temp_dir("connections-unenrolled");
    let instance = crate::instance::try_acquire_instance(&root).unwrap();
    let mut owner = Owner::start(
        &root,
        &instance,
        Arc::new(crate::FakeSecureStore::new()),
        SecureStoreEnvironment::Dev,
        None,
        crate::approval_session::ApprovalSessionRegistry::default(),
        None,
    )
    .unwrap();
    assert_eq!(
        owner.handle().request(None).unwrap().state,
        State::NotEnrolled
    );
    assert!(!root.join("connection-settings").exists());
    owner.shutdown().unwrap();
    drop(owner);
    drop(instance);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn queued_or_cancelled_request_cannot_begin_later() {
    let stop = AtomicBool::new(false);
    let interest = Interest::new();
    interest.check(&stop).unwrap();
    drop(RequestLifetime(interest.live.clone()));
    assert_eq!(interest.check(&stop), Err(Error::Unavailable));
    let mut expired = Interest::new();
    expired.deadline = Instant::now();
    assert_eq!(expired.check(&stop), Err(Error::Unavailable));
    let interest = Interest::new();
    stop.store(true, Ordering::Release);
    assert_eq!(interest.check(&stop), Err(Error::Unavailable));
}
