//! Probabilistic template mining and parsing
//! Main handler that routes to log or text parsers

pub mod code;
mod column_stats;
pub mod container;
pub mod media;
mod media_helpers;
mod office;
pub mod settings;
pub mod sqlite;
pub mod structured;
pub mod text;
pub mod traits;
pub use media_helpers::{BitrateMode, CompressionMode};

use crate::config::RuntimeConfig;
use crate::engine::tools::{detect_file_type, redact_path};
use crate::results::{CompressionStats, FileMetadata, MiningResult, Output, OutputMode, Template};
use anyhow::Result;
use memmap2::Mmap;
use serde_json;
use std::fs::{File, OpenOptions};
use std::io::Write;

/// Macro to extract metadata with error handling and fallback
/// Usage: extract_metadata_with_fallback!(stats.field, extract_fn, stats, MetadataType, type_name_expr)
#[macro_export]
macro_rules! extract_metadata_with_fallback {
    ($field:expr, $extract_fn:expr, $stats:expr, $metadata_type:path, $type_name:expr) => {
        $field = match $extract_fn {
            Ok(metadata) => Some(metadata),
            Err(e) => {
                // Extract clean error message (just the error code and message, not the full chain)
                let error_msg = if let Some(source) = e.source() {
                    format!("{}: {}", e, source)
                } else {
                    format!("{}", e)
                };
                log::debug!(
                    "{} metadata extraction failed for '{}': {}",
                    $type_name,
                    $stats.file_path,
                    error_msg
                );
                Some($crate::results::create_minimal_fallback::<$metadata_type>(
                    $stats.byte_count,
                ))
            }
        };
    };
}

/// Macro to handle metadata extraction (unless skipped) then run templates extractor.
/// Use from any parser mod: `crate::process_with_metadata!(stats, mmap, config, field_name, extract_meta_call, MetadataType, FileType::X, extract_templates_call)`
#[macro_export]
macro_rules! process_with_metadata {
    ($stats:expr, $mmap:expr, $config:expr, $field:ident, $extract_meta:expr, $metadata_type:path, $file_type:expr, $extract_templates:expr) => {{
        if !$config.skip_media_metadata {
            $crate::extract_metadata_with_fallback!(
                $stats.$field,
                $extract_meta,
                $stats,
                $metadata_type,
                $file_type.as_metadata_name()
            );
        }
        $extract_templates
    }};
}

/// Macro to copy all metadata fields from ParseResult to Output.
/// Usage: `copy_metadata_fields!(from_stats, to_output)`
/// This ensures all metadata fields are copied without having to manually list them.
#[macro_export]
macro_rules! copy_metadata_fields {
    ($from:expr, $to:expr) => {
        $to.image_metadata = $from.image_metadata.clone();
        $to.video_metadata = $from.video_metadata.clone();
        $to.audio_metadata = $from.audio_metadata.clone();
        $to.csv_metadata = $from.csv_metadata.clone();
        $to.pdf_metadata = $from.pdf_metadata.clone();
        $to.docx_metadata = $from.docx_metadata.clone();
        $to.sqlite_metadata = $from.sqlite_metadata.clone();
        $to.toml_metadata = $from.toml_metadata.clone();
        $to.zip_metadata = $from.zip_metadata.clone();
        $to.xml_metadata = $from.xml_metadata.clone();
        $to.html_metadata = $from.html_metadata.clone();
        $to.yaml_metadata = $from.yaml_metadata.clone();
        $to.ini_metadata = $from.ini_metadata.clone();
        $to.pptx_metadata = $from.pptx_metadata.clone();
        $to.epub_metadata = $from.epub_metadata.clone();
        $to.archive_metadata = $from.archive_metadata.clone();
        $to.code_metadata = $from.code_metadata.clone();
    };
}

/// Defines `extract_X_templates` that returns `empty_mining_result(stats)`.
/// Use for parsers that do metadata only and no template mining.
/// Usage: `crate::no_template_mining!(extract_toml_templates, "TOML is config; schema covers structure. No template mining.")`
#[macro_export]
macro_rules! no_template_mining {
    ($name:ident, $doc:expr) => {
        #[doc = $doc]
        pub fn $name(
            _content: &[u8],
            stats: &$crate::parsers::ParseResult,
            _config: &$crate::config::RuntimeConfig,
        ) -> anyhow::Result<$crate::results::MiningResult> {
            Ok($crate::parsers::traits::empty_mining_result(stats))
        }
    };
}

/// Parser category: maps to a folder/module and its `process` function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParserCategory {
    Media,      // Image | Video | Audio -> media::process
    Office,     // Docx | Xlsx | Pptx -> office::process
    Structured, // Csv | Html | Json | Epub | Pdf -> structured::process
    Settings,   // Toml | Yaml | Xml | Ini -> settings::process
    Container,  // Zip | Archive -> container::process
    Sqlite,
    Code,
}

impl ParserCategory {
    /// Dispatch to the category's parser module.
    pub fn process(
        self,
        stats: &mut ParseResult,
        mmap: &Mmap,
        config: &RuntimeConfig,
    ) -> Result<MiningResult> {
        match self {
            ParserCategory::Media => media::process(stats, mmap, config),
            ParserCategory::Office => office::process(stats, mmap, config),
            ParserCategory::Structured => structured::process(stats, mmap, config),
            ParserCategory::Settings => settings::process(stats, mmap, config),
            ParserCategory::Container => container::process(stats, mmap, config),
            ParserCategory::Sqlite => sqlite::process(stats, mmap, config),
            ParserCategory::Code => code::process(stats, mmap, config),
        }
    }
}

/// Supported file types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum FileType {
    Log = 0,
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
    Sqlite,
    Toml,
    Zip,
    Xml,
    Html,
    Yaml,
    Ini,
    Pptx,
    Epub,
    Archive,
    Code,
    #[default]
    Unknown,
}

const METADATA_NAMES: [&str; 23] = [
    "Log", "JSON", "Text", "Markdown", "Image", "Video", "Audio", "CSV", "PDF", "DOCX", "XLSX",
    "SQLite", "TOML", "ZIP", "XML", "HTML", "YAML", "INI", "PPTX", "EPUB", "Archive", "Code",
    "Unknown",
];

impl FileType {
    /// Get the string representation of the file type for metadata extraction
    pub fn as_metadata_name(&self) -> &'static str {
        METADATA_NAMES[*self as u8 as usize]
    }

    /// Check if this is a binary file type that still needs processing (metadata extraction)
    pub fn binary_needs_processing(&self) -> bool {
        self.parser_category().is_some()
    }

    /// Category for dispatch to the right parser module (Media -> media::process, etc.).
    pub fn parser_category(self) -> Option<ParserCategory> {
        match self {
            FileType::Image | FileType::Video | FileType::Audio => Some(ParserCategory::Media),
            FileType::Docx | FileType::Xlsx | FileType::Pptx => Some(ParserCategory::Office),
            FileType::Csv | FileType::Html | FileType::Json | FileType::Epub | FileType::Pdf => {
                Some(ParserCategory::Structured)
            }
            FileType::Toml | FileType::Yaml | FileType::Xml | FileType::Ini => {
                Some(ParserCategory::Settings)
            }
            FileType::Zip | FileType::Archive => Some(ParserCategory::Container),
            FileType::Sqlite => Some(ParserCategory::Sqlite),
            FileType::Code => Some(ParserCategory::Code),
            _ => None,
        }
    }
}

/// Parse result containing file statistics and metadata
#[derive(Debug, Clone, Default)]
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
    /// SQLite metadata (for SQLite database files)
    pub sqlite_metadata: Option<crate::results::SqliteMetadata>,
    /// TOML metadata (for TOML config files)
    pub toml_metadata: Option<crate::results::TomlMetadata>,
    /// ZIP metadata (for ZIP archives)
    pub zip_metadata: Option<crate::results::ZipMetadata>,
    /// XML metadata (for XML files)
    pub xml_metadata: Option<crate::results::XmlMetadata>,
    /// HTML metadata (for HTML files)
    pub html_metadata: Option<crate::results::HtmlMetadata>,
    /// YAML metadata (for YAML files)
    pub yaml_metadata: Option<crate::results::YamlMetadata>,
    /// INI metadata (for INI/.cfg config files)
    pub ini_metadata: Option<crate::results::IniMetadata>,
    /// PPTX metadata (for PowerPoint files)
    pub pptx_metadata: Option<crate::results::PptxMetadata>,
    /// EPUB metadata (for e-book files)
    pub epub_metadata: Option<crate::results::EpubMetadata>,
    /// Archive metadata (for TAR / tar.gz / tar.bz2 / tar.xz)
    pub archive_metadata: Option<crate::results::ArchiveMetadata>,
    /// Code/script metadata (for source code files)
    pub code_metadata: Option<crate::results::CodeMetadata>,
}

impl ParseResult {
    /// Helper method to set all metadata fields on an Output
    fn set_metadata_fields(&self, output: &mut Output) {
        copy_metadata_fields!(self, output);
    }

    /// Convert parse result to Output object
    pub fn to_output(&self, mode: OutputMode, config: &RuntimeConfig) -> Output {
        match mode {
            OutputMode::Templates => self.build_templates_output(config),
            OutputMode::Full => self.build_full_output(config),
        }
    }

    fn check_for_writing_footprint(&self, output: &mut Output) {
        if let Some(ref mining) = self.mining_result {
            output.writing_footprint = mining.writing_footprint.clone();
        }
    }

    /// Build templates-only output (with writing footprint and metadata)
    fn build_templates_output(&self, config: &RuntimeConfig) -> Output {
        let source_path = match config.redact_paths {
            true => redact_path(&self.file_path),
            false => self.file_path.clone(),
        };

        let templates = self
            .mining_result
            .as_ref()
            .map(|m| m.templates.clone())
            .unwrap_or_default();

        let mut output = Output::templates_only(
            templates,
            Some(source_path),
            Some(format!("{:?}", self.file_type)),
        );

        // Include writing footprint if available
        self.check_for_writing_footprint(&mut output);

        // Include media metadata if available
        self.set_metadata_fields(&mut output);
        output
    }

    /// Build full output (with all metadata and compression stats)
    fn build_full_output(&self, config: &RuntimeConfig) -> Output {
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

        let (templates, compression) = if let Some(ref mining) = self.mining_result {
            (
                mining.templates.clone(),
                CompressionStats {
                    original_tokens: mining.original_tokens,
                    compressed_tokens: mining.compressed_tokens,
                    reduction_percent: mining.token_reduction_percent,
                },
            )
        } else {
            (
                vec![],
                CompressionStats {
                    original_tokens: self.token_count,
                    compressed_tokens: 0,
                    reduction_percent: 0.0,
                },
            )
        };

        let mut output = Output::full(templates, metadata, compression);

        // Include writing footprint if available
        self.check_for_writing_footprint(&mut output);

        self.set_metadata_fields(&mut output);
        output
    }

    /// Write the parse result to an output file as JSON
    pub fn write_to_file(
        &self,
        output_path: &str,
        mode: OutputMode,
        config: &crate::config::RuntimeConfig,
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

/// Phase 1: Initial file scan (fast pass to gather file metadata).
/// Returns stats only; use [`initial_file_scan_with_mmap`] when the mmap should be reused (e.g. Phase 2).
pub fn initial_file_scan(path: &str) -> Result<ParseResult> {
    let (stats, _) = initial_file_scan_with_mmap(path)?;
    Ok(stats)
}

/// Phase 1 scan that returns the mmap so Phase 2 can reuse it (avoids double open per path).
pub(crate) fn initial_file_scan_with_mmap(path: &str) -> Result<(ParseResult, Mmap)> {
    let file_type = detect_file_type(path);
    let mmap = open_mmap(path)?;
    let byte_count = mmap.len();

    let (line_count, token_count, is_binary) = match std::str::from_utf8(&mmap) {
        Ok(content) => {
            let line_count = content.lines().count();
            let token_count = byte_count / 4;
            (line_count, token_count, false)
        }
        Err(_) => (0, 0, true),
    };

    let stats = ParseResult {
        file_path: path.to_string(),
        file_type,
        line_count,
        byte_count,
        token_count,
        duration: std::time::Duration::ZERO,
        is_binary,
        ..Default::default()
    };
    Ok((stats, mmap))
}

/// Process file types with no parser category (Log, Json, Text, Markdown, Unknown).
fn process_text_or_unknown(
    stats: &mut ParseResult,
    mmap: &Mmap,
    config: &RuntimeConfig,
) -> Result<MiningResult> {
    if stats.is_binary {
        return Ok(traits::empty_mining_result(stats));
    }
    let content = std::str::from_utf8(mmap)?;
    match stats.file_type {
        FileType::Log => text::log::extract_log_templates(content, stats, config),
        FileType::Markdown => text::markdown::extract_markdown_templates(content, stats, config),
        FileType::Text => text::plain_text::extract_text_templates(content, stats, config),
        FileType::Unknown => extract_unknown_templates(content, stats, config),
        file_type if file_type.binary_needs_processing() => unreachable!(),
        _ => unreachable!(),
    }
}

/// Extract templates from a file using probabilistic template mining.
/// If `mmap` is `Some`, it is reused (avoids double open when called from Phase 2); otherwise the file is opened.
pub fn extract_templates(
    stats: &mut ParseResult,
    config: &RuntimeConfig,
    mmap: Option<&Mmap>,
) -> Result<MiningResult> {
    let owned_mmap;
    let mmap_ref: &Mmap = match mmap {
        Some(m) => m,
        None => {
            owned_mmap = open_mmap(&stats.file_path)?;
            &owned_mmap
        }
    };
    match stats.file_type.parser_category() {
        Some(cat) => cat.process(stats, mmap_ref, config),
        None => process_text_or_unknown(stats, mmap_ref, config),
    }
}

/// Estimate compressed token count (shared utility)
pub(crate) fn estimate_compressed_tokens_with_footprint(
    templates: &[Template],
    _total_lines: usize,
    config: &RuntimeConfig,
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

fn extract_unknown_templates(
    content: &str,
    stats: &mut ParseResult,
    config: &RuntimeConfig,
) -> Result<MiningResult> {
    // Try JSON first (most structured), then log, then text
    serde_json::from_str::<serde_json::Value>(content.lines().next().unwrap_or("")).map_or_else(
        |_| text::log::extract_log_templates(content, stats, config),
        |_| structured::extract_json_templates(content, stats, config),
    )
}
