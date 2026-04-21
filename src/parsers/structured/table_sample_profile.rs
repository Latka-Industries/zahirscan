use std::collections::{HashMap, HashSet};

use dashmap::DashMap;
use rayon::prelude::*;

use crate::config::RuntimeConfig;
use crate::parsers::column_stats;
use crate::parsers::traits::AdaptiveParallel;
use crate::results::{BooleanStats, DateStats, NumericStats};
use crate::utils::typecheck::{
    is_boolean, is_date, is_number, parse_date_to_timestamp, parse_timestamp_to_seconds,
};

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
    let any_non_boolean = !column_non_null_values_all_boolean_like(sample_data, col_idx);
    finalize_column_type_from_entries(entries, any_non_boolean)
}

/// Finalize plurality type from score entries; `any_non_non_boolean` means some non-null cell was not boolean-like.
fn finalize_column_type_from_entries(entries: &[(String, usize)], any_non_boolean: bool) -> String {
    let Some((winner, _)) = entries.iter().max_by_key(|(_, c)| c) else {
        return "string".to_string();
    };
    if winner == "boolean" && any_non_boolean {
        return entries
            .iter()
            .filter(|(t, _)| t != "boolean")
            .max_by_key(|(_, c)| c)
            .map_or_else(|| "string".to_string(), |(t, _)| t.clone());
    }
    winner.clone()
}

/// CSV pass 1: streaming type plurality + total row count. `max_inference_rows` (`usize::MAX` = all rows).
pub(crate) fn csv_pass1_infer_types(
    mut column_count: usize,
    max_inference_rows: usize,
    rows: impl Iterator<Item = Vec<String>>,
) -> (Vec<String>, usize) {
    let lim = max_inference_rows;
    let mut type_scores: Vec<HashMap<String, usize>> = if column_count > 0 {
        (0..column_count).map(|_| HashMap::new()).collect()
    } else {
        Vec::new()
    };
    let mut any_non_boolean: Vec<bool> = if column_count > 0 {
        vec![false; column_count]
    } else {
        Vec::new()
    };
    let mut rows_total = 0usize;
    let mut rows_typed = 0usize;

    for row in rows {
        rows_total += 1;
        if column_count == 0 {
            column_count = row.len();
            type_scores = (0..column_count).map(|_| HashMap::new()).collect();
            any_non_boolean = vec![false; column_count];
        }
        if rows_typed < lim {
            rows_typed += 1;
            for (col_idx, value) in row.iter().enumerate() {
                if col_idx >= column_count {
                    break;
                }
                let inferred = infer_value_type_match(value);
                *type_scores[col_idx].entry(inferred).or_insert(0) += 1;
                if !is_null_like_cell(value) && !is_boolean(value) {
                    any_non_boolean[col_idx] = true;
                }
            }
        }
    }

    if column_count == 0 {
        return (Vec::new(), rows_total);
    }

    let types: Vec<String> = (0..column_count)
        .map(|col_idx| {
            let entries: Vec<(String, usize)> = type_scores[col_idx]
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect();
            finalize_column_type_from_entries(&entries, any_non_boolean[col_idx])
        })
        .collect();
    (types, rows_total)
}

struct CsvPass2ChunkAccum {
    chunk_len: usize,
    rows_used: usize,
    nulls: Vec<usize>,
    uniques: Vec<HashSet<String>>,
    numeric_vals: Vec<Option<Vec<f64>>>,
    date_ts: Vec<Option<Vec<i64>>>,
    boolean_non_null_count: Vec<usize>,
    boolean_true_count: Vec<usize>,
}

fn csv_pass2_accumulate_chunk_rows(
    column_types: &[String],
    col_start: usize,
    col_end: usize,
    max_stats_rows: usize,
    max_distinct: usize,
    rows: impl Iterator<Item = Vec<String>>,
) -> Option<CsvPass2ChunkAccum> {
    let chunk_len = col_end.saturating_sub(col_start);
    if chunk_len == 0 || col_end > column_types.len() {
        return None;
    }

    let lim = max_stats_rows;
    let mut nulls = vec![0usize; chunk_len];
    let mut uniques: Vec<HashSet<String>> = (0..chunk_len).map(|_| HashSet::new()).collect();
    let mut numeric_vals: Vec<Option<Vec<f64>>> = (0..chunk_len)
        .map(|i| {
            let global = col_start + i;
            if column_types.get(global).is_some_and(|t| t == "number") {
                Some(Vec::new())
            } else {
                None
            }
        })
        .collect();
    let mut date_ts: Vec<Option<Vec<i64>>> = (0..chunk_len)
        .map(|i| {
            let global = col_start + i;
            if column_types
                .get(global)
                .is_some_and(|t| t == "timestamp" || t == "date")
            {
                Some(Vec::new())
            } else {
                None
            }
        })
        .collect();
    let mut boolean_non_null_count = vec![0usize; chunk_len];
    let mut boolean_true_count = vec![0usize; chunk_len];

    let mut rows_used = 0usize;
    for row in rows {
        if rows_used >= lim {
            break;
        }
        rows_used += 1;
        for local_i in 0..chunk_len {
            let col_idx = col_start + local_i;
            let val = row.get(col_idx).map_or("", std::string::String::as_str);
            if is_null_like_cell(val) {
                nulls[local_i] += 1;
                continue;
            }
            let set = &mut uniques[local_i];
            if set.len() < max_distinct || set.contains(val) {
                set.insert(val.to_string());
            }

            match column_types.get(col_idx).map(String::as_str) {
                Some("number") => {
                    if let Some(v) = val.parse::<f64>().ok().filter(|x| x.is_finite())
                        && let Some(ref mut nv) = numeric_vals[local_i]
                    {
                        nv.push(v);
                    }
                }
                Some("timestamp" | "date") => {
                    if let Some(ts) =
                        parse_timestamp_to_seconds(val).or_else(|| parse_date_to_timestamp(val))
                        && let Some(ref mut dv) = date_ts[local_i]
                    {
                        dv.push(ts);
                    }
                }
                Some("boolean") => {
                    boolean_non_null_count[local_i] += 1;
                    if matches!(val.to_lowercase().as_str(), "true" | "yes" | "1" | "y") {
                        boolean_true_count[local_i] += 1;
                    }
                }
                _ => {}
            }
        }
    }

    if rows_used == 0 {
        return None;
    }

    Some(CsvPass2ChunkAccum {
        chunk_len,
        rows_used,
        nulls,
        uniques,
        numeric_vals,
        date_ts,
        boolean_non_null_count,
        boolean_true_count,
    })
}

#[allow(clippy::type_complexity)]
fn csv_pass2_finalize_chunk_stats(
    column_types: &[String],
    col_start: usize,
    accum: CsvPass2ChunkAccum,
) -> (
    Option<Vec<f64>>,
    Option<Vec<usize>>,
    Option<Vec<Option<NumericStats>>>,
    Option<Vec<Option<DateStats>>>,
    Option<Vec<Option<BooleanStats>>>,
    usize,
) {
    let CsvPass2ChunkAccum {
        chunk_len,
        rows_used,
        nulls,
        uniques,
        numeric_vals,
        date_ts,
        boolean_non_null_count,
        boolean_true_count,
    } = accum;

    let null_percentages: Vec<f64> = nulls
        .iter()
        .map(|n| (*n as f64 / rows_used as f64) * 100.0)
        .collect();
    let unique_counts: Vec<usize> = uniques.iter().map(HashSet::len).collect();

    let mut numeric_stats: Vec<Option<NumericStats>> = vec![None; chunk_len];
    let mut date_stats: Vec<Option<DateStats>> = vec![None; chunk_len];
    let mut boolean_stats: Vec<Option<BooleanStats>> = vec![None; chunk_len];

    for local_i in 0..chunk_len {
        let col_idx = col_start + local_i;
        match column_types.get(col_idx).map(String::as_str) {
            Some("number") => {
                if let Some(ref nv) = numeric_vals[local_i] {
                    numeric_stats[local_i] = column_stats::compute_numeric_stats_from_values(nv);
                }
            }
            Some("timestamp" | "date") => {
                if let Some(ref dv) = date_ts[local_i] {
                    date_stats[local_i] =
                        column_stats::compute_date_stats_from_timestamps(dv.clone());
                }
            }
            Some("boolean") => {
                let total = boolean_non_null_count[local_i];
                if total > 0 {
                    boolean_stats[local_i] = Some(BooleanStats {
                        true_percentage: Some(
                            (boolean_true_count[local_i] as f64 / total as f64) * 100.0,
                        ),
                    });
                }
            }
            _ => {}
        }
    }

    (
        Some(null_percentages),
        Some(unique_counts),
        Some(numeric_stats),
        Some(date_stats),
        Some(boolean_stats),
        rows_used,
    )
}

/// CSV pass 2: null / unique / type-specific stats for columns `[col_start, col_end)`.
/// Rows processed capped by `max_stats_rows`. Used in column chunks to bound memory on wide files.
#[allow(clippy::type_complexity)]
pub(crate) fn csv_pass2_stats_range(
    column_types: &[String],
    col_start: usize,
    col_end: usize,
    max_stats_rows: usize,
    max_distinct: usize,
    _config: &RuntimeConfig,
    rows: impl Iterator<Item = Vec<String>>,
) -> (
    Option<Vec<f64>>,
    Option<Vec<usize>>,
    Option<Vec<Option<NumericStats>>>,
    Option<Vec<Option<DateStats>>>,
    Option<Vec<Option<BooleanStats>>>,
    usize,
) {
    let Some(accum) = csv_pass2_accumulate_chunk_rows(
        column_types,
        col_start,
        col_end,
        max_stats_rows,
        max_distinct,
        rows,
    ) else {
        return (None, None, None, None, None, 0);
    };
    csv_pass2_finalize_chunk_stats(column_types, col_start, accum)
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

    let pairs: Vec<(f64, usize)> = (0..column_count)
        .into_par_iter()
        .map(|col_idx| {
            let values = column_stats::extract_column_values(sample_data, col_idx);
            column_stats::compute_null_and_unique_stats(&values, config)
        })
        .collect();

    let mut null_percentages = Vec::with_capacity(column_count);
    let mut unique_counts = Vec::with_capacity(column_count);
    for (null_pct, unique_ct) in pairs {
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
    let triples: Vec<(
        Option<NumericStats>,
        Option<DateStats>,
        Option<BooleanStats>,
    )> = (0..column_count)
        .into_par_iter()
        .map(|col_idx| {
            if col_idx >= column_types.len() {
                return (None, None, None);
            }
            let col_type = &column_types[col_idx];
            match col_type.as_str() {
                "number" => (
                    compute_numeric_stats(sample_data, col_idx, config),
                    None,
                    None,
                ),
                "timestamp" | "date" => {
                    (None, compute_date_stats(sample_data, col_idx, config), None)
                }
                "boolean" => (
                    None,
                    None,
                    compute_boolean_stats(sample_data, col_idx, config),
                ),
                _ => (None, None, None),
            }
        })
        .collect();

    let mut numeric_stats = Vec::with_capacity(column_count);
    let mut date_stats = Vec::with_capacity(column_count);
    let mut boolean_stats = Vec::with_capacity(column_count);
    for (n, d, b) in triples {
        numeric_stats.push(n);
        date_stats.push(d);
        boolean_stats.push(b);
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
