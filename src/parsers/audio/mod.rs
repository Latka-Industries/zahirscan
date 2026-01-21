//! Audio file metadata extraction

mod mp3;

use crate::config::Config;
use crate::parsers::{FileType, ParseResult, image, media_helpers};
use crate::results::{AudioMetadata as OutputAudioMetadata, ImageMetadata, MiningResult};
use crate::tools::{check_ffprobe_available, run_ffprobe_safe};
use anyhow::Result;
use lofty::{
    file::TaggedFileExt,
    picture::PictureType,
    read_from_path,
    tag::{Accessor, ItemKey},
};

/// Rich tag metadata extracted from audio files
struct RichTags {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    album_artist: Option<String>,
    track: Option<u32>,
    track_total: Option<u32>,
    year: Option<u32>,
    genre: Option<String>,
    comments: Option<String>,
    artwork: Option<ImageMetadata>,
}

fn extract_compression_mode(codec: &str) -> String {
    let codec_lower = codec.to_lowercase();
    if codec_lower.contains("flac")
        || codec_lower.contains("alac")
        || codec_lower.contains("wavpack")
        || codec_lower.contains("ape")
    {
        "lossless".to_string()
    } else {
        "lossy".to_string()
    }
}

/// Extract audio metadata using ffprobe
pub fn extract_audio_metadata(
    _content: &[u8],
    stats: &ParseResult,
    _config: &Config,
) -> Result<OutputAudioMetadata> {
    // Check if ffprobe is available before attempting extraction
    check_ffprobe_available()?;

    // Run ffprobe to get comprehensive metadata
    // Uses safe, hardcoded arguments via run_ffprobe_safe()
    let probe_result = run_ffprobe_safe(&stats.file_path)?;

    // ============================================================================
    // Format-level metadata (container information)
    // ============================================================================
    let (container_format, duration_seconds, _bitrate) =
        media_helpers::extract_format_metadata(&probe_result);

    // ============================================================================
    // Find audio stream
    // ============================================================================
    let audio_stream = media_helpers::find_stream_by_type(&probe_result, "audio");

    // ============================================================================
    // Audio stream metadata extraction
    // ============================================================================
    let audio_metadata = media_helpers::extract_audio_stream_metadata(audio_stream);
    let audio_codec = audio_metadata.codec;
    let audio_channels = audio_metadata.channels;
    let audio_channel_layout = audio_metadata.channel_layout;
    let audio_sample_rate = audio_metadata.sample_rate;
    let audio_bitrate = audio_metadata.bitrate;
    let audio_codec_profile = media_helpers::extract_codec_profile(audio_stream);
    let audio_stream_size = media_helpers::calculate_stream_size(audio_bitrate, duration_seconds);

    // Compression mode - infer from codec (lossless: flac, alac, etc.; lossy: mp3, aac, etc.)
    let compression_mode = audio_codec
        .as_ref()
        .map(|codec| extract_compression_mode(codec));

    // Bit depth (bits per sample) - only meaningful for lossless formats
    // Lossy formats (MP3, AAC, Opus, etc.) don't have meaningful bit depth
    // as they use perceptual coding, not direct sample representation
    let bit_depth = if compression_mode.as_deref() == Some("lossless") {
        media_helpers::extract_stream_bit_depth(audio_stream)
    } else {
        None
    };

    // Encoded library (from stream tags)
    // Note: ffprobe crate 0.4.0 has limited tag access, so we check encoder field
    let encoded_library = media_helpers::extract_encoded_library(audio_stream, &probe_result);

    // Bit rate mode (CBR/VBR/ABR) - try LAME tag first, then encoder string or codec heuristics
    let bit_rate_mode = if let Some(codec_str) = audio_codec.as_ref()
        && codec_str.to_lowercase() == "mp3"
        && let Some(mode) = mp3::read_lame_tag_bitrate_mode(&stats.file_path)
    {
        Some(mode)
    } else {
        media_helpers::extract_bitrate_mode(
            audio_codec.as_ref(),
            encoded_library.as_ref(),
            None, // Not MP3, so no file path needed
        )
    };

    // Language and creation time (from tags)
    let audio_language = media_helpers::extract_language(audio_stream);
    let creation_time = media_helpers::extract_creation_time(audio_stream, &probe_result);

    // ============================================================================
    // Rich tag metadata using lofty (title, artist, album, track, track_total, year, genre, artwork)
    // ============================================================================
    let rich_tags = extract_rich_tags(&stats.file_path);

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
        creation_time,
        title: rich_tags.title,
        artist: rich_tags.artist,
        album: rich_tags.album,
        album_artist: rich_tags.album_artist,
        track: rich_tags.track,
        track_total: rich_tags.track_total,
        year: rich_tags.year,
        genre: rich_tags.genre,
        comments: rich_tags.comments,
        bit_depth,
        compression_mode,
        encoded_library,
        bit_rate_mode,
        artwork: rich_tags.artwork,
    })
}

/// Extract rich tags (title, artist, album, track, track_total, year, genre, artwork) using lofty
fn extract_rich_tags(file_path: &str) -> RichTags {
    // Try to read tags using lofty
    let tagged_file = match read_from_path(file_path) {
        Ok(tf) => tf,
        Err(e) => {
            // Silently fail - not all files have tags, or lofty might not support the format
            log::debug!("Failed to read tags from {}: {}", file_path, e);
            return RichTags {
                title: None,
                artist: None,
                album: None,
                album_artist: None,
                track: None,
                track_total: None,
                year: None,
                genre: None,
                comments: None,
                artwork: None,
            };
        }
    };

    // Get the primary tag (most common/relevant tag for the format)
    let tag = match tagged_file.primary_tag() {
        Some(t) => t,
        None => {
            // Try to get the first available tag
            match tagged_file.first_tag() {
                Some(t) => t,
                None => {
                    return RichTags {
                        title: None,
                        artist: None,
                        album: None,
                        album_artist: None,
                        track: None,
                        track_total: None,
                        year: None,
                        genre: None,
                        comments: None,
                        artwork: None,
                    };
                }
            }
        }
    };

    // Extract common tag fields
    let title = tag.title().map(|s| s.to_string());
    let artist = tag.artist().map(|s| s.to_string());
    let album = tag.album().map(|s| s.to_string());

    // Track number (position) - already returns Option<u32>
    let track = tag.track();

    // Track total - try to get as string and parse
    let track_total = tag
        .get_string(&ItemKey::TrackTotal)
        .and_then(|s| s.parse::<u32>().ok());

    // Year - returns Option<u32>
    let year = tag.year();

    // Genre - use ItemKey::Genre
    let genre = tag.get_string(&ItemKey::Genre).map(|s| s.to_string());

    // Album artist - use ItemKey::AlbumArtist
    let album_artist = tag.get_string(&ItemKey::AlbumArtist).map(|s| s.to_string());

    // Comments - use ItemKey::Comment
    let comments = tag.get_string(&ItemKey::Comment).map(|s| s.to_string());

    // Extract and analyze artwork (cover art)
    let artwork = extract_artwork(tag);

    RichTags {
        title,
        artist,
        album,
        album_artist,
        track,
        track_total,
        year,
        genre,
        comments,
        artwork,
    }
}

/// Extract and analyze artwork/cover art from audio file tags
fn extract_artwork(tag: &lofty::tag::Tag) -> Option<ImageMetadata> {
    // Look for front cover artwork
    for picture in tag.pictures() {
        if picture.pic_type() == PictureType::CoverFront {
            // Get the image data
            let image_data = picture.data();

            // Use the existing image parser to analyze the artwork
            return analyze_image_data(image_data);
        }
    }

    // If no front cover found, try any cover type
    for picture in tag.pictures() {
        if matches!(
            picture.pic_type(),
            PictureType::CoverFront
                | PictureType::CoverBack
                | PictureType::Other
                | PictureType::Artist
        ) {
            return analyze_image_data(picture.data());
        }
    }

    None
}

/// Analyze image data using the existing image parser
fn analyze_image_data(image_data: &[u8]) -> Option<ImageMetadata> {
    // Create a minimal ParseResult for the image parser
    let stats = ParseResult {
        file_path: String::new(), // Not needed for image analysis
        file_type: FileType::Image,
        line_count: 0,
        byte_count: image_data.len(),
        token_count: 0,
        duration: std::time::Duration::ZERO,
        is_binary: true,
        mining_result: None,
        image_metadata: None,
        video_metadata: None,
        audio_metadata: None,
    };

    // Use the existing image parser
    let config = Config::default();
    match image::extract_image_metadata(image_data, &stats, &config) {
        Ok(metadata) => Some(metadata),
        Err(e) => {
            log::debug!("Failed to extract artwork metadata: {}", e);
            None
        }
    }
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
