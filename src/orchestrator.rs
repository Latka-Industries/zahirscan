//! Orchestration logic for processing multiple files

use crate::config::Config;
use crate::parsers::{FileType, ParseResult, initial_file_scan};
use crate::tools::{determine_output_path, format_bytes};
use anyhow::Result;
use log::{debug, error};
use rayon::prelude::*;

/// File processing task with stats and output path
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

/// Calculate adaptive chunking settings based on Phase 1 stats
/// Returns the multiplier for chunks per file (e.g., 1 = 13 chunks, 2 = 26 chunks for 13 workers)
pub fn calculate_adaptive_chunking(
    tasks: &[ProcessingTask],
    max_workers: usize,
    config: &crate::config::Config,
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

/// Phase 1: Initial scan to collect stats and prepare for template mining
pub fn phase1_scan(
    input_paths: &[String],
    output: Option<&str>,
    output_is_dir: bool,
    config: &Config,
) -> Vec<ProcessingTask> {
    use std::time::Instant;

    let phase1_start = Instant::now();
    debug!(
        "Phase 1: Starting initial scan of {} files",
        input_paths.len()
    );

    // Phase 1: Initial file scan for all files in parallel
    let scan_start = Instant::now();
    let stats_results: Vec<_> = input_paths
        .par_iter()
        .map(|input_path| initial_file_scan(input_path))
        .collect();
    let scan_duration = scan_start.elapsed();
    debug!(
        "Phase 1: File scanning completed in {:.2}s",
        scan_duration.as_secs_f64()
    );

    // Determine output paths and collect valid stats
    let path_start = Instant::now();
    let mut tasks = Vec::new();
    for (i, result) in stats_results.into_iter().enumerate() {
        match result {
            Ok(stats) => {
                let input_path = &input_paths[i];
                let output_path = determine_output_path(input_path, output, output_is_dir, config);
                tasks.push(ProcessingTask { stats, output_path });
            }
            Err(e) => {
                error!("Error collecting stats for {}: {}", input_paths[i], e);
            }
        }
    }
    let path_duration = path_start.elapsed();
    let phase1_duration = phase1_start.elapsed();

    debug!(
        "Phase 1: Processed {} files ({} valid) in {:.2}s (scan: {:.2}s, path setup: {:.2}s)",
        input_paths.len(),
        tasks.len(),
        phase1_duration.as_secs_f64(),
        scan_duration.as_secs_f64(),
        path_duration.as_secs_f64()
    );

    tasks
}

/// Phase 2: Template mining and processing
/// Writes to files and returns Output objects
pub fn phase2_mining(
    tasks: Vec<ProcessingTask>,
    config: &Config,
    adaptive: &AdaptiveChunking,
    max_workers: usize,
) -> Result<Vec<crate::results::Output>> {
    // Process all files in parallel (no batching)
    let results: Vec<_> = tasks
        .par_iter()
        .map(|task| process_single_task(task, config, adaptive, max_workers))
        .collect();

    // Collect all outputs (or handle errors)
    let mut outputs = Vec::new();
    for result in results {
        outputs.push(result?);
    }

    Ok(outputs)
}

/// Process a single file task (extracted for reuse)
fn process_single_task(
    task: &ProcessingTask,
    config: &Config,
    adaptive: &AdaptiveChunking,
    max_workers: usize,
) -> Result<crate::results::Output> {
    use std::time::Instant;

    let start = Instant::now();
    let mut stats = task.stats.clone();

    // Images, videos, and audio files are binary but still need metadata extraction
    let needs_processing = !stats.is_binary
        || stats.file_type == crate::parsers::FileType::Image
        || stats.file_type == crate::parsers::FileType::Video
        || stats.file_type == crate::parsers::FileType::Audio;

    if needs_processing {
        // Create modified config with adaptive chunking
        // Calculate target number of chunks: multiplier * max_workers
        let target_chunks = adaptive.chunks_per_file_multiplier * max_workers;
        let mut phase2_config = config.clone();
        phase2_config.target_chunks_per_file = target_chunks;

        // Template mining (and image metadata extraction for images)
        match crate::parsers::extract_templates(&mut stats, &phase2_config) {
            Ok(mining_result) => {
                stats.mining_result = Some(mining_result);
            }
            Err(e) => {
                error!("Error extracting templates for {}: {}", stats.file_path, e);
            }
        }
    }
    stats.duration = start.elapsed();

    // Write to file
    match stats.write_to_file(&task.output_path, config.output_mode) {
        Ok(_) => {
            debug!("Output written to: {}", task.output_path);
        }
        Err(e) => {
            error!("Error writing {}: {}", task.output_path, e);
            return Err(e);
        }
    }

    // Return the Output object
    Ok(stats.to_output(config.output_mode))
}
