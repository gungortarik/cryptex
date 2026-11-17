//! Real native exchange through two normal connection Owners. No manual inbox
//! copies, enqueue calls, publication calls or loopback policy overrides.
use super::*;
use connection_settings::{Location, Owner, PeerAddress, State, Update};
use std::collections::BTreeSet;
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, UdpSocket};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const ENV: SecureStoreEnvironment = SecureStoreEnvironment::Dev;

// Canonical fixture signing keys stay in memory; real native workers use exactly
// these two disposable OS credential labels. Never access owner credentials.
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
fn canonical(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let events = root.join("canonical/events");
    let mut entries = Vec::new();
    for dir in std::fs::read_dir(&events).unwrap() {
        for file in std::fs::read_dir(dir.unwrap().path()).unwrap() {
            let path = file.unwrap().path();
            if path.extension().is_some_and(|e| e == "seg") {
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
        assert!(
            Instant::now() < deadline,
            "normal product exchange did not reach guarded search on both peers"
        );
        std::thread::sleep(Duration::from_millis(500));
    }
    owner_a.shutdown().unwrap();
    owner_b.shutdown().unwrap();
    drop((owner_a, owner_b, instance_a, instance_b));
    let saved = canonical(&source);
    assert_eq!(saved, canonical(&receiver));
    let (mut owner_a, instance_a) = a.start(&source, &package_path, store.clone());
    let (mut owner_b, instance_b) = b.start(&receiver, &package_path, store.clone());
    // Saved settings, fresh instance and fresh transport ownership: no reconnect helper.
    assert_eq!(owner_a.handle().request(None).unwrap().error, None);
    assert_eq!(owner_b.handle().request(None).unwrap().error, None);
    let until = Instant::now() + Duration::from_secs(35);
    let mut observed_a = false;
    let mut observed_b = false;
    while Instant::now() < until {
        observed_a |= found(&source, &instance_a, &store, "native reverse note");
        observed_b |= found(&receiver, &instance_b, &store, "test 3");
        std::thread::sleep(Duration::from_millis(500));
    }
    owner_a.shutdown().unwrap();
    owner_b.shutdown().unwrap();
    assert!(observed_a && observed_b);
    assert_eq!(saved, canonical(&source));
    assert_eq!(saved, canonical(&receiver));
    drop((owner_a, owner_b, instance_a, instance_b, store));
    for root in [source, receiver, negative] {
        std::fs::remove_dir_all(root).unwrap();
    }
}
