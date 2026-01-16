//! Result structures for template mining and parsing

use serde::{Deserialize, Serialize};
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Output {
    /// Templates (always present)
    pub templates: Vec<Template>,

    // Mode 2 (Full) fields - all optional
    /// Source file path (Mode 2 only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// File type (Mode 2 only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_type: Option<String>,
    /// Line count (Mode 2 only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_count: Option<usize>,
    /// Byte count (Mode 2 only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_count: Option<usize>,
    /// Token count (Mode 2 only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_count: Option<usize>,
    /// Processing duration in milliseconds (Mode 2 only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processing_time_ms: Option<f64>,
    /// Whether file is binary (Mode 2 only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_binary: Option<bool>,
    /// Compression metrics (Mode 2 only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression: Option<CompressionStats>,
    /// Writing footprint metrics (for text/markdown files, included in both modes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub writing_footprint: Option<WritingFootprint>,
}

/// Compression statistics (Mode 2 only)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionStats {
    pub original_tokens: usize,
    pub compressed_tokens: usize,
    pub reduction_percent: f64,
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
        }
    }
}
