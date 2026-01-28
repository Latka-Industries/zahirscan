//! Configuration for ZahirScan

use anyhow::Result;
use serde::Deserialize;
use std::fs;
use std::path::Path;

// Helper functions for validation and type conversion
#[inline]
fn u64_to_usize_min(value: u64, min: u64) -> usize {
    value.max(min) as usize
}

#[inline]
fn u64_to_usize(value: u64) -> usize {
    value as usize
}

#[inline]
fn clamp_f64(value: f64, min: f64, max: f64) -> f64 {
    value.clamp(min, max)
}

/// TOML configuration structure matching config.toml
#[derive(Debug, Deserialize)]
struct TomlConfig {
    #[serde(default = "default_binary_name")]
    binary_name: String,
    #[serde(default)]
    concurrency: ConcurrencyConfig,
    #[serde(default)]
    mining: MiningConfig,
}

impl Default for TomlConfig {
    fn default() -> Self {
        Self {
            binary_name: default_binary_name(),
            concurrency: ConcurrencyConfig::default(),
            mining: MiningConfig::default(),
        }
    }
}

fn default_binary_name() -> String {
    "zahirscan".to_string()
}

#[derive(Debug, Deserialize)]
#[serde(default)]
struct ConcurrencyConfig {
    max_workers: Option<u64>,
    // Chunking is now fully adaptive (calculated from Phase 1 stats)
    // Byte thresholds for adaptive chunking multipliers
    small_file_threshold_bytes: u64,
    large_file_threshold_bytes: u64,
    // File batching threshold multiplier
    // Batching kicks in when files > workers * threshold_multiplier
    threshold_multiplier: u64,
    // Minimum collection size for chunking
    // Collections smaller than this will not be chunked (chunk_size = 1)
    min_collection_size_for_chunking: u64,
}

impl Default for ConcurrencyConfig {
    fn default() -> Self {
        Self {
            max_workers: None,
            small_file_threshold_bytes: 100_000,   // 100 KB
            large_file_threshold_bytes: 1_000_000, // 1 MB
            threshold_multiplier: 50,
            min_collection_size_for_chunking: 1000,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
struct MiningConfig {
    static_threshold: f64,
    /// Threshold for text files (typically much lower than static_threshold for logs)
    text_threshold: f64,
    max_sample_lines: u64,
    max_examples_per_placeholder: u64,
    min_ngram_size: u64,
    max_ngram_size: u64,
    min_phrase_length: u64,
    bytes_per_token: u64,
    json_overhead_tokens: u64,
    // Writing footprint token estimation
    footprint_base_overhead_tokens: u64,
    footprint_svo_metrics_tokens: u64,
    // Entropy calculation settings
    min_entropy_sample_size: u64,
    min_entropy_display: f64,
    max_entropy_display: f64,
    entropy_diversity_threshold: f64,
    entropy_small_sample_threshold: u64,
    entropy_small_sample_discount: f64,
    // Sentence filtering settings
    min_sentence_words: u64,
    min_sentence_words_alt: u64,
    min_sentence_length: u64,
    max_examples_for_entropy: u64,
    min_examples_per_placeholder: u64,
    short_prefix_threshold: u64,
    // Pivot point detection settings
    min_pivot_variation: u64,
    max_pivot_variation: u64,
    max_common_pivots: u64,
    // Markdown parsing settings
    markdown_preview_length: u64,
    // CSV parsing settings
    max_csv_sample_rows: u64,
    // Performance optimization settings (reserved for future use)
}

impl Default for MiningConfig {
    fn default() -> Self {
        Self {
            static_threshold: 0.8, // 80% for logs (high repetition)
            text_threshold: 0.01,  // 1% for text files (literary text has low repetition)
            max_sample_lines: 100,
            max_examples_per_placeholder: 10,
            min_ngram_size: 2,
            max_ngram_size: 4,
            min_phrase_length: 3,
            bytes_per_token: 4,
            json_overhead_tokens: 50,
            footprint_base_overhead_tokens: 20,
            footprint_svo_metrics_tokens: 10,
            min_entropy_sample_size: 5,
            min_entropy_display: 0.01,
            max_entropy_display: 0.99,
            entropy_diversity_threshold: 0.95,
            entropy_small_sample_threshold: 5,
            entropy_small_sample_discount: 0.85,
            min_sentence_words: 2,
            min_sentence_words_alt: 3,
            min_sentence_length: 8,
            max_examples_for_entropy: 10,
            min_examples_per_placeholder: 3,
            short_prefix_threshold: 3,
            min_pivot_variation: 2,
            max_pivot_variation: 50,
            max_common_pivots: 10,
            markdown_preview_length: 100,
            max_csv_sample_rows: 200,
        }
    }
}

/// Configuration struct for ZahirScan
#[derive(Debug, Clone)]
pub struct Config {
    /// Binary name for output file naming
    pub binary_name: String,
    /// Maximum number of parallel workers
    pub max_workers: usize,
    /// Target number of chunks per file (calculated adaptively, neat multiple of max_workers)
    pub target_chunks_per_file: usize,
    /// Output mode (templates only or full metadata)
    pub output_mode: crate::results::OutputMode,
    /// Static token threshold (0.0-1.0) - percentage of lines that must match for a token to be considered static
    pub static_threshold: f64,
    /// Threshold for text files (0.0-1.0) - typically much lower than static_threshold for literary text
    pub text_threshold: f64,
    /// Maximum number of lines to sample when extracting examples
    pub max_sample_lines: usize,
    /// Maximum number of examples to collect per placeholder
    pub max_examples_per_placeholder: usize,
    /// Minimum n-gram size for text parsing (typically 2)
    pub min_ngram_size: usize,
    /// Maximum n-gram size for text parsing (typically 4)
    pub max_ngram_size: usize,
    /// Minimum phrase length for text parsing (typically 3)
    pub min_phrase_length: usize,
    /// Bytes per token divisor for token estimation (typically 4)
    pub bytes_per_token: usize,
    /// JSON structure overhead for token estimation (typically 50)
    pub json_overhead_tokens: usize,
    /// Base overhead for writing footprint structure token estimation (typically 20)
    pub footprint_base_overhead_tokens: usize,
    /// Additional tokens for SVO analysis metrics token estimation (typically 10)
    pub footprint_svo_metrics_tokens: usize,
    /// Minimum number of matching sentences required to calculate entropy
    pub min_entropy_sample_size: usize,
    /// Minimum entropy value to display (below this, entropy is hidden)
    pub min_entropy_display: f64,
    /// Maximum entropy value to display (above this, entropy is hidden)
    pub max_entropy_display: f64,
    /// Diversity ratio threshold for applying small sample correction
    pub entropy_diversity_threshold: f64,
    /// Sample size threshold for applying small sample correction
    pub entropy_small_sample_threshold: usize,
    /// Discount factor for small samples with high diversity
    pub entropy_small_sample_discount: f64,
    /// Minimum number of words required for a sentence to be processed
    pub min_sentence_words: usize,
    /// Alternative word count: if sentence has this many words, length requirement is relaxed
    pub min_sentence_words_alt: usize,
    /// Minimum character length for a sentence (unless it meets word count alternative)
    pub min_sentence_length: usize,
    /// Maximum examples to use for entropy calculation
    pub max_examples_for_entropy: usize,
    /// Minimum examples per placeholder (used when calculating example limits)
    pub min_examples_per_placeholder: usize,
    /// Short prefix/suffix threshold (words) - below this, include in pattern; above, use placeholder
    pub short_prefix_threshold: usize,
    /// Minimum variation score (different words following pivot) to be considered a pivot
    pub min_pivot_variation: usize,
    /// Maximum variation score - above this, word is too random to be structural
    pub max_pivot_variation: usize,
    /// Maximum number of common pivots to include in SVO analysis output
    pub max_common_pivots: usize,
    /// Maximum length of paragraph text preview in examples (characters)
    pub markdown_preview_length: usize,
    /// Maximum number of rows to sample for CSV type inference
    pub max_csv_sample_rows: usize,
    /// Small file threshold (bytes) - files below this use multiplier=1
    pub small_file_threshold_bytes: usize,
    /// Large file threshold (bytes) - files above this use multiplier=3
    pub large_file_threshold_bytes: usize,
    /// File batching threshold multiplier - batching kicks in when files > workers * threshold_multiplier
    pub threshold_multiplier: usize,
    /// Minimum collection size for chunking - collections smaller than this will not be chunked (chunk_size = 1)
    pub min_collection_size_for_chunking: usize,
    /// Whether to redact file paths in output (show only filename as ***/filename.ext)
    pub redact_paths: bool,
    /// Whether to skip media metadata extraction (audio, video, image)
    pub skip_media_metadata: bool,
    /// Whether to show progress bars during processing
    pub show_progress: bool,
}

impl Config {
    /// Convert from TOML config to public Config struct
    fn from_toml_config(toml_config: TomlConfig) -> Self {
        let binary_name = toml_config.binary_name;

        let max_workers = toml_config
            .concurrency
            .max_workers
            .map(|w| w as usize)
            .unwrap_or(0);

        let mining = toml_config.mining;
        let concurrency = toml_config.concurrency;

        // Calculate actual max_workers (num_cpus - 1, or use configured value if > 0)
        let max_workers = if max_workers == 0 {
            num_cpus::get().saturating_sub(1).max(1)
        } else {
            max_workers
        };

        // Chunking is now fully adaptive (calculated from Phase 1 stats)
        // This default is not used - adaptive calculation overrides it
        let target_chunks_per_file = 0; // Will be calculated adaptively

        // Validate thresholds and sizes
        let static_threshold = clamp_f64(mining.static_threshold, 0.0, 1.0);
        let min_ngram_size = u64_to_usize_min(mining.min_ngram_size, 1);
        let max_ngram_size = u64_to_usize_min(mining.max_ngram_size.max(mining.min_ngram_size), 1);
        let min_phrase_length = u64_to_usize_min(mining.min_phrase_length, 2);
        let bytes_per_token = u64_to_usize_min(mining.bytes_per_token, 1);

        Self {
            binary_name,
            max_workers,
            target_chunks_per_file,
            output_mode: crate::results::OutputMode::Templates,
            static_threshold,
            text_threshold: clamp_f64(mining.text_threshold, 0.0, 1.0),
            max_sample_lines: u64_to_usize(mining.max_sample_lines),
            max_examples_per_placeholder: u64_to_usize(mining.max_examples_per_placeholder),
            min_ngram_size,
            max_ngram_size,
            min_phrase_length,
            bytes_per_token,
            json_overhead_tokens: u64_to_usize(mining.json_overhead_tokens),
            footprint_base_overhead_tokens: u64_to_usize(mining.footprint_base_overhead_tokens),
            footprint_svo_metrics_tokens: u64_to_usize(mining.footprint_svo_metrics_tokens),
            min_entropy_sample_size: u64_to_usize_min(mining.min_entropy_sample_size, 1),
            min_entropy_display: clamp_f64(mining.min_entropy_display, 0.0, 1.0),
            max_entropy_display: clamp_f64(mining.max_entropy_display, 0.0, 1.0),
            entropy_diversity_threshold: clamp_f64(mining.entropy_diversity_threshold, 0.0, 1.0),
            entropy_small_sample_threshold: u64_to_usize_min(
                mining.entropy_small_sample_threshold,
                1,
            ),
            entropy_small_sample_discount: clamp_f64(
                mining.entropy_small_sample_discount,
                0.0,
                1.0,
            ),
            min_sentence_words: u64_to_usize_min(mining.min_sentence_words, 1),
            min_sentence_words_alt: u64_to_usize_min(mining.min_sentence_words_alt, 1),
            min_sentence_length: u64_to_usize_min(mining.min_sentence_length, 1),
            max_examples_for_entropy: u64_to_usize_min(mining.max_examples_for_entropy, 1),
            min_examples_per_placeholder: u64_to_usize_min(mining.min_examples_per_placeholder, 1),
            short_prefix_threshold: u64_to_usize_min(mining.short_prefix_threshold, 1),
            min_pivot_variation: u64_to_usize_min(mining.min_pivot_variation, 1),
            max_pivot_variation: u64_to_usize_min(mining.max_pivot_variation, 1),
            max_common_pivots: u64_to_usize_min(mining.max_common_pivots, 1),
            markdown_preview_length: u64_to_usize_min(mining.markdown_preview_length, 1),
            max_csv_sample_rows: u64_to_usize_min(mining.max_csv_sample_rows, 1),
            small_file_threshold_bytes: u64_to_usize(concurrency.small_file_threshold_bytes),
            large_file_threshold_bytes: u64_to_usize(concurrency.large_file_threshold_bytes),
            threshold_multiplier: u64_to_usize(concurrency.threshold_multiplier),
            min_collection_size_for_chunking: u64_to_usize(
                concurrency.min_collection_size_for_chunking,
            ),
            // Set via CLI flag, not from TOML
            redact_paths: false,
            skip_media_metadata: false,
            show_progress: false,
        }
    }

    /// Load configuration from config.toml file
    ///
    /// Returns `Ok(Config)` if the file exists and parses successfully,
    /// or an error if the file cannot be read or parsed.
    /// Use `Config::default()` if you want defaults without file I/O.
    pub fn load() -> Result<Self> {
        let config_path = "config.toml";

        let toml_config: TomlConfig = if Path::new(config_path).exists() {
            let content = fs::read_to_string(config_path)?;
            toml::from_str(&content)?
        } else {
            TomlConfig::default()
        };

        Ok(Self::from_toml_config(toml_config))
    }

    /// Create a new default configuration (no file I/O)
    ///
    /// This creates a config with all default values from `MiningConfig::default()`
    /// and `ConcurrencyConfig::default()`. Use `Config::load()` to load from config.toml.
    pub fn new() -> Self {
        Self::from_toml_config(TomlConfig::default())
    }

    /// Get the default temp file extension
    pub fn temp_file_extension(&self) -> String {
        format!("{}.out", self.binary_name)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}
