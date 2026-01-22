//! CSV file metadata extraction

mod utils;

use crate::config::Config;
use crate::parsers::ParseResult;
use crate::parsers::traits::AdaptiveParallel;
use crate::results::{BooleanStats, CsvMetadata, DateStats, MiningResult, NumericStats};
use ::csv::ReaderBuilder;
use anyhow::Result;
use chrono::DateTime;
use dashmap::{DashMap, DashSet};
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

    // Use DashMap for thread-safe parallel null counting
    let null_counts: Vec<DashMap<(), usize>> = (0..column_count).map(|_| DashMap::new()).collect();
    // Use DashSet for thread-safe parallel unique value tracking
    let unique_sets: Vec<DashSet<String>> = (0..column_count).map(|_| DashSet::new()).collect();

    // Count nulls and collect unique values in parallel
    sample_data.par_iter_adaptive(config).for_each(|row| {
        for (col_idx, value) in row.iter().enumerate() {
            if col_idx >= column_count {
                break;
            }

            // Check if null/empty
            if value.is_empty()
                || value.eq_ignore_ascii_case("null")
                || value.eq_ignore_ascii_case("nil")
            {
                *null_counts[col_idx].entry(()).or_insert(0) += 1;
            }

            // Track unique values (DashSet handles deduplication)
            unique_sets[col_idx].insert(value.to_string());
        }
    });

    // Calculate percentages and unique counts
    let null_percentages: Vec<f64> = null_counts
        .into_iter()
        .map(|counts| {
            let count = counts.into_iter().next().map(|(_, c)| c).unwrap_or(0);
            (count as f64 / total_rows as f64) * 100.0
        })
        .collect();

    let unique_counts: Vec<usize> = unique_sets.into_iter().map(|set| set.len()).collect();

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
    // Collect and parse numeric values in parallel, filtering out NaN/invalid
    let values: Vec<f64> = sample_data
        .par_iter_adaptive(config)
        .filter_map(|row| {
            if col_idx >= row.len() {
                return None;
            }
            let val = &row[col_idx];
            // Skip null/empty values
            if val.is_empty() || val.eq_ignore_ascii_case("null") || val.eq_ignore_ascii_case("nil")
            {
                return None;
            }
            // Parse as float and filter finite values
            val.parse::<f64>().ok().filter(|&v| v.is_finite())
        })
        .collect();
    if values.is_empty() {
        return None;
    }

    // Sort for min/max/median/IQR calculations
    let mut sorted_values = values.clone();
    sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let min = sorted_values.first().copied();
    let max = sorted_values.last().copied();
    let range = min.zip(max).map(|(min_val, max_val)| max_val - min_val);

    // Calculate mean
    let mean = Some(values.iter().sum::<f64>() / values.len() as f64);

    // Calculate median
    let median = {
        let mid = sorted_values.len() / 2;
        if sorted_values.len().is_multiple_of(2) {
            // Even number of values: average of two middle values
            Some((sorted_values[mid - 1] + sorted_values[mid]) / 2.0)
        } else {
            // Odd number of values: middle value
            Some(sorted_values[mid])
        }
    };

    // Calculate IQR (interquartile range)
    let iqr = if sorted_values.len() >= 4 {
        let q1_idx = sorted_values.len() / 4;
        let q3_idx = (3 * sorted_values.len()) / 4;
        Some(sorted_values[q3_idx] - sorted_values[q1_idx])
    } else {
        None
    };

    // Calculate standard deviation
    let stdev = if let Some(mean_val) = mean
        && values.len() > 1
    {
        let variance =
            values.iter().map(|&v| (v - mean_val).powi(2)).sum::<f64>() / (values.len() - 1) as f64;
        Some(variance.sqrt())
    } else {
        None
    };

    Some(NumericStats {
        min,
        max,
        mean,
        median,
        range,
        iqr,
        stdev,
    })
}

/// Compute date statistics (span in days/minutes, min/max) for a column
fn compute_date_stats(
    sample_data: &[Vec<String>],
    col_idx: usize,
    _config: &Config,
) -> Option<DateStats> {
    // Collect and parse date/timestamp values
    let mut timestamps: Vec<i64> = sample_data
        .iter()
        .filter_map(|row| {
            if col_idx >= row.len() {
                return None;
            }
            let val = &row[col_idx];

            // First try parsing as Unix timestamp (if it's a numeric timestamp)
            // Otherwise try parsing as date string
            crate::tools::parse_timestamp_to_seconds(val)
                .or_else(|| crate::tools::parse_date_to_timestamp(val))
        })
        .collect();

    if timestamps.is_empty() {
        return None;
    }

    timestamps.sort();
    let min_ts = timestamps.first().copied()?;
    let max_ts = timestamps.last().copied()?;

    // Calculate span in seconds, then convert to days and minutes
    let span_seconds = (max_ts - min_ts) as f64;
    let span_days = span_seconds / 86400.0;
    let span_minutes = span_seconds / 60.0;

    // Convert back to strings for min/max
    let min_str = DateTime::from_timestamp(min_ts, 0).map(|dt| dt.to_rfc3339());
    let max_str = DateTime::from_timestamp(max_ts, 0).map(|dt| dt.to_rfc3339());

    Some(DateStats {
        span_days: Some(span_days),
        span_minutes: Some(span_minutes),
        min: min_str,
        max: max_str,
    })
}

/// Compute boolean statistics (percentage of true values) for a column
fn compute_boolean_stats(
    sample_data: &[Vec<String>],
    col_idx: usize,
    config: &Config,
) -> Option<BooleanStats> {
    use dashmap::DashMap;

    // Use DashMap for thread-safe parallel counting
    let total_count: DashMap<(), usize> = DashMap::new();
    let true_count: DashMap<(), usize> = DashMap::new();

    // Count totals and true values in parallel
    sample_data.par_iter_adaptive(config).for_each(|row| {
        if col_idx >= row.len() {
            return;
        }
        let val = &row[col_idx];
        // Skip null/empty values
        if val.is_empty() || val.eq_ignore_ascii_case("null") || val.eq_ignore_ascii_case("nil") {
            return;
        }
        *total_count.entry(()).or_insert(0) += 1;
        // Check if value represents true
        if matches!(val.to_lowercase().as_str(), "true" | "yes" | "1" | "y") {
            *true_count.entry(()).or_insert(0) += 1;
        }
    });

    let total = total_count.into_iter().next().map(|(_, c)| c).unwrap_or(0);
    if total == 0 {
        return None;
    }

    let true_val = true_count.into_iter().next().map(|(_, c)| c).unwrap_or(0);
    let true_percentage = (true_val as f64 / total as f64) * 100.0;

    Some(BooleanStats {
        true_percentage: Some(true_percentage),
    })
}

/// Extract templates from CSV files (CSV files don't have templates, return empty result)
pub fn extract_csv_templates(
    _content: &[u8],
    stats: &ParseResult,
    _config: &Config,
) -> Result<MiningResult> {
    // CSV files don't have templates, return empty result
    Ok(crate::parsers::traits::empty_mining_result(stats))
}
