//! Transport eligibility from a locked verified canonical snapshot, never from
//! mutable adapter configuration or transported pairing metadata. This is not
//! an export permit: the controller must retain/recheck runtime admission and
//! the snapshot through any configuration/export operation.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::knowledge_materialization::VerifiedCanonicalSnapshot;
use crate::syncthing_rest::native_id_shape;
use crate::trust_reducer::{
    ApprovedPeerBinding, TrustSnapshot, VerifiedHistoryState, VerifiedTrustHistory,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExchangePolicyError {
    WrongRuntime,
    HistoryUnavailable,
    InvalidBinding,
    LocalRestricted,
}

impl std::fmt::Display for ExchangePolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::WrongRuntime => "exchange_runtime_mismatch",
            Self::HistoryUnavailable => "exchange_history_unavailable",
            Self::InvalidBinding => "exchange_binding_invalid",
            Self::LocalRestricted => "exchange_local_restricted",
        })
    }
}
impl std::error::Error for ExchangePolicyError {}

fn context<'a>(
    snapshot: &'a VerifiedCanonicalSnapshot,
    root: &Path,
) -> Result<(&'a VerifiedTrustHistory, &'a TrustSnapshot), ExchangePolicyError> {
    if !snapshot.locks_runtime(root) {
        return Err(ExchangePolicyError::WrongRuntime);
    }
    snapshot
        .verified_trust_context()
        .ok_or(ExchangePolicyError::HistoryUnavailable)
}

/// Cached incoming history can remain valid after the last network peer is
/// restricted. Verify identity here; the publisher owns per-record cutoffs,
/// replay and whole-union validation. Never use active connection eligibility
/// as a substitute for those historical checks.
pub(crate) fn validate_incoming_binding(
    snapshot: &VerifiedCanonicalSnapshot,
    root: &Path,
    observed_local_transport: &str,
) -> Result<(), ExchangePolicyError> {
    let (history, trust) = context(snapshot, root)?;
    validate_bindings(history, trust, observed_local_transport)?;
    if history.approved_peer_bindings.is_empty() {
        return Err(ExchangePolicyError::InvalidBinding);
    }
    Ok(())
}

/// Non-cloneable view borrowing the snapshot that retains inventory/device
/// locks. Empty means no eligible connection, not permission to use adapter peers.
pub(crate) struct ExchangePeers<'a> {
    _snapshot: &'a VerifiedCanonicalSnapshot,
    root: PathBuf,
    local_transport: String,
    peers: Vec<&'a ApprovedPeerBinding>,
}

impl<'a> ExchangePeers<'a> {
    pub(crate) fn from_snapshot(
        snapshot: &'a VerifiedCanonicalSnapshot,
        root: &Path,
        observed_local_transport: &str,
    ) -> Result<Self, ExchangePolicyError> {
        let (history, trust) = context(snapshot, root)?;
        Ok(Self {
            _snapshot: snapshot,
            root: root.to_owned(),
            local_transport: observed_local_transport.to_owned(),
            peers: eligible_peers(history, trust, observed_local_transport)?,
        })
    }

    pub(crate) fn bindings(&self) -> &[&'a ApprovedPeerBinding] {
        &self.peers
    }

    pub(crate) fn runtime_root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn brain_id(&self) -> &str {
        self._snapshot.brain_id()
    }

    pub(crate) fn local_transport(&self) -> &str {
        &self.local_transport
    }

    pub(crate) fn revalidate(&self) -> Result<(), ExchangePolicyError> {
        let (history, trust) = context(self._snapshot, &self.root)?;
        let current = eligible_peers(history, trust, &self.local_transport)?;
        if current != self.peers {
            return Err(ExchangePolicyError::InvalidBinding);
        }
        Ok(())
    }
}

fn validate_bindings(
    history: &VerifiedTrustHistory,
    trust: &TrustSnapshot,
    observed_local_transport: &str,
) -> Result<(), ExchangePolicyError> {
    if history.state != VerifiedHistoryState::Complete
        || !trust.trusted_devices.contains(&history.local_device)
        || trust
            .trusted_devices
            .iter()
            .any(|id| !trust.effective_cutoffs.contains_key(id))
    {
        return Err(ExchangePolicyError::HistoryUnavailable);
    }
    if !native_id_shape(observed_local_transport)
        || history.approved_peer_bindings.len() != history.directly_approved_peers.len()
        || history
            .directly_approved_peers
            .contains(&history.local_device)
        || trust.trusted_devices.len() != history.directly_approved_peers.len() + 1
        || !history
            .directly_approved_peers
            .is_subset(&trust.trusted_devices)
    {
        return Err(ExchangePolicyError::InvalidBinding);
    }
    // Validate *all* bindings before filtering restricted peers: drift in a
    // retained approval is not silently discarded as a harmless empty roster.
    let mut transport_ids = BTreeSet::from([observed_local_transport]);
    for (device, binding) in &history.approved_peer_bindings {
        if device != &binding.device_id
            || !history.directly_approved_peers.contains(device)
            || binding.local_transport_device_id != observed_local_transport
            || !native_id_shape(&binding.transport_device_id)
            || !transport_ids.insert(binding.transport_device_id.as_str())
        {
            return Err(ExchangePolicyError::InvalidBinding);
        }
    }
    Ok(())
}

fn eligible_peers<'a>(
    history: &'a VerifiedTrustHistory,
    trust: &TrustSnapshot,
    observed_local_transport: &str,
) -> Result<Vec<&'a ApprovedPeerBinding>, ExchangePolicyError> {
    validate_bindings(history, trust, observed_local_transport)?;
    if history.restricted(trust, &history.local_device)
        || trust.effective_cutoffs[&history.local_device] != u64::MAX
    {
        return Err(ExchangePolicyError::LocalRestricted);
    }
    Ok(history
        .approved_peer_bindings
        .iter()
        .filter(|(device, _)| {
            trust.effective_cutoffs[*device] == u64::MAX && !history.restricted(trust, device)
        })
        .map(|(_, binding)| binding)
        .collect())
}

#[cfg(test)]
mod tests;
