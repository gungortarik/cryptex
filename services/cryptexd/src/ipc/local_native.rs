//! Feature-only synthetic native domain driver. No product socket/owner gate
//! is changed. Only a content-bound kernel-attributed connection can enter.
use super::*;
use crate::approval_session::ApprovalSessionRegistry;
use crate::m8_owner_channel::Method;
use crate::m8_owner_channel::local_startup::{BoundConnection, Incoming};

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
) -> io::Result<()> {
    // This factory is reachable only after the feature-specific child gate,
    // fresh marked runtime check, native startup and matching content binding.
    if crate::identity_runtime::brain_identity_path(root).exists() {
        return Err(invalid_data("native_fixture_reused"));
    }
    let store = Arc::new(crate::FakeSecureStore::new());
    crate::signed_transfer_probe::create(
        root,
        store.as_ref(),
        "aa5d7ae0-5321-4f53-a91e-36a411d88591",
        "d9885c8e-6a01-48bc-8488-cd52c4c1f725",
    )
    .map_err(|_| invalid_data("native_fixture_seed"))?;
    let context = SearchContext {
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
        #[cfg(test)]
        transport_device_id_override: None,
    };
    let original = event_hashes(&context)?;
    let registry = ApprovalSessionRegistry::default();
    let mut pending = None;
    connection.ready(Instant::now() + IPC_RESPONSE_DEADLINE)?;
    loop {
        let incoming = connection.receive(Instant::now() + IPC_APPROVAL_SESSION_DEADLINE)?;
        let received = match incoming {
            Incoming::Close => return Ok(()), // pending Drop burns grants, not durable cancellation.
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
            Incoming::Request(request) => connection.consume(*request)?,
        };
        // Consumption above is one-use and tied to this native allocation;
        // ordinary parsed JSON/roles/PIDs cannot construct the sealed value.
        let request = received.request;
        if let Some(session) = pending.take() {
            let session: approval_domain::PendingSession<'_> = session;
            // Reacquire the frozen admission, not a lease from another context.
            let permit = session.admit_followup().map_err(io::Error::other)?;
            let finished = session.finish(request, &permit)?;
            AdmittedReply::new(finished.reply, permit, Instant::now() + finished.timeout)?
                .send_using(|bytes, deadline| connection.send(bytes, deadline))?;
            continue;
        }
        let permit = context.admission.enter(root).map_err(io::Error::other)?;
        let (reply, timeout) = {
            match received.method {
                Method::PrepareOwnerOperation | Method::PrepareApproval => {
                    let missing_snapshot = request.request_id == "test-accepted-without-snapshot";
                    let mut prepared = if received.method == Method::PrepareOwnerOperation {
                        approval_domain::prepare_owner(&context, &registry, request, &permit)?
                    } else {
                        approval_domain::prepare_bootstrap(&context, &registry, request, &permit)?
                    };
                    // Authenticated synthetic fixture fault, never a product
                    // route or a renderer-selectable request ID.
                    if missing_snapshot {
                        prepared.reply.snapshot = None;
                    }
                    pending = prepared.session;
                    (prepared.reply, IPC_RESPONSE_DEADLINE)
                }
                Method::CompleteApproval => (
                    approval_domain::complete(&context, request, &permit),
                    IPC_RESPONSE_DEADLINE,
                ),
                Method::ReadBrainIdentity | Method::CreateOffer | Method::CreateProof => (
                    execute_bootstrap_exchange(&context, request),
                    IPC_RESPONSE_DEADLINE,
                ),
                _ => (
                    bootstrap_approval_response(
                        Some(request.request_id),
                        false,
                        Some("approval_session_missing"),
                        "owner.security.result",
                        None,
                        None,
                        None,
                    ),
                    IPC_REQUEST_DEADLINE,
                ),
            }
        };
        AdmittedReply::new(reply, permit, Instant::now() + timeout)?
            .send_using(|bytes, deadline| connection.send(bytes, deadline))?;
        // The response owns admission through the actual send. No lease is
        // retained while waiting for the owner's next native message.
    }
}
