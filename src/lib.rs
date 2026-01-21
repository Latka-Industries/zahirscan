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
//! # Example
//!
//! ```no_run
//! use zahirscan::{Config, phase1_scan, phase2_mining, calculate_adaptive_chunking};
//!
//! let config = Config::load().unwrap_or_default();
//! let paths = vec!["file.log".to_string()];
//! let tasks = phase1_scan(&paths, None, false, &config);
//! let adaptive = calculate_adaptive_chunking(&tasks, config.max_workers, &config);
//! let outputs = phase2_mining(tasks, &config, &adaptive, config.max_workers, false)?;
//! # Ok::<(), anyhow::Error>(())
//! ```

pub mod chunking;
pub mod config;
pub mod orchestrator;
pub mod parsers;
pub mod results;
pub mod tools;

// Re-export chunking utilities
pub use chunking::{ProcessingTask, calculate_adaptive_chunking};

// Re-export configuration
pub use config::Config;

// Re-export orchestrator functions (main entry points)
pub use orchestrator::{phase1_scan, phase2_mining};

// Re-export parser types and functions
pub use parsers::{FileType, ParseResult, extract_templates, initial_file_scan};

// Re-export result types
pub use results::{
    AudioMetadata, CompressionStats, FileMetadata, ImageMetadata, MiningResult, Output, OutputMode,
    Template, VideoMetadata,
};

// Re-export utility functions
pub use tools::{
    detect_file_type, determine_output_path, format_bytes, format_duration, get_temp_output_path,
    sanitize_filename,
};
