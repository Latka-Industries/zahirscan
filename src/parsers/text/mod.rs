//! Text-based file parsers (JSON, logs, markdown, plain text)

pub mod json;
pub mod log;
pub mod markdown;
pub mod plain_text;
pub mod writing_analysis;

// Re-export plain_text as text for backward compatibility
pub use plain_text as text;
