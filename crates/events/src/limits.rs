//! Resource limits from accepted ADR-0016 §7.1.
//!
//! `DEFAULT_SEAL_EVENT_THRESHOLD_HINT` is a configuration hint only. It is not a
//! frozen production protocol constant (ADR-0013 / ADR-0016).

/// Maximum accepted record / `body_len` bytes (1 MiB).
pub const MAX_ACCEPTED_RECORD_BYTES: u32 = 1_048_576;

/// Maximum `memory.note` payload text UTF-8 bytes (512 KiB).
pub const MAX_MEMORY_NOTE_TEXT_BYTES: usize = 524_288;

/// Maximum JSON nesting depth.
pub const MAX_JSON_NESTING_DEPTH: usize = 32;

/// Maximum JSON object/array elements per container.
pub const MAX_JSON_CONTAINER_ELEMENTS: usize = 4096;

/// Maximum total JSON string bytes decoded per record.
pub const MAX_JSON_STRING_BYTES_PER_RECORD: usize = 1_048_576;

/// Suggested default seal threshold for configuration only (not a protocol constant).
pub const DEFAULT_SEAL_EVENT_THRESHOLD_HINT: u64 = 1000;

/// Returns an error if `body_len` exceeds the accepted record maximum.
pub fn check_body_len(body_len: u32) -> crate::EventResult<()> {
    if body_len > MAX_ACCEPTED_RECORD_BYTES {
        return Err(crate::EventError::LimitExceeded(format!(
            "body_len {body_len} exceeds max {MAX_ACCEPTED_RECORD_BYTES}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_body_len_at_limit_and_rejects_above() {
        check_body_len(MAX_ACCEPTED_RECORD_BYTES).unwrap();
        let err = check_body_len(MAX_ACCEPTED_RECORD_BYTES + 1).unwrap_err();
        assert!(matches!(err, crate::EventError::LimitExceeded(_)));
    }

    #[test]
    fn seal_hint_is_not_zero_and_matches_adr_default() {
        assert_eq!(DEFAULT_SEAL_EVENT_THRESHOLD_HINT, 1000);
    }
}
