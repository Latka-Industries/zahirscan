//! Utility functions for CSV parsing and type inference

/// Common CSV characters and patterns
struct CsvSyntax {
    delimiters: [char; 5],
    quote_chars: [char; 2],
    backslash_escapes: [&'static str; 3],
}

impl CsvSyntax {
    const fn new() -> Self {
        Self {
            delimiters: [',', ';', '\t', '|', ':'],
            quote_chars: ['"', '\''],
            backslash_escapes: ["\\\"", "\\'", "\\\\"],
        }
    }
}

/// Sample the first lines and pick the most frequent delimiter among common separators.
#[must_use]
pub fn dominant_delimiter_char(content: &str) -> Option<char> {
    let syntax = CsvSyntax::new();
    let delimiters = syntax.delimiters;

    let sample_lines: Vec<&str> = content.lines().take(5).collect();
    if sample_lines.is_empty() {
        return None;
    }

    let mut delimiter_counts: Vec<(char, usize)> = delimiters
        .iter()
        .map(|&delim| {
            let count: usize = sample_lines
                .iter()
                .map(|line| line.matches(delim).count())
                .sum();
            (delim, count)
        })
        .collect();

    delimiter_counts.sort_by_key(|item| std::cmp::Reverse(item.1));

    match delimiter_counts.first() {
        Some((delim, count)) if *count > 0 => Some(*delim),
        _ => None,
    }
}

/// Byte to pass to [`csv::ReaderBuilder::delimiter`]. Defaults to comma if none dominates.
#[must_use]
pub fn detect_delimiter_byte(content: &str) -> u8 {
    dominant_delimiter_char(content).map_or(b',', |c| u8::try_from(u32::from(c)).unwrap_or(b','))
}

/// Format a single-byte delimiter for [`crate::results::CsvMetadata::delimiter`].
#[must_use]
pub fn format_delimiter_for_metadata(byte: u8) -> String {
    match char::from(byte) {
        '\t' => "\\t".to_string(),
        c => c.to_string(),
    }
}

/// Delimiter used for parsing: content-based detection, with optional path hints for
/// `.tsv`/`.tab` (tab) and `.psv` (pipe). Other extensions use [`detect_delimiter_byte`].
#[must_use]
pub fn delimiter_byte_for_reader(content: &str, file_path: &str) -> u8 {
    use std::path::Path;
    let ext = Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase);
    match ext.as_deref() {
        Some("tsv" | "tab") => b'\t',
        Some("psv") => b'|',
        _ => detect_delimiter_byte(content),
    }
}

/// Detect CSV quote character by analyzing field patterns.
/// `field_separator` is the delimiter between fields (comma, tab, pipe, etc.).
pub fn detect_quote_character(content: &str, field_separator: char) -> Option<String> {
    // Common quote characters: double quote (") and single quote (')
    let syntax = CsvSyntax::new();
    let quote_chars = syntax.quote_chars;

    // Sample first few lines
    let sample_lines: Vec<&str> = content.lines().take(10).collect();
    if sample_lines.is_empty() {
        return None;
    }

    // Count patterns that suggest quote usage:
    // - Paired quotes (start and end) - most reliable indicator
    // - Quote followed by delimiter or delimiter followed by quote
    let mut quote_scores: Vec<(char, usize)> = quote_chars
        .iter()
        .map(|&quote| {
            let mut score = 0;
            for line in &sample_lines {
                // Count paired quotes (more reliable indicator)
                let quote_count = line.matches(quote).count();
                if quote_count >= 2 && quote_count % 2 == 0 {
                    // Even number suggests paired quotes
                    score += quote_count / 2;
                }

                // Check for pattern: quote, content, quote (basic field pattern)
                if line.contains(&format!("{quote}{field_separator}"))
                    || line.contains(&format!("{field_separator}{quote}"))
                {
                    score += 1;
                }
            }
            (quote, score)
        })
        .collect();

    // Sort by score (descending) and pick the most likely
    quote_scores.sort_by_key(|item| std::cmp::Reverse(item.1));

    match quote_scores.first() {
        Some((quote, score)) if *score > 0 => {
            // Format quote for display (escape special characters)
            let display = match quote {
                '"' => "\"".to_string(),
                '\'' => "'".to_string(),
                _ => quote.to_string(),
            };
            Some(display)
        }
        _ => None,
    }
}

/// Detect CSV escape character by analyzing escape patterns
pub fn detect_escape_character(
    content: &str,
    _delimiter: Option<&str>,
    quote: Option<&str>,
) -> Option<String> {
    // Sample first few lines
    let sample_lines: Vec<&str> = content.lines().take(10).collect();
    if sample_lines.is_empty() {
        return None;
    }

    // Common escape characters: backslash (\) or same as quote (for doubled quotes)
    // Check for backslash escapes first (most common)
    let syntax = CsvSyntax::new();
    let mut backslash_escapes = 0;
    let mut doubled_quote_escapes = 0;

    for line in &sample_lines {
        // Count backslash escape patterns: \", \', \\, etc.
        if syntax
            .backslash_escapes
            .iter()
            .any(|pattern| line.contains(pattern))
        {
            backslash_escapes += 1;
        }

        // Check for doubled quote escapes (e.g., "" inside quoted field)
        if let Some(quote_char) = quote {
            let doubled_pattern = format!("{quote_char}{quote_char}");
            if line.contains(&doubled_pattern) {
                doubled_quote_escapes += 1;
            }
        }
    }

    // Prefer backslash if found, otherwise check for doubled quotes
    if backslash_escapes > 0 {
        Some("\\".to_string())
    } else if doubled_quote_escapes > 0 {
        // Doubled quote means escape is same as quote
        quote.map(std::string::ToString::to_string)
    } else {
        None
    }
}
