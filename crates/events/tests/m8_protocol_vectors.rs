//! Fixed wire bytes computed without the Cryptex Rust implementation.
//! These public synthetic keys must never be used for real identities.

use cryptex_core::ids::DeviceId;
use cryptex_events::security_v3::{
    sign_device_restricted_event, sign_device_self_retired_event,
};
use cryptex_events::*;
use ed25519_dalek::SigningKey;
use serde_json::Value;

fn corpus() -> Value {
    serde_json::from_str(include_str!("fixtures/m8-protocol-v1.json")).unwrap()
}

fn text<'a>(value: &'a Value, name: &str) -> &'a str {
    value[name].as_str().unwrap()
}

fn messages(value: &Value) -> Vec<&[u8]> {
    value["bootstrap"]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|message| text(message, "wire").as_bytes())
        .collect()
}

#[test]
fn independent_bootstrap_bytes_signatures_hashes_and_both_roles_match() {
    let data = corpus();
    let raw = messages(&data);
    let a = SigningKey::from_bytes(&[0x11; 32]);
    let b = SigningKey::from_bytes(&[0x22; 32]);
    let identities = data["identities"].as_array().unwrap();
    let offer: Value = serde_json::from_slice(raw[0]).unwrap();
    let generated = create_bootstrap_offer(
        text(&offer, "brain_id"),
        text(&offer, "transaction_id"),
        &DeviceId::new(text(&identities[0], "device_id")).unwrap(),
        text(&identities[0], "transport_id"),
        &[0x11; 32],
        &a,
    )
    .unwrap();
    assert_eq!(generated, raw[0]);
    assert_eq!(
        create_bootstrap_proof(
            raw[0],
            &DeviceId::new(text(&identities[1], "device_id")).unwrap(),
            text(&identities[1], "transport_id"),
            &[0x22; 32],
            &b,
        )
        .unwrap(),
        raw[1]
    );
    for (role, len, key) in [
        (BootstrapApprovalRole::Initiator, 2, &a),
        (BootstrapApprovalRole::Responder, 3, &b),
    ] {
        let request = verify_bootstrap_approval_request(&raw[..len], role).unwrap();
        assert_eq!(sign_bootstrap_approval(&request, key).unwrap(), raw[len]);
    }
    let verified = verify_bootstrap_transcript(&raw).unwrap();
    for (actual, index) in [
        (verified.offer_hash(), 0),
        (verified.proof_hash(), 1),
        (verified.initiator_approval_hash(), 2),
        (verified.responder_approval_hash(), 3),
    ] {
        assert_eq!(actual, text(&data["bootstrap"]["messages"][index], "message_hash"));
    }
    assert_eq!(verified.agreement_hash(), text(&data["bootstrap"], "agreement_hash"));
    assert_eq!(verified.transcript_hash(), text(&data["bootstrap"], "transcript_hash"));
    let mut reordered = raw.clone();
    reordered.swap(2, 3);
    assert!(verify_bootstrap_transcript(&reordered).is_err());
    for index in 0..4 {
        let mut altered: Value = serde_json::from_slice(raw[index]).unwrap();
        altered["transaction_id"] = Value::String("cd".repeat(32));
        let bytes = serde_json_canonicalizer::to_vec(&altered).unwrap();
        let mut tampered = raw.clone();
        tampered[index] = &bytes;
        assert!(verify_bootstrap_transcript(&tampered).is_err());
    }
}

#[test]
fn independent_v3_all_operations_and_cutoff_extremes_match_exact_writer_bytes() {
    let data = corpus();
    let key = SigningKey::from_bytes(&[0x11; 32]);
    let transcript = verify_bootstrap_transcript(&messages(&data)).unwrap();
    let events = data["security_events"].as_array().unwrap();
    for fixture in events {
        let raw = text(fixture, "wire").as_bytes();
        let wire: Value = serde_json::from_slice(raw).unwrap();
        let verified = read_security_v3_event(raw, &key.verifying_key()).unwrap();
        assert_eq!(verified.content_hash().as_str(), text(fixture, "content_hash"));
        assert_eq!(verified.operation_digest(), text(fixture, "operation_digest"));
        let device = DeviceId::new(text(&wire, "device_id")).unwrap();
        let seq = DeviceSeq::parse(text(&wire, "device_seq")).unwrap();
        let key_id = SigningKeyId::parse(text(&wire, "signing_key_id")).unwrap();
        let previous = ContentHashHex::parse(text(&wire, "prev_content_hash")).unwrap();
        let payload = &wire["payload"];
        let event_id = text(&wire, "event_id");
        let time = text(&wire, "created_at");
        let tx = text(payload, "transaction_id");
        let generated = match text(&wire, "event_type") {
            "security.peer_approved" => sign_peer_approved_event(
                &transcript, event_id, seq, time, &key_id, &previous, &key,
            ),
            "security.device_restricted" => sign_device_restricted_event(
                event_id, text(&wire, "brain_id"), &device, seq, time, &key_id, &previous,
                tx, &DeviceId::new(text(payload, "target_device_id")).unwrap(),
                &SigningKeyId::parse(text(payload, "target_signing_key_id")).unwrap(),
                &SigningKey::from_bytes(&[0x22; 32]).verifying_key().to_bytes(),
                RestrictionReasonV3::Compromised,
                text(payload, "cutoff_seq").parse().unwrap(),
                &ContentHashHex::parse(text(payload, "cutoff_content_hash")).unwrap(), &key,
            ),
            "security.device_self_retired" => sign_device_self_retired_event(
                event_id, text(&wire, "brain_id"), &device, seq, time, &key_id, &previous,
                tx, text(payload, "last_authorized_seq").parse().unwrap(),
                &ContentHashHex::parse(text(payload, "last_authorized_content_hash")).unwrap(), &key,
            ),
            _ => unreachable!(),
        }
        .unwrap();
        assert_eq!(generated, raw);
        assert!(read_security_v3_event(raw, &SigningKey::from_bytes(&[0x22; 32]).verifying_key()).is_err());
        let error = validate_accepted_record_bytes(raw, &key.verifying_key()).unwrap_err();
        assert_eq!(error.reason.as_str(), "unsupported_schema");
    }
    for invalid in ["0", "01", "18446744073709551616"] {
        assert!(DeviceSeq::parse(invalid).is_err());
    }
    let last = &events[4];
    let wire: Value = serde_json::from_str(text(last, "wire")).unwrap();
    assert!(sign_device_self_retired_event(
        text(&wire, "event_id"), text(&wire, "brain_id"),
        &DeviceId::new(text(&wire, "device_id")).unwrap(),
        DeviceSeq::parse("18446744073709551615").unwrap(), text(&wire, "created_at"),
        &SigningKeyId::parse(text(&wire, "signing_key_id")).unwrap(),
        &ContentHashHex::parse(text(&wire, "prev_content_hash")).unwrap(),
        text(&wire["payload"], "transaction_id"), u64::MAX,
        &ContentHashHex::parse(text(&wire["payload"], "last_authorized_content_hash")).unwrap(), &key,
    ).is_err());
}

#[test]
fn independent_mixed_chain_preserves_v1_v2_v3_v1_links_and_sequence_gaps() {
    let data = corpus();
    let key = SigningKey::from_bytes(&[0x11; 32]).verifying_key();
    let mut chain = VersionedChainVerifier::new(text(&data["identities"][0], "device_id"));
    for (index, fixture) in data["mixed_chain"].as_array().unwrap().iter().enumerate() {
        let raw = text(fixture, "wire").as_bytes();
        let record = read_versioned_record(raw, &key).unwrap();
        assert!(matches!((index, &record),
            (0 | 3, VersionedRecord::VerifiedV1(_))
            | (1, VersionedRecord::VerifiedV2Supported(_))
            | (2, VersionedRecord::VerifiedSecurityV3(_))));
        chain.verify_next(&record).unwrap();
        assert!(chain.verify_next(&record).is_err(), "same sequence replay cannot advance chain");
    }
    let raw = text(&data["mixed_chain"][3], "wire").as_bytes();
    let record = read_versioned_record(raw, &key).unwrap();
    assert!(VersionedChainVerifier::new(text(&data["identities"][0], "device_id"))
        .verify_next(&record).is_err(), "missing prefix cannot form a complete chain");
}
