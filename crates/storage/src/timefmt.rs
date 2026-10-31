//! UTC RFC 3339 second-precision formatting helpers.

/// Format Unix seconds as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
pub fn format_unix_secs_rfc3339_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let tod = (secs % 86_400) as u32;
    let hour = tod / 3_600;
    let minute = (tod % 3_600) / 60;
    let second = tod % 60;
    let (year, month, day) = civil_from_days(days + 719_468);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Howard Hinnant civil-from-days (days since 0000-03-01).
fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_unix_epoch() {
        assert_eq!(format_unix_secs_rfc3339_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(
            format_unix_secs_rfc3339_utc(1_000_000_000),
            "2001-09-09T01:46:40Z"
        );
    }
}
