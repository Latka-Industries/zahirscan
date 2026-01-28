//! Code/script file metadata (script type, size, line count; zero-copy extras: BOM, line endings, etc.)

use serde::{Deserialize, Serialize};

use crate::results::MinimalFallback;

/// Code/script file metadata: language, size, line count, and zero-copy extras (single pass over bytes).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CodeMetadata {
    /// Detected script/language (e.g. "python", "rust", "shell") from extension + optional shebang
    pub script_type: String,
    /// File size in bytes
    pub byte_count: usize,
    /// Number of lines
    pub line_count: usize,
    /// BOM if present (e.g. "UTF-8", "UTF-16LE", "UTF-16BE")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bom: Option<String>,
    /// Dominant line ending: "lf", "crlf", "cr", or "mixed"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_ending: Option<String>,
    /// True if file ends with newline
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trailing_newline: Option<bool>,
    /// Longest line length in bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_line_length: Option<usize>,
    /// Number of blank (empty or whitespace-only) lines
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blank_line_count: Option<usize>,
    /// Indentation of first non-blank lines: "spaces", "tabs", or "mixed"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indentation: Option<String>,
}

impl Default for CodeMetadata {
    fn default() -> Self {
        Self {
            script_type: "unknown".to_string(),
            byte_count: 0,
            line_count: 0,
            bom: None,
            line_ending: None,
            trailing_newline: None,
            max_line_length: None,
            blank_line_count: None,
            indentation: None,
        }
    }
}

impl MinimalFallback for CodeMetadata {
    fn minimal_fallback(file_size_bytes: usize) -> Self {
        Self {
            script_type: "unknown".to_string(),
            byte_count: file_size_bytes,
            line_count: 0,
            bom: None,
            line_ending: None,
            trailing_newline: None,
            max_line_length: None,
            blank_line_count: None,
            indentation: None,
        }
    }
}
