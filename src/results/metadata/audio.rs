//! Audio metadata structures

use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

use super::ImageMetadata;
use crate::parsers::{BitrateMode, CompressionMode};
use crate::results::MinimalFallback;

/// Audio metadata (Mode 2 only, for audio files)
#[derive(Debug, Clone, Deserialize, Default)]
pub struct AudioMetadata {
    // Audio technical properties
    /// Duration in seconds
    pub duration_seconds: Option<f64>,
    /// Audio codec (e.g., "aac", "mp3", "opus", "flac")
    pub audio_codec: Option<String>,
    /// Audio codec profile (e.g., "LC", "HE-AAC")
    pub audio_codec_profile: Option<String>,
    /// Audio stream bitrate in bits per second (audio data only)
    pub audio_bitrate: Option<u64>,
    /// Audio channels (e.g., 2 for stereo, 6 for 5.1)
    pub audio_channels: Option<u32>,
    /// Audio channel layout (e.g., "stereo", "5.1")
    pub audio_channel_layout: Option<String>,
    /// Audio sample rate in Hz
    pub audio_sample_rate: Option<u32>,
    /// Audio language code (e.g., "eng", "jpn")
    pub audio_language: Option<String>,
    /// Container format (e.g., "mp3", "flac", "m4a", "ogg")
    pub container_format: Option<String>,
    /// Audio stream size in bytes
    pub audio_stream_size: Option<u64>,
    /// Creation/encoded date (ISO 8601 format)
    pub creation_time: Option<String>,

    // Track metadata (ID3 tags, etc.)
    /// Track title
    pub title: Option<String>,
    /// Artist name
    pub artist: Option<String>,
    /// Album name
    pub album: Option<String>,
    /// Track number (position in album)
    pub track: Option<u32>,
    /// Total tracks in album
    pub track_total: Option<u32>,
    /// Release year
    pub year: Option<u32>,
    /// Genre
    pub genre: Option<String>,
    /// Album artist
    pub album_artist: Option<String>,

    // Audio encoding details
    /// Bit depth (bits per sample, e.g., 16, 24, 32)
    pub bit_depth: Option<u32>,
    /// Compression mode (e.g., "lossy", "lossless")
    pub compression_mode: Option<CompressionMode>,
    /// Encoding library/software (e.g., "LAME", "libopus", "libvorbis")
    pub encoded_library: Option<String>,
    /// Bit rate mode (e.g., "CBR", "VBR", "ABR")
    pub bit_rate_mode: Option<BitrateMode>,
    /// Comments/notes
    pub comments: Option<String>,
    /// Album artwork/cover art metadata (if present)
    pub artwork: Option<ImageMetadata>,
}

impl Serialize for AudioMetadata {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("AudioMetadata", 22)?;
        crate::serialize_optional!(state, self.duration_seconds, "duration_seconds");
        crate::serialize_optional!(state, self.audio_codec, "audio_codec");
        crate::serialize_optional!(state, self.audio_codec_profile, "audio_codec_profile");
        crate::serialize_optional!(state, self.audio_bitrate, "audio_bitrate");
        crate::serialize_optional!(state, self.audio_channels, "audio_channels");
        crate::serialize_optional!(state, self.audio_channel_layout, "audio_channel_layout");
        crate::serialize_optional!(state, self.audio_sample_rate, "audio_sample_rate");
        crate::serialize_optional!(state, self.audio_language, "audio_language");
        crate::serialize_optional!(state, self.container_format, "container_format");
        crate::serialize_optional!(state, self.audio_stream_size, "audio_stream_size");
        crate::serialize_optional!(state, self.creation_time, "creation_time");
        crate::serialize_optional!(state, self.title, "title");
        crate::serialize_optional!(state, self.artist, "artist");
        crate::serialize_optional!(state, self.album, "album");
        crate::serialize_optional!(state, self.track, "track");
        crate::serialize_optional!(state, self.track_total, "track_total");
        crate::serialize_optional!(state, self.year, "year");
        crate::serialize_optional!(state, self.genre, "genre");
        crate::serialize_optional!(state, self.album_artist, "album_artist");
        crate::serialize_optional!(state, self.bit_depth, "bit_depth");
        crate::serialize_optional!(state, self.compression_mode, "compression_mode");
        crate::serialize_optional!(state, self.encoded_library, "encoded_library");
        crate::serialize_optional!(state, self.bit_rate_mode, "bit_rate_mode");
        crate::serialize_optional!(state, self.comments, "comments");
        crate::serialize_optional!(state, self.artwork, "artwork");
        state.end()
    }
}

crate::impl_minimal_fallback!(AudioMetadata, _);
