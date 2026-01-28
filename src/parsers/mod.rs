//! Probabilistic template mining and parsing
//! Main handler that routes to log or text parsers

pub mod code;
mod column_stats;
pub mod container;
mod epub;
pub mod media;
mod media_helpers;
mod office;
pub mod pdf;
pub mod settings;
pub mod sqlite;
pub mod structured;
pub mod text;
pub mod traits;
pub use media_helpers::{BitrateMode, CompressionMode};

use crate::engine::config::Config;
use crate::engine::tools::{detect_file_type, redact_path};
use crate::results::{CompressionStats, FileMetadata, MiningResult, Output, OutputMode, Template};
use anyhow::Result;
use log::debug;
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
                // Extract clean error message (just the error code and message, not the full chain)
                let error_msg = if let Some(source) = e.source() {
                    format!("{}: {}", e, source)
                } else {
                    format!("{}", e)
                };
                debug!(
                    "{} metadata extraction failed for '{}': {}",
                    $type_name, $stats.file_path, error_msg
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
            _config: &$crate::engine::config::Config,
        ) -> anyhow::Result<$crate::results::MiningResult> {
            Ok($crate::parsers::traits::empty_mining_result(stats))
        }
    };
}

/// Supported file types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
            FileType::Sqlite => "SQLite",
            FileType::Toml => "TOML",
            FileType::Zip => "ZIP",
            FileType::Xml => "XML",
            FileType::Html => "HTML",
            FileType::Yaml => "YAML",
            FileType::Ini => "INI",
            FileType::Pptx => "PPTX",
            FileType::Epub => "EPUB",
            FileType::Archive => "Archive",
            FileType::Code => "Code",
            FileType::Log => "Log",
            FileType::Json => "JSON",
            FileType::Text => "Text",
            FileType::Markdown => "Markdown",
            FileType::Unknown => "Unknown",
        }
    }

    /// Check if this is a binary file type that still needs processing (metadata extraction)
    pub fn binary_needs_processing(&self) -> bool {
        matches!(
            self,
            FileType::Image
                | FileType::Video
                | FileType::Audio
                | FileType::Pdf
                | FileType::Docx
                | FileType::Xlsx
                | FileType::Csv
                | FileType::Sqlite
                | FileType::Toml
                | FileType::Zip
                | FileType::Xml
                | FileType::Html
                | FileType::Yaml
                | FileType::Ini
                | FileType::Pptx
                | FileType::Epub
                | FileType::Archive
                | FileType::Code
        )
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
        output.image_metadata = self.image_metadata.clone();
        output.video_metadata = self.video_metadata.clone();
        output.audio_metadata = self.audio_metadata.clone();
        output.csv_metadata = self.csv_metadata.clone();
        output.pdf_metadata = self.pdf_metadata.clone();
        output.docx_metadata = self.docx_metadata.clone();
        output.sqlite_metadata = self.sqlite_metadata.clone();
        output.toml_metadata = self.toml_metadata.clone();
        output.zip_metadata = self.zip_metadata.clone();
        output.xml_metadata = self.xml_metadata.clone();
        output.html_metadata = self.html_metadata.clone();
        output.yaml_metadata = self.yaml_metadata.clone();
        output.ini_metadata = self.ini_metadata.clone();
        output.pptx_metadata = self.pptx_metadata.clone();
        output.epub_metadata = self.epub_metadata.clone();
        output.archive_metadata = self.archive_metadata.clone();
        output.code_metadata = self.code_metadata.clone();
    }

    /// Convert parse result to Output object
    pub fn to_output(&self, mode: OutputMode, config: &crate::engine::config::Config) -> Output {
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
                    // Include media metadata if available (images, videos, audio, CSV, PDF, DOCX, SQLite)
                    self.set_metadata_fields(&mut output);
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
                    self.set_metadata_fields(&mut output);
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
                    self.set_metadata_fields(&mut output);
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
                    self.set_metadata_fields(&mut output);
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
        config: &crate::engine::config::Config,
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
        ..Default::default()
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
                media::extract_image_metadata,
                media::extract_image_templates,
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
                media::extract_video_metadata,
                media::extract_video_templates,
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
                media::extract_audio_metadata,
                media::extract_audio_templates,
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
                structured::extract_csv_metadata,
                structured::extract_csv_templates,
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
            process_media_file!(
                stats,
                &mmap,
                config,
                office::extract_docx_metadata,
                office::extract_docx_templates,
                docx_metadata,
                crate::results::DocumentMetadata,
                FileType::Docx
            )
        }
        FileType::Xlsx => {
            process_media_file!(
                stats,
                &mmap,
                config,
                office::extract_xlsx_metadata,
                office::extract_xlsx_templates,
                docx_metadata,
                crate::results::DocumentMetadata,
                FileType::Xlsx
            )
        }
        FileType::Sqlite => {
            // Extract metadata
            if !config.skip_media_metadata {
                extract_metadata_with_fallback!(
                    stats.sqlite_metadata,
                    sqlite::extract_sqlite_metadata(&mmap, stats, config),
                    stats,
                    crate::results::SqliteMetadata,
                    FileType::Sqlite.as_metadata_name()
                );
            }
            // Extract templates
            sqlite::extract_sqlite_templates(&mmap, stats, config)
        }
        FileType::Toml => {
            process_media_file!(
                stats,
                &mmap,
                config,
                settings::extract_toml_metadata,
                settings::extract_toml_templates,
                toml_metadata,
                crate::results::TomlMetadata,
                FileType::Toml
            )
        }
        FileType::Zip => {
            process_media_file!(
                stats,
                &mmap,
                config,
                container::zip::extract_zip_metadata,
                container::zip::extract_zip_templates,
                zip_metadata,
                crate::results::ZipMetadata,
                FileType::Zip
            )
        }
        FileType::Xml => {
            process_media_file!(
                stats,
                &mmap,
                config,
                settings::extract_xml_metadata,
                settings::extract_xml_templates,
                xml_metadata,
                crate::results::XmlMetadata,
                FileType::Xml
            )
        }
        FileType::Html => {
            process_media_file!(
                stats,
                &mmap,
                config,
                structured::extract_html_metadata,
                structured::extract_html_templates,
                html_metadata,
                crate::results::HtmlMetadata,
                FileType::Html
            )
        }
        FileType::Yaml => {
            process_media_file!(
                stats,
                &mmap,
                config,
                settings::extract_yaml_metadata,
                settings::extract_yaml_templates,
                yaml_metadata,
                crate::results::YamlMetadata,
                FileType::Yaml
            )
        }
        FileType::Ini => {
            process_media_file!(
                stats,
                &mmap,
                config,
                settings::extract_ini_metadata,
                settings::extract_ini_templates,
                ini_metadata,
                crate::results::IniMetadata,
                FileType::Ini
            )
        }
        FileType::Pptx => {
            process_media_file!(
                stats,
                &mmap,
                config,
                office::extract_pptx_metadata,
                office::extract_pptx_templates,
                pptx_metadata,
                crate::results::PptxMetadata,
                FileType::Pptx
            )
        }
        FileType::Epub => {
            process_media_file!(
                stats,
                &mmap,
                config,
                epub::extract_epub_metadata,
                epub::extract_epub_templates,
                epub_metadata,
                crate::results::EpubMetadata,
                FileType::Epub
            )
        }
        FileType::Archive => {
            process_media_file!(
                stats,
                &mmap,
                config,
                container::archive::extract_archive_metadata,
                container::archive::extract_archive_templates,
                archive_metadata,
                crate::results::ArchiveMetadata,
                FileType::Archive
            )
        }
        FileType::Code => {
            process_media_file!(
                stats,
                &mmap,
                config,
                code::extract_code_metadata,
                code::extract_code_templates,
                code_metadata,
                crate::results::CodeMetadata,
                FileType::Code
            )
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
                FileType::Unknown => extract_unknown_templates(content, stats, config),
                file_type if file_type.binary_needs_processing() => unreachable!(),
                // Catch-all for any other file types
                _ => unreachable!(),
            }
        }
    }
}

/// Estimate compressed token count (shared utility)
pub(crate) fn estimate_compressed_tokens_with_footprint(
    templates: &[Template],
    _total_lines: usize,
    config: &crate::engine::config::Config,
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
    config: &Config,
) -> Result<MiningResult> {
    // Try JSON first (most structured), then log, then text
    serde_json::from_str::<serde_json::Value>(content.lines().next().unwrap_or("")).map_or_else(
        |_| text::log::extract_log_templates(content, stats, config),
        |_| text::json::extract_json_templates(content, stats, config),
    )
}
