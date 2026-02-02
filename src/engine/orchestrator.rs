//! Orchestration logic for processing multiple files

use super::chunking::{AdaptiveChunking, ProcessingTask};
use super::progress::{ProgressBarConfig, create_progress_bar};
use super::tools::{
    determine_output_path, format_bytes, print_progress_handler, should_ignore_path,
};
use crate::config::RuntimeConfig;
use crate::parsers::{extract_templates, initial_file_scan_with_mmap};
use crate::results::{Output, Phase1Result, Phase2Result};
use anyhow::Result;
use kdam::Animation;
use log::{debug, error, info};
use rayon::prelude::*;
use std::path::Path;
use std::time::Duration;

/// Log Phase 1 processing metrics
fn log_phase1_metrics(
    duration: Duration,
    scan_duration: Duration,
    path_duration: Duration,
    input_file_count: usize,
    tasks: &[ProcessingTask],
    config: &RuntimeConfig,
) {
    let duration_secs = duration.as_secs_f64();
    let scan_secs = scan_duration.as_secs_f64();
    let path_secs = path_duration.as_secs_f64();
    let valid_file_count = tasks.len();

    print_progress_handler(
        &format!(
            "Phase 1: Processed {} files ({} valid) in {:.2}s (scan: {:.2}s, path setup: {:.2}s)",
            input_file_count, valid_file_count, duration_secs, scan_secs, path_secs
        ),
        config.show_progress,
    );
}

/// Log Phase 2 processing metrics
fn log_phase2_metrics(
    duration: Duration,
    tasks: &[ProcessingTask],
    max_workers: usize,
    config: &RuntimeConfig,
) {
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

    print_progress_handler(
        &format!(
            "Phase 2: Completed in {:.2}s. (workers: {}, files: {}, size: {}, size/file: {}, time/file: {:.4}s)",
            duration_secs,
            max_workers,
            file_count,
            format_bytes(total_bytes),
            format_bytes(mean_size_per_file),
            mean_time_per_file
        ),
        config.show_progress,
    );
}

/// Filter input paths for Phase 1: skip directories and paths that match ignore patterns
fn phase1_path_filter(p: &str, config: &RuntimeConfig) -> bool {
    if Path::new(p).is_dir() {
        info!("Skipping directory: {}", p);
        false
    } else if should_ignore_path(p, config) {
        debug!("Skipping {} (matches ignore filter)", p);
        false
    } else {
        true
    }
}

/// Phase 1: Initial scan to collect stats and prepare for template mining.
/// Returns tasks and failed paths with error messages (for TUI/lib to display).
pub fn phase1_scan(
    input_paths: &[String],
    output: Option<&str>,
    config: &RuntimeConfig,
) -> Phase1Result {
    use std::time::Instant;

    let input_paths: Vec<String> = input_paths
        .iter()
        .filter(|p| phase1_path_filter(p, config))
        .cloned()
        .collect();

    let phase1_start = Instant::now();
    debug!(
        "Phase 1: Starting initial scan of {} files",
        input_paths.len()
    );

    // Phase 1: Initial file scan for all files in parallel
    let scan_start = Instant::now();
    let pb = if config.show_progress {
        Some(create_progress_bar(ProgressBarConfig::new(
            input_paths.len(),
            "Phase 1: Scanning files",
            Animation::TqdmAscii,
        )))
    } else {
        None
    };
    // Progress bar will close automatically on drop
    let stats_results: Vec<_> = input_paths
        .par_iter()
        .map(|input_path| crate::with_progress!(&pb, initial_file_scan_with_mmap(input_path)))
        .collect();
    let scan_duration = scan_start.elapsed();
    debug!(
        "Phase 1: File scanning completed in {:.2}s",
        scan_duration.as_secs_f64()
    );

    // Determine output paths and collect valid stats; collect failed (path, message) for TUI/lib
    let path_start = Instant::now();
    let mut tasks = Vec::new();
    let mut failed: Vec<(String, String)> = Vec::new();
    for (i, result) in stats_results.into_iter().enumerate() {
        match result {
            Ok((stats, mmap)) => {
                let input_path = &input_paths[i];
                let output_path = determine_output_path(input_path, output, config);
                tasks.push(ProcessingTask {
                    stats,
                    output_path,
                    mmap: Some(mmap),
                });
            }
            Err(e) => {
                let path = input_paths[i].clone();
                let msg = e.to_string();
                error!("Error collecting stats for {}: {}", path, e);
                failed.push((path, msg));
            }
        }
    }
    if !failed.is_empty() {
        debug!(
            "Phase 1: {} of {} paths failed: {}",
            failed.len(),
            input_paths.len(),
            failed
                .iter()
                .map(|(p, _)| p.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    let path_duration = path_start.elapsed();

    // Calculate Phase 1 metrics
    log_phase1_metrics(
        phase1_start.elapsed(),
        scan_duration,
        path_duration,
        input_paths.len(),
        &tasks,
        config,
    );

    Phase1Result { tasks, failed }
}

/// Phase 2: Template mining and processing.
/// Writes to files (unless `skip_file_write` is true). Returns outputs and per-file failures (no short-circuit).
pub fn phase2_mining(
    tasks: Vec<ProcessingTask>,
    config: &RuntimeConfig,
    adaptive: &AdaptiveChunking,
    skip_file_write: bool,
) -> Phase2Result {
    use std::time::Instant;

    let phase2_start = Instant::now();
    debug!(
        "Phase 2: Starting template mining for {} files",
        tasks.len()
    );

    // One config with target_chunks_per_file set for the whole batch (same value for every file)
    let target_chunks = adaptive.chunks_per_file_multiplier * config.max_workers;
    let mut phase2_config = config.clone();
    phase2_config.target_chunks_per_file = target_chunks;

    // Process files in parallel with adaptive batching
    let pb = if config.show_progress {
        Some(create_progress_bar(ProgressBarConfig::new(
            tasks.len(),
            "Phase 2: Processing files",
            Animation::TqdmAscii,
        )))
    } else {
        None
    };
    // Progress bar will close automatically on drop
    let results: Vec<_> = process_files_with_adaptive_batching(
        &tasks,
        config.max_workers,
        config.threshold_multiplier,
        |task| {
            crate::with_progress!(
                &pb,
                process_task_phase2(task, &phase2_config, skip_file_write)
            )
        },
    );

    // Calculate and log Phase 2 metrics
    log_phase2_metrics(phase2_start.elapsed(), &tasks, config.max_workers, config);

    // Collect outputs and failures (partial success; no short-circuit)
    let mut outputs = Vec::new();
    let mut failed = Vec::new();
    for (result, task) in results.into_iter().zip(tasks.iter()) {
        match result {
            Ok(out) => outputs.push(out),
            Err(e) => failed.push((task.stats.file_path.clone(), e.to_string())),
        }
    }
    if !failed.is_empty() {
        debug!(
            "Phase 2: {} of {} paths failed: {}",
            failed.len(),
            tasks.len(),
            failed
                .iter()
                .map(|(p, _)| p.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    Phase2Result { outputs, failed }
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
    config: &RuntimeConfig,
    skip_file_write: bool,
) -> Result<Output> {
    use std::time::Instant;

    let start = Instant::now();
    let mut stats = task.stats.clone();

    // Images, videos, audio, PDFs, DOCX, XLSX, and CSVs are binary but still need metadata extraction
    let needs_processing = !stats.is_binary || stats.file_type.binary_needs_processing();

    if needs_processing {
        // config already has target_chunks_per_file set (once per batch in phase2_mining)
        stats.mining_result = extract_templates(&mut stats, config, task.mmap.as_ref())
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
