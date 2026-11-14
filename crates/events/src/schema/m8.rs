//! Candidate M8 structural contracts derived from the actual Rust wire types.
//! Shape validation never verifies signatures, chain links or owner authority.

use super::*;
use crate::bootstrap::{ApprovalPayload, OfferPayload, ProofPayload, WireMessage};
use crate::security_v3::{
    DeviceRestrictedPayload, DeviceSelfRetiredPayload, PeerApprovedPayload, SecurityV3Envelope,
};
use schemars::JsonSchema;

const UUID: &str = "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$";
const DEVICE: &str = "^(?!(?:[cC][oO][nN]|[pP][rR][nN]|[aA][uU][xX]|[nN][uU][lL]|[cC][oO][mM][1-9]|[lL][pP][tT][1-9])$)[A-Za-z0-9_-]{1,64}$";
const TRANSPORT: &str = "^[A-Z2-7]{7}(?:-[A-Z2-7]{7}){7}$";
const ZERO: &str = "0000000000000000000000000000000000000000000000000000000000000000";

// Inline derived definitions so tuple/variant composition has no displaced
// $refs. Current wire definitions are finite and have no recursive types.
fn wire<T: JsonSchema>() -> Value {
    fn expand(value: &Value, defs: &Value) -> Value {
        if let Some(reference) = value.get("$ref").and_then(Value::as_str) {
            let name = reference
                .strip_prefix("#/$defs/")
                .expect("local schema reference");
            return expand(defs.get(name).expect("schema definition exists"), defs);
        }
        match value {
            Value::Object(map) => Value::Object(
                map.iter()
                    .filter(|(key, _)| !matches!(key.as_str(), "$schema" | "$defs" | "title"))
                    .map(|(key, value)| (key.clone(), expand(value, defs)))
                    .collect(),
            ),
            Value::Array(values) => {
                Value::Array(values.iter().map(|value| expand(value, defs)).collect())
            }
            _ => value.clone(),
        }
    }
    let root = serde_json::to_value(schemars::schema_for!(T)).expect("wire schema");
    expand(&root, &root["$defs"])
}

fn hex(width: usize) -> Value {
    json!({"type":"string", "minLength":width, "maxLength":width, "pattern":format!("^[0-9a-f]{{{width}}}$")})
}

fn string(pattern: &str) -> Value {
    json!({"type":"string", "pattern":format!("{pattern}(?![\\s\\S])")})
}

fn set(schema: &mut Value, name: &str, property: Value) {
    assert!(
        schema["properties"].get(name).is_some(),
        "wire field absent: {name}"
    );
    schema["properties"][name] = property;
}

fn require_all(schema: &mut Value) {
    schema["required"] = Value::Array(
        schema["properties"]
            .as_object()
            .expect("wire object")
            .keys()
            .cloned()
            .map(Value::String)
            .collect(),
    );
    schema["additionalProperties"] = Value::Bool(false);
}

// Exact lexical u64 bound; unlike a 20-digit-only pattern this rejects MAX+1.
fn decimal(allow_zero: bool) -> Value {
    let max = u64::MAX.to_string();
    let mut choices = vec!["[1-9][0-9]{0,18}".to_owned(), max.clone()];
    for (index, digit) in max.bytes().enumerate() {
        let low = if index == 0 { b'1' } else { b'0' };
        if digit > low {
            choices.push(format!(
                "{}[{}-{}][0-9]{{{}}}",
                &max[..index],
                char::from(low),
                char::from(digit - 1),
                max.len() - index - 1
            ));
        }
    }
    if allow_zero {
        choices.push("0".into());
    }
    json!({"type":"string", "pattern":format!("^(?:{})$(?![\\s\\S])", choices.join("|")), "minLength":1, "maxLength":20})
}

fn message(kind: &str, role: Option<&str>) -> Value {
    let mut result = wire::<WireMessage>();
    set(
        &mut result,
        "protocol_version",
        json!({"type":"integer", "const":1}),
    );
    set(&mut result, "message_type", json!({"const":kind}));
    set(&mut result, "brain_id", string(UUID));
    set(&mut result, "sender_device_id", string(DEVICE));
    set(&mut result, "sender_signing_key_id", hex(32));
    set(&mut result, "sender_public_key", hex(64));
    set(&mut result, "sender_transport_device_id", string(TRANSPORT));
    set(&mut result, "signature", hex(128));
    set(&mut result, "transaction_id", hex(64));
    set(
        &mut result,
        "previous_hash",
        if kind == "offer" {
            json!({"const":ZERO})
        } else {
            hex(64)
        },
    );
    for (name, property) in [
        ("peer_device_id", string(DEVICE)),
        ("peer_signing_key_id", hex(32)),
        ("peer_public_key", hex(64)),
        ("peer_transport_device_id", string(TRANSPORT)),
    ] {
        set(
            &mut result,
            name,
            if kind == "offer" {
                json!({"type":"null"})
            } else {
                property
            },
        );
    }
    let mut payload = match kind {
        "offer" => wire::<OfferPayload>(),
        "proof" => wire::<ProofPayload>(),
        _ => wire::<ApprovalPayload>(),
    };
    if let Some(role) = role {
        set(&mut payload, "role", json!({"type":"string", "const":role}));
    }
    set(&mut result, "payload", payload);
    require_all(&mut result);
    result
}

fn transcript() -> Value {
    json!({"type":"array", "minItems":4, "maxItems":4, "additionalItems":false,
        "items":[message("offer", None), message("proof", None), message("approval", Some("initiator")), message("approval", Some("responder"))]})
}

fn cutoff_pair(payload: &mut Value, sequence: &str, hash: &str) {
    set(payload, sequence, decimal(true));
    set(payload, hash, hex(64));
    payload["allOf"] = json!([{"if":{"properties":{(sequence):{"const":"0"}}},
        "then":{"properties":{(hash):{"const":ZERO}}}, "else":{"properties":{(hash):{"not":{"const":ZERO}}}}}]);
}

fn event(kind: &str) -> Value {
    let mut result = wire::<SecurityV3Envelope>();
    set(
        &mut result,
        "schema_version",
        json!({"type":"integer", "const":3}),
    );
    set(
        &mut result,
        "event_type",
        json!({"type":"string", "const":kind}),
    );
    set(
        &mut result,
        "classification",
        json!({"type":"string", "const":"internal"}),
    );
    for name in ["event_id", "brain_id"] {
        set(&mut result, name, string(UUID));
    }
    set(&mut result, "device_id", string(DEVICE));
    set(&mut result, "device_seq", decimal(false));
    set(
        &mut result,
        "created_at",
        json!({"type":"string", "format":"date-time"}),
    );
    let mut payload = match kind {
        "security.peer_approved" => {
            let mut p = wire::<PeerApprovedPayload>();
            set(&mut p, "transcript", transcript());
            p
        }
        "security.device_restricted" => {
            let mut p = wire::<DeviceRestrictedPayload>();
            set(&mut p, "target_device_id", string(DEVICE));
            set(&mut p, "target_public_key", hex(64));
            set(
                &mut p,
                "reason",
                json!({"type":"string", "enum":["retired", "compromised"]}),
            );
            cutoff_pair(&mut p, "cutoff_seq", "cutoff_content_hash");
            p
        }
        _ => {
            let mut p = wire::<DeviceSelfRetiredPayload>();
            cutoff_pair(
                &mut p,
                "last_authorized_seq",
                "last_authorized_content_hash",
            );
            p
        }
    };
    set(&mut payload, "transaction_id", hex(64));
    set(&mut result, "payload", payload);
    require_all(&mut result);
    result
}

fn render(name: &str, title: &str, body: Value) -> String {
    let mut body = body;
    body["$schema"] = json!("http://json-schema.org/draft-07/schema#");
    body["$id"] = json!(format!("https://cryptex.local/schemas/{name}.schema.json"));
    body["title"] = json!(title);
    body["$comment"] = json!(
        "Candidate M8 structural contract. Duplicate keys, raw JSON limits/JCS, cryptographic keys/signatures, hash links, cross-field identity/sequence/history constraints and owner authority MUST still be checked by the Rust protocol verifier. Schema acceptance grants no trust."
    );
    format!(
        "{}\n",
        serde_json::to_string_pretty(&body).expect("M8 schema JSON")
    )
}

pub fn bootstrap_schema_json() -> String {
    render(
        "cryptex-bootstrap-v1",
        "CryptexBootstrapMessageV1",
        json!({"oneOf":[
        message("offer", None), message("proof", None), message("approval", Some("initiator")), message("approval", Some("responder"))]}),
    )
}

pub fn transcript_schema_json() -> String {
    render(
        "cryptex-bootstrap-transcript-v1",
        "CryptexBootstrapTranscriptV1",
        transcript(),
    )
}

pub fn security_schema_json() -> String {
    render(
        "cryptex-event-v3",
        "CryptexSecurityEventV3",
        json!({"oneOf":[
        event("security.peer_approved"), event("security.device_restricted"), event("security.device_self_retired")]}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_in_m8_schemas_match_rust_wire_definitions() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/schemas");
        for (name, expected) in [
            ("cryptex-bootstrap-v1", bootstrap_schema_json()),
            ("cryptex-bootstrap-transcript-v1", transcript_schema_json()),
            ("cryptex-event-v3", security_schema_json()),
        ] {
            assert_eq!(
                std::fs::read_to_string(root.join(format!("{name}.schema.json")))
                    .unwrap()
                    .replace("\r\n", "\n"),
                expected
            );
        }
    }
}
