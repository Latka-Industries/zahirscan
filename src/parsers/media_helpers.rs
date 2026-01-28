//! Shared helpers for audio and video metadata extraction

use ffprobe::{FfProbe, Stream};
use serde::{Deserialize, Serialize};

/// Bitrate encoding mode for audio/video streams
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum BitrateMode {
    /// Constant Bitrate - bitrate remains constant throughout
    Cbr,
    /// Variable Bitrate - bitrate varies based on content complexity
    Vbr,
    /// Average Bitrate - target average bitrate with some variation
    Abr,
    /// Lossless encoding (no bitrate mode applies)
    Lossless,
}

/// Audio compression mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompressionMode {
    /// Lossy compression (e.g., MP3, AAC, Opus)
    Lossy,
    /// Lossless compression (e.g., FLAC, ALAC, WavPack)
    Lossless,
}

/// Audio stream metadata extracted from ffprobe
#[derive(Debug, Clone)]
pub struct AudioStreamMetadata {
    pub codec: Option<String>,
    pub channels: Option<u32>,
    pub channel_layout: Option<String>,
    pub sample_rate: Option<u32>,
    pub bitrate: Option<u64>,
}

/// Extract format-level metadata (container, duration, bitrate)
pub fn extract_format_metadata(
    probe_result: &FfProbe,
) -> (Option<String>, Option<f64>, Option<u64>) {
    let format = &probe_result.format;
    let container_format = Some(format.format_name.clone());
    let duration_seconds = format.duration.as_ref().and_then(|d| d.parse::<f64>().ok());
    let bitrate = format.bit_rate.as_ref().and_then(|b| b.parse::<u64>().ok());
    (container_format, duration_seconds, bitrate)
}

/// Find a stream by codec type
pub fn find_stream_by_type<'a>(probe_result: &'a FfProbe, codec_type: &str) -> Option<&'a Stream> {
    probe_result
        .streams
        .iter()
        .find(|s| s.codec_type.as_deref() == Some(codec_type))
}

/// Extract audio stream metadata (codec, channels, sample_rate, bitrate)
pub fn extract_audio_stream_metadata(audio_stream: Option<&Stream>) -> AudioStreamMetadata {
    let codec = audio_stream.and_then(|s| s.codec_name.clone());
    let channels = audio_stream.and_then(|s| s.channels).map(|c| c as u32);
    let channel_layout = audio_stream.and_then(|s| s.channel_layout.clone());
    let sample_rate = audio_stream
        .and_then(|s| s.sample_rate.as_ref())
        .and_then(|sr| sr.parse::<u32>().ok());
    let bitrate = audio_stream
        .and_then(|s| s.bit_rate.as_ref())
        .and_then(|b| b.parse::<u64>().ok());
    AudioStreamMetadata {
        codec,
        channels,
        channel_layout,
        sample_rate,
        bitrate,
    }
}

/// Calculate stream size from bitrate and duration
pub fn calculate_stream_size(bitrate: Option<u64>, duration_seconds: Option<f64>) -> Option<u64> {
    bitrate
        .zip(duration_seconds)
        .map(|(br, dur)| (br as f64 * dur / 8.0) as u64) // Convert bits to bytes
}

/// Extract encoded_library from stream tags, with fallback to format-level tags
pub fn extract_encoded_library(stream: Option<&Stream>, probe_result: &FfProbe) -> Option<String> {
    stream
        .and_then(|s| s.tags.as_ref())
        .and_then(|tags| tags.encoder.clone())
        .or_else(|| {
            // Also check format-level tags
            probe_result
                .format
                .tags
                .as_ref()
                .and_then(|tags| tags.encoder.clone())
        })
}

/// Extract language from stream tags
pub fn extract_language(stream: Option<&Stream>) -> Option<String> {
    stream
        .and_then(|s| s.tags.as_ref())
        .and_then(|tags| tags.language.clone())
}

/// Extract creation_time from stream tags, with fallback to format-level tags
pub fn extract_creation_time(stream: Option<&Stream>, probe_result: &FfProbe) -> Option<String> {
    stream
        .and_then(|s| s.tags.as_ref())
        .and_then(|tags| tags.creation_time.clone())
        .or_else(|| {
            // Also check format-level tags
            probe_result
                .format
                .tags
                .as_ref()
                .and_then(|tags| tags.creation_time.clone())
        })
}

/// Extract codec profile from stream
pub fn extract_codec_profile(stream: Option<&Stream>) -> Option<String> {
    stream.and_then(|s| s.profile.clone())
}

/// Extract bitrate from stream
pub fn extract_stream_bitrate(stream: Option<&Stream>) -> Option<u64> {
    stream
        .and_then(|s| s.bit_rate.as_ref())
        .and_then(|b| b.parse::<u64>().ok())
}

/// Extract bit depth (bits per sample) from stream
pub fn extract_stream_bit_depth(stream: Option<&Stream>) -> Option<u32> {
    stream.and_then(|s| s.bits_per_sample).map(|b| b as u32)
}

/// Extract bitrate mode (CBR/VBR/ABR) from encoder string or codec heuristics
///
/// This is a fallback method that works for any codec by checking encoder strings
/// and codec-specific heuristics. For MP3 files, use the audio module's
/// `mp3::read_lame_tag_bitrate_mode` function for more accurate detection.
pub fn extract_bitrate_mode(
    codec: Option<&String>,
    encoder: Option<&String>,
    _file_path: Option<&str>, // Kept for API compatibility, but MP3 parsing is now in audio module
) -> Option<BitrateMode> {
    extract_bitrate_mode_from_metadata(codec, encoder)
}

/// Extract bitrate mode from encoder string or infer from codec (fallback method)
pub(crate) fn extract_bitrate_mode_from_metadata(
    codec: Option<&String>,
    encoder: Option<&String>,
) -> Option<BitrateMode> {
    // First, check if it's a lossless codec (not CBR/VBR)
    codec
        .and_then(|codec_str| {
            use crate::parsers::media::audio;
            // Check if it's a lossless codec using the helper from audio module
            if audio::is_lossless_codec(codec_str) {
                Some(BitrateMode::Lossless)
            } else {
                None
            }
        })
        .or_else(|| {
            // Check encoder string for mode hints
            // Many encoders include CBR/VBR/ABR in their encoder string
            encoder.and_then(|enc| {
                let enc_lower = enc.to_lowercase();
                // First, check for explicit mode indicators (works for any encoder)
                if enc_lower.contains("cbr") {
                    Some(BitrateMode::Cbr)
                } else if enc_lower.contains("abr") {
                    Some(BitrateMode::Abr)
                } else if enc_lower.contains("vbr") {
                    Some(BitrateMode::Vbr)
                } else {
                    use crate::parsers::media::audio;
                    // Opus is typically VBR
                    if audio::is_opus_codec(enc)
                        || codec.map(|c| audio::is_opus_codec(c)).unwrap_or(false)
                    {
                        Some(BitrateMode::Vbr)
                    } else {
                        None
                    }
                }
            })
        })
        .or_else(|| {
            // Codec-specific heuristics
            codec.and_then(|codec_str| {
                use crate::parsers::media::audio;
                // Opus is almost always VBR
                if audio::is_opus_codec(codec_str) {
                    Some(BitrateMode::Vbr)
                } else {
                    // AAC can be CBR or VBR, but many modern encoders default to VBR
                    // Without additional metadata, we can't be certain
                    None
                }
            })
        })
    // Can't determine reliably - returns None if all checks fail
}
