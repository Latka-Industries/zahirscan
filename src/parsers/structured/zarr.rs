//! `Zarr` directory store — `zarrs` + `zarrs_filesystem`, V2 and V3 hierarchy walk, per-array stats (like `.npz` members).

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use rayon::prelude::*;
use zarrs::array::ElementOwned;
use zarrs::array::{Array, ArraySubset, FromArrayBytes};
use zarrs::group::Group;
use zarrs::metadata::ArrayMetadata as ZarrArrayMeta;
use zarrs::metadata::GroupMetadata;
use zarrs::metadata::NodeMetadata;
use zarrs_filesystem::FilesystemStore;

use crate::config::RuntimeConfig;
use crate::parsers::{
    ParseResult,
    structured::{numpy, tensor3d::tensor3d_plane_stats_for_npy_bytes},
};
use crate::results::{
    ArrayLayoutSummary, ColumnarCommonFields, ZarrArrayEntrySummary, ZarrMetadata,
};

const MAX_ZARR_ARRAYS: usize = 128;

fn zarr_array_entry_error(name: String, e: impl std::fmt::Debug) -> ZarrArrayEntrySummary {
    ZarrArrayEntrySummary {
        name,
        entry_parse_error: Some(format!("{:#?}", e)),
        ..Default::default()
    }
}

fn group_root_label(m: &GroupMetadata) -> &'static str {
    match m {
        GroupMetadata::V2(_) => "2",
        GroupMetadata::V3(_) => "3",
    }
}

fn array_version_label(m: &ZarrArrayMeta) -> &'static str {
    match m {
        ZarrArrayMeta::V2(_) => "2",
        ZarrArrayMeta::V3(_) => "3",
    }
}

/// Map a [`zarrs::array::DataType`]-style label to a `NumPy` `descr` for [`column_common_from_npy_bytes`].
fn dtype_to_numpy_descr_for_stats(s: &str) -> Option<&'static str> {
    if s.contains("float64") || s.contains("<f8") {
        return Some("<f8");
    }
    if s.contains("float32") || s.contains("<f4") {
        return Some("<f4");
    }
    if s.contains("int64") && !s.contains("uint") {
        return Some("<i8");
    }
    if s.contains("int32") {
        return Some("<i4");
    }
    if s.contains("int16") {
        return Some("<i2");
    }
    if s.contains("uint8") {
        return Some("<u1");
    }
    if s.contains("int8") {
        return Some("<i1");
    }
    if s.contains("bool") {
        return Some("?");
    }
    None
}

fn sample_subset_for_array(sh: &[u64], max_rows: usize) -> ArraySubset {
    if sh.is_empty() {
        return ArraySubset::new_with_shape(sh.to_vec());
    }
    let cap_rows = max_rows.saturating_mul(2).clamp(1, 8_192);
    let mut n = sh.to_vec();
    n[0] = n[0].min(cap_rows as u64);
    if n.len() >= 2 {
        n[1] = n[1].min(4_096);
    }
    if n.len() >= 3 {
        n[2] = n[2].min(256);
    }
    for d in n.iter_mut().skip(3) {
        *d = (*d).min(32);
    }
    ArraySubset::new_with_start_shape(vec![0u64; n.len()], n)
        .expect("ArraySubset start and shape have same len")
}

fn f64_to_le(v: &[f64]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}
fn f32_to_le(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}
fn i32_to_le(v: &[i32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}
fn i64_to_le(v: &[i64]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn fill_expected_bytes(layout: &mut ArrayLayoutSummary, dtype: &str) {
    if let (Some(s), Some(e)) = (
        layout.shape.as_deref(),
        numpy::numpy_descr_element_nbytes(dtype),
    ) && let Some(n) = e.checked_mul(s.iter().product::<usize>())
    {
        layout.expected_data_bytes_from_dtype = Some(n);
    }
}

fn stats_from_typed_subset<T: ElementOwned + Copy + 'static>(
    array: &Array<FilesystemStore>,
    subset: &ArraySubset,
    layout: &ArrayLayoutSummary,
    numpy: &'static str,
    to_bytes: fn(&[T]) -> Vec<u8>,
    config: &RuntimeConfig,
) -> (
    ColumnarCommonFields,
    Option<crate::results::Tensor3DPlaneStats>,
)
where
    Vec<T>: FromArrayBytes,
{
    if numpy::numpy_descr_skips_tabular_stats(numpy) {
        return (numpy::column_shape_only_from_layout(layout), None);
    }
    let v: std::result::Result<Vec<T>, _> = array.retrieve_array_subset(subset);
    let Ok(v) = v else {
        return (numpy::column_shape_only_from_layout(layout), None);
    };
    let buf = to_bytes(&v);
    let mut l = layout.clone();
    l.dtype = Some(numpy.to_string());
    l.data_offset = Some(0);
    l.data_region_bytes = Some(buf.len());
    l.fortran_order = Some(false);
    if l.shape.as_ref().map_or(0, |s| s.len()) == 3 {
        let t3 = tensor3d_plane_stats_for_npy_bytes(&buf, &l);
        let c = if numpy::numpy_descr_element_nbytes(numpy).is_some() {
            numpy::column_common_from_npy_bytes(&buf, &l, config)
        } else {
            numpy::column_shape_only_from_layout(&l)
        };
        return (c, t3);
    }
    (numpy::column_common_from_npy_bytes(&buf, &l, config), None)
}

fn summarize_array(
    array: &Array<FilesystemStore>,
    name: &str,
    config: &RuntimeConfig,
) -> ZarrArrayEntrySummary {
    let am = array.metadata();
    let vlabel = array_version_label(am);
    let sh = array.shape();
    let shape: Vec<usize> = sh.iter().map(|u| *u as usize).collect();
    let dtype_s = array.data_type().to_string();
    let num_chunks: Option<usize> = Some(
        array
            .chunk_grid_shape()
            .iter()
            .map(|&x| (x as usize).max(1))
            .product(),
    );

    let dtype_key = dtype_to_numpy_descr_for_stats(&dtype_s);

    let mut layout = ArrayLayoutSummary {
        format_version: Some(format!("Zarr array metadata v{vlabel}")),
        dtype: Some(dtype_s),
        shape: Some(shape),
        fortran_order: Some(false),
        ..Default::default()
    };

    if let Some(npy) = dtype_key {
        fill_expected_bytes(&mut layout, npy);
    }

    let subset = sample_subset_for_array(sh, config.max_tabular_sample_rows);

    let (common, tensor3d) = if let Some(npy) = dtype_key {
        if npy == "<f8" {
            stats_from_typed_subset::<f64>(array, &subset, &layout, npy, f64_to_le, config)
        } else if npy == "<f4" {
            stats_from_typed_subset::<f32>(array, &subset, &layout, npy, f32_to_le, config)
        } else if npy == "<i4" {
            stats_from_typed_subset::<i32>(array, &subset, &layout, npy, i32_to_le, config)
        } else if npy == "<i8" {
            stats_from_typed_subset::<i64>(array, &subset, &layout, npy, i64_to_le, config)
        } else {
            (numpy::column_shape_only_from_layout(&layout), None)
        }
    } else {
        (numpy::column_shape_only_from_layout(&layout), None)
    };

    ZarrArrayEntrySummary {
        name: name.to_string(),
        zarr_array_metadata: Some(vlabel.to_string()),
        layout,
        common,
        tensor3d,
        num_chunks,
        entry_parse_error: None,
    }
}

type St = FilesystemStore;

/// # Errors
///
/// Returns I/O, Zarr store, or walk errors.
pub fn extract_zarr_metadata(
    store_path: &str,
    stats: &ParseResult,
    config: &RuntimeConfig,
) -> Result<ZarrMetadata> {
    let path = Path::new(store_path);
    let store: Arc<FilesystemStore> = Arc::new(
        FilesystemStore::new(path)
            .with_context(|| format!("open Zarr FilesystemStore at {store_path}"))?,
    );

    if let Ok(group) = Group::<St>::open(store.clone(), "/") {
        let root_label = group_root_label(group.metadata()).to_string();
        let nodes = group.traverse().context("traverse Zarr hierarchy")?;
        let mut array_paths: Vec<String> = Vec::new();
        for (node_path, node_meta) in nodes {
            if array_paths.len() >= MAX_ZARR_ARRAYS {
                break;
            }
            if !matches!(&node_meta, NodeMetadata::Array(_)) {
                continue;
            }
            array_paths.push(node_path.to_string());
        }
        let scanned = array_paths.len();
        let st = store.clone();
        let entries: Vec<ZarrArrayEntrySummary> = array_paths
            .par_iter()
            .map(|pstr| match Array::<St>::open(st.clone(), pstr.as_str()) {
                Ok(a) => summarize_array(&a, pstr, config),
                Err(e) => zarr_array_entry_error(pstr.clone(), &e),
            })
            .collect();
        return Ok(ZarrMetadata {
            byte_count: stats.byte_count,
            root_group_metadata: Some(root_label),
            array_entries_scanned: Some(scanned),
            array_entries: Some(entries),
        });
    }

    // Root may be a single array.
    let array = match Array::<St>::open(store, "/") {
        Ok(a) => a,
        Err(e) => {
            return Err(anyhow::anyhow!(e)
                .context("Zarr open: not a group at `/` and not a single array at `/`"));
        }
    };
    let one = summarize_array(&array, "/", config);
    Ok(ZarrMetadata {
        byte_count: stats.byte_count,
        root_group_metadata: None,
        array_entries_scanned: Some(1),
        array_entries: Some(vec![one]),
    })
}

crate::no_template_mining!(
    extract_zarr_templates,
    "`Zarr` store: hierarchy + per-array layout and bounded stats; no template mining."
);
