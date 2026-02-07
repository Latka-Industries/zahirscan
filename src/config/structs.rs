//! TOML configuration structs matching config.toml (deserialize-only).
//! Unknown keys produce a clear error via `deny_unknown_fields`.

use serde::Deserialize;

/// TOML configuration structure matching config.toml.
/// Unknown keys in the file produce a clear error (key not found / invalid key).
/// Binary name comes from crate::PKG_NAME, not config.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct TomlConfig {
    #[serde(default)]
    pub concurrency: ConcurrencyConfig,
    #[serde(default)]
    pub mining: MiningConfig,
    #[serde(default)]
    pub filter: FilterConfig,
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct FilterConfig {
    /// Basename patterns to skip (exact, *suffix, prefix*). Defaults include .DS_Store, Thumbs.db, desktop.ini, and temp patterns (*.swp, *.tmp, *~, ~$*, etc.).
    pub ignore_patterns: Vec<String>,
    /// When true, skip any file whose basename starts with `.` (Unix dotfiles). Does not control .DS_Store / temp patterns—those come from ignore_patterns.
    pub ignore_hidden_files: bool,
}

impl Default for FilterConfig {
    fn default() -> Self {
        Self {
            ignore_patterns: vec![
                ".DS_Store".into(),
                "Thumbs.db".into(),
                "desktop.ini".into(),
                "ehthumbs.db".into(),
                "*.swp".into(),
                "*.swo".into(),
                "*.tmp".into(),
                "*~".into(),
                "~$*".into(),
            ],
            ignore_hidden_files: true,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct ConcurrencyConfig {
    pub max_workers: Option<u64>,
    pub small_file_threshold_bytes: u64,
    pub large_file_threshold_bytes: u64,
    pub threshold_multiplier: u64,
    pub min_collection_size_for_chunking: u64,
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

/// Token estimation settings for compressed output size calculations
#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct TokenEstimationConfig {
    pub bytes_per_token: u64,
    pub json_overhead_tokens: u64,
    pub footprint_base_overhead_tokens: u64,
    pub footprint_svo_metrics_tokens: u64,
}

impl Default for TokenEstimationConfig {
    fn default() -> Self {
        Self {
            bytes_per_token: 4,
            json_overhead_tokens: 50,
            footprint_base_overhead_tokens: 20,
            footprint_svo_metrics_tokens: 10,
        }
    }
}

/// Entropy calculation settings for writing footprint analysis
#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct EntropyConfig {
    pub min_entropy_sample_size: u64,
    pub min_entropy_display: f64,
    pub max_entropy_display: f64,
    pub entropy_diversity_threshold: f64,
    pub entropy_small_sample_threshold: u64,
    pub entropy_small_sample_discount: f64,
    pub max_examples_for_entropy: u64,
}

impl Default for EntropyConfig {
    fn default() -> Self {
        Self {
            min_entropy_sample_size: 5,
            min_entropy_display: 0.01,
            max_entropy_display: 0.99,
            entropy_diversity_threshold: 0.95,
            entropy_small_sample_threshold: 5,
            entropy_small_sample_discount: 0.85,
            max_examples_for_entropy: 10,
        }
    }
}

/// Sentence filtering settings for text parsing
#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct SentenceFilterConfig {
    pub min_sentence_words: u64,
    pub min_sentence_words_alt: u64,
    pub min_sentence_length: u64,
    pub min_examples_per_placeholder: u64,
    pub short_prefix_threshold: u64,
}

impl Default for SentenceFilterConfig {
    fn default() -> Self {
        Self {
            min_sentence_words: 2,
            min_sentence_words_alt: 3,
            min_sentence_length: 8,
            min_examples_per_placeholder: 3,
            short_prefix_threshold: 3,
        }
    }
}

/// Pivot point detection settings for SVO analysis
#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct PivotConfig {
    pub min_pivot_variation: u64,
    pub max_pivot_variation: u64,
    pub max_common_pivots: u64,
}

impl Default for PivotConfig {
    fn default() -> Self {
        Self {
            min_pivot_variation: 2,
            max_pivot_variation: 50,
            max_common_pivots: 10,
        }
    }
}

/// File-type specific parsing settings
#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct FileTypeSpecificConfig {
    pub markdown_preview_length: u64,
    pub max_csv_sample_rows: u64,
}

impl Default for FileTypeSpecificConfig {
    fn default() -> Self {
        Self {
            markdown_preview_length: 100,
            max_csv_sample_rows: 200,
        }
    }
}

/// Core mining configuration with organized sub-configs
#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct MiningConfig {
    pub static_threshold: f64,
    pub text_threshold: f64,
    pub max_sample_lines: u64,
    pub max_examples_per_placeholder: u64,
    pub min_ngram_size: u64,
    pub max_ngram_size: u64,
    pub min_phrase_length: u64,
    pub tokens: TokenEstimationConfig,
    pub entropy: EntropyConfig,
    pub sentence: SentenceFilterConfig,
    pub pivot: PivotConfig,
    pub file_types: FileTypeSpecificConfig,
}

impl Default for MiningConfig {
    fn default() -> Self {
        Self {
            static_threshold: 0.8,
            text_threshold: 0.01,
            max_sample_lines: 100,
            max_examples_per_placeholder: 10,
            min_ngram_size: 2,
            max_ngram_size: 4,
            min_phrase_length: 3,
            tokens: TokenEstimationConfig::default(),
            entropy: EntropyConfig::default(),
            sentence: SentenceFilterConfig::default(),
            pivot: PivotConfig::default(),
            file_types: FileTypeSpecificConfig::default(),
        }
    }
}
