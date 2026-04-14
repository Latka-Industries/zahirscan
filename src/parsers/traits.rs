//! Common utilities for parsers

use rayon::prelude::*;

use crate::config::RuntimeConfig;
use crate::parsers::{ParseResult, estimate_compressed_tokens_with_footprint};
use crate::results::{MiningResult, Template};

/// Create an empty `MiningResult` (no templates found)
/// This is shared across all parsers for empty content cases
#[must_use]
pub fn empty_mining_result(stats: &ParseResult) -> MiningResult {
    MiningResult {
        templates: vec![],
        original_tokens: stats.token_count,
        compressed_tokens: 0,
        token_reduction_percent: 0.0,
        writing_footprint: None,
    }
}

/// Build `MiningResult` from templates (sorts and calculates compression)
/// This is shared across all parsers
#[must_use]
pub fn build_mining_result(
    templates: Vec<Template>,
    total_items: usize,
    stats: &ParseResult,
    config: &RuntimeConfig,
) -> MiningResult {
    build_mining_result_with_footprint(templates, total_items, stats, config, None)
}

/// Build `MiningResult` from templates including writing footprint in compression calculation
#[must_use]
pub fn build_mining_result_with_footprint(
    templates: Vec<Template>,
    total_items: usize,
    stats: &ParseResult,
    config: &RuntimeConfig,
    writing_footprint: Option<&crate::results::WritingFootprint>,
) -> MiningResult {
    // Sort by count (most common first)
    let mut sorted_templates = templates;
    sort_templates_by_count(&mut sorted_templates);

    // Calculate compression including writing footprint tokens
    let (original_tokens, compressed_tokens, token_reduction_percent) =
        calculate_compression_with_footprint(
            &sorted_templates,
            total_items,
            stats,
            config,
            writing_footprint,
        );

    MiningResult {
        templates: sorted_templates,
        original_tokens,
        compressed_tokens,
        token_reduction_percent,
        writing_footprint: writing_footprint.cloned(),
    }
}

/// Calculate compression metrics from templates
/// Returns (`original_tokens`, `compressed_tokens`, `token_reduction_percent`)
#[must_use]
pub fn calculate_compression(
    templates: &[Template],
    total_items: usize,
    stats: &ParseResult,
    config: &RuntimeConfig,
) -> (usize, usize, f64) {
    calculate_compression_with_footprint(templates, total_items, stats, config, None)
}

/// Calculate compression metrics from templates including writing footprint
/// Returns (`original_tokens`, `compressed_tokens`, `token_reduction_percent`)
#[must_use]
pub fn calculate_compression_with_footprint(
    templates: &[Template],
    total_items: usize,
    stats: &ParseResult,
    config: &RuntimeConfig,
    writing_footprint: Option<&crate::results::WritingFootprint>,
) -> (usize, usize, f64) {
    let original_tokens = stats.token_count;
    let compressed_tokens = estimate_compressed_tokens_with_footprint(
        templates,
        total_items,
        config,
        writing_footprint,
    );
    let token_reduction_percent = if original_tokens > 0 {
        let reduction = original_tokens.saturating_sub(compressed_tokens);
        (reduction as f64 / original_tokens as f64) * 100.0
    } else {
        0.0
    };
    (original_tokens, compressed_tokens, token_reduction_percent)
}

/// Sort templates by count (most common first)
/// This is shared across all parsers
pub fn sort_templates_by_count(templates: &mut [Template]) {
    templates.sort_by(|a, b| b.count.cmp(&a.count));
}

// ============================================================================
// Parallel Processing Utilities
// ============================================================================

// Re-export optimal_chunk_size from chunking module for convenience
pub use crate::engine::chunking::optimal_chunk_size;

/// Extension trait for collections to enable adaptive parallel iteration
/// Combines chunk size calculation with parallel iteration setup
pub trait AdaptiveParallel:
    IntoParallelIterator<Iter: rayon::iter::IndexedParallelIterator>
{
    /// Returns a parallel iterator configured with adaptive chunking based on config
    fn par_iter_adaptive(self, config: &RuntimeConfig) -> rayon::iter::MinLen<Self::Iter>
    where
        Self: Sized,
    {
        let iter = self.into_par_iter();
        let collection_size = iter.len();
        let chunk_size = optimal_chunk_size(
            collection_size,
            config.target_chunks_per_file,
            config.min_collection_size_for_chunking,
        );
        iter.with_min_len(chunk_size)
    }
}

// Implement for common collection types
impl<T> AdaptiveParallel for &[T] where T: Send + Sync {}
impl<T> AdaptiveParallel for Vec<T> where T: Send + Sync {}
