//! Secret-payload exposure detection for spike error-redaction cells.
//!
//! UTF-8 substring search alone is insufficient: `Debug` of `Vec<u8>` commonly
//! renders decimal bytes (e.g. `BadEncoding([83, 89, ...])`) from which the
//! secret is fully recoverable.

/// True when `text` lets an observer recover `secret` bytes (UTF-8 text and/or
/// common Debug/hex renderings). Does not print or return the secret.
pub fn payload_bytes_recoverable(secret: &[u8], text: &str) -> bool {
    if secret.is_empty() {
        return false;
    }

    if let Ok(utf8) = std::str::from_utf8(secret) {
        if !utf8.is_empty() && text.contains(utf8) {
            return true;
        }
    }

    let decimal_spaced: String = secret
        .iter()
        .map(|b| b.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    if text.contains(&decimal_spaced) {
        return true;
    }

    let decimal_compact: String = secret
        .iter()
        .map(|b| b.to_string())
        .collect::<Vec<_>>()
        .join(",");
    if text.contains(&decimal_compact) {
        return true;
    }

    // Continuous hex (lower/upper); require a minimum length to avoid tiny false positives.
    if secret.len() >= 4 {
        let hex_lower: String = secret.iter().map(|b| format!("{b:02x}")).collect();
        let hex_upper: String = secret.iter().map(|b| format!("{b:02X}")).collect();
        let lower = text.to_ascii_lowercase();
        if lower.contains(&hex_lower) || text.contains(&hex_upper) {
            return true;
        }
    }

    false
}

/// Separates safe missing-item surfaces from payload-bearing upstream errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RedactionAssessment {
    pub missing_item_safe: bool,
    pub payload_bearing_exposed: bool,
}

impl RedactionAssessment {
    pub fn overall_pass(&self) -> bool {
        self.missing_item_safe && !self.payload_bearing_exposed
    }

    pub fn matrix_note(&self) -> String {
        let missing = if self.missing_item_safe {
            "missing_item_errors=SAFE"
        } else {
            "missing_item_errors=EXPOSED"
        };
        let payload = if self.payload_bearing_exposed {
            "payload_bearing_upstream=EXPOSED"
        } else {
            "payload_bearing_upstream=SAFE"
        };
        let overall = if self.overall_pass() {
            "overall=PASS"
        } else {
            "overall=FAIL"
        };
        format!("{missing}; {payload}; {overall}")
    }
}

/// Assess K2-style surfaces: a missing-item error vs a known payload-bearing variant.
pub fn assess_k2_error_surfaces(
    secret: &[u8],
    missing_display: &str,
    missing_debug: &str,
    payload_display: &str,
    payload_debug: &str,
) -> RedactionAssessment {
    let missing_item_safe = !payload_bytes_recoverable(secret, missing_display)
        && !payload_bytes_recoverable(secret, missing_debug);
    let payload_bearing_exposed = payload_bytes_recoverable(secret, payload_display)
        || payload_bytes_recoverable(secret, payload_debug);
    RedactionAssessment {
        missing_item_safe,
        payload_bearing_exposed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keyring_core::Error;

    #[test]
    fn detects_utf8_text_leak() {
        let secret = b"SYNTHETIC_SPIKE_SECRET_NOT_REAL::unit::v1";
        assert!(payload_bytes_recoverable(
            secret,
            "oops SYNTHETIC_SPIKE_SECRET_NOT_REAL::unit::v1 leaked"
        ));
    }

    #[test]
    fn detects_debug_decimal_byte_list() {
        let secret = b"ABC";
        let debug = format!("{:?}", secret); // [65, 66, 67]
        assert!(payload_bytes_recoverable(secret, &debug));
        assert!(payload_bytes_recoverable(
            secret,
            &format!("BadEncoding({debug})")
        ));
    }

    #[test]
    fn detects_hex_encoding() {
        let secret = [0xde, 0xad, 0xbe, 0xef];
        assert!(payload_bytes_recoverable(&secret, "blob=deadbeef"));
        assert!(payload_bytes_recoverable(&secret, "blob=DEADBEEF"));
    }

    #[test]
    fn noentry_surfaces_do_not_expose_payload() {
        let secret = b"SYNTHETIC_SPIKE_SECRET_NOT_REAL::unit::v1";
        let err = Error::NoEntry;
        assert!(!payload_bytes_recoverable(secret, &format!("{err}")));
        assert!(!payload_bytes_recoverable(secret, &format!("{err:?}")));
    }

    #[test]
    fn bad_encoding_display_safe_but_debug_exposes_bytes() {
        let secret = b"SYNTHETIC_SPIKE_SECRET_NOT_REAL::unit::v1".to_vec();
        let err = Error::BadEncoding(secret.clone());
        let display = format!("{err}");
        let debug = format!("{err:?}");
        assert!(
            !payload_bytes_recoverable(&secret, &display),
            "Display should not embed bytes"
        );
        assert!(
            payload_bytes_recoverable(&secret, &debug),
            "Debug must be treated as payload exposure"
        );
    }

    #[test]
    fn assessment_fails_overall_when_payload_bearing_debug_exposes() {
        let secret = b"SYNTHETIC_SPIKE_SECRET_NOT_REAL::unit::v1".to_vec();
        let missing = Error::NoEntry;
        let payload = Error::BadEncoding(secret.clone());
        let a = assess_k2_error_surfaces(
            &secret,
            &format!("{missing}"),
            &format!("{missing:?}"),
            &format!("{payload}"),
            &format!("{payload:?}"),
        );
        assert!(a.missing_item_safe);
        assert!(a.payload_bearing_exposed);
        assert!(!a.overall_pass());
        let note = a.matrix_note();
        assert!(note.contains("missing_item_errors=SAFE"));
        assert!(note.contains("payload_bearing_upstream=EXPOSED"));
        assert!(note.contains("overall=FAIL"));
    }

    #[test]
    fn assessment_passes_when_both_surfaces_safe() {
        let secret = b"SYNTHETIC_SPIKE_SECRET_NOT_REAL::unit::v1";
        let a = assess_k2_error_surfaces(
            secret,
            "secure_store_missing_item",
            "Missing",
            "Password data is not valid UTF-8",
            "BadEncoding(<redacted>)",
        );
        assert!(a.overall_pass());
    }
}
