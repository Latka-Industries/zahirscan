//! Utility functions for CSV parsing and type inference

/// Detect CSV delimiter by trying common delimiters
pub(crate) fn detect_delimiter(content: &str) -> Option<String> {
    // Try common delimiters in order of frequency
    let delimiters = [',', ';', '\t', '|', ':'];

    // Sample first few lines to detect delimiter
    let sample_lines: Vec<&str> = content.lines().take(5).collect();
    if sample_lines.is_empty() {
        return None;
    }

    // Count occurrences of each delimiter in the sample
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

    // Sort by count (descending) and pick the most common
    delimiter_counts.sort_by(|a, b| b.1.cmp(&a.1));

    // Return the most common delimiter if it appears consistently
    match delimiter_counts.first() {
        Some((delim, count)) if *count > 0 => {
            // Format delimiter for display (escape special characters)
            let display = match delim {
                '\t' => "\\t".to_string(),
                _ => delim.to_string(),
            };
            Some(display)
        }
        _ => None,
    }
}

/// Detect CSV quote character by analyzing field patterns
pub(crate) fn detect_quote_character(content: &str) -> Option<String> {
    // Common quote characters: double quote (") and single quote (')
    let quote_chars = ['"', '\''];

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
                if line.contains(&format!("{quote},")) || line.contains(&format!(",{quote}")) {
                    score += 1;
                }
            }
            (quote, score)
        })
        .collect();

    // Sort by score (descending) and pick the most likely
    quote_scores.sort_by(|a, b| b.1.cmp(&a.1));

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
pub(crate) fn detect_escape_character(
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
    let mut backslash_escapes = 0;
    let mut doubled_quote_escapes = 0;

    for line in &sample_lines {
        // Count backslash escape patterns: \", \', \\, etc.
        if line.contains("\\\"") || line.contains("\\'") || line.contains("\\\\") {
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
        quote.map(|q| q.to_string())
    } else {
        None
    }
}

/// Infer the data type of a single value
pub(crate) fn infer_value_type(value: &str) -> String {
    match () {
        _ if value.is_empty()
            || value.eq_ignore_ascii_case("null")
            || value.eq_ignore_ascii_case("nil") =>
        {
            "null".to_string()
        }
        _ if crate::tools::is_boolean(value) => "boolean".to_string(),
        _ if crate::tools::parse_timestamp_to_seconds(value).is_some() => "timestamp".to_string(),
        _ if crate::tools::is_number(value) => "number".to_string(),
        _ if crate::tools::is_date(value) => "date".to_string(),
        _ => "string".to_string(),
    }
}
