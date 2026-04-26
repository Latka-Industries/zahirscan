//! Apache Arrow IPC file / stream and Feather v2 (IPC-based).
//!
//! Row count comes from a **first pass** that only sums per-batch row counts (no string materialization); the
//! sample cap then uses `Some(row_count)` for BPR. A **second pass** decodes batches again and fills the string sample.

use std::io::Cursor;

use anyhow::{Context, Result};
use arrow_array::RecordBatch;
use arrow_ipc::reader::{FileReader, StreamReader};
use arrow_schema::{ArrowError, Schema};

use crate::config::RuntimeConfig;
use crate::parsers::{
    ParseResult,
    structured::constants::{ArrowIpcContainerKind, StructuredEncoding},
};
use crate::results::{ArrowIpcMetadata, ColumnarCommonFields};

use super::utils;

fn feather_hint(path: &str, mmap: &[u8]) -> bool {
    path.to_lowercase().ends_with(".feather")
        || (mmap.len() >= 4 && mmap.get(0..4) == Some(b"FEA1"))
}

fn ipc_sum_row_counts(
    batches: impl Iterator<Item = std::result::Result<RecordBatch, ArrowError>>,
    decode_ctx: &'static str,
) -> Result<usize> {
    let mut n = 0usize;
    for batch in batches {
        let batch = batch.context(decode_ctx)?;
        n += batch.num_rows();
    }
    Ok(n)
}

fn ipc_column_layout(schema: &Schema) -> (Vec<String>, Vec<String>, usize) {
    let column_names = utils::schema_column_names(schema);
    let arrow_field_types = utils::schema_arrow_dtype_strings(schema);
    let column_count = column_names.len();
    (column_names, arrow_field_types, column_count)
}

fn ipc_bpr_max_sample(
    row_count_total: usize,
    file_bytes: u64,
    column_count: usize,
    config: &RuntimeConfig,
) -> usize {
    utils::tabular_effective_sample_rows(
        config.max_tabular_sample_rows,
        file_bytes,
        column_count.max(1),
        Some(row_count_total),
    )
}

fn ipc_reopen_file_and_sample(
    mmap: &[u8],
    max_sample: usize,
    config: &RuntimeConfig,
    decode_ctx: &'static str,
) -> Result<Vec<Vec<String>>> {
    let cursor = Cursor::new(mmap);
    let reader =
        FileReader::try_new(cursor, None).context("re-open Arrow IPC file reader for sampling")?;
    utils::record_batches_to_string_sample(reader, max_sample, config, decode_ctx)
}

fn ipc_reopen_stream_and_sample(
    mmap: &[u8],
    max_sample: usize,
    config: &RuntimeConfig,
    decode_ctx: &'static str,
) -> Result<Vec<Vec<String>>> {
    let cursor = Cursor::new(mmap);
    let reader = StreamReader::try_new(cursor, None)
        .context("re-open Arrow IPC stream reader for sampling")?;
    utils::record_batches_to_string_sample(reader, max_sample, config, decode_ctx)
}

/// Read Arrow IPC file, Feather v2, or IPC streaming format.
///
/// # Errors
///
/// Returns an error if the payload is not a recognized IPC/Feather encoding.
pub fn extract_arrow_ipc_metadata(
    mmap: &[u8],
    stats: &ParseResult,
    config: &RuntimeConfig,
) -> Result<ArrowIpcMetadata> {
    let (
        sample_data,
        row_count_total,
        column_names,
        arrow_field_types,
        column_count,
        container_kind,
    ) = if let Ok(file_reader) = FileReader::try_new(Cursor::new(mmap), None) {
        let schema = file_reader.schema();
        let (column_names, arrow_field_types, column_count) = ipc_column_layout(schema.as_ref());
        let file_bytes = mmap.len() as u64;
        let container_kind = Some(if feather_hint(&stats.file_path, mmap) {
            ArrowIpcContainerKind::FEATHER.to_string()
        } else {
            ArrowIpcContainerKind::IPC_FILE.to_string()
        });

        let row_count_total = ipc_sum_row_counts(file_reader, "decode IPC file batch (count)")?;
        let max_sample = ipc_bpr_max_sample(row_count_total, file_bytes, column_count, config);
        let sample_data =
            ipc_reopen_file_and_sample(mmap, max_sample, config, "decode IPC file batch")?;

        Ok::<_, anyhow::Error>((
            sample_data,
            row_count_total,
            column_names,
            arrow_field_types,
            column_count,
            container_kind,
        ))
    } else {
        let stream = StreamReader::try_new(Cursor::new(mmap), None)
            .context("open Arrow IPC stream reader")?;
        let schema = stream.schema();
        let (column_names, arrow_field_types, column_count) = ipc_column_layout(schema.as_ref());
        let file_bytes = mmap.len() as u64;
        let container_kind = Some(ArrowIpcContainerKind::IPC_STREAM.to_string());

        let row_count_total = ipc_sum_row_counts(stream, "decode IPC stream batch (count)")?;
        let max_sample = ipc_bpr_max_sample(row_count_total, file_bytes, column_count, config);
        let sample_data =
            ipc_reopen_stream_and_sample(mmap, max_sample, config, "decode IPC stream batch")?;

        Ok::<_, anyhow::Error>((
            sample_data,
            row_count_total,
            column_names,
            arrow_field_types,
            column_count,
            container_kind,
        ))
    }?;

    let stats_n = sample_data.len();
    let ts = utils::tabular_stats_from_sample(&sample_data, column_count, config);

    let columns = utils::columns_from_tabular_sample(
        column_count,
        Some(column_names),
        ts,
        Some(arrow_field_types),
    );

    Ok(ArrowIpcMetadata {
        common: ColumnarCommonFields {
            row_count: Some(row_count_total),
            column_count: Some(column_count),
            stats_rows_sampled: Some(stats_n),
            encoding: Some(StructuredEncoding::TABULAR_BINARY.to_string()),
            columns,
        },
        container_kind,
    })
}

crate::no_template_mining!(
    extract_arrow_ipc_templates,
    "Arrow IPC / Feather is binary columnar; no text template mining."
);
