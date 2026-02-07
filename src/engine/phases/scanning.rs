use std::path::Path;
use std::time::Duration;

use kdam::Animation;
use log::{debug, error};
use rayon::iter::IntoParallelRefIterator;
use rayon::prelude::*;

use crate::config::RuntimeConfig;
use crate::engine::chunking::ProcessingTask;
use crate::engine::progress::{ProgressBarConfig, create_progress_bar};
use crate::parsers::initial_file_scan_with_mmap;
use crate::results::Phase1Result;
use crate::utils::path_string_helper::{
    determine_output_path, print_progress_handler, should_ignore_path,
};

/// Log Phase 1 processing metrics
pub fn log_phase1_metrics(
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

/// Filter input paths for Phase 1: skip directories and paths that match ignore patterns
fn phase1_path_filter(p: &str, config: &RuntimeConfig) -> bool {
    if Path::new(p).is_dir() {
        debug!("Skipping directory: {}", p);
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
    output_dir: Option<&str>,
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
                let output_path = determine_output_path(input_path, output_dir, config);
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
