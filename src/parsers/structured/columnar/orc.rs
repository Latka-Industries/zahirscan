//! Apache ORC metadata via `orc-rust` (Arrow record batches).

use anyhow::{Context, Result};
use bytes::Bytes;
use orc_rust::arrow_reader::ArrowReaderBuilder;

use crate::config::RuntimeConfig;
use crate::parsers::ParseResult;
use crate::results::{ColumnarCommonFields, OrcMetadata};

use super::utils as columnar_utils;

/// Extract ORC footer stats, schema, and CSV-like column statistics from a sample.
///
/// # Errors
///
/// Returns an error if the bytes are not a valid ORC file or Arrow decoding fails.
pub fn extract_orc_metadata(
    mmap: &[u8],
    _stats: &ParseResult,
    config: &RuntimeConfig,
) -> Result<OrcMetadata> {
    let bytes = Bytes::copy_from_slice(mmap);
    let builder = ArrowReaderBuilder::try_new(bytes).context("open ORC")?;
    let row_count_total = builder.file_metadata().number_of_rows() as usize;
    let num_stripes = Some(builder.file_metadata().stripe_metadatas().len());
    let schema_ref = builder.schema();
    let column_names = columnar_utils::schema_column_names(schema_ref.as_ref());
    let arrow_field_types = columnar_utils::schema_arrow_dtype_strings(schema_ref.as_ref());
    let column_count = column_names.len();

    let max_sample = config.max_csv_sample_rows;
    let mut sample_data: Vec<Vec<String>> = Vec::new();
    let reader = builder.with_batch_size(8192).build();

    for batch in reader {
        let batch = batch
            .map_err(|e| anyhow::anyhow!(e))
            .context("read ORC batch")?;
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

    Ok(OrcMetadata {
        common: ColumnarCommonFields {
            row_count: row_count_total,
            column_count,
            column_names: Some(column_names),
            column_types: ts.column_types,
            encoding: Some("binary".to_string()),
            null_percentages: ts.null_percentages,
            unique_counts: ts.unique_counts,
            numeric_stats: ts.numeric_stats,
            date_stats: ts.date_stats,
            boolean_stats: ts.boolean_stats,
        },
        arrow_field_types: Some(arrow_field_types),
        num_stripes,
    })
}

crate::no_template_mining!(
    extract_orc_templates,
    "ORC is binary columnar; no text template mining."
);
