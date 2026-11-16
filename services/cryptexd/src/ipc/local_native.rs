//! Feature-only synthetic native domain driver. No product socket/owner gate
//! is changed. Only a content-bound kernel-attributed connection can enter.
use super::*;
use crate::approval_session::ApprovalSessionRegistry;
use crate::m8_owner_channel::local_startup::{BoundConnection, ContentProfile, Incoming};

fn event_hashes(context: &SearchContext) -> io::Result<Vec<String>> {
    let brain =
        load_brain_identity(&context.runtime_root).map_err(|_| invalid_data("fixture_brain"))?;
    let local = load_local_device_identity(&context.runtime_root, &brain.brain_id)
        .map_err(|_| invalid_data("fixture_identity"))?;
    let device = DeviceId::new(local.device_id).map_err(|_| invalid_data("fixture_device"))?;
    let key = load_existing_event_signing_key(
        context.store.as_ref(),
        context.environment,
        &context.runtime_root,
        &device,
    )
    .map_err(|_| invalid_data("fixture_key"))?;
    let snapshot = crate::knowledge_materialization::scan_verified_trust_policy(
        &context.runtime_root,
        &brain.brain_id,
        &device,
        &cryptex_events::SigningKeyId::parse(&key.public.signing_key_id)
            .map_err(|_| invalid_data("fixture_key"))?,
        &key.signing_key.verifying_key().to_bytes(),
        std::slice::from_ref(&device),
    )
    .map_err(|_| invalid_data("fixture_history"))?;
    Ok(snapshot
        .history
        .records()
        .iter()
        .map(|event| event.content_hash().as_str().to_owned())
        .collect())
}

pub(crate) fn serve(
    connection: &mut BoundConnection<'_>,
    root: &Path,
    instance: &crate::instance::InstanceLock,
    profile: ContentProfile,
) -> io::Result<()> {
    // This factory is reachable only after the feature-specific child gate,
    // fresh marked runtime check, native startup and matching content binding.
    let existing = crate::identity_runtime::brain_identity_path(root).exists();
    if existing != profile.resumes() {
        return Err(invalid_data("native_fixture_profile_state"));
    }
    let store: Arc<dyn SecureStore> = match profile {
        ContentProfile::ExistingProfile => {
            return Err(invalid_data("existing_profile_is_not_a_fixture"));
        }
        ContentProfile::Ephemeral => Arc::new(crate::FakeSecureStore::new()),
        ContentProfile::KeychainCreate
        | ContentProfile::KeychainResume
        | ContentProfile::TransportCreate
        | ContentProfile::TransportResume => Arc::new(crate::MacosKeychainStore::new()),
    };
    if !profile.resumes() {
        let event_id = if profile.transport() {
            new_bootstrap_event_id().map_err(invalid_data)?
        } else {
            "d9885c8e-6a01-48bc-8488-cd52c4c1f725".to_owned()
        };
        crate::signed_transfer_probe::create(
            root,
            store.as_ref(),
            "aa5d7ae0-5321-4f53-a91e-36a411d88591",
            &event_id,
        )
        .map_err(|_| invalid_data("native_fixture_seed"))?;
    }
    let mut context = SearchContext {
        runtime_root: root.to_path_buf(),
        admission: bound_runtime_admission(root, instance)?,
        operation: None,
        store,
        environment: SecureStoreEnvironment::Dev,
        // The native connection, not this legacy PID holder, authenticates
        // every exact request. This field only freezes existing domain context.
        expected_desktop: Arc::new(DesktopProcessIdentity {
            pid: unsafe { libc::getppid() } as u32,
        }),
        projection_sync: None,
        #[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
        connections: None,
        transport_identity: None,
        #[cfg(test)]
        transport_device_id_override: None,
    };
    if profile.resumes() {
        let device = crate::m8_owner_channel::local_startup::verified_resume_device(root, profile)
            .map_err(|_| invalid_data("native_fixture_profile"))?;
        match load_existing_event_signing_key(
            context.store.as_ref(),
            context.environment,
            root,
            &device,
        ) {
            Ok(_) => {}
            Err(crate::event_signing_key::EventSigningKeyError::SecureStore(
                crate::SecureStoreError::Missing,
            )) => {
                eprintln!("native_keychain_seed_missing");
                return Err(invalid_data("native_keychain_seed_missing"));
            }
            Err(_) => return Err(invalid_data("native_keychain_seed_unavailable")),
        }
    }
    let original = event_hashes(&context)?;
    let mut transport = if profile.transport() {
        Some(
            crate::m8_owner_channel::transport_helper::Worker::spawn(
                root.join("managed-transport"),
                !profile.resumes(),
            )
            .map_err(io::Error::other)?,
        )
    } else {
        None
    };
    context.transport_identity = transport.as_ref().map(|w| w.status_reader());
    let registry = ApprovalSessionRegistry::default();
    let mut domain = super::native_domain::Domain::from_context(context.clone(), &registry);
    connection.ready(Instant::now() + IPC_RESPONSE_DEADLINE)?;
    loop {
        let incoming = connection.receive(Instant::now() + IPC_APPROVAL_SESSION_DEADLINE)?;
        let received = match incoming {
            Incoming::Close => {
                if let Some(worker) = transport.take() {
                    worker.close().map_err(io::Error::other)?;
                }
                return Ok(()); // pending Drop burns grants, not durable cancellation.
            }
            Incoming::Ping => {
                connection.send(b"pong", Instant::now() + IPC_RESPONSE_DEADLINE)?;
                continue;
            }
            Incoming::Status => {
                let hashes = event_hashes(&context)?;
                let body = serde_json::to_vec(&serde_json::json!({
                    "eventCount": hashes.len(), "prefixPreserved": hashes.starts_with(&original),
                }))?;
                connection.send(&body, Instant::now() + IPC_RESPONSE_DEADLINE)?;
                continue;
            }
            Incoming::Request(request) => *request,
            Incoming::Settings(_) => {
                return Err(io::Error::other("settings_product_profile_required"));
            }
        };
        let mut response = domain.dispatch(connection, received)?;
        // This mutation belongs only to the isolated fixture. Shared/product
        // dispatch never recognizes magic request IDs or removes snapshots.
        if response.reply.request_id.as_deref() == Some("test-accepted-without-snapshot") {
            response.reply.snapshot = None;
        }
        response.send(connection)?;
        // The response owns admission through the actual send. No lease is
        // retained while waiting for the owner's next native message.
    }
}
