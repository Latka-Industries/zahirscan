//! Core result structures for template mining and parsing

use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};
use std::collections::BTreeMap;

use crate::engine::chunking::ProcessingTask;

#[allow(clippy::wildcard_imports)]
use super::metadata::*;
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
    /// Examples of values for each placeholder (`BTreeMap` for sorted keys)
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
#[derive(Debug, Clone, Default, Deserialize)]
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
    /// `SQLite` metadata (Mode 2 only, for `SQLite` database files)
    pub sqlite_metadata: Option<SqliteMetadata>,
    /// TOML metadata (Mode 2 only, for TOML config files)
    pub toml_metadata: Option<TomlMetadata>,
    /// ZIP metadata (Mode 2 only, for ZIP archives)
    pub zip_metadata: Option<ZipMetadata>,
    /// XML metadata (Mode 2 only, for XML files)
    pub xml_metadata: Option<XmlMetadata>,
    /// HTML metadata (Mode 2 only, for HTML files)
    pub html_metadata: Option<HtmlMetadata>,
    /// YAML metadata (Mode 2 only, for YAML files)
    pub yaml_metadata: Option<YamlMetadata>,
    /// INI metadata (Mode 2 only, for INI/.cfg config files)
    pub ini_metadata: Option<IniMetadata>,
    /// PPTX metadata (Mode 2 only, for `PowerPoint` files)
    pub pptx_metadata: Option<PptxMetadata>,
    /// EPUB metadata (Mode 2 only, for e-book files)
    pub epub_metadata: Option<EpubMetadata>,
    /// Archive metadata (Mode 2 only, for TAR / tar.gz / tar.bz2 / tar.xz)
    pub archive_metadata: Option<ArchiveMetadata>,
    /// Code/script metadata (Mode 2 only, for source code files)
    pub code_metadata: Option<CodeMetadata>,
    /// Log file metadata (Mode 2 only, for log files)
    pub log_metadata: Option<LogMetadata>,
    /// JSON file metadata (Mode 2 only, for JSON files)
    pub json_metadata: Option<JsonMetadata>,
    /// Parquet metadata (Mode 2 only)
    pub parquet_metadata: Option<super::metadata::ParquetMetadata>,
    /// Arrow IPC / Feather metadata (Mode 2 only)
    pub arrow_ipc_metadata: Option<super::metadata::ArrowIpcMetadata>,
    /// Avro OCF metadata (Mode 2 only)
    pub avro_metadata: Option<super::metadata::AvroMetadata>,
    /// ORC metadata (Mode 2 only)
    pub orc_metadata: Option<super::metadata::OrcMetadata>,
    /// `NumPy` `.npy` metadata (Mode 2 only)
    pub npy_metadata: Option<super::metadata::NpyMetadata>,
    /// `NumPy` `.npz` metadata (Mode 2 only)
    pub npz_metadata: Option<super::metadata::NpzMetadata>,
    /// HDF5 metadata (Mode 2 only)
    pub hdf5_metadata: Option<super::metadata::Hdf5Metadata>,
    /// `NetCDF` metadata (Mode 2 only)
    pub netcdf_metadata: Option<super::metadata::NetCdfMetadata>,
    /// Matrix Market `.mtx` metadata (Mode 2 only)
    pub mtx_metadata: Option<super::metadata::MtxMetadata>,
    /// MATLAB `.mat` metadata (Mode 2 only)
    pub mat_metadata: Option<super::metadata::MatMetadata>,
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
        // Maximum 39 fields: 1 required (templates) + 38 optional fields
        let mut state = serializer.serialize_struct("Output", 39)?;

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
        crate::serialize_optional!(state, self.toml_metadata, "toml_metadata");
        crate::serialize_optional!(state, self.zip_metadata, "zip_metadata");
        crate::serialize_optional!(state, self.xml_metadata, "xml_metadata");
        crate::serialize_optional!(state, self.html_metadata, "html_metadata");
        crate::serialize_optional!(state, self.yaml_metadata, "yaml_metadata");
        crate::serialize_optional!(state, self.ini_metadata, "ini_metadata");
        crate::serialize_optional!(state, self.pptx_metadata, "pptx_metadata");
        crate::serialize_optional!(state, self.epub_metadata, "epub_metadata");
        crate::serialize_optional!(state, self.archive_metadata, "archive_metadata");
        crate::serialize_optional!(state, self.code_metadata, "code_metadata");
        crate::serialize_optional!(state, self.log_metadata, "log_metadata");
        crate::serialize_optional!(state, self.json_metadata, "json_metadata");
        crate::serialize_optional!(state, self.parquet_metadata, "parquet_metadata");
        crate::serialize_optional!(state, self.arrow_ipc_metadata, "arrow_ipc_metadata");
        crate::serialize_optional!(state, self.avro_metadata, "avro_metadata");
        crate::serialize_optional!(state, self.orc_metadata, "orc_metadata");
        crate::serialize_optional!(state, self.npy_metadata, "npy_metadata");
        crate::serialize_optional!(state, self.npz_metadata, "npz_metadata");
        crate::serialize_optional!(state, self.hdf5_metadata, "hdf5_metadata");
        crate::serialize_optional!(state, self.netcdf_metadata, "netcdf_metadata");
        crate::serialize_optional!(state, self.mtx_metadata, "mtx_metadata");
        crate::serialize_optional!(state, self.mat_metadata, "mat_metadata");

        state.end()
    }
}

impl Output {
    /// Create Mode 1 output (templates + writing footprint if available)
    #[must_use]
    pub fn templates_only(
        templates: Vec<Template>,
        source: Option<String>,
        file_type: Option<String>,
    ) -> Self {
        Self {
            templates,
            source,
            file_type,
            ..Default::default()
        }
    }

    /// Create Mode 2 output (full metadata)
    #[must_use]
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
            ..Default::default()
        }
    }
}

/// Result of Phase 1 scan: valid tasks and paths that failed (for TUI/lib to display).
#[derive(Debug, Default)]
pub struct Phase1Result {
    pub tasks: Vec<ProcessingTask>,
    /// Paths that failed during initial scan, with error message (e.g. for TUI fallback).
    pub failed: Vec<(String, String)>,
}

/// Result of Phase 2: successful outputs and paths that failed (for TUI/lib to display).
#[derive(Debug, Clone, Default)]
pub struct Phase2Result {
    pub outputs: Vec<Output>,
    /// Paths that failed during template mining or write, with error message.
    pub failed: Vec<(String, String)>,
}

/// Result of schema extraction: outputs plus Phase 1 and Phase 2 failures (for TUI/lib to display).
#[derive(Debug, Clone, Default)]
pub struct ZahirScanResult {
    /// Successful outputs (one per file that passed Phase 1 and Phase 2).
    pub outputs: Vec<Output>,
    /// Paths that failed during Phase 1 (initial scan), with error message.
    pub phase1_failed: Vec<(String, String)>,
    /// Paths that failed during Phase 2 (template mining or write), with error message.
    pub phase2_failed: Vec<(String, String)>,
}
