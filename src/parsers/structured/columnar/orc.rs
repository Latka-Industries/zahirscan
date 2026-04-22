//! Apache ORC metadata via `orc-rust` (Arrow record batches).

use anyhow::{Context, Result};
use bytes::Bytes;
use orc_rust::arrow_reader::ArrowReaderBuilder;

use crate::config::RuntimeConfig;
use crate::parsers::{ParseResult, structured::constants::StructuredEncoding};
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
    let file_bytes = mmap.len() as u64;

    let max_sample = columnar_utils::tabular_effective_sample_rows(
        config.max_tabular_sample_rows,
        file_bytes,
        column_count.max(1),
        Some(row_count_total),
    );
    let reader = builder.with_batch_size(8192).build();
    let sample_data = columnar_utils::record_batches_to_string_sample(
        reader,
        max_sample,
        config,
        "read ORC batch",
    )?;

    let ts = columnar_utils::tabular_stats_from_sample(&sample_data, column_count, config);

    let columns = columnar_utils::columns_from_tabular_sample(
        column_count,
        Some(column_names),
        ts,
        Some(arrow_field_types),
    );

    Ok(OrcMetadata {
        common: ColumnarCommonFields {
            row_count: row_count_total,
            column_count,
            stats_rows_sampled: Some(sample_data.len()),
            encoding: Some(StructuredEncoding::TABULAR_BINARY.to_string()),
            columns,
        },
        num_stripes,
    })
}

crate::no_template_mining!(
    extract_orc_templates,
    "ORC is binary columnar; no text template mining."
);
