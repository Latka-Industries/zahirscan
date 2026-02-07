use chrono::{DateTime, NaiveDateTime};

/// Parse a date string into a Unix timestamp (seconds since epoch)
///
/// Tries multiple common date formats in order:
/// 1. RFC3339/ISO 8601 format (e.g., "2025-10-27T18:23:54+00:00")
/// 2. ISO date with time (e.g., "2025-10-27 18:23:54")
/// 3. ISO date with fractional seconds (e.g., "2025-10-27 18:23:54.123")
/// 4. ISO date only (e.g., "2025-10-27")
///
/// # Arguments
/// * `date_str` - Date string to parse
///
/// # Returns
/// * `Some(i64)` - Unix timestamp in seconds if parsing succeeds
/// * `None` - If the string cannot be parsed as any supported format
pub fn parse_date_to_timestamp(date_str: &str) -> Option<i64> {
    // Skip null/empty values
    if date_str.is_empty()
        || date_str.eq_ignore_ascii_case("null")
        || date_str.eq_ignore_ascii_case("nil")
    {
        return None;
    }

    // Try parsing as ISO 8601/RFC3339 format
    if let Ok(dt) = DateTime::parse_from_rfc3339(date_str) {
        return Some(dt.timestamp());
    }

    // Try parsing as ISO date with time: YYYY-MM-DD HH:MM:SS
    if let Ok(dt) = NaiveDateTime::parse_from_str(date_str, "%Y-%m-%d %H:%M:%S") {
        return Some(dt.and_utc().timestamp());
    }

    // Try parsing as ISO date with fractional seconds
    if let Ok(dt) = NaiveDateTime::parse_from_str(date_str, "%Y-%m-%d %H:%M:%S%.f") {
        return Some(dt.and_utc().timestamp());
    }

    // Try parsing as ISO date only: YYYY-MM-DD
    if let Ok(dt) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        return dt.and_hms_opt(0, 0, 0).map(|ndt| ndt.and_utc().timestamp());
    }

    None
}

/// Check if a value looks like a boolean
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

/// Check if a value looks like a number (integer or float)
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

/// Check if a value looks like a date
///
/// Supports:
/// - ISO 8601: YYYY-MM-DD, YYYY-MM-DD HH:MM:SS, or YYYY-MM-DDTHH:MM:SS (with optional timezone)
/// - US/European format: MM/DD/YYYY or DD/MM/YYYY (can't distinguish without context)
pub fn is_date(value: &str) -> bool {
    use regex::Regex;

    // Compile regexes once for better performance using cached_static macro
    let iso_regex = cached_static!(ISO_DATE_REGEX: Regex =
        Regex::new(r"^\d{4}-\d{2}-\d{2}(?:\s+\d{2}:\d{2}:\d{2}(?:\.\d+)?)?$").unwrap()
    );

    let iso_t_regex = cached_static!(ISO_DATE_T_REGEX: Regex =
        Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:[+-]\d{2}:\d{2}|Z)?$").unwrap()
    );

    let us_regex = cached_static!(US_DATE_REGEX: Regex =
        Regex::new(r"^\d{1,2}[/-]\d{1,2}[/-]\d{4}$").unwrap()
    );

    // ISO 8601 with space separator: YYYY-MM-DD or YYYY-MM-DD HH:MM:SS
    if iso_regex.is_match(value) {
        return true;
    }
    // ISO 8601 with T separator: YYYY-MM-DDTHH:MM:SS (with optional timezone)
    if iso_t_regex.is_match(value) {
        return true;
    }
    // US/European format: MM/DD/YYYY or DD/MM/YYYY (can't distinguish without context)
    if us_regex.is_match(value) {
        return true;
    }
    false
}

/// Check if stderr is a TTY
pub fn is_stderr_tty() -> bool {
    std::io::IsTerminal::is_terminal(&std::io::stderr())
}
