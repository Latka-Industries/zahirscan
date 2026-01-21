//! Writing analysis structures (footprint, SVO analysis, punctuation metrics)

use serde::{Deserialize, Serialize};

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

/// Compression statistics (Mode 2 only)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionStats {
    pub original_tokens: usize,
    pub compressed_tokens: usize,
    pub reduction_percent: f64,
}
