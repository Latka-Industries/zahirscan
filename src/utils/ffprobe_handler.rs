use std::path::Path;

use std::process::Command;

use anyhow::Result;
use ffprobe::{Config as FfprobeConfig, FfProbe, ffprobe_config};

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
