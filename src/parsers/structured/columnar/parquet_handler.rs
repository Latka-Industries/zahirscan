//! Apache Parquet metadata and column statistics.

use anyhow::{Context, Result};
use bytes::Bytes;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

use crate::config::RuntimeConfig;
use crate::parsers::{ParseResult, structured::constants::StructuredEncoding};
use crate::results::{ColumnarCommonFields, ParquetMetadata};

use super::utils as columnar_utils;

/// Extract Parquet schema, row counts, row groups, and CSV-like column statistics from a sample.
///
/// # Errors
///
/// Returns an error if the bytes are not a valid Parquet file or Arrow decoding fails.
pub fn extract_parquet_metadata(
    mmap: &[u8],
    _stats: &ParseResult,
    config: &RuntimeConfig,
) -> Result<ParquetMetadata> {
    let bytes = Bytes::copy_from_slice(mmap);
    let builder = ParquetRecordBatchReaderBuilder::try_new(bytes).context("open Parquet")?;
    let num_row_groups = Some(builder.metadata().num_row_groups());
    let row_count_total = builder
        .metadata()
        .file_metadata()
        .num_rows()
        .try_into()
        .unwrap_or(usize::MAX);
    let schema_ref = builder.schema().clone();
    let column_names = columnar_utils::schema_column_names(schema_ref.as_ref());
    let arrow_field_types = columnar_utils::schema_arrow_dtype_strings(schema_ref.as_ref());
    let column_count = column_names.len();
    let file_bytes = mmap.len() as u64;

    let max_sample = columnar_utils::tabular_effective_sample_rows(
        config.max_tabular_sample_rows,
        file_bytes,
        column_count.max(1),
    );
    let mut sample_data: Vec<Vec<String>> = Vec::new();
    let mut reader = builder.build().context("build Parquet reader")?;

    for batch in reader.by_ref() {
        let batch = batch.context("read Parquet batch")?;
        if sample_data.len() >= max_sample {
            break;
        }
        let rows = columnar_utils::record_batch_all_rows_as_strings(&batch, config)?;
        for row in rows {
            if sample_data.len() >= max_sample {
                break;
            }
            sample_data.push(row);
        }
    }

    let ts = columnar_utils::tabular_stats_from_sample(&sample_data, column_count, config);

    let columns = columnar_utils::columns_from_tabular_sample(
        column_count,
        Some(column_names),
        ts,
        Some(arrow_field_types),
    );

    Ok(ParquetMetadata {
        common: ColumnarCommonFields {
            row_count: row_count_total,
            column_count,
            stats_rows_sampled: Some(sample_data.len()),
            encoding: Some(StructuredEncoding::TABULAR_BINARY.to_string()),
            columns,
        },
        num_row_groups,
    })
}

crate::no_template_mining!(
    extract_parquet_templates,
    "Parquet is columnar binary; no text template mining."
);
