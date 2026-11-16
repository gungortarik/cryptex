use super::*;
use crate::trust_reducer::{TrustReduction, VerifiedRestriction, VerifiedSelfRetirement};
use std::collections::BTreeMap;

const LOCAL: &str = "U2SYIGU-YWZX757-2WPZGCR-DHD7MLH-ZKV7HLJ-EYO6CXE-K3DU3DU-SDFXYAE";
const PEER: &str = "XAAPCAD-GA4SF4J-BNQL3FS-T456WKV-FGLXUKO-CDWHVDW-EYWNRIG-SM5GUAZ";

fn id(value: &str) -> DeviceId {
    DeviceId::new(value).unwrap()
}

fn history() -> VerifiedTrustHistory {
    let key = ed25519_dalek::SigningKey::from_bytes(&[17; 32]).verifying_key();
    let key_id = cryptex_events::signing_key_id_from_verifying_key(&key).unwrap();
    let binding = ApprovedPeerBinding {
        device_id: id("peer"),
        signing_key_id: key_id.clone(),
        public_key: key.to_bytes(),
        fingerprint_sha256: [0; 32],
        local_transport_device_id: LOCAL.into(),
        transport_device_id: PEER.into(),
    };
    // Pure policy fixtures only. Production construction is exclusively through
    // from_snapshot; cryptographic validation belongs to that canonical reader.
    VerifiedTrustHistory {
        local_device: id("local"),
        local_signing_key_id: key_id,
        local_public_key: key.to_bytes(),
        known_devices: BTreeSet::from([id("local"), id("peer")]),
        directly_approved_peers: BTreeSet::from([id("peer")]),
        approved_peer_bindings: BTreeMap::from([(id("peer"), binding)]),
        restrictions: vec![],
        self_retirements: vec![],
        state: VerifiedHistoryState::Complete,
    }
}

fn reduce(history: &VerifiedTrustHistory) -> TrustSnapshot {
    match crate::trust_reducer::reduce_trust_history(history) {
        TrustReduction::Ready(trust) => trust,
        TrustReduction::Held(reason) => panic!("unexpected fixture hold: {reason:?}"),
    }
}

#[test]
fn only_direct_approval_bindings_are_returned_and_empty_is_not_authority() {
    let mut h = history();
    h.known_devices.insert(id("transitive"));
    let trust = reduce(&h);
    let peers = eligible_peers(&h, &trust, LOCAL).unwrap();
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].device_id, id("peer"));
    h.directly_approved_peers.clear();
    h.approved_peer_bindings.clear();
    assert!(eligible_peers(&h, &reduce(&h), LOCAL).unwrap().is_empty());
}

#[test]
fn incomplete_or_mismatched_authority_fails_closed() {
    let h = history();
    let trust = reduce(&h);
    for state in [
        VerifiedHistoryState::MissingOrUnverifiable,
        VerifiedHistoryState::CompetingFork,
    ] {
        let mut changed = h.clone();
        changed.state = state;
        assert_eq!(
            eligible_peers(&changed, &trust, LOCAL).unwrap_err(),
            ExchangePolicyError::HistoryUnavailable
        );
    }
    let mut missing = trust.clone();
    missing.effective_cutoffs.remove(&id("peer"));
    assert_eq!(
        eligible_peers(&h, &missing, LOCAL).unwrap_err(),
        ExchangePolicyError::HistoryUnavailable
    );
    let mut extra = trust.clone();
    extra.trusted_devices.insert(id("transitive"));
    extra.effective_cutoffs.insert(id("transitive"), u64::MAX);
    assert_eq!(
        eligible_peers(&h, &extra, LOCAL).unwrap_err(),
        ExchangePolicyError::InvalidBinding
    );
}

#[test]
fn transport_drift_self_collision_duplicate_and_missing_bindings_refuse() {
    let h = history();
    for value in [
        "invalid",
        LOCAL,
        "u2syigu-YWZX757-2WPZGCR-DHD7MLH-ZKV7HLJ-EYO6CXE-K3DU3DU-SDFXYAE",
    ] {
        let mut changed = h.clone();
        changed
            .approved_peer_bindings
            .get_mut(&id("peer"))
            .unwrap()
            .transport_device_id = value.into();
        assert!(eligible_peers(&changed, &reduce(&changed), LOCAL).is_err());
    }
    assert!(eligible_peers(&h, &reduce(&h), PEER).is_err());
    let mut changed = h.clone();
    changed.approved_peer_bindings.clear();
    assert!(eligible_peers(&changed, &reduce(&changed), LOCAL).is_err());
    let mut changed = h.clone();
    changed
        .approved_peer_bindings
        .get_mut(&id("peer"))
        .unwrap()
        .device_id = id("other");
    assert!(eligible_peers(&changed, &reduce(&changed), LOCAL).is_err());
    let mut changed = h.clone();
    let mut duplicate = changed.approved_peer_bindings[&id("peer")].clone();
    duplicate.device_id = id("other");
    changed.directly_approved_peers.insert(id("other"));
    changed
        .approved_peer_bindings
        .insert(id("other"), duplicate);
    assert!(eligible_peers(&changed, &reduce(&changed), LOCAL).is_err());
}

#[test]
fn explicit_restriction_excludes_peer_even_at_max_cutoff() {
    for cutoff in [0, 1, u64::MAX] {
        let mut h = history();
        h.restrictions.push(VerifiedRestriction {
            issuer: id("local"),
            issuer_seq: 2,
            target: id("peer"),
            cutoff_seq: cutoff,
        });
        assert!(eligible_peers(&h, &reduce(&h), LOCAL).unwrap().is_empty());
        // Restrictions do not excuse a changed retained transport binding.
        h.approved_peer_bindings
            .get_mut(&id("peer"))
            .unwrap()
            .local_transport_device_id = PEER.into();
        assert!(eligible_peers(&h, &reduce(&h), LOCAL).is_err());
    }
}

#[test]
fn retirement_and_local_restriction_stop_new_transport_eligibility() {
    let mut h = history();
    h.self_retirements.push(VerifiedSelfRetirement {
        device: id("peer"),
        event_seq: 3,
        last_authorized_seq: 2,
    });
    assert!(eligible_peers(&h, &reduce(&h), LOCAL).unwrap().is_empty());
    h.self_retirements[0].device = id("local");
    assert_eq!(
        eligible_peers(&h, &reduce(&h), LOCAL).unwrap_err(),
        ExchangePolicyError::LocalRestricted
    );
    let mut h = history();
    h.restrictions.push(VerifiedRestriction {
        issuer: id("peer"),
        issuer_seq: 2,
        target: id("local"),
        cutoff_seq: u64::MAX,
    });
    assert_eq!(
        eligible_peers(&h, &reduce(&h), LOCAL).unwrap_err(),
        ExchangePolicyError::LocalRestricted
    );
}

#[test]
fn untrusted_or_out_of_authority_restriction_cannot_disable_transport() {
    let mut h = history();
    h.restrictions.push(VerifiedRestriction {
        issuer: id("unapproved"),
        issuer_seq: 2,
        target: id("peer"),
        cutoff_seq: 0,
    });
    assert_eq!(eligible_peers(&h, &reduce(&h), LOCAL).unwrap().len(), 1);
    let mut other = h.approved_peer_bindings[&id("peer")].clone();
    other.device_id = id("other");
    other.transport_device_id = ["AAAAAAA"; 8].join("-");
    h.directly_approved_peers.insert(id("other"));
    h.approved_peer_bindings.insert(id("other"), other);
    h.restrictions = vec![
        VerifiedRestriction {
            issuer: id("local"),
            issuer_seq: 1,
            target: id("peer"),
            cutoff_seq: 1,
        },
        VerifiedRestriction {
            issuer: id("peer"),
            issuer_seq: 2,
            target: id("other"),
            cutoff_seq: 0,
        },
    ];
    // Peer was restricted before it could issue this action. Its attempted
    // restriction cannot disable the other directly approved installation.
    let trust = reduce(&h);
    let peers = eligible_peers(&h, &trust, LOCAL).unwrap();
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].device_id, id("other"));
}

#[test]
fn actual_signed_canonical_pairing_is_required_and_bound_to_runtime() {
    use cryptex_events::TrustedDaemonKeyResolver;
    let (source, receiver, negative, store) =
        crate::trusted_import_probe::tests::pair_with_transport(LOCAL, PEER);
    {
        let brain = crate::identity_runtime::load_brain_identity(&source).unwrap();
        let resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
            &source,
            &brain.brain_id,
            &store,
            crate::SecureStoreEnvironment::Dev,
        )
        .unwrap();
        let anchor = resolver.local_trust_anchor().unwrap();
        let snapshot =
            crate::knowledge_materialization::scan_verified_canonical_from_local_anchor_until(
                &source,
                &brain.brain_id,
                &anchor,
                None,
            )
            .unwrap();
        let peers = ExchangePeers::from_snapshot(&snapshot, &source, LOCAL).unwrap();
        assert_eq!(peers.bindings().len(), 1);
        assert_eq!(peers.bindings()[0].transport_device_id, PEER);
        assert!(matches!(
            ExchangePeers::from_snapshot(&snapshot, &receiver, LOCAL),
            Err(ExchangePolicyError::WrongRuntime)
        ));
        assert!(matches!(
            ExchangePeers::from_snapshot(&snapshot, &source, PEER),
            Err(ExchangePolicyError::InvalidBinding)
        ));
    }
    for root in [source, receiver, negative] {
        std::fs::remove_dir_all(root).unwrap();
    }
}
