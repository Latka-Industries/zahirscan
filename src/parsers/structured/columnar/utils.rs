//! Shared helpers for Arrow record batches and CSV-like column statistics.

use anyhow::Result;
use arrow_array::Array;
use arrow_array::RecordBatch;
use arrow_cast::display::array_value_to_string;
use arrow_schema::Schema;
use rayon::prelude::*;

use crate::config::RuntimeConfig;
use crate::parsers::{
    structured::{
        constants::limits::{
            TABULAR_BPR_SKINNY_MAX_BYTES, TABULAR_BYTE_SCALE_MIN_RETAIN_FRAC,
            TABULAR_BYTE_SCALE_PCT_PER_DECADE, TABULAR_COLUMN_CHUNK,
            TABULAR_FULL_SCAN_MAX_FILE_BYTES, TABULAR_FULL_SCAN_MAX_ROWS,
            TABULAR_SAMPLE_BYTE_THRESHOLD,
        },
        table_sample_profile,
    },
    traits::AdaptiveParallel,
};
use crate::results::{
    BooleanStats, DateStats, MergeColumnStatsInput, NumericStats, merge_column_stats,
};

/// Re-export for callers that referenced `columnar::utils::TABULAR_COL_SCALE_NUMERATOR`.
pub use crate::parsers::structured::constants::limits::TABULAR_COL_SCALE_NUMERATOR;

/// Above [`TABULAR_SAMPLE_BYTE_THRESHOLD`], each full file-size decade (relative to the threshold)
/// reduces retained row-sample fraction by [`TABULAR_BYTE_SCALE_PCT_PER_DECADE`] (e.g. 2% → 98%, 96%, …),
/// floored at [`TABULAR_BYTE_SCALE_MIN_RETAIN_FRAC`].
#[must_use]
pub(crate) fn effective_sample_rows_after_file_byte_scaling(base: usize, file_bytes: u64) -> usize {
    if base <= 1 {
        return 1;
    }
    if file_bytes <= TABULAR_SAMPLE_BYTE_THRESHOLD {
        return base;
    }
    let ratio = (file_bytes as f64) / (TABULAR_SAMPLE_BYTE_THRESHOLD as f64);
    let k = ratio.log10().floor() as i32;
    let k = k.max(1);
    let retain = (1.0_f64 - TABULAR_BYTE_SCALE_PCT_PER_DECADE * f64::from(k))
        .max(TABULAR_BYTE_SCALE_MIN_RETAIN_FRAC);
    let scaled = (base as f64 * retain).floor() as usize;
    scaled.max(1)
}

/// Effective max sample rows for tabular binary formats: `base * TABULAR_COL_SCALE_NUMERATOR / max(columns, TABULAR_COL_SCALE_NUMERATOR)`.
#[must_use]
pub(crate) fn effective_tabular_sample_rows(base: usize, column_count: usize) -> usize {
    if base <= 1 {
        return 1;
    }
    let cc = column_count.max(1);
    (base.saturating_mul(TABULAR_COL_SCALE_NUMERATOR) / cc.max(TABULAR_COL_SCALE_NUMERATOR)).max(1)
}

/// File-size decade scaling (with optional **BPR** = `file_bytes / row_count` when rows are known), then
/// column-count scaling. Skinny, small files may use **all** rows up to [`TABULAR_FULL_SCAN_MAX_ROWS`] instead
/// of shrinking the base cap by byte decade alone (so e.g. a 5 MiB / 80 k-row CSV can scan every row).
#[must_use]
pub(crate) fn tabular_effective_sample_rows(
    base: usize,
    file_bytes: u64,
    column_count: usize,
    row_count: Option<usize>,
) -> usize {
    let after_file = match row_count {
        None | Some(0) => effective_sample_rows_after_file_byte_scaling(base, file_bytes),
        Some(rc) => {
            let bpr = file_bytes / (rc as u64);
            let skinny = bpr <= TABULAR_BPR_SKINNY_MAX_BYTES;
            let small_file = file_bytes <= TABULAR_FULL_SCAN_MAX_FILE_BYTES;
            if skinny && small_file && rc <= TABULAR_FULL_SCAN_MAX_ROWS {
                rc
            } else if skinny && small_file {
                effective_sample_rows_after_file_byte_scaling(base, file_bytes).min(rc)
            } else {
                effective_sample_rows_after_file_byte_scaling(base, file_bytes)
            }
        }
    };
    effective_tabular_sample_rows(after_file, column_count.max(1))
}

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

fn narrow_rows_for_columns(
    sample_data: &[Vec<String>],
    col_start: usize,
    col_end: usize,
) -> Vec<Vec<String>> {
    let mut out = Vec::with_capacity(sample_data.len());
    for row in sample_data {
        let mut slice = Vec::with_capacity(col_end - col_start);
        for i in col_start..col_end {
            slice.push(row.get(i).cloned().unwrap_or_default());
        }
        out.push(slice);
    }
    out
}

fn tabular_stats_from_sample_one_chunk(
    sample_data: &[Vec<String>],
    column_count: usize,
    config: &RuntimeConfig,
) -> TabularSampleStats {
    let types = table_sample_profile::infer_column_types(sample_data, column_count, config);
    let (null_percentages, unique_counts) =
        table_sample_profile::compute_column_statistics(sample_data, column_count, config);
    let (numeric_stats, date_stats, boolean_stats) =
        table_sample_profile::compute_type_specific_statistics(
            sample_data,
            &types,
            column_count,
            config,
        );
    TabularSampleStats {
        column_types: Some(types),
        null_percentages: Some(null_percentages),
        unique_counts: Some(unique_counts),
        numeric_stats: Some(numeric_stats),
        date_stats: Some(date_stats),
        boolean_stats: Some(boolean_stats),
    }
}

/// Inference + column stats on a bounded in-memory row sample (Parquet, Arrow, NPY, MTX string pass).
/// Wide tables use column chunks (same width as CSV pass-2).
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

    if column_count <= TABULAR_COLUMN_CHUNK {
        return tabular_stats_from_sample_one_chunk(sample_data, column_count, config);
    }

    let mut types = vec![String::new(); column_count];
    let mut null_pct = vec![0.0f64; column_count];
    let mut uniq = vec![0usize; column_count];
    let mut numeric_stats = vec![None; column_count];
    let mut date_stats = vec![None; column_count];
    let mut boolean_stats = vec![None; column_count];

    for col_start in (0..column_count).step_by(TABULAR_COLUMN_CHUNK) {
        let col_end = (col_start + TABULAR_COLUMN_CHUNK).min(column_count);
        let chunk_len = col_end - col_start;
        let narrow = narrow_rows_for_columns(sample_data, col_start, col_end);
        let ts = tabular_stats_from_sample_one_chunk(&narrow, chunk_len, config);
        if let (Some(t), Some(np), Some(u), Some(ns), Some(ds), Some(bs)) = (
            ts.column_types,
            ts.null_percentages,
            ts.unique_counts,
            ts.numeric_stats,
            ts.date_stats,
            ts.boolean_stats,
        ) {
            types[col_start..col_end].clone_from_slice(&t);
            null_pct[col_start..col_end].clone_from_slice(&np);
            uniq[col_start..col_end].clone_from_slice(&u);
            for (i, v) in ns.into_iter().enumerate() {
                numeric_stats[col_start + i] = v;
            }
            for (i, v) in ds.into_iter().enumerate() {
                date_stats[col_start + i] = v;
            }
            for (i, v) in bs.into_iter().enumerate() {
                boolean_stats[col_start + i] = v;
            }
        }
    }

    TabularSampleStats {
        column_types: Some(types),
        null_percentages: Some(null_pct),
        unique_counts: Some(uniq),
        numeric_stats: Some(numeric_stats),
        date_stats: Some(date_stats),
        boolean_stats: Some(boolean_stats),
    }
}

/// Merge [`TabularSampleStats`] with optional per-column physical types into [`crate::results::ColumnStat`] rows.
#[must_use]
pub(crate) fn columns_from_tabular_sample(
    column_count: usize,
    column_names: Option<Vec<String>>,
    ts: TabularSampleStats,
    physical_types: Option<Vec<String>>,
) -> Option<Vec<crate::results::ColumnStat>> {
    merge_column_stats(&MergeColumnStatsInput {
        column_count,
        column_names,
        column_types: ts.column_types,
        null_percentages: ts.null_percentages,
        unique_counts: ts.unique_counts,
        numeric_stats: ts.numeric_stats,
        date_stats: ts.date_stats,
        boolean_stats: ts.boolean_stats,
        physical_types,
    })
}
