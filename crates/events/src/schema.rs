//! JSON Schema rendering for production event types (ADR-0016 §10).

use schemars::schema_for;
use serde_json::{Value, json};

use crate::envelope::AcceptedEvent;
use crate::v2_payload::V2KnowledgeAssertionEvent;

/// Render the checked-in production AcceptedEvent schema document as pretty JSON + LF.
pub fn render_accepted_event_schema_json() -> String {
    let schema = schema_for!(AcceptedEvent);
    let mut value = serde_json::to_value(&schema).expect("schema value");
    if let Value::Object(ref mut map) = value {
        map.insert(
            "$id".into(),
            json!("https://cryptex.local/schemas/cryptex-event-v1.schema.json"),
        );
        map.insert("title".into(), json!("CryptexAcceptedEventV1"));
    }
    let mut text = serde_json::to_string_pretty(&value).expect("schema json");
    text = text.replace("\r\n", "\n");
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text
}

/// Relative path from the repository root to the checked-in schema artifact.
pub const CHECKED_IN_SCHEMA_REL_PATH: &str = "packages/schemas/cryptex-event-v1.schema.json";

/// Render the ADR-0017 v2 structured-assertion schema as pretty JSON + LF.
pub fn render_v2_knowledge_assertion_schema_json() -> String {
    let schema = schema_for!(V2KnowledgeAssertionEvent);
    let mut value = serde_json::to_value(&schema).expect("schema value");
    if let Value::Object(ref mut map) = value {
        map.insert(
            "$id".into(),
            json!("https://cryptex.local/schemas/cryptex-event-v2.schema.json"),
        );
        map.insert("title".into(), json!("CryptexKnowledgeAssertionEventV2"));
    }
    let mut text = serde_json::to_string_pretty(&value).expect("schema json");
    text = text.replace("\r\n", "\n");
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text
}

pub const CHECKED_IN_V2_SCHEMA_REL_PATH: &str = "packages/schemas/cryptex-event-v2.schema.json";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device_seq::DEVICE_SEQ_PATTERN;
    use crate::hex::{CONTENT_HASH_HEX_PATTERN, SIGNATURE_HEX_PATTERN, SIGNING_KEY_ID_HEX_PATTERN};
    use serde_json::Value;
    use std::path::PathBuf;

    fn rendered_value() -> Value {
        serde_json::from_str(&render_accepted_event_schema_json()).unwrap()
    }

    fn def<'a>(root: &'a Value, name: &str) -> &'a Value {
        &root["$defs"][name]
    }

    #[test]
    fn schema_mentions_memory_note_and_string_device_seq() {
        let text = render_accepted_event_schema_json();
        assert!(text.contains("memory.note"));
        assert!(text.contains("device_seq"));
        assert!(!text.contains("integrity_probe"));
    }

    #[test]
    fn schema_emits_device_seq_pattern_and_exact_schema_version() {
        let root = rendered_value();
        let device_seq = def(&root, "DeviceSeq");
        assert_eq!(device_seq["type"], "string");
        assert_eq!(device_seq["pattern"], DEVICE_SEQ_PATTERN);
        assert_eq!(device_seq["minLength"], 1);
        assert_eq!(device_seq["maxLength"], 20);

        let schema_version = &root["properties"]["schema_version"];
        assert_eq!(schema_version["type"], "integer");
        assert_eq!(schema_version["minimum"], 1);
        assert_eq!(schema_version["maximum"], 1);
    }

    #[test]
    fn schema_emits_lowercase_hex_length_patterns() {
        let root = rendered_value();
        let content = def(&root, "ContentHashHex");
        assert_eq!(content["pattern"], CONTENT_HASH_HEX_PATTERN);
        assert_eq!(content["minLength"], 64);
        assert_eq!(content["maxLength"], 64);

        let key = def(&root, "SigningKeyId");
        assert_eq!(key["pattern"], SIGNING_KEY_ID_HEX_PATTERN);
        assert_eq!(key["minLength"], 32);
        assert_eq!(key["maxLength"], 32);

        let sig = def(&root, "SignatureHex");
        assert_eq!(sig["pattern"], SIGNATURE_HEX_PATTERN);
        assert_eq!(sig["minLength"], 128);
        assert_eq!(sig["maxLength"], 128);
    }

    #[test]
    fn schema_sets_additional_properties_false_on_objects() {
        let root = rendered_value();
        assert_eq!(root["additionalProperties"], false);
        assert_eq!(
            def(&root, "MemoryNotePayload")["additionalProperties"],
            false
        );
    }

    #[test]
    fn checked_in_schema_matches_rendered_output() {
        let rendered = render_accepted_event_schema_json();
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.pop();
        path.pop();
        path.push(CHECKED_IN_SCHEMA_REL_PATH);
        let on_disk = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        assert_eq!(
            on_disk.replace("\r\n", "\n"),
            rendered,
            "schema drift detected; run cryptex-events-schema-gen and commit packages/schemas"
        );
    }

    #[test]
    fn v2_schema_is_separate_strict_and_contains_all_payload_kinds() {
        let text = render_v2_knowledge_assertion_schema_json();
        let root: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(root["title"], "CryptexKnowledgeAssertionEventV2");
        assert_eq!(root["properties"]["schema_version"]["minimum"], 2);
        assert_eq!(root["properties"]["schema_version"]["maximum"], 2);
        assert_eq!(root["properties"]["payload_schema_version"]["minimum"], 1);
        assert_eq!(root["properties"]["payload_schema_version"]["maximum"], 1);
        assert_eq!(
            root["$defs"]["KnowledgeAssertionEventType"]["enum"][0],
            "knowledge.assertion"
        );
        for kind in [
            "entity.declare",
            "entity.alias",
            "fact.assert",
            "relation.assert",
            "assertion.correct",
            "entity.merge",
            "entity.merge_revoke",
            "assertion.resolve_conflict",
        ] {
            assert!(text.contains(kind), "missing schema payload kind {kind}");
        }
        assert!(text.contains("confirmed"));
        assert!(text.contains("inferred"));
        assert!(text.contains("uncertain"));
    }

    #[test]
    fn checked_in_v2_schema_matches_rendered_output() {
        let rendered = render_v2_knowledge_assertion_schema_json();
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.pop();
        path.pop();
        path.push(CHECKED_IN_V2_SCHEMA_REL_PATH);
        let on_disk = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        assert_eq!(on_disk.replace("\r\n", "\n"), rendered);
    }
}
