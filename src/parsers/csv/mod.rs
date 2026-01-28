//! CSV file metadata extraction

mod utils;

use crate::engine::config::Config;
use crate::parsers::ParseResult;
use crate::parsers::column_stats;
use crate::parsers::traits::AdaptiveParallel;
use crate::results::{BooleanStats, CsvMetadata, DateStats, NumericStats};
use ::csv::ReaderBuilder;
use anyhow::Result;
use dashmap::DashMap;
use rayon::prelude::*;
use std::io::Cursor;

use utils::{detect_delimiter, detect_escape_character, detect_quote_character};

/// Extract CSV metadata
pub fn extract_csv_metadata(
    content: &[u8],
    _stats: &ParseResult,
    _config: &Config,
) -> Result<CsvMetadata> {
    // Check if content is valid UTF-8
    let encoding = if std::str::from_utf8(content).is_ok() {
        Some("UTF-8".to_string())
    } else {
        // Try to detect other common encodings (simplified - just mark as non-UTF-8)
        Some("Non-UTF-8".to_string())
    };

    // Try to read as UTF-8 first
    let content_str = match std::str::from_utf8(content) {
        Ok(s) => s,
        Err(_) => {
            // If not UTF-8, return minimal metadata with encoding info
            return Ok(CsvMetadata {
                row_count: 0,
                column_count: 0,
                column_names: None,
                encoding,
                column_types: None,
                delimiter: None,
                quote_character: None,
                escape_character: None,
                has_header: None,
                null_percentages: None,
                unique_counts: None,
                numeric_stats: None,
                date_stats: None,
                boolean_stats: None,
            });
        }
    };

    // Build CSV reader with flexible delimiter detection
    let mut reader = ReaderBuilder::new()
        .has_headers(true) // Try to read headers first
        .flexible(true) // Allow varying number of fields per row
        .from_reader(Cursor::new(content_str));

    // Try to read headers and detect delimiter
    let headers_result = reader.headers();
    let (column_names, column_count_from_headers, has_header) = match headers_result {
        Ok(headers) => {
            let names: Vec<String> = headers.iter().map(|s: &str| s.to_string()).collect();
            let count = names.len();
            (Some(names), Some(count), Some(true))
        }
        Err(_) => (None, None, Some(false)),
    };

    // Detect delimiter, quote, and escape characters
    let delimiter = detect_delimiter(content_str);
    let quote_character = detect_quote_character(content_str);
    let escape_character = detect_escape_character(
        content_str,
        delimiter.as_deref(),
        quote_character.as_deref(),
    );

    // Sample rows for data type inference
    let max_sample_rows = _config.max_csv_sample_rows;
    let mut row_count = 0;
    let mut column_count: usize = column_count_from_headers.unwrap_or(0);
    let mut sample_data: Vec<Vec<String>> = Vec::new();

    for result in reader.records() {
        match result {
            Ok(record) => {
                row_count += 1;
                // If we didn't get column count from headers, use first row
                if column_count == 0 {
                    column_count = record.len();
                }
                // Collect samples for type inference
                if sample_data.len() < max_sample_rows {
                    let row: Vec<String> = record.iter().map(|s: &str| s.to_string()).collect();
                    sample_data.push(row);
                }
            }
            Err(_) => {
                // Skip malformed rows
                continue;
            }
        }
    }

    // Infer column types and compute statistics using probabilistic analysis
    let (column_types, null_percentages, unique_counts, numeric_stats, date_stats, boolean_stats) =
        if !sample_data.is_empty() && column_count > 0 {
            let types = infer_column_types(&sample_data, column_count, _config);
            let (null_pcts, unique_cts) =
                compute_column_statistics(&sample_data, column_count, _config);
            let (num_stats, dt_stats, bool_stats) =
                compute_type_specific_statistics(&sample_data, &types, column_count, _config);
            (
                Some(types),
                Some(null_pcts),
                Some(unique_cts),
                Some(num_stats),
                Some(dt_stats),
                Some(bool_stats),
            )
        } else {
            (None, None, None, None, None, None)
        };

    Ok(CsvMetadata {
        row_count,
        column_count,
        column_names,
        encoding,
        column_types,
        delimiter,
        quote_character,
        escape_character,
        has_header,
        null_percentages,
        unique_counts,
        numeric_stats,
        date_stats,
        boolean_stats,
    })
}

/// Infer data types for each column using probabilistic analysis
fn infer_column_types(
    sample_data: &[Vec<String>],
    column_count: usize,
    config: &Config,
) -> Vec<String> {
    // Use DashMap for thread-safe parallel updates
    let type_scores: Vec<DashMap<String, usize>> =
        (0..column_count).map(|_| DashMap::new()).collect();

    // Analyze each row in parallel with adaptive chunking
    sample_data.par_iter_adaptive(config).for_each(|row| {
        for (col_idx, value) in row.iter().enumerate() {
            if col_idx >= column_count {
                break;
            }
            let inferred_type = utils::infer_value_type(value);
            *type_scores[col_idx].entry(inferred_type).or_insert(0) += 1;
        }
    });

    // Determine the most likely type for each column
    type_scores
        .into_iter()
        .map(|scores| {
            // Find the type with the highest count
            scores
                .into_iter()
                .max_by_key(|(_, count)| *count)
                .map(|(type_name, _)| type_name)
                .unwrap_or_else(|| "string".to_string())
        })
        .collect()
}

/// Compute null percentages and unique value counts per column
fn compute_column_statistics(
    sample_data: &[Vec<String>],
    column_count: usize,
    config: &Config,
) -> (Vec<f64>, Vec<usize>) {
    let total_rows = sample_data.len();
    if total_rows == 0 {
        return (vec![0.0; column_count], vec![0; column_count]);
    }

    // Extract each column's values and compute statistics
    let mut null_percentages = Vec::with_capacity(column_count);
    let mut unique_counts = Vec::with_capacity(column_count);

    for col_idx in 0..column_count {
        let values = column_stats::extract_column_values(sample_data, col_idx);
        let (null_pct, unique_ct) = column_stats::compute_null_and_unique_stats(&values, config);
        null_percentages.push(null_pct);
        unique_counts.push(unique_ct);
    }

    (null_percentages, unique_counts)
}

type TypeSpecificStats = (
    Vec<Option<NumericStats>>,
    Vec<Option<DateStats>>,
    Vec<Option<BooleanStats>>,
);

/// Compute type-specific statistics (numeric, date, boolean) per column
#[allow(clippy::type_complexity)]
fn compute_type_specific_statistics(
    sample_data: &[Vec<String>],
    column_types: &[String],
    column_count: usize,
    config: &Config,
) -> TypeSpecificStats {
    let mut numeric_stats: Vec<Option<NumericStats>> = vec![None; column_count];
    let mut date_stats: Vec<Option<DateStats>> = vec![None; column_count];
    let mut boolean_stats: Vec<Option<BooleanStats>> = vec![None; column_count];

    // Process each column based on its inferred type
    for col_idx in 0..column_count {
        if col_idx >= column_types.len() {
            break;
        }

        let col_type = &column_types[col_idx];
        match col_type.as_str() {
            "number" => {
                numeric_stats[col_idx] = compute_numeric_stats(sample_data, col_idx, config);
            }
            "timestamp" | "date" => {
                // Timestamps can be treated as dates for statistics
                date_stats[col_idx] = compute_date_stats(sample_data, col_idx, config);
            }
            "boolean" => {
                boolean_stats[col_idx] = compute_boolean_stats(sample_data, col_idx, config);
            }
            _ => {
                // No statistics for string/null columns
            }
        }
    }

    (numeric_stats, date_stats, boolean_stats)
}

/// Compute numeric statistics (min, max, mean, median, range, IQR, stdev) for a column
fn compute_numeric_stats(
    sample_data: &[Vec<String>],
    col_idx: usize,
    config: &Config,
) -> Option<NumericStats> {
    let values = column_stats::extract_column_values(sample_data, col_idx);
    column_stats::compute_numeric_stats_from_strings(&values, config)
}

/// Compute date statistics (span in days/minutes, min/max) for a column
fn compute_date_stats(
    sample_data: &[Vec<String>],
    col_idx: usize,
    _config: &Config,
) -> Option<DateStats> {
    let values = column_stats::extract_column_values(sample_data, col_idx);
    column_stats::compute_date_stats_from_strings(&values)
}

/// Compute boolean statistics (percentage of true values) for a column
fn compute_boolean_stats(
    sample_data: &[Vec<String>],
    col_idx: usize,
    config: &Config,
) -> Option<BooleanStats> {
    let values = column_stats::extract_column_values(sample_data, col_idx);
    column_stats::compute_boolean_stats_from_strings(&values, config)
}

crate::no_template_mining!(
    extract_csv_templates,
    "CSV files don't have templates, return empty result."
);
