//! Matrix Market (`.mtx`) — metadata via [`matrix_market_rs`] (opens by path; `mmap` unused).
//!
//! Columns are always numeric; we do **not** run the CSV string-inference pipeline. **Per-column
//! [`NumericStats`](crate::results::NumericStats)**: dense arrays use every matrix entry; large sparse
//! matrices use a **single COO pass** to compute logical-column mean / min / max / stdev (implicit
//! zeros included). When `rows × cols` is small enough, sparse matrices are **materialized** per column
//! so median / IQR match the dense path (see [`MtxMetadata::numeric_stats_include_implicit_zeros`](crate::results::MtxMetadata)).

use anyhow::Result;
use matrix_market_rs::{MtxData, MtxError, SymInfo};
use memmap2::Mmap;
use rayon::prelude::*;

use crate::config::RuntimeConfig;
use crate::parsers::{
    ParseResult, column_stats,
    structured::constants::{MtxSymmetryLabel, StructuredEncoding, limits},
    traits::optimal_chunk_size,
};
use crate::results::{
    BooleanStats, ColumnarCommonFields, DateStats, MergeColumnStatsInput, MtxMetadata,
    NumericStats, merge_column_stats,
};

/// Inferred `column_types` plus non-numeric satellite vectors for [`merge_column_stats`](crate::results::merge_column_stats). MTX fills
/// numeric stats separately; string-based fields are left empty / default.
struct MtxInferenceTabular {
    column_types: Vec<String>,
    null_percentages: Option<Vec<f64>>,
    unique_counts: Option<Vec<usize>>,
    date_stats: Vec<Option<DateStats>>,
    boolean_stats: Vec<Option<BooleanStats>>,
}

fn symmetry_str(sym: SymInfo) -> &'static str {
    match sym {
        SymInfo::General => MtxSymmetryLabel::GENERAL,
        SymInfo::Symmetric => MtxSymmetryLabel::SYMMETRIC,
    }
}

/// Matrix Market dense `array` layout is column-major (Fortran order): index `k = row + col * rows`.
#[inline]
fn dense_linear_idx(row: usize, col: usize, rows: usize) -> usize {
    row + col * rows
}

fn default_mtx_inference(cols: usize) -> MtxInferenceTabular {
    MtxInferenceTabular {
        column_types: vec!["number".to_string(); cols],
        null_percentages: None,
        unique_counts: None,
        date_stats: vec![None; cols],
        boolean_stats: vec![None; cols],
    }
}

/// Per-column numeric stats with the same sequential vs Rayon split used elsewhere in this module.
fn numeric_stats_per_column(
    cols: usize,
    config: &RuntimeConfig,
    stat_col: impl Fn(usize) -> Option<NumericStats> + Sync + Send,
) -> Vec<Option<NumericStats>> {
    if cols < config.min_collection_size_for_chunking {
        return (0..cols).map(&stat_col).collect();
    }
    let chunk_size = optimal_chunk_size(
        cols,
        config.target_chunks_per_file,
        config.min_collection_size_for_chunking,
    );
    (0..cols)
        .into_par_iter()
        .with_min_len(chunk_size)
        .map(stat_col)
        .collect()
}

fn mtx_columnar_common(
    rows: usize,
    cols: usize,
    inf: MtxInferenceTabular,
    numeric_stats: Vec<Option<NumericStats>>,
) -> ColumnarCommonFields {
    let names: Vec<String> = (0..cols).map(|i| format!("col{i}")).collect();
    let columns = merge_column_stats(&MergeColumnStatsInput {
        column_count: cols,
        column_names: Some(names),
        column_types: Some(inf.column_types),
        null_percentages: inf.null_percentages,
        unique_counts: inf.unique_counts,
        numeric_stats: Some(numeric_stats),
        date_stats: Some(inf.date_stats),
        boolean_stats: Some(inf.boolean_stats),
        physical_types: None,
    });
    ColumnarCommonFields {
        row_count: rows,
        column_count: cols,
        // Same as `row_count`: MTX numeric stats use the full logical height (no row subsampling).
        // Kept for parity with other columnar types that report a true sample size.
        stats_rows_sampled: Some(rows),
        encoding: Some(StructuredEncoding::MATRIX_MARKET.to_string()),
        columns,
    }
}

fn full_numeric_dense<T: Copy + Default + Sync>(
    values: &[T],
    rows: usize,
    cols: usize,
    config: &RuntimeConfig,
    to_f64: impl Fn(T) -> f64 + Copy + Sync + Send,
) -> Vec<Option<NumericStats>> {
    numeric_stats_per_column(cols, config, |j| {
        let mut col = Vec::with_capacity(rows);
        for i in 0..rows {
            let k = dense_linear_idx(i, j, rows);
            col.push(to_f64(values.get(k).copied().unwrap_or_default()));
        }
        column_stats::compute_numeric_stats_from_values(&col)
    })
}

fn full_numeric_sparse_materialized<T: Copy>(
    indices: &[[usize; 2]],
    values: &[T],
    rows: usize,
    cols: usize,
    config: &RuntimeConfig,
    to_f64: impl Fn(T) -> f64 + Copy,
) -> Vec<Option<NumericStats>> {
    let mut full: Vec<Vec<f64>> = (0..cols).map(|_| vec![0.0f64; rows]).collect();
    for (idx, v) in indices.iter().zip(values.iter()) {
        let r = idx[0];
        let c = idx[1];
        if r < rows && c < cols {
            full[c][r] = to_f64(*v);
        }
    }
    numeric_stats_per_column(cols, config, |j| {
        column_stats::compute_numeric_stats_from_values(&full[j])
    })
}

/// Single pass over COO triplets: per **logical** column, mean / min / max / population stdev with
/// implicit zeros (missing `(row, col)` entries) included. Omits median and IQR (exact values need a
/// full sorted column or materialization).
fn full_numeric_sparse_logical_coo<T: Copy>(
    indices: &[[usize; 2]],
    values: &[T],
    rows: usize,
    cols: usize,
    to_f64: impl Fn(T) -> f64 + Copy,
) -> Vec<Option<NumericStats>> {
    struct Acc {
        sum: f64,
        sum_sq: f64,
        min_s: f64,
        max_s: f64,
        nnz: usize,
    }
    let mut acc: Vec<Acc> = (0..cols)
        .map(|_| Acc {
            sum: 0.0,
            sum_sq: 0.0,
            min_s: f64::INFINITY,
            max_s: f64::NEG_INFINITY,
            nnz: 0,
        })
        .collect();

    for (idx, v) in indices.iter().zip(values.iter()) {
        let r = idx[0];
        let c = idx[1];
        if r >= rows || c >= cols {
            continue;
        }
        let x = to_f64(*v);
        if !x.is_finite() {
            continue;
        }
        let a = &mut acc[c];
        a.sum += x;
        a.sum_sq += x * x;
        a.min_s = a.min_s.min(x);
        a.max_s = a.max_s.max(x);
        a.nnz += 1;
    }

    let rf = rows as f64;
    acc.into_iter()
        .map(|a| {
            let nnz = a.nnz;
            if nnz == 0 {
                return Some(NumericStats {
                    min: Some(0.0),
                    max: Some(0.0),
                    mean: Some(0.0),
                    median: None,
                    range: Some(0.0),
                    iqr: None,
                    stdev: Some(0.0),
                });
            }
            let mean = a.sum / rf;
            let e2 = a.sum_sq / rf;
            let var = (e2 - mean * mean).max(0.0);
            let stdev = var.sqrt();

            let min_l = if nnz < rows {
                a.min_s.min(0.0)
            } else {
                a.min_s
            };
            let max_l = if nnz < rows {
                a.max_s.max(0.0)
            } else {
                a.max_s
            };

            Some(NumericStats {
                min: Some(min_l),
                max: Some(max_l),
                mean: Some(mean),
                median: None,
                range: Some(max_l - min_l),
                iqr: None,
                stdev: Some(stdev),
            })
        })
        .collect()
}

fn mtx_shape_only_common(rows: usize, cols: usize) -> ColumnarCommonFields {
    ColumnarCommonFields {
        row_count: rows,
        column_count: cols,
        encoding: Some(StructuredEncoding::MATRIX_MARKET.to_string()),
        ..ColumnarCommonFields::default()
    }
}

fn column_common_dense<T: Copy + Default + Sync>(
    values: &[T],
    rows: usize,
    cols: usize,
    config: &RuntimeConfig,
    to_f64: impl Fn(T) -> f64 + Copy + Sync + Send,
) -> ColumnarCommonFields {
    if rows == 0 || cols == 0 {
        return mtx_shape_only_common(rows, cols);
    }
    let inf = default_mtx_inference(cols);
    let numeric_stats = full_numeric_dense(values, rows, cols, config, to_f64);
    mtx_columnar_common(rows, cols, inf, numeric_stats)
}

fn column_common_sparse<T: Copy + Sync>(
    dims: [usize; 2],
    indices: &[[usize; 2]],
    values: &[T],
    config: &RuntimeConfig,
    to_f64: impl Fn(T) -> f64 + Copy + Sync + Send,
) -> ColumnarCommonFields {
    let rows = dims[0];
    let cols = dims[1];
    if rows == 0 || cols == 0 {
        return mtx_shape_only_common(rows, cols);
    }
    let inf = default_mtx_inference(cols);

    let full_logical = rows.saturating_mul(cols) <= limits::MAX_MTX_TABULAR_CELLS;
    let numeric_stats = if full_logical {
        full_numeric_sparse_materialized(indices, values, rows, cols, config, to_f64)
    } else {
        full_numeric_sparse_logical_coo(indices, values, rows, cols, to_f64)
    };
    mtx_columnar_common(rows, cols, inf, numeric_stats)
}

fn mtx_metadata_dense(
    byte_count: usize,
    values_len: usize,
    sym: SymInfo,
    common: ColumnarCommonFields,
) -> MtxMetadata {
    MtxMetadata {
        byte_count,
        storage: Some("dense".to_string()),
        num_stored_values: Some(values_len),
        symmetry: Some(symmetry_str(sym).to_string()),
        parse_error: None,
        numeric_stats_include_implicit_zeros: Some(true),
        common,
    }
}

fn mtx_metadata_sparse(
    byte_count: usize,
    values_len: usize,
    sym: SymInfo,
    common: ColumnarCommonFields,
) -> MtxMetadata {
    MtxMetadata {
        byte_count,
        storage: Some("sparse".to_string()),
        num_stored_values: Some(values_len),
        symmetry: Some(symmetry_str(sym).to_string()),
        parse_error: None,
        // Dense path and sparse (materialized or COO moments) all treat the matrix as a full `rows × cols` table.
        numeric_stats_include_implicit_zeros: Some(true),
        common,
    }
}

fn from_f64(stats: &ParseResult, data: MtxData<f64>, config: &RuntimeConfig) -> MtxMetadata {
    let byte_count = stats.byte_count;
    match data {
        MtxData::Dense(dims, values, sym) => mtx_metadata_dense(
            byte_count,
            values.len(),
            sym,
            column_common_dense(&values, dims[0], dims[1], config, |v| v),
        ),
        MtxData::Sparse(dims, indices, values, sym) => {
            let common = column_common_sparse(dims, &indices, &values, config, |v| v);
            mtx_metadata_sparse(byte_count, values.len(), sym, common)
        }
    }
}

fn from_i32(stats: &ParseResult, data: MtxData<i32>, config: &RuntimeConfig) -> MtxMetadata {
    let byte_count = stats.byte_count;
    match data {
        MtxData::Dense(dims, values, sym) => mtx_metadata_dense(
            byte_count,
            values.len(),
            sym,
            column_common_dense(&values, dims[0], dims[1], config, f64::from),
        ),
        MtxData::Sparse(dims, indices, values, sym) => {
            let common = column_common_sparse(dims, &indices, &values, config, f64::from);
            mtx_metadata_sparse(byte_count, values.len(), sym, common)
        }
    }
}

fn mtx_err_string(e: &MtxError) -> String {
    match e {
        MtxError::IoError(io) => format!("{io}"),
        MtxError::EarlyEOF => "unexpected end of file".to_string(),
        MtxError::EarlyBannerEnd => "invalid Matrix Market banner".to_string(),
        MtxError::EarlySizesHeaderEnd => "invalid size line".to_string(),
        MtxError::UnsupportedSym(s) => format!("unsupported symmetry: {s}"),
        MtxError::UnsupportedNumType(s) => format!("unsupported numeric type: {s}"),
        MtxError::UnsupportedLayout(s) => format!("unsupported layout: {s}"),
        MtxError::InvalidNum(s) => format!("invalid number: {s}"),
        MtxError::InvalidCoordinate(e) => format!("invalid coordinate: {e}"),
    }
}

/// Parse Matrix Market text at [`ParseResult::file_path`] and summarize dimensions, symmetry, and column metadata.
///
/// # Errors
///
/// Returns an error when the file cannot be read or does not match the supported Matrix Market subset.
pub fn extract_mtx_metadata(
    _mmap: &Mmap,
    stats: &ParseResult,
    config: &RuntimeConfig,
) -> Result<MtxMetadata> {
    let path = stats.file_path.as_str();
    match MtxData::<f64>::from_file(path) {
        Ok(data) => Ok(from_f64(stats, data, config)),
        Err(e_f) => match MtxData::<i32>::from_file(path) {
            Ok(data) => Ok(from_i32(stats, data, config)),
            Err(e_i) => Err(anyhow::anyhow!(
                "Matrix Market parse failed (f64): {}; (i32): {}",
                mtx_err_string(&e_f),
                mtx_err_string(&e_i)
            )),
        },
    }
}

crate::no_template_mining!(
    extract_mtx_templates,
    "Matrix Market is numeric; no text template mining."
);
