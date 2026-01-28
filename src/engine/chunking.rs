//! Adaptive chunking utilities for parallel processing
//!
//! This module provides functions for calculating optimal chunk sizes
//! and adaptive chunking strategies based on file statistics.

use super::config::Config;
use super::tools::format_bytes;
use crate::parsers::{FileType, ParseResult};
use log::debug;

/// File processing task with stats and output path
/// Defined here to avoid circular dependency between chunking and orchestrator
pub struct ProcessingTask {
    pub stats: ParseResult,
    pub output_path: String,
}

/// Adaptive chunking settings calculated from Phase 1 stats
#[derive(Debug, Clone)]
pub struct AdaptiveChunking {
    /// Target number of chunks per file (neat multiple of max_workers)
    pub chunks_per_file_multiplier: usize,
}

/// Calculate optimal chunk size for parallel processing based on collection size and target number of chunks.
///
/// Creates chunks that approximate `target_chunks` chunks (neat multiple of workers).
/// Uses integer division, so the last chunk may be smaller if there's a remainder.
/// This ensures neat multiples of workers for optimal load balancing.
/// The adaptive chunking calculation already accounts for work complexity, so we respect the target.
pub fn optimal_chunk_size(
    collection_size: usize,
    target_chunks: usize,
    min_collection_size_for_chunking: usize,
) -> usize {
    if collection_size < min_collection_size_for_chunking || target_chunks == 0 {
        // Small collections or no chunks: no chunking needed
        return 1;
    }

    // Calculate chunk size to approximate target_chunks chunks
    // Integer division: remainder goes to the last chunk (which is fine)
    // Example: 10,000 items / 26 chunks = 384 per chunk, last chunk gets remainder (16 items)
    let chunk_size = collection_size / target_chunks.max(1);

    // Ensure minimum chunk size of 1 (shouldn't happen, but safety check)
    chunk_size.max(1)
}

/// Calculate adaptive chunking settings based on Phase 1 stats
/// Returns the multiplier for chunks per file (e.g., 1 = 13 chunks, 2 = 26 chunks for 13 workers)
pub fn calculate_adaptive_chunking(
    tasks: &[ProcessingTask],
    max_workers: usize,
    config: &Config,
) -> AdaptiveChunking {
    let num_files = tasks.len();

    // Calculate statistics from Phase 1
    let byte_counts: Vec<usize> = tasks.iter().map(|t| t.stats.byte_count).collect();
    let total_bytes: usize = byte_counts.iter().sum();
    let mean_bytes = if num_files > 0 {
        total_bytes / num_files
    } else {
        0
    };

    // Calculate standard deviation of file sizes
    let variance: f64 = if num_files > 0 {
        byte_counts
            .iter()
            .map(|&bytes| {
                let diff = bytes as f64 - mean_bytes as f64;
                diff * diff
            })
            .sum::<f64>()
            / num_files as f64
    } else {
        0.0
    };
    let std_dev = variance.sqrt();
    let coefficient_of_variation = if mean_bytes > 0 {
        std_dev / mean_bytes as f64
    } else {
        0.0
    };

    // Determine chunks per worker multiplier based on file size and variance
    // Target: create neat multiples of workers (e.g., 13, 26, 39 chunks for 13 workers)
    // Special cases:
    // - Single files: always use multiplier=1
    // - Image-only batches: always use multiplier=1 (image metadata extraction is fast, no chunking needed)
    let chunks_per_worker_multiplier = if num_files == 1 {
        1
    } else if tasks
        .iter()
        .all(|t| t.stats.file_type == FileType::Image || t.stats.file_type == FileType::Audio)
    {
        // All files are images or audio - no chunking needed, metadata extraction is fast
        1
    } else {
        let small_threshold = config.small_file_threshold_bytes;
        let large_threshold = config.large_file_threshold_bytes;

        match (mean_bytes, coefficient_of_variation > 0.5) {
            // Small files: 1 chunk per worker (minimal overhead)
            (bytes, _) if bytes < small_threshold => 1,
            // Medium files: 2 chunks per worker (good balance), fewer if high variance
            (bytes, high_variance) if bytes < large_threshold => {
                if high_variance {
                    1 // High variance: fewer chunks to avoid stragglers
                } else {
                    2
                }
            }
            // Very large files: 3 chunks per worker (better load balancing)
            (_, high_variance) => {
                if high_variance {
                    2 // High variance: moderate chunks
                } else {
                    3
                }
            }
        }
    };

    // Calculate target chunks and bytes per chunk for debug output
    let target_chunks = chunks_per_worker_multiplier * max_workers;
    let bytes_per_chunk = if target_chunks > 0 {
        mean_bytes / target_chunks
    } else {
        0
    };

    debug!(
        "Adaptive chunking: {} files, mean={}, cv={:.2}, multiplier={}, target_chunks={}, bytes_per_chunk={}",
        num_files,
        format_bytes(mean_bytes),
        coefficient_of_variation,
        chunks_per_worker_multiplier,
        target_chunks,
        format_bytes(bytes_per_chunk)
    );

    AdaptiveChunking {
        chunks_per_file_multiplier: chunks_per_worker_multiplier,
    }
}
