//! Shared closed wire types. Decoding is validation of untrusted data, not a
//! kernel peer proof and not permission to invoke a signer or canonical writer.

use serde::{Deserialize, Serialize};

use super::{ChannelError, Method};

pub(crate) const MAX_FRAME_BYTES: usize = 1024 * 1024 + 4096;
pub(super) const MAX_MESSAGE_BYTES: usize = 64 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerRequest {
    pub(crate) protocol_version: u32,
    pub(crate) request_id: String,
    pub(crate) client_role: String,
    pub(crate) capabilities: Vec<String>,
    pub(crate) operation: String,
    #[serde(default)]
    pub(crate) role: Option<String>,
    #[serde(default)]
    pub(crate) transcript_messages: Option<Vec<String>>,
    #[serde(default)]
    pub(crate) approval_request_digest: Option<String>,
    #[serde(default)]
    pub(crate) capability: Option<String>,
    #[serde(default)]
    pub(crate) transport_device_id: Option<String>,
    #[serde(default)]
    pub(crate) offer_message: Option<String>,
    #[serde(default)]
    pub(crate) brain_identity_json: Option<String>,
    #[serde(default)]
    pub(crate) transaction_id: Option<String>,
    #[serde(default)]
    pub(crate) owner_operation: Option<OwnerOperation>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum OwnerOperation {
    DeviceRestriction {
        target_device_id: String,
        reason: crate::bootstrap_journal::RestrictionReason,
        cutoff_seq: u64,
    },
    SelfRetirement,
}

impl std::fmt::Debug for OwnerRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OwnerRequest([redacted])")
    }
}

pub(crate) struct DecodedRequest {
    pub(crate) method: Method,
    pub(crate) request: OwnerRequest,
}

impl std::fmt::Debug for DecodedRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DecodedRequest")
            .field("method", &self.method)
            .finish_non_exhaustive()
    }
}

fn digest(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn transport(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        value.len() == 63
            && value.split('-').count() == 8
            && value.split('-').all(|group| {
                group.len() == 7
                    && group
                        .bytes()
                        .all(|byte| byte.is_ascii_uppercase() || (b'2'..=b'7').contains(&byte))
            })
    })
}

fn transcript(request: &OwnerRequest, count: usize) -> bool {
    request
        .transcript_messages
        .as_ref()
        .is_some_and(|messages| {
            messages.len() == count
                && messages
                    .iter()
                    .all(|message| !message.is_empty() && message.len() <= MAX_MESSAGE_BYTES)
        })
}

pub(crate) fn decode_untrusted(bytes: &[u8]) -> Result<DecodedRequest, ChannelError> {
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(ChannelError::FrameTooLarge);
    }
    let request: OwnerRequest =
        serde_json::from_slice(bytes).map_err(|_| ChannelError::MalformedRequest)?;
    if request.protocol_version != crate::compat::DAEMON_PROTOCOL_VERSION
        || request.client_role != "desktop"
        || request.request_id.is_empty()
        || request.request_id.len() > 128
        || !request
            .request_id
            .bytes()
            .all(|byte| byte.is_ascii_graphic())
        || request.capabilities.as_slice() != ["peer.approval.manage"]
    {
        return Err(ChannelError::UnauthorizedEnvelope);
    }
    let method = Method::parse(&request.operation)?;
    let is_owner = matches!(
        method,
        Method::PrepareOwnerOperation
            | Method::ConfirmOwnerOperation
            | Method::CancelOwnerOperation
    );
    if request.transport_device_id.is_some()
        && !matches!(method, Method::CreateOffer | Method::CreateProof)
        || request.offer_message.is_some() && method != Method::CreateProof
        || request.brain_identity_json.is_some() && method != Method::CreateProof
        || request.owner_operation.is_some() && method != Method::PrepareOwnerOperation
        || is_owner && (request.role.is_some() || request.transcript_messages.is_some())
    {
        return Err(ChannelError::InvalidRequestShape);
    }
    let no_grant = request.approval_request_digest.is_none() && request.capability.is_none();
    let grant =
        digest(request.approval_request_digest.as_deref()) && digest(request.capability.as_deref());
    let exchange = matches!(
        method,
        Method::ReadBrainIdentity
            | Method::ReadTransportIdentity
            | Method::CreateOffer
            | Method::CreateProof
    );
    if exchange && (request.role.is_some() || request.transcript_messages.is_some() || !no_grant) {
        return Err(ChannelError::InvalidRequestShape);
    }
    let valid = match method {
        Method::ReadBrainIdentity | Method::ReadTransportIdentity => {
            request.transaction_id.is_none()
        }
        Method::CreateOffer => {
            transport(request.transport_device_id.as_deref())
                && digest(request.transaction_id.as_deref())
        }
        Method::CreateProof => {
            request.transaction_id.is_none()
                && transport(request.transport_device_id.as_deref())
                && request
                    .offer_message
                    .as_ref()
                    .is_some_and(|value| !value.is_empty() && value.len() <= MAX_MESSAGE_BYTES)
                && request
                    .brain_identity_json
                    .as_ref()
                    .is_some_and(|value| !value.is_empty() && value.len() <= 4096)
        }
        Method::PrepareApproval | Method::ConfirmApproval | Method::CancelApproval => {
            let count = match request.role.as_deref() {
                Some("initiator") => 2,
                Some("responder") => 3,
                _ => return Err(ChannelError::InvalidRequestShape),
            };
            request.transaction_id.is_none()
                && transcript(&request, count)
                && if method == Method::PrepareApproval {
                    no_grant
                } else {
                    grant
                }
        }
        Method::CompleteApproval => {
            request.role.is_none()
                && request.transaction_id.is_none()
                && no_grant
                && transcript(&request, 4)
        }
        Method::PrepareOwnerOperation => {
            digest(request.transaction_id.as_deref())
                && no_grant
                && request.owner_operation.is_some()
        }
        Method::ConfirmOwnerOperation | Method::CancelOwnerOperation => {
            digest(request.transaction_id.as_deref()) && grant
        }
    };
    if !valid {
        return Err(ChannelError::InvalidRequestShape);
    }
    Ok(DecodedRequest { method, request })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn base(operation: &str) -> Value {
        json!({"protocolVersion":crate::compat::DAEMON_PROTOCOL_VERSION,
            "requestId":"local-request", "clientRole":"desktop",
            "capabilities":["peer.approval.manage"],"operation":operation})
    }
    fn decode(value: &Value) -> Result<DecodedRequest, ChannelError> {
        decode_untrusted(&serde_json::to_vec(value).unwrap())
    }

    #[test]
    fn shared_envelope_remains_wire_compatible_without_conferring_authority() {
        let value = base("bootstrap.exchange.brain_identity.read");
        let decoded = decode(&value).unwrap();
        assert_eq!(decoded.method, Method::ReadBrainIdentity);
        assert_eq!(decoded.request.request_id, "local-request");
        assert_eq!(
            serde_json::to_value(&decoded.request).unwrap()["operation"],
            value["operation"]
        );
        assert!(!format!("{decoded:?}").contains("local-request"));
        assert!(!format!("{:?}", decoded.request).contains("local-request"));
    }

    #[test]
    fn decode_rejects_duplicates_unknown_fields_oversize_and_claimed_authority() {
        assert!(matches!(
            decode_untrusted(&vec![b' '; MAX_FRAME_BYTES + 1]),
            Err(ChannelError::FrameTooLarge)
        ));
        let text = serde_json::to_string(&base("bootstrap.exchange.brain_identity.read")).unwrap();
        let duplicate = text.replacen('{', "{\"clientRole\":\"desktop\",", 1);
        assert!(matches!(
            decode_untrusted(duplicate.as_bytes()),
            Err(ChannelError::MalformedRequest)
        ));
        for (field, value) in [
            ("kernelVerified", json!(true)),
            ("peerPid", json!(123)),
            ("auditToken", json!("token")),
        ] {
            let mut request = base("bootstrap.exchange.brain_identity.read");
            request[field] = value;
            assert!(matches!(
                decode(&request),
                Err(ChannelError::MalformedRequest)
            ));
        }
        for (field, value) in [
            ("clientRole", json!("cli")),
            ("capabilities", json!(["status.read"])),
            ("requestId", json!("has space")),
            ("protocolVersion", json!(0)),
        ] {
            let mut request = base("bootstrap.exchange.brain_identity.read");
            request[field] = value;
            assert!(matches!(
                decode(&request),
                Err(ChannelError::UnauthorizedEnvelope)
            ));
        }
    }

    #[test]
    fn approval_stages_require_exact_prefix_count_and_single_use_grant_fields() {
        for (role, count) in [("initiator", 2), ("responder", 3)] {
            let mut request = base("bootstrap.approval.prepare");
            request["role"] = json!(role);
            request["transcriptMessages"] = json!(vec!["{}"; count]);
            decode(&request).unwrap();
            request["operation"] = json!("bootstrap.approval.confirm");
            assert!(matches!(
                decode(&request),
                Err(ChannelError::InvalidRequestShape)
            ));
            request["approvalRequestDigest"] = json!("ab".repeat(32));
            request["capability"] = json!("cd".repeat(32));
            decode(&request).unwrap();
            request["ownerOperation"] = json!({"kind":"self_retirement"});
            assert!(matches!(
                decode(&request),
                Err(ChannelError::InvalidRequestShape)
            ));
        }
        let mut complete = base("bootstrap.approval.complete");
        complete["transcriptMessages"] = json!(["{}", "{}", "{}", "{}"]);
        decode(&complete).unwrap();
        complete["capability"] = json!("ab".repeat(32));
        assert!(matches!(
            decode(&complete),
            Err(ChannelError::InvalidRequestShape)
        ));
    }

    #[test]
    fn native_transport_read_accepts_no_renderer_claims_or_grants() {
        let request = base("bootstrap.exchange.transport_identity.read");
        assert_eq!(
            decode(&request).unwrap().method,
            Method::ReadTransportIdentity
        );
        for (field, value) in [
            ("transportDeviceId", json!(["AAAAAAA"; 8].join("-"))),
            ("transactionId", json!("aa".repeat(32))),
            ("role", json!("initiator")),
            ("capability", json!("aa".repeat(32))),
            ("transcriptMessages", json!(["{}", "{}"])),
        ] {
            let mut supplied = request.clone();
            supplied[field] = value;
            assert!(matches!(
                decode(&supplied),
                Err(ChannelError::InvalidRequestShape)
            ));
        }
    }

    #[test]
    fn owner_and_exchange_payloads_are_disjoint_and_bounded() {
        let mut owner = base("owner.security.prepare");
        owner["transactionId"] = json!("ab".repeat(32));
        owner["ownerOperation"] = json!({"kind":"self_retirement"});
        decode(&owner).unwrap();
        owner["operation"] = json!("owner.security.confirm");
        owner.as_object_mut().unwrap().remove("ownerOperation");
        owner["approvalRequestDigest"] = json!("ab".repeat(32));
        owner["capability"] = json!("cd".repeat(32));
        decode(&owner).unwrap();
        owner["role"] = json!("initiator");
        assert!(matches!(
            decode(&owner),
            Err(ChannelError::InvalidRequestShape)
        ));
        let mut proof = base("bootstrap.exchange.proof.create");
        proof["transportDeviceId"] = json!(["AAAAAAA"; 8].join("-"));
        proof["offerMessage"] = json!("{}");
        proof["brainIdentityJson"] = json!("{}");
        decode(&proof).unwrap();
        proof["brainIdentityJson"] = json!("x".repeat(4097));
        assert!(matches!(
            decode(&proof),
            Err(ChannelError::InvalidRequestShape)
        ));
    }
}
