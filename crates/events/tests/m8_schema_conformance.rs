//! Independently re-signed synthetic records. Structural acceptance is never
//! substituted for signature, author binding or transcript verification.
use cryptex_events::{SecurityPayloadV3, verify_bootstrap_transcript_values};
use cryptex_events::security_v3::{read_security_v3_event, verify_peer_approval_protocol_binding};
use ed25519_dalek::{SigningKey, VerifyingKey};
use serde_json::Value;

fn manifest() -> Value {
    serde_json::from_str(include_str!("fixtures/m8-schema-conformance.json")).unwrap()
}

fn key(hex: &str) -> VerifyingKey {
    let bytes = (0..32).map(|index| u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap()).collect::<Vec<_>>();
    VerifyingKey::from_bytes(bytes.as_slice().try_into().unwrap()).unwrap()
}

#[test]
fn independently_signed_schema_cases_match_complete_protocol_validation() {
    let mut events = 0;
    let mut transcripts = 0;
    let mut structural_without_authority = 0;
    let data = manifest();
    for case in data["cases"].as_array().unwrap() {
        let expected = case["runtime_accepts"].as_bool().unwrap();
        let wire = case["wire"].as_str().unwrap();
        let accepted = match case["kind"].as_str().unwrap() {
            "event" => {
                events += 1;
                let author = key(case["public_key"].as_str().unwrap());
                read_security_v3_event(wire.as_bytes(), &author)
                    .and_then(|event| verify_peer_approval_protocol_binding(&event, &author)).is_ok()
            },
            "transcript" => {
                transcripts += 1;
                let messages: Value = serde_json::from_str(wire).unwrap();
                verify_bootstrap_transcript_values(messages.as_array().unwrap()).is_ok()
            },
            "message" => continue, // Actual private message reader has its own unit case.
            _ => panic!("unknown conformance case kind"),
        };
        assert_eq!(accepted, expected, "{}", case["name"].as_str().unwrap());
        if case["schema_accepts"] == true && !expected { structural_without_authority += 1; }
    }
    assert!(events >= 100 && transcripts >= 4);
    assert!(structural_without_authority >= 5);
}

#[test]
fn peer_approval_protocol_binding_compares_the_full_authenticated_author_key() {
    let data: Value = serde_json::from_str(include_str!("fixtures/m8-protocol-v1.json")).unwrap();
    let wire = data["security_events"][0]["wire"].as_str().unwrap();
    let author = SigningKey::from_bytes(&[0x11; 32]);
    let event = read_security_v3_event(wire.as_bytes(), &author.verifying_key()).unwrap();
    verify_peer_approval_protocol_binding(&event, &author.verifying_key()).unwrap();
    let other = SigningKey::from_bytes(&[0x22; 32]);
    assert!(verify_peer_approval_protocol_binding(&event, &other.verifying_key()).is_err());
    assert!(matches!(event.payload(), SecurityPayloadV3::PeerApproved { .. }));
}
