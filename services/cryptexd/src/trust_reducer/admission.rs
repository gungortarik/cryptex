//! Conservative proof of local grant eligibility, never an assumed roster.
//! Provisional signature pins do not escape as admitted authority.

use super::{TrustHold, TrustReduction, VerifiedHistoryState, VerifiedTrustHistory, reduce_authority_history};
use cryptex_core::DeviceId;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Bounds {
    lo: u64,
    hi: u64,
}

pub(super) fn resolve<C: FnMut() -> bool>(
    history: &mut VerifiedTrustHistory,
    first_grants: &BTreeMap<DeviceId, u64>,
    cancelled: &mut C,
) -> Result<(), TrustHold> {
    match history.state {
        VerifiedHistoryState::Complete => {}
        VerifiedHistoryState::MissingOrUnverifiable => return Err(TrustHold::MissingOrUnverifiableHistory),
        VerifiedHistoryState::CompetingFork => return Err(TrustHold::CompetingFork),
    }
    history.known_devices.insert(history.local_device.clone());
    history.known_devices.extend(first_grants.keys().cloned());
    let mut caps = history.known_devices.iter().cloned().map(|device| (device, u64::MAX)).collect::<BTreeMap<_, _>>();
    for retirement in &history.self_retirements {
        let cap = caps.get_mut(&retirement.device).ok_or(TrustHold::InvalidPolicyHistory)?;
        *cap = (*cap).min(retirement.last_authorized_seq);
    }
    let mut bounds = caps.iter().map(|(device, cap)| (device.clone(), Bounds { lo: 0, hi: *cap })).collect::<BTreeMap<_, _>>();
    loop {
        if cancelled() {
            return Err(TrustHold::Cancelled);
        }
        let local = bounds[&history.local_device];
        let mut next = caps.iter().map(|(device, cap)| (device.clone(), Bounds { lo: *cap, hi: *cap })).collect::<BTreeMap<_, _>>();
        for restriction in &history.restrictions {
            if cancelled() {
                return Err(TrustHold::Cancelled);
            }
            let issuer = bounds.get(&restriction.issuer).ok_or(TrustHold::InvalidPolicyHistory)?;
            let is_local = restriction.issuer == history.local_device;
            let grant = first_grants.get(&restriction.issuer);
            let possible = is_local || grant.is_some_and(|seq| *seq <= local.hi);
            let definite = is_local || grant.is_some_and(|seq| *seq <= local.lo);
            let target = next.get_mut(&restriction.target).ok_or(TrustHold::InvalidPolicyHistory)?;
            if possible && restriction.issuer_seq <= issuer.hi {
                target.lo = target.lo.min(restriction.cutoff_seq);
            }
            if definite && restriction.issuer_seq <= issuer.lo {
                target.hi = target.hi.min(restriction.cutoff_seq);
            }
        }
        // Sound intervals can only narrow. Refuse an inconsistent proof rather
        // than selecting a roster or silently relying on an iteration budget.
        for (device, candidate) in &next {
            let prior = bounds[device];
            if candidate.lo < prior.lo || candidate.hi > prior.hi || candidate.lo > candidate.hi {
                return Err(TrustHold::UnresolvedAdmissionDependency);
            }
        }
        if next == bounds {
            break;
        }
        bounds = next;
    }
    let local = bounds[&history.local_device];
    let mut admitted = std::collections::BTreeSet::new();
    for (peer, seq) in first_grants {
        if cancelled() {
            return Err(TrustHold::Cancelled);
        }
        if *seq <= local.lo {
            admitted.insert(peer.clone());
        } else if *seq <= local.hi {
            // Conservative proof may hold even where a unique consistent
            // assignment exists. It never chooses that assignment by iteration.
            return Err(TrustHold::UnresolvedAdmissionDependency);
        }
    }
    history.approved_peer_bindings.retain(|peer, _| admitted.contains(peer));
    history.directly_approved_peers = admitted;
    if cancelled() {
        return Err(TrustHold::Cancelled);
    }
    let result = reduce_authority_history(history);
    if cancelled() {
        return Err(TrustHold::Cancelled);
    }
    if let TrustReduction::Ready(authority) = result {
        let local_cutoff = authority.effective_cutoffs.get(&history.local_device).ok_or(TrustHold::UnresolvedAdmissionDependency)?;
        for (peer, seq) in first_grants {
            if authority.trusted_devices.contains(peer) != (*seq <= *local_cutoff) {
                return Err(TrustHold::UnresolvedAdmissionDependency);
            }
        }
        for (device, interval) in &bounds {
            let cutoff = authority.effective_cutoffs.get(device).ok_or(TrustHold::UnresolvedAdmissionDependency)?;
            if *cutoff < interval.lo || *cutoff > interval.hi {
                return Err(TrustHold::UnresolvedAdmissionDependency);
            }
        }
    }
    // Existing SCC holds remain holds. Numeric interval equality cannot turn
    // an unresolved restriction cycle into authority.
    Ok(())
}
