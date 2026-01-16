//! Configuration for ZahirScan

use anyhow::Result;
use serde::Deserialize;
use std::fs;
use std::path::Path;

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

#[derive(Debug, Deserialize, Default)]
struct ConcurrencyConfig {
    max_workers: Option<u64>,
    /// Maximum number of files to process concurrently (0 = unlimited, use all workers)
    max_concurrent_files: Option<u64>,
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
    /// Maximum number of files to process concurrently (0 = unlimited)
    pub max_concurrent_files: usize,
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
}

impl Config {
    /// Load configuration from config.toml, or use defaults
    pub fn load() -> Result<Self> {
        let config_path = "config.toml";

        let toml_config: TomlConfig = if Path::new(config_path).exists() {
            let content = fs::read_to_string(config_path)?;
            toml::from_str(&content)?
        } else {
            TomlConfig::default()
        };

        let binary_name = toml_config.binary_name;

        let max_workers = toml_config
            .concurrency
            .max_workers
            .map(|w| w as usize)
            .unwrap_or(0);

        let mining = toml_config.mining;

        // Calculate actual max_workers (num_cpus - 1, or use configured value if > 0)
        let max_workers = if max_workers == 0 {
            num_cpus::get().saturating_sub(1).max(1)
        } else {
            max_workers
        };

        // Calculate optimal max_concurrent_files if not specified
        // Default: 4-8 files concurrently to balance throughput and reduce contention
        // Adaptive: Use max_workers / 2, clamped to 4-8 range for optimal performance
        let max_concurrent_files = toml_config
            .concurrency
            .max_concurrent_files
            .map(|f| f as usize)
            .unwrap_or_else(|| {
                // Adaptive default: half of workers, but clamped to reasonable range
                let adaptive = (max_workers / 2).max(2);
                adaptive.min(8).max(4) // Clamp to 4-8 for optimal performance
            });

        // Validate static_threshold is in valid range
        let static_threshold = mining.static_threshold.clamp(0.0, 1.0);

        // Validate n-gram sizes
        let min_ngram_size = mining.min_ngram_size.max(1) as usize;
        let max_ngram_size = mining.max_ngram_size.max(min_ngram_size as u64) as usize;
        let min_phrase_length = mining.min_phrase_length.max(2) as usize;
        let bytes_per_token = mining.bytes_per_token.max(1) as usize;

        Ok(Self {
            binary_name,
            max_workers,
            max_concurrent_files,
            output_mode: crate::results::OutputMode::Templates,
            static_threshold,
            text_threshold: mining.text_threshold.clamp(0.0, 1.0),
            max_sample_lines: mining.max_sample_lines as usize,
            max_examples_per_placeholder: mining.max_examples_per_placeholder as usize,
            min_ngram_size,
            max_ngram_size,
            min_phrase_length,
            bytes_per_token,
            json_overhead_tokens: mining.json_overhead_tokens as usize,
            min_entropy_sample_size: mining.min_entropy_sample_size.max(1) as usize,
            min_entropy_display: mining.min_entropy_display.clamp(0.0, 1.0),
            max_entropy_display: mining.max_entropy_display.clamp(0.0, 1.0),
            entropy_diversity_threshold: mining.entropy_diversity_threshold.clamp(0.0, 1.0),
            entropy_small_sample_threshold: mining.entropy_small_sample_threshold.max(1) as usize,
            entropy_small_sample_discount: mining.entropy_small_sample_discount.clamp(0.0, 1.0),
            min_sentence_words: mining.min_sentence_words.max(1) as usize,
            min_sentence_words_alt: mining.min_sentence_words_alt.max(1) as usize,
            min_sentence_length: mining.min_sentence_length.max(1) as usize,
            max_examples_for_entropy: mining.max_examples_for_entropy.max(1) as usize,
            min_examples_per_placeholder: mining.min_examples_per_placeholder.max(1) as usize,
            short_prefix_threshold: mining.short_prefix_threshold.max(1) as usize,
            min_pivot_variation: mining.min_pivot_variation.max(1) as usize,
            max_pivot_variation: mining.max_pivot_variation.max(1) as usize,
        })
    }

    /// Create a new default configuration
    pub fn new() -> Self {
        let max_workers = num_cpus::get().saturating_sub(1).max(1);
        // Adaptive default: half of workers, clamped to 4-8 range
        let max_concurrent_files = {
            let adaptive = (max_workers / 2).max(2);
            adaptive.min(8).max(4)
        };

        Self {
            binary_name: "zahirscan".to_string(),
            max_workers,
            max_concurrent_files,
            output_mode: crate::results::OutputMode::Templates,
            static_threshold: 0.8,
            text_threshold: 0.01,
            max_sample_lines: 100,
            max_examples_per_placeholder: 10,
            min_ngram_size: 2,
            max_ngram_size: 4,
            min_phrase_length: 3,
            bytes_per_token: 4,
            json_overhead_tokens: 50,
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
        }
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
