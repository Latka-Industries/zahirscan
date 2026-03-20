use chrono::{DateTime, Datelike, NaiveDateTime, Utc};
use regex::Regex;
use std::sync::LazyLock;

/// Parse a date string into a Unix timestamp (seconds since epoch)
///
/// Tries multiple common date formats in order:
/// 1. RFC3339/ISO 8601 (e.g. "2026-01-21T05:15:52.116Z", "2025-10-27T18:23:54+00:00")
/// 2. ISO date with time (hyphens): YYYY-MM-DD HH:MM:SS
/// 3. ISO date with fractional seconds (hyphens)
/// 4. ISO date only (hyphens): YYYY-MM-DD
/// 5. Slash date with time: YYYY/MM/DD HH:MM:SS
/// 6. Slash date only: YYYY/MM/DD (e.g. "2025/11/27", "2026/01/21")
/// 7. US format: M/D/YYYY or MM/DD/YYYY (e.g. "1/10/2026")
/// 8. Compact DDMMYY (6 digits, e.g. "271125" = 27 Nov 2025)
/// 9. Unix asctime (e.g. "Mon Dec  1 19:26:16 2025")
/// 10. Syslog (no year): "Jan 15 00:07:12" — current year is assumed
///
/// Leading/trailing whitespace, brackets, and trailing comma/semicolon are trimmed (e.g. "1/10/2026,", "[2026-01-10 12:54:53.959]").
#[must_use]
pub fn parse_date_to_timestamp(date_str: &str) -> Option<i64> {
    let s = date_str
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim_end_matches([',', ';', ':']);
    if s.is_empty() || s.eq_ignore_ascii_case("null") || s.eq_ignore_ascii_case("nil") {
        return None;
    }

    // RFC3339/ISO 8601 (e.g. 2026-01-21T05:15:52.116Z)
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp());
    }

    // ISO 8601 with T, no timezone (e.g. 2025-10-24T19:12:43 or 2025-10-24T19:12:43.544)
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f") {
        return Some(dt.and_utc().timestamp());
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Some(dt.and_utc().timestamp());
    }

    // ISO date with time: YYYY-MM-DD HH:MM:SS
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Some(dt.and_utc().timestamp());
    }

    // ISO date with fractional seconds
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f") {
        return Some(dt.and_utc().timestamp());
    }

    // ISO date only: YYYY-MM-DD
    if let Ok(dt) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return dt.and_hms_opt(0, 0, 0).map(|ndt| ndt.and_utc().timestamp());
    }

    // Slash date with time: YYYY/MM/DD HH:MM:SS (e.g. 2026/01/21 05:20:35)
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y/%m/%d %H:%M:%S") {
        return Some(dt.and_utc().timestamp());
    }

    // Slash date only: YYYY/MM/DD
    if let Ok(dt) = chrono::NaiveDate::parse_from_str(s, "%Y/%m/%d") {
        return dt.and_hms_opt(0, 0, 0).map(|ndt| ndt.and_utc().timestamp());
    }

    // US format: M/D/YYYY or MM/DD/YYYY (e.g. 1/10/2026)
    if let Ok(dt) = chrono::NaiveDate::parse_from_str(s, "%m/%d/%Y") {
        return dt.and_hms_opt(0, 0, 0).map(|ndt| ndt.and_utc().timestamp());
    }

    // Compact DDMMYY (6 digits, e.g. 271125 = 27 Nov 2025)
    if s.len() == 6
        && s.chars().all(|c| c.is_ascii_digit())
        && let Ok(dt) = chrono::NaiveDate::parse_from_str(s, "%d%m%y")
    {
        return dt.and_hms_opt(0, 0, 0).map(|ndt| ndt.and_utc().timestamp());
    }

    // Unix asctime (e.g. Mon Dec  1 19:26:16 2025)
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%a %b %e %H:%M:%S %Y") {
        return Some(dt.and_utc().timestamp());
    }

    // Syslog (month day time, no year): e.g. Jan 15 00:07:12 — use current year
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() == 3 {
        let t = parts[2];
        // parts[2] should look like HH:MM:SS (or HH:MM:SS.fractional)
        if t.len() >= 8
            && t.as_bytes().get(2) == Some(&b':')
            && t.as_bytes().get(5) == Some(&b':')
            && t[..2].chars().all(|c| c.is_ascii_digit())
            && t[3..5].chars().all(|c| c.is_ascii_digit())
            && t[6..8].chars().all(|c| c.is_ascii_digit())
        {
            let with_year = format!(
                "{} {} {} {}",
                parts[0],
                parts[1],
                parts[2],
                Utc::now().year()
            );
            if let Ok(dt) = NaiveDateTime::parse_from_str(&with_year, "%b %e %H:%M:%S %Y") {
                return Some(dt.and_utc().timestamp());
            }
            // With fractional seconds (e.g. 00:07:12.123)
            if t.len() > 8
                && t.as_bytes().get(8) == Some(&b'.')
                && let Ok(dt) = NaiveDateTime::parse_from_str(&with_year, "%b %e %H:%M:%S%.f %Y")
            {
                return Some(dt.and_utc().timestamp());
            }
        }
    }

    None
}

/// Check if a value looks like a boolean
#[must_use]
pub fn is_boolean(value: &str) -> bool {
    matches!(
        value.to_lowercase().as_str(),
        "true" | "false" | "yes" | "no" | "1" | "0" | "y" | "n"
    )
}

// Unix timestamp range constants
// Seconds since epoch: Jan 1, 1970
// MIN_TIMESTAMP_SECONDS: ~2001-09-09 (1 billion seconds)
// MAX_TIMESTAMP_SECONDS: ~2096-10-02 (4 billion seconds)
const MIN_TIMESTAMP_SECONDS: i64 = 1_000_000_000;
const MAX_TIMESTAMP_SECONDS: i64 = 4_000_000_000;
const MIN_TIMESTAMP_MILLIS: i64 = 1_000_000_000_000;
const MAX_TIMESTAMP_MILLIS: i64 = 4_000_000_000_000;

const MIN_TIMESTAMP_SECONDS_F64: f64 = 1_000_000_000.0;
const MAX_TIMESTAMP_SECONDS_F64: f64 = 4_000_000_000.0;
const MIN_TIMESTAMP_MILLIS_F64: f64 = 1_000_000_000_000.0;
const MAX_TIMESTAMP_MILLIS_F64: f64 = 4_000_000_000_000.0;

/// Parse a Unix timestamp string to seconds since epoch
///
/// Handles both seconds and milliseconds, converting milliseconds to seconds.
/// Returns None if the value is not a valid Unix timestamp.
///
/// Valid ranges:
/// - Seconds: 1e9 (2001) to 4e9 (2090s)
/// - Milliseconds: 1e12 (2001) to 4e12 (2090s)
#[must_use]
pub fn parse_timestamp_to_seconds(value: &str) -> Option<i64> {
    // Try parsing as integer first (most timestamps are integers)
    if let Ok(ts) = value.parse::<i64>() {
        // Check if it's milliseconds (13 digits) and convert to seconds
        if (MIN_TIMESTAMP_MILLIS..=MAX_TIMESTAMP_MILLIS).contains(&ts) {
            return Some(ts / 1000);
        }
        // Already in seconds
        if (MIN_TIMESTAMP_SECONDS..=MAX_TIMESTAMP_SECONDS).contains(&ts) {
            return Some(ts);
        }
    }
    // Try parsing as float (less common but possible)
    if let Ok(ts) = value.parse::<f64>() {
        if (MIN_TIMESTAMP_MILLIS_F64..=MAX_TIMESTAMP_MILLIS_F64).contains(&ts) {
            return Some((ts / 1000.0) as i64);
        }
        if (MIN_TIMESTAMP_SECONDS_F64..=MAX_TIMESTAMP_SECONDS_F64).contains(&ts) {
            return Some(ts as i64);
        }
    }
    None
}

/// Returns true if the string contains any contiguous digit run of length 10–13 that parses as a Unix timestamp (seconds or milliseconds).
/// Used for `has_timestamps` heuristics when timestamps are embedded in a token (e.g. minified JSON).
#[must_use]
pub fn contains_unix_timestamp(s: &str) -> bool {
    let b = s.as_bytes();
    for len in [10, 11, 12, 13] {
        let Some(end) = b.len().checked_sub(len) else {
            continue;
        };
        for start in 0..=end {
            let slice = &b[start..start + len];
            if slice.iter().all(|&c| c.is_ascii_digit())
                && let Ok(sub) = std::str::from_utf8(slice)
                && parse_timestamp_to_seconds(sub).is_some()
            {
                return true;
            }
        }
    }
    false
}

/// Check if a value looks like a number (integer or float)
#[must_use]
pub fn is_number(value: &str) -> bool {
    // Skip if it's a timestamp (timestamps are numbers but should be treated separately)
    if parse_timestamp_to_seconds(value).is_some() {
        return false;
    }
    // Try parsing as integer first
    if value.parse::<i64>().is_ok() {
        return true;
    }
    // Try parsing as float
    if value.parse::<f64>().is_ok() {
        return true;
    }
    false
}

static ISO_DATE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\d{4}-\d{2}-\d{2}(?:\s+\d{2}:\d{2}:\d{2}(?:\.\d+)?)?$")
        .expect("ISO_DATE_REGEX pattern is valid")
});

static ISO_DATE_T_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:[+-]\d{2}:\d{2}|Z)?$")
        .expect("ISO_DATE_T_REGEX pattern is valid")
});

static US_DATE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\d{1,2}[/-]\d{1,2}[/-]\d{4}$").expect("US_DATE_REGEX pattern is valid")
});

/// Check if a value looks like a date
///
/// Supports:
/// - ISO 8601: YYYY-MM-DD, YYYY-MM-DD HH:MM:SS, or YYYY-MM-DDTHH:MM:SS (with optional timezone)
/// - US/European format: MM/DD/YYYY or DD/MM/YYYY (can't distinguish without context)
#[must_use]
pub fn is_date(value: &str) -> bool {
    // ISO 8601 with space separator: YYYY-MM-DD or YYYY-MM-DD HH:MM:SS
    if ISO_DATE_REGEX.is_match(value) {
        return true;
    }
    // ISO 8601 with T separator: YYYY-MM-DDTHH:MM:SS (with optional timezone)
    if ISO_DATE_T_REGEX.is_match(value) {
        return true;
    }
    // US/European format: MM/DD/YYYY or DD/MM/YYYY (can't distinguish without context)
    US_DATE_REGEX.is_match(value)
}

/// Check if stderr is a TTY
#[must_use]
pub fn is_stderr_tty() -> bool {
    std::io::IsTerminal::is_terminal(&std::io::stderr())
}
