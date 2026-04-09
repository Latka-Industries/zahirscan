//! Shared column statistics computation utilities
//! Used by both CSV and `SQLite` parsers for consistent statistics calculation

use chrono::DateTime;
use dashmap::{DashMap, DashSet};
use rayon::prelude::*;

use crate::config::RuntimeConfig;
use crate::parsers::traits::AdaptiveParallel;
use crate::results::{BooleanStats, DateStats, NumericStats};

/// Number of seconds constants for date statistics
const SECONDS_PER_MINUTE: f64 = 60.0;
const SECONDS_PER_HOUR: f64 = 60.0 * SECONDS_PER_MINUTE;
const SECONDS_PER_DAY: f64 = 24.0 * SECONDS_PER_HOUR;

/// Check if a string value is null or empty
/// Treats empty strings, "null", and "nil" as null values (case-insensitive)
#[inline]
fn is_null_or_empty(val: &str) -> bool {
    val.is_empty() || val.eq_ignore_ascii_case("null") || val.eq_ignore_ascii_case("nil")
}

/// Check if a string value represents a boolean "true"
/// Used for computing boolean statistics (true percentage)
#[inline]
fn is_true_value(val: &str) -> bool {
    matches!(val.to_lowercase().as_str(), "true" | "yes" | "1" | "y")
}

/// Cached mean for [`StatsCalculator`]: unset, computed empty slice, or computed value.
#[derive(Clone, Copy)]
enum MeanCache {
    Uncached,
    None,
    Some(f64),
}

/// Optimized statistics calculator with lazy computation and caching
/// Avoids redundant calculations (e.g., mean is reused by stdev)
struct StatsCalculator<'a> {
    values: &'a [f64],
    sorted_values: &'a [f64],
    cached_mean: std::cell::Cell<MeanCache>,
}

impl<'a> StatsCalculator<'a> {
    /// Create a new statistics calculator
    /// Requires both unsorted values (for mean/stdev) and sorted values (for median/IQR)
    fn new(values: &'a [f64], sorted_values: &'a [f64]) -> Self {
        Self {
            values,
            sorted_values,
            cached_mean: std::cell::Cell::new(MeanCache::Uncached),
        }
    }

    /// Calculate the mean, with caching to avoid redundant computation
    fn mean(&self) -> Option<f64> {
        match self.cached_mean.get() {
            MeanCache::Uncached => {
                let result = if self.values.is_empty() {
                    None
                } else {
                    Some(self.values.iter().sum::<f64>() / self.values.len() as f64)
                };
                self.cached_mean.set(match result {
                    None => MeanCache::None,
                    Some(m) => MeanCache::Some(m),
                });
                result
            }
            MeanCache::None => None,
            MeanCache::Some(m) => Some(m),
        }
    }

    /// Calculate the median from sorted values
    fn median(&self) -> Option<f64> {
        if self.sorted_values.is_empty() {
            return None;
        }
        let mid = self.sorted_values.len() / 2;
        if self.sorted_values.len().is_multiple_of(2) {
            Some(f64::midpoint(
                self.sorted_values[mid - 1],
                self.sorted_values[mid],
            ))
        } else {
            Some(self.sorted_values[mid])
        }
    }

    /// Calculate the interquartile range from sorted values
    fn iqr(&self) -> Option<f64> {
        if self.sorted_values.is_empty() {
            return None;
        }
        let q1_idx = self.sorted_values.len() / 4;
        let q3_idx = (3 * self.sorted_values.len()) / 4;
        Some(self.sorted_values[q3_idx] - self.sorted_values[q1_idx])
    }

    /// Calculate the standard deviation, reusing cached mean
    fn stdev(&self) -> Option<f64> {
        if self.values.is_empty() {
            return None;
        }
        let mean = self.mean()?; // Reuses cached mean!
        let variance = self.values.iter().map(|&v| (v - mean).powi(2)).sum::<f64>()
            / (self.values.len() - 1) as f64;
        Some(variance.sqrt())
    }
}

/// Extract a column's values from row-based data (e.g., CSV rows)
/// Returns a vector of strings for the specified column index
pub fn extract_column_values(sample_data: &[Vec<String>], col_idx: usize) -> Vec<String> {
    sample_data
        .iter()
        .filter_map(|row| {
            if col_idx < row.len() {
                Some(row[col_idx].clone())
            } else {
                None
            }
        })
        .collect()
}

/// Compute numeric statistics (min, max, mean, median, range, IQR, stdev) from a vector of values
/// Works with any iterable of f64 values (from CSV strings or `SQLite` query results)
pub fn compute_numeric_stats_from_values(values: &[f64]) -> Option<NumericStats> {
    if values.is_empty() {
        return None;
    }

    // Sort for min/max/median/IQR calculations
    let mut sorted_values = values.to_owned();
    sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let min = sorted_values.first().copied();
    let max = sorted_values.last().copied();
    let range = min.zip(max).map(|(min_val, max_val)| max_val - min_val);

    // Use optimized calculator to compute all statistics (mean is cached and reused by stdev)
    let calculator = StatsCalculator::new(values, &sorted_values);
    let mean = calculator.mean();
    let median = calculator.median();
    let iqr = calculator.iqr();
    let stdev = calculator.stdev(); // Reuses cached mean!

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

/// Compute numeric statistics from string values (parses strings to f64)
/// Used by CSV parser which works with string data
pub fn compute_numeric_stats_from_strings(
    values: &[String],
    config: &RuntimeConfig,
) -> Option<NumericStats> {
    // Collect and parse numeric values in parallel, filtering out NaN/invalid
    let parsed_values: Vec<f64> = values
        .par_iter_adaptive(config)
        .filter_map(|val| {
            // Skip null/empty values
            if is_null_or_empty(val) {
                return None;
            }
            // Parse as float and filter finite values
            val.parse::<f64>().ok().filter(|&v| v.is_finite())
        })
        .collect();

    compute_numeric_stats_from_values(&parsed_values)
}

/// Compute date statistics (span in days/minutes, min/max) from timestamp values
pub fn compute_date_stats_from_timestamps(timestamps: Vec<i64>) -> Option<DateStats> {
    if timestamps.is_empty() {
        return None;
    }

    let mut sorted = timestamps;
    sorted.sort_unstable();
    let min_ts = sorted.first().copied()?;
    let max_ts = sorted.last().copied()?;

    // Calculate span in seconds, then convert to days and minutes
    let span_seconds = (max_ts - min_ts) as f64;
    let span_days = span_seconds / SECONDS_PER_DAY;
    let span_minutes = span_seconds / SECONDS_PER_MINUTE;

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

/// Compute date statistics from string values (parses strings to timestamps)
/// Used by CSV parser which works with string data
pub fn compute_date_stats_from_strings(values: &[String]) -> Option<DateStats> {
    // Collect and parse date/timestamp values
    let timestamps: Vec<i64> = values
        .iter()
        .filter_map(|val| {
            // First try parsing as Unix timestamp (if it's a numeric timestamp)
            // Otherwise try parsing as date string
            crate::utils::typecheck::parse_timestamp_to_seconds(val)
                .or_else(|| crate::utils::typecheck::parse_date_to_timestamp(val))
        })
        .collect();

    compute_date_stats_from_timestamps(timestamps)
}

/// Compute boolean statistics (percentage of true values) from string values
/// Used by CSV parser which works with string data
pub fn compute_boolean_stats_from_strings(
    values: &[String],
    config: &RuntimeConfig,
) -> Option<BooleanStats> {
    // Use DashMap for thread-safe parallel counting
    let total_count: DashMap<(), usize> = DashMap::new();
    let true_count: DashMap<(), usize> = DashMap::new();

    // Count totals and true values in parallel
    values.par_iter_adaptive(config).for_each(|val| {
        // Skip null/empty values
        if is_null_or_empty(val) {
            return;
        }
        *total_count.entry(()).or_insert(0) += 1;
        // Check if value represents true
        if is_true_value(val) {
            *true_count.entry(()).or_insert(0) += 1;
        }
    });

    let total = total_count.into_iter().next().map_or(0, |((), c)| c);
    if total == 0 {
        return None;
    }

    let true_val = true_count.into_iter().next().map_or(0, |((), c)| c);
    let true_percentage = (true_val as f64 / total as f64) * 100.0;

    Some(BooleanStats {
        true_percentage: Some(true_percentage),
    })
}

/// Compute null percentage and unique count from string values
/// Returns (`null_percentage`, `unique_count`)
pub fn compute_null_and_unique_stats(values: &[String], config: &RuntimeConfig) -> (f64, usize) {
    let total = values.len();
    if total == 0 {
        return (0.0, 0);
    }

    // Use DashSet for thread-safe parallel unique value tracking
    let unique_set: DashSet<String> = DashSet::new();
    let null_count: DashMap<(), usize> = DashMap::new();

    // Count nulls and collect unique values in parallel
    values.par_iter_adaptive(config).for_each(|val| {
        // Check if null/empty
        if is_null_or_empty(val) {
            *null_count.entry(()).or_insert(0) += 1;
        } else {
            // Track unique values (DashSet handles deduplication)
            unique_set.insert(val.clone());
        }
    });

    let null_count_val = null_count.into_iter().next().map_or(0, |((), c)| c);
    let null_percentage = (null_count_val as f64 / total as f64) * 100.0;
    let unique_count = unique_set.len();

    (null_percentage, unique_count)
}

/// Compute text statistics (min/max/avg length, unique count) from string values
pub fn compute_text_stats_from_strings(
    values: &[String],
    config: &RuntimeConfig,
) -> Option<(usize, usize, f64, usize)> {
    // Filter out null/empty values for length calculations
    let non_null_values: Vec<&String> = values.iter().filter(|v| !is_null_or_empty(v)).collect();

    if non_null_values.is_empty() {
        return None;
    }

    let lengths: Vec<usize> = non_null_values.iter().map(|v| v.len()).collect();
    let min_length = *lengths.iter().min()?;
    let max_length = *lengths.iter().max()?;
    let avg_length = lengths.iter().sum::<usize>() as f64 / lengths.len() as f64;

    // Count unique values
    let unique_set: DashSet<String> = DashSet::new();
    non_null_values.par_iter_adaptive(config).for_each(|val| {
        unique_set.insert(val.clone());
    });
    let unique_count = unique_set.len();

    Some((min_length, max_length, avg_length, unique_count))
}
