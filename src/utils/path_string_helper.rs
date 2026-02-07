use std::path::{Path, PathBuf};

use log::debug;

use crate::config::RuntimeConfig;

/// Format duration into human-readable format
pub fn format_duration(duration: std::time::Duration) -> String {
    let secs = duration.as_secs_f64();

    if secs < 0.001 {
        format!("{:.2} μs", secs * 1_000_000.0)
    } else if secs < 1.0 {
        format!("{:.2} ms", secs * 1000.0)
    } else if secs < 60.0 {
        format!("{:.2} s", secs)
    } else {
        let mins = secs / 60.0;
        format!("{:.2} m", mins)
    }
}

/// Format bytes into human-readable format (B, KB, MB, GB, TB, PB)
pub fn format_bytes(bytes: usize) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB", "PB"];
    const THRESHOLD: f64 = 1024.0;

    if bytes == 0 {
        return "0 B".to_string();
    }

    let bytes_f64 = bytes as f64;
    let exp = (bytes_f64.ln() / THRESHOLD.ln()).floor() as usize;
    let exp = exp.min(UNITS.len() - 1);

    let value = bytes_f64 / THRESHOLD.powi(exp as i32);

    if exp == 0 {
        format!("{} {}", bytes, UNITS[exp])
    } else {
        format!("{:.2} {}", value, UNITS[exp])
    }
}

/// Sanitize filename by removing whitespace, apostrophes, commas & replacing brackets/parentheses with underscores
pub fn sanitize_filename(name: &str) -> String {
    name.chars()
        .filter_map(|c| match c {
            ' ' | '\t' | '\n' | '\r' => None,               // Remove whitespace
            '\'' | ',' => None,                             // Remove apostrophes and commas
            '{' | '}' | '[' | ']' | '(' | ')' => Some('_'), // Replace brackets/parentheses with underscore
            _ => Some(c),                                   // Keep all other characters
        })
        .collect()
}

/// Redact a file path to show only the filename (for privacy)
/// Returns `***/filename.ext` format
pub fn redact_path(path: &str) -> String {
    let filename = Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    format!("***/{}", filename)
}

/// Whether to skip a path based on `[filter]` ignore_patterns and ignore_hidden_files.
/// Patterns: exact basename (case-insensitive), `*suffix` (ends with), or `prefix*` (starts with).
/// Note: ignore_hidden_files only skips when basename starts with `.` (dotfiles). .DS_Store / Thumbs.db / temp patterns come from the default ignore_patterns list.
/// Used for both top-level file paths and ZIP entry paths.
pub fn should_ignore_path(path: &str, config: &RuntimeConfig) -> bool {
    let basename = Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if basename.is_empty() {
        return false;
    }
    if config.ignore_hidden_files && basename.starts_with('.') {
        return true;
    }
    for pat in &config.ignore_patterns {
        if pat.is_empty() {
            continue;
        }
        if pat.starts_with('*') {
            if basename.ends_with(pat.get(1..).unwrap_or("")) {
                return true;
            }
        } else if pat.ends_with('*') && pat.len() > 1 {
            if basename.starts_with(pat.get(..pat.len() - 1).unwrap_or("")) {
                return true;
            }
        } else if basename.eq_ignore_ascii_case(pat) {
            return true;
        }
    }
    false
}

/// Get temporary output path for a file
pub fn get_temp_output_path(input_path: &str, config: &RuntimeConfig) -> String {
    let path = Path::new(input_path);
    // Get filename with extension (e.g., "file.txt" -> "file.txt.zahirscan.out")
    let input_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(crate::PKG_NAME);
    let sanitized_name = sanitize_filename(input_name);
    let temp_dir = std::env::temp_dir();
    temp_dir
        .join(format!(
            "{}.{}",
            sanitized_name,
            config.temp_file_extension()
        ))
        .to_string_lossy()
        .to_string()
}

/// Determine output path for a given input file.
/// When output_dir is Some we write filename.ext.zahirscan.out there; when None we use a temp file.
pub fn determine_output_path(
    input_path: &str,
    output_dir: Option<&str>,
    config: &RuntimeConfig,
) -> String {
    if let Some(out_dir) = output_dir {
        let path = Path::new(input_path);
        let input_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(crate::PKG_NAME);
        let sanitized_name = sanitize_filename(input_name);
        PathBuf::from(out_dir)
            .join(format!(
                "{}.{}",
                sanitized_name,
                config.temp_file_extension()
            ))
            .to_string_lossy()
            .to_string()
    } else {
        get_temp_output_path(input_path, config)
    }
}

/// Placeholder type for template patterns
/// Each parser uses a specific placeholder type for consistency
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceholderType {
    /// Word placeholders for text/markdown parsers (e.g., "WORD_00")
    Word,
    /// Prefix slot in structural text patterns (e.g., "PREFIX_00")
    Prefix,
    /// Suffix slot in structural text patterns (e.g., "SUFFIX_00")
    Suffix,
    /// Position placeholders for log parsers (e.g., "POS_00")
    Position,
    /// Position placeholders for JSON parsers (lowercase, e.g., "pos_00")
    Pos,
    /// Column placeholders for JSON parsers (e.g., "col_00")
    Col,
    /// Header placeholders for markdown parsers (e.g., "HEADER_00")
    Header,
    /// List placeholders for markdown parsers (e.g., "LIST_00")
    List,
    /// Code block placeholders for markdown parsers (e.g., "CODE_BLOCK_00")
    CodeBlock,
    /// Paragraph placeholders for markdown parsers (e.g., "PARAGRAPH_00")
    Paragraph,
}

impl PlaceholderType {
    /// Get the string representation of the placeholder type
    pub fn as_str(self) -> &'static str {
        match self {
            PlaceholderType::Word => "WORD",
            PlaceholderType::Prefix => "PREFIX",
            PlaceholderType::Suffix => "SUFFIX",
            PlaceholderType::Position => "POS",
            PlaceholderType::Pos => "pos",
            PlaceholderType::Col => "col",
            PlaceholderType::Header => "HEADER",
            PlaceholderType::List => "LIST",
            PlaceholderType::CodeBlock => "CODE_BLOCK",
            PlaceholderType::Paragraph => "PARAGRAPH",
        }
    }
}

/// Format a placeholder name with zero-padded index (e.g., "WORD_00", "POS_01")
/// Ensures proper lexicographic sorting: WORD_00, WORD_01, ..., WORD_09, WORD_10
pub fn format_placeholder(name: &str, index: usize) -> String {
    format!("{}_{:02}", name, index)
}

/// Format a placeholder using PlaceholderType enum
pub fn format_placeholder_typed(placeholder_type: PlaceholderType, index: usize) -> String {
    format_placeholder(placeholder_type.as_str(), index)
}

/// Format a bracketed placeholder for pattern strings (e.g., "[WORD_00]", "[POS_01]")
/// Ensures proper lexicographic sorting in pattern strings
pub fn format_placeholder_bracketed(name: &str, index: usize) -> String {
    format!("[{}]", format_placeholder(name, index))
}

/// Format a bracketed placeholder using PlaceholderType enum.
/// Prefix and Suffix are formatted without an index (e.g. "[PREFIX]", "[SUFFIX]").
pub fn format_placeholder_bracketed_typed(
    placeholder_type: PlaceholderType,
    index: usize,
) -> String {
    match placeholder_type {
        PlaceholderType::Prefix => "[PREFIX]".to_string(),
        PlaceholderType::Suffix => "[SUFFIX]".to_string(),
        _ => format_placeholder_bracketed(placeholder_type.as_str(), index),
    }
}

/// Print a message if progress bars are enabled, and always log it with debug!
///
/// This is useful for progress-related messages that should be displayed to users
/// when progress bars are enabled, while still being logged for debugging.
pub fn print_progress_handler(message: &str, show_progress: bool) {
    debug!("{}", message);
    if show_progress {
        println!("{}", message);
    }
}

/// Helper trait to convert both `&str` and collections into an iterator of strings
pub(crate) trait ToPathIter {
    fn to_path_iter(self) -> Vec<String>;
}

// Macro to generate ToPathIter implementations with different conversion strategies
macro_rules! impl_to_path_iter {
    // Direct pass-through (no conversion needed) - for Vec<String>
    (pass: $t:ty) => {
        impl ToPathIter for $t {
            fn to_path_iter(self) -> Vec<String> {
                self
            }
        }
    };

    // Wrap single String in Vec
    (wrap: $t:ty) => {
        impl ToPathIter for $t {
            fn to_path_iter(self) -> Vec<String> {
                vec![self]
            }
        }
    };

    // Clone strategy - for &String
    (clone: $t:ty) => {
        impl ToPathIter for $t {
            fn to_path_iter(self) -> Vec<String> {
                vec![self.clone()]
            }
        }
    };

    // Clone from reference
    (clone_ref: $t:ty) => {
        impl ToPathIter for $t {
            fn to_path_iter(self) -> Vec<String> {
                self.clone()
            }
        }
    };

    // Convert to String
    (to_string: $t:ty) => {
        impl ToPathIter for $t {
            fn to_path_iter(self) -> Vec<String> {
                vec![self.to_string()]
            }
        }
    };

    // Convert slice to Vec
    (to_vec: $t:ty) => {
        impl ToPathIter for $t {
            fn to_path_iter(self) -> Vec<String> {
                self.to_vec()
            }
        }
    };

    // Map iterator to String
    (map_iter: $t:ty) => {
        impl ToPathIter for $t {
            fn to_path_iter(self) -> Vec<String> {
                self.iter().map(|s| s.to_string()).collect()
            }
        }
    };

    // Map into_iter to String
    (map_into: $t:ty) => {
        impl ToPathIter for $t {
            fn to_path_iter(self) -> Vec<String> {
                self.into_iter().map(|s| s.to_string()).collect()
            }
        }
    };
}

// Generate implementations
impl_to_path_iter!(to_string: &str);
impl_to_path_iter!(clone: &String);
impl_to_path_iter!(wrap: String);
impl_to_path_iter!(pass: Vec<String>);
impl_to_path_iter!(clone_ref: &Vec<String>);
impl_to_path_iter!(to_vec: &[String]);
impl_to_path_iter!(map_iter: &[&str]);
impl_to_path_iter!(map_into: Vec<&str>);
impl_to_path_iter!(map_iter: &Vec<&str>);

// Array implementation needs const generic
impl<const N: usize> ToPathIter for [&str; N] {
    fn to_path_iter(self) -> Vec<String> {
        self.into_iter().map(|s| s.to_string()).collect()
    }
}
