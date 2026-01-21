//! Result structures for template mining and parsing

use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};
use std::collections::BTreeMap;

/// Output mode for results
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    /// Mode 1: Templates + Writing Footprint (minimal, for AI consumption and style analysis)
    /// Writing footprint is only included for text/markdown files, not logs
    Templates,
    /// Mode 2: Full metadata (for development/debugging)
    Full,
}

/// Extracted template with pattern and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    /// Template pattern with placeholders (e.g., "[DATE] [TIME] ERROR: Process [PID] failed")
    pub pattern: String,
    /// Number of lines matching this template
    pub count: usize,
    /// Examples of values for each placeholder (BTreeMap for sorted keys)
    pub examples: BTreeMap<String, Vec<String>>,
}

/// SVO structure analysis (inferred from templates)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SVOAnalysis {
    /// Percentage of templates that show SVO-like structure (have pivot points)
    pub svo_structure_percent: f64,
    /// Average subject length (words before pivot)
    pub avg_subject_length: f64,
    /// Average object length (words after pivot)
    pub avg_object_length: f64,
    /// Most common pivot words (likely verbs/structural elements)
    pub common_pivots: Vec<String>,
}

/// Writing footprint metrics for text/markdown analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WritingFootprint {
    /// Vocabulary richness: unique words / total words (0.0-1.0)
    pub vocabulary_richness: f64,
    /// Average sentence length in words
    pub avg_sentence_length: f64,
    /// Punctuation diversity metrics
    pub punctuation: PunctuationMetrics,
    /// Template diversity: number of unique patterns
    pub template_diversity: usize,
    /// Average entropy across all templates (0.0-1.0)
    pub avg_entropy: f64,
    /// SVO structure analysis (inferred from templates)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub svo_analysis: Option<SVOAnalysis>,
}

/// Punctuation usage metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PunctuationMetrics {
    /// Percentage of sentences ending with period
    pub period_percent: f64,
    /// Percentage of sentences ending with question mark
    pub question_percent: f64,
    /// Percentage of sentences ending with exclamation
    pub exclamation_percent: f64,
    /// Percentage of sentences containing quotes (dialogue)
    pub dialogue_percent: f64,
    /// Average commas per sentence
    pub avg_commas_per_sentence: f64,
}

/// Template mining results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiningResult {
    pub templates: Vec<Template>,
    pub original_tokens: usize,
    pub compressed_tokens: usize,
    pub token_reduction_percent: f64,
    /// Writing footprint metrics (for text/markdown files)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub writing_footprint: Option<WritingFootprint>,
}

/// Unified output structure - can represent both modes
#[derive(Debug, Clone, Deserialize)]
pub struct Output {
    /// Templates (always present)
    pub templates: Vec<Template>,

    // Mode 2 (Full) fields - all optional
    /// Source file path (Mode 2 only)
    pub source: Option<String>,
    /// File type (Mode 2 only)
    pub file_type: Option<String>,
    /// Line count (Mode 2 only)
    pub line_count: Option<usize>,
    /// Byte count (Mode 2 only)
    pub byte_count: Option<usize>,
    /// Token count (Mode 2 only)
    pub token_count: Option<usize>,
    /// Processing duration in milliseconds (Mode 2 only)
    pub processing_time_ms: Option<f64>,
    /// Whether file is binary (Mode 2 only)
    pub is_binary: Option<bool>,
    /// Compression metrics (Mode 2 only)
    pub compression: Option<CompressionStats>,
    /// Writing footprint metrics (for text/markdown files, included in both modes)
    pub writing_footprint: Option<WritingFootprint>,
    /// Image metadata (Mode 2 only, for image files)
    pub image_metadata: Option<ImageMetadata>,
    /// Video metadata (Mode 2 only, for video files)
    pub video_metadata: Option<VideoMetadata>,
    /// Audio metadata (Mode 2 only, for audio files)
    pub audio_metadata: Option<AudioMetadata>,
}

/// Compression statistics (Mode 2 only)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionStats {
    pub original_tokens: usize,
    pub compressed_tokens: usize,
    pub reduction_percent: f64,
}

/// Image metadata (Mode 2 only, for image files)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ImageMetadata {
    /// Image width in pixels
    pub width: usize,
    /// Image height in pixels
    pub height: usize,
    /// Aspect ratio (width/height)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aspect_ratio: Option<f64>,
    /// File size in bytes (stream size)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_size: Option<usize>,
    /// Color type (e.g., "Rgb8", "Rgba8", "L8")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_type: Option<String>,
    /// Image format (e.g., "Jpeg", "Png", "Gif")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// Chroma subsampling (JPEG only, e.g., "4:2:0", "4:2:2", "4:4:4")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chroma_subsampling: Option<String>,
    /// Compression info (format-specific, e.g., "JPEG quality: 85")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression: Option<String>,
}

impl ImageMetadata {
    /// Create minimal fallback metadata when extraction fails
    /// Only sets the file size (stream_size), all other fields are None/0
    pub fn minimal_fallback(file_size_bytes: usize) -> Self {
        Self {
            stream_size: Some(file_size_bytes),
            ..Default::default()
        }
    }
}

/// Video metadata (Mode 2 only, for video files)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VideoMetadata {
    /// Video width in pixels
    pub width: usize,
    /// Video height in pixels
    pub height: usize,
    /// Aspect ratio (width/height)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aspect_ratio: Option<f64>,
    /// Display aspect ratio (DAR) - accounts for pixel aspect ratio
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_aspect_ratio: Option<String>,
    /// Sample aspect ratio (SAR) - pixel aspect ratio (e.g., "1:1", "4:3")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_aspect_ratio: Option<String>,
    /// Coded width in pixels (may include padding, can differ from display width)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coded_width: Option<usize>,
    /// Coded height in pixels (may include padding, can differ from display height)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coded_height: Option<usize>,
    /// Whether video uses B-frames (affects encoding complexity)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_b_frames: Option<bool>,
    /// Video language code (e.g., "eng", "jpn")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_language: Option<String>,
    /// Creation/encoded date (ISO 8601 format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
    /// Duration in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f64>,
    /// Video codec (e.g., "h264", "hevc", "vp9")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_codec: Option<String>,
    /// Video codec profile (e.g., "High", "Main", "Baseline")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_codec_profile: Option<String>,
    /// Video codec level (e.g., "4.1", "5.0")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_codec_level: Option<String>,
    /// Pixel format (e.g., "yuv420p", "yuv422p")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pixel_format: Option<String>,
    /// Bit depth (bits per sample)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bit_depth: Option<u32>,
    /// Color space (e.g., "bt709", "bt2020")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_space: Option<String>,
    /// Chroma subsampling (e.g., "4:2:0", "4:2:2")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chroma_subsampling: Option<String>,
    /// Scan type (e.g., "progressive", "interlaced")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scan_type: Option<String>,
    /// Frame rate (frames per second)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame_rate: Option<f64>,
    /// Frame count (total number of video frames)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame_count: Option<u64>,
    /// Overall bitrate in bits per second
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitrate: Option<u64>,
    /// Video stream bitrate in bits per second
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_bitrate: Option<u64>,
    /// Bitrate mode (e.g., "VBR", "CBR")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitrate_mode: Option<String>,
    /// Audio codec (e.g., "aac", "mp3", "opus")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_codec: Option<String>,
    /// Audio bitrate in bits per second
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_bitrate: Option<u64>,
    /// Audio channels (e.g., 2 for stereo, 6 for 5.1)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_channels: Option<u32>,
    /// Audio channel layout (e.g., "stereo", "5.1")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_channel_layout: Option<String>,
    /// Audio sample rate in Hz
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_sample_rate: Option<u32>,
    /// Audio language code (e.g., "eng", "jpn")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_language: Option<String>,
    /// Container format (e.g., "mp4", "mkv", "avi")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_format: Option<String>,
    /// Video stream size in bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_stream_size: Option<u64>,
    /// Audio stream size in bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_stream_size: Option<u64>,
    /// File size in bytes (total)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_size: Option<usize>,
    /// Encoded library/software (e.g., "x264", "HandBrake")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoded_library: Option<String>,
}

impl VideoMetadata {
    /// Create minimal fallback metadata when extraction fails
    /// Only sets the file size (stream_size), all other fields are None/0
    pub fn minimal_fallback(file_size_bytes: usize) -> Self {
        Self {
            stream_size: Some(file_size_bytes),
            ..Default::default()
        }
    }
}

/// Audio metadata (Mode 2 only, for audio files)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AudioMetadata {
    /// Duration in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f64>,
    /// Audio codec (e.g., "aac", "mp3", "opus", "flac")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_codec: Option<String>,
    /// Audio codec profile (e.g., "LC", "HE-AAC")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_codec_profile: Option<String>,
    /// Audio bitrate in bits per second
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_bitrate: Option<u64>,
    /// Overall bitrate in bits per second
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitrate: Option<u64>,
    /// Audio channels (e.g., 2 for stereo, 6 for 5.1)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_channels: Option<u32>,
    /// Audio channel layout (e.g., "stereo", "5.1")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_channel_layout: Option<String>,
    /// Audio sample rate in Hz
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_sample_rate: Option<u32>,
    /// Audio language code (e.g., "eng", "jpn")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_language: Option<String>,
    /// Container format (e.g., "mp3", "flac", "m4a", "ogg")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_format: Option<String>,
    /// Audio stream size in bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_stream_size: Option<u64>,
    /// File size in bytes (total)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_size: Option<usize>,
    /// Creation/encoded date (ISO 8601 format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
    /// Track title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Artist name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    /// Album name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    /// Track number
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track: Option<u32>,
    /// Release year
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<u32>,
}

impl AudioMetadata {
    /// Create minimal fallback metadata when extraction fails
    /// Only sets the file size (stream_size), all other fields are None/0
    pub fn minimal_fallback(file_size_bytes: usize) -> Self {
        Self {
            stream_size: Some(file_size_bytes),
            ..Default::default()
        }
    }
}

/// File metadata for Mode 2 output
#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub source: String,
    pub file_type: String,
    pub line_count: usize,
    pub byte_count: usize,
    pub token_count: usize,
    pub processing_time_ms: f64,
    pub is_binary: bool,
}

impl Serialize for Output {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Helper macro to conditionally serialize optional fields
        macro_rules! serialize_optional {
            ($state:expr, $field:ident, $name:literal) => {
                if let Some(ref val) = self.$field {
                    $state.serialize_field($name, val)?;
                }
            };
        }

        let mut state = serializer.serialize_struct("Output", 10)?;

        // Always serialize templates
        state.serialize_field("templates", &self.templates)?;

        // Conditionally serialize optional fields (skip if None)
        serialize_optional!(state, source, "source");
        serialize_optional!(state, file_type, "file_type");
        serialize_optional!(state, line_count, "line_count");
        serialize_optional!(state, byte_count, "byte_count");
        serialize_optional!(state, token_count, "token_count");
        serialize_optional!(state, processing_time_ms, "processing_time_ms");
        serialize_optional!(state, is_binary, "is_binary");
        serialize_optional!(state, compression, "compression");
        serialize_optional!(state, writing_footprint, "writing_footprint");
        serialize_optional!(state, image_metadata, "image_metadata");
        serialize_optional!(state, video_metadata, "video_metadata");
        serialize_optional!(state, audio_metadata, "audio_metadata");

        state.end()
    }
}

impl Output {
    /// Create Mode 1 output (templates + writing footprint if available)
    pub fn templates_only(templates: Vec<Template>) -> Self {
        Self {
            templates,
            source: None,
            file_type: None,
            line_count: None,
            byte_count: None,
            token_count: None,
            processing_time_ms: None,
            is_binary: None,
            compression: None,
            writing_footprint: None, // Set by caller if available
            image_metadata: None,    // Set by ParseResult::to_output if available
            video_metadata: None,    // Set by ParseResult::to_output if available
            audio_metadata: None,    // Set by ParseResult::to_output if available
        }
    }

    /// Create Mode 2 output (full metadata)
    pub fn full(
        templates: Vec<Template>,
        metadata: FileMetadata,
        compression: CompressionStats,
    ) -> Self {
        Self {
            templates,
            source: Some(metadata.source),
            file_type: Some(metadata.file_type),
            line_count: Some(metadata.line_count),
            byte_count: Some(metadata.byte_count),
            token_count: Some(metadata.token_count),
            processing_time_ms: Some(metadata.processing_time_ms),
            is_binary: Some(metadata.is_binary),
            compression: Some(compression),
            writing_footprint: None, // Set by ParseResult::to_output if available
            image_metadata: None,    // Set by ParseResult::to_output if available
            video_metadata: None,    // Set by ParseResult::to_output if available
            audio_metadata: None,    // Set by ParseResult::to_output if available
        }
    }
}
