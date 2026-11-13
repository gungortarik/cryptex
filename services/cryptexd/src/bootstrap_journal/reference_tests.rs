//! Owner display digests fixed by the independent wire reference tool.

use super::ApprovalSnapshot;
use cryptex_events::{BootstrapApprovalRole, verify_bootstrap_approval_request};
use serde_json::Value;

#[test]
fn independent_snapshots_bind_both_pairing_roles_and_owner_operations() {
    let data: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/events/tests/fixtures/m8-protocol-v1.json"
    )))
    .unwrap();
    let raw: Vec<_> = data["bootstrap"]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|message| message["wire"].as_str().unwrap().as_bytes())
        .collect();
    assert_eq!(data["approval_snapshots"].as_array().unwrap().len(), 4);
    for (index, fixture) in data["approval_snapshots"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let expected_operations = [
            "peer_admission",
            "peer_admission",
            "device_restriction",
            "self_retirement",
        ];
        let expected_roles = ["initiator", "responder", "issuer", "issuer"];
        assert_eq!(fixture["snapshot"]["operation"], expected_operations[index]);
        assert_eq!(fixture["snapshot"]["role"], expected_roles[index]);
        let snapshot: ApprovalSnapshot =
            serde_json::from_value(fixture["snapshot"].clone()).unwrap();
        assert_eq!(
            snapshot.digest().unwrap(),
            fixture["digest"].as_str().unwrap()
        );
        if index < 2 {
            let (len, role) = if index == 0 {
                (2, BootstrapApprovalRole::Initiator)
            } else {
                (3, BootstrapApprovalRole::Responder)
            };
            let request = verify_bootstrap_approval_request(&raw[..len], role).unwrap();
            assert_eq!(
                ApprovalSnapshot::from_verified_bootstrap_request(&request).unwrap(),
                snapshot
            );
        }
        let mut changed = snapshot.clone();
        changed.transaction_id = "ef".repeat(32);
        assert_ne!(changed.digest().unwrap(), snapshot.digest().unwrap());
    }
}
