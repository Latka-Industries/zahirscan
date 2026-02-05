use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::RuntimeConfig;
use crate::parsers::FileType;
use anyhow::Result;
use chrono::{DateTime, NaiveDateTime};
use ffprobe::{Config as FfprobeConfig, FfProbe, ffprobe_config};
use log::debug;

/// Macro to create a lazily-initialized static value using OnceLock
///
/// Usage:
/// ```
/// use zahirscan::cached_static;
/// use regex::Regex;
///
/// let pattern = cached_static!(PATTERN: Regex = Regex::new(r"\d+").unwrap());
/// assert!(pattern.is_match("123"));
/// ```
///
/// This expands to:
/// ```
/// use std::sync::OnceLock;
/// use regex::Regex;
///
/// static PATTERN: OnceLock<Regex> = OnceLock::new();
/// let pattern = PATTERN.get_or_init(|| Regex::new(r"\d+").unwrap());
/// ```
#[macro_export]
macro_rules! cached_static {
    ($name:ident: $ty:ty = $init:expr) => {{
        use std::sync::OnceLock;
        static $name: OnceLock<$ty> = OnceLock::new();
        $name.get_or_init(|| $init)
    }};
}

/// Expands to the full `[(ext, FileType), ...]` array. Each `Variant: "a", "b"` becomes
/// `("a", FileType::Variant), ("b", FileType::Variant)`. Macros must expand to a complete
/// expression, so everything is in one macro.
macro_rules! file_extension_map {
    (
        $(
            $ft:ident: $($e:literal),+ $(,)?
        );+ $(;)?
    ) => {
        &[
            $( $( ( $e, FileType::$ft ) ),+ ),+
        ]
    };
}

/// File extension to FileType mapping. Grouped by FileType for easier maintenance.
const FILE_EXTENSION_MAP: &[(&str, FileType)] = file_extension_map! {
    Log: "log";
    Json: "json";
    Text: "txt";
    Markdown: "md", "markdown";
    Image: "jpg", "jpeg", "png", "gif", "bmp", "tiff", "tif", "webp", "ico", "svg";
    Video: "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "3gp", "ogv";
    Audio: "mp3", "flac", "wav", "m4a", "aac", "ogg", "opus", "wma", "ape", "dsd", "dsf";
    Csv: "csv";
    Html: "html", "htm";
    Docx: "docx";
    Xlsx: "xlsx";
    Pptx: "pptx";
    Sqlite: "db", "sqlite", "sqlite3";
    Toml: "toml", "lock";
    Ini: "ini", "cfg";
    Xml: "xml";
    Yaml: "yaml", "yml";
    Zip: "zip";
    Archive: "tar", "gz", "bz2", "xz", "tgz";
    Epub: "epub";
    Pdf: "pdf";
};

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
/// use zahirscan::engine::tools::is_codec_for_file_type;
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

/// Detect file type from extension.
/// Compound extensions (e.g. .tar.gz, .tar.bz2, .tgz, .tar.xz) are checked first.
/// If extension is unknown, tries linguist (extension + filename) as fallback for code/script files.
pub fn detect_file_type(path: &str) -> FileType {
    let lo = path.to_lowercase();
    if lo.ends_with(".tar.xz")
        || lo.ends_with(".tar.bz2")
        || lo.ends_with(".tar.gz")
        || lo.ends_with(".tgz")
    {
        return FileType::Archive;
    }

    let extension = Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let file_type = get_file_type_from_extension(&extension);
    if file_type == FileType::Unknown {
        let p = Path::new(path);
        if p.exists() {
            let by_ext = linguist::detect_language_by_extension(p)
                .ok()
                .unwrap_or_default();
            if !by_ext.is_empty() {
                return FileType::Code;
            }
            let by_name = linguist::detect_language_by_filename(p)
                .ok()
                .unwrap_or_default();
            if !by_name.is_empty() {
                return FileType::Code;
            }
        }
    }
    file_type
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

/// Whether to skip a path based on `[filter]` ignore_patterns and ignore_hidden_files.
/// Patterns: exact basename (case-insensitive), `*suffix` (ends with), or `prefix*` (starts with).
/// Note: ignore_hidden_files only skips when basename starts with `.` (dotfiles). .DS_Store / Thumbs.db / temp patterns come from the default ignore_patterns list.
/// Used for both top-level file paths and ZIP entry paths.
pub fn should_ignore_path(path: &str, config: &RuntimeConfig) -> bool {
    let basename = Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if basename.is_empty() {
        return false;
    }
    if config.ignore_hidden_files && basename.starts_with('.') {
        return true;
    }
    for pat in &config.ignore_patterns {
        if pat.is_empty() {
            continue;
        }
        if pat.starts_with('*') {
            if basename.ends_with(pat.get(1..).unwrap_or("")) {
                return true;
            }
        } else if pat.ends_with('*') && pat.len() > 1 {
            if basename.starts_with(pat.get(..pat.len() - 1).unwrap_or("")) {
                return true;
            }
        } else if basename.eq_ignore_ascii_case(pat) {
            return true;
        }
    }
    false
}

/// Get temporary output path for a file
pub fn get_temp_output_path(input_path: &str, config: &RuntimeConfig) -> String {
    let path = Path::new(input_path);
    // Get filename with extension (e.g., "file.txt" -> "file.txt.zahirscan.out")
    let input_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(crate::PKG_NAME);
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

/// Determine output path for a given input file.
/// When output_dir is Some we write filename.ext.zahirscan.out there; when None we use a temp file.
pub fn determine_output_path(
    input_path: &str,
    output_dir: Option<&str>,
    config: &RuntimeConfig,
) -> String {
    if let Some(out_dir) = output_dir {
        let path = Path::new(input_path);
        let input_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(crate::PKG_NAME);
        let sanitized_name = sanitize_filename(input_name);
        PathBuf::from(out_dir)
            .join(format!(
                "{}.{}",
                sanitized_name,
                config.temp_file_extension()
            ))
            .to_string_lossy()
            .to_string()
    } else {
        get_temp_output_path(input_path, config)
    }
}

/// Placeholder type for template patterns
/// Each parser uses a specific placeholder type for consistency
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceholderType {
    /// Word placeholders for text/markdown parsers (e.g., "WORD_00")
    Word,
    /// Prefix slot in structural text patterns (e.g., "PREFIX_00")
    Prefix,
    /// Suffix slot in structural text patterns (e.g., "SUFFIX_00")
    Suffix,
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
            PlaceholderType::Prefix => "PREFIX",
            PlaceholderType::Suffix => "SUFFIX",
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

/// Format a bracketed placeholder using PlaceholderType enum.
/// Prefix and Suffix are formatted without an index (e.g. "[PREFIX]", "[SUFFIX]").
pub fn format_placeholder_bracketed_typed(
    placeholder_type: PlaceholderType,
    index: usize,
) -> String {
    match placeholder_type {
        PlaceholderType::Prefix => "[PREFIX]".to_string(),
        PlaceholderType::Suffix => "[SUFFIX]".to_string(),
        _ => format_placeholder_bracketed(placeholder_type.as_str(), index),
    }
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

/// Print a message if progress bars are enabled, and always log it with debug!
///
/// This is useful for progress-related messages that should be displayed to users
/// when progress bars are enabled, while still being logged for debugging.
pub fn print_progress_handler(message: &str, show_progress: bool) {
    debug!("{}", message);
    if show_progress {
        println!("{}", message);
    }
}

/// Check if stderr is a TTY
pub fn is_stderr_tty() -> bool {
    std::io::IsTerminal::is_terminal(&std::io::stderr())
}
