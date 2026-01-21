pub mod config;
pub mod orchestrator;
pub mod parsers;
pub mod results;
pub mod tools;

/// Re-export commonly used types
pub use config::Config;
pub use orchestrator::{calculate_adaptive_chunking, phase1_scan, phase2_mining};
pub use parsers::{FileType, ParseResult, extract_templates, initial_file_scan};
pub use results::{
    AudioMetadata, CompressionStats, FileMetadata, ImageMetadata, MiningResult, Output, OutputMode,
    Template, VideoMetadata,
};
pub use tools::{
    detect_file_type, determine_output_path, format_bytes, format_duration, get_temp_output_path,
    sanitize_filename,
};
