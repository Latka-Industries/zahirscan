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
//! # Simple API Example
//!
//! ```no_run
//! use zahirscan::{extract_schema, OutputMode};
//!
//! // Process a single file
//! let result = extract_schema("file.log", OutputMode::Full)?;
//! println!("Found {} templates", result.outputs[0].templates.len());
//!
//! // Process multiple files
//! let files = vec!["file1.log", "file2.log"];
//! let result = extract_schema(files.as_slice(), OutputMode::Full)?;
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! # Advanced API Example
//!
//! ```no_run
//! use zahirscan::{RuntimeConfig, phase1_scan, phase2_mining, calculate_adaptive_chunking};
//!
//! let config = RuntimeConfig::new();
//! let paths = vec!["file.log".to_string()];
//! let phase1 = phase1_scan(&paths, None, &config);
//! let tasks = phase1.tasks;
//! let adaptive = calculate_adaptive_chunking(&tasks, config.max_workers, &config);
//! let phase2 = phase2_mining(tasks, &config, &adaptive, false);
//! let outputs = phase2.outputs;
//! # Ok::<(), anyhow::Error>(())
//! ```

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
pub use engine::orchestrator::{phase1_scan, phase2_mining};
pub use engine::tools::*;
pub use parsers::{FileType, ParseResult, extract_templates, initial_file_scan};
pub use results::*;

// Simple API wrapper functions
use anyhow::Result;
use engine::ToPathIter;

/// Extract schema (templates and metadata) from one or more files.
///
/// Same return type as [`extract_schema_with_config`]: a [`ZahirScanResult`] with
/// `outputs`, `phase1_failed`, and `phase2_failed`. Uses embedded default config only
/// (no user config file).
///
/// # Arguments
///
/// * `paths` - A single file path (`&str`), multiple paths (`&[&str]` or `Vec<&str>`), or a single String (`&String` or `String`)
/// * `mode` - Output mode (Templates or Full)
///
/// # Example - Single file
///
/// ```no_run
/// use zahirscan::{extract_schema, OutputMode};
///
/// let result = extract_schema("document.log", OutputMode::Full)?;
/// println!("Templates: {}", result.outputs[0].templates.len());
/// # Ok::<(), anyhow::Error>(())
/// ```
///
/// # Example - Multiple files
///
/// ```no_run
/// use zahirscan::{extract_schema, OutputMode};
///
/// let files = vec!["file1.log", "file2.log", "file3.log"];
/// let result = extract_schema(&files, OutputMode::Full)?;
/// for output in &result.outputs {
///     println!("Templates: {}", output.templates.len());
/// }
/// # Ok::<(), anyhow::Error>(())
/// ```
///
#[allow(private_bounds)]
pub fn extract_schema<P: ToPathIter>(paths: P, mode: OutputMode) -> Result<ZahirScanResult> {
    let config =
        RuntimeConfig::load_config_with_overlay(DEFAULT_CONFIG_TOML, None::<&std::path::Path>)
            .unwrap_or_default();
    config.validate_external()?;
    extract_schema_with_config(paths, mode, &config)
}

/// Extract schema and metadata from files with a provided configuration.
///
/// This is the advanced version of [`extract_schema`] that allows you to provide a custom
/// configuration. Use this when you need to:
/// - Reuse the same config across multiple calls (optimal for TUI/loops)
/// - Provide a programmatically constructed config
/// - Avoid repeated disk I/O from loading `config.toml`
///
/// For simple one-time usage, prefer [`extract_schema`] which loads config automatically.
///
/// # Example - Reusing config (optimal for loops/TUI)
///
/// ```no_run
/// use zahirscan::{extract_schema_with_config, OutputMode, RuntimeConfig};
///
/// let config = RuntimeConfig::new();
/// let file_batches = vec![
///     vec!["batch1_file1.log", "batch1_file2.log"],
///     vec!["batch2_file1.log", "batch2_file2.log"],
/// ];
///
/// for batch in file_batches {
///     let result = extract_schema_with_config(batch.as_slice(), OutputMode::Templates, &config)?;
///     let outputs = result.outputs;
///     // result.phase1_failed, result.phase2_failed for TUI display
/// }
/// # Ok::<(), anyhow::Error>(())
/// ```
#[allow(private_bounds)]
pub fn extract_schema_with_config<P: ToPathIter>(
    paths: P,
    mode: OutputMode,
    config: &RuntimeConfig,
) -> Result<ZahirScanResult> {
    config.validate_external()?;

    let path_strings = paths.to_path_iter();

    // Validate input - fail fast with clear error
    if path_strings.is_empty() {
        return Err(anyhow::anyhow!("No file paths provided"));
    }

    // Phase 1: Initial scan
    let phase1 = phase1_scan(&path_strings, None, config);
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

    // Calculate adaptive chunking
    let adaptive = calculate_adaptive_chunking(&tasks, config.max_workers, config);

    // Phase 2: Template mining and metadata extraction
    let mut config_with_mode = config.clone();
    config_with_mode.output_mode = mode;
    let phase2 = phase2_mining(tasks, &config_with_mode, &adaptive, true);

    Ok(ZahirScanResult {
        outputs: phase2.outputs,
        phase1_failed: phase1.failed,
        phase2_failed: phase2.failed,
    })
}
