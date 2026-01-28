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
//! let outputs = extract_schema("file.log", OutputMode::Full)?;
//! println!("Found {} templates", outputs[0].templates.len());
//!
//! // Process multiple files
//! let files = vec!["file1.log", "file2.log"];
//! let outputs = extract_schema(files.as_slice(), OutputMode::Full)?;
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! # Advanced API Example
//!
//! ```no_run
//! use zahirscan::{Config, phase1_scan, phase2_mining, calculate_adaptive_chunking};
//!
//! let config = Config::load().unwrap_or_default();
//! let paths = vec!["file.log".to_string()];
//! let tasks = phase1_scan(&paths, None, false, &config);
//! let adaptive = calculate_adaptive_chunking(&tasks, config.max_workers, &config);
//! let outputs = phase2_mining(tasks, &config, &adaptive, false)?;
//! # Ok::<(), anyhow::Error>(())
//! ```

pub mod engine;
pub mod parsers;
pub mod results;

// Re-export chunking utilities
pub use engine::chunking::{ProcessingTask, calculate_adaptive_chunking};

// Re-export configuration
pub use engine::config::Config;

// Re-export orchestrator functions (main entry points)
pub use engine::orchestrator::{phase1_scan, phase2_mining};

// Re-export parser types and functions
pub use parsers::{FileType, ParseResult, extract_templates, initial_file_scan};

// Re-export result types
pub use results::{
    AudioMetadata, CompressionStats, CsvMetadata, DocumentMetadata, FileMetadata, ImageMetadata,
    MiningResult, Output, OutputMode, PdfMetadata, Template, VideoMetadata,
};

// Re-export utility functions
pub use engine::tools::{
    detect_file_type, determine_output_path, format_bytes, format_duration, get_temp_output_path,
    is_stderr_tty, print_progress_handler, sanitize_filename, should_ignore_path,
};

// Simple API wrapper functions
use anyhow::Result;
use engine::ToPathIter;

/// Extract schema (templates and metadata) from one or more files
///
/// This is a convenience function that handles all the internal complexity:
/// - Loads configuration (or uses defaults)
/// - Performs Phase 1 scan for all files
/// - Calculates adaptive chunking
/// - Performs Phase 2 mining and metadata extraction
/// - Returns a vector of Outputs containing templates and metadata
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
/// let outputs = extract_schema("document.log", OutputMode::Full)?;
/// println!("Templates: {}", outputs[0].templates.len());
/// # Ok::<(), anyhow::Error>(())
/// ```
///
/// # Example - Multiple files
///
/// ```no_run
/// use zahirscan::{extract_schema, OutputMode};
///
/// let files = vec!["file1.log", "file2.log", "file3.log"];
/// let outputs = extract_schema(files.as_slice(), OutputMode::Full)?;
/// for output in outputs {
///     println!("Templates: {}", output.templates.len());
/// }
/// # Ok::<(), anyhow::Error>(())
/// ```
#[allow(private_bounds)]
pub fn extract_schema<P: ToPathIter>(paths: P, mode: OutputMode) -> Result<Vec<Output>> {
    let config = Config::load().unwrap_or_default();
    let path_strings = paths.to_path_iter();

    // Validate input - fail fast with clear error
    if path_strings.is_empty() {
        return Err(anyhow::anyhow!("No file paths provided"));
    }

    // Phase 1: Initial scan
    let tasks = phase1_scan(&path_strings, None, false, &config);
    if tasks.is_empty() {
        return Err(anyhow::anyhow!(
            "No valid files found. All provided paths failed to scan or do not exist"
        ));
    }

    // Calculate adaptive chunking
    let adaptive = calculate_adaptive_chunking(&tasks, config.max_workers, &config);

    // Phase 2: Template mining and metadata extraction
    // Set output mode in config
    let mut config_with_mode = config.clone();
    config_with_mode.output_mode = mode;

    // Process (skip file write for library usage)
    phase2_mining(tasks, &config_with_mode, &adaptive, true)
}
