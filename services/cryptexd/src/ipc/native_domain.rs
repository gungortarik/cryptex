//! Shared native owner domain. Native allocation seals are consumed here;
//! decoded JSON, claimed PIDs and fixture profile flags are not authority.
use super::*;
use crate::approval_session::ApprovalSessionRegistry;
use crate::m8_owner_channel::Method;
use crate::m8_owner_channel::local_startup::{BoundConnection, ReceivedRequest};
use crate::runtime_admission::OperationPermit;

pub(super) struct Domain<'a> {
    context: SearchContext,
    registry: &'a ApprovalSessionRegistry,
    pending: Option<approval_domain::PendingSession<'a>>,
}

pub(super) struct Response {
    pub(super) reply: BootstrapApprovalIpcResponse,
    permit: OperationPermit,
    timeout: Duration,
}
impl Response {
    pub(super) fn send(self, connection: &mut BoundConnection<'_>) -> io::Result<()> {
        AdmittedReply::new(self.reply, self.permit, Instant::now() + self.timeout)?
            .send_using(|bytes, deadline| connection.send(bytes, deadline))
    }
}
impl<'a> Domain<'a> {
    pub(super) fn from_context(context: SearchContext, registry: &'a ApprovalSessionRegistry) -> Self {
        Self { context, registry, pending: None }
    }
    pub(super) fn dispatch(
        &mut self, connection: &mut BoundConnection<'_>, received: ReceivedRequest,
    ) -> io::Result<Response> {
        // Consumption here is one-use and tied to this native allocation;
        // ordinary parsed JSON/roles/PIDs cannot construct the sealed value.
        let received = connection.consume(received)?;
        let request = received.request;
        if let Some(session) = self.pending.take() {
            let session: approval_domain::PendingSession<'_> = session;
            // Reacquire the frozen admission, not a lease from another context.
            let permit = session.admit_followup().map_err(io::Error::other)?;
            let finished = session.finish(request, &permit)?;
            return Ok(Response { reply: finished.reply, permit, timeout: finished.timeout });
        }
        let permit = self.context.admission.enter(&self.context.runtime_root).map_err(io::Error::other)?;
        let (reply, timeout) = {
            match received.method {
                Method::PrepareOwnerOperation | Method::PrepareApproval => {
                    let prepared = if received.method == Method::PrepareOwnerOperation {
                        approval_domain::prepare_owner(&self.context, self.registry, request, &permit)?
                    } else {
                        approval_domain::prepare_bootstrap(&self.context, self.registry, request, &permit)?
                    };
                    self.pending = prepared.session;
                    (prepared.reply, IPC_RESPONSE_DEADLINE)
                }
                Method::CompleteApproval => (
                    approval_domain::complete(&self.context, request, &permit),
                    IPC_RESPONSE_DEADLINE,
                ),
                Method::ReadTransportIdentity => {
                    let result = current_transport_device_id(&self.context);
                    (
                        bootstrap_exchange_response(
                            Some(request.request_id),
                            result.is_ok(),
                            result.as_ref().err().copied(),
                            "bootstrap.exchange.transport_identity.read",
                            None,
                            result.ok(),
                            None,
                        ),
                        IPC_RESPONSE_DEADLINE,
                    )
                }
                Method::ReadBrainIdentity | Method::CreateOffer | Method::CreateProof => (
                    execute_bootstrap_exchange(&self.context, request),
                    IPC_RESPONSE_DEADLINE,
                ),
                _ => (
                    bootstrap_approval_response(
                        Some(request.request_id),
                        false,
                        Some("approval_session_missing"),
                        match received.method {
                            Method::ConfirmApproval | Method::CancelApproval => {
                                "bootstrap.approval.result"
                            }
                            _ => "owner.security.result",
                        },
                        None,
                        None,
                        None,
                    ),
                    IPC_REQUEST_DEADLINE,
                ),
            }
        };
        Ok(Response { reply, permit, timeout })
    }
}
