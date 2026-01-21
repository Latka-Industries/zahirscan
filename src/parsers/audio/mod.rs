//! Audio file metadata extraction

use crate::config::Config;
use crate::parsers::{FileType, ParseResult, image};
use crate::results::{AudioMetadata as OutputAudioMetadata, ImageMetadata, MiningResult};
use crate::tools::check_ffprobe_available;
use anyhow::Result;
use ffprobe::ffprobe;
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

    // Bit depth (bits per sample)
    let bit_depth = audio_stream
        .and_then(|s| s.bits_per_sample)
        .map(|b| b as u32);

    // Compression mode - infer from codec (lossless: flac, alac, etc.; lossy: mp3, aac, etc.)
    let compression_mode = audio_codec.as_ref().map(|codec| {
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
    });

    // Encoded library and encode settings (from stream tags)
    // Note: ffprobe crate 0.4.0 has limited tag access, so we check encoder field
    let encoded_library = audio_stream
        .and_then(|s| s.tags.as_ref())
        .and_then(|tags| tags.encoder.clone())
        .or_else(|| {
            // Also check format-level tags
            probe_result
                .format
                .tags
                .as_ref()
                .and_then(|tags| tags.encoder.clone())
        });

    // Encode settings - same as encoded_library for now (ffprobe doesn't expose separate settings)
    let encode_settings = encoded_library.clone();

    // Bit rate mode (CBR/VBR/ABR) - infer from codec or leave None
    // Most codecs don't expose this directly in ffprobe 0.4.0
    // Could be inferred: FLAC/ALAC = lossless, MP3/AAC = usually VBR/CBR
    let bit_rate_mode: Option<String> = audio_codec.as_ref().and_then(|codec| {
        let codec_lower = codec.to_lowercase();
        if codec_lower.contains("flac") || codec_lower.contains("alac") {
            Some("lossless".to_string())
        } else {
            // For lossy codecs, we can't determine CBR/VBR without additional metadata
            None
        }
    });

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
        stream_size: Some(stats.byte_count),
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
        encode_settings,
        bit_rate_mode,
        artwork: rich_tags.artwork,
        bitrate,
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
