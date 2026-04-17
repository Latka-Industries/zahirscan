//! Delimiter-separated text metadata (CSV, TSV, pipe-separated, etc.).
//!
//! Extensions `csv`, `tsv`, `tab`, and `psv` map to this parser; the `csv` crate reads rows using a
//! detected or path-hinted delimiter.

mod utils;

pub use utils::*;

use anyhow::Result;
use csv::ReaderBuilder;
use std::io::Cursor;

use crate::config::RuntimeConfig;
use crate::parsers::{ParseResult, structured::table_sample_profile};
use crate::results::{ColumnarCommonFields, CsvMetadata};

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
    // Check if content is valid UTF-8
    let encoding = if std::str::from_utf8(content).is_ok() {
        Some("UTF-8".to_string())
    } else {
        // Try to detect other common encodings (simplified - just mark as non-UTF-8)
        Some("Non-UTF-8".to_string())
    };

    // Try to read as UTF-8 first
    let Ok(content_str) = std::str::from_utf8(content) else {
        // If not UTF-8, return minimal metadata with encoding info
        return Ok(CsvMetadata {
            common: ColumnarCommonFields {
                encoding,
                ..Default::default()
            },
            ..Default::default()
        });
    };

    // Delimiter: content sniffing + path hints (`.tsv`/`.tab` → tab, `.psv` → pipe)
    let delim_byte = utils::delimiter_byte_for_reader(content_str, &stats.file_path);
    let field_sep = char::from_u32(u32::from(delim_byte)).unwrap_or(',');

    let mut reader = ReaderBuilder::new()
        .delimiter(delim_byte)
        .has_headers(true) // Try to read headers first
        .flexible(true) // Allow varying number of fields per row
        .from_reader(Cursor::new(content_str));

    // Try to read headers
    let headers_result = reader.headers();
    let (column_names, column_count_from_headers, has_header) = match headers_result {
        Ok(headers) => {
            let names: Vec<String> = headers.iter().map(|s: &str| s.to_string()).collect();
            let count = names.len();
            (Some(names), Some(count), Some(true))
        }
        Err(_) => (None, None, Some(false)),
    };

    let delimiter_display = utils::format_delimiter_for_metadata(delim_byte);
    let quote_character = utils::detect_quote_character(content_str, field_sep);
    let escape_character = utils::detect_escape_character(
        content_str,
        Some(delimiter_display.as_str()),
        quote_character.as_deref(),
    );

    // Sample rows for data type inference
    let max_sample_rows = config.max_csv_sample_rows;
    let mut row_count = 0;
    let mut column_count: usize = column_count_from_headers.unwrap_or(0);
    let mut sample_data: Vec<Vec<String>> = Vec::new();

    for result in reader.records() {
        if let Ok(record) = result {
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
        } else {
            // Skip malformed rows
        }
    }

    // Infer column types and compute statistics using probabilistic analysis
    let (column_types, null_percentages, unique_counts, numeric_stats, date_stats, boolean_stats) =
        if !sample_data.is_empty() && column_count > 0 {
            let types =
                table_sample_profile::infer_column_types(&sample_data, column_count, config);
            let (null_pcts, unique_cts) =
                table_sample_profile::compute_column_statistics(&sample_data, column_count, config);
            let (num_stats, dt_stats, bool_stats) =
                table_sample_profile::compute_type_specific_statistics(
                    &sample_data,
                    &types,
                    column_count,
                    config,
                );
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
        common: ColumnarCommonFields {
            row_count,
            column_count,
            column_names,
            encoding,
            column_types,
            null_percentages,
            unique_counts,
            numeric_stats,
            date_stats,
            boolean_stats,
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
