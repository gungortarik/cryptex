//! Real native exchange through two normal connection Owners. No manual inbox
//! copies, enqueue calls, publication calls or loopback policy overrides.
use super::*;
use connection_settings::{Location, Owner, PeerAddress, State, Update};
use cryptex_events::TrustedDaemonKeyResolver;
use std::collections::BTreeSet;
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, UdpSocket};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const ENV: SecureStoreEnvironment = SecureStoreEnvironment::Dev;

mod reader_preflight;
mod restriction;

// Canonical fixture signing keys stay in memory; real native workers use exactly
// the fixture's explicit disposable OS credential labels. Never access owner credentials.
struct Stores {
    canonical: crate::FakeSecureStore,
    transport: BTreeSet<(String, String)>,
}
impl SecureStore for Stores {
    fn get(&self, service: &str, account: &str) -> crate::secure_store::SecureStoreResult<Vec<u8>> {
        if self.transport.contains(&(service.into(), account.into())) {
            crate::WindowsCredentialStore::new().get(service, account)
        } else {
            self.canonical.get(service, account)
        }
    }
    fn put(
        &self,
        service: &str,
        account: &str,
        secret: &[u8],
    ) -> crate::secure_store::SecureStoreResult<()> {
        assert!(!self.transport.contains(&(service.into(), account.into())));
        self.canonical.put(service, account, secret)
    }
    fn delete(&self, service: &str, account: &str) -> crate::secure_store::SecureStoreResult<()> {
        assert!(!self.transport.contains(&(service.into(), account.into())));
        self.canonical.delete(service, account)
    }
}
struct Native {
    parent: PathBuf,
    profile: PathBuf,
    ports: native::NativePorts,
    identity: String,
    labels: (String, String),
}
impl Native {
    fn new(package: &crate::syncthing_package::PinnedSyncthingPackage, ip: Ipv4Addr) -> Self {
        let parent = crate::test_temp::exclusive_temp_dir("normal-product-native");
        let profile_path = parent.join("00000000000000000000000000000001");
        let mut profile = ManagedProfile::create(&profile_path, ENV).unwrap();
        let labels = profile.labels().unwrap();
        let api = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let sync = TcpListener::bind((ip, 0)).unwrap();
        let ports = native::NativePorts::new(
            api.local_addr().unwrap().port(),
            sync.local_addr().unwrap().port(),
        )
        .unwrap();
        drop((api, sync));
        let store = crate::WindowsCredentialStore::new();
        let mut prepared = profile.prepare_native(package, &store, ports).unwrap();
        let identity = prepared.configured_transport_device_id().to_owned();
        prepared
            .start(package, &store)
            .unwrap()
            .stop(&store)
            .unwrap();
        drop(prepared);
        drop(profile);
        Self {
            parent,
            profile: profile_path,
            ports,
            identity,
            labels,
        }
    }
    fn start(
        &self,
        root: &Path,
        package: &Path,
        store: Arc<Stores>,
    ) -> (Owner, Arc<Mutex<crate::instance::InstanceLock>>) {
        let instance = Arc::new(Mutex::new(
            crate::instance::try_acquire_instance(root).unwrap(),
        ));
        let owner = Owner::start(
            root,
            &instance.lock().unwrap(),
            store,
            ENV,
            Some(Location::windows(
                self.profile.clone(),
                package.to_owned(),
                self.ports,
            )),
            crate::approval_session::ApprovalSessionRegistry::default(),
            Some(instance.clone()),
        )
        .unwrap();
        (owner, instance)
    }
}
impl Drop for Native {
    fn drop(&mut self) {
        let store = crate::WindowsCredentialStore::new();
        store.delete(&self.labels.0, &self.labels.1).unwrap();
        assert!(matches!(
            store.get(&self.labels.0, &self.labels.1),
            Err(SecureStoreError::Missing)
        ));
        std::fs::remove_dir_all(&self.parent).unwrap();
    }
}
fn note(root: &Path, store: &Stores, text: &str) {
    let brain = crate::identity_runtime::load_brain_identity(root).unwrap();
    let local =
        crate::local_device_identity::load_local_device_identity(root, &brain.brain_id).unwrap();
    crate::canonical_writer::append_memory_note(
        store,
        ENV,
        crate::canonical_writer::AppendMemoryNoteArgs {
            runtime_root: root,
            brain_id: &brain.brain_id,
            device_id: &cryptex_core::DeviceId::new(local.device_id).unwrap(),
            classification: cryptex_events::Classification::Public,
            text,
            event_id: None,
            config: &crate::canonical_writer::CanonicalWriterConfig {
                seal_event_threshold: 1,
            },
        },
    )
    .unwrap();
}
fn found(
    root: &Path,
    instance: &Arc<Mutex<crate::instance::InstanceLock>>,
    store: &Stores,
    text: &str,
) -> bool {
    let Ok(instance) = instance.try_lock() else {
        return false;
    };
    let Ok(_permit) = instance.admission().enter(root) else {
        return false;
    };
    let brain = crate::identity_runtime::load_brain_identity(root).unwrap();
    let Ok(db) = cryptex_projection::ProjectionDb::open_existing_read_only(root) else {
        return false;
    };
    let mut resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
        root,
        &brain.brain_id,
        store,
        ENV,
    )
    .unwrap();
    let safety = crate::knowledge_materialization::evaluate_retrieval_safety(
        root,
        &brain.brain_id,
        &[],
        &mut resolver,
        &db,
    );
    safety.allows_content()
        && cryptex_projection::search::search_events(db.connection(), &Default::default())
            .unwrap()
            .iter()
            .any(|hit| hit.text == text)
}
fn connect(owner: &Owner, ip: Ipv4Addr, peer_port: u16) {
    let handle = owner.handle();
    let initial = handle.request(None).unwrap();
    assert_eq!(initial.error, None);
    assert_eq!(initial.devices.len(), 1);
    let view = handle
        .request(Some(Update {
            expected_revision: initial.revision,
            paused: false,
            listen_address: Some(ip),
            peers: vec![PeerAddress {
                device_id: initial.devices[0].device_id.clone(),
                endpoint: SocketAddrV4::new(ip, peer_port),
            }],
        }))
        .unwrap();
    assert_eq!(view.error, None);
    assert_eq!(view.state, State::ReadyLocally);
}
fn pause(owner: &Owner, paused: bool) {
    let handle = owner.handle();
    let initial = handle.request(None).unwrap();
    assert_eq!(initial.error, None);
    let view = handle
        .request(Some(Update {
            expected_revision: initial.revision,
            paused,
            listen_address: initial.listen_address,
            peers: initial
                .devices
                .into_iter()
                .map(|device| PeerAddress {
                    device_id: device.device_id,
                    endpoint: device.endpoint.unwrap(),
                })
                .collect(),
        }))
        .unwrap();
    assert_eq!(view.error, None);
    assert_eq!(view.paused, paused);
    assert_eq!(
        view.state,
        if paused {
            State::Paused
        } else {
            State::ReadyLocally
        }
    );
}
fn canonical(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    canonical_files(root, false)
}
fn canonical_files(root: &Path, include_active: bool) -> Vec<(PathBuf, Vec<u8>)> {
    let events = root.join("canonical/events");
    let mut entries = Vec::new();
    for dir in std::fs::read_dir(&events).unwrap() {
        let dir = dir.unwrap();
        if !dir.file_type().unwrap().is_dir() {
            continue; // Inventory coordination files are not device streams.
        }
        for file in std::fs::read_dir(dir.path()).unwrap() {
            let path = file.unwrap().path();
            if path
                .extension()
                .is_some_and(|e| e == "seg" || (include_active && e == "open"))
            {
                entries.push((
                    path.strip_prefix(&events).unwrap().to_owned(),
                    std::fs::read(path).unwrap(),
                ));
            }
        }
    }
    entries.sort();
    entries
}

// Exercise the daemon security journal/writer and the same rebuild invalidation
// used by the owner adapter. This is not an authenticated Desktop approval test.
fn security_change(
    root: &Path,
    instance: &Arc<Mutex<crate::instance::InstanceLock>>,
    store: &Stores,
    retire_self: bool,
) {
    security_change_for(root, instance, store, retire_self, None);
}

fn security_change_for(
    root: &Path,
    instance: &Arc<Mutex<crate::instance::InstanceLock>>,
    store: &Stores,
    retire_self: bool,
    target: Option<cryptex_core::DeviceId>,
) {
    use crate::bootstrap_journal::{BootstrapJournalStore, RestrictionReason};
    let admission = instance.lock().unwrap().admission();
    // Normal incoming replay may already own the exclusive gate. Do not treat
    // a legitimate Busy response as failure, or bypass any held/error state.
    let deadline = Instant::now() + Duration::from_secs(20);
    let permit = loop {
        match admission.enter(root) {
            Ok(permit) => break permit,
            Err(crate::runtime_admission::AdmissionError::Busy) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(error) => panic!("security fixture could not obtain ordinary admission: {error:?}"),
        }
    };
    let brain = crate::identity_runtime::load_brain_identity(root).unwrap();
    let resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
        root,
        &brain.brain_id,
        store,
        ENV,
    )
    .unwrap();
    let snapshot =
        crate::knowledge_materialization::scan_verified_canonical_from_local_anchor_until(
            root,
            &brain.brain_id,
            &resolver.local_trust_anchor().unwrap(),
            None,
        )
        .unwrap();
    let (history, _) = snapshot.verified_trust_context().unwrap();
    let local = history.local_device.clone();
    let target = target.unwrap_or_else(|| {
        history
            .directly_approved_peers
            .iter()
            .next()
            .unwrap()
            .clone()
    });
    assert!(history.directly_approved_peers.contains(&target));
    let devices = history.known_devices.iter().cloned().collect::<Vec<_>>();
    let key_id = history.local_signing_key_id.clone();
    let public_key = history.local_public_key;
    drop(snapshot);
    let policy = crate::knowledge_materialization::scan_verified_trust_policy(
        root,
        &brain.brain_id,
        &local,
        &key_id,
        &public_key,
        &devices,
    )
    .unwrap();
    let transaction = if retire_self { "df" } else { "de" }.repeat(32);
    let approval = if retire_self {
        policy
            .self_retirement_approval_snapshot(transaction.clone())
            .unwrap()
    } else {
        let cutoff = policy
            .history
            .records()
            .iter()
            .filter(|record| record.device_id() == target.as_str())
            .map(|record| record.device_seq())
            .max()
            .unwrap();
        assert!(cutoff > 0);
        let approval = policy
            .device_restriction_approval_snapshot(
                transaction.clone(),
                &target,
                RestrictionReason::Compromised,
                cutoff,
            )
            .unwrap();
        assert_ne!(
            approval.cutoff_content_hash.as_deref(),
            Some(cryptex_events::ZERO_CONTENT_HASH_HEX)
        );
        approval
    };
    drop(policy);
    let journal = BootstrapJournalStore::new(root).unwrap();
    journal
        .begin_owner_operation(
            approval.clone(),
            if retire_self {
                "99999999-9999-4999-8999-999999999993"
            } else {
                "99999999-9999-4999-8999-999999999994"
            },
        )
        .unwrap();
    journal
        .confirm_owner_operation(&transaction, &approval.digest().unwrap())
        .unwrap();
    admission.require_derived_rebuild(root, &permit).unwrap();
    crate::knowledge_materialization::append_confirmed_owner_security_operation(
        root,
        &brain.brain_id,
        store,
        ENV,
        &transaction,
    )
    .unwrap();
}

fn restricted_view(owner: &Owner, retired: bool) {
    let view = owner.handle().request(None).unwrap();
    assert!(view.devices.is_empty());
    if retired {
        assert_eq!(view.state, State::Attention);
        assert_eq!(
            view.error,
            Some(connection_settings::Error::LocalRestricted)
        );
        assert!(owner.handle().reader().is_none());
    } else {
        assert_eq!(view.state, State::AddressNeeded);
        assert_eq!(view.error, None);
        assert!(!view.paused);
    }
}

#[test]
#[ignore = "requires explicit pinned package, private CI interface and disposable OS credentials"]
fn two_normal_owners_exchange_offline_notes_and_replay_after_restart() {
    let package_path = PathBuf::from(
        std::env::var_os("CRYPTEX_EXCHANGE_NATIVE_PACKAGE").expect("explicit pinned package"),
    );
    let package = crate::syncthing_package::PinnedSyncthingPackage::open(&package_path).unwrap();
    // UDP connect selects a local interface; no datagram is sent to this TEST-NET address.
    let route = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).unwrap();
    route.connect("192.0.2.1:9").unwrap();
    let std::net::IpAddr::V4(ip) = route.local_addr().unwrap().ip() else {
        panic!("IPv4 required")
    };
    assert!(
        ip.is_private(),
        "only the disposable runner's private interface is allowed"
    );
    let a = Native::new(&package, ip);
    let b = Native::new(&package, ip);
    let (source, receiver, negative, canonical_store) =
        crate::trusted_import_probe::tests::pair_with_transport(&a.identity, &b.identity);
    let store = Arc::new(Stores {
        canonical: canonical_store,
        transport: BTreeSet::from([a.labels.clone(), b.labels.clone()]),
    });
    note(&source, &store, "test 3");
    note(&receiver, &store, "native reverse note");
    let (mut owner_a, instance_a) = a.start(&source, &package_path, store.clone());
    let (mut owner_b, instance_b) = b.start(&receiver, &package_path, store.clone());
    connect(&owner_a, ip, b.ports.sync);
    connect(&owner_b, ip, a.ports.sync);
    let deadline = Instant::now() + Duration::from_secs(180);
    while !found(&receiver, &instance_b, &store, "test 3")
        || !found(&source, &instance_a, &store, "native reverse note")
    {
        for owner in [&owner_a, &owner_b] {
            assert_eq!(owner.handle().request(None).unwrap().error, None);
        }
        assert!(
            Instant::now() < deadline,
            "normal product exchange did not reach guarded search on both peers"
        );
        std::thread::sleep(Duration::from_millis(500));
    }
    let consent = crate::ipc::public_consent::Session::default();
    let handle = owner_a.handle();
    let active_revision = handle.request(None).unwrap().revision;
    assert_eq!(
        handle.set_maintenance(true, consent.clone()).unwrap().state,
        State::Maintenance
    );
    assert!(handle.reader().is_none());
    let restored = handle.set_maintenance(false, consent).unwrap();
    assert_eq!(restored.revision, active_revision);
    assert_eq!(restored.state, State::ReadyLocally);
    drop(handle);
    pause(&owner_a, true);
    pause(&owner_b, true);
    owner_a.shutdown().unwrap();
    owner_b.shutdown().unwrap();
    drop((owner_a, owner_b, instance_a, instance_b));
    let delivered = canonical(&source);
    assert_eq!(delivered, canonical(&receiver));

    // Both transport owners are stopped. Only the daemon's ordinary writer
    // creates the two new signed notes; no delivery or publication shortcut.
    note(&source, &store, "offline source tail");
    note(&receiver, &store, "offline receiver tail");
    let isolated_a = canonical(&source);
    let isolated_b = canonical(&receiver);
    assert_ne!(isolated_a, isolated_b);
    let (mut owner_a, instance_a) = a.start(&source, &package_path, store.clone());
    let (mut owner_b, instance_b) = b.start(&receiver, &package_path, store.clone());
    for owner in [&owner_a, &owner_b] {
        let view = owner.handle().request(None).unwrap();
        assert_eq!(view.error, None);
        assert_eq!(view.state, State::Paused);
        assert!(view.paused);
    }
    // More than two ordinary five-second receive/export timer intervals.
    let until = Instant::now() + Duration::from_secs(12);
    while Instant::now() < until {
        assert!(found(&source, &instance_a, &store, "native reverse note"));
        assert!(found(&receiver, &instance_b, &store, "test 3"));
        assert!(found(&source, &instance_a, &store, "offline source tail"));
        assert!(found(
            &receiver,
            &instance_b,
            &store,
            "offline receiver tail"
        ));
        assert!(!found(
            &receiver,
            &instance_b,
            &store,
            "offline source tail"
        ));
        assert!(!found(
            &source,
            &instance_a,
            &store,
            "offline receiver tail"
        ));
        assert_eq!(isolated_a, canonical(&source));
        assert_eq!(isolated_b, canonical(&receiver));
        std::thread::sleep(Duration::from_millis(500));
    }
    pause(&owner_a, false);
    pause(&owner_b, false);
    let deadline = Instant::now() + Duration::from_secs(180);
    while !found(&receiver, &instance_b, &store, "offline source tail")
        || !found(&source, &instance_a, &store, "offline receiver tail")
    {
        for owner in [&owner_a, &owner_b] {
            assert_eq!(owner.handle().request(None).unwrap().error, None);
        }
        assert!(
            Instant::now() < deadline,
            "offline tails did not converge after resume"
        );
        std::thread::sleep(Duration::from_millis(500));
    }
    // The physical peers each received the queued note, then missed the next
    // segment. Keep both workers running and capture a second wave with the
    // ordinary writer threshold. Only the product exporter may seal it.
    for (root, instance, text) in [
        (&source, &instance_a, "source live acknowledgement"),
        (&receiver, &instance_b, "receiver live acknowledgement"),
    ] {
        let admission = instance.lock().unwrap().admission();
        let until = Instant::now() + Duration::from_secs(30);
        let _permit = loop {
            match admission.enter(root) {
                Ok(permit) => break permit,
                Err(crate::runtime_admission::AdmissionError::Busy) if Instant::now() < until => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(error) => panic!("live capture admission failed: {error:?}"),
            }
        };
        let outcome = crate::capture::capture_memory_note(
            store.as_ref(),
            ENV,
            root,
            crate::capture::CaptureRequest {
                capture_id: "a6354484-3ea5-4359-a72b-34fbe0447a0a".into(),
                classification: cryptex_events::Classification::Public,
                text: text.into(),
            },
            &crate::canonical_writer::CanonicalWriterConfig::default(),
        );
        assert!(
            matches!(outcome, crate::capture::CaptureOutcome::Committed(_)),
            "{outcome:?}"
        );
        // This fixture starts connection owners without the IPC projection
        // worker. Model its post-capture refresh under ordinary admission.
        let brain = crate::identity_runtime::load_brain_identity(root).unwrap();
        crate::incoming_publication::rebuild_imported_state(
            root,
            &brain.brain_id,
            store.as_ref(),
            ENV,
        )
        .unwrap();
    }
    let deadline = Instant::now() + Duration::from_secs(180);
    while !found(
        &receiver,
        &instance_b,
        &store,
        "source live acknowledgement",
    ) || !found(
        &source,
        &instance_a,
        &store,
        "receiver live acknowledgement",
    ) {
        assert!(
            Instant::now() < deadline,
            "live acknowledgements failed to converge"
        );
        std::thread::sleep(Duration::from_secs(2));
    }
    owner_a.shutdown().unwrap();
    owner_b.shutdown().unwrap();
    drop((owner_a, owner_b, instance_a, instance_b));
    let saved = canonical(&source);
    assert_eq!(saved, canonical(&receiver));
    for (path, bytes) in delivered {
        assert!(
            saved.contains(&(path, bytes)),
            "previously delivered bytes changed"
        );
    }
    let (mut owner_a, instance_a) = a.start(&source, &package_path, store.clone());
    let (mut owner_b, instance_b) = b.start(&receiver, &package_path, store.clone());
    // Saved settings, fresh instance and fresh transport ownership: no reconnect helper.
    assert_eq!(owner_a.handle().request(None).unwrap().error, None);
    assert_eq!(owner_b.handle().request(None).unwrap().error, None);
    let until = Instant::now() + Duration::from_secs(35);
    let mut observed_a = false;
    let mut observed_b = false;
    while Instant::now() < until {
        observed_a |= found(&source, &instance_a, &store, "native reverse note")
            && found(&source, &instance_a, &store, "offline receiver tail");
        observed_b |= found(&receiver, &instance_b, &store, "test 3")
            && found(&receiver, &instance_b, &store, "offline source tail");
        std::thread::sleep(Duration::from_millis(500));
    }
    owner_a.shutdown().unwrap();
    owner_b.shutdown().unwrap();
    assert!(observed_a && observed_b);
    assert_eq!(saved, canonical(&source));
    assert_eq!(saved, canonical(&receiver));
    drop((owner_a, owner_b, instance_a, instance_b));

    // Reopen the real normal Owner, then restrict its last peer while live.
    // Saved endpoints are preferences only and must never regrant authority.
    let (mut owner_a, instance_a) = a.start(&source, &package_path, store.clone());
    let view = owner_a.handle().request(None).unwrap();
    assert_eq!(view.state, State::ReadyLocally);
    let stale = Update {
        expected_revision: view.revision.clone(),
        paused: false,
        listen_address: view.listen_address,
        peers: view
            .devices
            .iter()
            .map(|device| PeerAddress {
                device_id: device.device_id.clone(),
                endpoint: device.endpoint.unwrap(),
            })
            .collect(),
    };
    let old_reader = owner_a.handle().reader().unwrap();
    security_change(&source, &instance_a, &store, false);
    assert!(old_reader.try_transport_device_id().is_err());
    // A revoked handle still retains the original instance exclusion until
    // dropped. Finish this negative observation before attempting a new owner.
    drop(old_reader);
    restricted_view(&owner_a, false);
    assert!(matches!(
        owner_a.handle().request(Some(stale.clone())),
        Err(connection_settings::Error::UnknownDevice)
    ));
    restricted_view(&owner_a, false);
    assert!(
        std::net::TcpStream::connect_timeout(
            &SocketAddrV4::new(ip, a.ports.sync).into(),
            Duration::from_secs(1),
        )
        .is_err(),
        "restricted last peer must remove the LAN listener"
    );
    assert!(found(&source, &instance_a, &store, "offline receiver tail"));
    owner_a.shutdown().unwrap();
    drop((owner_a, instance_a));
    // Security appends can remain in the durable active pack when the last
    // peer is removed. Count/include that pack; do not force a fixture seal.
    let restricted = canonical_files(&source, true);
    assert!(restricted.len() > saved.len());
    for previous in &saved {
        assert!(restricted.contains(previous));
    }
    let (mut owner_a, instance_a) = a.start(&source, &package_path, store.clone());
    restricted_view(&owner_a, false);
    assert!(matches!(
        owner_a.handle().request(Some(stale.clone())),
        Err(connection_settings::Error::UnknownDevice)
    ));
    assert_eq!(restricted, canonical_files(&source, true));
    assert!(found(&source, &instance_a, &store, "offline receiver tail"));

    // A retired local identity has no transport authority, including after reopen.
    security_change(&source, &instance_a, &store, true);
    restricted_view(&owner_a, true);
    owner_a.shutdown().unwrap();
    drop((owner_a, instance_a));
    let retired = canonical_files(&source, true);
    assert_eq!(retired.len(), restricted.len() + 1, "one terminal segment");
    for (path, bytes) in &restricted {
        // Retirement seals the existing prefix, then writes its own segment.
        let sealed_path = path.with_extension("seg");
        let after = &retired
            .iter()
            .find(|(p, _)| *p == sealed_path)
            .expect("sealed prefix present")
            .1;
        assert_eq!(after, bytes, "all prefix bytes must remain immutable");
    }
    let (mut owner_a, instance_a) = a.start(&source, &package_path, store.clone());
    restricted_view(&owner_a, true);
    assert!(matches!(
        owner_a.handle().request(Some(stale)),
        Err(connection_settings::Error::LocalRestricted)
    ));
    restricted_view(&owner_a, true);
    assert_eq!(retired, canonical_files(&source, true));
    owner_a.shutdown().unwrap();
    drop((owner_a, instance_a, store));
    for root in [source, receiver, negative] {
        std::fs::remove_dir_all(root).unwrap();
    }
}
