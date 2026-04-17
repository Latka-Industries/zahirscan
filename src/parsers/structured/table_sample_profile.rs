use dashmap::DashMap;
use rayon::prelude::*;

use crate::config::RuntimeConfig;
use crate::parsers::column_stats;
use crate::parsers::traits::AdaptiveParallel;
use crate::results::{BooleanStats, DateStats, NumericStats};
use crate::utils::typecheck::{is_boolean, is_date, is_number, parse_timestamp_to_seconds};

struct TabularValueTypes;

impl TabularValueTypes {
    const NULL: &'static str = "null";
    const BOOLEAN: &'static str = "boolean";
    const TIMESTAMP: &'static str = "timestamp";
    const NUMBER: &'static str = "number";
    const DATE: &'static str = "date";
    const STRING: &'static str = "string";
}

#[must_use]
pub fn infer_value_type_match(value: &str) -> String {
    match () {
        () if value.is_empty()
            || value.eq_ignore_ascii_case("null")
            || value.eq_ignore_ascii_case("nil") =>
        {
            TabularValueTypes::NULL.to_string()
        }
        () if is_boolean(value) => TabularValueTypes::BOOLEAN.to_string(),
        () if parse_timestamp_to_seconds(value).is_some() => {
            TabularValueTypes::TIMESTAMP.to_string()
        }
        () if is_number(value) => TabularValueTypes::NUMBER.to_string(),
        () if is_date(value) => TabularValueTypes::DATE.to_string(),
        () => TabularValueTypes::STRING.to_string(),
    }
}

#[inline]
fn is_null_like_cell(value: &str) -> bool {
    value.is_empty() || value.eq_ignore_ascii_case("null") || value.eq_ignore_ascii_case("nil")
}

/// True if every non-null cell matches [`is_boolean`]. Used to reject a plurality "boolean"
/// when the column also contains integers like `-1` or `2` (still numeric, not boolean).
fn column_non_null_values_all_boolean_like(sample_data: &[Vec<String>], col_idx: usize) -> bool {
    for row in sample_data {
        if col_idx >= row.len() {
            continue;
        }
        let v = row[col_idx].as_str();
        if is_null_like_cell(v) {
            continue;
        }
        if !is_boolean(v) {
            return false;
        }
    }
    true
}

fn winning_type_from_scores(
    entries: &[(String, usize)],
    sample_data: &[Vec<String>],
    col_idx: usize,
) -> String {
    let Some((winner, _)) = entries.iter().max_by_key(|(_, c)| c) else {
        return "string".to_string();
    };
    if winner == "boolean" && !column_non_null_values_all_boolean_like(sample_data, col_idx) {
        return entries
            .iter()
            .filter(|(t, _)| t != "boolean")
            .max_by_key(|(_, c)| c)
            .map(|(t, _)| t.clone())
            .unwrap_or_else(|| "string".to_string());
    }
    winner.clone()
}

/// Infer data types for each column using probabilistic analysis
pub(crate) fn infer_column_types(
    sample_data: &[Vec<String>],
    column_count: usize,
    config: &RuntimeConfig,
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
            let inferred_type = infer_value_type_match(value);
            *type_scores[col_idx].entry(inferred_type).or_insert(0) += 1;
        }
    });

    // Determine the most likely type for each column
    type_scores
        .into_iter()
        .enumerate()
        .map(|(col_idx, scores)| {
            let entries: Vec<(String, usize)> = scores.into_iter().collect();
            winning_type_from_scores(&entries, sample_data, col_idx)
        })
        .collect()
}

/// Compute null percentages and unique value counts per column
pub(crate) fn compute_column_statistics(
    sample_data: &[Vec<String>],
    column_count: usize,
    config: &RuntimeConfig,
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
pub(crate) fn compute_type_specific_statistics(
    sample_data: &[Vec<String>],
    column_types: &[String],
    column_count: usize,
    config: &RuntimeConfig,
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
    config: &RuntimeConfig,
) -> Option<NumericStats> {
    let values = column_stats::extract_column_values(sample_data, col_idx);
    column_stats::compute_numeric_stats_from_strings(&values, config)
}

/// Compute date statistics (span in days/minutes, min/max) for a column
fn compute_date_stats(
    sample_data: &[Vec<String>],
    col_idx: usize,
    _config: &RuntimeConfig,
) -> Option<DateStats> {
    let values = column_stats::extract_column_values(sample_data, col_idx);
    column_stats::compute_date_stats_from_strings(&values)
}

/// Compute boolean statistics (percentage of true values) for a column
fn compute_boolean_stats(
    sample_data: &[Vec<String>],
    col_idx: usize,
    config: &RuntimeConfig,
) -> Option<BooleanStats> {
    let values = column_stats::extract_column_values(sample_data, col_idx);
    column_stats::compute_boolean_stats_from_strings(&values, config)
}
