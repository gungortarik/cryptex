//! Deterministic M8 trust/cutoff reduction over a complete, pre-verified union.
//!
//! This module performs no parsing, key resolution, signature verification, or
//! filesystem access. Its input must be built only from a complete verified
//! security history. It is not yet wired to retrieval, import publication, or
//! transport; callers must keep those surfaces held until the reader and
//! `RetrievalSafetyState` integration is complete and independently reviewed.

use std::collections::{BTreeMap, BTreeSet};

use cryptex_core::ids::DeviceId;
use cryptex_events::{
    AuthenticatedSecurityRecord, SecurityPayloadV3, SigningKeyId, TrustedDaemonKeyResolver,
    VerifiedSecurityHistory, ZERO_CONTENT_HASH_HEX, verify_bootstrap_transcript_values,
    verify_security_history_cancellable,
};
use ed25519_dalek::VerifyingKey;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VerifiedRestriction {
    pub issuer: DeviceId,
    pub issuer_seq: u64,
    pub target: DeviceId,
    pub cutoff_seq: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VerifiedSelfRetirement {
    pub device: DeviceId,
    pub event_seq: u64,
    pub last_authorized_seq: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VerifiedHistoryState {
    Complete,
    MissingOrUnverifiable,
    CompetingFork,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VerifiedTrustHistory {
    pub local_device: DeviceId,
    pub local_signing_key_id: SigningKeyId,
    pub local_public_key: [u8; 32],
    pub known_devices: BTreeSet<DeviceId>,
    pub directly_approved_peers: BTreeSet<DeviceId>,
    pub approved_peer_bindings: BTreeMap<DeviceId, ApprovedPeerBinding>,
    pub restrictions: Vec<VerifiedRestriction>,
    pub self_retirements: Vec<VerifiedSelfRetirement>,
    pub state: VerifiedHistoryState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApprovedPeerBinding {
    pub device_id: DeviceId,
    pub signing_key_id: SigningKeyId,
    pub public_key: [u8; 32],
    pub fingerprint_sha256: [u8; 32],
    pub local_transport_device_id: String,
    pub transport_device_id: String,
}

/// Resolver built only from the local installation key and peer bindings
/// reconstructed from its own verified approval anchors.
pub(crate) struct LocalAnchorResolver {
    local_device: DeviceId,
    local_key_id: SigningKeyId,
    local_key: VerifyingKey,
    peers: BTreeMap<DeviceId, ApprovedPeerBinding>,
}

impl TrustedDaemonKeyResolver for LocalAnchorResolver {
    fn trusted_daemon_key(&mut self, device_id: &str, key_id: &str) -> Option<VerifyingKey> {
        let device = DeviceId::new(device_id.to_owned()).ok()?;
        if device == self.local_device && key_id == self.local_key_id.as_str() {
            return Some(self.local_key);
        }
        let binding = self.peers.get(&device)?;
        (binding.signing_key_id.as_str() == key_id)
            .then(|| VerifyingKey::from_bytes(&binding.public_key).ok())
            .flatten()
    }
}

impl VerifiedTrustHistory {
    pub(crate) fn approved_peer_binding(
        &self,
        device_id: &DeviceId,
    ) -> Option<&ApprovedPeerBinding> {
        self.approved_peer_bindings.get(device_id)
    }

    pub(crate) fn key_resolver(&self) -> LocalAnchorResolver {
        LocalAnchorResolver {
            local_device: self.local_device.clone(),
            local_key_id: self.local_signing_key_id.clone(),
            local_key: VerifyingKey::from_bytes(&self.local_public_key)
                .expect("verified trust history has a valid local key"),
            peers: self.approved_peer_bindings.clone(),
        }
    }
}

struct LocalOnlyResolver {
    local_device: DeviceId,
    local_key_id: SigningKeyId,
    local_key: VerifyingKey,
}

impl TrustedDaemonKeyResolver for LocalOnlyResolver {
    fn trusted_daemon_key(&mut self, device_id: &str, key_id: &str) -> Option<VerifyingKey> {
        (device_id == self.local_device.as_str() && key_id == self.local_key_id.as_str())
            .then_some(self.local_key)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrustHold {
    Cancelled,
    MissingOrUnverifiableHistory,
    CompetingFork,
    InvalidPolicyHistory,
    UnresolvedRestrictionCycle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TrustSnapshot {
    pub trusted_devices: BTreeSet<DeviceId>,
    pub effective_cutoffs: BTreeMap<DeviceId, u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TrustReduction {
    Ready(TrustSnapshot),
    Held(TrustHold),
}

/// Derive reducer input from authenticated records. `history_state` must be
/// supplied by the caller that enumerated the complete lock-scoped canonical
/// snapshot; this function cannot establish source completeness itself.
pub(crate) fn build_verified_trust_history(
    history: &VerifiedSecurityHistory,
    local_device: DeviceId,
    local_signing_key_id: &SigningKeyId,
    local_public_key: &[u8; 32],
    history_state: VerifiedHistoryState,
) -> Result<VerifiedTrustHistory, TrustHold> {
    let approved_peer_bindings = derive_local_approval_roster(
        history,
        &local_device,
        local_signing_key_id,
        local_public_key,
    )?;
    let mut known_devices = BTreeSet::from([local_device.clone()]);
    let mut restrictions = Vec::new();
    let mut self_retirements = Vec::new();
    let mut transactions = BTreeMap::<DeviceId, BTreeSet<String>>::new();

    // Derive the direct roster before interpreting policy issuers. Records are
    // sorted by device stream, so a peer stream may sort before the local
    // stream even when its restriction is chronologically later. A single
    // interleaved pass would then silently drop an authorized restriction.
    for record in history.records() {
        let device = DeviceId::new(record.device_id().to_owned())
            .map_err(|_| TrustHold::InvalidPolicyHistory)?;
        known_devices.insert(device.clone());
        let AuthenticatedSecurityRecord::SecurityV3(event) = record else {
            continue;
        };
        known_devices.insert(device.clone());

        let transaction_id = match event.payload() {
            SecurityPayloadV3::PeerApproved { transaction_id, .. }
            | SecurityPayloadV3::DeviceRestricted { transaction_id, .. }
            | SecurityPayloadV3::DeviceSelfRetired { transaction_id, .. } => transaction_id,
        };
        if !transactions
            .entry(device.clone())
            .or_default()
            .insert(transaction_id.clone())
        {
            return Err(TrustHold::InvalidPolicyHistory);
        }

        match event.payload() {
            SecurityPayloadV3::PeerApproved { .. } => {}
            SecurityPayloadV3::DeviceRestricted {
                target_device_id, ..
            } => {
                known_devices.insert(target_device_id.clone());
            }
            SecurityPayloadV3::DeviceSelfRetired {
                last_authorized_seq,
                last_authorized_content_hash,
                ..
            } => {
                let event_seq = event
                    .device_seq()
                    .to_u64()
                    .map_err(|_| TrustHold::InvalidPolicyHistory)?;
                let previous = history
                    .records()
                    .iter()
                    .filter(|candidate| {
                        candidate.device_id() == device.as_str()
                            && candidate.device_seq() < event_seq
                    })
                    .max_by_key(|candidate| candidate.device_seq());
                match (*last_authorized_seq, previous) {
                    (0, None) if last_authorized_content_hash.as_str() == ZERO_CONTENT_HASH_HEX => {
                    }
                    (seq, Some(previous))
                        if seq == previous.device_seq()
                            && last_authorized_content_hash == previous.content_hash() => {}
                    _ => return Err(TrustHold::InvalidPolicyHistory),
                }
                if history.records().iter().any(|candidate| {
                    candidate.device_id() == device.as_str() && candidate.device_seq() > event_seq
                }) {
                    return Err(TrustHold::InvalidPolicyHistory);
                }
                self_retirements.push(VerifiedSelfRetirement {
                    device,
                    event_seq,
                    last_authorized_seq: *last_authorized_seq,
                });
            }
        }
    }

    // Evaluate restrictions only after the complete local approval roster has
    // been derived from verified local anchors.
    for record in history.records() {
        let AuthenticatedSecurityRecord::SecurityV3(event) = record else {
            continue;
        };
        let issuer = event.device_id().clone();
        let SecurityPayloadV3::DeviceRestricted {
            target_device_id,
            target_signing_key_id,
            cutoff_seq,
            cutoff_content_hash,
            ..
        } = event.payload()
        else {
            continue;
        };
        let issuer_is_authorized =
            issuer == local_device || approved_peer_bindings.contains_key(&issuer);
        if !issuer_is_authorized {
            continue;
        }
        if *cutoff_seq == 0 {
            if cutoff_content_hash.as_str() != ZERO_CONTENT_HASH_HEX {
                return Err(TrustHold::InvalidPolicyHistory);
            }
        } else {
            let boundary = history.records().iter().find(|candidate| {
                candidate.device_id() == target_device_id.as_str()
                    && candidate.device_seq() == *cutoff_seq
            });
            let Some(boundary) = boundary else {
                return Err(TrustHold::InvalidPolicyHistory);
            };
            if boundary.content_hash() != cutoff_content_hash
                || boundary.signing_key_id() != target_signing_key_id
            {
                return Err(TrustHold::InvalidPolicyHistory);
            }
        }
        restrictions.push(VerifiedRestriction {
            issuer,
            issuer_seq: event
                .device_seq()
                .to_u64()
                .map_err(|_| TrustHold::InvalidPolicyHistory)?,
            target: target_device_id.clone(),
            cutoff_seq: *cutoff_seq,
        });
    }

    let directly_approved_peers = approved_peer_bindings.keys().cloned().collect();
    Ok(VerifiedTrustHistory {
        local_device,
        local_signing_key_id: local_signing_key_id.clone(),
        local_public_key: *local_public_key,
        known_devices,
        directly_approved_peers,
        approved_peer_bindings,
        restrictions,
        self_retirements,
        state: history_state,
    })
}

fn derive_local_approval_roster(
    history: &VerifiedSecurityHistory,
    local_device: &DeviceId,
    local_signing_key_id: &SigningKeyId,
    local_public_key: &[u8; 32],
) -> Result<BTreeMap<DeviceId, ApprovedPeerBinding>, TrustHold> {
    let mut bindings = BTreeMap::<DeviceId, ApprovedPeerBinding>::new();
    for record in history.records() {
        if record.device_id() != local_device.as_str() {
            continue;
        }
        let AuthenticatedSecurityRecord::SecurityV3(event) = record else {
            continue;
        };
        let SecurityPayloadV3::PeerApproved {
            transaction_id,
            transcript,
        } = event.payload()
        else {
            continue;
        };
        let messages = transcript
            .as_array()
            .ok_or(TrustHold::InvalidPolicyHistory)?;
        let verified = verify_bootstrap_transcript_values(messages)
            .map_err(|_| TrustHold::InvalidPolicyHistory)?;
        if verified.transaction_id() != transaction_id
            || verified.brain_id() != history.expected_brain_id()
        {
            return Err(TrustHold::InvalidPolicyHistory);
        }
        let initiator_is_local = verified.initiator().device_id == *local_device;
        let responder_is_local = verified.responder().device_id == *local_device;
        if initiator_is_local == responder_is_local {
            return Err(TrustHold::InvalidPolicyHistory);
        }
        let (local_identity, peer_identity, local_transport, peer_transport) = if initiator_is_local
        {
            (
                verified.initiator(),
                verified.responder(),
                verified.initiator_transport_device_id(),
                verified.responder_transport_device_id(),
            )
        } else {
            (
                verified.responder(),
                verified.initiator(),
                verified.responder_transport_device_id(),
                verified.initiator_transport_device_id(),
            )
        };
        if local_identity.signing_key_id != *local_signing_key_id
            || &local_identity.public_key != local_public_key
            || record.signing_key_id() != local_signing_key_id
            || peer_identity.device_id == *local_device
            || peer_identity.signing_key_id == *local_signing_key_id
            || &peer_identity.public_key == local_public_key
            || peer_transport == local_transport
        {
            return Err(TrustHold::InvalidPolicyHistory);
        }
        let binding = ApprovedPeerBinding {
            device_id: peer_identity.device_id.clone(),
            signing_key_id: peer_identity.signing_key_id.clone(),
            public_key: peer_identity.public_key,
            fingerprint_sha256: peer_identity.fingerprint_sha256,
            local_transport_device_id: local_transport.to_owned(),
            transport_device_id: peer_transport.to_owned(),
        };
        if bindings
            .get(&binding.device_id)
            .is_some_and(|existing| existing != &binding)
            || bindings.values().any(|existing| {
                existing.device_id != binding.device_id
                    && (existing.transport_device_id == binding.transport_device_id
                        || existing.local_transport_device_id != binding.local_transport_device_id
                        || existing.signing_key_id == binding.signing_key_id
                        || existing.public_key == binding.public_key)
            })
        {
            return Err(TrustHold::InvalidPolicyHistory);
        }
        bindings.insert(binding.device_id.clone(), binding);
    }
    Ok(bindings)
}

/// Authenticate a complete lock-scoped canonical snapshot from this
/// installation's local anchor outward. `raw_records` must be the exhaustive
/// byte set supplied by the caller while its canonical inventory/device locks
/// are held. Unapproved device streams fail because the derived resolver has
/// no key for them.
#[allow(dead_code)] // Read-only resolver path awaits canonical scanner wiring.
pub(crate) fn verify_history_from_local_anchors(
    raw_records: &[&[u8]],
    expected_brain_id: &str,
    local_device: DeviceId,
    local_signing_key_id: SigningKeyId,
    local_public_key: [u8; 32],
) -> Result<(VerifiedSecurityHistory, TrustReduction), TrustHold> {
    verify_history_from_local_anchors_cancellable(
        raw_records,
        expected_brain_id,
        local_device,
        local_signing_key_id,
        local_public_key,
        || false,
    )
}

pub(crate) fn verify_history_from_local_anchors_cancellable<C>(
    raw_records: &[&[u8]],
    expected_brain_id: &str,
    local_device: DeviceId,
    local_signing_key_id: SigningKeyId,
    local_public_key: [u8; 32],
    cancelled: C,
) -> Result<(VerifiedSecurityHistory, TrustReduction), TrustHold>
where
    C: FnMut() -> bool,
{
    let (history, reduction, _) = verify_history_from_local_anchors_with_verified_cancellable(
        raw_records,
        expected_brain_id,
        local_device,
        local_signing_key_id,
        local_public_key,
        cancelled,
    )?;
    Ok((history, reduction))
}

pub(crate) fn verify_history_from_local_anchors_with_verified_cancellable<C>(
    raw_records: &[&[u8]],
    expected_brain_id: &str,
    local_device: DeviceId,
    local_signing_key_id: SigningKeyId,
    local_public_key: [u8; 32],
    mut cancelled: C,
) -> Result<
    (
        VerifiedSecurityHistory,
        TrustReduction,
        VerifiedTrustHistory,
    ),
    TrustHold,
>
where
    C: FnMut() -> bool,
{
    if cancelled() {
        return Err(TrustHold::Cancelled);
    }
    let local_key =
        VerifyingKey::from_bytes(&local_public_key).map_err(|_| TrustHold::InvalidPolicyHistory)?;
    if cryptex_events::signing_key_id_from_verifying_key(&local_key)
        .map_err(|_| TrustHold::InvalidPolicyHistory)?
        != local_signing_key_id
    {
        return Err(TrustHold::InvalidPolicyHistory);
    }
    let local_records = raw_records
        .iter()
        .filter_map(|bytes| {
            serde_json::from_slice::<serde_json::Value>(bytes)
                .ok()
                .filter(|value| {
                    value.get("device_id").and_then(serde_json::Value::as_str)
                        == Some(local_device.as_str())
                })
                .map(|_| *bytes)
        })
        .collect::<Vec<_>>();
    if local_records.is_empty() {
        return Err(TrustHold::MissingOrUnverifiableHistory);
    }
    let local_history = verify_security_history_cancellable(
        &local_records,
        expected_brain_id,
        LocalOnlyResolver {
            local_device: local_device.clone(),
            local_key_id: local_signing_key_id.clone(),
            local_key,
        },
        &mut cancelled,
    )
    .map_err(|error| match error {
        cryptex_events::CancellableSecurityHistoryError::Cancelled => TrustHold::Cancelled,
        cryptex_events::CancellableSecurityHistoryError::InvalidRecord(_) => {
            TrustHold::MissingOrUnverifiableHistory
        }
    })?;
    if cancelled() {
        return Err(TrustHold::Cancelled);
    }
    let peers = derive_local_approval_roster(
        &local_history,
        &local_device,
        &local_signing_key_id,
        &local_public_key,
    )?;
    let history = verify_security_history_cancellable(
        raw_records,
        expected_brain_id,
        LocalAnchorResolver {
            local_device: local_device.clone(),
            local_key_id: local_signing_key_id.clone(),
            local_key,
            peers,
        },
        &mut cancelled,
    )
    .map_err(|error| match error {
        cryptex_events::CancellableSecurityHistoryError::Cancelled => TrustHold::Cancelled,
        cryptex_events::CancellableSecurityHistoryError::InvalidRecord(_) => {
            TrustHold::MissingOrUnverifiableHistory
        }
    })?;
    if cancelled() {
        return Err(TrustHold::Cancelled);
    }
    let verified = build_verified_trust_history(
        &history,
        local_device,
        &local_signing_key_id,
        &local_public_key,
        VerifiedHistoryState::Complete,
    )?;
    let reduction = reduce_trust_history(&verified);
    if cancelled() {
        return Err(TrustHold::Cancelled);
    }
    Ok((history, reduction, verified))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Edge {
    issuer: DeviceId,
    issuer_seq: u64,
    target: DeviceId,
    cutoff_seq: u64,
}

pub(crate) fn reduce_trust_history(history: &VerifiedTrustHistory) -> TrustReduction {
    match history.state {
        VerifiedHistoryState::Complete => {}
        VerifiedHistoryState::MissingOrUnverifiable => {
            return TrustReduction::Held(TrustHold::MissingOrUnverifiableHistory);
        }
        VerifiedHistoryState::CompetingFork => {
            return TrustReduction::Held(TrustHold::CompetingFork);
        }
    }

    let mut trusted_devices = history.directly_approved_peers.clone();
    trusted_devices.insert(history.local_device.clone());

    let mut nodes = history.known_devices.clone();
    nodes.extend(trusted_devices.iter().cloned());
    for restriction in &history.restrictions {
        nodes.insert(restriction.issuer.clone());
        nodes.insert(restriction.target.clone());
        if restriction.issuer_seq == 0 || restriction.cutoff_seq == 0 && restriction.issuer_seq == 0
        {
            return TrustReduction::Held(TrustHold::InvalidPolicyHistory);
        }
    }

    let mut retirement_caps = BTreeMap::<DeviceId, u64>::new();
    for retirement in &history.self_retirements {
        nodes.insert(retirement.device.clone());
        if retirement.event_seq == 0
            || retirement.last_authorized_seq >= retirement.event_seq
            || retirement_caps
                .insert(retirement.device.clone(), retirement.last_authorized_seq)
                .is_some()
        {
            return TrustReduction::Held(TrustHold::InvalidPolicyHistory);
        }
    }

    let mut edges = Vec::<Edge>::new();
    for restriction in &history.restrictions {
        // Imported peer approvals never create local trust or issuer authority.
        if trusted_devices.contains(&restriction.issuer) {
            edges.push(Edge {
                issuer: restriction.issuer.clone(),
                issuer_seq: restriction.issuer_seq,
                target: restriction.target.clone(),
                cutoff_seq: restriction.cutoff_seq,
            });
        }
    }
    edges.sort_by(edge_order);
    if edges.windows(2).any(|pair| pair[0] == pair[1]) {
        return TrustReduction::Held(TrustHold::InvalidPolicyHistory);
    }

    // A cycle can be filtered only by cutoffs already resolved in upstream
    // condensation components. Recompute SCCs after each affected component;
    // if that component splits but any of its pieces remains cyclic, hold the
    // entire Brain instead of selecting an acyclic subset.
    loop {
        let components = strongly_connected_components(&nodes, &edges);
        let component_of = component_index(&components);
        let order = component_topological_order(&components, &component_of, &edges);

        let Some(component_index) = order
            .iter()
            .copied()
            .find(|index| component_is_cyclic(&components[*index], &edges))
        else {
            let cutoffs = resolve_acyclic_cutoffs(&nodes, &edges, &retirement_caps);
            return TrustReduction::Ready(TrustSnapshot {
                trusted_devices,
                effective_cutoffs: cutoffs,
            });
        };

        let component = &components[component_index];
        let upstream_cutoffs = resolve_upstream_cutoffs(
            component_index,
            &order,
            &components,
            &component_of,
            &edges,
            &retirement_caps,
        );
        let mut filter_thresholds = BTreeMap::new();
        for device in component {
            let mut threshold = retirement_caps.get(device).copied().unwrap_or(u64::MAX);
            for edge in &edges {
                if edge.target == *device
                    && component_of.get(&edge.issuer) != Some(&component_index)
                    && upstream_cutoffs
                        .get(&edge.issuer)
                        .is_some_and(|cutoff| edge.issuer_seq <= *cutoff)
                {
                    threshold = threshold.min(edge.cutoff_seq);
                }
            }
            filter_thresholds.insert(device.clone(), threshold);
        }

        let before = edges.len();
        edges.retain(|edge| {
            filter_thresholds
                .get(&edge.issuer)
                .is_none_or(|threshold| edge.issuer_seq <= *threshold)
        });
        if edges.len() == before {
            return TrustReduction::Held(TrustHold::UnresolvedRestrictionCycle);
        }

        let affected_nodes: BTreeSet<_> = component.iter().cloned().collect();
        let affected = strongly_connected_components(&affected_nodes, &edges);
        if affected
            .iter()
            .any(|piece| component_is_cyclic(piece, &edges))
        {
            return TrustReduction::Held(TrustHold::UnresolvedRestrictionCycle);
        }
    }
}

fn edge_order(left: &Edge, right: &Edge) -> std::cmp::Ordering {
    left.issuer
        .as_str()
        .as_bytes()
        .cmp(right.issuer.as_str().as_bytes())
        .then_with(|| left.issuer_seq.cmp(&right.issuer_seq))
        .then_with(|| {
            left.target
                .as_str()
                .as_bytes()
                .cmp(right.target.as_str().as_bytes())
        })
        .then_with(|| left.cutoff_seq.cmp(&right.cutoff_seq))
}

fn component_index(components: &[Vec<DeviceId>]) -> BTreeMap<DeviceId, usize> {
    components
        .iter()
        .enumerate()
        .flat_map(|(index, members)| members.iter().cloned().map(move |member| (member, index)))
        .collect()
}

fn component_topological_order(
    components: &[Vec<DeviceId>],
    component_of: &BTreeMap<DeviceId, usize>,
    edges: &[Edge],
) -> Vec<usize> {
    let mut outgoing = vec![BTreeSet::<usize>::new(); components.len()];
    let mut indegree = vec![0usize; components.len()];
    for edge in edges {
        let (Some(&from), Some(&to)) = (
            component_of.get(&edge.issuer),
            component_of.get(&edge.target),
        ) else {
            continue;
        };
        if from != to && outgoing[from].insert(to) {
            indegree[to] += 1;
        }
    }
    let mut ready = BTreeSet::<(Vec<String>, usize)>::new();
    for (index, degree) in indegree.iter().enumerate() {
        if *degree == 0 {
            ready.insert((component_sort_key(&components[index]), index));
        }
    }
    let mut order = Vec::with_capacity(components.len());
    while let Some((key, index)) = ready.pop_first() {
        let _ = key;
        order.push(index);
        for target in &outgoing[index] {
            indegree[*target] -= 1;
            if indegree[*target] == 0 {
                ready.insert((component_sort_key(&components[*target]), *target));
            }
        }
    }
    order
}

fn component_sort_key(component: &[DeviceId]) -> Vec<String> {
    let mut key: Vec<_> = component
        .iter()
        .map(|device| device.as_str().to_owned())
        .collect();
    key.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    key
}

fn component_is_cyclic(component: &[DeviceId], edges: &[Edge]) -> bool {
    component.len() > 1
        || component.first().is_some_and(|device| {
            edges
                .iter()
                .any(|edge| edge.issuer == *device && edge.target == *device)
        })
}

fn resolve_acyclic_cutoffs(
    nodes: &BTreeSet<DeviceId>,
    edges: &[Edge],
    retirement_caps: &BTreeMap<DeviceId, u64>,
) -> BTreeMap<DeviceId, u64> {
    let components = strongly_connected_components(nodes, edges);
    let component_of = component_index(&components);
    let order = component_topological_order(&components, &component_of, edges);
    let mut cutoffs = BTreeMap::new();
    for component in order {
        for device in &components[component] {
            let mut cutoff = retirement_caps.get(device).copied().unwrap_or(u64::MAX);
            for edge in edges.iter().filter(|edge| edge.target == *device) {
                if cutoffs
                    .get(&edge.issuer)
                    .is_some_and(|issuer_cutoff| edge.issuer_seq <= *issuer_cutoff)
                {
                    cutoff = cutoff.min(edge.cutoff_seq);
                }
            }
            cutoffs.insert(device.clone(), cutoff);
        }
    }
    cutoffs
}

fn resolve_upstream_cutoffs(
    target_component: usize,
    order: &[usize],
    components: &[Vec<DeviceId>],
    component_of: &BTreeMap<DeviceId, usize>,
    edges: &[Edge],
    retirement_caps: &BTreeMap<DeviceId, u64>,
) -> BTreeMap<DeviceId, u64> {
    let mut cutoffs = BTreeMap::new();
    for component in order
        .iter()
        .copied()
        .take_while(|index| *index != target_component)
    {
        for device in &components[component] {
            let mut cutoff = retirement_caps.get(device).copied().unwrap_or(u64::MAX);
            for edge in edges.iter().filter(|edge| edge.target == *device) {
                if component_of.get(&edge.issuer) == Some(&component)
                    || cutoffs
                        .get(&edge.issuer)
                        .is_some_and(|issuer_cutoff| edge.issuer_seq <= *issuer_cutoff)
                {
                    cutoff = cutoff.min(edge.cutoff_seq);
                }
            }
            cutoffs.insert(device.clone(), cutoff);
        }
    }
    cutoffs
}

fn strongly_connected_components(nodes: &BTreeSet<DeviceId>, edges: &[Edge]) -> Vec<Vec<DeviceId>> {
    struct Tarjan<'a> {
        adjacency: BTreeMap<&'a DeviceId, Vec<&'a DeviceId>>,
        next_index: usize,
        indices: BTreeMap<&'a DeviceId, usize>,
        lowlinks: BTreeMap<&'a DeviceId, usize>,
        stack: Vec<&'a DeviceId>,
        on_stack: BTreeSet<&'a DeviceId>,
        components: Vec<Vec<DeviceId>>,
    }

    fn visit<'a>(node: &'a DeviceId, state: &mut Tarjan<'a>) {
        let index = state.next_index;
        state.next_index += 1;
        state.indices.insert(node, index);
        state.lowlinks.insert(node, index);
        state.stack.push(node);
        state.on_stack.insert(node);

        let neighbors = state.adjacency.get(node).cloned().unwrap_or_default();
        for neighbor in neighbors {
            if !state.indices.contains_key(neighbor) {
                visit(neighbor, state);
                let neighbor_lowlink = state.lowlinks[neighbor];
                let node_lowlink = state.lowlinks[node];
                state
                    .lowlinks
                    .insert(node, node_lowlink.min(neighbor_lowlink));
            } else if state.on_stack.contains(neighbor) {
                let neighbor_index = state.indices[neighbor];
                let node_lowlink = state.lowlinks[node];
                state
                    .lowlinks
                    .insert(node, node_lowlink.min(neighbor_index));
            }
        }

        if state.lowlinks[node] == state.indices[node] {
            let mut component = Vec::new();
            while let Some(member) = state.stack.pop() {
                state.on_stack.remove(member);
                component.push(member.clone());
                if member == node {
                    break;
                }
            }
            component.sort_by(|left: &DeviceId, right: &DeviceId| {
                left.as_str().as_bytes().cmp(right.as_str().as_bytes())
            });
            state.components.push(component);
        }
    }

    let mut adjacency = BTreeMap::new();
    for node in nodes {
        adjacency.insert(node, Vec::new());
    }
    for edge in edges {
        adjacency
            .entry(&edge.issuer)
            .or_insert_with(Vec::new)
            .push(&edge.target);
    }
    for neighbors in adjacency.values_mut() {
        neighbors.sort_by(|left, right| left.as_str().as_bytes().cmp(right.as_str().as_bytes()));
        neighbors.dedup();
    }

    let mut state = Tarjan {
        adjacency,
        next_index: 0,
        indices: BTreeMap::new(),
        lowlinks: BTreeMap::new(),
        stack: Vec::new(),
        on_stack: BTreeSet::new(),
        components: Vec::new(),
    };
    for node in nodes {
        if !state.indices.contains_key(node) {
            visit(node, &mut state);
        }
    }
    state
        .components
        .sort_by_key(|component| component_sort_key(component));
    state.components
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
    use serde_json::Value;

    use super::*;
    use cryptex_events::{
        Classification, ContentHashHex, DeviceSeq, EventPayload, EventType, MemoryNotePayload,
        SignableEvent, SignatureHex, TrustedDaemonKeyResolver, accept_event,
        signing_key_id_from_verifying_key, verify_security_history,
    };

    const BRAIN: &str = "22222222-2222-4222-8222-222222222222";
    const TEST_SEED: [u8; 32] = [0x63; 32];

    fn device(value: &str) -> DeviceId {
        DeviceId::new(value).unwrap()
    }

    struct TestResolver(VerifyingKey);

    impl TrustedDaemonKeyResolver for TestResolver {
        fn trusted_daemon_key(&mut self, _device_id: &str, key_id: &str) -> Option<VerifyingKey> {
            let expected = signing_key_id_from_verifying_key(&self.0).ok()?;
            (key_id == expected.as_str()).then_some(self.0)
        }
    }

    struct TwoDeviceResolver {
        local: VerifyingKey,
        peer: VerifyingKey,
        local_id: String,
        peer_id: String,
    }

    impl TrustedDaemonKeyResolver for TwoDeviceResolver {
        fn trusted_daemon_key(&mut self, device_id: &str, key_id: &str) -> Option<VerifyingKey> {
            if device_id == self.local_id
                && signing_key_id_from_verifying_key(&self.local)
                    .ok()?
                    .as_str()
                    == key_id
            {
                return Some(self.local);
            }
            if device_id == self.peer_id
                && signing_key_id_from_verifying_key(&self.peer).ok()?.as_str() == key_id
            {
                return Some(self.peer);
            }
            None
        }
    }

    fn make_v1(device_id: &str, seq: u64, prev_hash: &str, key: &SigningKey) -> Vec<u8> {
        let device_prefix = device_id.bytes().fold(0_u32, |value, byte| {
            value.wrapping_mul(31).wrapping_add(u32::from(byte))
        });
        let event = accept_event(
            SignableEvent {
                schema_version: 1,
                event_id: format!("{device_prefix:08x}-aaaa-4aaa-8aaa-{seq:012x}"),
                brain_id: BRAIN.into(),
                device_id: device_id.into(),
                device_seq: DeviceSeq::from_u64(seq).unwrap(),
                created_at: "2026-10-29T00:00:00Z".into(),
                event_type: EventType::MemoryNote,
                classification: Classification::Public,
                signing_key_id: signing_key_id_from_verifying_key(&key.verifying_key()).unwrap(),
                payload: EventPayload::MemoryNote(MemoryNotePayload::new("synthetic").unwrap()),
                prev_content_hash: ContentHashHex::parse(prev_hash).unwrap(),
            },
            key,
        )
        .unwrap();
        serde_json_canonicalizer::to_vec(&event).unwrap()
    }

    fn make_v3(
        device_id: &str,
        event_id: &str,
        seq: u64,
        prev_hash: &str,
        event_type: &str,
        payload: Value,
        key: &SigningKey,
    ) -> Vec<u8> {
        let key_id = signing_key_id_from_verifying_key(&key.verifying_key()).unwrap();
        let mut record = serde_json::json!({
            "schema_version": 3,
            "event_id": event_id,
            "brain_id": BRAIN,
            "device_id": device_id,
            "device_seq": seq.to_string(),
            "created_at": "2026-10-29T00:00:01Z",
            "event_type": event_type,
            "classification": "internal",
            "signing_key_id": key_id.as_str(),
            "payload": payload,
            "prev_content_hash": prev_hash,
            "content_hash": "00".repeat(32),
            "signature": "00".repeat(64)
        });
        record.as_object_mut().unwrap().remove("content_hash");
        record.as_object_mut().unwrap().remove("signature");
        let signable = serde_json_canonicalizer::to_vec(&record).unwrap();
        let mut hasher = blake3::Hasher::new();
        hasher.update(cryptex_events::SECURITY_V3_CONTENT_HASH_DOMAIN);
        hasher.update(&[0]);
        hasher.update(&signable);
        let hash = *hasher.finalize().as_bytes();
        let signature =
            key.sign(&[cryptex_events::SECURITY_V3_SIGNATURE_DOMAIN, &[0], &hash].concat());
        let object = record.as_object_mut().unwrap();
        object.insert(
            "content_hash".into(),
            Value::String(ContentHashHex::from_bytes(&hash).unwrap().as_str().into()),
        );
        object.insert(
            "signature".into(),
            Value::String(
                SignatureHex::from_bytes(&signature.to_bytes())
                    .unwrap()
                    .as_str()
                    .into(),
            ),
        );
        serde_json_canonicalizer::to_vec(&record).unwrap()
    }

    fn verified_history(records: &[&[u8]], key: &SigningKey) -> VerifiedSecurityHistory {
        verify_security_history(records, BRAIN, TestResolver(key.verifying_key())).unwrap()
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn bootstrap_hash(domain: &[u8], value: &Value) -> String {
        let canonical = serde_json_canonicalizer::to_vec(value).unwrap();
        let mut hasher = blake3::Hasher::new();
        hasher.update(domain);
        hasher.update(&[0]);
        hasher.update(&canonical);
        hex(hasher.finalize().as_bytes())
    }

    fn sign_bootstrap_message(mut value: Value, key: &SigningKey) -> Value {
        value.as_object_mut().unwrap().remove("signature");
        let canonical = serde_json_canonicalizer::to_vec(&value).unwrap();
        let mut preimage = b"cryptex-peer-bootstrap-sig-v1".to_vec();
        preimage.push(0);
        preimage.extend_from_slice(&canonical);
        let signature = key.sign(&preimage);
        value.as_object_mut().unwrap().insert(
            "signature".into(),
            Value::String(hex(&signature.to_bytes())),
        );
        value
    }

    fn bootstrap_identity(id: &str, key: &SigningKey) -> Value {
        serde_json::json!({
            "device_id": id,
            "signing_key_id": signing_key_id_from_verifying_key(&key.verifying_key()).unwrap().as_str(),
            "public_key": hex(&key.verifying_key().to_bytes())
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn bootstrap_message(
        kind: &str,
        transaction_id: &str,
        sender_id: &str,
        sender_key: &SigningKey,
        sender_transport_id: &str,
        peer: Option<(&str, &SigningKey, &str)>,
        previous_hash: &str,
        payload: Value,
    ) -> Value {
        let sender_key_id = signing_key_id_from_verifying_key(&sender_key.verifying_key()).unwrap();
        let (peer_id, peer_key_id, peer_public_key, peer_transport_id) = match peer {
            Some((id, key, transport)) => (
                Value::String(id.into()),
                Value::String(
                    signing_key_id_from_verifying_key(&key.verifying_key())
                        .unwrap()
                        .as_str()
                        .into(),
                ),
                Value::String(hex(&key.verifying_key().to_bytes())),
                Value::String(transport.into()),
            ),
            None => (Value::Null, Value::Null, Value::Null, Value::Null),
        };
        serde_json::json!({
            "protocol_version": 1,
            "message_type": kind,
            "transaction_id": transaction_id,
            "brain_id": BRAIN,
            "sender_device_id": sender_id,
            "sender_signing_key_id": sender_key_id.as_str(),
            "sender_public_key": hex(&sender_key.verifying_key().to_bytes()),
            "sender_transport_device_id": sender_transport_id,
            "peer_device_id": peer_id,
            "peer_signing_key_id": peer_key_id,
            "peer_public_key": peer_public_key,
            "peer_transport_device_id": peer_transport_id,
            "previous_hash": previous_hash,
            "payload": payload,
            "signature": "00".repeat(64)
        })
    }

    fn valid_bootstrap_transcript(
        initiator_id: &str,
        initiator_key: &SigningKey,
        responder_id: &str,
        responder_key: &SigningKey,
    ) -> Value {
        const INIT_TRANSPORT: &str =
            "AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA";
        const RESP_TRANSPORT: &str =
            "BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB";
        const ZERO: &str = "0000000000000000000000000000000000000000000000000000000000000000";
        const MESSAGE_DOMAIN: &[u8] = b"cryptex-peer-bootstrap-message-v1";
        const AGREEMENT_DOMAIN: &[u8] = b"cryptex-peer-bootstrap-agreement-v1";
        const TRANSCRIPT_DOMAIN: &[u8] = b"cryptex-peer-bootstrap-transcript-v1";
        let transaction_id = "ab".repeat(32);
        let offer = sign_bootstrap_message(
            bootstrap_message(
                "offer",
                &transaction_id,
                initiator_id,
                initiator_key,
                INIT_TRANSPORT,
                None,
                ZERO,
                serde_json::json!({"initiator_nonce":"11".repeat(32)}),
            ),
            initiator_key,
        );
        let offer_hash = bootstrap_hash(MESSAGE_DOMAIN, &offer);
        let proof = sign_bootstrap_message(
            bootstrap_message(
                "proof",
                &transaction_id,
                responder_id,
                responder_key,
                RESP_TRANSPORT,
                Some((initiator_id, initiator_key, INIT_TRANSPORT)),
                &offer_hash,
                serde_json::json!({
                    "offer_hash": offer_hash,
                    "responder_nonce": "22".repeat(32)
                }),
            ),
            responder_key,
        );
        let proof_hash = bootstrap_hash(MESSAGE_DOMAIN, &proof);
        let agreement_hash = bootstrap_hash(
            AGREEMENT_DOMAIN,
            &serde_json::json!({
                "offer_hash": offer_hash,
                "proof_hash": proof_hash,
                "brain_id": BRAIN,
                "initiator_identity": bootstrap_identity(initiator_id, initiator_key),
                "responder_identity": bootstrap_identity(responder_id, responder_key),
                "transport_association": {
                    "initiator_transport_device_id": INIT_TRANSPORT,
                    "responder_transport_device_id": RESP_TRANSPORT
                }
            }),
        );
        let initiator_approval = sign_bootstrap_message(
            bootstrap_message(
                "approval",
                &transaction_id,
                initiator_id,
                initiator_key,
                INIT_TRANSPORT,
                Some((responder_id, responder_key, RESP_TRANSPORT)),
                &proof_hash,
                serde_json::json!({"agreement_hash": agreement_hash, "role":"initiator"}),
            ),
            initiator_key,
        );
        let initiator_approval_hash = bootstrap_hash(MESSAGE_DOMAIN, &initiator_approval);
        let responder_approval = sign_bootstrap_message(
            bootstrap_message(
                "approval",
                &transaction_id,
                responder_id,
                responder_key,
                RESP_TRANSPORT,
                Some((initiator_id, initiator_key, INIT_TRANSPORT)),
                &initiator_approval_hash,
                serde_json::json!({"agreement_hash": agreement_hash, "role":"responder"}),
            ),
            responder_key,
        );
        let messages = vec![offer, proof, initiator_approval, responder_approval];
        let transcript_hash = bootstrap_hash(TRANSCRIPT_DOMAIN, &Value::Array(messages.clone()));
        let _ = transcript_hash;
        Value::Array(messages)
    }

    fn history(
        peers: &[&str],
        restrictions: &[(&str, u64, &str, u64)],
        retirements: &[(&str, u64, u64)],
    ) -> VerifiedTrustHistory {
        let local_device = device("L");
        let directly_approved_peers: BTreeSet<_> = peers.iter().map(|id| device(id)).collect();
        let mut known_devices = directly_approved_peers.clone();
        known_devices.insert(local_device.clone());
        let restrictions: Vec<_> = restrictions
            .iter()
            .map(|(issuer, issuer_seq, target, cutoff_seq)| {
                let issuer = device(issuer);
                let target = device(target);
                known_devices.insert(issuer.clone());
                known_devices.insert(target.clone());
                VerifiedRestriction {
                    issuer,
                    issuer_seq: *issuer_seq,
                    target,
                    cutoff_seq: *cutoff_seq,
                }
            })
            .collect();
        let self_retirements = retirements
            .iter()
            .map(|(id, event_seq, last_authorized_seq)| {
                let device = device(id);
                known_devices.insert(device.clone());
                VerifiedSelfRetirement {
                    device,
                    event_seq: *event_seq,
                    last_authorized_seq: *last_authorized_seq,
                }
            })
            .collect();
        VerifiedTrustHistory {
            local_device,
            local_signing_key_id: signing_key_id_from_verifying_key(
                &SigningKey::from_bytes(&TEST_SEED).verifying_key(),
            )
            .unwrap(),
            local_public_key: SigningKey::from_bytes(&TEST_SEED)
                .verifying_key()
                .to_bytes(),
            known_devices,
            directly_approved_peers,
            approved_peer_bindings: BTreeMap::new(),
            restrictions,
            self_retirements,
            state: VerifiedHistoryState::Complete,
        }
    }

    fn cutoff(result: &TrustReduction, id: &str) -> u64 {
        let TrustReduction::Ready(snapshot) = result else {
            panic!("expected ready reduction, got {result:?}");
        };
        snapshot.effective_cutoffs[&device(id)]
    }

    #[test]
    fn verified_history_builder_binds_restriction_to_exact_target_record() {
        let key = SigningKey::from_bytes(&TEST_SEED);
        let target = make_v1("target", 1, ZERO_CONTENT_HASH_HEX, &key);
        let target_record: Value = serde_json::from_slice(&target).unwrap();
        let target_hash = target_record["content_hash"].as_str().unwrap();
        let target_key_id = signing_key_id_from_verifying_key(&key.verifying_key()).unwrap();
        let target_public_key = key
            .verifying_key()
            .to_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let restriction = make_v3(
            "local",
            "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            1,
            ZERO_CONTENT_HASH_HEX,
            "security.device_restricted",
            serde_json::json!({
                "transaction_id": "11".repeat(32),
                "target_device_id": "target",
                "target_signing_key_id": target_key_id.as_str(),
                "target_public_key": target_public_key.clone(),
                "reason": "retired",
                "cutoff_seq": "1",
                "cutoff_content_hash": target_hash
            }),
            &key,
        );
        let verified = verified_history(&[restriction.as_slice(), target.as_slice()], &key);
        let derived = build_verified_trust_history(
            &verified,
            device("local"),
            &signing_key_id_from_verifying_key(&key.verifying_key()).unwrap(),
            &key.verifying_key().to_bytes(),
            VerifiedHistoryState::Complete,
        )
        .unwrap();
        let reduced = reduce_trust_history(&derived);
        assert_eq!(cutoff(&reduced, "target"), 1);

        let mismatched = make_v3(
            "local",
            "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
            1,
            ZERO_CONTENT_HASH_HEX,
            "security.device_restricted",
            serde_json::json!({
                "transaction_id": "22".repeat(32),
                "target_device_id": "target",
                "target_signing_key_id": target_key_id.as_str(),
                "target_public_key": target_public_key,
                "reason": "retired",
                "cutoff_seq": "1",
                "cutoff_content_hash": "aa".repeat(32)
            }),
            &key,
        );
        let mismatched_history =
            verified_history(&[mismatched.as_slice(), target.as_slice()], &key);
        assert_eq!(
            build_verified_trust_history(
                &mismatched_history,
                device("local"),
                &signing_key_id_from_verifying_key(&key.verifying_key()).unwrap(),
                &key.verifying_key().to_bytes(),
                VerifiedHistoryState::Complete,
            ),
            Err(TrustHold::InvalidPolicyHistory)
        );
    }

    #[test]
    fn only_a_verified_local_peer_approval_anchor_builds_the_direct_roster() {
        let local_key = SigningKey::from_bytes(&TEST_SEED);
        let peer_key = SigningKey::from_bytes(&[0x27; 32]);
        let transcript = valid_bootstrap_transcript("local", &local_key, "peer", &peer_key);
        let transaction_id = "ab".repeat(32);
        let local_anchor = make_v3(
            "local",
            "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            1,
            ZERO_CONTENT_HASH_HEX,
            "security.peer_approved",
            serde_json::json!({
                "transaction_id": transaction_id,
                "transcript": transcript
            }),
            &local_key,
        );
        let local_history = verify_security_history(
            &[local_anchor.as_slice()],
            BRAIN,
            TwoDeviceResolver {
                local: local_key.verifying_key(),
                peer: peer_key.verifying_key(),
                local_id: "local".into(),
                peer_id: "peer".into(),
            },
        )
        .unwrap();
        let local_signing_key_id =
            signing_key_id_from_verifying_key(&local_key.verifying_key()).unwrap();
        let derived = build_verified_trust_history(
            &local_history,
            device("local"),
            &local_signing_key_id,
            &local_key.verifying_key().to_bytes(),
            VerifiedHistoryState::Complete,
        )
        .unwrap();
        assert_eq!(
            derived.directly_approved_peers,
            BTreeSet::from([device("peer")])
        );
        let binding = &derived.approved_peer_bindings[&device("peer")];
        assert_eq!(
            binding.signing_key_id,
            signing_key_id_from_verifying_key(&peer_key.verifying_key()).unwrap()
        );
        assert_eq!(binding.public_key, peer_key.verifying_key().to_bytes());
        assert_eq!(
            binding.local_transport_device_id,
            "AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA"
        );
        assert_eq!(
            binding.transport_device_id,
            "BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB"
        );

        let peer_anchor = make_v3(
            "peer",
            "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
            1,
            ZERO_CONTENT_HASH_HEX,
            "security.peer_approved",
            serde_json::json!({
                "transaction_id": transaction_id,
                "transcript": valid_bootstrap_transcript("local", &local_key, "peer", &peer_key)
            }),
            &peer_key,
        );
        let peer_only_history = verify_security_history(
            &[peer_anchor.as_slice()],
            BRAIN,
            TwoDeviceResolver {
                local: local_key.verifying_key(),
                peer: peer_key.verifying_key(),
                local_id: "local".into(),
                peer_id: "peer".into(),
            },
        )
        .unwrap();
        let peer_only_derived = build_verified_trust_history(
            &peer_only_history,
            device("local"),
            &local_signing_key_id,
            &local_key.verifying_key().to_bytes(),
            VerifiedHistoryState::Complete,
        )
        .unwrap();
        assert!(peer_only_derived.directly_approved_peers.is_empty());
    }

    #[test]
    fn peer_policy_is_authorized_independent_of_device_sort_order() {
        let local_key = SigningKey::from_bytes(&TEST_SEED);
        let peer_key = SigningKey::from_bytes(&[0x31; 32]);
        let target_key = SigningKey::from_bytes(&[0x32; 32]);
        let transcript = valid_bootstrap_transcript("zlocal", &local_key, "ape", &peer_key);
        let local_anchor = make_v3(
            "zlocal",
            "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            1,
            ZERO_CONTENT_HASH_HEX,
            "security.peer_approved",
            serde_json::json!({
                "transaction_id": "ab".repeat(32),
                "transcript": transcript
            }),
            &local_key,
        );
        let peer_restriction = make_v3(
            "ape",
            "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
            1,
            ZERO_CONTENT_HASH_HEX,
            "security.device_restricted",
            serde_json::json!({
                "transaction_id": "cd".repeat(32),
                "target_device_id": "target",
                "target_signing_key_id": signing_key_id_from_verifying_key(&target_key.verifying_key()).unwrap().as_str(),
                "target_public_key": hex(&target_key.verifying_key().to_bytes()),
                "reason": "retired",
                "cutoff_seq": "0",
                "cutoff_content_hash": ZERO_CONTENT_HASH_HEX
            }),
            &peer_key,
        );
        let history = verify_security_history(
            &[local_anchor.as_slice(), peer_restriction.as_slice()],
            BRAIN,
            TwoDeviceResolver {
                local: local_key.verifying_key(),
                peer: peer_key.verifying_key(),
                local_id: "zlocal".into(),
                peer_id: "ape".into(),
            },
        )
        .unwrap();
        assert_eq!(history.records()[0].device_id(), "ape");

        let derived = build_verified_trust_history(
            &history,
            device("zlocal"),
            &signing_key_id_from_verifying_key(&local_key.verifying_key()).unwrap(),
            &local_key.verifying_key().to_bytes(),
            VerifiedHistoryState::Complete,
        )
        .unwrap();
        assert_eq!(derived.restrictions.len(), 1);
        assert_eq!(derived.restrictions[0].issuer, device("ape"));

        let (anchored_history, anchored_reduction) = verify_history_from_local_anchors(
            &[local_anchor.as_slice(), peer_restriction.as_slice()],
            BRAIN,
            device("zlocal"),
            signing_key_id_from_verifying_key(&local_key.verifying_key()).unwrap(),
            local_key.verifying_key().to_bytes(),
        )
        .unwrap();
        assert_eq!(anchored_history.records().len(), 2);
        assert!(matches!(anchored_reduction, TrustReduction::Ready(_)));

        let rogue_key = SigningKey::from_bytes(&[0x33; 32]);
        let rogue_record = make_v3(
            "rogue",
            "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
            1,
            ZERO_CONTENT_HASH_HEX,
            "security.device_self_retired",
            serde_json::json!({
                "transaction_id": "ef".repeat(32),
                "last_authorized_seq": "0",
                "last_authorized_content_hash": ZERO_CONTENT_HASH_HEX
            }),
            &rogue_key,
        );
        assert!(
            verify_history_from_local_anchors(
                &[
                    local_anchor.as_slice(),
                    peer_restriction.as_slice(),
                    rogue_record.as_slice()
                ],
                BRAIN,
                device("zlocal"),
                signing_key_id_from_verifying_key(&local_key.verifying_key()).unwrap(),
                local_key.verifying_key().to_bytes(),
            )
            .is_err()
        );
    }

    #[test]
    fn verified_history_builder_enforces_terminal_self_retirement_boundary() {
        let key = SigningKey::from_bytes(&TEST_SEED);
        let prior = make_v1("local", 1, ZERO_CONTENT_HASH_HEX, &key);
        let prior_record: Value = serde_json::from_slice(&prior).unwrap();
        let prior_hash = prior_record["content_hash"].as_str().unwrap();
        let retirement = make_v3(
            "local",
            "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            3,
            prior_hash,
            "security.device_self_retired",
            serde_json::json!({
                "transaction_id": "33".repeat(32),
                "last_authorized_seq": "1",
                "last_authorized_content_hash": prior_hash
            }),
            &key,
        );
        let verified = verified_history(&[retirement.as_slice(), prior.as_slice()], &key);
        let derived = build_verified_trust_history(
            &verified,
            device("local"),
            &signing_key_id_from_verifying_key(&key.verifying_key()).unwrap(),
            &key.verifying_key().to_bytes(),
            VerifiedHistoryState::Complete,
        )
        .unwrap();
        assert_eq!(cutoff(&reduce_trust_history(&derived), "local"), 1);

        let retirement_record: Value = serde_json::from_slice(&retirement).unwrap();
        let retirement_hash = retirement_record["content_hash"].as_str().unwrap();
        let later = make_v1("local", 4, retirement_hash, &key);
        let after_retirement = verified_history(
            &[prior.as_slice(), retirement.as_slice(), later.as_slice()],
            &key,
        );
        assert_eq!(
            build_verified_trust_history(
                &after_retirement,
                device("local"),
                &signing_key_id_from_verifying_key(&key.verifying_key()).unwrap(),
                &key.verifying_key().to_bytes(),
                VerifiedHistoryState::Complete,
            ),
            Err(TrustHold::InvalidPolicyHistory)
        );
    }

    #[test]
    fn eligible_external_breaker_filters_cycle_and_resolves() {
        let input = history(
            &["X", "A", "B"],
            &[("X", 1, "A", 2), ("A", 10, "B", 8), ("B", 1, "A", 9)],
            &[],
        );
        let result = reduce_trust_history(&input);
        assert_eq!(cutoff(&result, "A"), 2);
        assert_eq!(cutoff(&result, "B"), u64::MAX);
    }

    #[test]
    fn ineligible_external_breaker_does_not_break_mutual_cycle() {
        let input = history(
            &["C", "D", "A", "B"],
            &[
                ("C", 1, "D", 2),
                ("D", 5, "A", 9),
                ("A", 10, "B", u64::MAX),
                ("B", 10, "A", u64::MAX),
            ],
            &[],
        );
        assert_eq!(
            reduce_trust_history(&input),
            TrustReduction::Held(TrustHold::UnresolvedRestrictionCycle)
        );
    }

    #[test]
    fn upstream_unresolved_cycle_holds_before_downstream_reduction() {
        let input = history(
            &["C", "D", "A", "B"],
            &[
                ("C", 1, "D", u64::MAX),
                ("D", 1, "C", u64::MAX),
                ("C", 2, "A", 2),
                ("A", 10, "B", u64::MAX),
                ("B", 10, "A", u64::MAX),
            ],
            &[],
        );
        assert_eq!(
            reduce_trust_history(&input),
            TrustReduction::Held(TrustHold::UnresolvedRestrictionCycle)
        );
    }

    #[test]
    fn self_retirement_filters_post_retirement_edge_inside_cycle() {
        let input = history(
            &["A", "B"],
            &[("A", 3, "B", 1), ("B", 1, "A", 9)],
            &[("A", 4, 2)],
        );
        let result = reduce_trust_history(&input);
        assert_eq!(cutoff(&result, "A"), 2);
        assert_eq!(cutoff(&result, "B"), u64::MAX);
    }

    #[test]
    fn shuffled_verified_union_produces_same_cutoffs() {
        let first = history(
            &["X", "A", "B", "E"],
            &[
                ("X", 1, "A", 2),
                ("A", 10, "B", u64::MAX),
                ("B", 1, "A", 9),
                ("A", 3, "E", 0),
            ],
            &[],
        );
        let mut shuffled = first.clone();
        shuffled.restrictions.reverse();
        let left = reduce_trust_history(&first);
        let right = reduce_trust_history(&shuffled);
        assert_eq!(left, right);
    }

    #[test]
    fn resolved_external_cutoff_filters_only_post_cutoff_issuer_actions() {
        let input = history(
            &["X", "A", "B", "E"],
            &[
                ("X", 1, "A", 2),
                ("A", 10, "B", u64::MAX),
                ("B", 1, "A", u64::MAX),
                ("A", 3, "E", 0),
            ],
            &[],
        );
        let result = reduce_trust_history(&input);
        assert_eq!(cutoff(&result, "A"), 2);
        assert_eq!(cutoff(&result, "E"), u64::MAX);
    }

    #[test]
    fn partial_cycle_after_scc_filter_holds_brain() {
        let input = history(
            &["X", "A", "B", "C"],
            &[
                ("X", 1, "A", 2),
                ("A", 10, "B", u64::MAX),
                ("B", 1, "C", u64::MAX),
                ("C", 1, "A", u64::MAX),
                ("A", 1, "C", u64::MAX),
            ],
            &[],
        );
        assert_eq!(
            reduce_trust_history(&input),
            TrustReduction::Held(TrustHold::UnresolvedRestrictionCycle)
        );
    }

    #[test]
    fn incomplete_or_forked_history_fails_closed() {
        let mut input = history(&["A"], &[], &[]);
        input.state = VerifiedHistoryState::MissingOrUnverifiable;
        assert_eq!(
            reduce_trust_history(&input),
            TrustReduction::Held(TrustHold::MissingOrUnverifiableHistory)
        );
        input.state = VerifiedHistoryState::CompetingFork;
        assert_eq!(
            reduce_trust_history(&input),
            TrustReduction::Held(TrustHold::CompetingFork)
        );
    }

    #[test]
    fn unapproved_peer_restrictions_never_create_local_cutoffs() {
        let input = history(&["A"], &[("Untrusted", 1, "A", 0)], &[]);
        let result = reduce_trust_history(&input);
        assert_eq!(cutoff(&result, "A"), u64::MAX);
        let TrustReduction::Ready(snapshot) = result else {
            unreachable!();
        };
        assert!(!snapshot.trusted_devices.contains(&device("Untrusted")));
    }
}
