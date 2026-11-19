//! Default-off existing-profile owner. No synthetic seed, credential creation,
//! repair override, fixture status or request-ID fault exists on this path.
use super::*;
use crate::approval_session::ApprovalSessionRegistry;
use crate::import_staging::macos_fs::{Directory, RuntimeIdentity, require_product_os};
use crate::import_staging::macos_inbox::Inbox;
use crate::m8_owner_channel::local_startup::{BoundConnection, Incoming};
use crate::m8_owner_channel::transport_helper::StatusReader;
use crate::managed_profile::incoming::{IncomingController, IncomingControllerError, IncomingTick};
use std::os::unix::fs::MetadataExt;

/// Fixed progress labels only. They diagnose a blocked startup operation, but
/// confer no readiness, identity or publication authority.
pub(crate) fn trace_stage<T, E>(
    _name: &'static str,
    work: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
    #[cfg(feature = "m8-development-product")]
    eprintln!(
        "{}",
        serde_json::json!({"m8ProfileStage": _name, "phase": "begin"})
    );
    let result = work();
    #[cfg(feature = "m8-development-product")]
    eprintln!(
        "{}",
        serde_json::json!({"m8ProfileStage": _name,
        "phase": if result.is_ok() { "complete" } else { "failed" }})
    );
    result
}

/// This read-only preflight is not publication/owner authority. The live domain
/// repeats anchor/policy verification under the actual instance admission.
pub(crate) fn validate_existing(root: &Path) -> io::Result<()> {
    require_product_os()?;
    crate::paths::assert_outside_repository(root).map_err(io::Error::other)?;
    let directory = Directory::open(root, true)?;
    let metadata = std::fs::symlink_metadata(root)?;
    if metadata.mode() & 0o077 != 0 {
        return Err(invalid_data("native_profile_not_private"));
    }
    let brain = load_brain_identity(root).map_err(|_| invalid_data("native_profile_brain"))?;
    let local = load_local_device_identity(root, &brain.brain_id)
        .map_err(|_| invalid_data("native_profile_identity"))?;
    DeviceId::new(local.device_id).map_err(|_| invalid_data("native_profile_device"))?;
    // Pre-authentication child CLI preflight reads public metadata only.
    // The OS-backed key is loaded after native image/content attribution,
    // inside prepare_existing_runtime and the admitted live context.
    directory.validate()
}

fn bind_verified_namespace<S: SecureStore + ?Sized>(
    root: &Path,
    instance: &crate::instance::InstanceLock,
    namespace: &RuntimeIdentity,
    store: &S,
    environment: SecureStoreEnvironment,
) -> io::Result<()> {
    use cryptex_events::TrustedDaemonKeyResolver;
    namespace.validate()?;
    let _permit = bound_runtime_admission(root, instance)?
        .enter(root)
        .map_err(io::Error::other)?;
    let brain = load_brain_identity(root).map_err(io::Error::other)?;
    let resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
        root,
        &brain.brain_id,
        store,
        environment,
    )
    .map_err(io::Error::other)?;
    let anchor = resolver
        .local_trust_anchor()
        .ok_or_else(|| invalid_data("native_profile_anchor"))?;
    let snapshot =
        crate::knowledge_materialization::scan_verified_canonical_from_local_anchor_until(
            root,
            &brain.brain_id,
            &anchor,
            None,
        )
        .map_err(io::Error::other)?;
    if snapshot.verified_trust_context().is_none() || !snapshot.locks_runtime(root) {
        return Err(invalid_data("native_profile_namespace_unavailable"));
    }
    for device in snapshot.locked_devices() {
        // pin() validates existing capabilities and never refreshes them to a
        // replacement directory. Keep these in the original live Backend.
        namespace.pin(&cryptex_storage::canonical_events_device_dir(root, device))?;
    }
    if !snapshot.locks_runtime(root) {
        return Err(invalid_data("native_profile_namespace_changed"));
    }
    namespace.validate()
}

fn prepare_existing_runtime(
    root: &Path,
    instance: &mut crate::instance::InstanceLock,
    namespace: &RuntimeIdentity,
) -> io::Result<()> {
    namespace.validate()?;
    trace_stage("recover", || {
        crate::incoming_publication::recover_live_with_instance(
            root,
            instance,
            &crate::MacosKeychainStore::new(),
            SecureStoreEnvironment::Dev,
        )
    })
    .map_err(io::Error::other)?;
    trace_stage("namespace", || {
        bind_verified_namespace(
            root,
            instance,
            namespace,
            &crate::MacosKeychainStore::new(),
            SecureStoreEnvironment::Dev,
        )
    })
}

fn context(
    root: &Path,
    instance: &crate::instance::InstanceLock,
    transport: Option<StatusReader>,
) -> io::Result<SearchContext> {
    validate_existing(root)?;
    let context = SearchContext {
        runtime_root: root.to_owned(),
        admission: bound_runtime_admission(root, instance)?,
        operation: None,
        store: Arc::new(crate::MacosKeychainStore::new()),
        environment: SecureStoreEnvironment::Dev,
        expected_desktop: Arc::new(DesktopProcessIdentity {
            pid: unsafe { libc::getppid() } as u32,
        }),
        projection_sync: None,
        #[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
        connections: None,
        transport_identity: transport,
        #[cfg(test)]
        isolated_attachment_writer: false,
        #[cfg(test)]
        purge_reviews: None,
        #[cfg(test)]
        transport_device_id_override: None,
    };
    let _permit = context.admission.enter(root).map_err(io::Error::other)?;
    let brain = load_brain_identity(root).map_err(|_| invalid_data("native_profile_brain"))?;
    use cryptex_events::TrustedDaemonKeyResolver;
    let resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
        root,
        &brain.brain_id,
        context.store.as_ref(),
        context.environment,
    )
    .map_err(|_| invalid_data("native_profile_anchor"))?;
    let anchor = resolver
        .local_trust_anchor()
        .ok_or_else(|| invalid_data("native_profile_anchor"))?;
    let snapshot =
        crate::knowledge_materialization::scan_verified_canonical_from_local_anchor_until(
            root,
            &brain.brain_id,
            &anchor,
            None,
        )
        .map_err(|_| invalid_data("native_profile_canonical_unavailable"))?;
    if snapshot.verified_trust_context().is_none() {
        return Err(invalid_data("native_profile_policy_held"));
    }
    drop(snapshot);
    drop(_permit);
    Ok(context)
}

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Protection {
    LocalOnly,
    Attention,
}
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum IncomingState {
    Idle,
    WaitingForTransport,
    Deferred,
    Published,
    Rejected,
    Held,
    Unavailable,
}
#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Status {
    pub schema_version: u32,
    pub protection: Protection,
    pub incoming: IncomingState,
    pub transport_observed: bool,
    // M8 has no authenticated remote durable receipt here. Never expose
    // 'protected' or 'replicated' from helper readiness/local publication.
    pub remote_durability_verified: bool,
}
impl Status {
    fn new(incoming: IncomingState, transport_observed: bool) -> Self {
        Self {
            schema_version: 1,
            protection: if matches!(
                incoming,
                IncomingState::Held | IncomingState::Unavailable | IncomingState::Rejected
            ) {
                Protection::Attention
            } else {
                Protection::LocalOnly
            },
            incoming,
            transport_observed,
            remote_durability_verified: false,
        }
    }
}

struct Backend {
    namespace: RuntimeIdentity,
    inbox: Inbox,
    controller: IncomingController,
    status: Status,
}
impl Backend {
    fn new(root: &Path) -> io::Result<Self> {
        let namespace = RuntimeIdentity::capture(root)?;
        namespace.pin(&root.join("canonical/events"))?;
        Ok(Self {
            namespace,
            inbox: Inbox::new(root)?,
            controller: IncomingController::new(root.to_owned(), SecureStoreEnvironment::Dev)
                .map_err(io::Error::other)?,
            status: Status::new(IncomingState::Idle, false),
        })
    }
    fn tick(
        &mut self,
        context: &SearchContext,
        instance: &mut crate::instance::InstanceLock,
    ) -> io::Result<Status> {
        self.namespace.validate()?;
        let transport = context
            .connections
            .as_ref()
            .and_then(|handle| handle.reader())
            .or_else(|| context.transport_identity.clone());
        let observed = transport.as_ref().is_some_and(|reader| {
            reader.belongs_to_runtime(&context.runtime_root)
                && reader.try_transport_device_id().is_ok()
        });
        // Inspect the shared barrier before discovery; held bytes remain exactly
        // where R4 recovery retained them, and this route cannot clear a hold.
        match context.admission.enter(&context.runtime_root) {
            Ok(permit) => drop(permit),
            Err(crate::runtime_admission::AdmissionError::Held(_)) => {
                self.status = Status::new(IncomingState::Held, observed);
                return Ok(self.status.clone());
            }
            Err(_) => {
                self.status = Status::new(IncomingState::Deferred, observed);
                return Ok(self.status.clone());
            }
        }
        let Some(transport) = transport.as_ref() else {
            self.status = Status::new(IncomingState::WaitingForTransport, false);
            return Ok(self.status.clone());
        };
        let paths = match self.inbox.discover() {
            Ok(paths) => paths,
            Err(error) => {
                // Discovery errors contain fixed adapter stages/OS codes only;
                // no parsed record bodies, paths or credential data enter them.
                eprintln!(
                    "component=mac_native_incoming stage=discovery status=unavailable kind={:?} diagnostic={error}",
                    error.kind()
                );
                self.status = Status::new(IncomingState::Unavailable, observed);
                return Ok(self.status.clone());
            }
        };
        if context.connections.is_none() && !paths.is_empty() && observed {
            prepare_existing_runtime(&context.runtime_root, instance, &self.namespace)?;
        }
        if !paths.is_empty() {
            self.controller.enqueue(paths).map_err(io::Error::other)?;
        }
        self.status = match self
            .controller
            .poll(transport, instance, context.store.as_ref())
        {
            Ok(IncomingTick::Published(_)) => {
                bind_verified_namespace(
                    &context.runtime_root,
                    instance,
                    &self.namespace,
                    context.store.as_ref(),
                    context.environment,
                )?;
                self.inbox.consumed()?;
                Status::new(IncomingState::Published, observed)
            }
            Ok(IncomingTick::Idle) => Status::new(IncomingState::Idle, observed),
            Ok(IncomingTick::Deferred) => Status::new(IncomingState::Deferred, observed),
            Ok(IncomingTick::WaitingForTransport) => {
                Status::new(IncomingState::WaitingForTransport, false)
            }
            Err(IncomingControllerError::Publication(
                crate::incoming_publication::IncomingPublicationError::Rejected(_),
            )) => {
                self.inbox.rejected()?;
                Status::new(IncomingState::Rejected, observed)
            }
            Err(IncomingControllerError::Publication(_))
            | Err(IncomingControllerError::Admission(_)) => {
                Status::new(IncomingState::Held, observed)
            }
            Err(error) => {
                // IncomingController::Display is a closed static code inventory.
                eprintln!(
                    "component=mac_native_incoming stage=controller status=unavailable code={error}"
                );
                Status::new(IncomingState::Unavailable, observed)
            }
        };
        self.namespace.validate()?;
        self.status.transport_observed = if let Some(handle) = &context.connections {
            handle
                .reader()
                .is_some_and(|reader| reader.try_transport_device_id().is_ok())
        } else {
            observed
        };
        Ok(self.status.clone())
    }
}

fn send_checked(
    namespace: &RuntimeIdentity,
    send: impl FnOnce() -> io::Result<()>,
) -> io::Result<()> {
    namespace.validate()?;
    send()?;
    namespace.validate()
}

pub(crate) fn serve(
    connection: &mut BoundConnection<'_>,
    root: &Path,
    instance: &mut crate::instance::InstanceLock,
) -> io::Result<()> {
    trace_stage("validate", || validate_existing(root))?;
    let mut backend = trace_stage("backend", || Backend::new(root))?;
    bound_runtime_admission(root, instance)?
        .bind_namespace(backend.namespace.clone())
        .map_err(io::Error::other)?;
    trace_stage("prepare", || {
        prepare_existing_runtime(root, instance, &backend.namespace)
    })?;
    // Open an explicitly enrolled owned helper only. Absence is local-only;
    // never generate/adopt a profile or infer peer trust during native startup.
    let helper_root = root.join("managed-transport");
    let registry = ApprovalSessionRegistry::default();
    let mut context = trace_stage("context", || context(root, instance, None))?;
    let mut connections = crate::managed_profile::connection_settings::Owner::start(
        root,
        instance,
        context.store.clone(),
        context.environment,
        helper_root
            .try_exists()?
            .then(|| crate::managed_profile::connection_settings::Location::mac(root)),
        registry.clone(),
        None,
    )
    .map_err(io::Error::other)?;
    context.connections = Some(connections.handle());
    // Complete the initial coordinator admission before accepting owner frames.
    // A transport-domain Attention result still permits local profile use.
    connections
        .handle()
        .request(None)
        .map_err(io::Error::other)?;
    // No Unix or derived worker is exposed until it shares the original
    // native namespace capability set and verified device pins.
    #[cfg(feature = "m8-development-product")]
    trace_stage("unix_data", || {
        super::spawn_retrieval_server(root, instance, context.store.clone(), context.environment)
    })?;
    let mut domain = super::native_domain::Domain::from_context(context.clone(), &registry);
    send_checked(&backend.namespace, || {
        connection.ready(Instant::now() + IPC_RESPONSE_DEADLINE)
    })?;
    loop {
        backend.namespace.validate()?;
        match connection.receive(Instant::now() + IPC_APPROVAL_SESSION_DEADLINE)? {
            Incoming::Close => {
                // Drop pending grants before stopping the helper. No grant or
                // derived-worker lease survives into a replacement process.
                drop(domain);
                connections.shutdown().map_err(io::Error::other)?;
                return Ok(());
            }
            Incoming::Ping => {
                backend.tick(&context, instance)?;
                send_checked(&backend.namespace, || {
                    connection.send(b"pong", Instant::now() + IPC_RESPONSE_DEADLINE)
                })?;
            }
            Incoming::Status => {
                let status = backend.tick(&context, instance)?;
                send_checked(&backend.namespace, || {
                    connection.send(
                        &serde_json::to_vec(&status)?,
                        Instant::now() + IPC_RESPONSE_DEADLINE,
                    )
                })?;
            }
            Incoming::Settings(received) => {
                let bytes = connection.consume_settings(received)?;
                if super::is_retirement_notice(&bytes) {
                    domain = super::native_domain::Domain::from_context(context.clone(), &registry);
                    let (reply, _lease) = crate::retirement_notice::reply(
                        &bytes,
                        root,
                        instance,
                        context.store.as_ref(),
                        context.environment,
                    )?;
                    send_checked(&backend.namespace, || {
                        connection.send(&reply, Instant::now() + IPC_RESPONSE_DEADLINE)
                    })?;
                    continue;
                }
                if crate::managed_profile::connection_settings::Request::decode(&bytes)
                    .is_ok_and(|request| request.update.is_some())
                {
                    domain = super::native_domain::Domain::from_context(context.clone(), &registry);
                }
                let mut reply = super::connection_settings_reply(&bytes, Some(&context), true);
                let _response_lease =
                    super::connection_settings_response_lease(Some(&context), &mut reply);
                send_checked(&backend.namespace, || {
                    connection.send(
                        &serde_json::to_vec(&reply)?,
                        Instant::now() + IPC_RESPONSE_DEADLINE,
                    )
                })?;
            }
            Incoming::Request(received) => {
                backend.tick(&context, instance)?;
                // Publication may revoke transport while rebuilding canonical
                // views. Settle its fresh roster before the next owner admission.
                connections
                    .handle()
                    .request(None)
                    .map_err(io::Error::other)?;
                let response = domain.dispatch(connection, *received)?;
                let reconcile = response.requires_reconcile;
                if reconcile {
                    response.hold_rebuild(&context)?;
                }
                // Validate while the real response still owns ordinary admission.
                // No observed drift can first be reported as native success.
                send_checked(&backend.namespace, || response.send(connection))?;
                if reconcile {
                    prepare_existing_runtime(root, instance, &backend.namespace)?;
                    connections
                        .handle()
                        .request(None)
                        .map_err(io::Error::other)?;
                }
            }
        }
        backend.namespace.validate()?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_verified_device_binding_survives_reconciliation_and_refuses_subtree_replacement() {
        let root = crate::test_temp::exclusive_temp_dir("native-live-device-drift");
        let instance = crate::instance::try_acquire_instance(&root).unwrap();
        let store = crate::FakeSecureStore::new();
        let brain = "aa5d7ae0-5321-4f53-a91e-36a411d88591";
        crate::signed_transfer_probe::create(
            &root,
            &store,
            brain,
            "24376f10-1d44-4673-960a-cb46ff26ab65",
        )
        .unwrap();
        let local = load_local_device_identity(&root, brain).unwrap();
        let device = DeviceId::new(local.device_id).unwrap();
        let namespace = RuntimeIdentity::capture(&root).unwrap();
        namespace.pin(&root.join("canonical/events")).unwrap();
        bind_verified_namespace(
            &root,
            &instance,
            &namespace,
            &store,
            SecureStoreEnvironment::Dev,
        )
        .unwrap();
        let directory = cryptex_storage::canonical_events_device_dir(&root, &device);
        let preserved = root.join("canonical/preserved-device");
        std::fs::rename(&directory, &preserved).unwrap();
        std::fs::create_dir(&directory).unwrap();
        for entry in std::fs::read_dir(&preserved).unwrap() {
            let entry = entry.unwrap();
            std::fs::copy(entry.path(), directory.join(entry.file_name())).unwrap();
        }
        assert!(
            bind_verified_namespace(
                &root,
                &instance,
                &namespace,
                &store,
                SecureStoreEnvironment::Dev
            )
            .is_err()
        );
        let mut sent = false;
        assert!(
            send_checked(&namespace, || {
                sent = true;
                Ok(())
            })
            .is_err()
        );
        assert!(!sent);
        assert!(preserved.join("00000000000000000000.seg").is_file());
        assert!(directory.join("00000000000000000000.seg").is_file());
        drop(instance);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn observed_namespace_drift_after_work_sends_no_success_reply() {
        use std::os::unix::fs::DirBuilderExt;
        let parent = crate::test_temp::exclusive_temp_dir("native-pre-reply-drift");
        let root = parent.join("runtime");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap();
        let namespace = RuntimeIdentity::capture(&root).unwrap();
        std::fs::write(root.join("preserved"), b"original").unwrap();
        std::fs::rename(&root, parent.join("original")).unwrap();
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap();
        std::fs::write(root.join("preserved"), b"replacement").unwrap();
        let mut sent = false;
        assert!(
            send_checked(&namespace, || {
                sent = true;
                Ok(())
            })
            .is_err()
        );
        assert!(!sent);
        assert_eq!(
            std::fs::read(parent.join("original/preserved")).unwrap(),
            b"original"
        );
        assert_eq!(
            std::fs::read(root.join("preserved")).unwrap(),
            b"replacement"
        );
        std::fs::remove_dir_all(parent).unwrap();
    }
    #[test]
    fn transport_readiness_and_local_publication_never_claim_remote_protection() {
        for incoming in [
            IncomingState::Idle,
            IncomingState::Published,
            IncomingState::Held,
            IncomingState::Rejected,
            IncomingState::Unavailable,
        ] {
            let status = Status::new(incoming, true);
            assert!(!status.remote_durability_verified);
            assert_ne!(
                serde_json::to_value(&status).unwrap()["protection"],
                "protected"
            );
            assert_eq!(
                status.protection == Protection::Attention,
                matches!(
                    incoming,
                    IncomingState::Held | IncomingState::Rejected | IncomingState::Unavailable
                )
            );
        }
    }
    #[test]
    fn missing_existing_profile_is_refused_without_creating_runtime_data() {
        let parent = crate::test_temp::exclusive_temp_dir("native-existing-missing");
        let missing = parent.join("missing");
        assert!(validate_existing(&missing).is_err());
        assert!(!missing.exists());
        std::fs::remove_dir_all(parent).unwrap();
    }
}
