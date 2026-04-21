//! Apache Arrow IPC file / stream and Feather v2 (IPC-based).

use std::io::Cursor;

use anyhow::{Context, Result};

use arrow_ipc::reader::{FileReader, StreamReader};

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
    let mut sample_data: Vec<Vec<String>> = Vec::new();
    let mut row_count_total: usize = 0;
    let column_count: usize;
    let column_names: Vec<String>;
    let arrow_field_types: Vec<String>;
    let container_kind: Option<String>;

    let cursor = Cursor::new(mmap);
    if let Ok(file_reader) = FileReader::try_new(cursor, None) {
        let schema = file_reader.schema();
        column_names = utils::schema_column_names(schema.as_ref());
        arrow_field_types = utils::schema_arrow_dtype_strings(schema.as_ref());
        column_count = column_names.len();
        let file_bytes = mmap.len() as u64;
        let max_sample = utils::tabular_effective_sample_rows(
            config.max_tabular_sample_rows,
            file_bytes,
            column_count.max(1),
        );
        container_kind = Some(if feather_hint(&stats.file_path, mmap) {
            ArrowIpcContainerKind::FEATHER.to_string()
        } else {
            ArrowIpcContainerKind::IPC_FILE.to_string()
        });

        for batch in file_reader {
            let batch = batch.context("decode IPC file batch")?;
            row_count_total += batch.num_rows();
            if sample_data.len() >= max_sample {
                continue;
            }
            let rows = utils::record_batch_all_rows_as_strings(&batch, config)?;
            for row in rows {
                if sample_data.len() >= max_sample {
                    break;
                }
                sample_data.push(row);
            }
        }
    } else {
        let cursor = Cursor::new(mmap);
        let stream = StreamReader::try_new(cursor, None).context("open Arrow IPC stream reader")?;
        let schema = stream.schema();
        column_names = utils::schema_column_names(schema.as_ref());
        arrow_field_types = utils::schema_arrow_dtype_strings(schema.as_ref());
        column_count = column_names.len();
        let file_bytes = mmap.len() as u64;
        let max_sample = utils::tabular_effective_sample_rows(
            config.max_tabular_sample_rows,
            file_bytes,
            column_count.max(1),
        );
        container_kind = Some(ArrowIpcContainerKind::IPC_STREAM.to_string());

        for batch in stream {
            let batch = batch.context("decode IPC stream batch")?;
            row_count_total += batch.num_rows();
            if sample_data.len() >= max_sample {
                continue;
            }
            let rows = utils::record_batch_all_rows_as_strings(&batch, config)?;
            for row in rows {
                if sample_data.len() >= max_sample {
                    break;
                }
                sample_data.push(row);
            }
        }
    }

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
            row_count: row_count_total,
            column_count,
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
