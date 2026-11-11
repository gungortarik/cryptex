//! Explicit domain fixtures, not authenticated product transports or Mac peer proofs.
use super::*;
use crate::approval_session::{ApprovalSessionError, ApprovalSessionRegistry};
use crate::bootstrap_journal::{BootstrapJournalStore, BootstrapState};

fn events(fixture: &SearchFixture) -> Vec<String> {
    crate::canonical_writer::load_verified_events(
        fixture.store.as_ref(),
        SecureStoreEnvironment::Dev,
        &fixture.root,
        &fixture.device,
    )
    .unwrap()
    .into_iter()
    .map(|event| event.content_hash.as_str().to_owned())
    .collect()
}

fn owner_request(transaction: &str, operation: &str) -> BootstrapApprovalIpcRequest {
    BootstrapApprovalIpcRequest {
        role: None,
        transcript_messages: None,
        transaction_id: Some(transaction.into()),
        owner_operation: if operation == "owner.security.prepare" {
            Some(OwnerSecurityOperationRequest::SelfRetirement)
        } else {
            None
        },
        ..approval_request(operation)
    }
}

fn prepared_bootstrap<'a>(
    fixture: &SearchFixture,
    registry: &'a ApprovalSessionRegistry,
) -> (
    approval_domain::PendingSession<'a>,
    BootstrapApprovalIpcResponse,
    Vec<String>,
) {
    let prefix = synthetic_initiator_prefix(fixture);
    let permit = fixture.context.admission.enter(&fixture.root).unwrap();
    let preparation = approval_domain::prepare_bootstrap(
        &fixture.context,
        registry,
        BootstrapApprovalIpcRequest {
            transcript_messages: Some(prefix.clone()),
            ..approval_request("bootstrap.approval.prepare")
        },
        &permit,
    )
    .unwrap();
    assert!(preparation.reply.accepted, "{:?}", preparation.reply.error);
    drop(permit);
    (preparation.session.unwrap(), preparation.reply, prefix)
}

fn bootstrap_followup(
    reply: &BootstrapApprovalIpcResponse,
    prefix: Vec<String>,
    operation: &str,
) -> BootstrapApprovalIpcRequest {
    BootstrapApprovalIpcRequest {
        request_id: reply.request_id.clone().unwrap(),
        transcript_messages: Some(prefix),
        capability: reply.capability.clone(),
        approval_request_digest: reply.approval_request_digest.clone(),
        ..approval_request(operation)
    }
}

fn assert_burned(registry: &ApprovalSessionRegistry, reply: &BootstrapApprovalIpcResponse) {
    let fresh = registry.open_connection().unwrap();
    assert!(matches!(
        registry.consume(
            &fresh,
            reply.capability.as_deref().unwrap(),
            reply.approval_request_digest.as_deref().unwrap()
        ),
        Err(ApprovalSessionError::UnknownSession)
    ));
}

#[test]
fn domain_abandonment_burns_grant_without_durable_cancellation_or_append() {
    let fixture = prepared_search_fixture("domain-abandon");
    let before = events(&fixture);
    let registry = ApprovalSessionRegistry::default();
    let (session, reply, _) = prepared_bootstrap(&fixture, &registry);
    assert!(!format!("{session:?}").contains(reply.capability.as_deref().unwrap()));
    let exclusive = fixture
        .context
        .admission
        .exclusive(
            &fixture.root,
            Instant::now() + Duration::from_secs(2),
            false,
        )
        .unwrap();
    exclusive.finish(None); // A parked session owns no think-time admission.
    drop(session);
    assert_burned(&registry, &reply);
    let journal = BootstrapJournalStore::new(&fixture.root)
        .unwrap()
        .load()
        .unwrap();
    assert_eq!(journal.transactions[0].state, BootstrapState::OwnerPending);
    assert!(!journal.transactions[0].local_approval_signature_generated);
    assert_eq!(events(&fixture), before);
    fs::remove_dir_all(fixture.root).unwrap();
}

#[test]
fn domain_mismatch_is_terminal_but_a_fresh_review_can_sign_once() {
    let fixture = prepared_search_fixture("domain-mismatch");
    let before = events(&fixture);
    let registry = ApprovalSessionRegistry::default();
    let (session, reply, prefix) = prepared_bootstrap(&fixture, &registry);
    let mut followup = bootstrap_followup(&reply, prefix, "bootstrap.approval.confirm");
    followup.request_id = "another-review".into();
    let permit = session.admit_followup().unwrap();
    let refused = session.finish(followup, &permit).unwrap().reply;
    assert_eq!(refused.error, Some("approval_session_mismatch"));
    drop(permit);
    assert_burned(&registry, &reply);
    let (fresh, new_reply, new_prefix) = prepared_bootstrap(&fixture, &registry);
    assert_ne!(reply.capability, new_reply.capability);
    let permit = fresh.admit_followup().unwrap();
    let signed = fresh
        .finish(
            bootstrap_followup(&new_reply, new_prefix, "bootstrap.approval.confirm"),
            &permit,
        )
        .unwrap()
        .reply;
    assert!(signed.accepted, "{:?}", signed.error);
    assert!(signed.signed_message.is_some());
    drop(permit);
    assert_burned(&registry, &new_reply);
    let journal = BootstrapJournalStore::new(&fixture.root)
        .unwrap()
        .load()
        .unwrap();
    assert!(
        journal.transactions[0].local_approval_signature_generated
            && journal.transactions[0].local_approval_signature_released
    );
    assert_eq!(events(&fixture), before);
    fs::remove_dir_all(fixture.root).unwrap();
}

#[test]
fn domain_replacement_registry_cannot_redeem_old_grant_after_reauthentication() {
    let fixture = prepared_search_fixture("domain-epoch");
    let old = ApprovalSessionRegistry::default();
    let (old_session, old_reply, _) = prepared_bootstrap(&fixture, &old);
    drop(old_session);
    drop(old);
    let replacement = ApprovalSessionRegistry::default();
    let (fresh, fresh_reply, prefix) = prepared_bootstrap(&fixture, &replacement);
    let permit = fresh.admit_followup().unwrap();
    let refused = fresh
        .finish(
            bootstrap_followup(&old_reply, prefix, "bootstrap.approval.confirm"),
            &permit,
        )
        .unwrap()
        .reply;
    assert!(!refused.accepted);
    assert_eq!(refused.error, Some("approval_confirmation_failed"));
    drop(permit);
    assert_burned(&replacement, &fresh_reply);
    assert!(
        !BootstrapJournalStore::new(&fixture.root)
            .unwrap()
            .load()
            .unwrap()
            .transactions[0]
            .local_approval_signature_generated
    );
    fs::remove_dir_all(fixture.root).unwrap();
}

#[test]
fn domain_work_rejects_foreign_admission_incarnation_before_any_journal_work() {
    let fixture = prepared_search_fixture("domain-admission");
    let before = events(&fixture);
    let registry = ApprovalSessionRegistry::default();
    let other = crate::runtime_admission::RuntimeAdmission::detached_for_tests(&fixture.root);
    let foreign = other.enter(&fixture.root).unwrap();
    let prefix = synthetic_initiator_prefix(&fixture);
    let rejected = approval_domain::prepare_bootstrap(
        &fixture.context,
        &registry,
        BootstrapApprovalIpcRequest {
            transcript_messages: Some(prefix),
            ..approval_request("bootstrap.approval.prepare")
        },
        &foreign,
    )
    .unwrap();
    assert_eq!(rejected.reply.error, Some("runtime_mismatch"));
    assert!(rejected.session.is_none());
    let rejected = approval_domain::prepare_owner(
        &fixture.context,
        &registry,
        owner_request(&"cd".repeat(32), "owner.security.prepare"),
        &foreign,
    )
    .unwrap();
    assert_eq!(rejected.reply.error, Some("runtime_mismatch"));
    assert!(rejected.session.is_none());
    let rejected = approval_domain::complete(
        &fixture.context,
        approval_request("bootstrap.approval.complete"),
        &foreign,
    );
    assert_eq!(rejected.error, Some("runtime_mismatch"));
    drop(foreign);
    assert!(
        BootstrapJournalStore::new(&fixture.root)
            .unwrap()
            .load()
            .unwrap()
            .transactions
            .is_empty()
    );
    assert_eq!(events(&fixture), before);
    fs::remove_dir_all(fixture.root).unwrap();
}

#[test]
fn domain_finish_rejects_foreign_permit_and_burns_the_pending_grant() {
    let fixture = prepared_search_fixture("domain-finish-admission");
    let registry = ApprovalSessionRegistry::default();
    let (session, reply, prefix) = prepared_bootstrap(&fixture, &registry);
    let other = crate::runtime_admission::RuntimeAdmission::detached_for_tests(&fixture.root);
    let foreign = other.enter(&fixture.root).unwrap();
    let refused = session
        .finish(
            bootstrap_followup(&reply, prefix, "bootstrap.approval.confirm"),
            &foreign,
        )
        .unwrap()
        .reply;
    assert_eq!(refused.error, Some("runtime_mismatch"));
    drop(foreign);
    assert_burned(&registry, &reply);
    assert!(
        !BootstrapJournalStore::new(&fixture.root)
            .unwrap()
            .load()
            .unwrap()
            .transactions[0]
            .local_approval_signature_generated
    );
    fs::remove_dir_all(fixture.root).unwrap();
}

#[test]
fn domain_pending_context_cannot_be_replaced_by_a_different_runtime_or_store() {
    let mut fixture = prepared_search_fixture("domain-frozen");
    let other = prepared_search_fixture("domain-other");
    let registry = ApprovalSessionRegistry::default();
    let (session, reply, prefix) = prepared_bootstrap(&fixture, &registry);
    fixture.context = other.context.clone(); // The pending session already owns its original bindings.
    let permit = session.admit_followup().unwrap();
    let signed = session
        .finish(
            bootstrap_followup(&reply, prefix, "bootstrap.approval.confirm"),
            &permit,
        )
        .unwrap()
        .reply;
    assert!(signed.accepted, "{:?}", signed.error);
    drop(permit);
    assert!(
        BootstrapJournalStore::new(&other.root)
            .unwrap()
            .load()
            .unwrap()
            .transactions
            .is_empty()
    );
    assert!(
        BootstrapJournalStore::new(&fixture.root)
            .unwrap()
            .load()
            .unwrap()
            .transactions[0]
            .local_approval_signature_generated
    );
    fs::remove_dir_all(fixture.root).unwrap();
    fs::remove_dir_all(other.root).unwrap();
}

#[test]
fn domain_owner_cancel_never_appends_and_confirmation_appends_once() {
    for confirm in [false, true] {
        let fixture = prepared_search_fixture("domain-owner");
        let before = events(&fixture);
        let registry = ApprovalSessionRegistry::default();
        let transaction = "cd".repeat(32);
        let permit = fixture.context.admission.enter(&fixture.root).unwrap();
        let prepared = approval_domain::prepare_owner(
            &fixture.context,
            &registry,
            owner_request(&transaction, "owner.security.prepare"),
            &permit,
        )
        .unwrap();
        assert!(prepared.reply.accepted, "{:?}", prepared.reply.error);
        drop(permit);
        let session = prepared.session.unwrap();
        let permit = session.admit_followup().unwrap();
        let mut followup = owner_request(
            &transaction,
            if confirm {
                "owner.security.confirm"
            } else {
                "owner.security.cancel"
            },
        );
        followup.capability = prepared.reply.capability.clone();
        followup.approval_request_digest = prepared.reply.approval_request_digest.clone();
        let result = session.finish(followup, &permit).unwrap().reply;
        assert!(result.accepted, "{:?}", result.error);
        drop(permit);
        assert_burned(&registry, &prepared.reply);
        let journal = BootstrapJournalStore::new(&fixture.root)
            .unwrap()
            .load()
            .unwrap();
        assert_eq!(
            journal.transactions[0].state,
            if confirm {
                BootstrapState::Anchored
            } else {
                BootstrapState::Cancelled
            }
        );
        let after = events(&fixture);
        assert_eq!(&after[..before.len()], before.as_slice());
        assert_eq!(after.len(), before.len() + usize::from(confirm));
        let permit = fixture.context.admission.enter(&fixture.root).unwrap();
        let retry = approval_domain::prepare_owner(
            &fixture.context,
            &registry,
            owner_request(&transaction, "owner.security.prepare"),
            &permit,
        )
        .unwrap();
        assert!(!retry.reply.accepted);
        assert!(retry.session.is_none());
        drop(permit);
        assert_eq!(events(&fixture), after);
        fs::remove_dir_all(fixture.root).unwrap();
    }
}

#[test]
fn domain_prepare_delivery_failure_drops_session_without_durable_cancel() {
    let fixture = prepared_search_fixture("domain-prepare-send");
    let registry = ApprovalSessionRegistry::default();
    let prefix = synthetic_initiator_prefix(&fixture);
    let permit = fixture.context.admission.enter(&fixture.root).unwrap();
    let prepared = approval_domain::prepare_bootstrap(
        &fixture.context,
        &registry,
        BootstrapApprovalIpcRequest {
            transcript_messages: Some(prefix),
            ..approval_request("bootstrap.approval.prepare")
        },
        &permit,
    )
    .unwrap();
    let cap = prepared.reply.capability.clone().unwrap();
    let digest = prepared.reply.approval_request_digest.clone().unwrap();
    let (name, listener) = isolated_ipc_listener("domain-prepare-send");
    let client = Stream::connect(name.borrow()).unwrap();
    let mut server = listener.accept().unwrap();
    arm_ipc_stream(&server).unwrap();
    drop(client);
    assert!(deliver_approval_session(&mut server, prepared, permit).is_err());
    drop(server);
    drop(listener);
    let connection = registry.open_connection().unwrap();
    assert!(matches!(
        registry.consume(&connection, &cap, &digest),
        Err(ApprovalSessionError::UnknownSession)
    ));
    let journal = BootstrapJournalStore::new(&fixture.root)
        .unwrap()
        .load()
        .unwrap();
    assert_eq!(journal.transactions[0].state, BootstrapState::OwnerPending);
    assert!(!journal.transactions[0].local_approval_signature_generated);
    fs::remove_dir_all(fixture.root).unwrap();
}

#[test]
fn domain_final_delivery_failure_preserves_durable_signature_without_cancellation() {
    let fixture = prepared_search_fixture("domain-final-send");
    let registry = ApprovalSessionRegistry::default();
    let before = events(&fixture);
    let (session, prepared, prefix) = prepared_bootstrap(&fixture, &registry);
    let permit = session.admit_followup().unwrap();
    let signed = session
        .finish(
            bootstrap_followup(&prepared, prefix, "bootstrap.approval.confirm"),
            &permit,
        )
        .unwrap();
    assert!(signed.reply.accepted);
    let error = AdmittedReply::new(signed.reply, permit, Instant::now() + signed.timeout)
        .unwrap()
        .send_using(|_, _| {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "fixture closed peer",
            ))
        })
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    assert_burned(&registry, &prepared);
    let journal = BootstrapJournalStore::new(&fixture.root)
        .unwrap()
        .load()
        .unwrap();
    assert!(
        journal.transactions[0].local_approval_signature_generated
            && journal.transactions[0].local_approval_signature_released
    );
    assert!(!matches!(
        journal.transactions[0].state,
        BootstrapState::Cancelled | BootstrapState::CancelledAfterRelease
    ));
    assert_eq!(events(&fixture), before);
    fs::remove_dir_all(fixture.root).unwrap();
}

#[test]
fn domain_socket_wrapper_releases_think_time_and_rechecks_admission() {
    let fixture = prepared_search_fixture("domain-think-time");
    let registry = Arc::new(ApprovalSessionRegistry::default());
    let server_registry = Arc::clone(&registry);
    let context = fixture.context.clone();
    let prefix = synthetic_initiator_prefix(&fixture);
    let (name, listener) = isolated_ipc_listener("domain-think-time");
    let server = thread::spawn(move || {
        let mut stream = listener.accept().unwrap();
        arm_ipc_stream(&stream).unwrap();
        let frame = read_ipc_frame(
            &mut stream,
            MAX_BOOTSTRAP_APPROVAL_FRAME_BYTES,
            Instant::now() + IPC_RESPONSE_DEADLINE,
        )
        .unwrap();
        let request = serde_json::from_slice(&frame).unwrap();
        let permit = context.admission.enter(&context.runtime_root).unwrap();
        let prepared =
            approval_domain::prepare_bootstrap(&context, &server_registry, request, &permit)
                .unwrap();
        deliver_approval_session(&mut stream, prepared, permit).unwrap();
    });
    let mut client = Stream::connect(name.borrow()).unwrap();
    let prepared = exchange_bootstrap_approval(
        &mut client,
        &BootstrapApprovalIpcRequest {
            transcript_messages: Some(prefix.clone()),
            ..approval_request("bootstrap.approval.prepare")
        },
        Instant::now() + IPC_RESPONSE_DEADLINE,
    )
    .unwrap();
    assert!(prepared.accepted);
    let exclusive = fixture
        .context
        .admission
        .exclusive(
            &fixture.root,
            Instant::now() + Duration::from_secs(2),
            false,
        )
        .unwrap();
    let response = exchange_bootstrap_approval(
        &mut client,
        &BootstrapApprovalIpcRequest {
            operation: "bootstrap.approval.confirm".into(),
            transcript_messages: Some(prefix),
            capability: prepared.capability.clone(),
            approval_request_digest: prepared.approval_request_digest.clone(),
            ..approval_request("bootstrap.approval.confirm")
        },
        Instant::now() + IPC_RESPONSE_DEADLINE,
    )
    .unwrap();
    assert_eq!(response.error.as_deref(), Some("runtime_busy"));
    exclusive.finish(None);
    server.join().unwrap();
    let fresh = registry.open_connection().unwrap();
    assert!(matches!(
        registry.consume(
            &fresh,
            prepared.capability.as_deref().unwrap(),
            prepared.approval_request_digest.as_deref().unwrap()
        ),
        Err(ApprovalSessionError::UnknownSession)
    ));
    assert!(
        !BootstrapJournalStore::new(&fixture.root)
            .unwrap()
            .load()
            .unwrap()
            .transactions[0]
            .local_approval_signature_generated
    );
    fs::remove_dir_all(fixture.root).unwrap();
}

#[test]
fn domain_durable_completion_requires_journaled_owner_approval_and_replay_is_noop() {
    let fixture = prepared_search_fixture("domain-completion");
    let before = events(&fixture);
    let prefix = synthetic_initiator_prefix(&fixture);
    let key = load_existing_event_signing_key(
        fixture.store.as_ref(),
        SecureStoreEnvironment::Dev,
        &fixture.root,
        &fixture.device,
    )
    .unwrap();
    let verified = cryptex_events::verify_bootstrap_approval_request(
        &prefix.iter().map(String::as_bytes).collect::<Vec<_>>(),
        cryptex_events::BootstrapApprovalRole::Initiator,
    )
    .unwrap();
    let initiator = String::from_utf8(
        cryptex_events::sign_bootstrap_approval(&verified, &key.signing_key).unwrap(),
    )
    .unwrap();
    let mut transcript = prefix.clone();
    transcript.push(initiator);
    let responder = cryptex_events::verify_bootstrap_approval_request(
        &transcript.iter().map(String::as_bytes).collect::<Vec<_>>(),
        cryptex_events::BootstrapApprovalRole::Responder,
    )
    .unwrap();
    transcript.push(
        String::from_utf8(
            cryptex_events::sign_bootstrap_approval(
                &responder,
                &SigningKey::from_bytes(&[0x42; 32]),
            )
            .unwrap(),
        )
        .unwrap(),
    );
    let request = || BootstrapApprovalIpcRequest {
        role: None,
        transcript_messages: Some(transcript.clone()),
        ..approval_request("bootstrap.approval.complete")
    };
    let permit = fixture.context.admission.enter(&fixture.root).unwrap();
    let refused = approval_domain::complete(&fixture.context, request(), &permit);
    assert_eq!(refused.error, Some("approval_transaction_conflict"));
    drop(permit);
    assert_eq!(events(&fixture), before);
    let registry = ApprovalSessionRegistry::default();
    let (session, prepared, prefix) = prepared_bootstrap(&fixture, &registry);
    let permit = session.admit_followup().unwrap();
    let signed = session
        .finish(
            bootstrap_followup(&prepared, prefix, "bootstrap.approval.confirm"),
            &permit,
        )
        .unwrap();
    assert_eq!(
        signed.reply.signed_message.as_deref(),
        Some(transcript[2].as_str())
    );
    drop(permit);
    let permit = fixture.context.admission.enter(&fixture.root).unwrap();
    assert!(approval_domain::complete(&fixture.context, request(), &permit).accepted);
    let after = events(&fixture);
    assert_eq!(after.len(), before.len() + 1);
    assert_eq!(&after[..before.len()], before.as_slice());
    assert!(approval_domain::complete(&fixture.context, request(), &permit).accepted);
    drop(permit);
    assert_eq!(events(&fixture), after);
    let journal = BootstrapJournalStore::new(&fixture.root)
        .unwrap()
        .load()
        .unwrap();
    assert_eq!(journal.transactions[0].state, BootstrapState::Anchored);
    assert_eq!(journal.transactions[0].append_attempts.len(), 1);
    fs::remove_dir_all(fixture.root).unwrap();
}
