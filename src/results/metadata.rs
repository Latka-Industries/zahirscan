//! Media metadata structures (images, videos, audio)

use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

use super::MinimalFallback;

/// Image metadata (Mode 2 only, for image files)
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ImageMetadata {
    /// Image width in pixels
    pub width: usize,
    /// Image height in pixels
    pub height: usize,
    /// Aspect ratio (width/height)
    pub aspect_ratio: Option<f64>,
    /// File size in bytes (stream size)
    pub stream_size: Option<usize>,
    /// Color type (e.g., "Rgb8", "Rgba8", "L8")
    pub color_type: Option<String>,
    /// Image format (e.g., "Jpeg", "Png", "Gif")
    pub format: Option<String>,
    /// Chroma subsampling (JPEG only, e.g., "4:2:0", "4:2:2", "4:4:4")
    pub chroma_subsampling: Option<String>,
    /// Compression info (format-specific, e.g., "JPEG quality: 85")
    pub compression: Option<String>,
    /// Bit depth (bits per sample/channel, e.g., 8, 16, 24, 32)
    pub bit_depth: Option<u32>,
}

impl Serialize for ImageMetadata {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("ImageMetadata", 9)?;
        state.serialize_field("width", &self.width)?;
        state.serialize_field("height", &self.height)?;
        crate::serialize_optional!(state, self.aspect_ratio, "aspect_ratio");
        crate::serialize_optional!(state, self.stream_size, "stream_size");
        crate::serialize_optional!(state, self.color_type, "color_type");
        crate::serialize_optional!(state, self.format, "format");
        crate::serialize_optional!(state, self.chroma_subsampling, "chroma_subsampling");
        crate::serialize_optional!(state, self.compression, "compression");
        crate::serialize_optional!(state, self.bit_depth, "bit_depth");
        state.end()
    }
}

crate::impl_minimal_fallback!(ImageMetadata);

/// Video metadata (Mode 2 only, for video files)
#[derive(Debug, Clone, Deserialize, Default)]
pub struct VideoMetadata {
    /// Video width in pixels
    pub width: usize,
    /// Video height in pixels
    pub height: usize,
    /// Aspect ratio (width/height)
    pub aspect_ratio: Option<f64>,
    /// Display aspect ratio (DAR) - accounts for pixel aspect ratio
    pub display_aspect_ratio: Option<String>,
    /// Sample aspect ratio (SAR) - pixel aspect ratio (e.g., "1:1", "4:3")
    pub sample_aspect_ratio: Option<String>,
    /// Coded width in pixels (may include padding, can differ from display width)
    pub coded_width: Option<usize>,
    /// Coded height in pixels (may include padding, can differ from display height)
    pub coded_height: Option<usize>,
    /// Whether video uses B-frames (affects encoding complexity)
    pub has_b_frames: Option<bool>,
    /// Video language code (e.g., "eng", "jpn")
    pub video_language: Option<String>,
    /// Creation/encoded date (ISO 8601 format)
    pub creation_time: Option<String>,
    /// Duration in seconds
    pub duration_seconds: Option<f64>,
    /// Video codec (e.g., "h264", "hevc", "vp9")
    pub video_codec: Option<String>,
    /// Video codec profile (e.g., "High", "Main", "Baseline")
    pub video_codec_profile: Option<String>,
    /// Video codec level (e.g., "4.1", "5.0")
    pub video_codec_level: Option<String>,
    /// Pixel format (e.g., "yuv420p", "yuv422p")
    pub pixel_format: Option<String>,
    /// Bit depth (bits per sample)
    pub bit_depth: Option<u32>,
    /// Color space (e.g., "bt709", "bt2020")
    pub color_space: Option<String>,
    /// Chroma subsampling (e.g., "4:2:0", "4:2:2")
    pub chroma_subsampling: Option<String>,
    /// Scan type (e.g., "progressive", "interlaced")
    pub scan_type: Option<String>,
    /// Frame rate (frames per second)
    pub frame_rate: Option<f64>,
    /// Frame count (total number of video frames)
    pub frame_count: Option<u64>,
    /// Overall bitrate in bits per second
    pub bitrate: Option<u64>,
    /// Video stream bitrate in bits per second
    pub video_bitrate: Option<u64>,
    /// Bitrate mode (e.g., "VBR", "CBR")
    pub bitrate_mode: Option<String>,
    /// Audio codec (e.g., "aac", "mp3", "opus")
    pub audio_codec: Option<String>,
    /// Audio bitrate in bits per second
    pub audio_bitrate: Option<u64>,
    /// Audio channels (e.g., 2 for stereo, 6 for 5.1)
    pub audio_channels: Option<u32>,
    /// Audio channel layout (e.g., "stereo", "5.1")
    pub audio_channel_layout: Option<String>,
    /// Audio sample rate in Hz
    pub audio_sample_rate: Option<u32>,
    /// Audio language code (e.g., "eng", "jpn")
    pub audio_language: Option<String>,
    /// Container format (e.g., "mp4", "mkv", "avi")
    pub container_format: Option<String>,
    /// Video stream size in bytes
    pub video_stream_size: Option<u64>,
    /// Audio stream size in bytes
    pub audio_stream_size: Option<u64>,
    /// File size in bytes (total)
    pub stream_size: Option<usize>,
    /// Encoded library/software (e.g., "x264", "HandBrake")
    pub encoded_library: Option<String>,
}

impl Serialize for VideoMetadata {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("VideoMetadata", 30)?;
        state.serialize_field("width", &self.width)?;
        state.serialize_field("height", &self.height)?;
        crate::serialize_optional!(state, self.aspect_ratio, "aspect_ratio");
        crate::serialize_optional!(state, self.display_aspect_ratio, "display_aspect_ratio");
        crate::serialize_optional!(state, self.sample_aspect_ratio, "sample_aspect_ratio");
        crate::serialize_optional!(state, self.coded_width, "coded_width");
        crate::serialize_optional!(state, self.coded_height, "coded_height");
        crate::serialize_optional!(state, self.has_b_frames, "has_b_frames");
        crate::serialize_optional!(state, self.video_language, "video_language");
        crate::serialize_optional!(state, self.creation_time, "creation_time");
        crate::serialize_optional!(state, self.duration_seconds, "duration_seconds");
        crate::serialize_optional!(state, self.video_codec, "video_codec");
        crate::serialize_optional!(state, self.video_codec_profile, "video_codec_profile");
        crate::serialize_optional!(state, self.video_codec_level, "video_codec_level");
        crate::serialize_optional!(state, self.pixel_format, "pixel_format");
        crate::serialize_optional!(state, self.bit_depth, "bit_depth");
        crate::serialize_optional!(state, self.color_space, "color_space");
        crate::serialize_optional!(state, self.chroma_subsampling, "chroma_subsampling");
        crate::serialize_optional!(state, self.scan_type, "scan_type");
        crate::serialize_optional!(state, self.frame_rate, "frame_rate");
        crate::serialize_optional!(state, self.frame_count, "frame_count");
        crate::serialize_optional!(state, self.bitrate, "bitrate");
        crate::serialize_optional!(state, self.video_bitrate, "video_bitrate");
        crate::serialize_optional!(state, self.bitrate_mode, "bitrate_mode");
        crate::serialize_optional!(state, self.audio_codec, "audio_codec");
        crate::serialize_optional!(state, self.audio_bitrate, "audio_bitrate");
        crate::serialize_optional!(state, self.audio_channels, "audio_channels");
        crate::serialize_optional!(state, self.audio_channel_layout, "audio_channel_layout");
        crate::serialize_optional!(state, self.audio_sample_rate, "audio_sample_rate");
        crate::serialize_optional!(state, self.audio_language, "audio_language");
        crate::serialize_optional!(state, self.container_format, "container_format");
        crate::serialize_optional!(state, self.video_stream_size, "video_stream_size");
        crate::serialize_optional!(state, self.audio_stream_size, "audio_stream_size");
        crate::serialize_optional!(state, self.stream_size, "stream_size");
        crate::serialize_optional!(state, self.encoded_library, "encoded_library");
        state.end()
    }
}

crate::impl_minimal_fallback!(VideoMetadata);

/// Audio metadata (Mode 2 only, for audio files)
#[derive(Debug, Clone, Deserialize, Default)]
pub struct AudioMetadata {
    /// Duration in seconds
    pub duration_seconds: Option<f64>,
    /// Audio codec (e.g., "aac", "mp3", "opus", "flac")
    pub audio_codec: Option<String>,
    /// Audio codec profile (e.g., "LC", "HE-AAC")
    pub audio_codec_profile: Option<String>,
    /// Audio bitrate in bits per second
    pub audio_bitrate: Option<u64>,
    /// Overall bitrate in bits per second
    pub bitrate: Option<u64>,
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
    /// File size in bytes (total)
    pub stream_size: Option<usize>,
    /// Creation/encoded date (ISO 8601 format)
    pub creation_time: Option<String>,
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
    /// Bit depth (bits per sample, e.g., 16, 24, 32)
    pub bit_depth: Option<u32>,
    /// Compression mode (e.g., "lossy", "lossless")
    pub compression_mode: Option<String>,
    /// Encoding library/software (e.g., "LAME", "libopus", "libvorbis")
    pub encoded_library: Option<String>,
    /// Encoding settings/parameters
    pub encode_settings: Option<String>,
    /// Bit rate mode (e.g., "CBR", "VBR", "ABR")
    pub bit_rate_mode: Option<String>,
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
        let mut state = serializer.serialize_struct("AudioMetadata", 25)?;
        crate::serialize_optional!(state, self.duration_seconds, "duration_seconds");
        crate::serialize_optional!(state, self.audio_codec, "audio_codec");
        crate::serialize_optional!(state, self.audio_codec_profile, "audio_codec_profile");
        crate::serialize_optional!(state, self.audio_bitrate, "audio_bitrate");
        crate::serialize_optional!(state, self.bitrate, "bitrate");
        crate::serialize_optional!(state, self.audio_channels, "audio_channels");
        crate::serialize_optional!(state, self.audio_channel_layout, "audio_channel_layout");
        crate::serialize_optional!(state, self.audio_sample_rate, "audio_sample_rate");
        crate::serialize_optional!(state, self.audio_language, "audio_language");
        crate::serialize_optional!(state, self.container_format, "container_format");
        crate::serialize_optional!(state, self.audio_stream_size, "audio_stream_size");
        crate::serialize_optional!(state, self.stream_size, "stream_size");
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
        crate::serialize_optional!(state, self.encode_settings, "encode_settings");
        crate::serialize_optional!(state, self.bit_rate_mode, "bit_rate_mode");
        crate::serialize_optional!(state, self.comments, "comments");
        crate::serialize_optional!(state, self.artwork, "artwork");
        state.end()
    }
}

crate::impl_minimal_fallback!(AudioMetadata);
