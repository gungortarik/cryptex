use super::*;
const LOCAL: &str = "U2SYIGU-YWZX757-2WPZGCR-DHD7MLH-ZKV7HLJ-EYO6CXE-K3DU3DU-SDFXYAE";
const REMOTE: &str = "XAAPCAD-GA4SF4J-BNQL3FS-T456WKV-FGLXUKO-CDWHVDW-EYWNRIG-SM5GUAZ";

#[test]
fn closed_worker_cannot_leave_a_restricted_peer_in_settings() {
    // Exercise both orderings without sleeping. A restriction can invalidate
    // the guard during the identity read, or after the transport has stopped.
    for restrict_before_close in [true, false] {
        let root = crate::test_temp::exclusive_temp_dir("settings-restriction-generation");
        let admission = RuntimeAdmission::detached_for_tests(&root);
        let exclusive = || {
            admission
                .exclusive(&root, Instant::now() + Duration::from_secs(1), false)
                .unwrap()
        };
        let snapshot = exclusive();
        let guard = snapshot.transport_guard(&root).unwrap();
        snapshot.finish(None);
        let mut controller = Controller {
            root: root.clone(),
            admission: admission.clone(),
            store: Arc::new(crate::FakeSecureStore::default()),
            environment: SecureStoreEnvironment::Dev,
            // With no enrolled location, the rescan must withhold the old list.
            location: None,
            registry: Default::default(),
            settings: None,
            preferences: Preferences::default(),
            worker: None,
            guard: Some(guard),
            reader: Arc::new(Mutex::new(None)),
            view: View {
                revision: "7".into(),
                paused: true,
                listen_address: None,
                local_transport_port: None,
                devices: vec![Device {
                    device_id: "other".into(),
                    transport_id: REMOTE.into(),
                    endpoint: None,
                }],
                state: State::Paused,
                error: None,
            },
            cleanup_failed: false,
            exporter: Default::default(),
            next_export: Instant::now(),
            #[cfg(windows)]
            incoming: None,
            #[cfg(windows)]
            next_incoming: Instant::now(),
            #[cfg(windows)]
            identity_only: false,
            stop: Arc::new(AtomicBool::new(false)),
            recovery_owner: None,
        };
        assert!(controller.guard.as_ref().unwrap().check().is_ok());
        if restrict_before_close {
            exclusive().finish(None);
        }
        controller.reconcile_transport_observation(Err(Error::Start));
        if !restrict_before_close {
            assert_eq!(controller.view.error, Some(Error::Start));
            assert_eq!(controller.view.devices.len(), 1);
            exclusive().finish(None);
            controller.check_generation();
        }
        assert!(
            controller.view.devices.is_empty(),
            "revoked roster survived worker shutdown"
        );
        assert_eq!(controller.view.state, State::NotEnrolled);
        assert_eq!(controller.view.revision, "7");
        drop(controller);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(windows)]
#[test]
fn normal_windows_locator_is_read_only_pinned_and_refuses_incomplete_enrollment() {
    let root = crate::test_temp::exclusive_temp_dir("normal-enrolled-transport");
    assert!(Location::windows_enrolled(&root).unwrap().is_none());
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    let helper = root.join("managed-transport");
    std::fs::write(&helper, b"not a directory").unwrap();
    assert!(matches!(
        Location::windows_enrolled(&root),
        Err(Error::Authority)
    ));
    assert_eq!(std::fs::read(&helper).unwrap(), b"not a directory");
    std::fs::remove_file(&helper).unwrap();
    std::fs::create_dir(&helper).unwrap();
    assert!(matches!(
        Location::windows_enrolled(&root),
        Err(Error::NotEnrolled)
    ));
    assert_eq!(std::fs::read_dir(&helper).unwrap().count(), 0);
    let profile = helper.join("00000000000000000000000000000001");
    let package = helper.join("package");
    std::fs::create_dir(&profile).unwrap();
    std::fs::create_dir(&package).unwrap();
    let location = Location::windows_enrolled(&root).unwrap().unwrap();
    assert_eq!(location.profile, profile);
    assert_eq!(location.package, package);
    assert!(location.expected_ports.is_none());
    assert!(std::fs::rename(&helper, root.join("replaced")).is_err());
    assert!(matches!(
        location.candidate(
            &crate::FakeSecureStore::default(),
            SecureStoreEnvironment::Dev
        ),
        Err(Error::NotEnrolled)
    ));
    assert_eq!(std::fs::read_dir(&profile).unwrap().count(), 0);
    assert_eq!(std::fs::read_dir(&package).unwrap().count(), 0);
    drop(location);
    std::fs::remove_dir_all(root).unwrap();
}

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
    let (history, _) = snapshot.verified_trust_context().unwrap();
    let issuer = history.local_device.clone();
    let target = history
        .directly_approved_peers
        .iter()
        .next()
        .unwrap()
        .clone();
    let key_id = history.local_signing_key_id.clone();
    let public_key = history.local_public_key;
    drop(snapshot);
    let policy = crate::knowledge_materialization::scan_verified_trust_policy(
        &receiver,
        &brain.brain_id,
        &issuer,
        &key_id,
        &public_key,
        &[issuer.clone(), target.clone()],
    )
    .unwrap();
    let transaction = "cd".repeat(32);
    let approval = policy
        .device_restriction_approval_snapshot(
            transaction.clone(),
            &target,
            crate::bootstrap_journal::RestrictionReason::Compromised,
            0,
        )
        .unwrap();
    drop(policy);
    let journal = crate::bootstrap_journal::BootstrapJournalStore::new(&receiver).unwrap();
    journal
        .begin_owner_operation(approval.clone(), "99999999-9999-4999-8999-999999999997")
        .unwrap();
    journal
        .confirm_owner_operation(&transaction, &approval.digest().unwrap())
        .unwrap();
    crate::knowledge_materialization::append_confirmed_owner_security_operation(
        &receiver,
        &brain.brain_id,
        &store,
        SecureStoreEnvironment::Dev,
        &transaction,
    )
    .unwrap();
    drop(journal);
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

#[cfg(windows)]
#[test]
fn bootstrap_phase_preserves_explicit_join_and_never_falls_back_from_existing_history() {
    let root = crate::test_temp::exclusive_temp_dir("connections-joining");
    let store = crate::FakeSecureStore::new();
    let env = SecureStoreEnvironment::Dev;
    crate::install_runtime::prepare_m1_runtime_foundation(&root).unwrap();
    assert_eq!(bootstrap::identity_only(&root, &store, env), Ok(true));
    assert!(!root.join("canonical/events/inventory.lock").exists());
    let brain = cryptex_core::BrainIdentity::new(
        "33333333-3333-4333-8333-333333333333",
        "2026-11-15T00:00:00Z",
    )
    .unwrap();
    let raw = brain.to_json_lf().unwrap();
    crate::identity_runtime::write_brain_identity(&root, &brain).unwrap();
    assert_eq!(bootstrap::identity_only(&root, &store, env), Ok(true));
    let identity = crate::local_device_identity::join_existing_brain(
        &root,
        &store,
        env,
        &raw,
        &brain.brain_id,
    )
    .unwrap();
    assert_eq!(bootstrap::identity_only(&root, &store, env), Ok(true));
    assert!(bootstrap::identity_only(&root, &crate::FakeSecureStore::new(), env).is_err());
    let device = cryptex_core::ids::DeviceId::new(identity.device_id).unwrap();
    let inventory = crate::canonical_writer::acquire_canonical_inventory_lock(&root).unwrap();
    let device_lock = crate::canonical_writer::acquire_device_canonical_lock_under_inventory(
        &root, &device, &inventory,
    )
    .unwrap();
    drop(device_lock);
    drop(inventory);
    assert_eq!(bootstrap::identity_only(&root, &store, env), Ok(true));
    // Any stream artifact enters the full scanner even if malformed or empty.
    std::fs::write(
        cryptex_storage::canonical_events_device_dir(&root, &device).join("000000.pack"),
        b"invalid",
    )
    .unwrap();
    assert_eq!(bootstrap::identity_only(&root, &store, env), Ok(false));
    std::fs::remove_dir_all(root).unwrap();

    let root = crate::test_temp::exclusive_temp_dir("connections-bootstrap-corrupt");
    crate::install_runtime::prepare_m1_runtime_foundation(&root).unwrap();
    std::fs::write(
        crate::identity_runtime::brain_identity_path(&root),
        b"invalid",
    )
    .unwrap();
    assert_eq!(
        bootstrap::identity_only(&root, &store, env),
        Err(Error::Authority)
    );
    std::fs::remove_dir_all(root).unwrap();

    let root = crate::test_temp::exclusive_temp_dir("connections-bootstrap-settings");
    crate::install_runtime::prepare_m1_runtime_foundation(&root).unwrap();
    std::fs::create_dir(root.join("connection-settings")).unwrap();
    assert_eq!(bootstrap::identity_only(&root, &store, env), Ok(false));
    std::fs::remove_dir_all(root).unwrap();
}
