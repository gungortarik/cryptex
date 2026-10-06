//! Shared format validators for ADR-0015 public metadata fields.

use crate::error::{CoreError, CoreResult};

/// Accepts canonical UUID text: 8-4-4-4-12 lowercase/uppercase hex with hyphens.
pub fn parse_uuid_str(value: &str) -> CoreResult<String> {
    let value = value.trim();
    if value.len() != 36 {
        return Err(CoreError::InvalidId(
            "brain_id must be a UUID string (8-4-4-4-12)".into(),
        ));
    }
    let bytes = value.as_bytes();
    let hyphen_at = [8usize, 13, 18, 23];
    for (i, b) in bytes.iter().enumerate() {
        if hyphen_at.contains(&i) {
            if *b != b'-' {
                return Err(CoreError::InvalidId(
                    "brain_id must be a UUID string (8-4-4-4-12)".into(),
                ));
            }
            continue;
        }
        if !b.is_ascii_hexdigit() {
            return Err(CoreError::InvalidId(
                "brain_id must be a UUID string (8-4-4-4-12)".into(),
            ));
        }
    }
    Ok(value.to_string())
}

/// Minimal RFC 3339 / ISO-8601 profile used for display timestamps.
///
/// Accepts `YYYY-MM-DDTHH:MM:SS[.fff]Z` or `YYYY-MM-DDTHH:MM:SS[.fff]+HH:MM` / `-HH:MM`.
pub fn parse_rfc3339_str(value: &str) -> CoreResult<String> {
    let value = value.trim();
    if value.len() < 20 {
        return Err(CoreError::InvalidId(
            "timestamp must be RFC 3339 (e.g. 2026-10-06T00:00:00Z)".into(),
        ));
    }
    let bytes = value.as_bytes();
    let fixed_ok = bytes.len() >= 19
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && (bytes[10] == b'T' || bytes[10] == b't')
        && bytes[13] == b':'
        && bytes[16] == b':'
        && bytes[0..4].iter().all(|b| b.is_ascii_digit())
        && bytes[5..7].iter().all(|b| b.is_ascii_digit())
        && bytes[8..10].iter().all(|b| b.is_ascii_digit())
        && bytes[11..13].iter().all(|b| b.is_ascii_digit())
        && bytes[14..16].iter().all(|b| b.is_ascii_digit())
        && bytes[17..19].iter().all(|b| b.is_ascii_digit());
    if !fixed_ok {
        return Err(CoreError::InvalidId(
            "timestamp must be RFC 3339 (e.g. 2026-10-06T00:00:00Z)".into(),
        ));
    }
    let rest = &value[19..];
    if rest.is_empty() {
        return Err(CoreError::InvalidId(
            "timestamp must include timezone offset or Z".into(),
        ));
    }
    let rest_bytes = rest.as_bytes();
    let mut i = 0usize;
    if rest_bytes[0] == b'.' {
        i = 1;
        if i >= rest_bytes.len() || !rest_bytes[i].is_ascii_digit() {
            return Err(CoreError::InvalidId(
                "timestamp fractional seconds are malformed".into(),
            ));
        }
        while i < rest_bytes.len() && rest_bytes[i].is_ascii_digit() {
            i += 1;
        }
    }
    if i >= rest_bytes.len() {
        return Err(CoreError::InvalidId(
            "timestamp must include timezone offset or Z".into(),
        ));
    }
    match rest_bytes[i] {
        b'Z' | b'z' if i + 1 == rest_bytes.len() => Ok(value.to_string()),
        b'+' | b'-' => {
            let off = &rest[i..];
            if off.len() != 6 {
                return Err(CoreError::InvalidId(
                    "timestamp offset must be +/-HH:MM".into(),
                ));
            }
            let ob = off.as_bytes();
            if ob[3] != b':'
                || !ob[1].is_ascii_digit()
                || !ob[2].is_ascii_digit()
                || !ob[4].is_ascii_digit()
                || !ob[5].is_ascii_digit()
            {
                return Err(CoreError::InvalidId(
                    "timestamp offset must be +/-HH:MM".into(),
                ));
            }
            Ok(value.to_string())
        }
        _ => Err(CoreError::InvalidId(
            "timestamp must include timezone offset or Z".into(),
        )),
    }
}

pub fn require_exact_schema(kind: &str, found: u32, expected: u32) -> CoreResult<()> {
    if found != expected {
        return Err(CoreError::UnsupportedSchema {
            kind: kind.to_string(),
            found,
            expected,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_uuid_and_rfc3339() {
        parse_uuid_str("11111111-1111-4111-8111-111111111111").unwrap();
        parse_rfc3339_str("2026-10-06T00:00:00Z").unwrap();
        parse_rfc3339_str("2026-10-06T00:00:00.123+00:00").unwrap();
    }

    #[test]
    fn rejects_bad_uuid_and_timestamp() {
        assert!(parse_uuid_str("not-a-uuid").is_err());
        assert!(parse_rfc3339_str("2026-10-06").is_err());
        assert!(parse_rfc3339_str("2026-10-06T00:00:00").is_err());
    }

    #[test]
    fn unsupported_schema_fails_closed() {
        let err = require_exact_schema("brain identity", 99, 1).unwrap_err();
        assert!(matches!(
            err,
            CoreError::UnsupportedSchema {
                found: 99,
                expected: 1,
                ..
            }
        ));
    }
}
