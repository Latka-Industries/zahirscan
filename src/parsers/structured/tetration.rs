//! Tetration `.tet` — mmap-friendly chunked tensor container (layout v1).
//!
//! Catalog via [`tetration::catalog::read_tet_summary_v1`]; per-dataset numeric / column stats via
//! the Tetration query engine (streaming fold, same path as `tet query -x`).

use std::path::Path;

use memmap2::Mmap;
use rayon::prelude::*;
use tetration::catalog::metadata::TetMetadataV1;
use tetration::catalog::{
    CHUNK_PAYLOAD_CODEC_V1, DATASET_DTYPE_TAG_V1, DatasetRecordV1, read_tet_summary_v1,
};
use tetration::query::{ExecuteQueryOptions, QueryExecutionPreview, execute_query_json};

use crate::config::RuntimeConfig;
use crate::parsers::ParseResult;
use crate::parsers::structured::numpy::{self, numpy_descr_element_nbytes};
use crate::results::{
    ArrayLayoutSummary, ColumnStat, ColumnarCommonFields, NumericStats, Tensor3DGlobalStats,
    Tensor3DPlaneStats, TetDatasetSummary, TetrationMetadata,
};

const MAX_DATASETS_LISTED: usize = 10_000;
/// Cap datasets that run query-engine stats (each may issue several fold passes).
const MAX_DATASETS_STATS: usize = 32;
const QUERY_ENCODING: &str = "tetration-query";

fn dtype_label(tag: u32) -> String {
    let tags = DATASET_DTYPE_TAG_V1;
    if tags.is_f32(tag) {
        "f32".to_string()
    } else if tags.is_f64(tag) {
        "f64".to_string()
    } else if tags.is_i32(tag) {
        "i32".to_string()
    } else if tags.is_i64(tag) {
        "i64".to_string()
    } else if tags.is_u8(tag) {
        "u8".to_string()
    } else if tags.is_u16(tag) {
        "u16".to_string()
    } else if tags.is_i16(tag) {
        "i16".to_string()
    } else if tags.is_u32(tag) {
        "u32".to_string()
    } else if tags.is_f16(tag) {
        "f16".to_string()
    } else if tags.is_u64(tag) {
        "u64".to_string()
    } else {
        format!("wire:{tag}")
    }
}

fn tetration_dtype_to_numpy_descr(tag: u32) -> Option<&'static str> {
    let tags = DATASET_DTYPE_TAG_V1;
    if tags.is_f32(tag) || tags.is_f16(tag) {
        Some("<f4")
    } else if tags.is_f64(tag) {
        Some("<f8")
    } else if tags.is_i32(tag) || tags.is_u32(tag) {
        Some("<i4")
    } else if tags.is_i64(tag) || tags.is_u64(tag) {
        Some("<i8")
    } else if tags.is_i16(tag) || tags.is_u16(tag) {
        Some("<i2")
    } else if tags.is_u8(tag) {
        Some("<u1")
    } else {
        None
    }
}

fn chunk_counts_by_dataset(chunks: &[tetration::catalog::ChunkIndexEntryV1], n: usize) -> Vec<u64> {
    let mut counts = vec![0u64; n];
    for entry in chunks {
        let id = entry.dataset_id as usize;
        if id < n {
            counts[id] = counts[id].saturating_add(1);
        }
    }
    counts
}

fn layout_from_record(ds: &DatasetRecordV1) -> ArrayLayoutSummary {
    let shape: Vec<usize> = ds.shape.iter().map(|&d| d as usize).collect();
    let dtype = Some(dtype_label(ds.dtype));
    let mut layout = ArrayLayoutSummary {
        format_version: Some("tetration-v1".into()),
        dtype: dtype.clone(),
        shape: Some(shape.clone()),
        fortran_order: Some(false),
        ..Default::default()
    };
    if let Some(descr) = tetration_dtype_to_numpy_descr(ds.dtype) {
        let elems: Option<usize> = shape.iter().try_fold(1usize, |a, &d| a.checked_mul(d));
        if let Some(n) =
            elems.and_then(|e| numpy_descr_element_nbytes(descr).and_then(|b| b.checked_mul(e)))
        {
            layout.expected_data_bytes_from_dtype = Some(n);
        }
    }
    layout
}

fn query_json(dataset: &str, op: &str, axes: &serde_json::Value) -> String {
    serde_json::json!({
        "dataset": dataset,
        op: axes,
    })
    .to_string()
}

fn run_query(
    mmap: &[u8],
    path: &Path,
    json: &str,
) -> Result<tetration::query::QueryResponse, String> {
    execute_query_json(
        json,
        path,
        mmap,
        ExecuteQueryOptions::execute_no_preview(),
        None,
    )
    .map_err(|e| e.to_string())
}

fn execution_preview(
    mmap: &[u8],
    path: &Path,
    json: &str,
) -> Result<QueryExecutionPreview, String> {
    run_query(mmap, path, json)?
        .execution
        .ok_or_else(|| "query accepted but execution block missing".to_string())
}

struct ReductionPreviews {
    mean: QueryExecutionPreview,
    min: QueryExecutionPreview,
    max: QueryExecutionPreview,
    std: QueryExecutionPreview,
}

fn fetch_reduction_previews(
    mmap: &[u8],
    path: &Path,
    dataset: &str,
    axes: &serde_json::Value,
) -> Result<ReductionPreviews, String> {
    Ok(ReductionPreviews {
        mean: execution_preview(mmap, path, &query_json(dataset, "mean", axes))?,
        min: execution_preview(mmap, path, &query_json(dataset, "min", axes))?,
        max: execution_preview(mmap, path, &query_json(dataset, "max", axes))?,
        std: execution_preview(mmap, path, &query_json(dataset, "std", axes))?,
    })
}

fn numeric_range(min: Option<f64>, max: Option<f64>) -> Option<f64> {
    match (min, max) {
        (Some(a), Some(b)) => Some(b - a),
        _ => None,
    }
}

fn numeric_stats_from_scalar_previews(p: &ReductionPreviews) -> NumericStats {
    NumericStats {
        min: p.min.operation_min,
        max: p.max.operation_max,
        mean: p.mean.operation_mean,
        median: p.mean.operation_median,
        range: numeric_range(p.min.operation_min, p.max.operation_max),
        stdev: p.std.operation_std,
        ..NumericStats::default()
    }
}

fn single_column_common(
    row_count: Option<usize>,
    name: Option<String>,
    dtype: &str,
    num: NumericStats,
) -> ColumnarCommonFields {
    ColumnarCommonFields {
        row_count,
        column_count: Some(1),
        encoding: Some(QUERY_ENCODING.into()),
        columns: Some(vec![column_stat_number(0, name, dtype, num)]),
        ..Default::default()
    }
}

fn column_names_for_table(
    meta: &TetMetadataV1,
    dataset: &str,
    rank: usize,
    shape: &[u64],
) -> Option<Vec<String>> {
    let ncol = match rank {
        1 => shape.first().copied().unwrap_or(0) as usize,
        2 => shape.get(1).copied().unwrap_or(0) as usize,
        _ => return None,
    };
    if ncol == 0 {
        return None;
    }
    let axis_label = meta
        .datasets
        .get(dataset)
        .and_then(|dm| dm.dim_names.as_ref())
        .and_then(|names| names.get(rank.saturating_sub(1)).cloned());
    Some(
        (0..ncol)
            .map(|i| {
                axis_label
                    .as_ref()
                    .map_or_else(|| format!("col{i}"), |l| format!("{l}[{i}]"))
            })
            .collect(),
    )
}

fn column_stat_number(
    i: usize,
    name: Option<String>,
    dtype: &str,
    num: NumericStats,
) -> ColumnStat {
    ColumnStat {
        i,
        name,
        t: "number".to_string(),
        pt: Some(dtype.to_string()),
        num: Some(num),
        null_pct: None,
        uniq: None,
        bool_stats: None,
        date: None,
    }
}

fn columns_from_axis0(
    mins: Option<&Vec<f64>>,
    maxs: Option<&Vec<f64>>,
    means: Option<&Vec<f64>>,
    stds: Option<&Vec<f64>>,
    dtype: &str,
    names: Option<&Vec<String>>,
) -> Option<Vec<ColumnStat>> {
    let n = mins.or(maxs).or(means).or(stds)?.len();
    if n == 0 {
        return None;
    }
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let min = mins.and_then(|v| v.get(i).copied());
        let max = maxs.and_then(|v| v.get(i).copied());
        out.push(column_stat_number(
            i,
            names.and_then(|v| v.get(i).cloned()),
            dtype,
            NumericStats {
                min,
                max,
                mean: means.and_then(|v| v.get(i).copied()),
                range: numeric_range(min, max),
                stdev: stds.and_then(|v| v.get(i).copied()),
                ..NumericStats::default()
            },
        ));
    }
    Some(out)
}

fn fetch_scalar_stats(mmap: &[u8], path: &Path, dataset: &str) -> Result<NumericStats, String> {
    Ok(numeric_stats_from_scalar_previews(
        &fetch_reduction_previews(mmap, path, dataset, &serde_json::json!([]))?,
    ))
}

fn fetch_axis0_column_stats(
    mmap: &[u8],
    path: &Path,
    dataset: &str,
    dtype: &str,
    names: Option<&Vec<String>>,
) -> Result<ColumnarCommonFields, String> {
    let p = fetch_reduction_previews(mmap, path, dataset, &serde_json::json!(0))?;
    let cols = columns_from_axis0(
        p.min.operation_reduced_min.as_ref(),
        p.max.operation_reduced_max.as_ref(),
        p.mean.operation_reduced_mean.as_ref(),
        p.std.operation_reduced_std.as_ref(),
        dtype,
        names,
    );
    Ok(ColumnarCommonFields {
        column_count: cols.as_ref().map(Vec::len),
        encoding: Some(QUERY_ENCODING.into()),
        columns: cols,
        ..Default::default()
    })
}

fn global_tensor3d_from_scalar_queries(
    mmap: &[u8],
    path: &Path,
    dataset: &str,
    shape: &[usize],
) -> Result<Tensor3DPlaneStats, String> {
    let p = fetch_reduction_previews(mmap, path, dataset, &serde_json::json!([]))?;
    let along_axis = shape
        .iter()
        .enumerate()
        .min_by_key(|(_, d)| *d)
        .map_or(0, |(i, _)| i as u8);
    let n_pos = p
        .mean
        .operation_element_count
        .or(p.min.operation_element_count)
        .unwrap_or(0);
    Ok(Tensor3DPlaneStats {
        along_axis,
        elements_sampled: n_pos,
        global: Some(Tensor3DGlobalStats {
            n_pos,
            n_nan: p.mean.operation_nan_count.map_or(0, |n| n.round() as usize),
            n_inf: p.mean.operation_inf_count.map_or(0, |n| n.round() as usize),
            min: p.min.operation_min.unwrap_or(0.0),
            max: p.max.operation_max.unwrap_or(0.0),
            mean: p.mean.operation_mean.unwrap_or(0.0),
            stdev: p.std.operation_std,
        }),
        planes: Vec::new(),
    })
}

fn enrich_dataset_with_queries(
    entry: &mut TetDatasetSummary,
    ds: &DatasetRecordV1,
    file_meta: &TetMetadataV1,
    mmap: &[u8],
    path: &Path,
) {
    let layout = layout_from_record(ds);
    entry.layout = layout.clone();
    let dtype = entry.dtype.as_deref().unwrap_or("unknown");
    let rank = ds.shape.len();
    let shape_usize: Vec<usize> = ds.shape.iter().map(|&d| d as usize).collect();

    let result = (|| -> Result<(), String> {
        match rank {
            0 => {
                entry.common = single_column_common(
                    None,
                    None,
                    dtype,
                    fetch_scalar_stats(mmap, path, &ds.name)?,
                );
            }
            1 => {
                let col_name = file_meta
                    .datasets
                    .get(&ds.name)
                    .and_then(|dm| dm.dim_names.as_ref())
                    .and_then(|names| names.first().cloned());
                entry.common = single_column_common(
                    Some(shape_usize[0]),
                    col_name,
                    dtype,
                    fetch_scalar_stats(mmap, path, &ds.name)?,
                );
            }
            2 => {
                let rows = shape_usize.first().copied().unwrap_or(0);
                let names = column_names_for_table(file_meta, &ds.name, 2, &ds.shape);
                let common = fetch_axis0_column_stats(mmap, path, &ds.name, dtype, names.as_ref())?;
                entry.common = ColumnarCommonFields {
                    row_count: Some(rows),
                    ..common
                };
            }
            _ => {
                let t3 = global_tensor3d_from_scalar_queries(mmap, path, &ds.name, &shape_usize)?;
                let num = t3.global.as_ref().map(|g| NumericStats {
                    min: Some(g.min),
                    max: Some(g.max),
                    mean: Some(g.mean),
                    range: Some(g.max - g.min),
                    stdev: g.stdev,
                    ..NumericStats::default()
                });
                entry.tensor3d = Some(t3);
                entry.common = ColumnarCommonFields {
                    row_count: Some(shape_usize.iter().product()),
                    column_count: Some(0),
                    encoding: Some(QUERY_ENCODING.into()),
                    columns: num.map(|n| vec![column_stat_number(0, None, dtype, n)]),
                    ..Default::default()
                };
            }
        }
        Ok(())
    })();

    if let Err(e) = result {
        entry.query_error = Some(e);
        if numpy::numpy_descr_skips_tabular_stats(
            tetration_dtype_to_numpy_descr(ds.dtype).unwrap_or(""),
        ) {
            entry.common = numpy::column_shape_only_from_layout(&layout);
        } else if let Some((r, c)) = layout.shape_row_col_counts() {
            entry.common = ColumnarCommonFields {
                row_count: Some(r),
                column_count: Some(c),
                ..Default::default()
            };
        }
    }
}

/// Extract Tetration layout v1 catalog summary from a memory-mapped `.tet` file.
///
/// # Errors
///
/// Returns an error when the mapped bytes are not a valid layout-v1 Tetration file.
pub fn extract_tetration_metadata(
    mmap: &Mmap,
    stats: &ParseResult,
    _config: &RuntimeConfig,
) -> anyhow::Result<TetrationMetadata> {
    let path = Path::new(&stats.file_path);
    match read_tet_summary_v1(mmap) {
        Ok(summary) => {
            let per_ds = chunk_counts_by_dataset(&summary.chunks, summary.datasets.len());
            let mut raw_chunk_count = 0usize;
            let mut zstd_chunk_count = 0usize;
            for entry in &summary.chunks {
                if CHUNK_PAYLOAD_CODEC_V1.is_raw(entry.codec) {
                    raw_chunk_count += 1;
                } else if CHUNK_PAYLOAD_CODEC_V1.is_zstd(entry.codec) {
                    zstd_chunk_count += 1;
                }
            }

            let datasets_stats_skipped = summary
                .datasets
                .len()
                .saturating_sub(MAX_DATASETS_STATS.min(MAX_DATASETS_LISTED));

            let datasets: Vec<TetDatasetSummary> = summary
                .datasets
                .par_iter()
                .enumerate()
                .take(MAX_DATASETS_LISTED)
                .map(|(i, ds)| {
                    let mut entry = TetDatasetSummary {
                        name: ds.name.clone(),
                        dtype: Some(dtype_label(ds.dtype)),
                        shape: ds.shape.clone(),
                        chunk_shape: ds.chunk_shape.clone(),
                        chunk_count: Some(per_ds.get(i).copied().unwrap_or(0)),
                        ..Default::default()
                    };
                    if i < MAX_DATASETS_STATS {
                        enrich_dataset_with_queries(&mut entry, ds, &summary.metadata, mmap, path);
                    } else {
                        entry.layout = layout_from_record(ds);
                    }
                    entry
                })
                .collect();

            Ok(TetrationMetadata {
                byte_count: stats.byte_count,
                layout_version: Some(summary.superblock.layout_version),
                dataset_count: Some(summary.superblock.dataset_count),
                chunk_count: Some(summary.chunks.len()),
                raw_chunk_count: Some(raw_chunk_count),
                zstd_chunk_count: Some(zstd_chunk_count),
                history_event_count: Some(summary.history.len()),
                datasets: Some(datasets),
                datasets_stats_skipped: (datasets_stats_skipped > 0)
                    .then_some(datasets_stats_skipped),
                parse_error: None,
            })
        }
        Err(e) => {
            log::debug!(
                "Tetration catalog read failed for '{}': {e}",
                stats.file_path
            );
            Ok(TetrationMetadata {
                byte_count: stats.byte_count,
                parse_error: Some(e.to_string()),
                ..Default::default()
            })
        }
    }
}

crate::no_template_mining!(
    extract_tetration_templates,
    "Tetration .tet is a binary tensor container; no text template mining."
);
