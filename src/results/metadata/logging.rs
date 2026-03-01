//! Log file metadata (line-based stats and timestamp detection).

use serde::{Deserialize, Serialize};

/// Log file metadata: byte count, line count, line ending, max line length, blank lines, timestamp detection.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct LogMetadata {
    /// File size in bytes
    pub byte_count: usize,
    /// Number of lines
    pub line_count: usize,
    /// Dominant line ending: "lf", "crlf", or "cr"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_ending: Option<String>,
    /// Longest line length in bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_line_length: Option<usize>,
    /// Number of blank (empty or whitespace-only) lines
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blank_line_count: Option<usize>,
    /// True if any line looks like it contains a parseable timestamp (ISO8601 or Unix epoch)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_timestamps: Option<bool>,
}
