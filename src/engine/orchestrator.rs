//! Orchestration logic for processing multiple files

use anyhow::Result;

use super::chunking::{PathBatchMode, calculate_adaptive_chunking, determine_batch_size};
use super::phases::{mining::phase2_mining, scanning::phase1_scan};
use crate::config::RuntimeConfig;
use crate::results::Phase2Result;
use crate::setup::OutputSink;

/// Run the full pipeline: Phase 1 → empty-tasks check → adaptive chunking → Phase 2.
/// When path count exceeds the batch size, processes in batches so mmaps are dropped between batches (avoids "too many open files").
/// Batch size: `config.path_batch_size` if > 0; otherwise derived from the process fd limit ([`fd_limit_batch_size`]) or [`DEFAULT_AUTO_BATCH`].
/// Caller must set `config.output_mode` before calling. Returns phase1 failed list and phase2 result.
/// `output_sink` controls where results go; when not [`Collect`](OutputSink::Collect), `Phase2Result::outputs` is empty.
pub fn run_pipeline(
    input_paths: &[String],
    output_dir: Option<&str>,
    config: &RuntimeConfig,
    output_sink: &OutputSink,
) -> Result<(Vec<(String, String)>, Phase2Result)> {
    let skip_file_write = output_dir.is_none();

    match determine_batch_size(input_paths) {
        PathBatchMode::Single => run_pipeline_single(
            input_paths,
            output_dir,
            config,
            output_sink,
            skip_file_write,
        ),
        PathBatchMode::Batched { batch_size } => run_pipeline_batched(
            input_paths,
            output_dir,
            config,
            output_sink,
            skip_file_write,
            batch_size,
        ),
    }
}

/// Single-batch path: Phase 1 on all paths, then Phase 2. No batching.
fn run_pipeline_single(
    input_paths: &[String],
    output_dir: Option<&str>,
    config: &RuntimeConfig,
    output_sink: &OutputSink,
    skip_file_write: bool,
) -> Result<(Vec<(String, String)>, Phase2Result)> {
    let phase1 = phase1_scan(input_paths, output_dir, config);
    let tasks = phase1.tasks;
    if tasks.is_empty() {
        let msg = if phase1.failed.is_empty() {
            "No valid files found. All provided paths failed to scan or do not exist".to_string()
        } else {
            let details: Vec<String> = phase1
                .failed
                .iter()
                .map(|(p, e)| format!("{}: {}", p, e))
                .collect();
            format!(
                "No valid files found. {} path(s) failed: {}",
                phase1.failed.len(),
                details.join("; ")
            )
        };
        return Err(anyhow::anyhow!("{}", msg));
    }
    let adaptive = calculate_adaptive_chunking(&tasks, config.max_workers, config);
    let phase2 = phase2_mining(tasks, config, &adaptive, skip_file_write, output_sink);
    Ok((phase1.failed, phase2))
}

/// Batched path: Phase 1 + Phase 2 per chunk of paths; mmaps dropped after each chunk.
fn run_pipeline_batched(
    input_paths: &[String],
    output_dir: Option<&str>,
    config: &RuntimeConfig,
    output_sink: &OutputSink,
    skip_file_write: bool,
    batch_size: usize,
) -> Result<(Vec<(String, String)>, Phase2Result)> {
    let mut all_phase1_failed = Vec::new();
    let mut all_phase2_failed = Vec::new();
    let mut all_outputs = Vec::new();
    let mut any_batch_had_tasks = false;

    for chunk in input_paths.chunks(batch_size) {
        let phase1 = phase1_scan(chunk, output_dir, config);
        if phase1.tasks.is_empty() {
            all_phase1_failed.extend(phase1.failed);
            continue;
        }
        any_batch_had_tasks = true;
        let adaptive = calculate_adaptive_chunking(&phase1.tasks, config.max_workers, config);
        let phase2 = phase2_mining(
            phase1.tasks,
            config,
            &adaptive,
            skip_file_write,
            output_sink,
        );
        all_phase1_failed.extend(phase1.failed);
        all_phase2_failed.extend(phase2.failed);
        if matches!(output_sink, OutputSink::Collect) {
            all_outputs.extend(phase2.outputs);
        }
        // phase1.tasks and phase2 go out of scope here → mmaps dropped before next chunk
    }

    if !any_batch_had_tasks {
        let msg = if all_phase1_failed.is_empty() {
            "No valid files found. All provided paths failed to scan or do not exist".to_string()
        } else {
            let details: Vec<String> = all_phase1_failed
                .iter()
                .take(5)
                .map(|(p, e)| format!("{}: {}", p, e))
                .collect::<Vec<_>>();
            let more = all_phase1_failed.len().saturating_sub(5);
            let suffix = if more > 0 {
                format!(" (and {} more)", more)
            } else {
                String::new()
            };
            format!(
                "No valid files found. {} path(s) failed: {}{}",
                all_phase1_failed.len(),
                details.join("; "),
                suffix
            )
        };
        return Err(anyhow::anyhow!("{}", msg));
    }

    Ok((
        all_phase1_failed,
        Phase2Result {
            outputs: all_outputs,
            failed: all_phase2_failed,
        },
    ))
}
