//! JSON file metadata (line-based stats, root type, depth, formatting).

use serde::{Deserialize, Serialize};

/// JSON file metadata: byte count, line count, line ending, max line length, blank lines,
/// root type, root size, max depth, and whether the content is pretty-printed.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct JsonMetadata {
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
    /// Root value type: "array" or "object"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_type: Option<String>,
    /// If root is array, number of elements
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_array_length: Option<usize>,
    /// If root is object, number of keys
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_object_key_count: Option<usize>,
    /// Maximum nesting depth of the JSON structure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_depth: Option<usize>,
    /// True if content appears pretty-printed (contains newlines after structural chars)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pretty_printed: Option<bool>,
}
