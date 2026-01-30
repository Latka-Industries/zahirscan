//! Utility functions for PDF metadata extraction

/// Default timezone values
mod defaults {
    pub const TIMEZONE_HOUR_MIN: &str = "00";
    pub const TIMEZONE_UTC: &str = "+00:00";
}

use defaults::{TIMEZONE_HOUR_MIN, TIMEZONE_UTC};

/// Extract PDF date string from Debug representation
/// The pdf crate's Date type formats as "Date(...)" in Debug, so we extract the inner string
fn extract_date_string_from_debug(debug_str: &str) -> &str {
    // Debug format is "Date(...)" - extract the inner content
    debug_str
        .strip_prefix("Date(")
        .and_then(|s| s.strip_suffix(")"))
        .unwrap_or(debug_str)
}

/// Extract and format a PDF date to ISO 8601 format
/// Takes a pdf::primitive::Date reference and returns Option<String> with ISO 8601 date
pub(crate) fn extract_pdf_date_to_iso8601(date: &pdf::primitive::Date) -> Option<String> {
    let debug_str = format!("{:?}", date);
    let date_str = extract_date_string_from_debug(&debug_str);
    format_pdf_date(date_str)
}

/// Extract PdfString to String, converting None to None
pub(crate) fn extract_text_str(text_str: Option<&pdf::primitive::PdfString>) -> Option<String> {
    text_str.map(|t| t.to_string_lossy())
}

/// Extract and validate a date component from a PDF date string
/// Returns the component string if valid, or None if invalid
fn extract_date_component<'a>(
    date_str: &'a str,
    start: usize,
    end: usize,
    min_len: usize,
    min_val: u8,
    max_val: u8,
    default: &'a str,
) -> Option<&'a str> {
    if date_str.len() < min_len {
        return Some(default);
    }

    let component = date_str.get(start..end)?;
    if !component.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    let num: u8 = component.parse().ok()?;
    if num < min_val || num > max_val {
        return None;
    }

    Some(component)
}

/// Format PDF date string to ISO 8601 format
/// PDF dates are in format: D:YYYYMMDDHHmmSSOHH'mm
/// Where O is + or - for timezone offset, and ' is a literal apostrophe
/// Handles partial dates (minimum YYYYMMDD required)
pub(crate) fn format_pdf_date(date_str: &str) -> Option<String> {
    // Remove "D:" prefix if present
    let date_str = date_str.strip_prefix("D:").unwrap_or(date_str);

    // Minimum required: YYYYMMDD (8 characters)
    if date_str.len() < 8 {
        return None;
    }

    // Validate and extract date components
    let year = date_str.get(0..4)?;
    if !year.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    let month = extract_date_component(date_str, 4, 6, 6, 1, 12, "01")?;
    let day = extract_date_component(date_str, 6, 8, 8, 1, 31, "01")?;
    let hour = extract_date_component(date_str, 8, 10, 10, 0, 23, TIMEZONE_HOUR_MIN)?;
    let minute = extract_date_component(date_str, 10, 12, 12, 0, 59, TIMEZONE_HOUR_MIN)?;
    let second = extract_date_component(date_str, 12, 14, 14, 0, 59, TIMEZONE_HOUR_MIN)?;

    // Parse timezone if present
    // Format: O (sign) + HH (hours) + ' (apostrophe) + mm (minutes)
    // Example: +05'30 or -08'00
    let timezone = if let Some(tz_str) = date_str.get(14..) {
        let mut chars = tz_str.chars();
        let tz_char = chars.next()?;

        if tz_char == '+' || tz_char == '-' {
            // Parse hours (2 digits)
            let tz_hour = if tz_str.len() >= 3 {
                let hour_str = tz_str.get(1..3)?;
                if hour_str.chars().all(|c| c.is_ascii_digit()) {
                    let hour_num: u8 = hour_str.parse().unwrap_or(0);
                    if hour_num <= 23 {
                        hour_str
                    } else {
                        TIMEZONE_HOUR_MIN
                    }
                } else {
                    TIMEZONE_HOUR_MIN
                }
            } else {
                TIMEZONE_HOUR_MIN
            };

            // Parse minutes (after apostrophe, 2 digits)
            let tz_min = if tz_str.len() >= 6 && tz_str.chars().nth(3) == Some('\'') {
                let min_str = tz_str.get(4..6)?;
                if min_str.chars().all(|c| c.is_ascii_digit()) {
                    let min_num: u8 = min_str.parse().unwrap_or(0);
                    if min_num <= 59 {
                        min_str
                    } else {
                        TIMEZONE_HOUR_MIN
                    }
                } else {
                    TIMEZONE_HOUR_MIN
                }
            } else {
                TIMEZONE_HOUR_MIN
            };

            Some(format!("{}{}:{}", tz_char, tz_hour, tz_min))
        } else {
            Some(TIMEZONE_UTC.to_string())
        }
    } else {
        Some(TIMEZONE_UTC.to_string())
    };

    // Format as ISO 8601: YYYY-MM-DDTHH:MM:SS+HH:MM
    Some(format!(
        "{}-{}-{}T{}:{}:{}{}",
        year,
        month,
        day,
        hour,
        minute,
        second,
        timezone.unwrap_or_else(|| TIMEZONE_UTC.to_string())
    ))
}
