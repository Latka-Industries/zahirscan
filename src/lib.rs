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
//! let result = extract_zahir("file.log", OutputMode::Full, None, None, None)?;
//!
//! // Process with explicit config and optional output dir (None = no file write)
//! let config = zahirscan::RuntimeConfig::new();
//! let result = extract_zahir(
//!     vec!["file1.log", "file2.log"],
//!     OutputMode::Templates,
//!     Some(&config),
//!     None,
//!     None,
//! )?;
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! # Callback example (store each output in an object outside the call)
//!
//! Pass `on_output` to run a function as each file completes; store results in your own structure (e.g. TUI DB).
//!
//! ```no_run
//! use std::sync::Mutex;
//! use zahirscan::{extract_zahir, Output, OutputMode};
//!
//! let collected: Mutex<Vec<Output>> = Mutex::new(Vec::new());
//! let result = extract_zahir(
//!     ["file1.log", "file2.log"],
//!     OutputMode::Full,
//!     None,
//!     None,
//!     Some(&|out: Output| {
//!         collected.lock().unwrap().push(out);
//!     }),
//! )?;
//! // collected has each file's Output as it completed; result.outputs has the same layout
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! # Streaming input
//!
//! Use [`extract_zahir_from_stream`] when paths come from a channel (e.g. nefaxer's `on_entry`
//! callback). Producer sends path strings; when the sender is dropped, zahirscan drains the
//! receiver and runs the pipeline, streaming results via `on_output`.
//!
//! ```no_run
//! use std::sync::mpsc;
//! use zahirscan::{extract_zahir_from_stream, OutputMode};
//!
//! let (tx, rx) = mpsc::channel();
//! // In another thread: run nefaxer with on_entry: Some(|e| { tx.send(e.path.to_string_lossy().into_owned()).ok(); });
//! // Then drop(tx). This thread:
//! let result = extract_zahir_from_stream(rx, OutputMode::Full, None, None, None)?;
//! # Ok::<(), anyhow::Error>(())
//! ```

// Binary name
pub const PKG_NAME: &str = env!("CARGO_PKG_NAME");

pub use config::DEFAULT_CONFIG_TOML;

pub mod analysis;
pub mod config;
pub mod engine;
pub mod parsers;
pub mod results;
pub mod setup;

// Re-export all public types and functions
pub use config::RuntimeConfig;
pub use engine::chunking::{ProcessingTask, calculate_adaptive_chunking};
pub use engine::orchestrator::{phase1_scan, phase2_mining, run_pipeline};
pub use engine::tools::*;
pub use parsers::{FileType, ParseResult, extract_templates, initial_file_scan};
pub use results::*;

// Single entry-point API
use anyhow::Result;
use engine::ToPathIter;
use std::sync::mpsc::Receiver;

/// Single entry point: extract templates and metadata from one or more files.
///
/// * `paths` - A single path or multiple paths (see [`ToPathIter`]).
/// * `mode` - Output mode (Templates or Full).
/// * `config` - If `None`, uses embedded default config only (no overlay). Overlay is for CLI via `setup::load_config()`.
/// * `output_dir` - If `Some(dir)`, writes per-file output under that directory; if `None`, skips file write (library usage) or uses temp (CLI).
/// * `on_output` - If `Some`, invoked from worker threads as each file's [`Output`] is ready. Errors are in the returned [`ZahirScanResult::phase1_failed`] and [`phase2_failed`](ZahirScanResult::phase2_failed).
///
/// Returns [`ZahirScanResult`] with `outputs`, `phase1_failed`, and `phase2_failed`.
///
/// # Example
///
/// See crate-level docs for examples (i.e. no callback, with callback).
#[allow(private_bounds)]
pub fn extract_zahir<P: ToPathIter>(
    paths: P,
    mode: OutputMode,
    config: Option<&RuntimeConfig>,
    output_dir: Option<&str>,
    on_output: Option<&(dyn Fn(Output) + Send + Sync)>,
) -> Result<ZahirScanResult> {
    let config = match config {
        Some(c) => c.clone(),
        None => {
            RuntimeConfig::load_config_with_overlay(DEFAULT_CONFIG_TOML, None::<&std::path::Path>)
                .unwrap_or_default()
        }
    };
    config.validate_external()?;

    let path_strings = paths.to_path_iter();
    if path_strings.is_empty() {
        return Err(anyhow::anyhow!("No file paths provided"));
    }

    let mut config_with_mode = config;
    config_with_mode.output_mode = mode;
    let (phase1_failed, phase2) =
        run_pipeline(&path_strings, output_dir, &config_with_mode, on_output)?;

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
/// * `paths_rx` - Channel receiver; block until closed, collecting all path strings.
/// * `mode`, `config`, `output_dir`, `on_output` - Same as [`extract_zahir`].
#[allow(clippy::module_name_repetitions)]
pub fn extract_zahir_from_stream(
    paths_rx: Receiver<String>,
    mode: OutputMode,
    config: Option<&RuntimeConfig>,
    output_dir: Option<&str>,
    on_output: Option<&(dyn Fn(Output) + Send + Sync)>,
) -> Result<ZahirScanResult> {
    let path_strings: Vec<String> = paths_rx.iter().collect();
    extract_zahir(&path_strings, mode, config, output_dir, on_output)
}
