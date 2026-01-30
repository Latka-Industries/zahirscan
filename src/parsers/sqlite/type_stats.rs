//! Type-specific column statistics for SQLite (INTEGER, REAL, TEXT, BLOB).

use log::debug;
use rusqlite::Connection;

use crate::engine::config::Config;
use crate::engine::tools::{is_boolean, parse_date_to_timestamp, parse_timestamp_to_seconds};
use crate::parsers::column_stats;
use crate::results::{BlobStats, ColumnInfo, TextStats};

/// Compute type-specific statistics for a column based on its SQLite type.
/// Dispatches to appropriate compute function: numeric, text/date, or blob stats.
pub(super) fn compute_stats_for_type(
    conn: &Connection,
    quoted_table: &str,
    quoted_col: &str,
    col: &mut ColumnInfo,
    values: &[String],
    config: &Config,
) {
    match col.type_name.as_deref() {
        Some("INTEGER") | Some("REAL") => {
            compute_numeric_and_bool_stats(conn, quoted_table, quoted_col, col, values, config);
        }
        Some("TEXT") => compute_text_and_date_stats(col, values, config),
        Some("BLOB") => compute_blob_stats(conn, quoted_table, quoted_col, col),
        _ => {}
    }
}

/// Fetches a column's values as strings (CAST to TEXT). Empty on error or no rows.
pub(super) fn fetch_column_as_strings(
    conn: &Connection,
    quoted_table: &str,
    quoted_col: &str,
    table_name: &str,
    col_name: &str,
) -> Vec<String> {
    let query = format!("SELECT CAST({} AS TEXT) FROM {};", quoted_col, quoted_table);
    let all_values: Vec<Option<String>> = match conn.prepare(&query) {
        Ok(mut stmt) => stmt
            .query_map([], |row| row.get::<_, Option<String>>(0))
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default(),
        Err(e) => {
            debug!(
                "SQLite query failed for column '{}' in table '{}': {}",
                col_name, table_name, e
            );
            return Vec::new();
        }
    };
    all_values
        .into_iter()
        .map(|v| v.unwrap_or_default())
        .collect()
}

/// Fills numeric_stats and, for INTEGER with 0/1-only values, boolean_stats.
fn compute_numeric_and_bool_stats(
    conn: &Connection,
    quoted_table: &str,
    quoted_col: &str,
    col: &mut ColumnInfo,
    values: &[String],
    config: &Config,
) {
    let f64_query = format!("SELECT {} FROM {};", quoted_col, quoted_table);
    let numeric_values: Vec<f64> = match conn.prepare(&f64_query) {
        Ok(mut stmt) => stmt
            .query_map([], |row| row.get::<_, Option<f64>>(0))
            .ok()
            .map(|rows| {
                rows.filter_map(|r| r.ok().flatten())
                    .filter(|&v| v.is_finite())
                    .collect()
            })
            .unwrap_or_default(),
        Err(_) => values
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
    if !numeric_values.is_empty() {
        col.numeric_stats = column_stats::compute_numeric_stats_from_values(numeric_values);
    }
    if col.type_name.as_deref() == Some("INTEGER") && !values.is_empty() {
        let non_empty: Vec<&String> = values.iter().filter(|v| !v.is_empty()).collect();
        if !non_empty.is_empty() && non_empty.iter().all(|v| is_boolean(v)) {
            col.boolean_stats = column_stats::compute_boolean_stats_from_strings(values, config);
        }
    }
}

/// Fills text_stats, unique_count (from text), and date_stats when >50% values look like dates.
fn compute_text_and_date_stats(col: &mut ColumnInfo, values: &[String], config: &Config) {
    if let Some((min_len, max_len, avg_len, unique_ct)) =
        column_stats::compute_text_stats_from_strings(values, config)
    {
        col.text_stats = Some(TextStats {
            min_length: Some(min_len),
            max_length: Some(max_len),
            avg_length: Some(avg_len),
        });
        col.unique_count = Some(unique_ct);
    }
    let date_like = values
        .iter()
        .filter(|v| {
            !v.is_empty()
                && (parse_timestamp_to_seconds(v).is_some() || parse_date_to_timestamp(v).is_some())
        })
        .count();
    if date_like as f64 / values.len() as f64 > 0.5 {
        col.date_stats = column_stats::compute_date_stats_from_strings(values);
    }
}

/// Fills blob_stats (min/max/avg byte size) via SQLite length().
fn compute_blob_stats(
    conn: &Connection,
    quoted_table: &str,
    quoted_col: &str,
    col: &mut ColumnInfo,
) {
    let sql = format!(
        "SELECT length({}) FROM {} WHERE {} IS NOT NULL;",
        quoted_col, quoted_table, quoted_col
    );
    let sizes: Vec<usize> = match conn.prepare(&sql) {
        Ok(mut stmt) => stmt
            .query_map([], |row| row.get::<_, Option<i64>>(0))
            .ok()
            .map(|rows| {
                rows.filter_map(|r| r.ok().flatten().map(|s| s as usize))
                    .collect()
            })
            .unwrap_or_default(),
        Err(_) => Vec::new(),
    };
    if !sizes.is_empty() {
        let min_size = *sizes.iter().min().unwrap();
        let max_size = *sizes.iter().max().unwrap();
        let avg_size = sizes.iter().sum::<usize>() as f64 / sizes.len() as f64;
        col.blob_stats = Some(BlobStats {
            min_size: Some(min_size),
            max_size: Some(max_size),
            avg_size: Some(avg_size),
        });
    }
}
