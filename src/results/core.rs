//! Core result structures for template mining and parsing

use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};
use std::collections::BTreeMap;

use super::metadata::{
    AudioMetadata, DocumentMetadata, ImageMetadata, SqliteMetadata, VideoMetadata,
};
use super::writing::{CompressionStats, WritingFootprint};

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
    /// Source file path (in both modes)
    pub source: Option<String>,
    /// File type (in both modes)
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
    /// CSV metadata (Mode 2 only, for CSV files)
    pub csv_metadata: Option<super::metadata::CsvMetadata>,
    /// PDF metadata (Mode 2 only, for PDF files)
    pub pdf_metadata: Option<super::metadata::PdfMetadata>,
    /// Document metadata (Mode 2 only, for DOCX and Pages files)
    pub docx_metadata: Option<DocumentMetadata>,
    /// SQLite metadata (Mode 2 only, for SQLite database files)
    pub sqlite_metadata: Option<SqliteMetadata>,
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
        // Maximum 17 fields: 1 required (templates) + 16 optional fields
        let mut state = serializer.serialize_struct("Output", 17)?;

        // Always serialize templates
        state.serialize_field("templates", &self.templates)?;

        // Conditionally serialize optional fields (skip if None)
        crate::serialize_optional!(state, self.source, "source");
        crate::serialize_optional!(state, self.file_type, "file_type");
        crate::serialize_optional!(state, self.line_count, "line_count");
        crate::serialize_optional!(state, self.byte_count, "byte_count");
        crate::serialize_optional!(state, self.token_count, "token_count");
        crate::serialize_optional!(state, self.processing_time_ms, "processing_time_ms");
        crate::serialize_optional!(state, self.is_binary, "is_binary");
        crate::serialize_optional!(state, self.compression, "compression");
        crate::serialize_optional!(state, self.writing_footprint, "writing_footprint");
        crate::serialize_optional!(state, self.image_metadata, "image_metadata");
        crate::serialize_optional!(state, self.video_metadata, "video_metadata");
        crate::serialize_optional!(state, self.audio_metadata, "audio_metadata");
        crate::serialize_optional!(state, self.csv_metadata, "csv_metadata");
        crate::serialize_optional!(state, self.pdf_metadata, "pdf_metadata");
        crate::serialize_optional!(state, self.docx_metadata, "docx_metadata");
        crate::serialize_optional!(state, self.sqlite_metadata, "sqlite_metadata");

        state.end()
    }
}

impl Output {
    /// Create Mode 1 output (templates + writing footprint if available)
    pub fn templates_only(
        templates: Vec<Template>,
        source: Option<String>,
        file_type: Option<String>,
    ) -> Self {
        Self {
            templates,
            // Source and file_type are included in both modes
            source,
            file_type,
            line_count: None,
            byte_count: None,
            token_count: None,
            processing_time_ms: None,
            is_binary: None,
            compression: None,
            // These are set by caller if available
            writing_footprint: None,
            image_metadata: None,
            video_metadata: None,
            audio_metadata: None,
            csv_metadata: None,
            pdf_metadata: None,
            docx_metadata: None,
            sqlite_metadata: None,
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
            // Mode 2 fields from FileMetadata
            source: Some(metadata.source),
            file_type: Some(metadata.file_type),
            line_count: Some(metadata.line_count),
            byte_count: Some(metadata.byte_count),
            token_count: Some(metadata.token_count),
            processing_time_ms: Some(metadata.processing_time_ms),
            is_binary: Some(metadata.is_binary),
            compression: Some(compression),
            // These are set by caller if available
            writing_footprint: None,
            image_metadata: None,
            video_metadata: None,
            audio_metadata: None,
            csv_metadata: None,
            pdf_metadata: None,
            docx_metadata: None,
            sqlite_metadata: None,
        }
    }
}
