//! Transport-neutral M8 approval work. These private functions do not authenticate
//! a peer. The socket wrapper retains its platform checks; Mac product routes
//! remain closed until an independent native exact-request authority exists.

use crate::approval_session::{ApprovalConnection, ApprovalSessionRegistry};
use crate::bootstrap_journal::{
    ApprovalSnapshot, BootstrapJournalStore, BootstrapOperation, BootstrapState,
};
use crate::m8_owner_channel::reply::OwnerReply;
use crate::m8_owner_channel::request::{OwnerOperation, OwnerRequest};
use crate::runtime_admission::OperationPermit;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;

/// Copy only immutable domain bindings and the admission/process owners. Never
/// clone SearchContext::operation into an owner think-time session.
struct SessionContext {
    runtime_root: PathBuf,
    admission: crate::runtime_admission::RuntimeAdmission,
    store: Arc<dyn crate::secure_store::SecureStore>,
    environment: crate::secure_store::SecureStoreEnvironment,
    expected_desktop: Arc<super::DesktopProcessIdentity>,
}

impl From<&SearchContext> for SessionContext {
    fn from(context: &SearchContext) -> Self {
        Self {
            runtime_root: context.runtime_root.clone(),
            admission: context.admission.clone(),
            store: Arc::clone(&context.store),
            environment: context.environment,
            expected_desktop: Arc::clone(&context.expected_desktop),
        }
    }
}

use super::{
    SearchContext, approval_request_has_followup_shape, approval_request_has_prepare_shape,
    approval_request_is_authorized, bootstrap_approval_response, invalid_data,
    new_bootstrap_event_id, owner_security_followup_has_shape, owner_security_prepare_has_shape,
    validate_local_bootstrap_identity, verify_approval_prefix,
};

pub(super) struct Preparation<'a> {
    pub(super) reply: OwnerReply,
    pub(super) session: Option<PendingSession<'a>>,
}

pub(super) struct FinishedReply {
    pub(super) reply: OwnerReply,
    pub(super) timeout: std::time::Duration,
}

impl FinishedReply {
    fn request(reply: OwnerReply) -> Self {
        Self {
            reply,
            timeout: super::IPC_REQUEST_DEADLINE,
        }
    }
    fn final_result(reply: OwnerReply) -> Self {
        Self {
            reply,
            timeout: super::IPC_RESPONSE_DEADLINE,
        }
    }
}

enum PendingKind {
    Bootstrap {
        verified: Box<cryptex_events::VerifiedBootstrapApprovalRequest>,
        role: Option<String>,
        grant_digest: String,
    },
    Owner {
        brain_id: String,
        snapshot: Box<ApprovalSnapshot>,
        transaction_id: String,
    },
}

/// One owned approval attempt, frozen to its original runtime and registry
/// allocation (the process-local epoch). No Clone, deserialization or public
/// authority factory. Dropping the owned connection invalidates ephemeral grants
/// only; it must never roll back or durably cancel an uncertain operation.
pub(super) struct PendingSession<'a> {
    context: SessionContext,
    registry: &'a ApprovalSessionRegistry,
    connection: ApprovalConnection,
    request_id: String,
    journal: BootstrapJournalStore,
    kind: PendingKind,
}

impl std::fmt::Debug for PendingSession<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PendingSession([redacted])")
    }
}

fn rejection(request_id: &str, owner: bool, error: &'static str) -> OwnerReply {
    bootstrap_approval_response(
        Some(request_id.to_owned()),
        false,
        Some(error),
        if owner {
            "owner.security.result"
        } else {
            "bootstrap.approval.result"
        },
        None,
        None,
        None,
    )
}

fn refused(request_id: &str, owner: bool, error: &'static str) -> Preparation<'static> {
    Preparation {
        reply: rejection(request_id, owner, error),
        session: None,
    }
}

pub(super) fn prepare_bootstrap<'a>(
    context: &SearchContext,
    registry: &'a ApprovalSessionRegistry,
    request: OwnerRequest,
    permit: &OperationPermit,
) -> io::Result<Preparation<'a>> {
    if let Err(error) = context
        .admission
        .validate_operation(&context.runtime_root, permit)
    {
        return Ok(refused(&request.request_id, false, error.code()));
    }
    if !approval_request_is_authorized(&request)
        || !approval_request_has_prepare_shape(&request)
        || request.operation != "bootstrap.approval.prepare"
    {
        return Ok(refused(
            &request.request_id,
            false,
            "approval_request_rejected",
        ));
    }
    let (verified, messages) = match verify_approval_prefix(&request) {
        Ok(prefix) => prefix,
        Err(error) => return Ok(refused(&request.request_id, false, error)),
    };
    if let Err(error) = validate_local_bootstrap_identity(context, &verified) {
        return Ok(refused(&request.request_id, false, error));
    }
    let snapshot = ApprovalSnapshot::from_verified_bootstrap_request(&verified)
        .map_err(|_| invalid_data("approval snapshot invalid"))?;
    let journal = BootstrapJournalStore::new(&context.runtime_root)
        .map_err(|_| invalid_data("approval journal unavailable"))?;
    let event_id = new_bootstrap_event_id().map_err(invalid_data)?;
    if journal
        .begin_verified_approval_prefix(&verified, &messages, &event_id)
        .is_err()
    {
        return Ok(refused(
            &request.request_id,
            false,
            "approval_transaction_conflict",
        ));
    }
    let connection = registry
        .open_connection()
        .map_err(|_| invalid_data("approval session unavailable"))?;
    let grant = registry
        .issue(&connection, snapshot.clone())
        .map_err(|_| invalid_data("approval session unavailable"))?;
    let reply = bootstrap_approval_response(
        Some(request.request_id.clone()),
        true,
        None,
        "bootstrap.approval.prepared",
        Some(grant.clone()),
        Some(snapshot),
        None,
    );
    Ok(Preparation {
        reply,
        session: Some(PendingSession {
            context: context.into(),
            registry,
            connection,
            request_id: request.request_id,
            journal,
            kind: PendingKind::Bootstrap {
                verified: Box::new(verified),
                role: request.role,
                grant_digest: grant.approval_request_digest,
            },
        }),
    })
}

pub(super) fn prepare_owner<'a>(
    context: &SearchContext,
    registry: &'a ApprovalSessionRegistry,
    request: OwnerRequest,
    permit: &OperationPermit,
) -> io::Result<Preparation<'a>> {
    if let Err(error) = context
        .admission
        .validate_operation(&context.runtime_root, permit)
    {
        return Ok(refused(&request.request_id, true, error.code()));
    }
    if !approval_request_is_authorized(&request)
        || !owner_security_prepare_has_shape(&request)
        || request.operation != "owner.security.prepare"
    {
        return Ok(refused(
            &request.request_id,
            true,
            "owner_operation_request_rejected",
        ));
    }
    let transaction_id = request.transaction_id.as_deref().unwrap_or_default();
    let (operation, target, reason, cutoff) = match request.owner_operation.as_ref() {
        Some(OwnerOperation::DeviceRestriction {
            target_device_id,
            reason,
            cutoff_seq,
        }) => (
            BootstrapOperation::DeviceRestriction,
            Some(target_device_id.as_str()),
            Some(*reason),
            Some(*cutoff_seq),
        ),
        Some(OwnerOperation::SelfRetirement) => {
            (BootstrapOperation::SelfRetirement, None, None, None)
        }
        None => {
            return Ok(refused(
                &request.request_id,
                true,
                "owner_operation_request_rejected",
            ));
        }
    };
    let brain = match crate::identity_runtime::load_brain_identity(&context.runtime_root) {
        Ok(brain) => brain,
        Err(_) => {
            return Ok(refused(
                &request.request_id,
                true,
                "local_identity_unavailable",
            ));
        }
    };
    let snapshot = match crate::knowledge_materialization::build_owner_security_operation_snapshot(
        &context.runtime_root,
        &brain.brain_id,
        context.store.as_ref(),
        context.environment,
        crate::knowledge_materialization::OwnerSecuritySnapshotRequest {
            transaction_id: transaction_id.to_owned(),
            operation,
            target_device_id: target,
            reason,
            cutoff_seq: cutoff,
        },
    ) {
        Ok(snapshot) => snapshot,
        Err(_) => {
            return Ok(refused(
                &request.request_id,
                true,
                "owner_operation_policy_held",
            ));
        }
    };
    let journal = match BootstrapJournalStore::new(&context.runtime_root) {
        Ok(journal) => journal,
        Err(_) => {
            return Ok(refused(
                &request.request_id,
                true,
                "approval_journal_unavailable",
            ));
        }
    };
    let existing = match journal.load() {
        Ok(journal) => journal
            .transactions
            .into_iter()
            .find(|transaction| transaction.snapshot.transaction_id == transaction_id),
        Err(_) => {
            return Ok(refused(
                &request.request_id,
                true,
                "approval_journal_unavailable",
            ));
        }
    };
    if let Some(existing) = existing {
        if existing.snapshot != snapshot || existing.state != BootstrapState::OwnerPending {
            return Ok(refused(
                &request.request_id,
                true,
                "approval_transaction_conflict",
            ));
        }
    } else {
        let event_id = match new_bootstrap_event_id() {
            Ok(event_id) => event_id,
            Err(error) => return Ok(refused(&request.request_id, true, error)),
        };
        if journal
            .begin_owner_operation(snapshot.clone(), &event_id)
            .is_err()
        {
            return Ok(refused(
                &request.request_id,
                true,
                "approval_transaction_conflict",
            ));
        }
    }
    let connection = match registry.open_connection() {
        Ok(connection) => connection,
        Err(_) => {
            return Ok(refused(
                &request.request_id,
                true,
                "approval_session_unavailable",
            ));
        }
    };
    let grant = match registry.issue(&connection, snapshot.clone()) {
        Ok(grant) => grant,
        Err(_) => {
            return Ok(refused(
                &request.request_id,
                true,
                "approval_session_unavailable",
            ));
        }
    };
    let reply = bootstrap_approval_response(
        Some(request.request_id.clone()),
        true,
        None,
        "owner.security.prepared",
        Some(grant),
        Some(snapshot.clone()),
        None,
    );
    Ok(Preparation {
        reply,
        session: Some(PendingSession {
            context: context.into(),
            registry,
            connection,
            request_id: request.request_id,
            journal,
            kind: PendingKind::Owner {
                brain_id: brain.brain_id,
                snapshot: Box::new(snapshot),
                transaction_id: transaction_id.to_owned(),
            },
        }),
    })
}

impl PendingSession<'_> {
    pub(super) fn is_owner(&self) -> bool {
        matches!(self.kind, PendingKind::Owner { .. })
    }
    pub(super) fn admit_followup(
        &self,
    ) -> Result<crate::runtime_admission::OperationPermit, crate::runtime_admission::AdmissionError>
    {
        self.context.admission.enter(&self.context.runtime_root)
    }
    pub(super) fn matches_expected_peer_pid(&self, pid: u32) -> bool {
        self.context.expected_desktop.matches_peer(pid)
    }

    /// Envelope consistency only. This is never a peer authentication proof.
    pub(super) fn matches_followup(&self, request: &OwnerRequest) -> bool {
        if !approval_request_is_authorized(request) || request.request_id != self.request_id {
            return false;
        }
        match &self.kind {
            PendingKind::Bootstrap { role, .. } => {
                approval_request_has_followup_shape(request) && request.role == *role
            }
            PendingKind::Owner { transaction_id, .. } => {
                owner_security_followup_has_shape(request)
                    && request.transaction_id.as_deref() == Some(transaction_id.as_str())
                    && matches!(
                        request.operation.as_str(),
                        "owner.security.confirm" | "owner.security.cancel"
                    )
            }
        }
    }

    pub(super) fn reject(self, error: &'static str) -> OwnerReply {
        rejection(&self.request_id, self.is_owner(), error)
    }

    /// Consume this attempt even on mismatch/sign/journal failure. The wrapper
    /// must independently authenticate this exact followup and hold admission.
    pub(super) fn finish(
        self,
        request: OwnerRequest,
        permit: &OperationPermit,
    ) -> io::Result<FinishedReply> {
        if let Err(error) = self
            .context
            .admission
            .validate_operation(&self.context.runtime_root, permit)
        {
            return Ok(FinishedReply::request(self.reject(error.code())));
        }
        if !self.matches_followup(&request) {
            return Ok(FinishedReply::request(
                self.reject("approval_session_mismatch"),
            ));
        }
        let capability = request.capability.as_deref().unwrap_or_default();
        let digest = request
            .approval_request_digest
            .as_deref()
            .unwrap_or_default();
        match &self.kind {
            PendingKind::Bootstrap {
                verified,
                grant_digest,
                ..
            } => {
                let (confirm_request, _) = match verify_approval_prefix(&request) {
                    Ok(prefix) => prefix,
                    Err(error) => return Ok(FinishedReply::request(self.reject(error))),
                };
                if confirm_request != **verified || digest != grant_digest {
                    return Ok(FinishedReply::request(
                        self.reject("approval_snapshot_changed"),
                    ));
                }
                if request.operation == "bootstrap.approval.cancel" {
                    let result = self
                        .registry
                        .cancel(&self.connection, capability)
                        .map_err(|_| invalid_data("approval cancellation rejected"))
                        .and_then(|()| {
                            self.journal
                                .cancel_pending_approval(verified.transaction_id())
                                .map_err(|_| {
                                    invalid_data("approval cancellation could not be persisted")
                                })
                        });
                    return Ok(FinishedReply::final_result(bootstrap_approval_response(
                        Some(self.request_id.clone()),
                        result.is_ok(),
                        result.err().map(|_| "approval_cancellation_rejected"),
                        "bootstrap.approval.cancelled",
                        None,
                        None,
                        None,
                    )));
                }
                if request.operation != "bootstrap.approval.confirm" {
                    return Ok(FinishedReply::request(
                        self.reject("invalid_approval_action"),
                    ));
                }
                let signed = self.registry.confirm_bootstrap_approval_journaled(
                    &self.connection,
                    capability,
                    digest,
                    &confirm_request,
                    &self.journal,
                    |verified_request| sign_bootstrap(&self.context, verified_request),
                );
                Ok(FinishedReply::final_result(match signed {
                    Ok(bytes) => bootstrap_approval_response(
                        Some(self.request_id.clone()), true, None,
                        "bootstrap.approval.signed", None, None,
                        Some(String::from_utf8(bytes).map_err(|_| invalid_data("signed approval encoding invalid"))?),
                    ),
                    Err(error) => rejection(&self.request_id, false, match error {
                        crate::approval_session::ApprovalSessionError::ApprovalJournalFailed => "approval_journal_failed",
                        crate::approval_session::ApprovalSessionError::SnapshotMismatch => "approval_snapshot_changed",
                        _ => "approval_confirmation_failed",
                    }),
                }))
            }
            PendingKind::Owner {
                brain_id,
                snapshot,
                transaction_id,
            } => {
                match self.registry.consume(&self.connection, capability, digest) {
                    Ok(consumed) if consumed == **snapshot => {}
                    Ok(_) => {
                        return Ok(FinishedReply::request(
                            self.reject("approval_snapshot_changed"),
                        ));
                    }
                    Err(_) => {
                        return Ok(FinishedReply::request(
                            self.reject("approval_confirmation_failed"),
                        ));
                    }
                }
                if request.operation == "owner.security.cancel" {
                    let result = self.journal.cancel_pending_approval(transaction_id);
                    return Ok(FinishedReply::final_result(bootstrap_approval_response(
                        Some(self.request_id.clone()),
                        result.is_ok(),
                        result.err().map(|_| "approval_cancellation_rejected"),
                        "owner.security.cancelled",
                        None,
                        None,
                        None,
                    )));
                }
                if self
                    .journal
                    .confirm_owner_operation(transaction_id, digest)
                    .is_err()
                {
                    return Ok(FinishedReply::request(
                        self.reject("approval_journal_failed"),
                    ));
                }
                let result =
                    crate::knowledge_materialization::append_confirmed_owner_security_operation(
                        &self.context.runtime_root,
                        brain_id,
                        self.context.store.as_ref(),
                        self.context.environment,
                        transaction_id,
                    );
                Ok(FinishedReply::final_result(bootstrap_approval_response(
                    Some(self.request_id.clone()),
                    result.is_ok(),
                    result.err().map(|_| "owner_operation_append_held"),
                    "owner.security.result",
                    None,
                    None,
                    None,
                )))
            }
        }
    }
}

fn sign_bootstrap(
    context: &SessionContext,
    request: &cryptex_events::VerifiedBootstrapApprovalRequest,
) -> Result<Vec<u8>, crate::approval_session::ApprovalSessionError> {
    use crate::approval_session::ApprovalSessionError::ApprovalSigningFailed;
    let brain = crate::identity_runtime::load_brain_identity(&context.runtime_root)
        .map_err(|_| ApprovalSigningFailed)?;
    let local = crate::local_device_identity::load_local_device_identity(
        &context.runtime_root,
        &brain.brain_id,
    )
    .map_err(|_| ApprovalSigningFailed)?;
    let device = cryptex_core::DeviceId::new(local.device_id).map_err(|_| ApprovalSigningFailed)?;
    let ready = crate::event_signing_key::load_existing_event_signing_key(
        context.store.as_ref(),
        context.environment,
        &context.runtime_root,
        &device,
    )
    .map_err(|_| ApprovalSigningFailed)?;
    if brain.brain_id != request.brain_id()
        || device.as_str() != request.local_identity().device_id.as_str()
        || ready.public.signing_key_id != request.local_identity().signing_key_id.as_str()
        || ready.signing_key.verifying_key().to_bytes() != request.local_identity().public_key
    {
        return Err(ApprovalSigningFailed);
    }
    cryptex_events::sign_bootstrap_approval(request, &ready.signing_key)
        .map_err(|_| ApprovalSigningFailed)
}

/// Stateless durable completion, not peer authentication. The socket wrapper
/// keeps the Mac refusal and all admission/peer checks before entering here.
pub(super) fn complete(
    context: &SearchContext,
    request: OwnerRequest,
    permit: &OperationPermit,
) -> OwnerReply {
    let reject = |error| {
        bootstrap_approval_response(
            Some(request.request_id.clone()),
            false,
            Some(error),
            "bootstrap.approval.anchored",
            None,
            None,
            None,
        )
    };
    if let Err(error) = context
        .admission
        .validate_operation(&context.runtime_root, permit)
    {
        return reject(error.code());
    }
    if !approval_request_is_authorized(&request)
        || request.operation != "bootstrap.approval.complete"
        || request.role.is_some()
        || request.approval_request_digest.is_some()
        || request.capability.is_some()
    {
        return reject("approval_request_rejected");
    }
    let Some(transcript) = request.transcript_messages.as_ref() else {
        return reject("approval_request_rejected");
    };
    let messages = transcript
        .iter()
        .map(|message| message.as_bytes().to_vec())
        .collect::<Vec<_>>();
    let verified =
        if messages.len() == 4 && messages.iter().all(|message| message.len() <= 64 * 1024) {
            let refs = messages.iter().map(Vec::as_slice).collect::<Vec<_>>();
            cryptex_events::verify_bootstrap_transcript(&refs).ok()
        } else {
            None
        };
    let Some(verified) = verified else {
        return reject("invalid_transcript");
    };
    let journal = match BootstrapJournalStore::new(&context.runtime_root) {
        Ok(journal) => journal,
        Err(_) => return reject("approval_journal_unavailable"),
    };
    if journal
        .record_verified_complete_transcript(verified.transaction_id(), &messages)
        .is_err()
    {
        reject("approval_transaction_conflict")
    } else if crate::knowledge_materialization::append_local_peer_approval_anchor(
        &context.runtime_root,
        verified.brain_id(),
        context.store.as_ref(),
        context.environment,
        verified.transaction_id(),
        &messages,
    )
    .is_err()
    {
        reject("approval_anchor_held")
    } else {
        bootstrap_approval_response(
            Some(request.request_id.clone()),
            true,
            None,
            "bootstrap.approval.anchored",
            None,
            None,
            None,
        )
    }
}
