//! Orchestration logic for processing multiple files

use crate::config::Config;
use crate::parsers::{ParseResult, initial_file_scan};
use crate::tools::determine_output_path;
use anyhow::Result;
use log::{debug, error};
use rayon::prelude::*;

/// File processing task with stats and output path
pub struct ProcessingTask {
    pub stats: ParseResult,
    pub output_path: String,
}

/// Phase 1: Initial scan to collect stats and prepare for template mining
pub fn phase1_scan(
    input_paths: &[String],
    output: Option<&str>,
    output_is_dir: bool,
    config: &Config,
) -> Vec<ProcessingTask> {
    // Phase 1: Initial file scan for all files in parallel
    let stats_results: Vec<_> = input_paths
        .par_iter()
        .map(|input_path| initial_file_scan(input_path))
        .collect();

    // Determine output paths and collect valid stats
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
    tasks
}

/// Phase 2: Template mining and processing
/// Writes to files and returns Output objects
pub fn phase2_mining(
    tasks: Vec<ProcessingTask>,
    config: &Config,
) -> Result<Vec<crate::results::Output>> {
    // Process files in batches if max_concurrent_files is set
    if config.max_concurrent_files > 0 && tasks.len() > config.max_concurrent_files {
        debug!(
            "Processing {} files in batches of {} to reduce contention",
            tasks.len(),
            config.max_concurrent_files
        );

        let mut all_outputs = Vec::new();
        for chunk in tasks.chunks(config.max_concurrent_files) {
            let chunk_results: Vec<_> = chunk
                .par_iter()
                .map(|task| process_single_task(task, config))
                .collect();

            // Collect results from this batch
            for result in chunk_results {
                all_outputs.push(result?);
            }
        }
        Ok(all_outputs)
    } else {
        // Process all files in parallel (original behavior)
        let results: Vec<_> = tasks
            .par_iter()
            .map(|task| process_single_task(task, config))
            .collect();

        // Collect all outputs (or handle errors)
        let mut outputs = Vec::new();
        for result in results {
            outputs.push(result?);
        }

        Ok(outputs)
    }
}

/// Process a single file task (extracted for reuse)
fn process_single_task(task: &ProcessingTask, config: &Config) -> Result<crate::results::Output> {
    use std::time::Instant;

    let start = Instant::now();
    let mut stats = task.stats.clone();

    match stats.is_binary {
        true => {
            stats.duration = start.elapsed();
        }
        false => {
            // Template mining
            match crate::parsers::extract_templates(&stats, config) {
                Ok(mining_result) => {
                    stats.mining_result = Some(mining_result);
                }
                Err(e) => {
                    error!("Error extracting templates for {}: {}", stats.file_path, e);
                }
            }
            stats.duration = start.elapsed();
        }
    }

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
