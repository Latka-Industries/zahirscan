use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::Config;
use crate::parsers::FileType;
use anyhow::Result;
use chrono::{DateTime, NaiveDateTime};
use ffprobe::{Config as FfprobeConfig, FfProbe, ffprobe_config};

/// File extension to FileType mapping
/// Organized by file type for easy maintenance
const FILE_EXTENSION_MAP: &[(&str, FileType)] = &[
    // Text formats
    ("log", FileType::Log),
    ("json", FileType::Json),
    ("txt", FileType::Text),
    ("md", FileType::Markdown),
    ("markdown", FileType::Markdown),
    // Image formats
    ("jpg", FileType::Image),
    ("jpeg", FileType::Image),
    ("png", FileType::Image),
    ("gif", FileType::Image),
    ("bmp", FileType::Image),
    ("tiff", FileType::Image),
    ("tif", FileType::Image),
    ("webp", FileType::Image),
    ("ico", FileType::Image),
    ("svg", FileType::Image),
    // Video formats
    ("mp4", FileType::Video),
    ("mkv", FileType::Video),
    ("avi", FileType::Video),
    ("mov", FileType::Video),
    ("wmv", FileType::Video),
    ("flv", FileType::Video),
    ("webm", FileType::Video),
    ("m4v", FileType::Video),
    ("3gp", FileType::Video),
    ("ogv", FileType::Video),
    // Audio formats
    ("mp3", FileType::Audio),
    ("flac", FileType::Audio),
    ("wav", FileType::Audio),
    ("m4a", FileType::Audio),
    ("aac", FileType::Audio),
    ("ogg", FileType::Audio),
    ("opus", FileType::Audio),
    ("wma", FileType::Audio),
    ("ape", FileType::Audio),
    ("dsd", FileType::Audio),
    ("dsf", FileType::Audio),
    // Other formats
    ("csv", FileType::Csv),
    ("pdf", FileType::Pdf),
];

/// Get FileType from extension using linear search
/// Returns FileType::Unknown if extension is not recognized
///
/// For ~44 extensions, linear search is faster than HashMap due to:
/// - No hash computation overhead
/// - No memory allocation
/// - Cache-friendly sequential access
/// - Most common extensions (jpg, mp3, pdf) are near the start
fn get_file_type_from_extension(extension: &str) -> FileType {
    FILE_EXTENSION_MAP
        .iter()
        .find(|(ext, _)| *ext == extension)
        .map(|(_, file_type)| *file_type)
        .unwrap_or(FileType::Unknown)
}

/// Get all file extensions for a given FileType
/// Useful for validation, documentation, or UI display
pub fn get_extensions_for_file_type(file_type: FileType) -> Vec<&'static str> {
    FILE_EXTENSION_MAP
        .iter()
        .filter_map(|(ext, ft)| if *ft == file_type { Some(*ext) } else { None })
        .collect()
}

/// Check if a codec name (string) matches any extension for a given FileType
/// Useful for checking codec names from ffprobe against our known file types
///
/// Example:
/// ```
/// use zahirscan::tools::is_codec_for_file_type;
/// use zahirscan::parsers::FileType;
///
/// assert!(is_codec_for_file_type("mp3", FileType::Audio));
/// assert!(is_codec_for_file_type("flac", FileType::Audio));
/// assert!(!is_codec_for_file_type("mp3", FileType::Video));
/// ```
pub fn is_codec_for_file_type(codec: &str, file_type: FileType) -> bool {
    let codec_lower = codec.to_lowercase();
    FILE_EXTENSION_MAP
        .iter()
        .any(|(ext, ft)| *ft == file_type && codec_lower.contains(ext))
}

/// Detect file type from extension
pub fn detect_file_type(path: &str) -> FileType {
    let extension = Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    get_file_type_from_extension(&extension)
}

/// Format duration into human-readable format
pub fn format_duration(duration: std::time::Duration) -> String {
    let secs = duration.as_secs_f64();

    if secs < 0.001 {
        format!("{:.2} μs", secs * 1_000_000.0)
    } else if secs < 1.0 {
        format!("{:.2} ms", secs * 1000.0)
    } else if secs < 60.0 {
        format!("{:.2} s", secs)
    } else {
        let mins = secs / 60.0;
        format!("{:.2} m", mins)
    }
}

/// Format bytes into human-readable format (B, KB, MB, GB, TB, PB)
pub fn format_bytes(bytes: usize) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB", "PB"];
    const THRESHOLD: f64 = 1024.0;

    if bytes == 0 {
        return "0 B".to_string();
    }

    let bytes_f64 = bytes as f64;
    let exp = (bytes_f64.ln() / THRESHOLD.ln()).floor() as usize;
    let exp = exp.min(UNITS.len() - 1);

    let value = bytes_f64 / THRESHOLD.powi(exp as i32);

    if exp == 0 {
        format!("{} {}", bytes, UNITS[exp])
    } else {
        format!("{:.2} {}", value, UNITS[exp])
    }
}

/// Sanitize filename by removing whitespace, apostrophes, commas & replacing brackets/parentheses with underscores
pub fn sanitize_filename(name: &str) -> String {
    name.chars()
        .filter_map(|c| match c {
            ' ' | '\t' | '\n' | '\r' => None,               // Remove whitespace
            '\'' | ',' => None,                             // Remove apostrophes and commas
            '{' | '}' | '[' | ']' | '(' | ')' => Some('_'), // Replace brackets/parentheses with underscore
            _ => Some(c),                                   // Keep all other characters
        })
        .collect()
}

/// Redact a file path to show only the filename (for privacy)
/// Returns `***/filename.ext` format
pub fn redact_path(path: &str) -> String {
    let filename = Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    format!("***/{}", filename)
}

/// Get temporary output path for a file
pub fn get_temp_output_path(input_path: &str, config: &Config) -> String {
    let path = Path::new(input_path);
    // Get filename with extension (e.g., "file.txt" -> "file.txt.zahirscan.out")
    let input_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("zahirscan");
    let sanitized_name = sanitize_filename(input_name);
    let temp_dir = std::env::temp_dir();
    temp_dir
        .join(format!(
            "{}.{}",
            sanitized_name,
            config.temp_file_extension()
        ))
        .to_string_lossy()
        .to_string()
}

/// Determine output path for a given input file
pub fn determine_output_path(
    input_path: &str,
    output: Option<&str>,
    output_is_dir: bool,
    config: &Config,
) -> String {
    if let Some(output) = output {
        if output_is_dir {
            // Output to folder: create filename.ext.zahirscan.out in the folder
            // Preserves original extension (e.g., "file.txt" -> "file.txt.zahirscan.out")
            let path = Path::new(input_path);
            let input_name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("zahirscan");
            let sanitized_name = sanitize_filename(input_name);
            PathBuf::from(output)
                .join(format!(
                    "{}.{}",
                    sanitized_name,
                    config.temp_file_extension()
                ))
                .to_string_lossy()
                .to_string()
        } else {
            // Single file output (only for single input)
            output.to_string()
        }
    } else {
        // No output specified, use temp file
        get_temp_output_path(input_path, config)
    }
}

/// Placeholder type for template patterns
/// Each parser uses a specific placeholder type for consistency
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceholderType {
    /// Word placeholders for text/markdown parsers (e.g., "WORD_00")
    Word,
    /// Position placeholders for log parsers (e.g., "POS_00")
    Position,
    /// Position placeholders for JSON parsers (lowercase, e.g., "pos_00")
    Pos,
    /// Column placeholders for JSON parsers (e.g., "col_00")
    Col,
    /// Header placeholders for markdown parsers (e.g., "HEADER_00")
    Header,
    /// List placeholders for markdown parsers (e.g., "LIST_00")
    List,
    /// Code block placeholders for markdown parsers (e.g., "CODE_BLOCK_00")
    CodeBlock,
    /// Paragraph placeholders for markdown parsers (e.g., "PARAGRAPH_00")
    Paragraph,
}

impl PlaceholderType {
    /// Get the string representation of the placeholder type
    pub fn as_str(self) -> &'static str {
        match self {
            PlaceholderType::Word => "WORD",
            PlaceholderType::Position => "POS",
            PlaceholderType::Pos => "pos",
            PlaceholderType::Col => "col",
            PlaceholderType::Header => "HEADER",
            PlaceholderType::List => "LIST",
            PlaceholderType::CodeBlock => "CODE_BLOCK",
            PlaceholderType::Paragraph => "PARAGRAPH",
        }
    }
}

/// Format a placeholder name with zero-padded index (e.g., "WORD_00", "POS_01")
/// Ensures proper lexicographic sorting: WORD_00, WORD_01, ..., WORD_09, WORD_10
pub fn format_placeholder(name: &str, index: usize) -> String {
    format!("{}_{:02}", name, index)
}

/// Format a placeholder using PlaceholderType enum
pub fn format_placeholder_typed(placeholder_type: PlaceholderType, index: usize) -> String {
    format_placeholder(placeholder_type.as_str(), index)
}

/// Format a bracketed placeholder for pattern strings (e.g., "[WORD_00]", "[POS_01]")
/// Ensures proper lexicographic sorting in pattern strings
pub fn format_placeholder_bracketed(name: &str, index: usize) -> String {
    format!("[{}]", format_placeholder(name, index))
}

/// Format a bracketed placeholder using PlaceholderType enum
pub fn format_placeholder_bracketed_typed(
    placeholder_type: PlaceholderType,
    index: usize,
) -> String {
    format_placeholder_bracketed(placeholder_type.as_str(), index)
}

/// Check if ffprobe is available on the system
///
/// Returns Ok(()) if ffprobe is available, Err with a warning logged if not.
/// This is used by video and audio parsers to check for ffprobe before attempting metadata extraction.
pub fn check_ffprobe_available() -> Result<()> {
    match Command::new("ffprobe").arg("-version").output() {
        Ok(output) if output.status.success() => Ok(()),
        _ => {
            log::warn!(
                "ffprobe not found. Skipping video/audio metadata extraction.\n\
                 Install FFmpeg (includes ffprobe) for full media metadata support:\n\
                 https://ffmpeg.org/download.html"
            );
            Err(anyhow::anyhow!("ffprobe not available"))
        }
    }
}

/// Run ffprobe with safe, hardcoded arguments to extract media metadata
///
/// This function uses the Config API to ensure safe argument handling.
/// The ffprobe crate hardcodes safe arguments: `-v quiet -show_format -show_streams -print_format json`
/// This prevents arbitrary argument injection and limits output to JSON format only.
///
/// # Arguments
/// * `file_path` - Path to the media file to analyze
///
/// # Returns
/// * `Ok(FfProbe)` - Successfully extracted metadata
/// * `Err` - If ffprobe fails or file cannot be analyzed
pub fn run_ffprobe_safe(file_path: impl AsRef<Path>) -> Result<FfProbe> {
    let config = FfprobeConfig::builder().build();
    ffprobe_config(config, file_path).map_err(|e| anyhow::anyhow!("ffprobe failed: {}", e))
}

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
    use std::sync::OnceLock;

    // Compile regexes once for better performance
    static ISO_DATE_REGEX: OnceLock<Regex> = OnceLock::new();
    static ISO_DATE_T_REGEX: OnceLock<Regex> = OnceLock::new();
    static US_DATE_REGEX: OnceLock<Regex> = OnceLock::new();

    // ISO 8601 with space separator: YYYY-MM-DD or YYYY-MM-DD HH:MM:SS
    let iso_regex = ISO_DATE_REGEX.get_or_init(|| {
        Regex::new(r"^\d{4}-\d{2}-\d{2}(?:\s+\d{2}:\d{2}:\d{2}(?:\.\d+)?)?$").unwrap()
    });

    // ISO 8601 with T separator (RFC3339-like): YYYY-MM-DDTHH:MM:SS (with optional timezone)
    let iso_t_regex = ISO_DATE_T_REGEX.get_or_init(|| {
        Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:[+-]\d{2}:\d{2}|Z)?$")
            .unwrap()
    });

    let us_regex =
        US_DATE_REGEX.get_or_init(|| Regex::new(r"^\d{1,2}[/-]\d{1,2}[/-]\d{4}$").unwrap());

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
