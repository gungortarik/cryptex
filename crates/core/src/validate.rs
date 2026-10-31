//! Shared format validators for ADR-0015 public metadata fields.

use crate::error::{CoreError, CoreResult};

/// Accepts canonical UUID text: 8-4-4-4-12 lowercase/uppercase hex with hyphens.
pub fn parse_uuid_str(value: &str) -> CoreResult<String> {
    // Reject leading/trailing whitespace; do not silently normalize.
    if value != value.trim() {
        return Err(CoreError::InvalidId(
            "brain_id must not have leading or trailing whitespace".into(),
        ));
    }
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
    let _ = parse_rfc3339_components(value)?;
    Ok(value.to_string())
}

/// Exact UTC instant from a validated M2 RFC 3339 timestamp.
///
/// Whole seconds use the same leap-second clamp as [`rfc3339_unix_seconds`].
/// The fractional component preserves every accepted digit so same-second
/// instants remain distinguishable without adding a stored projection column.
/// Fractional digits are normalized by stripping trailing zeros (`.1` == `.10`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rfc3339UtcInstant {
    /// Unix seconds (UTC), leap seconds (`:60`) clamped to `:59`.
    pub unix_seconds: i64,
    /// Digits after the decimal point (ASCII only); empty when absent.
    frac_digits: String,
}

impl Rfc3339UtcInstant {
    /// Fractional-second digits as accepted by the RFC3339 parser (may be empty).
    pub fn frac_digits(&self) -> &str {
        &self.frac_digits
    }
}

impl PartialOrd for Rfc3339UtcInstant {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Rfc3339UtcInstant {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.unix_seconds
            .cmp(&other.unix_seconds)
            .then_with(|| cmp_frac_digits(&self.frac_digits, &other.frac_digits))
    }
}

fn normalize_frac_digits(digits: &str) -> String {
    digits.trim_end_matches('0').to_string()
}

fn cmp_frac_digits(a: &str, b: &str) -> std::cmp::Ordering {
    let len = a.len().max(b.len());
    for i in 0..len {
        let da = a.as_bytes().get(i).copied().unwrap_or(b'0');
        let db = b.as_bytes().get(i).copied().unwrap_or(b'0');
        match da.cmp(&db) {
            std::cmp::Ordering::Equal => {}
            other => return other,
        }
    }
    std::cmp::Ordering::Equal
}

/// Convert a validated M2 RFC 3339 timestamp to an exact UTC instant.
///
/// Accepts the same forms as [`parse_rfc3339_str`], including numeric offsets
/// and fractional seconds. Equivalent offset spellings that name the same
/// instant (including matching fractions) compare equal.
/// Leap seconds (`:60`) are clamped to `:59` for the Unix timeline.
pub fn rfc3339_utc_instant(value: &str) -> CoreResult<Rfc3339UtcInstant> {
    let parsed = parse_rfc3339_components(value)?;
    let day_seconds = i64::from(parsed.hour) * 3600
        + i64::from(parsed.minute) * 60
        + i64::from(parsed.second.min(59));
    let days = days_from_civil(parsed.year as i32, parsed.month, parsed.day);
    // Local civil time as if UTC, then subtract the RFC3339 offset to reach UTC.
    let unix_seconds = days
        .checked_mul(86_400)
        .and_then(|d| d.checked_add(day_seconds))
        .and_then(|t| t.checked_sub(parsed.offset_secs))
        .ok_or_else(|| CoreError::InvalidId("timestamp instant overflows i64".into()))?;
    Ok(Rfc3339UtcInstant {
        unix_seconds,
        frac_digits: normalize_frac_digits(&parsed.frac_digits),
    })
}

/// Convert a validated M2 RFC 3339 timestamp to Unix seconds (UTC).
///
/// Accepts the same forms as [`parse_rfc3339_str`], including numeric offsets.
/// Equivalent instants with different offset spellings compare equal.
/// Leap seconds (`:60`) are clamped to `:59` for the Unix timeline.
/// Fractional seconds are discarded; prefer [`rfc3339_utc_instant`] when
/// sub-second ordering or inclusive bounds matter.
pub fn rfc3339_unix_seconds(value: &str) -> CoreResult<i64> {
    Ok(rfc3339_utc_instant(value)?.unix_seconds)
}

#[derive(Debug, Clone)]
struct Rfc3339Components {
    year: u32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
    /// Digits after `.` when present; empty when the timestamp has no fraction.
    frac_digits: String,
    /// Seconds east of UTC (`+01:00` => +3600, `-05:00` => -18000, `Z` => 0).
    offset_secs: i64,
}

fn parse_rfc3339_components(value: &str) -> CoreResult<Rfc3339Components> {
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
    let mut frac_digits = String::new();
    if rest_bytes[0] == b'.' {
        i = 1;
        if i >= rest_bytes.len() || !rest_bytes[i].is_ascii_digit() {
            return Err(CoreError::InvalidId(
                "timestamp fractional seconds are malformed".into(),
            ));
        }
        let frac_start = i;
        while i < rest_bytes.len() && rest_bytes[i].is_ascii_digit() {
            i += 1;
        }
        frac_digits = rest[frac_start..i].to_string();
    }
    if i >= rest_bytes.len() {
        return Err(CoreError::InvalidId(
            "timestamp must include timezone offset or Z".into(),
        ));
    }
    let offset_secs = match rest_bytes[i] {
        b'Z' | b'z' if i + 1 == rest_bytes.len() => 0,
        b'+' | b'-' => {
            let offset = &rest[i..];
            validate_offset(offset)?;
            let sign = if offset.as_bytes()[0] == b'+' {
                1i64
            } else {
                -1i64
            };
            let oh = parse_u32_digits(&offset[1..3]).expect("offset hours validated");
            let om = parse_u32_digits(&offset[4..6]).expect("offset minutes validated");
            sign * (i64::from(oh) * 3600 + i64::from(om) * 60)
        }
        _ => {
            return Err(CoreError::InvalidId(
                "timestamp must include timezone offset or Z".into(),
            ));
        }
    };

    Ok(Rfc3339Components {
        year,
        month,
        day,
        hour,
        minute,
        second,
        frac_digits,
        offset_secs,
    })
}

/// Howard Hinnant days-from-civil (proleptic Gregorian) → days since Unix epoch.
fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let mut y = year;
    let m = month as i32;
    let d = day as i32;
    y -= if m <= 2 { 1 } else { 0 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp as u64 + 2) / 5 + d as u64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era as i64) * 146_097 + doe as i64 - 719_468
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
    fn rfc3339_unix_seconds_equates_offset_spellings() {
        let z = rfc3339_unix_seconds("2026-10-06T17:30:45Z").unwrap();
        let offset = rfc3339_unix_seconds("2026-10-06T12:30:45-05:00").unwrap();
        assert_eq!(z, offset);
        let plus = rfc3339_unix_seconds("2026-10-06T18:30:45+01:00").unwrap();
        assert_eq!(z, plus);
    }

    #[test]
    fn rfc3339_utc_instant_preserves_fractional_ordering_and_offsets() {
        let early = rfc3339_utc_instant("2026-10-06T12:00:00.100Z").unwrap();
        let mid = rfc3339_utc_instant("2026-10-06T12:00:00.2Z").unwrap();
        let late = rfc3339_utc_instant("2026-10-06T12:00:00.2001Z").unwrap();
        assert!(early < mid);
        assert!(mid < late);
        assert_eq!(early.unix_seconds, mid.unix_seconds);

        // Equivalent offset spellings with matching fractions.
        let z = rfc3339_utc_instant("2026-10-06T17:30:45.123Z").unwrap();
        let offset = rfc3339_utc_instant("2026-10-06T12:30:45.123-05:00").unwrap();
        assert_eq!(z, offset);

        // Trailing-zero padding: .1 == .10 == .100
        let a = rfc3339_utc_instant("2026-10-06T00:00:00.1Z").unwrap();
        let b = rfc3339_utc_instant("2026-10-06T00:00:00.10Z").unwrap();
        let c = rfc3339_utc_instant("2026-10-06T00:00:00.100Z").unwrap();
        assert_eq!(a, b);
        assert_eq!(b, c);

        // Whole-second form has empty fraction and sorts before any positive fraction.
        let whole = rfc3339_utc_instant("2026-10-06T12:00:00Z").unwrap();
        assert!(whole < early);
        assert_eq!(
            rfc3339_unix_seconds("2026-10-06T12:00:00.999Z").unwrap(),
            whole.unix_seconds
        );
    }

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
    fn rejects_leading_or_trailing_whitespace_on_uuid() {
        assert!(parse_uuid_str(" 11111111-1111-4111-8111-111111111111").is_err());
        assert!(parse_uuid_str("11111111-1111-4111-8111-111111111111 ").is_err());
        assert!(parse_uuid_str("\t11111111-1111-4111-8111-111111111111").is_err());
        parse_uuid_str("11111111-1111-4111-8111-111111111111").unwrap();
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
