//! Shared helpers for Arrow record batches and CSV-like column statistics.

use anyhow::Result;
use arrow_array::Array;
use arrow_array::RecordBatch;
use arrow_cast::display::array_value_to_string;
use arrow_schema::Schema;
use rayon::prelude::*;

use crate::config::RuntimeConfig;
use crate::parsers::{structured::table_sample_profile, traits::AdaptiveParallel};
use crate::results::{BooleanStats, DateStats, NumericStats};

/// One row as UTF-8 cell strings (empty string for null), for [`csv::infer_column_types`].
pub(crate) fn record_batch_row_to_strings(batch: &RecordBatch, row: usize) -> Result<Vec<String>> {
    let mut out = Vec::with_capacity(batch.num_columns());
    for col in 0..batch.num_columns() {
        let arr = batch.column(col);
        if arr.is_null(row) {
            out.push(String::new());
        } else {
            out.push(array_value_to_string(arr.as_ref(), row)?);
        }
    }
    Ok(out)
}

/// Materialize every row in `batch` as string vectors; uses adaptive parallel row decoding
/// when the batch is large enough to amortize Rayon overhead.
pub(crate) fn record_batch_all_rows_as_strings(
    batch: &RecordBatch,
    config: &RuntimeConfig,
) -> Result<Vec<Vec<String>>> {
    let n = batch.num_rows();
    if n == 0 {
        return Ok(Vec::new());
    }
    if n >= config.min_collection_size_for_chunking {
        (0..n)
            .collect::<Vec<_>>()
            .par_iter_adaptive(config)
            .map(|i| record_batch_row_to_strings(batch, i))
            .collect()
    } else {
        let mut rows = Vec::with_capacity(n);
        for i in 0..n {
            rows.push(record_batch_row_to_strings(batch, i)?);
        }
        Ok(rows)
    }
}

#[must_use]
pub(crate) fn schema_column_names(schema: &Schema) -> Vec<String> {
    schema.fields().iter().map(|f| f.name().clone()).collect()
}

#[must_use]
pub(crate) fn schema_arrow_dtype_strings(schema: &Schema) -> Vec<String> {
    schema
        .fields()
        .iter()
        .map(|f| format!("{}", f.data_type()))
        .collect()
}

pub(crate) struct TabularSampleStats {
    pub column_types: Option<Vec<String>>,
    pub null_percentages: Option<Vec<f64>>,
    pub unique_counts: Option<Vec<usize>>,
    pub numeric_stats: Option<Vec<Option<NumericStats>>>,
    pub date_stats: Option<Vec<Option<DateStats>>>,
    pub boolean_stats: Option<Vec<Option<BooleanStats>>>,
}

/// Run the same inference and [`crate::parsers::column_stats`] pipeline as CSV on sampled rows.
pub(crate) fn tabular_stats_from_sample(
    sample_data: &[Vec<String>],
    column_count: usize,
    config: &RuntimeConfig,
) -> TabularSampleStats {
    if sample_data.is_empty() || column_count == 0 {
        return TabularSampleStats {
            column_types: None,
            null_percentages: None,
            unique_counts: None,
            numeric_stats: None,
            date_stats: None,
            boolean_stats: None,
        };
    }
    let types = table_sample_profile::infer_column_types(sample_data, column_count, config);
    let (null_pcts, unique_cts) =
        table_sample_profile::compute_column_statistics(sample_data, column_count, config);
    let (num_stats, dt_stats, bool_stats) = table_sample_profile::compute_type_specific_statistics(
        sample_data,
        &types,
        column_count,
        config,
    );
    TabularSampleStats {
        column_types: Some(types),
        null_percentages: Some(null_pcts),
        unique_counts: Some(unique_cts),
        numeric_stats: Some(num_stats),
        date_stats: Some(dt_stats),
        boolean_stats: Some(bool_stats),
    }
}
