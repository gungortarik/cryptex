//! Signed histories for owner-selected admission and strict-terminal rules.
//! All keys/content/identities are synthetic and never used by a real profile.
use super::*;
use super::tests::{BRAIN, history as numeric_history, make_v1, make_v3};
use cryptex_events::{BootstrapApprovalRole, signing_key_id_from_verifying_key};
use ed25519_dalek::SigningKey;
use serde_json::{Value, json};

fn id(value: &str) -> DeviceId {
    DeviceId::new(value).unwrap()
}

fn hash(record: &[u8]) -> String {
    serde_json::from_slice::<Value>(record).unwrap()["content_hash"].as_str().unwrap().to_owned()
}

fn policy(author: &str, key: &SigningKey, seq: u64, previous: &str, kind: &str, payload: Value) -> Vec<u8> {
    let prefix = author.bytes().fold(0_u32, |acc, byte| acc.wrapping_mul(31).wrapping_add(u32::from(byte)));
    make_v3(author, &format!("{prefix:08x}-bbbb-4bbb-8bbb-{seq:012x}"), seq, previous, kind, payload, key)
}

fn grant(local: &SigningKey, peer: &str, peer_key: &SigningKey, seq: u64, previous: &str, transaction: u8, transport: &str) -> Vec<u8> {
    let tx = format!("{transaction:02x}").repeat(32);
    let offer = cryptex_events::create_bootstrap_offer(BRAIN, &tx, &id("L"), &["AAAAAAA"; 8].join("-"), &[transaction; 32], local).unwrap();
    let proof = cryptex_events::create_bootstrap_proof(&offer, &id(peer), &[transport; 8].join("-"), &[transaction + 1; 32], peer_key).unwrap();
    let request = cryptex_events::verify_bootstrap_approval_request(&[&offer, &proof], BootstrapApprovalRole::Initiator).unwrap();
    let approval = cryptex_events::sign_bootstrap_approval(&request, local).unwrap();
    let request = cryptex_events::verify_bootstrap_approval_request(&[&offer, &proof, &approval], BootstrapApprovalRole::Responder).unwrap();
    let final_approval = cryptex_events::sign_bootstrap_approval(&request, peer_key).unwrap();
    let transcript = [&offer, &proof, &approval, &final_approval].iter().map(|raw| serde_json::from_slice::<Value>(raw).unwrap()).collect::<Vec<_>>();
    policy("L", local, seq, previous, "security.peer_approved", json!({"transaction_id":tx,"transcript":transcript}))
}

fn restriction(author: &str, key: &SigningKey, seq: u64, previous: &str, target: (&str, &SigningKey), cutoff: u64, boundary: &str) -> Vec<u8> {
    let (target, target_key) = target;
    let public: String = target_key.verifying_key().as_bytes().iter().map(|b| format!("{b:02x}")).collect();
    policy(author, key, seq, previous, "security.device_restricted", json!({
        "transaction_id":format!("{seq:064x}"), "target_device_id":target,
        "target_signing_key_id":signing_key_id_from_verifying_key(&target_key.verifying_key()).unwrap().as_str(),
        "target_public_key":public, "reason":"compromised", "cutoff_seq":cutoff.to_string(), "cutoff_content_hash":boundary
    }))
}

fn verify(records: &[Vec<u8>], local: &SigningKey) -> Result<(VerifiedSecurityHistory, TrustReduction, VerifiedTrustHistory), TrustHold> {
    verify_history_from_local_anchors_with_verified_cancellable(&records.iter().map(Vec::as_slice).collect::<Vec<_>>(), BRAIN, id("L"),
        signing_key_id_from_verifying_key(&local.verifying_key()).unwrap(), local.verifying_key().to_bytes(), || false)
}

#[test]
fn signed_local_grant_above_cutoff_never_admits_its_peer_or_exports_a_resolver_pin() {
    let local = SigningKey::from_bytes(&[0x51; 32]);
    let a = SigningKey::from_bytes(&[0x52; 32]);
    let c = SigningKey::from_bytes(&[0x53; 32]);
    let first = grant(&local, "A", &a, 1, ZERO_CONTENT_HASH_HEX, 1, "BBBBBBB");
    let second = grant(&local, "C", &c, 2, &hash(&first), 2, "CCCCCCC");
    let limit = restriction("A", &a, 1, ZERO_CONTENT_HASH_HEX, ("L", &local), 1, &hash(&first));
    let content = make_v1("C", 1, ZERO_CONTENT_HASH_HEX, &c);
    let mut records = vec![first, second, limit, content];
    let original = records.clone();
    for _ in 0..2 {
        let (history, reduction, verified) = verify(&records, &local).unwrap();
        let TrustReduction::Ready(snapshot) = reduction else { panic!("acyclic independent grant proof must resolve"); };
        assert_eq!(snapshot.trusted_devices, BTreeSet::from([id("L"), id("A")]));
        assert_eq!(snapshot.effective_cutoffs[&id("L")], 1);
        assert_eq!(snapshot.effective_cutoffs[&id("C")], 0);
        assert!(!verified.approved_peer_bindings.contains_key(&id("C")));
        assert!(!verified.directly_approved_peers.contains(&id("C")));
        assert!(verified.key_resolver().trusted_daemon_key("C", signing_key_id_from_verifying_key(&c.verifying_key()).unwrap().as_str()).is_none());
        let cutoffs = snapshot.effective_cutoffs.iter().map(|(device, cutoff)| (device.to_string(), *cutoff)).collect();
        assert!(history.content_records_through_cutoffs(&cutoffs).next().is_none());
        records.reverse();
    }
    assert_eq!(records, original);
}

#[test]
fn a_signed_earlier_equivalent_grant_stays_eligible_after_the_later_grant_is_cut_off() {
    let local = SigningKey::from_bytes(&[0x51; 32]);
    let a = SigningKey::from_bytes(&[0x52; 32]);
    let c = SigningKey::from_bytes(&[0x53; 32]);
    let first = grant(&local, "C", &c, 1, ZERO_CONTENT_HASH_HEX, 1, "CCCCCCC");
    let second = grant(&local, "A", &a, 2, &hash(&first), 2, "BBBBBBB");
    let third = grant(&local, "C", &c, 3, &hash(&second), 3, "CCCCCCC");
    let limit = restriction("A", &a, 1, ZERO_CONTENT_HASH_HEX, ("L", &local), 2, &hash(&second));
    let records = vec![first, second, third, limit, make_v1("C", 1, ZERO_CONTENT_HASH_HEX, &c)];
    let (history, reduction, verified) = verify(&records, &local).unwrap();
    let TrustReduction::Ready(snapshot) = reduction else { panic!("earlier equivalent grant is independently eligible"); };
    assert_eq!(snapshot.effective_cutoffs[&id("L")], 2);
    assert!(snapshot.trusted_devices.contains(&id("C")));
    assert!(verified.approved_peer_bindings.contains_key(&id("C")));
    let cutoffs = snapshot.effective_cutoffs.iter().map(|(device, cutoff)| (device.to_string(), *cutoff)).collect();
    assert_eq!(history.content_records_through_cutoffs(&cutoffs).count(), 1);
}

#[test]
fn signed_direct_indirect_and_conservative_unique_model_dependencies_hold_without_a_roster_guess() {
    let local = SigningKey::from_bytes(&[0x51; 32]);
    let a = SigningKey::from_bytes(&[0x52; 32]);
    let c = SigningKey::from_bytes(&[0x53; 32]);
    let first = grant(&local, "A", &a, 1, ZERO_CONTENT_HASH_HEX, 1, "BBBBBBB");
    let direct = vec![first.clone(), restriction("A", &a, 1, ZERO_CONTENT_HASH_HEX, ("L", &local), 0, ZERO_CONTENT_HASH_HEX)];
    let second = grant(&local, "C", &c, 2, &hash(&first), 2, "CCCCCCC");
    let limit = restriction("A", &a, 1, ZERO_CONTENT_HASH_HEX, ("L", &local), 1, &hash(&first));
    let indirect = vec![first.clone(), second.clone(), limit.clone(), restriction("C", &c, 1, ZERO_CONTENT_HASH_HEX, ("A", &a), 0, ZERO_CONTENT_HASH_HEX)];
    // A unique consistent assignment need not be provable by conservative
    // bounds. Do not choose it by a nonmonotone fixed-point search.
    let unique = vec![first, second, limit, restriction("C", &c, 1, ZERO_CONTENT_HASH_HEX, ("L", &local), 0, ZERO_CONTENT_HASH_HEX)];
    for mut records in [direct, indirect, unique] {
        let original = records.clone();
        for _ in 0..2 {
            assert!(matches!(verify(&records, &local), Err(TrustHold::UnresolvedAdmissionDependency)));
            records.reverse();
        }
        assert_eq!(records, original);
    }
}

#[test]
fn a_signed_ineligible_issuer_cannot_restrict_a_still_eligible_peer() {
    let local = SigningKey::from_bytes(&[0x51; 32]);
    let a = SigningKey::from_bytes(&[0x52; 32]);
    let b = SigningKey::from_bytes(&[0x53; 32]);
    let c = SigningKey::from_bytes(&[0x54; 32]);
    let first = grant(&local, "A", &a, 1, ZERO_CONTENT_HASH_HEX, 1, "BBBBBBB");
    let second = grant(&local, "B", &b, 2, &hash(&first), 2, "CCCCCCC");
    let third = grant(&local, "C", &c, 3, &hash(&second), 3, "DDDDDDD");
    let records = vec![first, second.clone(), third, restriction("A", &a, 1, ZERO_CONTENT_HASH_HEX, ("L", &local), 2, &hash(&second)),
        restriction("C", &c, 1, ZERO_CONTENT_HASH_HEX, ("B", &b), 0, ZERO_CONTENT_HASH_HEX), make_v1("B", 1, ZERO_CONTENT_HASH_HEX, &b)];
    let (_, reduction, verified) = verify(&records, &local).unwrap();
    let TrustReduction::Ready(snapshot) = reduction else { panic!("ineligible returning authority cannot be assumed"); };
    assert_eq!(snapshot.effective_cutoffs[&id("B")], u64::MAX);
    assert!(!verified.approved_peer_bindings.contains_key(&id("C")));
    assert!(!snapshot.trusted_devices.contains(&id("C")));
}

#[test]
fn signed_retirement_gap_content_policy_and_ambiguous_tails_enforce_the_terminal_contract() {
    let local = SigningKey::from_bytes(&[0x51; 32]);
    let peer = SigningKey::from_bytes(&[0x52; 32]);
    let prior = make_v1("L", 1, ZERO_CONTENT_HASH_HEX, &local);
    let retirement = policy("L", &local, 3, &hash(&prior), "security.device_self_retired", json!({"transaction_id":"33".repeat(32), "last_authorized_seq":"1", "last_authorized_content_hash":hash(&prior)}));
    let (history, reduction, _) = verify(&[prior.clone(), retirement.clone()], &local).unwrap();
    let TrustReduction::Ready(snapshot) = reduction else { panic!("valid terminal gap must resolve"); };
    assert_eq!(snapshot.effective_cutoffs[&id("L")], 1);
    let cutoffs = snapshot.effective_cutoffs.iter().map(|(device, cutoff)| (device.to_string(), *cutoff)).collect();
    assert_eq!(history.content_records_through_cutoffs(&cutoffs).count(), 1);
    let tails = [make_v1("L", 4, &hash(&retirement), &local), restriction("L", &local, 4, &hash(&retirement), ("peer", &peer), 0, ZERO_CONTENT_HASH_HEX),
        policy("L", &local, 4, &hash(&retirement), "security.device_self_retired", json!({"transaction_id":"44".repeat(32), "last_authorized_seq":"3", "last_authorized_content_hash":hash(&retirement)}))];
    for tail in tails {
        let records = vec![prior.clone(), retirement.clone(), tail];
        let original = records.clone();
        assert!(matches!(verify(&records, &local), Err(TrustHold::TerminalRetirementTail)));
        assert_eq!(records, original);
    }
    let empty_retirement = policy("L", &local, 1, ZERO_CONTENT_HASH_HEX, "security.device_self_retired", json!({"transaction_id":"55".repeat(32), "last_authorized_seq":"0", "last_authorized_content_hash":ZERO_CONTENT_HASH_HEX}));
    let (_, reduction, _) = verify(&[empty_retirement], &local).unwrap();
    let TrustReduction::Ready(snapshot) = reduction else { panic!("zero-boundary retirement is valid"); };
    assert_eq!(snapshot.effective_cutoffs[&id("L")], 0);
}

#[test]
fn signed_retirement_cannot_name_an_earlier_record_to_manufacture_an_scc_breaker() {
    let local = SigningKey::from_bytes(&[0x51; 32]);
    let a = SigningKey::from_bytes(&[0x52; 32]);
    let b = SigningKey::from_bytes(&[0x53; 32]);
    let first = grant(&local, "A", &a, 1, ZERO_CONTENT_HASH_HEX, 1, "BBBBBBB");
    let second = grant(&local, "B", &b, 2, &hash(&first), 2, "CCCCCCC");
    let a1 = make_v1("A", 1, ZERO_CONTENT_HASH_HEX, &a);
    let b1 = make_v1("B", 1, ZERO_CONTENT_HASH_HEX, &b);
    let a3 = restriction("A", &a, 3, &hash(&a1), ("B", &b), 1, &hash(&b1));
    let b3 = restriction("B", &b, 3, &hash(&b1), ("A", &a), 1, &hash(&a1));
    let bad_retirement = policy("A", &a, 4, &hash(&a3), "security.device_self_retired", json!({"transaction_id":"44".repeat(32),"last_authorized_seq":"1","last_authorized_content_hash":hash(&a1)}));
    let mut records = vec![first, second, a1, b1, a3, b3];
    assert!(matches!(verify(&records, &local).unwrap().1, TrustReduction::Held(TrustHold::UnresolvedRestrictionCycle)));
    records.push(bad_retirement);
    assert!(matches!(verify(&records, &local), Err(TrustHold::InvalidPolicyHistory)));
}

#[test]
fn admission_bound_proof_can_be_cancelled_without_exporting_authority() {
    let mut candidate = numeric_history(&["A"], &[("A", 1, "L", 0)], &[]);
    let grants = BTreeMap::from([(id("A"), 1)]);
    let mut calls = 0;
    assert_eq!(admission::resolve(&mut candidate, &grants, &mut || { calls += 1; calls >= 2 }), Err(TrustHold::Cancelled));
}

#[test]
fn conservative_admission_results_agree_with_every_bounded_consistent_assignment() {
    // Independent finite reference: enumerate numeric assignments and derive
    // their roster directly, rather than reproducing interval propagation.
    // These are arithmetic policy models, not claimed signed-history fixtures.
    let possible = [("A", 1, "L", 1), ("A", 2, "B", 0), ("B", 1, "L", 0),
        ("B", 2, "A", 0), ("L", 3, "A", 1), ("L", 4, "B", 1)];
    let names = ["L", "A", "B"];
    let position = |name: &str| names.iter().position(|item| *item == name).unwrap();
    let values = [0_u64, 1, 2, u64::MAX];
    let grants = BTreeMap::from([(id("A"), 1), (id("B"), 2)]);
    let mut ready_cases = 0;
    let mut held_cases = 0;
    for mask in 0..64 {
        let edges = possible.iter().enumerate().filter_map(|(index, edge)| (mask & (1 << index) != 0).then_some(*edge)).collect::<Vec<_>>();
        let mut candidate = numeric_history(&["A", "B"], &edges, &[]);
        if admission::resolve(&mut candidate, &grants, &mut || false).is_err() {
            held_cases += 1;
            continue;
        }
        let TrustReduction::Ready(result) = reduce_trust_history(&candidate) else {
            held_cases += 1;
            continue;
        };
        ready_cases += 1;
        let mut consistent = 0;
        for l in values { for a in values { for b in values {
            let assignment = [l, a, b];
            let admitted = [true, 1 <= l, 2 <= l];
            let mut computed = [u64::MAX; 3];
            for (issuer, seq, target, cutoff) in &edges {
                let issuer = position(issuer);
                let target = position(target);
                if admitted[issuer] && *seq <= assignment[issuer] {
                    computed[target] = computed[target].min(*cutoff);
                }
            }
            if computed != assignment { continue; }
            consistent += 1;
            for (index, name) in names.iter().enumerate() {
                assert_eq!(result.trusted_devices.contains(&id(name)), admitted[index]);
                if admitted[index] {
                    assert_eq!(result.effective_cutoffs[&id(name)], assignment[index]);
                }
            }
        } } }
        assert!(consistent > 0, "Ready must have a consistent authority assignment");
    }
    assert_eq!(ready_cases + held_cases, 64);
    assert!(ready_cases > 0 && held_cases > 0);
}
