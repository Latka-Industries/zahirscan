//! Video file metadata extraction

use crate::config::Config;
use crate::parsers::{ParseResult, media_helpers};
use crate::results::VideoMetadata as OutputVideoMetadata;
use crate::tools::{check_ffprobe_available, run_ffprobe_safe};
use anyhow::Result;
use ffprobe::Stream;

/// Extract video metadata using ffprobe
pub fn extract_video_metadata(
    _content: &[u8],
    stats: &ParseResult,
    _config: &Config,
) -> Result<OutputVideoMetadata> {
    // Check if ffprobe is available before attempting extraction
    check_ffprobe_available()?;

    // Run ffprobe to get comprehensive metadata
    // Uses safe, hardcoded arguments via run_ffprobe_safe()
    let probe_result = run_ffprobe_safe(&stats.file_path)?;

    // ============================================================================
    // Format-level metadata (container information)
    // ============================================================================
    let (container_format, duration_seconds, bitrate) =
        media_helpers::extract_format_metadata(&probe_result);

    // ============================================================================
    // Find video and audio streams
    // ============================================================================
    let video_stream = media_helpers::find_stream_by_type(&probe_result, "video");
    let audio_stream = media_helpers::find_stream_by_type(&probe_result, "audio");

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
    let video_codec_profile = media_helpers::extract_codec_profile(video_stream);
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
    let video_language = media_helpers::extract_language(video_stream);
    let creation_time = media_helpers::extract_creation_time(video_stream, &probe_result);

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
    let video_bitrate = media_helpers::extract_stream_bitrate(video_stream);
    let video_stream_size = media_helpers::calculate_stream_size(video_bitrate, duration_seconds);

    // Encoding information
    let encoded_library = media_helpers::extract_encoded_library(video_stream, &probe_result);

    // Bitrate mode (CBR/VBR/ABR) - check encoder string or infer from codec
    // Note: LAME tag parsing is MP3-specific, so pass None for video files
    let bitrate_mode = media_helpers::extract_bitrate_mode(
        video_codec.as_ref(),
        encoded_library.as_ref(),
        None, // Video files don't have LAME tags
    );

    // ============================================================================
    // Audio stream metadata extraction
    // ============================================================================
    let audio_metadata = media_helpers::extract_audio_stream_metadata(audio_stream);
    let audio_codec = audio_metadata.codec;
    let audio_channels = audio_metadata.channels;
    let audio_channel_layout = audio_metadata.channel_layout;
    let audio_sample_rate = audio_metadata.sample_rate;
    let audio_bitrate = audio_metadata.bitrate;
    let audio_stream_size = media_helpers::calculate_stream_size(audio_bitrate, duration_seconds);
    let audio_language = media_helpers::extract_language(audio_stream);

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
        bitrate_mode,
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

crate::no_template_mining!(
    extract_video_templates,
    "Videos don't have templates, return empty result."
);
