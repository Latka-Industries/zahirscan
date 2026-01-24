//! Probabilistic template mining and parsing
//! Main handler that routes to log or text parsers

mod audio;
pub mod csv;
pub mod docx;
pub mod image;
mod media_helpers;
pub mod xlsx;
pub use media_helpers::{BitrateMode, CompressionMode};
pub mod pdf;
pub mod text;
pub mod traits;
mod video;

use crate::config::Config;
use crate::results::{CompressionStats, FileMetadata, MiningResult, Output, OutputMode, Template};
use crate::tools::{detect_file_type, redact_path};
use anyhow::Result;
use log::warn;
use memmap2::Mmap;
use serde_json;
use std::fs::{File, OpenOptions};
use std::io::Write;

/// Macro to extract metadata with error handling and fallback
/// Usage: extract_metadata_with_fallback!(stats.field, extract_fn, stats, MetadataType, type_name_expr)
macro_rules! extract_metadata_with_fallback {
    ($field:expr, $extract_fn:expr, $stats:expr, $metadata_type:path, $type_name:expr) => {
        $field = match $extract_fn {
            Ok(metadata) => Some(metadata),
            Err(e) => {
                warn!(
                    "{} metadata extraction failed for {}: {:?}",
                    $type_name, $stats.file_path, e
                );
                Some(crate::results::create_minimal_fallback::<$metadata_type>(
                    $stats.byte_count,
                ))
            }
        };
    };
}

/// Macro to handle media file processing (metadata + templates)
/// Usage: process_media_file!(stats, mmap, config, module::extract_X_metadata, module::extract_X_templates, field_name, MetadataType, FileType::X)
macro_rules! process_media_file {
    ($stats:expr, $mmap:expr, $config:expr, $extract_metadata_fn:path, $extract_templates_fn:path, $field:ident, $metadata_type:path, $file_type:expr) => {{
        // Extract metadata in Phase 2 (unless skipped)
        if !$config.skip_media_metadata {
            extract_metadata_with_fallback!(
                $stats.$field,
                $extract_metadata_fn($mmap, $stats, $config),
                $stats,
                $metadata_type,
                $file_type.as_metadata_name()
            );
        }
        $extract_templates_fn($mmap, $stats, $config)
    }};
}

/// Supported file types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Log,
    Json,
    Text,
    Markdown,
    Image,
    Video,
    Audio,
    Csv,
    Pdf,
    Docx,
    Xlsx,
    Unknown,
}

impl FileType {
    /// Get the string representation of the file type for metadata extraction
    pub fn as_metadata_name(&self) -> &'static str {
        match self {
            FileType::Image => "Image",
            FileType::Video => "Video",
            FileType::Audio => "Audio",
            FileType::Csv => "CSV",
            FileType::Pdf => "PDF",
            FileType::Docx => "DOCX",
            FileType::Xlsx => "XLSX",
            FileType::Log => "Log",
            FileType::Json => "JSON",
            FileType::Text => "Text",
            FileType::Markdown => "Markdown",
            FileType::Unknown => "Unknown",
        }
    }

    /// Check if this file type needs processing (binary files that still need metadata extraction)
    pub fn needs_processing(&self) -> bool {
        matches!(
            self,
            FileType::Image
                | FileType::Video
                | FileType::Audio
                | FileType::Pdf
                | FileType::Docx
                | FileType::Xlsx
                | FileType::Csv
        )
    }
}

/// Parse result containing file statistics and metadata
#[derive(Debug, Clone)]
pub struct ParseResult {
    pub file_path: String,
    pub file_type: FileType,
    pub line_count: usize,
    pub byte_count: usize,
    /// Estimated token count (bytes / 4), or 0 if binary
    pub token_count: usize,
    pub duration: std::time::Duration,
    /// Whether the file is binary (invalid UTF-8)
    pub is_binary: bool,
    /// Template mining results (if extracted)
    pub mining_result: Option<MiningResult>,
    /// Image metadata (for image files)
    pub image_metadata: Option<crate::results::ImageMetadata>,
    /// Video metadata (for video files)
    pub video_metadata: Option<crate::results::VideoMetadata>,
    /// Audio metadata (for audio files)
    pub audio_metadata: Option<crate::results::AudioMetadata>,
    /// CSV metadata (for CSV files)
    pub csv_metadata: Option<crate::results::CsvMetadata>,
    /// PDF metadata (for PDF files)
    pub pdf_metadata: Option<crate::results::PdfMetadata>,
    /// Document metadata (for DOCX files)
    pub docx_metadata: Option<crate::results::DocumentMetadata>,
}

impl ParseResult {
    /// Convert parse result to Output object
    pub fn to_output(&self, mode: OutputMode, config: &crate::config::Config) -> Output {
        if let Some(ref mining) = self.mining_result {
            match mode {
                OutputMode::Templates => {
                    // Mode 1: Templates + Writing Footprint (for text/markdown files)
                    // Also include media metadata (images, videos, audio) even in templates mode
                    // Include source and file_type in both modes
                    let source_path = match config.redact_paths {
                        true => redact_path(&self.file_path),
                        false => self.file_path.clone(),
                    };
                    let mut output = Output::templates_only(
                        mining.templates.clone(),
                        Some(source_path),
                        Some(format!("{:?}", self.file_type)),
                    );
                    // Include writing footprint if available (text/markdown files only, not DOCX/XLSX)
                    if self.file_type != crate::parsers::FileType::Docx
                        && self.file_type != crate::parsers::FileType::Xlsx
                    {
                        output.writing_footprint = mining.writing_footprint.clone();
                    }
                    // Include media metadata if available (images, videos, audio, CSV, PDF, DOCX)
                    output.image_metadata = self.image_metadata.clone();
                    output.video_metadata = self.video_metadata.clone();
                    output.audio_metadata = self.audio_metadata.clone();
                    output.csv_metadata = self.csv_metadata.clone();
                    output.pdf_metadata = self.pdf_metadata.clone();
                    output.docx_metadata = self.docx_metadata.clone();
                    output
                }
                OutputMode::Full => {
                    // Mode 2: Full metadata
                    // Redact path if configured (only in Full mode where source is shown)
                    let source_path = match config.redact_paths {
                        true => redact_path(&self.file_path),
                        false => self.file_path.clone(),
                    };
                    let metadata = FileMetadata {
                        source: source_path,
                        file_type: format!("{:?}", self.file_type),
                        line_count: self.line_count,
                        byte_count: self.byte_count,
                        token_count: self.token_count,
                        processing_time_ms: self.duration.as_secs_f64() * 1000.0, // Convert to milliseconds
                        is_binary: self.is_binary,
                    };
                    let compression = CompressionStats {
                        original_tokens: mining.original_tokens,
                        compressed_tokens: mining.compressed_tokens,
                        reduction_percent: mining.token_reduction_percent,
                    };
                    let mut output = Output::full(mining.templates.clone(), metadata, compression);
                    // Only include writing_footprint for text/markdown files, not DOCX/XLSX
                    if self.file_type != crate::parsers::FileType::Docx
                        && self.file_type != crate::parsers::FileType::Xlsx
                    {
                        output.writing_footprint = mining.writing_footprint.clone();
                    }
                    output.image_metadata = self.image_metadata.clone();
                    output.video_metadata = self.video_metadata.clone();
                    output.audio_metadata = self.audio_metadata.clone();
                    output.csv_metadata = self.csv_metadata.clone();
                    output.pdf_metadata = self.pdf_metadata.clone();
                    output.docx_metadata = self.docx_metadata.clone();
                    output
                }
            }
        } else {
            // No mining results (binary file or error)
            match mode {
                OutputMode::Templates => {
                    // Include media metadata even when there are no templates (e.g., pure media files)
                    // Include source and file_type in both modes
                    let source_path = match config.redact_paths {
                        true => redact_path(&self.file_path),
                        false => self.file_path.clone(),
                    };
                    let mut output = Output::templates_only(
                        vec![],
                        Some(source_path),
                        Some(format!("{:?}", self.file_type)),
                    );
                    output.image_metadata = self.image_metadata.clone();
                    output.video_metadata = self.video_metadata.clone();
                    output.audio_metadata = self.audio_metadata.clone();
                    output.csv_metadata = self.csv_metadata.clone();
                    output.pdf_metadata = self.pdf_metadata.clone();
                    output.docx_metadata = self.docx_metadata.clone();
                    output
                }
                OutputMode::Full => {
                    // Redact path if configured
                    let source_path = match config.redact_paths {
                        true => redact_path(&self.file_path),
                        false => self.file_path.clone(),
                    };
                    let metadata = FileMetadata {
                        source: source_path,
                        file_type: format!("{:?}", self.file_type),
                        line_count: self.line_count,
                        byte_count: self.byte_count,
                        token_count: self.token_count,
                        processing_time_ms: self.duration.as_secs_f64() * 1000.0,
                        is_binary: self.is_binary,
                    };
                    let compression = CompressionStats {
                        original_tokens: self.token_count,
                        compressed_tokens: 0,
                        reduction_percent: 0.0,
                    };
                    let mut output = Output::full(vec![], metadata, compression);
                    output.writing_footprint = None;
                    // Include image metadata if available
                    output.image_metadata = self.image_metadata.clone();
                    // Include video metadata if available
                    output.video_metadata = self.video_metadata.clone();
                    // Include audio metadata if available
                    output.audio_metadata = self.audio_metadata.clone();
                    // Include CSV metadata if available
                    output.csv_metadata = self.csv_metadata.clone();
                    // Include PDF metadata if available
                    output.pdf_metadata = self.pdf_metadata.clone();
                    // Include DOCX metadata if available
                    output.docx_metadata = self.docx_metadata.clone();
                    output
                }
            }
        }
    }

    /// Write the parse result to an output file as JSON
    pub fn write_to_file(
        &self,
        output_path: &str,
        mode: OutputMode,
        config: &crate::config::Config,
    ) -> Result<()> {
        let mut output_file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(output_path)?;

        // Both modes now use the Output object to include writing_footprint
        let output = self.to_output(mode, config);
        let json = serde_json::to_string_pretty(&output)?;
        writeln!(output_file, "{}", json)?;

        Ok(())
    }
}

/// Open and memory-map a file
pub(crate) fn open_mmap(path: &str) -> Result<Mmap> {
    let file = File::open(path)?;
    let mmap = unsafe { Mmap::map(&file)? };
    Ok(mmap)
}

/// Phase 1: Initial file scan (fast pass to gather file metadata)
pub fn initial_file_scan(path: &str) -> Result<ParseResult> {
    // Detect file type from extension
    let file_type = detect_file_type(path);

    // Open and memory-map the file
    let mmap = open_mmap(path)?;
    let byte_count = mmap.len();

    // Check if file is binary (invalid UTF-8)
    let (line_count, token_count, is_binary) = match std::str::from_utf8(&mmap) {
        Ok(content) => {
            // Valid UTF-8 - collect stats
            let line_count = content.lines().count();
            // Note: bytes_per_token is not available here, using default of 4
            // This is Phase 1, so we use a simple estimate
            let token_count = byte_count / 4;
            (line_count, token_count, false)
        }
        Err(_) => {
            // Binary file - return metadata only
            (0, 0, true)
        }
    };

    Ok(ParseResult {
        file_path: path.to_string(),
        file_type,
        line_count,
        byte_count,
        token_count,
        duration: std::time::Duration::ZERO, // Will be set in Phase 2
        is_binary,
        mining_result: None,
        image_metadata: None,
        video_metadata: None,
        audio_metadata: None,
        csv_metadata: None,
        pdf_metadata: None,
        docx_metadata: None,
    })
}

/// Extract templates from a file using probabilistic template mining
/// Main handler that routes to log or text parsers
/// For images, also extracts image metadata
pub fn extract_templates(stats: &mut ParseResult, config: &Config) -> Result<MiningResult> {
    // Re-open and memory-map the file
    let mmap = open_mmap(&stats.file_path)?;

    match stats.file_type {
        FileType::Image => {
            process_media_file!(
                stats,
                &mmap,
                config,
                image::extract_image_metadata,
                image::extract_image_templates,
                image_metadata,
                crate::results::ImageMetadata,
                FileType::Image
            )
        }
        FileType::Video => {
            process_media_file!(
                stats,
                &mmap,
                config,
                video::extract_video_metadata,
                video::extract_video_templates,
                video_metadata,
                crate::results::VideoMetadata,
                FileType::Video
            )
        }
        FileType::Audio => {
            process_media_file!(
                stats,
                &mmap,
                config,
                audio::extract_audio_metadata,
                audio::extract_audio_templates,
                audio_metadata,
                crate::results::AudioMetadata,
                FileType::Audio
            )
        }
        FileType::Csv => {
            process_media_file!(
                stats,
                &mmap,
                config,
                csv::extract_csv_metadata,
                csv::extract_csv_templates,
                csv_metadata,
                crate::results::CsvMetadata,
                FileType::Csv
            )
        }
        FileType::Pdf => {
            process_media_file!(
                stats,
                &mmap,
                config,
                pdf::extract_pdf_metadata,
                pdf::extract_pdf_templates,
                pdf_metadata,
                crate::results::PdfMetadata,
                FileType::Pdf
            )
        }
        FileType::Docx => {
            // Extract metadata
            if !config.skip_media_metadata {
                extract_metadata_with_fallback!(
                    stats.docx_metadata,
                    docx::extract_docx_metadata(&mmap, stats, config),
                    stats,
                    crate::results::DocumentMetadata,
                    FileType::Docx.as_metadata_name()
                );
            }
            // Extract templates
            docx::extract_docx_templates(&mmap, stats, config)
        }
        FileType::Xlsx => {
            // Extract metadata
            if !config.skip_media_metadata {
                extract_metadata_with_fallback!(
                    stats.docx_metadata,
                    xlsx::extract_xlsx_metadata(&mmap, stats, config),
                    stats,
                    crate::results::DocumentMetadata,
                    FileType::Xlsx.as_metadata_name()
                );
            }
            // Extract templates
            xlsx::extract_xlsx_templates(&mmap, stats, config)
        }
        _ => {
            // Skip binary files (non-images, videos, audio, CSV, PDF, etc)
            if stats.is_binary {
                return Ok(traits::empty_mining_result(stats));
            }

            let content = std::str::from_utf8(&mmap)?;
            match stats.file_type {
                FileType::Log => text::log::extract_log_templates(content, stats, config),
                FileType::Json => text::json::extract_json_templates(content, stats, config),
                FileType::Markdown => {
                    text::markdown::extract_markdown_templates(content, stats, config)
                }
                FileType::Text => text::plain_text::extract_text_templates(content, stats, config),
                FileType::Unknown => {
                    // Try JSON first (most structured), then log, then text
                    serde_json::from_str::<serde_json::Value>(content.lines().next().unwrap_or(""))
                        .map_or_else(
                            |_| text::log::extract_log_templates(content, stats, config),
                            |_| text::json::extract_json_templates(content, stats, config),
                        )
                }
                FileType::Image => unreachable!(),
                FileType::Video => unreachable!(),
                FileType::Audio => unreachable!(),
                FileType::Csv => unreachable!(),
                FileType::Pdf => unreachable!(),
                FileType::Docx => unreachable!(),
                FileType::Xlsx => unreachable!(),
            }
        }
    }
}

/// Estimate compressed token count (shared utility)
pub(crate) fn estimate_compressed_tokens_with_footprint(
    templates: &[Template],
    _total_lines: usize,
    config: &crate::config::Config,
    writing_footprint: Option<&crate::results::WritingFootprint>,
) -> usize {
    // Rough estimate: template patterns + examples
    let template_tokens: usize = templates
        .iter()
        .map(|t| t.pattern.split_whitespace().count())
        .sum();
    // Use max_examples_per_placeholder for estimation (but cap at 5 for estimation purposes)
    let example_limit = config.max_examples_per_placeholder.min(5);
    let example_tokens: usize = templates
        .iter()
        .flat_map(|t| t.examples.values())
        .map(|v| v.len().min(example_limit))
        .sum();

    // Estimate writing footprint tokens if present
    let footprint_tokens = if let Some(footprint) = writing_footprint {
        // Rough estimate: count significant fields in writing footprint
        // Vocabulary richness, sentence length, punctuation metrics, template diversity, entropy
        // SVO analysis: structure percent, subject/object lengths, common pivots
        let mut tokens = config.footprint_base_overhead_tokens; // Base overhead for structure
        if let Some(ref svo) = footprint.svo_analysis {
            tokens += config.footprint_svo_metrics_tokens; // SVO metrics
            tokens += svo.common_pivots.len().min(config.max_common_pivots); // Common pivots
        }
        tokens
    } else {
        0
    };

    template_tokens + example_tokens + footprint_tokens + config.json_overhead_tokens
}
