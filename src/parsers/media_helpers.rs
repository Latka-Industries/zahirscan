//! Shared helpers for audio and video metadata extraction

use ffprobe::{FfProbe, Stream};

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
) -> Option<String> {
    extract_bitrate_mode_from_metadata(codec, encoder)
}

/// Extract bitrate mode from encoder string or infer from codec (fallback method)
pub(crate) fn extract_bitrate_mode_from_metadata(
    codec: Option<&String>,
    encoder: Option<&String>,
) -> Option<String> {
    // First, check if it's a lossless codec (not CBR/VBR)
    if let Some(codec_str) = codec {
        let codec_lower = codec_str.to_lowercase();
        if codec_lower.contains("flac") || codec_lower.contains("alac") {
            return Some("lossless".to_string());
        }
    }

    // Check encoder string for mode hints
    // Many encoders include CBR/VBR/ABR in their encoder string
    if let Some(enc) = encoder {
        let enc_lower = enc.to_lowercase();

        // First, check for explicit mode indicators (works for any encoder)
        if enc_lower.contains("cbr") {
            return Some("CBR".to_string());
        } else if enc_lower.contains("abr") {
            return Some("ABR".to_string());
        } else if enc_lower.contains("vbr") {
            return Some("VBR".to_string());
        }

        // Opus is typically VBR
        if enc_lower.contains("opus")
            || codec
                .map(|c| c.to_lowercase().contains("opus"))
                .unwrap_or(false)
        {
            return Some("VBR".to_string());
        }
    }

    // Codec-specific heuristics
    if let Some(codec_str) = codec {
        let codec_lower = codec_str.to_lowercase();
        // Opus is almost always VBR
        if codec_lower.contains("opus") {
            return Some("VBR".to_string());
        }
        // AAC can be CBR or VBR, but many modern encoders default to VBR
        // Without additional metadata, we can't be certain
    }

    // Can't determine reliably
    None
}
