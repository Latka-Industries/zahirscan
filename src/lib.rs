//! Token-efficient content compression using probabilistic template mining.
//!
//! This crate provides tools for analyzing text files, logs, and media files to extract
//! templates and metadata. It uses probabilistic template mining to compress content
//! while preserving structure, making it efficient for AI consumption.
//!
//! # Main Workflow
//!
//! 1. **Phase 1**: Initial file scan to collect statistics and prepare for processing
//! 2. **Phase 2**: Template mining and metadata extraction
//!
//! # API Example
//!
//! ```no_run
//! use zahirscan::{extract_zahir, OutputMode};
//!
//! // Process with default config (no overlay)
//! let result = extract_zahir("file.log", OutputMode::Full, None, None, &zahirscan::OutputSink::Collect)?;
//!
//! // Process with explicit config and optional output dir (None = no file write)
//! let config = zahirscan::RuntimeConfig::new();
//! let result = extract_zahir(
//!     vec!["file1.log", "file2.log"],
//!     OutputMode::Templates,
//!     Some(&config),
//!     None,
//!     &zahirscan::OutputSink::Collect,
//! )?;
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! # Stream-only example (no collection, bounded memory)
//!
//! Use [`OutputSink::StreamOnly`] to receive each `(path, Output)` in a callback; the engine does not collect.
//!
//! ```no_run
//! use std::sync::{Arc, Mutex};
//! use zahirscan::{extract_zahir, Output, OutputMode, OutputSink};
//!
//! let collected = Arc::new(Mutex::new(Vec::<(String, zahirscan::Output)>::new()));
//! let collected_clone = Arc::clone(&collected);
//! let sink = OutputSink::StreamOnly(Box::new(move |path, out| {
//!     collected_clone.lock().unwrap().push((path, out));
//! }));
//! let result = extract_zahir(
//!     ["file1.log", "file2.log"],
//!     OutputMode::Full,
//!     None,
//!     None,
//!     &sink,
//! )?;
//! // result.outputs is empty; collected has each (path, Output) as it completed
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! # Streaming input
//!
//! Use [`extract_zahir_from_stream`] when paths come from a channel (e.g. nefaxer's `on_entry`
//! callback). Producer sends path strings; when the sender is dropped, zahirscan drains the
//! receiver and runs the pipeline. Pass [`OutputSink::Collect`] to get all outputs in the result, or [`OutputSink::StreamOnly`] / [`OutputSink::Channel`] to stream out.
//!
//! ```no_run
//! use std::sync::mpsc;
//! use zahirscan::{extract_zahir_from_stream, OutputMode, OutputSink};
//!
//! let (tx, rx) = mpsc::channel();
//! // In another thread: run nefaxer with on_entry: Some(|e| { tx.send(e.path.to_string_lossy().into_owned()).ok(); });
//! // Then drop(tx). This thread:
//! let result = extract_zahir_from_stream(&rx, OutputMode::Full, None, None, &OutputSink::Collect)?;
//! # Ok::<(), anyhow::Error>(())
//! ```

// Binary name
pub const PKG_NAME: &str = env!("CARGO_PKG_NAME");

pub use config::DEFAULT_CONFIG_TOML;

pub mod analysis;
pub mod config;
pub mod engine;
mod metadata_fields;
pub mod parsers;
pub mod results;
pub mod setup;
pub mod utils;

// Re-export all public types and functions
pub use config::{RuntimeConfig, RuntimeFlags};
pub use engine::chunking::{ProcessingTask, calculate_adaptive_chunking};
pub use engine::orchestrator::run_pipeline;
pub use engine::phases::{mining::phase2_mining, scanning::phase1_scan};
pub use parsers::{FileType, ParseResult, extract_templates, initial_file_scan};
pub use results::*;
pub use setup::OutputSink;
pub use utils::*;

// Single entry-point API
use anyhow::Result;
use log::debug;
use std::sync::mpsc::Receiver;
use utils::path_string_helper::ToPathIter;

/// Single entry point: extract templates and metadata from one or more files.
///
/// * `paths` - A single path or multiple paths (see [`ToPathIter`]).
/// * `mode` - Output mode (Templates or Full).
/// * `config` - If `None`, uses embedded default config only (no overlay). Overlay is for CLI via `setup::load_config()`.
/// * `output_dir` - If `Some(dir)`, writes per-file output under that directory; if `None`, skips file write (library usage) or uses temp (CLI).
/// * `output_sink` - Where results go: [`OutputSink::Collect`] (default), [`OutputSink::StreamOnly`] (callback, no collection), or [`OutputSink::Channel`] (send on channel, no collection). Pass by reference (e.g. `&OutputSink::Collect`). Errors are always in the returned [`ZahirScanResult::phase1_failed`] and [`phase2_failed`](ZahirScanResult::phase2_failed).
///
/// Returns [`ZahirScanResult`]; `outputs` is empty when using `StreamOnly` or `Channel`.
///
/// # Errors
///
/// Returns [`anyhow::Error`] when no paths are given, [`RuntimeConfig`] validation fails, or the processing pipeline fails (I/O, mmap, parsing, etc.).
///
/// # Example
///
/// See crate-level docs for examples (i.e. Collect, or `StreamOnly` / Channel).
#[allow(private_bounds)]
pub fn extract_zahir<P: ToPathIter>(
    paths: P,
    mode: OutputMode,
    config: Option<&RuntimeConfig>,
    output_dir: Option<&str>,
    output_sink: &OutputSink,
) -> Result<ZahirScanResult> {
    let config = match config {
        Some(c) => c.clone(),
        None => {
            RuntimeConfig::load_config_with_overlay(DEFAULT_CONFIG_TOML, None::<&std::path::Path>)
                .unwrap_or_default()
        }
    };
    config.validate_external()?;
    let config_str = format!("{} CONFIG: {:#?}", PKG_NAME.to_uppercase(), config);
    debug!("{config_str}");

    let path_strings = paths.to_path_iter();
    if path_strings.is_empty() {
        return Err(anyhow::anyhow!("No file paths provided"));
    }

    let mut config_with_mode = config;
    config_with_mode.output_mode = mode;
    let (phase1_failed, phase2) =
        run_pipeline(&path_strings, output_dir, &config_with_mode, output_sink)?;

    Ok(ZahirScanResult {
        outputs: phase2.outputs,
        phase1_failed,
        phase2_failed: phase2.failed,
    })
}

/// Extract templates and metadata from paths received over a channel (streaming input).
///
/// Drains `paths_rx` until the sender is dropped (channel closed), then runs the same pipeline
/// as [`extract_zahir`]. Use this when paths are produced by another component (e.g. [nefaxer]'s
/// `on_entry` callback): spawn a thread that sends paths into the channel; when the producer is
/// done, drop the sender; this function then processes all received paths and streams results
/// via `on_output`.
///
/// * `paths_rx` - Reference to the channel receiver; block until closed, collecting all path strings.
/// * `mode`, `config`, `output_dir`, `output_sink` - Same as [`extract_zahir`].
///
/// # Errors
///
/// Same as [`extract_zahir`] (delegates after collecting paths from the channel).
#[allow(clippy::module_name_repetitions)]
pub fn extract_zahir_from_stream(
    paths_rx: &Receiver<String>,
    mode: OutputMode,
    config: Option<&RuntimeConfig>,
    output_dir: Option<&str>,
    output_sink: &OutputSink,
) -> Result<ZahirScanResult> {
    let path_strings: Vec<String> = paths_rx.iter().collect();
    extract_zahir(&path_strings, mode, config, output_dir, output_sink)
}
