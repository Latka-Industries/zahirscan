//! Adaptive chunking utilities for parallel processing
//!
//! This module provides functions for calculating optimal chunk sizes
//! and adaptive chunking strategies based on file statistics.

use log::{debug, warn};
use memmap2::Mmap;
use rayon::iter::IntoParallelRefIterator;
use rayon::prelude::*;

use crate::config::RuntimeConfig;
use crate::parsers::{FileType, ParseResult};
use crate::utils::path_string_helper::format_bytes;

/// File processing task with stats, output path, and optional mmap from Phase 1 (reused in Phase 2 to avoid double open).
#[derive(Debug)]
pub struct ProcessingTask {
    pub stats: ParseResult,
    pub output_path: String,
    /// When set, Phase 2 reuses this mmap instead of opening the file again.
    pub mmap: Option<Mmap>,
}

/// Adaptive chunking settings calculated from Phase 1 stats
#[derive(Debug, Clone)]
pub struct AdaptiveChunking {
    /// Target number of chunks per file (neat multiple of `max_workers`)
    pub chunks_per_file_multiplier: usize,
}

/// Calculate optimal chunk size for parallel processing based on collection size and target number of chunks.
///
/// Creates chunks that approximate `target_chunks` chunks (neat multiple of workers).
/// Uses integer division, so the last chunk may be smaller if there's a remainder.
/// This ensures neat multiples of workers for optimal load balancing.
/// The adaptive chunking calculation already accounts for work complexity, so we respect the target.
#[must_use]
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
#[must_use]
pub fn calculate_adaptive_chunking(
    tasks: &[ProcessingTask],
    max_workers: usize,
    config: &RuntimeConfig,
) -> AdaptiveChunking {
    let num_files = tasks.len();

    // Calculate statistics from Phase 1
    let byte_counts: Vec<usize> = tasks.iter().map(|t| t.stats.byte_count).collect();
    let total_bytes: usize = byte_counts.iter().sum();
    let mean_bytes = total_bytes.checked_div(num_files).unwrap_or(0);

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
    let bytes_per_chunk = mean_bytes.checked_div(target_chunks).unwrap_or(0);

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

pub fn setup_rayon_thread_pool(max_workers: usize) {
    rayon::ThreadPoolBuilder::new()
        .num_threads(max_workers)
        .build_global()
        .unwrap_or_else(|_| {
            warn!("Failed to set thread pool, using default");
        });
}

/// Reserve this many fds for stdio, libs, progress, etc., when deriving batch size from the process limit.
const RESERVED_FDS: usize = 256;
/// Upper bound on auto batch size when the fd limit is very high or unlimited.
const MAX_AUTO_BATCH: usize = 100_000;
/// Fallback batch size when the fd limit cannot be read (e.g. unsupported platform).
pub const DEFAULT_AUTO_BATCH: usize = 10_000;

/// Returns a safe path batch size from the process file descriptor limit so we don't exceed "too many open files".
/// Used when `config.path_batch_size` is 0. Leaves [`RESERVED_FDS`] headroom and caps at [`MAX_AUTO_BATCH`].
#[must_use]
pub fn fd_limit_batch_size() -> Option<usize> {
    #[cfg(unix)]
    {
        let (soft, _hard) = rlimit::getrlimit(rlimit::Resource::NOFILE).ok()?;
        // Soft limit can be "unlimited" (often u64::MAX or a very large value)
        let limit = if soft == rlimit::INFINITY || soft > MAX_AUTO_BATCH as u64 {
            MAX_AUTO_BATCH as u64
        } else {
            soft
        };
        let batch_u64 = limit.saturating_sub(RESERVED_FDS as u64);
        let batch = usize::try_from(batch_u64).unwrap_or(MAX_AUTO_BATCH);
        Some(batch.max(1))
    }

    #[cfg(windows)]
    {
        // Windows: getmaxstdio() is the C runtime limit for open streams; use it as a conservative proxy.
        let limit = rlimit::getmaxstdio();
        if limit == 0 {
            return Some(DEFAULT_AUTO_BATCH);
        }
        let batch = (limit as usize).saturating_sub(RESERVED_FDS).max(1);
        Some(batch.min(MAX_AUTO_BATCH))
    }
}

/// Whether to run the pipeline in a single batch or in path batches (to limit open files).
#[derive(Debug, Clone, Copy)]
pub enum PathBatchMode {
    /// Process all paths in one go (Phase 1 + Phase 2 on full set).
    Single,
    /// Process paths in chunks of this size; mmaps dropped between chunks.
    Batched { batch_size: usize },
}

#[must_use]
pub fn determine_batch_size(input_paths: &[String]) -> PathBatchMode {
    let batch_size = fd_limit_batch_size().unwrap_or(DEFAULT_AUTO_BATCH);
    if input_paths.len() > batch_size {
        debug!(
            "Path batch size: {} (path count {}), running in batches",
            batch_size,
            input_paths.len()
        );
        PathBatchMode::Batched { batch_size }
    } else {
        PathBatchMode::Single
    }
}

/// Process files with adaptive batching based on file count and worker count
/// Uses a scaled heuristic: batching kicks in when files > workers * `threshold_multiplier`
/// This prevents thread pool saturation for large batches while maintaining
/// optimal performance for smaller batches (tested: 224 files = 40s without batching)
pub fn process_files_with_adaptive_batching<R, F>(
    tasks: &[ProcessingTask],
    max_workers: usize,
    threshold_multiplier: usize,
    f: F,
) -> Vec<R>
where
    F: Fn(&ProcessingTask) -> R + Send + Sync,
    R: Send,
{
    // Scaled heuristic: threshold = workers * multiplier
    // This scales with available parallelism rather than a fixed number
    // Multiplier of 50 means: 13 workers = 650 file threshold, 20 workers = 1000 threshold
    let batching_threshold = max_workers * threshold_multiplier;

    if tasks.len() > batching_threshold {
        // Large batches: use adaptive batching to prevent thread pool saturation
        // Batch size scales with how far above threshold we are:
        let ratio = tasks.len() as f64 / batching_threshold as f64;
        let batch_multiplier = if ratio < 2.0 {
            2 // Light batching for 1-2x threshold
        } else if ratio < 3.0 {
            3 // Moderate batching for 2-3x threshold
        } else {
            4 // Heavy batching for 3x+ threshold
        };
        let batch_size = (max_workers * batch_multiplier).max(1);
        debug!(
            "Adaptive batching: files={}, threshold={}, ratio={:.2}, batch_multiplier={}, batch_size={}",
            tasks.len(),
            batching_threshold,
            ratio,
            batch_multiplier,
            batch_size
        );
        tasks.par_iter().with_min_len(batch_size).map(f).collect()
    } else {
        // Small batches: full parallelism is optimal
        tasks.par_iter().map(f).collect()
    }
}
