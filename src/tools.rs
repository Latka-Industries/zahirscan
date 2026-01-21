use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::Config;
use crate::parsers::FileType;
use anyhow::Result;
use ffprobe::{Config as FfprobeConfig, FfProbe, ffprobe_config};

/// Detect file type from extension
pub fn detect_file_type(path: &str) -> FileType {
    let extension = Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    match extension.as_str() {
        "log" => FileType::Log,
        "json" => FileType::Json,
        "txt" => FileType::Text,
        "md" | "markdown" => FileType::Markdown,
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "tiff" | "tif" | "webp" | "ico" | "svg" => {
            FileType::Image
        }
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "3gp" | "ogv" => {
            FileType::Video
        }
        "mp3" | "flac" | "wav" | "m4a" | "aac" | "ogg" | "opus" | "wma" | "ape" | "dsd" | "dsf" => {
            FileType::Audio
        }
        _ => FileType::Unknown,
    }
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
    let input_name = Path::new(input_path)
        .file_stem()
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
            // Output to folder: create filename.zahirscan.out in the folder
            let input_name = Path::new(input_path)
                .file_stem()
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
