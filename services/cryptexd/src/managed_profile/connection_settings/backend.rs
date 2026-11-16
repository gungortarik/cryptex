use super::*;
use crate::managed_profile::{
    ManagedProfile, native::NativePorts, native_xml::exchange::ExchangeConfigPolicy,
};
use crate::managed_profile::{
    exchange_install::Leaf, exchange_policy::ExchangePeers, platform::PrivateDirectory,
};
use crate::runtime_admission::{RuntimeAdmission, TransportGuard};
use crate::secure_store::{SecureStore, SecureStoreEnvironment};
use cryptex_events::TrustedDaemonKeyResolver;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

#[cfg(windows)]
mod bootstrap;

#[cfg(windows)]
type Worker = crate::managed_profile::worker::ManagedTransportWorker;
#[cfg(windows)]
pub(crate) type Reader = crate::managed_profile::worker::StatusReader;
#[cfg(target_os = "macos")]
type Worker = crate::m8_owner_channel::transport_helper::Worker;
#[cfg(target_os = "macos")]
pub(crate) type Reader = crate::m8_owner_channel::transport_helper::StatusReader;

/// Backend-selected enrolled locations. Never deserialized from a UI request.
pub struct Location {
    profile: PathBuf,
    package: PathBuf,
    expected_ports: Option<NativePorts>,
}
impl Location {
    #[cfg(windows)]
    pub fn windows(profile: PathBuf, package: PathBuf, ports: NativePorts) -> Self {
        Self {
            profile,
            package,
            expected_ports: Some(ports),
        }
    }
    #[cfg(target_os = "macos")]
    pub(crate) fn mac(root: &Path) -> Self {
        let helper = root.join("managed-transport");
        Self {
            profile: helper.join("00000000000000000000000000000001"),
            package: helper.join("package"),
            expected_ports: None,
        }
    }
    pub(crate) fn candidate(
        &self,
        store: &dyn SecureStore,
        environment: SecureStoreEnvironment,
    ) -> Result<(String, NativePorts), Error> {
        let package = crate::syncthing_package::PinnedSyncthingPackage::open(&self.package)
            .map_err(|_| Error::NotEnrolled)?;
        let mut profile =
            ManagedProfile::open(&self.profile, environment).map_err(|_| Error::NotEnrolled)?;
        let ports = profile
            .existing_native_ports()
            .map_err(|_| Error::NotEnrolled)?;
        if self.expected_ports.is_some_and(|p| p != ports) {
            return Err(Error::NotEnrolled);
        }
        let candidate = profile
            .reopen_exchange_candidate(&package, store, ports)
            .map_err(|_| Error::NotEnrolled)?;
        Ok((candidate.configured_transport_device_id().to_owned(), ports))
    }
}

struct Interest {
    deadline: Instant,
    live: Arc<AtomicBool>,
}
impl Interest {
    fn new() -> Self {
        Self {
            deadline: Instant::now() + Duration::from_secs(105),
            live: Arc::new(AtomicBool::new(true)),
        }
    }
    fn check(&self, stop: &AtomicBool) -> Result<(), Error> {
        if stop.load(Ordering::Acquire)
            || !self.live.load(Ordering::Acquire)
            || Instant::now() >= self.deadline
        {
            Err(Error::Unavailable)
        } else {
            Ok(())
        }
    }
}
struct RequestLifetime(Arc<AtomicBool>);
impl Drop for RequestLifetime {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
enum Command {
    Read(Interest, mpsc::SyncSender<Result<View, Error>>),
    Update(Update, Interest, mpsc::SyncSender<Result<View, Error>>),
}
#[derive(Clone)]
pub struct Handle {
    sender: mpsc::SyncSender<Command>,
    reader: Arc<Mutex<Option<Reader>>>,
}
impl Handle {
    pub(crate) fn reader(&self) -> Option<Reader> {
        self.reader.try_lock().ok()?.clone()
    }
    pub(crate) fn request(&self, update: Option<Update>) -> Result<View, Error> {
        let (sender, receiver) = mpsc::sync_channel(1);
        let interest = Interest::new();
        let _lifetime = RequestLifetime(interest.live.clone());
        let command = match update {
            Some(update) => Command::Update(update, interest, sender),
            None => Command::Read(interest, sender),
        };
        self.sender
            .try_send(command)
            .map_err(|_| Error::Unavailable)?;
        receiver
            .recv_timeout(Duration::from_secs(110))
            .map_err(|_| Error::Unavailable)?
    }
}

pub struct Owner {
    handle: Handle,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<Result<(), Error>>>,
    shutdown_failed: bool,
}
impl Owner {
    pub fn start(
        root: &Path,
        instance: &crate::instance::InstanceLock,
        store: Arc<dyn SecureStore>,
        environment: SecureStoreEnvironment,
        location: Option<Location>,
        registry: crate::approval_session::ApprovalSessionRegistry,
        recovery_owner: Option<Arc<Mutex<crate::instance::InstanceLock>>>,
    ) -> Result<Self, Error> {
        if instance.path() != crate::paths::lock_path(root) {
            return Err(Error::Authority);
        }
        let (sender, receive) = mpsc::sync_channel(4);
        let reader = Arc::new(Mutex::new(None));
        let handle = Handle {
            sender,
            reader: reader.clone(),
        };
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let root = root.to_owned();
        let admission = instance.admission();
        let instance_pin = instance.retain_lock();
        let thread = std::thread::Builder::new()
            .name("cryptex-connections".into())
            .spawn(move || {
                let _instance_pin = instance_pin;
                let mut controller = Controller {
                    root,
                    admission,
                    store,
                    environment,
                    location,
                    registry,
                    recovery_owner,
                    settings: None,
                    preferences: Preferences::default(),
                    worker: None,
                    guard: None,
                    reader,
                    cleanup_failed: false,
                    exporter: super::super::sealed_export::Exporter::default(),
                    next_export: Instant::now(),
                    #[cfg(windows)]
                    identity_only: false,
                    stop: worker_stop.clone(),
                    view: View {
                        revision: "0".into(),
                        paused: true,
                        listen_address: None,
                        local_transport_port: None,
                        devices: Vec::new(),
                        state: State::Applying,
                        error: None,
                    },
                };
                if let Some(owner) = &controller.recovery_owner
                    && controller.location.is_some()
                    && controller.requires_history_recovery()
                {
                    let mut instance = owner.lock().map_err(|_| Error::Unavailable)?;
                    // First startup uses the same canonical recovery/rebuild gate
                    // as later owner changes, before any network admission.
                    let _ = crate::incoming_publication::recover_live_with_instance(
                        &controller.root,
                        &mut instance,
                        controller.store.as_ref(),
                        controller.environment,
                    );
                }
                let _ = controller.apply(None, &Interest::new());
                while !worker_stop.load(Ordering::Acquire) {
                    match receive.recv_timeout(Duration::from_millis(500)) {
                        Ok(Command::Read(interest, reply)) => {
                            if interest.check(&worker_stop).is_err() {
                                let _ = reply.try_send(Err(Error::Unavailable));
                                continue;
                            }
                            controller.check_generation();
                            let _ = reply.try_send(
                                interest
                                    .check(&worker_stop)
                                    .map(|()| controller.view.clone()),
                            );
                        }
                        Ok(Command::Update(update, interest, reply)) => {
                            let result = controller.apply(Some(update), &interest);
                            let _ = reply.try_send(result);
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => controller.check_generation(),
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
                controller.close()
            })
            .map_err(|_| Error::Unavailable)?;
        Ok(Self {
            handle,
            stop,
            thread: Some(thread),
            shutdown_failed: false,
        })
    }
    pub fn handle(&self) -> Handle {
        self.handle.clone()
    }
    pub fn shutdown(&mut self) -> Result<(), Error> {
        self.stop.store(true, Ordering::Release);
        if self.shutdown_failed {
            return Err(Error::Shutdown);
        }
        if let Some(thread) = self.thread.take() {
            let until = Instant::now() + Duration::from_secs(120);
            while !thread.is_finished() {
                if Instant::now() >= until {
                    // Detached thread retains original instance pin until its
                    // real cleanup. A timeout never means the daemon is closed.
                    self.shutdown_failed = true;
                    return Err(Error::Shutdown);
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            if thread
                .join()
                .map_err(|_| Error::Shutdown)
                .and_then(|result| result)
                .is_err()
            {
                self.shutdown_failed = true;
                return Err(Error::Shutdown);
            }
        }
        Ok(())
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        if self.shutdown().is_err() {
            eprintln!("component=connections stage=shutdown status=unresolved");
        }
    }
}

struct Controller {
    root: PathBuf,
    admission: RuntimeAdmission,
    store: Arc<dyn SecureStore>,
    environment: SecureStoreEnvironment,
    location: Option<Location>,
    registry: crate::approval_session::ApprovalSessionRegistry,
    settings: Option<SettingsStore>,
    preferences: Preferences,
    worker: Option<Worker>,
    guard: Option<TransportGuard>,
    reader: Arc<Mutex<Option<Reader>>>,
    view: View,
    cleanup_failed: bool,
    exporter: super::super::sealed_export::Exporter,
    next_export: Instant,
    #[cfg(windows)]
    identity_only: bool,
    stop: Arc<AtomicBool>,
    recovery_owner: Option<Arc<Mutex<crate::instance::InstanceLock>>>,
}
impl Controller {
    fn requires_history_recovery(&self) -> bool {
        #[cfg(windows)]
        {
            bootstrap::identity_only(&self.root, self.store.as_ref(), self.environment) == Ok(false)
        }
        #[cfg(not(windows))]
        {
            true
        }
    }
    fn close(&mut self) -> Result<(), Error> {
        *self.reader.lock().map_err(|_| Error::Unavailable)? = None;
        if self.cleanup_failed {
            return Err(Error::Shutdown);
        }
        if let Some(worker) = self.worker.take() {
            #[cfg(windows)]
            let mut worker = worker;
            #[cfg(windows)]
            let result = worker
                .shutdown(Duration::from_secs(40))
                .map(|_| ())
                .map_err(|_| Error::Shutdown);
            #[cfg(target_os = "macos")]
            let result = worker.close_for_reconcile().map_err(|_| Error::Shutdown);
            if result.is_err() {
                self.cleanup_failed = true;
                // An unresolved child must not release the device instance
                // exclusion and permit another owner in this process lifetime.
                if let Some(guard) = self.guard.take() {
                    std::mem::forget(guard);
                }
            }
            result?;
        }
        self.guard = None;
        Ok(())
    }
    fn check_generation(&mut self) {
        #[cfg(windows)]
        if self.identity_only
            && matches!(
                bootstrap::identity_only(&self.root, self.store.as_ref(), self.environment),
                Ok(false) | Err(Error::Authority)
            )
        {
            // First canonical data requires full recovery and fresh trust scan.
            // No settings or cached identity can clear a preexisting hold.
            if let Ok(permit) = self.admission.enter(&self.root) {
                let _ = self.admission.require_derived_rebuild(&self.root, &permit);
            }
            self.view.error = Some(Error::Held);
        }
        if matches!(
            self.admission.enter(&self.root),
            Err(crate::runtime_admission::AdmissionError::Held(
                crate::runtime_admission::RuntimeHold::DerivedRebuildRequired
            ))
        ) && let Some(owner) = &self.recovery_owner
            && let Ok(mut instance) = owner.try_lock()
        {
            let _ = crate::incoming_publication::recover_live_with_instance(
                &self.root,
                &mut instance,
                self.store.as_ref(),
                self.environment,
            );
        }
        let stale = self.guard.as_ref().is_some_and(|g| g.check().is_err());
        if stale || self.view.error == Some(Error::Held) {
            let _ = self.apply(None, &Interest::new());
        } else if self.worker.is_some()
            && self
                .reader
                .lock()
                .ok()
                .and_then(|r| r.clone())
                .is_none_or(|reader| reader.try_transport_device_id().is_err())
        {
            let error = self.close().err().unwrap_or(Error::Start);
            self.view.state = State::Attention;
            self.view.error = Some(error);
        }
        if self.view.state == State::ReadyLocally
            && self.view.error.is_none()
            && Instant::now() >= self.next_export
        {
            self.next_export = Instant::now() + Duration::from_secs(5);
            let reader = self.reader.lock().ok().and_then(|r| r.clone());
            if let (Some(guard), Some(reader)) = (&self.guard, reader)
                && let Ok(identity) = reader.try_transport_device_id()
            {
                match self.exporter.tick(
                    &self.root,
                    &self.admission,
                    guard,
                    &identity,
                    (self.store.as_ref(), self.environment),
                    &self.stop,
                ) {
                    Ok(report) => {
                        if report.withheld > 0 {
                            eprintln!(
                                "component=sealed_export status=partial reason=cutoff_boundary withheld_packs={}",
                                report.withheld
                            );
                        }
                    }
                    Err(super::super::sealed_export::Error::Deferred) => {}
                    Err(error) => {
                        eprintln!("component=sealed_export status=refused error={error:?}");
                        if error == super::super::sealed_export::Error::Authority
                            && let Ok(permit) = self.admission.exclusive(
                                &self.root,
                                Instant::now() + Duration::from_secs(10),
                                false,
                            )
                        {
                            permit.finish(Some(
                                crate::runtime_admission::RuntimeHold::RecoveryRequired,
                            ));
                        }
                        self.view.error = Some(self.close().err().unwrap_or(Error::Storage));
                        self.view.state = State::Attention;
                    }
                }
            }
        }
    }
    fn apply(&mut self, update: Option<Update>, interest: &Interest) -> Result<View, Error> {
        let result = self.apply_inner(update, interest);
        let mut restored = false;
        if matches!(
            result,
            Err(Error::StaleRevision | Error::UnknownDevice | Error::InvalidEndpoint)
        ) && self.worker.is_none()
            && !self.cleanup_failed
        {
            // A stale editor never changes preferences. Restore from a NEW
            // canonical scan rather than leaving a valid saved configuration off.
            restored = self.apply_inner(None, interest).is_ok();
        }
        if let Err(error) = result
            && !restored
        {
            self.view.state = if error == Error::NotEnrolled {
                State::NotEnrolled
            } else {
                State::Attention
            };
            self.view.error = Some(error);
            // Cached rows are not a fresh trust list when authority is held.
            if matches!(
                error,
                Error::Held | Error::Authority | Error::LocalRestricted
            ) {
                self.view.devices.clear();
            }
        }
        result.map(|()| self.view.clone())
    }
    fn apply_inner(&mut self, update: Option<Update>, interest: &Interest) -> Result<(), Error> {
        interest.check(&self.stop)?;
        if self.cleanup_failed {
            return Err(Error::Shutdown);
        }
        if self.location.is_none() {
            return Err(Error::NotEnrolled);
        }
        #[cfg(windows)]
        if self.identity_only && update.is_some() {
            return Err(Error::NotEnrolled);
        }
        if let Some(update) = &update {
            update.validate()?;
        }
        // Requesting exclusive admission revokes old ready handles even on a
        // timeout. Never wait for canonical locks while holding ordinary work.
        let permit = match self.admission.exclusive(
            &self.root,
            Instant::now() + Duration::from_secs(10),
            false,
        ) {
            Ok(permit) => permit,
            Err(_) => {
                self.close()?;
                return Err(Error::Held);
            }
        };
        if let Err(error) = self.close() {
            permit.finish(None);
            return Err(error);
        }
        let result = (|| {
            self.registry
                .invalidate_all()
                .map_err(|_| Error::Unavailable)?;
            let location = self.location.as_ref().ok_or(Error::NotEnrolled)?;
            let (identity, ports) = location.candidate(self.store.as_ref(), self.environment)?;
            #[cfg(windows)]
            {
                self.identity_only =
                    bootstrap::identity_only(&self.root, self.store.as_ref(), self.environment)?;
                if self.identity_only {
                    if update.is_some() {
                        return Err(Error::NotEnrolled);
                    }
                    self.guard = Some(
                        permit
                            .transport_guard(&self.root)
                            .map_err(|_| Error::Held)?,
                    );
                    // Original bootstrap image admits self/loopback only and
                    // refuses any persisted exchange image or transaction.
                    self.worker = Some(
                        Worker::spawn_existing_guarded(
                            location.profile.clone(),
                            location.package.clone(),
                            self.environment,
                            ports,
                            self.guard.clone(),
                        )
                        .map_err(|_| Error::Start)?,
                    );
                    let reader = self.worker.as_ref().ok_or(Error::Start)?.status_reader();
                    let deadline = Instant::now() + Duration::from_secs(25);
                    let started = (|| {
                        loop {
                            interest.check(&self.stop)?;
                            if let Ok(current) = reader.try_transport_device_id() {
                                if current != identity {
                                    return Err(Error::Start);
                                }
                                break;
                            }
                            if Instant::now() >= deadline {
                                return Err(Error::Start);
                            }
                            std::thread::sleep(Duration::from_millis(20));
                        }
                        *self.reader.lock().map_err(|_| Error::Unavailable)? = Some(reader);
                        Ok(())
                    })();
                    if let Err(error) = started {
                        self.close()?;
                        return Err(error);
                    }
                    self.view.state = State::NotEnrolled;
                    self.view.error = None;
                    self.view.local_transport_port = Some(ports.sync);
                    self.view.devices.clear();
                    return Ok(());
                }
            }
            let brain = crate::identity_runtime::load_brain_identity(&self.root)
                .map_err(|_| Error::Authority)?;
            let resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
                &self.root,
                &brain.brain_id,
                self.store.as_ref(),
                self.environment,
            )
            .map_err(|_| Error::Authority)?;
            let anchor = resolver.local_trust_anchor().ok_or(Error::Authority)?;
            interest.check(&self.stop)?;
            // Reserve the bounded owned startup handshake after canonical scan.
            let scan_deadline = interest
                .deadline
                .checked_sub(Duration::from_secs(50))
                .ok_or(Error::Unavailable)?;
            if Instant::now() >= scan_deadline {
                return Err(Error::Unavailable);
            }
            let snapshot =
                crate::knowledge_materialization::scan_verified_canonical_from_local_anchor_until(
                    &self.root,
                    &brain.brain_id,
                    &anchor,
                    Some(scan_deadline),
                )
                .map_err(|error| match error {
                    crate::knowledge_materialization::KnowledgeMaterializationError::DeadlineExceeded => Error::Unavailable,
                    _ => Error::Authority,
                })?;
            let peers = ExchangePeers::from_snapshot(&snapshot, &self.root, &identity)
                .map_err(|error| match error {
                    crate::managed_profile::exchange_policy::ExchangePolicyError::LocalRestricted => Error::LocalRestricted,
                    _ => Error::Authority,
                })?;
            if self.settings.is_none() {
                self.settings = Some(SettingsStore::open(&self.root)?);
            }
            let settings = self.settings.as_ref().ok_or(Error::Storage)?;
            self.preferences = settings.load()?;
            if let Some(update) = &update {
                let preferences = self.preferences.update(update, &peers, ports.sync)?;
                interest.check(&self.stop)?;
                settings.save(&preferences)?;
                self.preferences = preferences;
            }
            let endpoints = self.preferences.endpoints(&peers);
            self.view = View {
                revision: self.preferences.revision.to_string(),
                paused: self.preferences.paused,
                listen_address: self.preferences.listen,
                local_transport_port: Some(ports.sync),
                devices: self.preferences.devices(&peers),
                state: State::Applying,
                error: None,
            };
            let local_only = self.preferences.paused
                || peers.bindings().is_empty()
                || self.preferences.listen.is_none()
                || endpoints.is_none();
            let policy = if local_only {
                ExchangeConfigPolicy::local_only(&peers, ports)
            } else {
                ExchangeConfigPolicy::new(
                    &peers,
                    ports,
                    SocketAddrV4::new(
                        self.preferences.listen.ok_or(Error::InvalidEndpoint)?,
                        ports.sync,
                    ),
                    endpoints.ok_or(Error::InvalidEndpoint)?,
                )
            }
            .map_err(|_| Error::InvalidEndpoint)?;
            let tree = crate::managed_profile::platform::ensure_exchange_tree(&self.root)
                .map_err(|_| Error::Storage)?;
            interest.check(&self.stop)?;
            if Instant::now() >= scan_deadline {
                return Err(Error::Unavailable);
            }
            // Retain exclusion before handing off to native startup, including
            // cancellation or namespace failure immediately after it returns.
            self.guard = Some(
                permit
                    .transport_guard(&self.root)
                    .map_err(|_| Error::Held)?,
            );
            #[cfg(windows)]
            let worker = Worker::start_exchange(
                location.profile.clone(),
                location.package.clone(),
                self.environment,
                &policy,
                &permit,
            )
            .map_err(|_| Error::Start)?;
            #[cfg(target_os = "macos")]
            let worker =
                Worker::start_exchange(self.root.join("managed-transport"), &policy, &permit)
                    .map_err(|_| Error::Start)?;
            // Retain ownership before any fallible post-start validation so an
            // uncertain close permanently latches this controller.
            self.worker = Some(worker);
            let validation = (|| {
                interest.check(&self.stop)?;
                tree.validate().map_err(|_| Error::Storage)?;
                let reader = self.worker.as_ref().ok_or(Error::Start)?.status_reader();
                if reader.try_transport_device_id().map_err(|_| Error::Start)? != identity {
                    return Err(Error::Start);
                }
                *self.reader.lock().map_err(|_| Error::Unavailable)? = Some(reader);
                Ok(())
            })();
            if let Err(error) = validation {
                self.close()?;
                return Err(error);
            }
            self.view = View {
                revision: self.preferences.revision.to_string(),
                paused: self.preferences.paused,
                listen_address: self.preferences.listen,
                local_transport_port: Some(ports.sync),
                devices: self.preferences.devices(&peers),
                state: if self.preferences.paused {
                    State::Paused
                } else if local_only {
                    State::AddressNeeded
                } else {
                    State::ReadyLocally
                },
                error: None,
            };
            Ok(())
        })();
        // Transport/settings failures cannot authorize a restart or clear an
        // existing security hold. Canonical trust failure itself keeps the gate.
        permit.finish(if result == Err(Error::Authority) {
            Some(crate::runtime_admission::RuntimeHold::RecoveryRequired)
        } else {
            None
        });
        result
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedPeer {
    device_id: String,
    transport_id: String,
    endpoint: SocketAddrV4,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Preferences {
    schema: u32,
    revision: u64,
    paused: bool,
    listen: Option<Ipv4Addr>,
    peers: Vec<SavedPeer>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            schema: 1,
            revision: 0,
            paused: true,
            listen: None,
            peers: Vec::new(),
        }
    }
}
impl Preferences {
    fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > MAX_FRAME {
            return Err(Error::Storage);
        }
        let text = std::str::from_utf8(bytes).map_err(|_| Error::Storage)?;
        cryptex_events::reject_duplicate_json_keys(text).map_err(|_| Error::Storage)?;
        let value: Self = serde_json::from_str(text).map_err(|_| Error::Storage)?;
        if value.schema != 1
            || value
                .peers
                .iter()
                .any(|p| !crate::syncthing_rest::native_id_shape(&p.transport_id))
        {
            return Err(Error::Storage);
        }
        Update {
            expected_revision: value.revision.to_string(),
            paused: value.paused,
            listen_address: value.listen,
            peers: value
                .peers
                .iter()
                .map(|p| PeerAddress {
                    device_id: p.device_id.clone(),
                    endpoint: p.endpoint,
                })
                .collect(),
        }
        .validate()
        .map_err(|_| Error::Storage)?;
        Ok(value)
    }
    fn update(
        &self,
        update: &Update,
        peers: &ExchangePeers<'_>,
        local_port: u16,
    ) -> Result<Self, Error> {
        peers.revalidate().map_err(|_| Error::Authority)?;
        if update.validate()? != self.revision {
            return Err(Error::StaleRevision);
        }
        let mut endpoints = BTreeSet::new();
        if let Some(ip) = update.listen_address {
            endpoints.insert(SocketAddrV4::new(ip, local_port));
        }
        let mut saved = Vec::new();
        for input in &update.peers {
            let peer = peers
                .bindings()
                .iter()
                .find(|p| p.device_id.as_str() == input.device_id)
                .ok_or(Error::UnknownDevice)?;
            if !endpoints.insert(input.endpoint) {
                return Err(Error::InvalidEndpoint);
            }
            saved.push(SavedPeer {
                device_id: input.device_id.clone(),
                transport_id: peer.transport_device_id.clone(),
                endpoint: input.endpoint,
            });
        }
        saved.sort_by(|a, b| a.device_id.cmp(&b.device_id));
        Ok(Self {
            schema: 1,
            revision: self.revision.checked_add(1).ok_or(Error::Storage)?,
            paused: update.paused,
            listen: update.listen_address,
            peers: saved,
        })
    }
    fn devices(&self, peers: &ExchangePeers<'_>) -> Vec<Device> {
        peers
            .bindings()
            .iter()
            .map(|peer| Device {
                device_id: peer.device_id.as_str().into(),
                transport_id: peer.transport_device_id.clone(),
                endpoint: self
                    .peers
                    .iter()
                    .find(|saved| {
                        saved.device_id == peer.device_id.as_str()
                            && saved.transport_id == peer.transport_device_id
                    })
                    .map(|saved| saved.endpoint),
            })
            .collect()
    }
    fn endpoints(&self, peers: &ExchangePeers<'_>) -> Option<BTreeMap<String, SocketAddrV4>> {
        self.devices(peers)
            .into_iter()
            .map(|p| Some((p.transport_id, p.endpoint?)))
            .collect()
    }
}

// One private daemon-owned directory, independent of the active engine's lease.
// Retain the ORIGINAL namespace for the lifetime of the controller. Staging is
// never interpreted as a successful save or silently discarded.
struct SettingsStore {
    directory: PrivateDirectory,
}
impl SettingsStore {
    fn open(root: &Path) -> Result<Self, Error> {
        let path = root.join("connection-settings");
        let exists = path.try_exists().map_err(|_| Error::Storage)?;
        let directory = PrivateDirectory::acquire(&path, !exists).map_err(|_| Error::Storage)?;
        Ok(Self { directory })
    }
    fn read_file(&self, name: &str) -> Result<Option<Vec<u8>>, Error> {
        let Some(file) = self.directory.open_read(name).map_err(|_| Error::Storage)? else {
            return Ok(None);
        };
        let mut bytes = Vec::new();
        file.take((MAX_FRAME + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::Storage)?;
        if bytes.len() > MAX_FRAME {
            return Err(Error::Storage);
        }
        Ok(Some(bytes))
    }
    fn load(&self) -> Result<Preferences, Error> {
        if self.read_file(Leaf::Settings.temporary())?.is_some() {
            return Err(Error::PendingSettings);
        }
        match self.read_file(Leaf::Settings.name())? {
            Some(bytes) => {
                let value = Preferences::decode(&bytes)?;
                self.directory
                    .sync_settings_file()
                    .map_err(|_| Error::Storage)?;
                Ok(value)
            }
            None => Ok(Preferences::default()),
        }
    }
    fn save(&self, value: &Preferences) -> Result<(), Error> {
        let mut bytes = serde_json::to_vec(value).map_err(|_| Error::Storage)?;
        bytes.push(b'\n');
        if bytes.len() > MAX_FRAME {
            return Err(Error::Storage);
        }
        self.directory
            .replace_exchange_file(Leaf::Settings, &bytes)
            .map_err(|_| Error::Storage)?;
        self.directory
            .sync_settings_file()
            .map_err(|_| Error::Storage)?;
        if self.read_file(Leaf::Settings.name())? != Some(bytes)
            || self.read_file(Leaf::Settings.temporary())?.is_some()
        {
            return Err(Error::Storage);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
