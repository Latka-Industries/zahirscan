use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::Config;
use crate::parsers::FileType;
use anyhow::Result;

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

/// Check if ffprobe is available on the system
///
/// Returns Ok(()) if ffprobe is available, Err with a warning logged if not.
/// This is used by video and audio parsers to check for ffprobe before attempting metadata extraction.
pub fn check_ffprobe_available() -> Result<()> {
    match Command::new("ffprobe").arg("-version").output() {
        Ok(output) if output.status.success() => Ok(()),
        _ => {
            log::warn!(
                "ffprobe not found. Media metadata extraction will be limited.\n\
                 Install ffmpeg for full metadata support:\n\
                 - macOS: brew install ffmpeg\n\
                 - Ubuntu/Debian: sudo apt-get install ffmpeg\n\
                 - Fedora: sudo dnf install ffmpeg"
            );
            Err(anyhow::anyhow!("ffprobe not available"))
        }
    }
}
