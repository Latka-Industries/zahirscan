use std::time::Duration;

use anyhow::Result;
use kdam::Animation;
use log::{debug, error};

use crate::config::RuntimeConfig;
use crate::engine::{
    chunking::{AdaptiveChunking, ProcessingTask, process_files_with_adaptive_batching},
    progress::{ProgressBarConfig, create_progress_bar},
};
use crate::parsers::extract_templates;
use crate::results::{Output, Phase2Result};
use crate::setup::OutputSink;
use crate::utils::path_string_helper::{format_bytes, print_progress_handler};

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
        config.flags.show_progress,
    );
}

/// Phase 2: Template mining and processing.
/// Writes to files (unless `skip_file_write` is true). Returns outputs and per-file failures (no short-circuit).
/// When `output_sink` is [`StreamOnly`](OutputSink::StreamOnly) or [`Channel`](OutputSink::Channel), each result is emitted there and not collected; `outputs` is empty.
#[must_use]
pub fn phase2_mining(
    tasks: &[ProcessingTask],
    config: &RuntimeConfig,
    adaptive: &AdaptiveChunking,
    skip_file_write: bool,
    output_sink: &OutputSink,
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
    let pb = if config.flags.show_progress {
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
        tasks,
        config.max_workers,
        config.threshold_multiplier,
        |task| {
            let r = crate::with_progress!(
                &pb,
                process_task_phase2(task, &phase2_config, skip_file_write)
            );
            if let Ok(out) = &r {
                match output_sink {
                    OutputSink::StreamOnly(cb) => cb(task.stats.file_path.clone(), out.clone()),
                    OutputSink::Channel(tx) => {
                        let _ = tx.send((task.stats.file_path.clone(), out.clone()));
                    }
                    OutputSink::Collect => {}
                }
            }
            r
        },
    );

    // Calculate and log Phase 2 metrics
    log_phase2_metrics(phase2_start.elapsed(), tasks, config.max_workers, config);

    // Collect outputs and failures (partial success; no short-circuit). Only fill outputs when Collect.
    let mut outputs = Vec::new();
    let mut failed = Vec::new();
    for (result, task) in results.into_iter().zip(tasks.iter()) {
        match result {
            Ok(out) => {
                if matches!(output_sink, OutputSink::Collect) {
                    outputs.push(out);
                }
            }
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
