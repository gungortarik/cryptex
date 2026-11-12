//! Feature-only Desktop-side review handles. Wire capabilities never leave
//! this owner, are single-use and are lost with the exact native connection.
use super::*;
use crate::ipc::{
    DesktopBootstrapApprovalPrepared, DesktopBootstrapApprovalResult,
    DesktopBootstrapExchangeResult, DesktopOwnerSecurityOperationInput,
    DesktopOwnerSecurityOperationPrepared, DesktopOwnerSecurityOperationResult,
};
use crate::m8_owner_channel::request::OwnerRequest;

pub(super) struct PendingReview {
    handle: String,
    request: OwnerRequest,
}
impl PendingReview {
    fn followup(mut self, handle: &str, peer: bool, confirm: bool) -> Result<OwnerRequest, Error> {
        let family = if peer {
            "bootstrap.approval."
        } else {
            "owner.security."
        };
        if self.handle != handle || !self.request.operation.starts_with(family) {
            return Err(Error::Arguments);
        }
        self.request.operation = format!("{family}{}", if confirm { "confirm" } else { "cancel" });
        Ok(self.request)
    }
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireReply {
    protocol_version: u32,
    request_id: Option<String>,
    accepted: bool,
    error: Option<String>,
    operation: String,
    approval_request_digest: Option<String>,
    capability: Option<String>,
    snapshot: Option<crate::bootstrap_journal::ApprovalSnapshot>,
    signed_message: Option<String>,
    exchange_message: Option<String>,
    transaction_id: Option<String>,
    brain_identity_json: Option<String>,
}
fn random_id() -> Result<String, Error> {
    let mut bytes = [0; 32];
    getrandom::getrandom(&mut bytes).map_err(|_| Error::Channel)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
fn request(operation: &str) -> Result<OwnerRequest, Error> {
    serde_json::from_value(serde_json::json!({
        "protocolVersion": crate::compat::DAEMON_PROTOCOL_VERSION,
        "requestId": random_id()?, "clientRole":"desktop",
        "capabilities":["peer.approval.manage"], "operation": operation,
    }))
    .map_err(|_| Error::Arguments)
}
fn valid_reply(reply: &WireReply, request: &OwnerRequest, operation: &str) -> bool {
    let refusal_operation = match request.operation.as_str() {
        "bootstrap.exchange.offer.create" | "bootstrap.exchange.proof.create" => {
            "bootstrap.exchange.result"
        }
        "bootstrap.approval.prepare"
        | "bootstrap.approval.confirm"
        | "bootstrap.approval.cancel" => "bootstrap.approval.result",
        "owner.security.prepare" | "owner.security.cancel" => "owner.security.result",
        _ => operation,
    };
    let refusal = !reply.accepted
        && reply.error.is_some()
        && reply.snapshot.is_none()
        && reply.capability.is_none()
        && reply.approval_request_digest.is_none()
        && reply.signed_message.is_none()
        && reply.exchange_message.is_none()
        && reply.brain_identity_json.is_none();
    reply.protocol_version == crate::compat::DAEMON_PROTOCOL_VERSION
        && reply.request_id.as_deref() == Some(request.request_id.as_str())
        && if reply.accepted {
            reply.error.is_none()
                && reply.operation == operation
                && (operation != "bootstrap.approval.signed"
                    || reply.signed_message.as_ref().is_some_and(|message| {
                        !message.is_empty()
                            && message.len() <= super::super::request::MAX_MESSAGE_BYTES
                    }))
                && (matches!(
                    request.operation.as_str(),
                    "owner.security.prepare" | "bootstrap.approval.prepare"
                ) || (reply.snapshot.is_none()
                    && reply.capability.is_none()
                    && reply.approval_request_digest.is_none()))
        } else {
            refusal && (reply.operation == operation || reply.operation == refusal_operation)
        }
}
fn peer_snapshot(
    request: &OwnerRequest,
) -> Result<crate::bootstrap_journal::ApprovalSnapshot, Error> {
    let role = match request.role.as_deref() {
        Some("initiator") => cryptex_events::BootstrapApprovalRole::Initiator,
        Some("responder") => cryptex_events::BootstrapApprovalRole::Responder,
        _ => return Err(Error::Arguments),
    };
    let messages = request
        .transcript_messages
        .as_ref()
        .ok_or(Error::Arguments)?;
    let refs = messages.iter().map(|m| m.as_bytes()).collect::<Vec<_>>();
    let verified = cryptex_events::verify_bootstrap_approval_request(&refs, role)
        .map_err(|_| Error::Arguments)?;
    crate::bootstrap_journal::ApprovalSnapshot::from_verified_bootstrap_request(&verified)
        .map_err(|_| Error::Arguments)
}

impl LocalDaemon {
    pub fn capture_confirmation_probe(&self) -> Result<ConfirmationReplayProbe, Error> {
        let pending = self.pending_review.as_ref().ok_or(Error::Arguments)?;
        if pending.request.operation != "owner.security.prepare" {
            return Err(Error::Arguments);
        }
        let mut request: OwnerRequest = serde_json::from_slice(
            &serde_json::to_vec(&pending.request).map_err(|_| Error::Arguments)?,
        )
        .map_err(|_| Error::Arguments)?;
        request.operation = "owner.security.confirm".into();
        Ok(ConfirmationReplayProbe {
            bytes: serde_json::to_vec(&request).map_err(|_| Error::Arguments)?,
            _thread: std::marker::PhantomData,
        })
    }
    pub fn require_wire_replay_refusal(
        &mut self,
        probe: &ConfirmationReplayProbe,
    ) -> Result<(), Error> {
        let request: OwnerRequest =
            serde_json::from_slice(&probe.bytes).map_err(|_| Error::Arguments)?;
        // rpc serializes this same struct. Assert the exact recorded bytes,
        // including request ID, digest, transaction and capability, are reused.
        if serde_json::to_vec(&request).map_err(|_| Error::Arguments)? != probe.bytes {
            return Err(Error::Arguments);
        }
        let reply = self.rpc(&request, "owner.security.result")?;
        if reply.accepted || reply.error.as_deref() != Some("approval_session_missing") {
            return Err(Error::StaleAccepted);
        }
        Ok(())
    }
    pub fn invalidate_session(&mut self) {
        self.burn();
    }
    pub fn content(&self) -> Result<crate::ProtectedContent, Error> {
        if !self.content_mode {
            return Err(Error::Arguments);
        }
        self.release.frontend()
    }
    /// Called by signed Desktop backend only after installing the provider.
    /// The renderer has no command for supplying this binding.
    pub fn bind_content(&mut self, binding: &[u8; 32]) -> Result<(), Error> {
        step("content_bind", || self.bind_content_inner(binding))
    }
    fn bind_content_inner(&mut self, binding: &[u8; 32]) -> Result<(), Error> {
        if self.bound || self.content()?.binding() != binding {
            return Err(Error::Arguments);
        }
        let deadline = Instant::now() + Duration::from_secs(15);
        let channel = self.channel.as_mut().ok_or(Error::Channel)?;
        channel
            .send(&frame(4, &self.epoch, binding), deadline)
            .map_err(Error::from)?;
        if channel
            .receive(&mut [0; 54], deadline)
            .map_err(Error::from)?
            != frame(5, &self.epoch, binding)
        {
            return Err(Error::Channel);
        }
        self.bound = true;
        Ok(())
    }
    fn exchange(&mut self, bytes: &[u8]) -> Result<Vec<u8>, Error> {
        if !self.bound || bytes.is_empty() || bytes.len() > super::super::request::MAX_FRAME_BYTES {
            return Err(Error::Arguments);
        }
        let deadline = Instant::now() + Duration::from_secs(15);
        let result = step("owner_exchange", || {
            let channel = self.channel.as_mut().ok_or(Error::Channel)?;
            channel.send(bytes, deadline).map_err(Error::from)?;
            let mut response = vec![0; super::super::request::MAX_FRAME_BYTES];
            Ok(channel
                .receive(&mut response, deadline)
                .map_err(Error::from)?
                .to_vec())
        });
        if result.is_err() {
            self.burn();
        }
        result
    }
    fn burn(&mut self) {
        self.pending_review.take();
        self.bound = false;
        if let Some(channel) = self.channel.take()
            && channel.close().is_err()
        {
            self.client_cleanup_failed = true;
        }
    }
    pub fn ping(&mut self) -> Result<(), Error> {
        if self.exchange(PING)? != b"pong" {
            self.burn();
            return Err(Error::Channel);
        }
        Ok(())
    }
    pub fn fixture_status(&mut self) -> Result<serde_json::Value, Error> {
        let result =
            (|| serde_json::from_slice(&self.exchange(STATUS)?).map_err(|_| Error::Channel))();
        if result.is_err() {
            self.burn();
        }
        result
    }
    fn rpc(&mut self, request: &OwnerRequest, operation: &str) -> Result<WireReply, Error> {
        let bytes = serde_json::to_vec(request).map_err(|_| Error::Arguments)?;
        super::super::request::decode_untrusted(&bytes).map_err(|_| Error::Arguments)?;
        let result = (|| {
            let bytes = self.exchange(&bytes)?;
            let text = std::str::from_utf8(&bytes).map_err(|_| Error::Channel)?;
            cryptex_events::reject_duplicate_json_keys(text).map_err(|_| Error::Channel)?;
            let response: WireReply = serde_json::from_slice(&bytes).map_err(|_| Error::Channel)?;
            if !valid_reply(&response, request, operation) {
                return Err(Error::Channel);
            }
            Ok(response)
        })();
        if result.is_err() {
            self.burn();
        }
        result
    }
    fn pairing_exchange(
        &mut self,
        request: OwnerRequest,
        operation: &str,
    ) -> Result<DesktopBootstrapExchangeResult, Error> {
        if self.pending_review.is_some() {
            return Err(Error::Arguments);
        }
        let reply = self.rpc(&request, operation)?;
        Ok(DesktopBootstrapExchangeResult {
            accepted: reply.accepted,
            error: reply.error,
            transaction_id: reply.transaction_id,
            exchange_message: reply.exchange_message,
            brain_identity_json: reply.brain_identity_json,
        })
    }
    pub fn read_pairing_brain_identity(&mut self) -> Result<String, Error> {
        let reply = self.pairing_exchange(
            request("bootstrap.exchange.brain_identity.read")?,
            "bootstrap.exchange.brain_identity.read",
        )?;
        if !reply.accepted {
            return Err(Error::Arguments);
        }
        let identity = reply.brain_identity_json.ok_or(Error::Channel)?;
        cryptex_core::BrainIdentity::from_json(&identity).map_err(|_| Error::Channel)?;
        Ok(identity)
    }
    pub fn read_pairing_transport_identity(
        &mut self,
    ) -> Result<DesktopBootstrapExchangeResult, Error> {
        let reply = self.pairing_exchange(
            request("bootstrap.exchange.transport_identity.read")?,
            "bootstrap.exchange.transport_identity.read",
        )?;
        if reply.accepted
            && !reply
                .exchange_message
                .as_deref()
                .is_some_and(crate::syncthing_rest::native_id_shape)
        {
            self.burn();
            return Err(Error::Channel);
        }
        Ok(reply)
    }
    pub fn create_pairing_offer(
        &mut self,
        transaction_id: String,
        transport_device_id: String,
    ) -> Result<DesktopBootstrapExchangeResult, Error> {
        let mut request = request("bootstrap.exchange.offer.create")?;
        request.transaction_id = Some(transaction_id);
        request.transport_device_id = Some(transport_device_id);
        self.pairing_exchange(request, "bootstrap.exchange.offer.created")
    }
    pub fn create_pairing_proof(
        &mut self,
        offer: String,
        brain: String,
        transport: String,
    ) -> Result<DesktopBootstrapExchangeResult, Error> {
        let mut request = request("bootstrap.exchange.proof.create")?;
        request.offer_message = Some(offer);
        request.brain_identity_json = Some(brain);
        request.transport_device_id = Some(transport);
        self.pairing_exchange(request, "bootstrap.exchange.proof.created")
    }
    pub fn prepare_peer(
        &mut self,
        role: String,
        messages: Vec<String>,
    ) -> Result<DesktopBootstrapApprovalPrepared, Error> {
        if self.pending_review.is_some() {
            return Err(Error::Arguments);
        }
        let result = (|| {
            let mut request = request("bootstrap.approval.prepare")?;
            request.role = Some(role);
            request.transcript_messages = Some(messages);
            super::super::request::decode_untrusted(
                &serde_json::to_vec(&request).map_err(|_| Error::Arguments)?,
            )
            .map_err(|_| Error::Arguments)?;
            // Compare the complete signed request, not just its displayed IDs.
            let expected = peer_snapshot(&request)?;
            let reply = self.rpc(&request, "bootstrap.approval.prepared")?;
            if !reply.accepted {
                return Err(Error::Arguments);
            }
            let snapshot = reply.snapshot.ok_or(Error::Channel)?;
            let digest = snapshot.digest().map_err(|_| Error::Channel)?;
            if snapshot != expected
                || reply.approval_request_digest.as_deref() != Some(digest.as_str())
            {
                return Err(Error::Channel);
            }
            request.approval_request_digest = Some(digest);
            request.capability = reply.capability;
            request.operation = "bootstrap.approval.confirm".into();
            super::super::request::decode_untrusted(
                &serde_json::to_vec(&request).map_err(|_| Error::Arguments)?,
            )
            .map_err(|_| Error::Channel)?;
            let handle = random_id()?;
            self.pending_review = Some(PendingReview {
                handle: handle.clone(),
                request,
            });
            Ok(DesktopBootstrapApprovalPrepared {
                session_handle: handle,
                snapshot: snapshot.into(),
            })
        })();
        if result.is_err() {
            self.burn();
        }
        result
    }
    pub fn finish_peer(
        &mut self,
        handle: &str,
        confirm: bool,
    ) -> Result<DesktopBootstrapApprovalResult, Error> {
        let Some(pending) = self.pending_review.take() else {
            return Err(Error::Arguments);
        };
        let request = pending
            .followup(handle, true, confirm)
            .inspect_err(|_| self.burn())?;
        let reply = self.rpc(
            &request,
            if confirm {
                "bootstrap.approval.signed"
            } else {
                "bootstrap.approval.cancelled"
            },
        )?;
        Ok(DesktopBootstrapApprovalResult {
            accepted: reply.accepted,
            error: reply.error,
            signed_message: reply.signed_message,
        })
    }
    pub fn complete_pairing(
        &mut self,
        messages: Vec<String>,
    ) -> Result<DesktopBootstrapApprovalResult, Error> {
        if self.pending_review.is_some() {
            return Err(Error::Arguments);
        }
        let mut request = request("bootstrap.approval.complete")?;
        request.transcript_messages = Some(messages);
        let reply = self.rpc(&request, "bootstrap.approval.anchored")?;
        Ok(DesktopBootstrapApprovalResult {
            accepted: reply.accepted,
            error: reply.error,
            signed_message: reply.signed_message,
        })
    }
    /// Actual native domain calls; these never manufacture an active adapter.
    pub fn exercise_pairing_boundary(&mut self) -> Result<(), Error> {
        let before = self.fixture_status()?;
        let brain = self.read_pairing_brain_identity()?;
        if cryptex_core::BrainIdentity::from_json(&brain)
            .map_err(|_| Error::Channel)?
            .brain_id
            != "aa5d7ae0-5321-4f53-a91e-36a411d88591"
        {
            return Err(Error::Channel);
        }
        let transport = "AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA";
        for reply in [
            self.read_pairing_transport_identity()?,
            self.create_pairing_offer(random_id()?, transport.into())?,
            self.create_pairing_proof("{}".into(), brain, transport.into())?,
        ] {
            if reply.accepted
                || reply.error.as_deref() != Some("transport_identity_unavailable")
                || reply.exchange_message.is_some()
            {
                return Err(Error::StaleAccepted);
            }
        }
        let complete = self.complete_pairing(vec!["{}".into(); 4])?;
        if complete.accepted || complete.signed_message.is_some() {
            return Err(Error::StaleAccepted);
        }
        let mut stale = request("bootstrap.approval.confirm")?;
        stale.role = Some("initiator".into());
        stale.transcript_messages = Some(vec!["{}".into(); 2]);
        stale.approval_request_digest = Some("aa".repeat(32));
        stale.capability = Some("bb".repeat(32));
        let refused = self.rpc(&stale, "bootstrap.approval.result")?;
        if refused.accepted
            || refused.error.as_deref() != Some("approval_session_missing")
            || self.fixture_status()? != before
        {
            return Err(Error::StaleAccepted);
        }
        eprintln!("native_pairing_boundary_refusals=passed");
        Ok(())
    }
    pub fn prepare_owner(
        &mut self,
        input: DesktopOwnerSecurityOperationInput,
    ) -> Result<DesktopOwnerSecurityOperationPrepared, Error> {
        self.prepare_owner_with(input, random_id()?, random_id)
    }
    fn prepare_owner_with(
        &mut self,
        input: DesktopOwnerSecurityOperationInput,
        request_id: String,
        allocate_handle: impl FnOnce() -> Result<String, Error>,
    ) -> Result<DesktopOwnerSecurityOperationPrepared, Error> {
        if self.pending_review.is_some() {
            return Err(Error::Arguments);
        }
        let result = (|| {
            let operation =
                crate::ipc::parse_owner_operation(input).map_err(|_| Error::Arguments)?;
            let mut request: OwnerRequest = serde_json::from_value(serde_json::json!({
            "protocolVersion": crate::compat::DAEMON_PROTOCOL_VERSION, "requestId": request_id,
            "clientRole":"desktop", "capabilities":["peer.approval.manage"], "operation":"owner.security.prepare",
            "transactionId":random_id()?, "ownerOperation": operation,
        })).map_err(|_| Error::Arguments)?;
            let reply = self.rpc(&request, "owner.security.prepared")?;
            if !reply.accepted {
                return Err(Error::Arguments);
            }
            let snapshot = reply.snapshot.ok_or(Error::Channel)?;
            let digest = snapshot.digest().map_err(|_| Error::Channel)?;
            if reply.approval_request_digest.as_deref() != Some(digest.as_str())
                || snapshot.transaction_id != request.transaction_id.as_deref().unwrap_or_default()
            {
                self.burn();
                return Err(Error::Channel);
            }
            request.approval_request_digest = Some(digest);
            request.capability = reply.capability;
            request.owner_operation = None;
            if request.capability.as_ref().is_none_or(|c| c.len() != 64) {
                self.burn();
                return Err(Error::Channel);
            }
            let handle = allocate_handle()?;
            self.pending_review = Some(PendingReview {
                handle: handle.clone(),
                request,
            });
            Ok(DesktopOwnerSecurityOperationPrepared {
                session_handle: handle,
                snapshot: snapshot.into(),
            })
        })();
        if result.is_err() {
            self.burn();
        }
        result
    }
    pub fn verify_invalidated(&self) -> Result<(), Error> {
        if self.bound || self.channel.is_some() || self.pending_review.is_some() {
            Err(Error::StaleAccepted)
        } else {
            Ok(())
        }
    }
    pub fn exercise_prepare_failure(&mut self, missing_snapshot: bool) -> Result<(), Error> {
        let input = DesktopOwnerSecurityOperationInput {
            kind: "self_retirement".into(),
            target_device_id: None,
            reason: None,
            cutoff_seq: None,
        };
        let id = if missing_snapshot {
            "test-accepted-without-snapshot".into()
        } else {
            random_id()?
        };
        let result = self.prepare_owner_with(input, id, || {
            if missing_snapshot {
                random_id()
            } else {
                Err(Error::Channel)
            }
        });
        if result.is_ok() {
            return Err(Error::StaleAccepted);
        }
        self.verify_invalidated()
    }
    pub fn finish_owner(
        &mut self,
        handle: &str,
        confirm: bool,
    ) -> Result<DesktopOwnerSecurityOperationResult, Error> {
        let Some(pending) = self.pending_review.take() else {
            return Err(Error::Arguments);
        };
        let request = pending
            .followup(handle, false, confirm)
            .inspect_err(|_| self.burn())?;
        let operation = if confirm {
            "owner.security.result"
        } else {
            "owner.security.cancelled"
        };
        let reply = self.rpc(&request, operation)?;
        Ok(DesktopOwnerSecurityOperationResult {
            accepted: reply.accepted,
            error: reply.error,
        })
    }
    /// Synthetic driver only: real domain prepare/cancel/confirm, mismatch and
    /// replay over this actual native channel. Never claimed as human approval.
    pub fn exercise_synthetic_owner_sessions(&mut self) -> Result<(), Error> {
        self.exercise_and_capture_confirmation().map(|_| ())
    }
    pub fn exercise_and_capture_confirmation(&mut self) -> Result<ConfirmationReplayProbe, Error> {
        let input = || DesktopOwnerSecurityOperationInput {
            kind: "self_retirement".into(),
            target_device_id: None,
            reason: None,
            cutoff_seq: None,
        };
        let check = |status: serde_json::Value, count: u64| -> Result<(), Error> {
            if status["eventCount"].as_u64() != Some(count) || status["prefixPreserved"] != true {
                return Err(Error::Channel);
            }
            Ok(())
        };
        check(self.fixture_status()?, 1)?;
        let prepared = self.prepare_owner(input())?;
        if !self.finish_owner(&prepared.session_handle, false)?.accepted {
            return Err(Error::Channel);
        }
        if self.finish_owner(&prepared.session_handle, true).is_ok() {
            return Err(Error::Channel);
        }
        check(self.fixture_status()?, 1)?;
        self.prepare_owner(input())?;
        let mut pending = self.pending_review.take().ok_or(Error::Channel)?;
        pending.request.operation = "owner.security.confirm".into();
        let exact = serde_json::to_vec(&pending.request).map_err(|_| Error::Channel)?;
        pending.request.approval_request_digest = Some("ab".repeat(32));
        if self
            .rpc(&pending.request, "owner.security.result")?
            .accepted
        {
            return Err(Error::Channel);
        }
        let original: OwnerRequest = serde_json::from_slice(&exact).map_err(|_| Error::Channel)?;
        if self.rpc(&original, "owner.security.result")?.accepted {
            return Err(Error::Channel);
        }
        check(self.fixture_status()?, 1)?;
        let prepared = self.prepare_owner(input())?;
        let confirmed = self.capture_confirmation_probe()?;
        if !self.finish_owner(&prepared.session_handle, true)?.accepted {
            return Err(Error::Channel);
        }
        self.require_wire_replay_refusal(&confirmed)?;
        check(self.fixture_status()?, 2)?;
        Ok(confirmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn refusal(request: &OwnerRequest) -> WireReply {
        serde_json::from_value(serde_json::json!({
            "protocolVersion":crate::compat::DAEMON_PROTOCOL_VERSION,
            "requestId":request.request_id,"accepted":false,
            "error":"transport_identity_unavailable","operation":"bootstrap.exchange.result"
        }))
        .unwrap()
    }
    #[test]
    fn exchange_refusal_cannot_masquerade_as_success_or_another_request() {
        let request = request("bootstrap.exchange.offer.create").unwrap();
        let mut reply = refusal(&request);
        assert!(valid_reply(
            &reply,
            &request,
            "bootstrap.exchange.offer.created"
        ));
        reply.accepted = true;
        assert!(!valid_reply(
            &reply,
            &request,
            "bootstrap.exchange.offer.created"
        ));
        reply.accepted = false;
        reply.request_id = Some("another-request".into());
        assert!(!valid_reply(
            &reply,
            &request,
            "bootstrap.exchange.offer.created"
        ));
        reply.request_id = Some(request.request_id.clone());
        reply.capability = Some("aa".repeat(32));
        assert!(!valid_reply(
            &reply,
            &request,
            "bootstrap.exchange.offer.created"
        ));
        reply.capability = None;
        reply.exchange_message = Some("must-not-release-a-message".into());
        assert!(!valid_reply(
            &reply,
            &request,
            "bootstrap.exchange.offer.created"
        ));
    }
    #[test]
    fn peer_confirmation_requires_signed_success_and_accepts_only_empty_refusal() {
        let request = request("bootstrap.approval.confirm").unwrap();
        let mut reply = refusal(&request);
        reply.operation = "bootstrap.approval.signed".into();
        reply.accepted = true;
        reply.error = None;
        reply.signed_message = Some("signed-message-fixture".into());
        assert!(valid_reply(&reply, &request, "bootstrap.approval.signed"));
        reply.signed_message = None;
        assert!(!valid_reply(&reply, &request, "bootstrap.approval.signed"));
        reply.signed_message = Some("signed-message-fixture".into());
        // The generic result is a refusal envelope, never successful signing.
        reply.operation = "bootstrap.approval.result".into();
        assert!(!valid_reply(&reply, &request, "bootstrap.approval.signed"));
        reply.accepted = false;
        reply.error = Some("approval_confirmation_failed".into());
        assert!(!valid_reply(&reply, &request, "bootstrap.approval.signed"));
        reply.signed_message = None;
        assert!(valid_reply(&reply, &request, "bootstrap.approval.signed"));
        reply.request_id = Some("another-request".into());
        assert!(!valid_reply(&reply, &request, "bootstrap.approval.signed"));
    }
    #[test]
    fn pending_handles_cannot_cross_owner_and_peer_operations() {
        for (operation, peer) in [
            ("owner.security.prepare", true),
            ("bootstrap.approval.confirm", false),
        ] {
            let pending = PendingReview {
                handle: "local".into(),
                request: request(operation).unwrap(),
            };
            assert!(pending.followup("local", peer, true).is_err());
        }
        let pending = PendingReview {
            handle: "local".into(),
            request: request("bootstrap.approval.confirm").unwrap(),
        };
        assert!(pending.followup("other", true, true).is_err());
    }
    #[test]
    fn peer_followup_keeps_the_exact_reviewed_transcript_and_private_grant() {
        let mut request = request("bootstrap.approval.confirm").unwrap();
        request.role = Some("responder".into());
        request.transcript_messages = Some(vec!["offer".into(), "proof".into(), "approval".into()]);
        request.approval_request_digest = Some("aa".repeat(32));
        request.capability = Some("bb".repeat(32));
        let mut expected = serde_json::to_value(&request).unwrap();
        expected["operation"] = "bootstrap.approval.cancel".into();
        let pending = PendingReview {
            handle: "local".into(),
            request,
        };
        let followup = pending.followup("local", true, false).unwrap();
        assert_eq!(serde_json::to_value(followup).unwrap(), expected);
    }
    #[test]
    fn peer_snapshot_requires_signed_order_and_binds_both_transport_ids() {
        use cryptex_core::DeviceId;
        use ed25519_dalek::SigningKey;
        let key = SigningKey::from_bytes(&[0x31; 32]);
        let peer_key = SigningKey::from_bytes(&[0x32; 32]);
        let local = DeviceId::new("11111111-1111-4111-8111-111111111111").unwrap();
        let peer = DeviceId::new("22222222-2222-4222-8222-222222222222").unwrap();
        let transport = "AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA";
        let peer_transport = "BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB";
        let offer = cryptex_events::create_bootstrap_offer(
            "aa5d7ae0-5321-4f53-a91e-36a411d88591",
            &"ab".repeat(32),
            &local,
            transport,
            &[1; 32],
            &key,
        )
        .unwrap();
        let proof = cryptex_events::create_bootstrap_proof(
            &offer,
            &peer,
            peer_transport,
            &[2; 32],
            &peer_key,
        )
        .unwrap();
        let mut request = request("bootstrap.approval.prepare").unwrap();
        request.role = Some("initiator".into());
        request.transcript_messages = Some(vec![
            String::from_utf8(offer).unwrap(),
            String::from_utf8(proof).unwrap(),
        ]);
        let snapshot = peer_snapshot(&request).unwrap();
        let association = snapshot.transport_association.unwrap();
        assert_eq!(association.local_device_id, transport);
        assert_eq!(association.peer_device_id, peer_transport);
        request.transcript_messages.as_mut().unwrap().swap(0, 1);
        assert!(peer_snapshot(&request).is_err());
        request.transcript_messages.as_mut().unwrap().swap(0, 1);
        request.role = Some("responder".into());
        assert!(peer_snapshot(&request).is_err());
    }
}
