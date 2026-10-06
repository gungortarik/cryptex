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

fn parse_u32_digits(s: &str) -> Option<u32> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

fn is_leap_year(year: u32) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

fn days_in_month(year: u32, month: u32) -> Option<u32> {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Some(31),
        4 | 6 | 9 | 11 => Some(30),
        2 => Some(if is_leap_year(year) { 29 } else { 28 }),
        _ => None,
    }
}

fn validate_calendar_date(year: u32, month: u32, day: u32) -> CoreResult<()> {
    let Some(dim) = days_in_month(year, month) else {
        return Err(CoreError::InvalidId("timestamp month must be 01-12".into()));
    };
    if day == 0 || day > dim {
        return Err(CoreError::InvalidId(
            "timestamp day is not valid for the given month/year".into(),
        ));
    }
    Ok(())
}

fn validate_clock(hour: u32, minute: u32, second: u32) -> CoreResult<()> {
    if hour > 23 {
        return Err(CoreError::InvalidId("timestamp hour must be 00-23".into()));
    }
    if minute > 59 {
        return Err(CoreError::InvalidId(
            "timestamp minute must be 00-59".into(),
        ));
    }
    // RFC 3339 allows leap seconds (60).
    if second > 60 {
        return Err(CoreError::InvalidId(
            "timestamp second must be 00-60".into(),
        ));
    }
    Ok(())
}

fn validate_offset(sign_and_rest: &str) -> CoreResult<()> {
    // Expected: ±HH:MM (length 6 including sign)
    if sign_and_rest.len() != 6 {
        return Err(CoreError::InvalidId(
            "timestamp offset must be +/-HH:MM".into(),
        ));
    }
    let bytes = sign_and_rest.as_bytes();
    if bytes[0] != b'+' && bytes[0] != b'-' {
        return Err(CoreError::InvalidId(
            "timestamp offset must be +/-HH:MM".into(),
        ));
    }
    if bytes[3] != b':' {
        return Err(CoreError::InvalidId(
            "timestamp offset must be +/-HH:MM".into(),
        ));
    }
    let hours = parse_u32_digits(std::str::from_utf8(&bytes[1..3]).unwrap_or(""))
        .ok_or_else(|| CoreError::InvalidId("timestamp offset hours are malformed".into()))?;
    let minutes = parse_u32_digits(std::str::from_utf8(&bytes[4..6]).unwrap_or(""))
        .ok_or_else(|| CoreError::InvalidId("timestamp offset minutes are malformed".into()))?;
    if hours > 23 {
        return Err(CoreError::InvalidId(
            "timestamp offset hours must be 00-23".into(),
        ));
    }
    if minutes > 59 {
        return Err(CoreError::InvalidId(
            "timestamp offset minutes must be 00-59".into(),
        ));
    }
    Ok(())
}

/// RFC 3339 profile for display timestamps (ADR-0015).
///
/// Accepts `YYYY-MM-DDTHH:MM:SS[.fff]Z` or `YYYY-MM-DDTHH:MM:SS[.fff]±HH:MM`.
/// Rejects impossible calendar dates, clock fields, and timezone offsets.
pub fn parse_rfc3339_str(value: &str) -> CoreResult<String> {
    // Reject leading/trailing whitespace; do not silently normalize.
    if value != value.trim() {
        return Err(CoreError::InvalidId(
            "timestamp must not have leading or trailing whitespace".into(),
        ));
    }
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

    let year = parse_u32_digits(&value[0..4])
        .ok_or_else(|| CoreError::InvalidId("timestamp year is malformed".into()))?;
    let month = parse_u32_digits(&value[5..7])
        .ok_or_else(|| CoreError::InvalidId("timestamp month is malformed".into()))?;
    let day = parse_u32_digits(&value[8..10])
        .ok_or_else(|| CoreError::InvalidId("timestamp day is malformed".into()))?;
    let hour = parse_u32_digits(&value[11..13])
        .ok_or_else(|| CoreError::InvalidId("timestamp hour is malformed".into()))?;
    let minute = parse_u32_digits(&value[14..16])
        .ok_or_else(|| CoreError::InvalidId("timestamp minute is malformed".into()))?;
    let second = parse_u32_digits(&value[17..19])
        .ok_or_else(|| CoreError::InvalidId("timestamp second is malformed".into()))?;

    validate_calendar_date(year, month, day)?;
    validate_clock(hour, minute, second)?;

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
            validate_offset(&rest[i..])?;
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
        parse_rfc3339_str("2024-02-29T23:59:60Z").unwrap(); // leap day + leap second
        parse_rfc3339_str("2026-10-06T12:30:45-05:00").unwrap();
    }

    #[test]
    fn rejects_bad_uuid_and_timestamp_shape() {
        assert!(parse_uuid_str("not-a-uuid").is_err());
        assert!(parse_rfc3339_str("2026-10-06").is_err());
        assert!(parse_rfc3339_str("2026-10-06T00:00:00").is_err());
    }

    #[test]
    fn rejects_leading_or_trailing_whitespace_on_rfc3339() {
        assert!(parse_rfc3339_str(" 2026-10-06T00:00:00Z").is_err());
        assert!(parse_rfc3339_str("2026-10-06T00:00:00Z ").is_err());
        assert!(parse_rfc3339_str("\t2026-10-06T00:00:00Z").is_err());
        assert!(parse_rfc3339_str("2026-10-06T00:00:00Z\n").is_err());
        // Still accept canonical forms without surrounding whitespace.
        parse_rfc3339_str("2026-10-06T00:00:00Z").unwrap();
        parse_rfc3339_str("2026-10-06T00:00:00.123+00:00").unwrap();
    }

    #[test]
    fn rejects_impossible_calendar_values() {
        assert!(parse_rfc3339_str("2026-13-01T00:00:00Z").is_err()); // month
        assert!(parse_rfc3339_str("2026-00-01T00:00:00Z").is_err());
        assert!(parse_rfc3339_str("2026-04-31T00:00:00Z").is_err()); // Apr 31
        assert!(parse_rfc3339_str("2026-02-30T00:00:00Z").is_err());
        assert!(parse_rfc3339_str("2025-02-29T00:00:00Z").is_err()); // non-leap
        assert!(parse_rfc3339_str("2026-10-00T00:00:00Z").is_err());
    }

    #[test]
    fn rejects_impossible_clock_and_offset_values() {
        assert!(parse_rfc3339_str("2026-10-06T24:00:00Z").is_err());
        assert!(parse_rfc3339_str("2026-10-06T12:60:00Z").is_err());
        assert!(parse_rfc3339_str("2026-10-06T12:00:61Z").is_err());
        assert!(parse_rfc3339_str("2026-10-06T00:00:00+24:00").is_err());
        assert!(parse_rfc3339_str("2026-10-06T00:00:00+00:60").is_err());
        assert!(parse_rfc3339_str("2026-10-06T00:00:00+99:99").is_err());
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
