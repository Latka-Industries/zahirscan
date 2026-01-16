//! Probabilistic template mining and parsing
//! Main handler that routes to log or text parsers

mod json;
mod log;
mod markdown;
mod text;
pub mod traits;
pub mod writing_analysis;

use crate::config::Config;
use crate::results::{CompressionStats, FileMetadata, MiningResult, Output, OutputMode, Template};
use crate::tools::detect_file_type;
use anyhow::Result;
use memmap2::Mmap;
use serde_json;
use std::fs::{File, OpenOptions};
use std::io::Write;

/// Supported file types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Log,
    Json,
    Text,
    Markdown,
    Unknown,
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
}

impl ParseResult {
    /// Convert parse result to Output object
    pub fn to_output(&self, mode: OutputMode) -> Output {
        if let Some(ref mining) = self.mining_result {
            match mode {
                OutputMode::Templates => {
                    // Mode 1: Templates + Writing Footprint (for text/markdown files)
                    let mut output = Output::templates_only(mining.templates.clone());
                    // Include writing footprint if available (text/markdown files only)
                    output.writing_footprint = mining.writing_footprint.clone();
                    output
                }
                OutputMode::Full => {
                    // Mode 2: Full metadata
                    let metadata = FileMetadata {
                        source: self.file_path.clone(),
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
                    // Include writing footprint if available
                    output.writing_footprint = mining.writing_footprint.clone();
                    output
                }
            }
        } else {
            // No mining results (binary file or error)
            match mode {
                OutputMode::Templates => Output::templates_only(vec![]),
                OutputMode::Full => {
                    let metadata = FileMetadata {
                        source: self.file_path.clone(),
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
                    output
                }
            }
        }
    }

    /// Write the parse result to an output file as JSON
    pub fn write_to_file(&self, output_path: &str, mode: OutputMode) -> Result<()> {
        let mut output_file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(output_path)?;

        // Both modes now use the Output object to include writing_footprint
        let output = self.to_output(mode);
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
    })
}

/// Extract templates from a file using probabilistic template mining
/// Main handler that routes to log or text parsers
pub fn extract_templates(stats: &ParseResult, config: &Config) -> Result<MiningResult> {
    // Skip binary files
    if stats.is_binary {
        return Ok(traits::empty_mining_result(stats));
    }

    // Re-open and memory-map the file for template mining
    let mmap = open_mmap(&stats.file_path)?;
    let content = std::str::from_utf8(&mmap)?;

    match stats.file_type {
        FileType::Log => log::extract_log_templates(content, stats, config),
        FileType::Json => json::extract_json_templates(content, stats, config),
        FileType::Markdown => markdown::extract_markdown_templates(content, stats, config),
        FileType::Text => text::extract_text_templates(content, stats, config),
        FileType::Unknown => {
            // Try JSON first (most structured), then log, then text
            if serde_json::from_str::<serde_json::Value>(content.lines().next().unwrap_or(""))
                .is_ok()
            {
                json::extract_json_templates(content, stats, config)
            } else {
                log::extract_log_templates(content, stats, config)
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
        // SVO analysis: structure percent, subject/object lengths, common pivots (up to 10)
        let mut tokens = 20; // Base overhead for structure
        if let Some(ref svo) = footprint.svo_analysis {
            tokens += 10; // SVO metrics
            tokens += svo.common_pivots.len().min(10); // Common pivots
        }
        tokens
    } else {
        0
    };

    template_tokens + example_tokens + footprint_tokens + config.json_overhead_tokens
}
