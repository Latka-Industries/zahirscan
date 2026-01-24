//! Orchestration logic for processing multiple files

use crate::chunking::{AdaptiveChunking, ProcessingTask};
use crate::config::Config;
use crate::parsers::{extract_templates, initial_file_scan};
use crate::results::Output;
use crate::tools::{determine_output_path, format_bytes};
use anyhow::Result;
use log::{debug, error};
use rayon::prelude::*;
use std::time::Duration;

/// Log Phase 1 processing metrics
fn log_phase1_metrics(
    duration: Duration,
    scan_duration: Duration,
    path_duration: Duration,
    input_file_count: usize,
    tasks: &[ProcessingTask],
) {
    let duration_secs = duration.as_secs_f64();
    let scan_secs = scan_duration.as_secs_f64();
    let path_secs = path_duration.as_secs_f64();
    let valid_file_count = tasks.len();

    debug!(
        "Phase 1: Processed {} files ({} valid) in {:.2}s (scan: {:.2}s, path setup: {:.2}s)",
        input_file_count, valid_file_count, duration_secs, scan_secs, path_secs
    );
}

/// Log Phase 2 processing metrics
fn log_phase2_metrics(duration: Duration, tasks: &[ProcessingTask], max_workers: usize) {
    let total_bytes = tasks.iter().map(|t| t.stats.byte_count).sum::<usize>();
    let file_count = tasks.len();
    let duration_secs = duration.as_secs_f64();

    let mean_time_per_file = if file_count > 0 {
        duration_secs / file_count as f64
    } else {
        0.0
    };

    let mean_size_per_file = if file_count > 0 {
        (total_bytes as f64 / file_count as f64) as usize
    } else {
        0
    };

    debug!(
        "Phase 2: Completed in {:.2}s. (workers: {}, files: {}, size: {}, size/file: {}, time/file: {:.4}s)",
        duration_secs,
        max_workers,
        file_count,
        format_bytes(total_bytes),
        format_bytes(mean_size_per_file),
        mean_time_per_file,
    );
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

    // Calculate Phase 1 metrics
    log_phase1_metrics(
        phase1_start.elapsed(),
        scan_duration,
        path_duration,
        input_paths.len(),
        &tasks,
    );

    tasks
}

/// Phase 2: Template mining and processing
/// Writes to files (unless `skip_file_write` is true) and returns Output objects
pub fn phase2_mining(
    tasks: Vec<ProcessingTask>,
    config: &Config,
    adaptive: &AdaptiveChunking,
    max_workers: usize,
    skip_file_write: bool,
) -> Result<Vec<Output>> {
    use std::time::Instant;

    let phase2_start = Instant::now();
    debug!(
        "Phase 2: Starting template mining for {} files",
        tasks.len()
    );

    // Process files in parallel with adaptive batching
    let results: Vec<_> = process_files_with_adaptive_batching(
        &tasks,
        max_workers,
        config.threshold_multiplier,
        |task| process_task_phase2(task, config, adaptive, max_workers, skip_file_write),
    );

    // Calculate and log Phase 2 metrics
    log_phase2_metrics(phase2_start.elapsed(), &tasks, max_workers);

    // Collect all outputs (or handle errors)
    let mut outputs = Vec::new();
    for result in results {
        outputs.push(result?);
    }

    Ok(outputs)
}

/// Process files with adaptive batching based on file count and worker count
/// Uses a scaled heuristic: batching kicks in when files > workers * threshold_multiplier
/// This prevents thread pool saturation for large batches while maintaining
/// optimal performance for smaller batches (tested: 224 files = 40s without batching)
fn process_files_with_adaptive_batching<R, F>(
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

/// Process a single file task (extracted for reuse)
fn process_task_phase2(
    task: &ProcessingTask,
    config: &Config,
    adaptive: &AdaptiveChunking,
    max_workers: usize,
    skip_file_write: bool,
) -> Result<Output> {
    use std::time::Instant;

    let start = Instant::now();
    let mut stats = task.stats.clone();

    // Images, videos, audio, PDFs, DOCX, XLSX, and CSVs are binary but still need metadata extraction
    let needs_processing = !stats.is_binary || stats.file_type.needs_processing();

    if needs_processing {
        // Create modified config with adaptive chunking
        // Calculate target number of chunks: multiplier * max_workers
        let target_chunks = adaptive.chunks_per_file_multiplier * max_workers;
        let mut phase2_config = config.clone();
        phase2_config.target_chunks_per_file = target_chunks;

        // Template mining (and image metadata extraction for images)
        stats.mining_result = extract_templates(&mut stats, &phase2_config)
            .inspect_err(|e| {
                error!("Error extracting templates for {}: {}", stats.file_path, e);
            })
            .ok();
    }
    stats.duration = start.elapsed();

    // Write to file (unless skipped for library usage)
    if !skip_file_write {
        stats
            .write_to_file(&task.output_path, config.output_mode, config)
            .inspect(|_| debug!("Output written to: {}", task.output_path))
            .inspect_err(|e| error!("Error writing {}: {}", task.output_path, e))?;
    }

    // Return the Output object
    Ok(stats.to_output(config.output_mode, config))
}
