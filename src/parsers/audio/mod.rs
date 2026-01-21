//! Audio file metadata extraction

use crate::config::Config;
use crate::parsers::ParseResult;
use crate::results::{AudioMetadata as OutputAudioMetadata, MiningResult};
use crate::tools::check_ffprobe_available;
use anyhow::Result;
use ffprobe::ffprobe;

/// Extract audio metadata using ffprobe
pub fn extract_audio_metadata(
    _content: &[u8],
    stats: &ParseResult,
    _config: &Config,
) -> Result<OutputAudioMetadata> {
    // Check if ffprobe is available before attempting extraction
    check_ffprobe_available()?;

    // Run ffprobe to get comprehensive metadata
    let probe_result = ffprobe(&stats.file_path)?;
    let format = &probe_result.format;

    // ============================================================================
    // Format-level metadata (container information)
    // ============================================================================
    let container_format = Some(format.format_name.clone());
    let duration_seconds = format.duration.as_ref().and_then(|d| d.parse::<f64>().ok());
    let bitrate = format.bit_rate.as_ref().and_then(|b| b.parse::<u64>().ok());

    // ============================================================================
    // Find audio stream
    // ============================================================================
    let audio_stream = probe_result
        .streams
        .iter()
        .find(|s| s.codec_type.as_deref() == Some("audio"));

    // ============================================================================
    // Audio stream metadata extraction
    // ============================================================================
    let audio_codec = audio_stream.and_then(|s| s.codec_name.clone());
    let audio_codec_profile = audio_stream.and_then(|s| s.profile.clone());
    let audio_channels = audio_stream.and_then(|s| s.channels).map(|c| c as u32);
    let audio_channel_layout = audio_stream.and_then(|s| s.channel_layout.clone());
    let audio_sample_rate = audio_stream
        .and_then(|s| s.sample_rate.as_ref())
        .and_then(|sr| sr.parse::<u32>().ok());
    let audio_bitrate = audio_stream
        .and_then(|s| s.bit_rate.as_ref())
        .and_then(|b| b.parse::<u64>().ok());
    let audio_stream_size = audio_bitrate
        .zip(duration_seconds)
        .map(|(ab, dur)| (ab as f64 * dur / 8.0) as u64); // Convert bits to bytes

    // Language and creation time (from tags)
    let audio_language = audio_stream
        .and_then(|s| s.tags.as_ref())
        .and_then(|tags| tags.language.clone());
    let creation_time = audio_stream
        .and_then(|s| s.tags.as_ref())
        .and_then(|tags| tags.creation_time.clone())
        .or_else(|| {
            // Also check format-level tags
            probe_result
                .format
                .tags
                .as_ref()
                .and_then(|tags| tags.creation_time.clone())
        });

    // Additional audio-specific metadata (from format tags)
    // Note: ffprobe crate 0.4.0 FormatTags doesn't expose common ID3 tags directly
    // These would need to be extracted from the raw tags if available
    // For now, we'll leave them as None - they can be added later if needed
    let title = None;
    let artist = None;
    let album = None;
    let track = None;
    let year = None;

    Ok(OutputAudioMetadata {
        duration_seconds,
        audio_codec,
        audio_codec_profile,
        audio_bitrate,
        audio_channels,
        audio_channel_layout,
        audio_sample_rate,
        audio_language,
        container_format,
        audio_stream_size,
        stream_size: Some(stats.byte_count),
        creation_time,
        title,
        artist,
        album,
        track,
        year,
        bitrate,
    })
}

/// Extract templates from audio files (audio files don't have templates, return empty result)
pub fn extract_audio_templates(
    _content: &[u8],
    stats: &ParseResult,
    _config: &Config,
) -> Result<MiningResult> {
    // Audio files don't have templates, return empty result
    Ok(crate::parsers::traits::empty_mining_result(stats))
}
