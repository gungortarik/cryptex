//! JSON Schema rendering for production event types (ADR-0016 §10).

use schemars::schema_for;
use serde_json::{Value, json};

use crate::envelope::AcceptedEvent;

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn schema_mentions_memory_note_and_string_device_seq() {
        let text = render_accepted_event_schema_json();
        assert!(text.contains("memory.note"));
        assert!(text.contains("device_seq"));
        // Must not advertise fixture-only production types.
        assert!(!text.contains("integrity_probe"));
    }

    #[test]
    fn checked_in_schema_matches_rendered_output() {
        let rendered = render_accepted_event_schema_json();
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.pop(); // crates
        path.pop(); // repo root
        path.push(CHECKED_IN_SCHEMA_REL_PATH);
        let on_disk = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        assert_eq!(
            on_disk.replace("\r\n", "\n"),
            rendered,
            "schema drift detected; run cryptex-events-schema-gen and commit packages/schemas"
        );
    }
}
