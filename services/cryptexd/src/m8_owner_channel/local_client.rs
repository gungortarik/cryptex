//! Feature-only Desktop-side review handles. Wire capabilities never leave
//! this owner, are single-use and are lost with the exact native connection.
use super::*;
use crate::ipc::{
    DesktopOwnerSecurityOperationInput, DesktopOwnerSecurityOperationPrepared,
    DesktopOwnerSecurityOperationResult,
};
use crate::m8_owner_channel::request::OwnerRequest;

pub(super) struct PendingReview {
    handle: String,
    request: OwnerRequest,
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
impl LocalDaemon {
    pub fn capture_confirmation_probe(&self) -> Result<ConfirmationReplayProbe, Error> {
        let pending = self.pending_review.as_ref().ok_or(Error::Arguments)?;
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
        let result = (|| {
            let bytes = self.exchange(&bytes)?;
            let text = std::str::from_utf8(&bytes).map_err(|_| Error::Channel)?;
            cryptex_events::reject_duplicate_json_keys(text).map_err(|_| Error::Channel)?;
            let response: WireReply = serde_json::from_slice(&bytes).map_err(|_| Error::Channel)?;
            if response.protocol_version != crate::compat::DAEMON_PROTOCOL_VERSION
                || response.request_id.as_deref() != Some(&request.request_id)
                || response.operation != operation
            {
                return Err(Error::Channel);
            }
            Ok(response)
        })();
        if result.is_err() {
            self.burn();
        }
        result
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
        let Some(mut pending) = self.pending_review.take() else {
            return Err(Error::Arguments);
        };
        if pending.handle != handle {
            self.burn();
            return Err(Error::Arguments);
        }
        pending.request.operation = if confirm {
            "owner.security.confirm"
        } else {
            "owner.security.cancel"
        }
        .into();
        let operation = if confirm {
            "owner.security.result"
        } else {
            "owner.security.cancelled"
        };
        let reply = self.rpc(&pending.request, operation)?;
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
