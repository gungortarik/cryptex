//! Explicit synthetic host-relay -> VM integration. No normal activation path.
use super::{
    ManagedProfile,
    controller::Phase,
    incoming::{IncomingController, IncomingTick},
    native::NativePorts,
    worker::ManagedTransportWorker,
};
use crate::secure_store::{SecureStore, SecureStoreEnvironment, WindowsCredentialStore};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

const ENV: SecureStoreEnvironment = SecureStoreEnvironment::Dev;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Relay {
    schema: u32,
    session: String,
    peer_id: String,
    address: String,
}

fn digest(path: &Path) -> String {
    let meta = fs::symlink_metadata(path).unwrap();
    assert!(meta.is_file() && !meta.file_type().is_symlink() && meta.len() <= 8 * 1024 * 1024);
    format!("{:x}", Sha256::digest(fs::read(path).unwrap()))
}
fn canonical_hashes(root: &Path) -> std::collections::BTreeMap<PathBuf, String> {
    let mut result = std::collections::BTreeMap::new();
    for device in fs::read_dir(root.join("canonical/events")).unwrap() {
        let device = device.unwrap();
        let kind = device.file_type().unwrap();
        // The canonical writer keeps this coordination file beside device directories.
        if device.file_name() == "inventory.lock" {
            assert!(kind.is_file(), "inventory lock must be a regular file");
            continue;
        }
        assert!(kind.is_dir(), "unexpected canonical inventory entry");
        for file in fs::read_dir(device.path()).unwrap() {
            let path = file.unwrap().path();
            if path
                .extension()
                .is_some_and(|ext| ext == "seg" || ext == "open")
            {
                result.insert(path.strip_prefix(root).unwrap().to_owned(), digest(&path));
            }
        }
    }
    result
}
#[test]
fn canonical_inventory_tracks_event_bytes_with_root_inventory_lock() {
    let (_, receiver, _, _) = crate::trusted_import_probe::tests::pair();
    assert!(receiver.join("canonical/events/inventory.lock").is_file());
    let before = canonical_hashes(&receiver);
    assert!(!before.is_empty());
    let relative = before.keys().next().unwrap();
    let path = receiver.join(relative);
    let original = fs::read(&path).unwrap();
    // Deliberately mutate only this disposable fixture to test the evidence helper.
    fs::write(&path, b"changed synthetic event bytes").unwrap();
    assert_ne!(before, canonical_hashes(&receiver));
    fs::write(&path, original).unwrap();
    assert_eq!(before, canonical_hashes(&receiver));
    let extra = path.parent().unwrap().join("00000000000000000099.seg");
    fs::write(&extra, b"additional synthetic segment").unwrap();
    assert_eq!(canonical_hashes(&receiver).len(), before.len() + 1);
    fs::remove_file(&extra).unwrap();
    fs::remove_file(&path).unwrap();
    assert_ne!(before, canonical_hashes(&receiver));
}
#[test]
fn canonical_inventory_refuses_unexpected_root_file() {
    let root = crate::test_temp::exclusive_temp_dir("native-inventory-invalid");
    fs::create_dir_all(root.join("canonical/events")).unwrap();
    fs::write(root.join("canonical/events/unexpected.seg"), b"unexpected").unwrap();
    assert!(std::panic::catch_unwind(|| canonical_hashes(&root)).is_err());
}
#[test]
fn canonical_inventory_refuses_directory_in_lock_slot() {
    let root = crate::test_temp::exclusive_temp_dir("native-inventory-lock-slot");
    fs::create_dir_all(root.join("canonical/events/inventory.lock")).unwrap();
    assert!(std::panic::catch_unwind(|| canonical_hashes(&root)).is_err());
}
fn wait_ready(worker: &ManagedTransportWorker, attempt: u8) -> u32 {
    let until = Instant::now() + Duration::from_secs(40);
    loop {
        let s = worker.snapshot().unwrap();
        assert!(s.error.is_none(), "native worker failed: {:?}", s.error);
        if let Some(status) = s.controller
            && status.phase == Phase::LocalReady
            && status.attempt >= attempt
        {
            worker.status_reader().try_transport_device_id().unwrap();
            return status.pid.unwrap();
        }
        assert!(Instant::now() < until, "native ready deadline");
        std::thread::sleep(Duration::from_millis(50));
    }
}
fn checkpoint(work: &Path, value: &str) {
    println!("native_transfer_checkpoint={value}");
    fs::write(work.join("checkpoint.txt"), value).unwrap();
}

struct PausedStore {
    inner: Arc<crate::FakeSecureStore>,
    destination: PathBuf,
    once: AtomicBool,
    started: std::sync::mpsc::SyncSender<()>,
    resume: std::sync::Mutex<std::sync::mpsc::Receiver<()>>,
}
impl SecureStore for PausedStore {
    fn put(
        &self,
        service: &str,
        account: &str,
        secret: &[u8],
    ) -> crate::secure_store::SecureStoreResult<()> {
        self.inner.put(service, account, secret)
    }
    fn get(&self, service: &str, account: &str) -> crate::secure_store::SecureStoreResult<Vec<u8>> {
        if self.destination.is_file() && !self.once.swap(true, Ordering::AcqRel) {
            self.started.send(()).unwrap();
            self.resume
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(10))
                .unwrap();
        }
        self.inner.get(service, account)
    }
    fn delete(&self, service: &str, account: &str) -> crate::secure_store::SecureStoreResult<()> {
        self.inner.delete(service, account)
    }
}

#[test]
#[ignore = "explicit one-click host/VM synthetic relay fixture required"]
fn real_delivery_worker_publication_crash_restart_and_replay() {
    let work = PathBuf::from(
        std::env::var_os("CRYPTEX_NATIVE_TRANSFER_TEST").expect("explicit pilot required"),
    );
    assert!(work.is_absolute());
    crate::paths::assert_outside_repository(&work).unwrap();
    for name in [
        "result.json",
        "public-request",
        "request.ready",
        "checkpoint.txt",
        "cancel.request",
    ] {
        assert!(
            !work.join(name).exists(),
            "fresh pilot work directory required"
        );
    }
    let raw = fs::read(work.join("relay.json")).unwrap();
    assert!(raw.len() < 2048);
    let relay: Relay = serde_json::from_slice(&raw).unwrap();
    assert_eq!(relay.schema, 1);
    assert_eq!(relay.session.len(), 32);
    assert!(
        relay
            .session
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    );
    let package_path = work.join("native-package");
    let package = crate::syncthing_package::PinnedSyncthingPackage::open(&package_path).unwrap();
    let mut random = [0; 16];
    getrandom::getrandom(&mut random).unwrap();
    let leaf: String = random.iter().map(|b| format!("{b:02x}")).collect();
    let profile_path = work.join(leaf);
    let mut profile = ManagedProfile::create(&profile_path, ENV).unwrap();
    let os_store = WindowsCredentialStore::new();
    let (service, account) = profile.labels().unwrap();
    struct CredentialCleanup(WindowsCredentialStore, Option<(String, String)>);
    impl Drop for CredentialCleanup {
        fn drop(&mut self) {
            if let Some((service, account)) = &self.1
                && self.0.delete(service, account).is_err()
            {
                eprintln!("native_test_credential_cleanup=failed");
            }
        }
    }
    let mut credential_cleanup =
        CredentialCleanup(WindowsCredentialStore::new(), Some((service, account)));
    let port = || {
        std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    };
    let api = port();
    let mut sync = port();
    while api == sync {
        sync = port();
    }
    let ports = NativePorts::new(api, sync).unwrap();
    let mut prepared = profile.prepare_native(&package, &os_store, ports).unwrap();
    let identity = prepared.configured_transport_device_id().to_owned();
    let (source, receiver, negative, store) =
        crate::trusted_import_probe::tests::pair_with_transport(&relay.peer_id, &identity);
    let report = crate::trusted_import_probe::prepare(
        &source,
        &receiver,
        &negative,
        "44444444-4444-4444-8444-444444444444",
        &store,
    )
    .unwrap();
    let store = Arc::new(store);
    let baseline = canonical_hashes(&receiver);
    let incoming = receiver.join("transport/incoming");
    fs::create_dir_all(&incoming).unwrap();
    let folder = format!("cx-native-{}", relay.session);
    let policy = prepared
        .configure_transfer_test(
            &os_store,
            &relay.peer_id,
            &relay.address,
            &folder,
            &incoming,
        )
        .unwrap();
    drop(prepared);
    drop(profile);
    // Product worker must not adopt this test-only peer/folder extension.
    let mut normal = ManagedProfile::open(&profile_path, ENV).unwrap();
    assert!(normal.reopen_native(&package, &os_store, ports).is_err());
    drop(normal);
    drop(package);
    let paths: Vec<_> = report
        .segments
        .iter()
        .map(|s| PathBuf::from(&s.relative_path))
        .collect();
    let crash = Arc::new(super::controller::NativeTestCrash::default());
    let spawn = || {
        ManagedTransportWorker::spawn_transfer_test(
            profile_path.clone(),
            package_path.clone(),
            ports,
            policy.clone(),
            crash.clone(),
        )
        .unwrap()
    };
    let mut worker = spawn();
    wait_ready(&worker, 1);
    assert_eq!(
        worker.status_reader().try_transport_device_id().unwrap(),
        identity
    );
    checkpoint(&work, "owned_rest_ready");
    let export = work.join("public-request");
    fs::create_dir(&export).unwrap();
    for path in &paths {
        let dest = export.join("payload").join(path);
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        fs::copy(source.join("canonical/events").join(path), dest).unwrap();
    }
    fs::write(
        export.join("request.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema":1, "session":relay.session, "receiver_id":identity,
            "relay_id":relay.peer_id, "folder_id":folder, "segments":report.segments,
            "scope":"synthetic signed fixture; host is an untrusted relay; no owner profile"
        }))
        .unwrap(),
    )
    .unwrap();
    fs::write(work.join("request.ready"), b"ready\n").unwrap();
    checkpoint(&work, "waiting_for_host_delivery");
    let until = Instant::now() + Duration::from_secs(1800);
    loop {
        assert!(
            worker.snapshot().unwrap().error.is_none(),
            "worker failed while waiting"
        );
        assert!(
            !work.join("cancel.request").exists(),
            "runner cancelled the isolated pilot"
        );
        if report.segments.iter().all(|s| {
            let p = incoming.join(&s.relative_path);
            p.is_file() && fs::metadata(&p).unwrap().len() == s.bytes && digest(&p) == s.sha256
        }) {
            break;
        }
        assert!(
            Instant::now() < until,
            "host delivery deadline; do not report pass"
        );
        std::thread::sleep(Duration::from_millis(250));
    }
    for path in &paths {
        assert!(!receiver.join("canonical/events").join(path).exists());
    }
    checkpoint(&work, "actual_delivery_verified_before_publication");
    let mut instance = crate::instance::try_acquire_instance(&receiver).unwrap();
    let mut controller = IncomingController::new(receiver.clone(), ENV).unwrap();
    controller.enqueue(paths.clone()).unwrap();
    let before_crash = worker.snapshot().unwrap().controller.unwrap();
    assert_eq!(before_crash.phase, Phase::LocalReady);
    assert_eq!(
        before_crash.attempt, 1,
        "unexpected spontaneous restart during delivery"
    );
    crash.requested.store(true, Ordering::Release);
    let until = Instant::now() + Duration::from_secs(15);
    loop {
        let s = worker.snapshot().unwrap();
        assert!(s.error.is_none(), "crash cleanup failed: {:?}", s.error);
        if crash.completed.load(Ordering::Acquire) == 1
            && s.controller
                .is_some_and(|s| s.phase == Phase::RestartWaiting)
        {
            break;
        }
        assert!(Instant::now() < until, "owned job restart not observed");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(matches!(
        worker
            .process_incoming(&mut controller, &mut instance, store.as_ref())
            .unwrap(),
        IncomingTick::WaitingForTransport
    ));
    for path in &paths {
        assert!(!receiver.join("canonical/events").join(path).exists());
    }
    let second_pid = wait_ready(&worker, before_crash.attempt + 1);
    assert_eq!(
        worker.snapshot().unwrap().controller.unwrap().attempt,
        before_crash.attempt + 1
    );
    assert_eq!(crash.completed.load(Ordering::Acquire), 1);
    assert_ne!(before_crash.pid.unwrap(), second_pid);
    assert_eq!(
        worker.status_reader().try_transport_device_id().unwrap(),
        identity
    );
    checkpoint(&work, "owned_job_crash_restarted_identity_unchanged");
    let gate = instance.admission();
    let exclusive = gate
        .exclusive(&receiver, Instant::now() + Duration::from_secs(2), false)
        .unwrap();
    assert!(matches!(
        worker
            .process_incoming(&mut controller, &mut instance, store.as_ref())
            .unwrap(),
        IncomingTick::Deferred
    ));
    crate::ipc::tests::native_transfer_search(&receiver, gate.clone(), store.clone(), true);
    exclusive.finish(None);
    let (started_send, started_recv) = std::sync::mpsc::sync_channel(1);
    let (resume_send, resume_recv) = std::sync::mpsc::channel();
    let paused = PausedStore {
        inner: store.clone(),
        destination: receiver.join("canonical/events").join(&paths[0]),
        once: AtomicBool::new(false),
        started: started_send,
        resume: std::sync::Mutex::new(resume_recv),
    };
    let result = std::thread::scope(|scope| {
        let publishing =
            scope.spawn(|| worker.process_incoming(&mut controller, &mut instance, &paused));
        started_recv.recv_timeout(Duration::from_secs(10)).unwrap();
        crate::ipc::tests::native_transfer_search(&receiver, gate.clone(), store.clone(), true);
        resume_send.send(()).unwrap();
        publishing.join().unwrap().unwrap()
    });
    let IncomingTick::Published(published) = result else {
        panic!("publication missing")
    };
    assert_eq!(published.published_segments, paths.len());
    let after_publication = canonical_hashes(&receiver);
    assert_eq!(after_publication.len(), baseline.len() + paths.len());
    for (path, hash) in &baseline {
        assert_eq!(after_publication.get(path), Some(hash));
    }
    for segment in &report.segments {
        assert_eq!(
            digest(
                &receiver
                    .join("canonical/events")
                    .join(&segment.relative_path)
            ),
            segment.sha256
        );
    }
    crate::ipc::tests::native_transfer_search(&receiver, gate, store.clone(), false);
    checkpoint(&work, "worker_publication_and_real_ipc_search_passed");
    assert_eq!(
        worker.shutdown(Duration::from_secs(40)).unwrap(),
        super::controller::Outcome::CleanShutdown
    );
    instance.drain_runtime(Duration::from_secs(2)).unwrap();
    drop(instance);
    // Reopen the actual native profile and reacquire daemon exclusion. This is
    // restart/replay, not a physical power loss or a production startup feeder.
    let mut instance = crate::instance::try_acquire_instance(&receiver).unwrap();
    let mut worker = spawn();
    wait_ready(&worker, 1);
    assert_eq!(
        worker.status_reader().try_transport_device_id().unwrap(),
        identity
    );
    let mut replay = IncomingController::new(receiver.clone(), ENV).unwrap();
    replay.enqueue(paths.clone()).unwrap();
    let IncomingTick::Published(replayed) = worker
        .process_incoming(&mut replay, &mut instance, store.as_ref())
        .unwrap()
    else {
        panic!("replay missing")
    };
    assert_eq!(replayed.published_segments, 0);
    assert_eq!(replayed.identical_segments, paths.len());
    assert_eq!(after_publication, canonical_hashes(&receiver));
    crate::ipc::tests::native_transfer_search(&receiver, instance.admission(), store, false);
    assert_eq!(
        worker.shutdown(Duration::from_secs(40)).unwrap(),
        super::controller::Outcome::CleanShutdown
    );
    instance.drain_runtime(Duration::from_secs(2)).unwrap();
    checkpoint(&work, "restart_replay_no_duplicate_passed");
    let (service, account) = credential_cleanup.1.as_ref().unwrap();
    os_store
        .delete(service, account)
        .expect("synthetic API credential cleanup failed");
    credential_cleanup.1 = None;
    fs::write(work.join("result.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "schema":1,"session":relay.session,"passed":true,"receiver_id":identity,"relay_id":relay.peer_id,
        "published_segments":published.published_segments,"replayed_identical_segments":replayed.identical_segments,
        "real_owned_rest":true,"native_job_crash_before_publication":true,"restart_identity_unchanged":true,
        "real_ipc_busy_and_search":true,"ipc_peer_auth":"existing test seam; not native Desktop attestation",
        "immutable_segment_hashes":true,"clean_shutdown":true,"synthetic_api_credential_deleted":true,
        "scope":"test-only native peer/folder policy; synthetic canonical keys; no normal activation; no power-cut claim"
    })).unwrap()).unwrap();
}
