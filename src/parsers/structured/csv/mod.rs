//! Delimiter-separated text metadata (CSV, TSV, pipe-separated, etc.).
//!
//! Extensions `csv`, `tsv`, `tab`, and `psv` map to this parser; the `csv` crate reads rows using a
//! detected or path-hinted delimiter.
//!
//! **Row count** is a separate streaming pass that only counts records (no per-column stats).
//!
//! **Types and column stats** use a **bounded row sample** in memory: [`RuntimeConfig::max_csv_scan_rows`]
//! is the base cap, scaled down for large files (see byte threshold below) and for many columns.
//! Stats are computed in **column chunks** to cap memory on wide tables.
//! [`RuntimeConfig::max_csv_max_distinct_per_column`] bounds distinct strings per column.
//!
//! Large-file sample shrink uses the same per-decade rule as other tabular formats (see
//! [`crate::parsers::structured::columnar::utils::effective_sample_rows_after_file_byte_scaling`]).

mod utils;

pub use utils::*;

use anyhow::Result;
use csv::ReaderBuilder;
use std::io::Cursor;

use crate::config::RuntimeConfig;
use crate::parsers::{
    ParseResult,
    structured::{
        columnar::utils as columnar_utils,
        constants::{CsvEncodingLabel, limits},
        table_sample_profile,
    },
};
use crate::results::{
    BooleanStats, ColumnarCommonFields, CsvMetadata, DateStats, MergeColumnStatsInput,
    NumericStats, merge_column_stats,
};

fn csv_reader(content_str: &str, delim_byte: u8) -> csv::Reader<Cursor<&str>> {
    ReaderBuilder::new()
        .delimiter(delim_byte)
        .has_headers(true)
        .flexible(true)
        .from_reader(Cursor::new(content_str))
}

fn count_data_rows(content_str: &str, delim_byte: u8) -> usize {
    let mut reader = csv_reader(content_str, delim_byte);
    let _ = reader.headers();
    reader.records().filter_map(std::result::Result::ok).count()
}

fn collect_sample_rows(content_str: &str, delim_byte: u8, max_rows: usize) -> Vec<Vec<String>> {
    let mut reader = csv_reader(content_str, delim_byte);
    let _ = reader.headers();
    reader
        .records()
        .filter_map(std::result::Result::ok)
        .take(max_rows)
        .map(|rec| {
            rec.iter()
                .map(|s: &str| s.to_string())
                .collect::<Vec<String>>()
        })
        .collect()
}

fn csv_encoding_label(content: &[u8]) -> String {
    if std::str::from_utf8(content).is_ok() {
        CsvEncodingLabel::UTF8.to_string()
    } else {
        CsvEncodingLabel::NON_UTF8.to_string()
    }
}

fn non_utf8_csv_metadata(encoding: String) -> CsvMetadata {
    CsvMetadata {
        common: ColumnarCommonFields {
            encoding: Some(encoding),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn read_csv_headers(
    content_str: &str,
    delim_byte: u8,
) -> (Option<Vec<String>>, Option<usize>, Option<bool>) {
    let mut header_reader = csv_reader(content_str, delim_byte);
    match header_reader.headers() {
        Ok(headers) => {
            let names: Vec<String> = headers.iter().map(|s: &str| s.to_string()).collect();
            let count = names.len();
            (Some(names), Some(count), Some(true))
        }
        Err(_) => (None, None, Some(false)),
    }
}

fn csv_scaled_sample_cap(
    base_sample: usize,
    file_bytes: u64,
    column_count_from_headers: Option<usize>,
    row_count: usize,
) -> usize {
    let cc = column_count_from_headers
        .filter(|&c| c > 0)
        .unwrap_or(limits::TABULAR_COL_SCALE_NUMERATOR);
    let effective_sample =
        columnar_utils::tabular_effective_sample_rows(base_sample, file_bytes, cc);
    effective_sample.max(1).min(row_count)
}

#[allow(clippy::type_complexity)]
fn csv_pass2_chunked_column_stats(
    column_types_vec: Vec<String>,
    column_count: usize,
    stats_rows_sampled: usize,
    max_distinct: usize,
    config: &RuntimeConfig,
    sample_rows: &[Vec<String>],
) -> (
    Option<Vec<String>>,
    Option<Vec<f64>>,
    Option<Vec<usize>>,
    Option<Vec<Option<NumericStats>>>,
    Option<Vec<Option<DateStats>>>,
    Option<Vec<Option<BooleanStats>>>,
) {
    if column_count == 0 || column_types_vec.is_empty() || stats_rows_sampled == 0 {
        return (None, None, None, None, None, None);
    }

    let mut null_pct = vec![0.0f64; column_count];
    let mut unique_c = vec![0usize; column_count];
    let mut numeric_stats_acc = vec![None; column_count];
    let mut date_stats_acc = vec![None; column_count];
    let mut boolean_stats_acc = vec![None; column_count];

    for col_start in (0..column_count).step_by(limits::TABULAR_COLUMN_CHUNK) {
        let col_end = (col_start + limits::TABULAR_COLUMN_CHUNK).min(column_count);
        let (np, uc, ns, ds, bs, _ru) = table_sample_profile::csv_pass2_stats_range(
            &column_types_vec,
            col_start,
            col_end,
            stats_rows_sampled,
            max_distinct,
            config,
            sample_rows.iter().cloned(),
        );
        if let (Some(np), Some(uc), Some(ns), Some(ds), Some(bs)) = (np, uc, ns, ds, bs) {
            null_pct[col_start..col_end].copy_from_slice(&np);
            unique_c[col_start..col_end].copy_from_slice(&uc);
            for (i, v) in ns.into_iter().enumerate() {
                numeric_stats_acc[col_start + i] = v;
            }
            for (i, v) in ds.into_iter().enumerate() {
                date_stats_acc[col_start + i] = v;
            }
            for (i, v) in bs.into_iter().enumerate() {
                boolean_stats_acc[col_start + i] = v;
            }
        }
    }

    (
        Some(column_types_vec),
        Some(null_pct),
        Some(unique_c),
        Some(numeric_stats_acc),
        Some(date_stats_acc),
        Some(boolean_stats_acc),
    )
}

/// Extract CSV metadata
///
/// # Errors
///
/// Currently always returns [`Ok`]; malformed rows are skipped rather than failing.
pub fn extract_csv_metadata(
    content: &[u8],
    stats: &ParseResult,
    config: &RuntimeConfig,
) -> Result<CsvMetadata> {
    let encoding = csv_encoding_label(content);

    let Ok(content_str) = std::str::from_utf8(content) else {
        return Ok(non_utf8_csv_metadata(encoding));
    };

    let file_bytes = content.len() as u64;
    let base_sample = config.max_csv_scan_rows.max(1);
    let delim_byte = utils::delimiter_byte_for_reader(content_str, &stats.file_path);
    let field_sep = char::from_u32(u32::from(delim_byte)).unwrap_or(',');

    let (column_names, column_count_from_headers, has_header) =
        read_csv_headers(content_str, delim_byte);
    let row_count = count_data_rows(content_str, delim_byte);
    let effective_sample = csv_scaled_sample_cap(
        base_sample,
        file_bytes,
        column_count_from_headers,
        row_count,
    );

    let sample_rows = collect_sample_rows(content_str, delim_byte, effective_sample);
    let stats_rows_sampled = sample_rows.len();

    let mut column_count = column_count_from_headers.unwrap_or(0);
    let (column_types_vec, _) = table_sample_profile::csv_pass1_infer_types(
        column_count,
        stats_rows_sampled,
        sample_rows.iter().cloned(),
    );
    column_count = column_types_vec.len();

    let delimiter_display = utils::format_delimiter_for_metadata(delim_byte);
    let quote_character = utils::detect_quote_character(content_str, field_sep);
    let escape_character = utils::detect_escape_character(
        content_str,
        Some(delimiter_display.as_str()),
        quote_character.as_deref(),
    );

    let (column_types, null_percentages, unique_counts, numeric_stats, date_stats, boolean_stats) =
        csv_pass2_chunked_column_stats(
            column_types_vec,
            column_count,
            stats_rows_sampled,
            config.max_csv_max_distinct_per_column,
            config,
            &sample_rows,
        );

    let columns = if column_count > 0 {
        merge_column_stats(MergeColumnStatsInput {
            column_count,
            column_names: column_names.clone(),
            column_types,
            null_percentages,
            unique_counts,
            numeric_stats,
            date_stats,
            boolean_stats,
            physical_types: None,
        })
    } else {
        None
    };

    Ok(CsvMetadata {
        common: ColumnarCommonFields {
            row_count,
            column_count,
            stats_rows_sampled: Some(stats_rows_sampled),
            encoding: Some(encoding),
            columns,
        },
        delimiter: Some(delimiter_display),
        quote_character,
        escape_character,
        has_header,
    })
}

crate::no_template_mining!(
    extract_csv_templates,
    "CSV files don't have templates, return empty result."
);
