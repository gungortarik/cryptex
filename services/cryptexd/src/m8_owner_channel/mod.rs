//! Initially disabled Mac M8 owner-message adapter.
//!
//! The approved candidate does not select a release signer, establish a stable
//! bundle snapshot, or activate native IPC. No environment variable, wire field
//! or successful Unix health request can enable it. Ordinary Unix APIs and the
//! Windows named-pipe implementation keep their existing policies.

use std::fmt;

pub(crate) mod request;

/// Closed operation inventory. A family prefix is routing information, never
/// authentication; unknown methods in these families must not fall back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Method {
    ReadBrainIdentity,
    CreateOffer,
    CreateProof,
    PrepareApproval,
    ConfirmApproval,
    CancelApproval,
    CompleteApproval,
    PrepareOwnerOperation,
    ConfirmOwnerOperation,
    CancelOwnerOperation,
}

impl Method {
    pub(crate) fn parse(operation: &str) -> Result<Self, ChannelError> {
        match operation {
            "bootstrap.exchange.brain_identity.read" => Ok(Self::ReadBrainIdentity),
            "bootstrap.exchange.offer.create" => Ok(Self::CreateOffer),
            "bootstrap.exchange.proof.create" => Ok(Self::CreateProof),
            "bootstrap.approval.prepare" => Ok(Self::PrepareApproval),
            "bootstrap.approval.confirm" => Ok(Self::ConfirmApproval),
            "bootstrap.approval.cancel" => Ok(Self::CancelApproval),
            "bootstrap.approval.complete" => Ok(Self::CompleteApproval),
            "owner.security.prepare" => Ok(Self::PrepareOwnerOperation),
            "owner.security.confirm" => Ok(Self::ConfirmOwnerOperation),
            "owner.security.cancel" => Ok(Self::CancelOwnerOperation),
            _ => Err(ChannelError::UnsupportedMethod),
        }
    }
}

pub(crate) fn owns_family(operation: &str) -> bool {
    operation.starts_with("bootstrap.exchange.")
        || operation.starts_with("bootstrap.approval.")
        || operation.starts_with("owner.security.")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelError {
    Disabled,
    UnsupportedMethod,
    FrameTooLarge,
    MalformedRequest,
    UnauthorizedEnvelope,
    InvalidRequestShape,
}

impl ChannelError {
    pub fn code(self) -> &'static str {
        match self {
            Self::Disabled => "mac_owner_channel_disabled",
            Self::UnsupportedMethod => "owner_channel_method_rejected",
            Self::FrameTooLarge => "owner_channel_frame_too_large",
            Self::MalformedRequest => "owner_channel_malformed_request",
            Self::UnauthorizedEnvelope => "owner_channel_envelope_rejected",
            Self::InvalidRequestShape => "owner_channel_request_shape_rejected",
        }
    }
}

impl fmt::Display for ChannelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl std::error::Error for ChannelError {}

/// This value cannot be changed by runtime configuration. Enabling an adapter
/// requires a later reviewed code change after product-policy/native gates.
pub(crate) fn require_candidate() -> Result<(), ChannelError> {
    Err(ChannelError::Disabled)
}

/// Called before any M8 Desktop socket connection, random grant generation or
/// native allocation. Windows continues through its existing authenticated pipe.
pub(crate) fn require_desktop_route(operation: &str) -> std::io::Result<()> {
    if !owns_family(operation) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            ChannelError::UnsupportedMethod,
        ));
    }
    Method::parse(operation)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
    #[cfg(target_os = "macos")]
    {
        require_candidate()
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::PermissionDenied, error))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const METHODS: [&str; 10] = [
        "bootstrap.exchange.brain_identity.read",
        "bootstrap.exchange.offer.create",
        "bootstrap.exchange.proof.create",
        "bootstrap.approval.prepare",
        "bootstrap.approval.confirm",
        "bootstrap.approval.cancel",
        "bootstrap.approval.complete",
        "owner.security.prepare",
        "owner.security.confirm",
        "owner.security.cancel",
    ];

    #[test]
    fn closed_inventory_never_uses_family_as_authorization() {
        for operation in METHODS {
            assert!(owns_family(operation));
            assert!(Method::parse(operation).is_ok());
        }
        for operation in [
            "bootstrap.exchange.unknown",
            "bootstrap.approval.prepare.extra",
            "owner.security.unknown",
        ] {
            assert!(owns_family(operation));
            assert_eq!(
                Method::parse(operation),
                Err(ChannelError::UnsupportedMethod)
            );
        }
        for operation in [
            "health.ping",
            "content.search",
            "owner.security",
            "Owner.security.prepare",
        ] {
            assert!(!owns_family(operation));
            assert_eq!(
                Method::parse(operation),
                Err(ChannelError::UnsupportedMethod)
            );
        }
    }

    #[test]
    fn candidate_has_no_enabled_or_configuration_fallback_path() {
        for _ in 0..3 {
            assert_eq!(require_candidate(), Err(ChannelError::Disabled));
        }
        for operation in METHODS {
            let route = require_desktop_route(operation);
            if cfg!(target_os = "macos") {
                let error = route.unwrap_err();
                assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
                assert_eq!(error.to_string(), "mac_owner_channel_disabled");
            } else {
                route.unwrap();
            }
        }
    }
}
