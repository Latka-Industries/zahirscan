//! Runtime configuration struct and loading (from TOML, overlay merge, validation).
//!
//! At build, repo `config.toml` is embedded as a string. [`RuntimeConfig::new`] parses that into the
//! default config (no file I/O). Use [`load_from_path`](RuntimeConfig::load_from_path) to load from
//! a file; use [`load_config_with_overlay`](RuntimeConfig::load_config_with_overlay) or
//! [`load_with_overlay`](RuntimeConfig::load_with_overlay) to merge a base with an overlay path.

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use toml::Value as TomlValue;
use toml::map::Map;

use super::helpers::{clamp_f64, deep_merge_toml, u64_to_usize, u64_to_usize_min};
use super::structs::TomlConfig;
use crate::{validate_min, validate_range_01};

/// Configuration struct for ZahirScan.
/// Binary name is always [`crate::PKG_NAME`], not stored in config.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
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
    /// File basename patterns to skip before Phase 1 (exact, *suffix, or prefix*)
    pub ignore_patterns: Vec<String>,
    /// Skip Unix hidden files (basename starts with .)
    pub ignore_hidden_files: bool,
}

impl RuntimeConfig {
    /// Convert from TOML config to public Config struct
    fn from_toml_config(toml_config: TomlConfig) -> Self {
        let max_workers = toml_config
            .concurrency
            .max_workers
            .map(|w| w as usize)
            .unwrap_or(0);

        let mining = toml_config.mining;
        let concurrency = toml_config.concurrency;

        let max_workers = if max_workers == 0 {
            num_cpus::get().saturating_sub(1).max(1)
        } else {
            max_workers
        };

        let target_chunks_per_file = 0; // Will be calculated adaptively

        let static_threshold = clamp_f64(mining.static_threshold, 0.0, 1.0);
        let min_ngram_size = u64_to_usize_min(mining.min_ngram_size, 1);
        let max_ngram_size = u64_to_usize_min(mining.max_ngram_size.max(mining.min_ngram_size), 1);
        let min_phrase_length = u64_to_usize_min(mining.min_phrase_length, 2);
        let bytes_per_token = u64_to_usize_min(mining.tokens.bytes_per_token, 1);

        Self {
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
            json_overhead_tokens: u64_to_usize(mining.tokens.json_overhead_tokens),
            footprint_base_overhead_tokens: u64_to_usize(
                mining.tokens.footprint_base_overhead_tokens,
            ),
            footprint_svo_metrics_tokens: u64_to_usize(mining.tokens.footprint_svo_metrics_tokens),
            min_entropy_sample_size: u64_to_usize_min(mining.entropy.min_entropy_sample_size, 1),
            min_entropy_display: clamp_f64(mining.entropy.min_entropy_display, 0.0, 1.0),
            max_entropy_display: clamp_f64(mining.entropy.max_entropy_display, 0.0, 1.0),
            entropy_diversity_threshold: clamp_f64(
                mining.entropy.entropy_diversity_threshold,
                0.0,
                1.0,
            ),
            entropy_small_sample_threshold: u64_to_usize_min(
                mining.entropy.entropy_small_sample_threshold,
                1,
            ),
            entropy_small_sample_discount: clamp_f64(
                mining.entropy.entropy_small_sample_discount,
                0.0,
                1.0,
            ),
            max_examples_for_entropy: u64_to_usize_min(mining.entropy.max_examples_for_entropy, 1),
            min_sentence_words: u64_to_usize_min(mining.sentence.min_sentence_words, 1),
            min_sentence_words_alt: u64_to_usize_min(mining.sentence.min_sentence_words_alt, 1),
            min_sentence_length: u64_to_usize_min(mining.sentence.min_sentence_length, 1),
            min_examples_per_placeholder: u64_to_usize_min(
                mining.sentence.min_examples_per_placeholder,
                1,
            ),
            short_prefix_threshold: u64_to_usize_min(mining.sentence.short_prefix_threshold, 1),
            min_pivot_variation: u64_to_usize_min(mining.pivot.min_pivot_variation, 1),
            max_pivot_variation: u64_to_usize_min(mining.pivot.max_pivot_variation, 1),
            max_common_pivots: u64_to_usize_min(mining.pivot.max_common_pivots, 1),
            markdown_preview_length: u64_to_usize_min(mining.file_types.markdown_preview_length, 1),
            max_csv_sample_rows: u64_to_usize_min(mining.file_types.max_csv_sample_rows, 1),
            small_file_threshold_bytes: u64_to_usize(concurrency.small_file_threshold_bytes),
            large_file_threshold_bytes: u64_to_usize(concurrency.large_file_threshold_bytes),
            threshold_multiplier: u64_to_usize(concurrency.threshold_multiplier),
            min_collection_size_for_chunking: u64_to_usize(
                concurrency.min_collection_size_for_chunking,
            ),
            redact_paths: false,
            skip_media_metadata: false,
            show_progress: false,
            ignore_patterns: toml_config.filter.ignore_patterns,
            ignore_hidden_files: toml_config.filter.ignore_hidden_files,
        }
    }

    /// Load configuration from a specific path.
    ///
    /// The file must exist and be valid TOML. Config is validated before return.
    /// For embedded default with no file I/O, use [`RuntimeConfig::new`](Self::new).
    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config from {}", path.display()))?;
        let toml_config: TomlConfig = toml::from_str(&content)
            .with_context(|| format!("Invalid TOML in {}", path.display()))?;
        let config = Self::from_toml_config(toml_config);
        config.validate_external().with_context(|| {
            format!(
                "Invalid config in {} (check ranges in docs)",
                path.display()
            )
        })?;
        Ok(config)
    }

    /// Load base config from TOML string and optional overlay file. Only keys present in the overlay override the base.
    ///
    /// Used by the CLI: base is the embedded default; overlay is the user config file (app data dir).
    pub fn load_config_with_overlay(
        base_toml: &str,
        overlay_path: Option<impl AsRef<Path>>,
    ) -> Result<Self> {
        let base_value: TomlValue =
            toml::from_str(base_toml).context("Invalid embedded default config TOML")?;
        let merged_value = if let Some(overlay_path) = overlay_path {
            let overlay_path = overlay_path.as_ref();
            if overlay_path.exists() {
                let content = fs::read_to_string(overlay_path).with_context(|| {
                    format!("Failed to read overlay from {}", overlay_path.display())
                })?;
                let overlay_value: TomlValue = toml::from_str(&content)
                    .with_context(|| format!("Invalid TOML in {}", overlay_path.display()))?;
                deep_merge_toml(base_value, overlay_value)
            } else {
                base_value
            }
        } else {
            base_value
        };
        let merged_str = toml::to_string(&merged_value).context("Serializing merged config")?;
        let toml_config: TomlConfig = toml::from_str(&merged_str)
            .context("Invalid merged config (unknown keys or invalid values)")?;
        let config = Self::from_toml_config(toml_config);
        config
            .validate_external()
            .context("Invalid config (check ranges in docs)")?;
        Ok(config)
    }

    /// Load base config from file and optional overlay file. Only keys present in the overlay override the base.
    ///
    /// For library use when you have a project config.toml on disk (e.g. tests or custom loader).
    pub fn load_with_overlay(
        base_path: impl AsRef<Path>,
        overlay_path: Option<impl AsRef<Path>>,
    ) -> Result<Self> {
        let base_path = base_path.as_ref();
        let base_value = if base_path.exists() {
            let content = fs::read_to_string(base_path)
                .with_context(|| format!("Failed to read config from {}", base_path.display()))?;
            toml::from_str(&content)
                .with_context(|| format!("Invalid TOML in {}", base_path.display()))?
        } else {
            TomlValue::Table(Map::new())
        };
        let merged_value = if let Some(overlay_path) = overlay_path {
            let overlay_path = overlay_path.as_ref();
            if overlay_path.exists() {
                let content = fs::read_to_string(overlay_path).with_context(|| {
                    format!("Failed to read overlay from {}", overlay_path.display())
                })?;
                let overlay_value: TomlValue = toml::from_str(&content)
                    .with_context(|| format!("Invalid TOML in {}", overlay_path.display()))?;
                deep_merge_toml(base_value, overlay_value)
            } else {
                base_value
            }
        } else {
            base_value
        };
        let merged_str = toml::to_string(&merged_value).context("Serializing merged config")?;
        let toml_config: TomlConfig = toml::from_str(&merged_str)
            .context("Invalid merged config (unknown keys or invalid values)")?;
        let config = Self::from_toml_config(toml_config);
        config
            .validate_external()
            .context("Invalid config (check ranges in docs)")?;
        Ok(config)
    }

    /// Overwrite this config with values from another (e.g. user overrides).
    ///
    /// Every field is replaced with the value from `other`. Used by the CLI to
    /// apply XDG/user config over the project config.toml.
    pub fn merge_from(&mut self, other: &Self) {
        self.max_workers = other.max_workers;
        self.target_chunks_per_file = other.target_chunks_per_file;
        self.output_mode = other.output_mode;
        self.static_threshold = other.static_threshold;
        self.text_threshold = other.text_threshold;
        self.max_sample_lines = other.max_sample_lines;
        self.max_examples_per_placeholder = other.max_examples_per_placeholder;
        self.min_ngram_size = other.min_ngram_size;
        self.max_ngram_size = other.max_ngram_size;
        self.min_phrase_length = other.min_phrase_length;
        self.bytes_per_token = other.bytes_per_token;
        self.json_overhead_tokens = other.json_overhead_tokens;
        self.footprint_base_overhead_tokens = other.footprint_base_overhead_tokens;
        self.footprint_svo_metrics_tokens = other.footprint_svo_metrics_tokens;
        self.min_entropy_sample_size = other.min_entropy_sample_size;
        self.min_entropy_display = other.min_entropy_display;
        self.max_entropy_display = other.max_entropy_display;
        self.entropy_diversity_threshold = other.entropy_diversity_threshold;
        self.entropy_small_sample_threshold = other.entropy_small_sample_threshold;
        self.entropy_small_sample_discount = other.entropy_small_sample_discount;
        self.max_examples_for_entropy = other.max_examples_for_entropy;
        self.min_sentence_words = other.min_sentence_words;
        self.min_sentence_words_alt = other.min_sentence_words_alt;
        self.min_sentence_length = other.min_sentence_length;
        self.min_examples_per_placeholder = other.min_examples_per_placeholder;
        self.short_prefix_threshold = other.short_prefix_threshold;
        self.min_pivot_variation = other.min_pivot_variation;
        self.max_pivot_variation = other.max_pivot_variation;
        self.max_common_pivots = other.max_common_pivots;
        self.markdown_preview_length = other.markdown_preview_length;
        self.max_csv_sample_rows = other.max_csv_sample_rows;
        self.small_file_threshold_bytes = other.small_file_threshold_bytes;
        self.large_file_threshold_bytes = other.large_file_threshold_bytes;
        self.threshold_multiplier = other.threshold_multiplier;
        self.min_collection_size_for_chunking = other.min_collection_size_for_chunking;
        self.redact_paths = other.redact_paths;
        self.skip_media_metadata = other.skip_media_metadata;
        self.show_progress = other.show_progress;
        self.ignore_patterns = other.ignore_patterns.clone();
        self.ignore_hidden_files = other.ignore_hidden_files;
    }

    /// Default configuration from the embedded config.toml (same source of truth as repo).
    /// No file I/O; uses the TOML baked in at build.
    pub fn new() -> Self {
        Self::load_config_with_overlay(super::DEFAULT_CONFIG_TOML, None::<&Path>)
            .expect("embedded default config must be valid")
    }

    /// Get the default temp file extension (uses crate [`PKG_NAME`](crate::PKG_NAME)).
    pub fn temp_file_extension(&self) -> String {
        format!("{}.out", crate::PKG_NAME)
    }

    /// Validate config that did not come from file (e.g. programmatic or user overlay).
    ///
    /// Use before [`extract_zahir`](crate::extract_zahir) or after merging
    /// overlay so invalid values yield a clear error. Config loaded via [`load_from_path`](Self::load_from_path)
    /// or [`load_config_with_overlay`](Self::load_config_with_overlay) is already validated; this is for external config.
    ///
    /// Note: When loading from file, `max_workers = 0` is normalized to `num_cpus - 1` in
    /// [`from_toml_config`](Self::from_toml_config), so the `max_workers == 0` check only fires for
    /// programmatic config.
    pub fn validate_external(&self) -> Result<()> {
        if self.max_workers == 0 {
            return Err(anyhow::anyhow!(
                "config: max_workers must be >= 1 (got 0). Set in [concurrency] or use RuntimeConfig::new() for default (num_cpus - 1)"
            ));
        }
        validate_range_01!(self, static_threshold, "static_threshold");
        validate_range_01!(self, text_threshold, "text_threshold");
        validate_min!(self, min_ngram_size, 1, "min_ngram_size");
        if self.max_ngram_size < self.min_ngram_size {
            return Err(anyhow::anyhow!(
                "config: max_ngram_size ({}) must be >= min_ngram_size ({})",
                self.max_ngram_size,
                self.min_ngram_size
            ));
        }
        validate_min!(self, min_phrase_length, 2, "min_phrase_length");
        validate_min!(self, bytes_per_token, 1, "bytes_per_token");
        validate_range_01!(self, min_entropy_display, "min_entropy_display");
        validate_range_01!(self, max_entropy_display, "max_entropy_display");
        validate_range_01!(
            self,
            entropy_diversity_threshold,
            "entropy_diversity_threshold"
        );
        validate_range_01!(
            self,
            entropy_small_sample_discount,
            "entropy_small_sample_discount"
        );
        Ok(())
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self::new()
    }
}
