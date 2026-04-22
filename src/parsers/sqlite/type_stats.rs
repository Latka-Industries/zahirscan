//! Type-specific column statistics for `SQLite` (INTEGER, REAL, TEXT, BLOB).

use log::debug;
use rusqlite::Connection;

use crate::config::RuntimeConfig;
use crate::parsers::column_stats;
use crate::results::{BlobStats, ColumnInfo, TextStats};
use crate::utils::typecheck::{is_boolean, parse_date_to_timestamp, parse_timestamp_to_seconds};

/// Compute type-specific statistics for a column based on its `SQLite` type.
/// Dispatches to appropriate compute function: numeric, text/date, or blob stats.
pub(super) fn compute_stats_for_type(
    conn_ref: &Connection,
    quoted_table_ref: &str,
    quoted_col_ref: &str,
    col_mut_ref: &mut ColumnInfo,
    values_ref: &[String],
    config_ref: &RuntimeConfig,
) {
    match col_mut_ref.type_name.as_deref() {
        Some("INTEGER" | "REAL" | "NUMERIC") => {
            compute_numeric_and_bool_stats(
                conn_ref,
                quoted_table_ref,
                quoted_col_ref,
                col_mut_ref,
                values_ref,
                config_ref,
            );
        }
        Some("TEXT") => compute_text_and_date_stats(col_mut_ref, values_ref, config_ref),
        Some("BLOB") => compute_blob_stats(conn_ref, quoted_table_ref, quoted_col_ref, col_mut_ref),
        _ => {}
    }
}

/// Fetches a column's values as strings (CAST to TEXT). Empty on error or no rows.
pub(super) fn fetch_column_as_strings(
    conn_ref: &Connection,
    quoted_table_ref: &str,
    quoted_col_ref: &str,
    table_name_ref: &str,
    col_name_ref: &str,
) -> Vec<String> {
    let query = format!("SELECT CAST({quoted_col_ref} AS TEXT) FROM {quoted_table_ref};");
    let all_values: Vec<Option<String>> = match conn_ref.prepare(&query) {
        Ok(mut stmt) => stmt
            .query_map([], |row| row.get::<_, Option<String>>(0))
            .ok()
            .map(|rows| rows.filter_map(std::result::Result::ok).collect())
            .unwrap_or_default(),
        Err(e) => {
            debug!(
                "SQLite query failed for column '{col_name_ref}' in table '{table_name_ref}': {e}"
            );
            return Vec::new();
        }
    };
    all_values
        .into_iter()
        .map(std::option::Option::unwrap_or_default)
        .collect()
}

/// Fills `numeric_stats` for INTEGER/REAL, or only `boolean_stats` for INTEGER columns whose
/// non-empty values all look boolean (e.g. 0/1). Those two are mutually exclusive.
fn compute_numeric_and_bool_stats(
    conn_ref: &Connection,
    quoted_table_ref: &str,
    quoted_col_ref: &str,
    col_mut_ref: &mut ColumnInfo,
    values_ref: &[String],
    config_ref: &RuntimeConfig,
) {
    let f64_query = format!("SELECT {quoted_col_ref} FROM {quoted_table_ref};");
    let numeric_values: Vec<f64> = match conn_ref.prepare(&f64_query) {
        Ok(mut stmt) => stmt
            .query_map([], |row| row.get::<_, Option<f64>>(0))
            .ok()
            .map(|rows| {
                rows.filter_map(|r| r.ok().flatten())
                    .filter(|&v| v.is_finite())
                    .collect()
            })
            .unwrap_or_default(),
        Err(_) => values_ref
            .iter()
            .filter_map(|v| {
                if v.is_empty() || v.eq_ignore_ascii_case("null") || v.eq_ignore_ascii_case("nil") {
                    None
                } else {
                    v.parse::<f64>().ok().filter(|&v| v.is_finite())
                }
            })
            .collect(),
    };

    let integer_all_boolean_like =
        col_mut_ref.type_name.as_deref() == Some("INTEGER") && !values_ref.is_empty() && {
            let non_empty: Vec<&String> = values_ref.iter().filter(|v| !v.is_empty()).collect();
            !non_empty.is_empty() && non_empty.iter().all(|v| is_boolean(v))
        };

    if integer_all_boolean_like {
        col_mut_ref.numeric_stats = None;
        col_mut_ref.boolean_stats =
            column_stats::compute_boolean_stats_from_strings(values_ref, config_ref);
        return;
    }

    if !numeric_values.is_empty() {
        col_mut_ref.numeric_stats =
            column_stats::compute_numeric_stats_from_values(&numeric_values);
    }
}

/// Fills `text_stats` and `date_stats` when >50% values look like dates.
/// `unique_count` is left to [`column_stats::compute_null_and_unique_stats`] (same distinct-non-null definition).
fn compute_text_and_date_stats(
    col_mut_ref: &mut ColumnInfo,
    values_ref: &[String],
    config_ref: &RuntimeConfig,
) {
    if let Some((min_len, max_len, avg_len, _unique_ct)) =
        column_stats::compute_text_stats_from_strings(values_ref, config_ref)
    {
        col_mut_ref.text_stats = Some(TextStats {
            min_length: Some(min_len),
            max_length: Some(max_len),
            avg_length: Some(avg_len),
        });
    }
    let date_like = values_ref
        .iter()
        .filter(|v| {
            !v.is_empty()
                && (parse_timestamp_to_seconds(v).is_some() || parse_date_to_timestamp(v).is_some())
        })
        .count();
    if date_like as f64 / values_ref.len() as f64 > 0.5 {
        col_mut_ref.date_stats = column_stats::compute_date_stats_from_strings(values_ref);
    }
}

/// Fills `blob_stats` (min/max/avg byte size) via `SQLite` `length()`.
fn compute_blob_stats(
    conn_ref: &Connection,
    quoted_table_ref: &str,
    quoted_col_ref: &str,
    col_mut_ref: &mut ColumnInfo,
) {
    let sql = format!(
        "SELECT length({quoted_col_ref}) FROM {quoted_table_ref} WHERE {quoted_col_ref} IS NOT NULL;"
    );
    let sizes: Vec<usize> = match conn_ref.prepare(&sql) {
        Ok(mut stmt) => stmt
            .query_map([], |row| row.get::<_, Option<i64>>(0))
            .ok()
            .map(|rows| {
                rows.filter_map(|r| r.ok().flatten().and_then(|s| usize::try_from(s).ok()))
                    .collect()
            })
            .unwrap_or_default(),
        Err(_) => Vec::new(),
    };
    if !sizes.is_empty() {
        // Guard above guarantees sizes is non-empty.
        let min_size = *sizes.iter().min().expect("sizes non-empty");
        let max_size = *sizes.iter().max().expect("sizes non-empty");
        let avg_size = sizes.iter().sum::<usize>() as f64 / sizes.len() as f64;
        col_mut_ref.blob_stats = Some(BlobStats {
            min_size: Some(min_size),
            max_size: Some(max_size),
            avg_size: Some(avg_size),
        });
    }
}
