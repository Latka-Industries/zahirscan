//! Video file metadata extraction

use crate::config::Config;
use crate::parsers::ParseResult;
use crate::results::{MiningResult, VideoMetadata as OutputVideoMetadata};
use crate::tools::check_ffprobe_available;
use anyhow::Result;
use ffprobe::{Stream, ffprobe};

/// Extract video metadata using ffprobe
pub fn extract_video_metadata(
    _content: &[u8],
    stats: &ParseResult,
    _config: &Config,
) -> Result<OutputVideoMetadata> {
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
    // Find video and audio streams
    // ============================================================================
    let video_stream = probe_result
        .streams
        .iter()
        .find(|s| s.codec_type.as_deref() == Some("video"));
    let audio_stream = probe_result
        .streams
        .iter()
        .find(|s| s.codec_type.as_deref() == Some("audio"));

    // ============================================================================
    // Video stream metadata extraction
    // ============================================================================

    // Basic video properties
    let (width, height) = video_stream
        .and_then(|s| Some((s.width? as usize, s.height? as usize)))
        .unwrap_or((0, 0));
    let coded_width = video_stream.and_then(|s| s.coded_width.map(|w| w as usize));
    let coded_height = video_stream.and_then(|s| s.coded_height.map(|h| h as usize));
    // has_b_frames is Option<u32> (number of B-frames), convert to bool
    let has_b_frames = video_stream
        .and_then(|s| s.has_b_frames)
        .map(|count| count > 0);

    // Codec information
    let video_codec = video_stream.and_then(|s| s.codec_name.clone());
    let video_codec_profile = video_stream.and_then(|s| s.profile.clone());
    let video_codec_level = video_stream
        .and_then(|s| s.level)
        .map(|level| format!("{}.{}", level / 10, level % 10)); // Convert 40 -> "4.0", 41 -> "4.1"

    // Pixel format and color information
    let pixel_format = video_stream.and_then(|s| s.pix_fmt.clone());
    let bit_depth = extract_bit_depth(video_stream, &pixel_format);
    let color_space = video_stream.and_then(|s| s.color_space.clone());
    let chroma_subsampling = pixel_format.as_deref().and_then(extract_chroma_subsampling);

    // Aspect ratios
    let display_aspect_ratio = video_stream.and_then(|s| s.display_aspect_ratio.clone());
    let sample_aspect_ratio = video_stream.and_then(|s| s.sample_aspect_ratio.clone());

    // Language and creation time (from tags)
    let video_language = video_stream
        .and_then(|s| s.tags.as_ref())
        .and_then(|tags| tags.language.clone());
    let creation_time = video_stream
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

    // Scan type (progressive vs interlaced)
    let scan_type = video_stream
        .and_then(|s| s.field_order.as_deref())
        .and_then(extract_scan_type);

    // Frame rate and count
    let frame_rate = video_stream.and_then(|s| {
        // Try r_frame_rate first (real frame rate), fallback to avg_frame_rate
        parse_frame_rate(&s.r_frame_rate).or_else(|| parse_frame_rate(&s.avg_frame_rate))
    });
    let frame_count = video_stream
        .and_then(|s| s.nb_frames.as_ref())
        .and_then(|f| f.parse::<u64>().ok());

    // Bitrate and stream size
    let video_bitrate = video_stream
        .and_then(|s| s.bit_rate.as_ref())
        .and_then(|b| b.parse::<u64>().ok());
    let video_stream_size = video_bitrate
        .zip(duration_seconds)
        .map(|(vb, dur)| (vb as f64 * dur / 8.0) as u64); // Convert bits to bytes

    // Encoding information
    let encoded_library = video_stream
        .and_then(|s| s.tags.as_ref())
        .and_then(|tags| tags.encoder.clone());

    // ============================================================================
    // Audio stream metadata extraction
    // ============================================================================
    let audio_codec = audio_stream.and_then(|s| s.codec_name.clone());
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
    let audio_language = audio_stream
        .and_then(|s| s.tags.as_ref())
        .and_then(|tags| tags.language.clone());

    // ============================================================================
    // Derived metadata
    // ============================================================================
    let aspect_ratio = (height > 0).then(|| width as f64 / height as f64);

    Ok(OutputVideoMetadata {
        width,
        height,
        aspect_ratio,
        display_aspect_ratio,
        sample_aspect_ratio,
        coded_width,
        coded_height,
        has_b_frames,
        video_language,
        creation_time,
        duration_seconds,
        video_codec,
        video_codec_profile,
        video_codec_level,
        pixel_format,
        bit_depth,
        color_space,
        chroma_subsampling,
        scan_type,
        frame_rate,
        frame_count,
        bitrate,
        video_bitrate,
        bitrate_mode: None, // Not directly available from ffprobe Format struct
        audio_codec,
        audio_bitrate,
        audio_channels,
        audio_channel_layout,
        audio_sample_rate,
        audio_language,
        container_format,
        video_stream_size,
        audio_stream_size,
        stream_size: Some(stats.byte_count),
        encoded_library,
    })
}

/// Extract bit depth from stream or derive from pixel format
fn extract_bit_depth(stream: Option<&Stream>, pixel_format: &Option<String>) -> Option<u32> {
    stream
        .and_then(|s| s.bits_per_raw_sample.as_ref())
        .and_then(|b| b.parse::<u32>().ok())
        .or_else(|| {
            pixel_format.as_ref().and_then(|pix_fmt| {
                if pix_fmt.contains("10") {
                    Some(10)
                } else if pix_fmt.contains("12") {
                    Some(12)
                } else if pix_fmt.contains("14") {
                    Some(14)
                } else if pix_fmt.contains("16") {
                    Some(16)
                } else if !pix_fmt.contains("8") {
                    Some(8)
                } else {
                    None
                }
            })
        })
}

/// Extract chroma subsampling from pixel format
///
/// Derives chroma subsampling ratio from pixel format name.
/// Examples: yuv420p -> "4:2:0", yuv422p -> "4:2:2"
fn extract_chroma_subsampling(pix_fmt: &str) -> Option<String> {
    if pix_fmt.contains("420") {
        Some("4:2:0".to_string())
    } else if pix_fmt.contains("422") {
        Some("4:2:2".to_string())
    } else if pix_fmt.contains("444") {
        Some("4:4:4".to_string())
    } else if pix_fmt.contains("411") {
        Some("4:1:1".to_string())
    } else {
        None
    }
}

/// Extract scan type from field order
///
/// Converts ffprobe field_order values to human-readable scan type strings.
/// - "progressive" -> progressive scan
/// - "tt"/"tb" -> interlaced (top field first)
/// - "bb"/"bt" -> interlaced (bottom field first)
fn extract_scan_type(field_order: &str) -> Option<String> {
    match field_order {
        "progressive" => Some("progressive".to_string()),
        "tt" | "tb" => Some("interlaced (top field first)".to_string()),
        "bb" | "bt" => Some("interlaced (bottom field first)".to_string()),
        _ => None,
    }
}

/// Parse frame rate string into a floating-point value
///
/// Supports two formats:
/// - Fraction format: "25/1", "30000/1001" -> converts to decimal
/// - Decimal format: "29.97" -> parsed directly
fn parse_frame_rate(fr_str: &str) -> Option<f64> {
    if fr_str.is_empty() {
        return None;
    }

    if fr_str.contains('/') {
        // Fraction format: "25/1" or "30000/1001"
        let parts: Vec<&str> = fr_str.split('/').collect();
        if parts.len() == 2
            && let (Ok(num), Ok(den)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>())
            && den != 0.0
        {
            return Some(num / den);
        }
        None
    } else {
        // Decimal format: "29.97"
        fr_str.parse::<f64>().ok()
    }
}

/// Extract templates from video files (videos don't have templates, return empty result)
pub fn extract_video_templates(
    _content: &[u8],
    stats: &ParseResult,
    _config: &Config,
) -> Result<MiningResult> {
    // Videos don't have templates, return empty result
    Ok(crate::parsers::traits::empty_mining_result(stats))
}
