//! MATLAB `.mat` (classic v7) — variable listing, [`ArrayLayoutSummary`] per entry, bounded column stats for dense real numeric arrays.

use std::borrow::Cow;

use anyhow::Result;
use matrw::{MatVariable, MatlabType, MatrwError, OwnedIndex, load_matfile_from_u8};
use memmap2::Mmap;

use crate::config::RuntimeConfig;
use crate::parsers::ParseResult;
use crate::parsers::structured::{
    columnar::utils as columnar_utils, numpy::tabular_sample_rows_for_dense_array,
    tensor3d::tensor3d_plane_stats_for_mat_colmaj,
};
use crate::results::Tensor3DPlaneStats;
use crate::results::{ArrayLayoutSummary, ColumnarCommonFields, MatArrayEntrySummary, MatMetadata};

const MAX_MAT_VARIABLES: usize = 128;
/// First 116 bytes of a MAT 5.x file are the human-readable description (see MATLAB MAT-file spec).
const MAT5_HEADER_DESC_BYTES: usize = 116;
/// HDF5 superblock signature. MAT v7.3 (MATLAB or `SciPy` `format='7.3'`) is HDF5 and does not start
/// with the v5/v7 text header, so `matrw` fails on `matfile_ver` unless we detect this first.
const HDF5_FILE_SIGNATURE: &[u8] = &[0x89, b'H', b'D', b'F', b'\r', b'\n', 0x1a, b'\n'];
/// `SciPy` [`savemat`](https://docs.scipy.org/doc/scipy/reference/generated/scipy.io.savemat.html) writes
/// `MATLAB 5.0 MAT-file Platform:...` (no comma after `MAT-file`). Official MATLAB and `matrw` use
/// `MATLAB 5.0 MAT-file, Platform:...`. Without this fix, `matrw` rejects the header (`matfile_ver`).
const SCIPY_MAT5_DESCRIPTION_PREFIX: &[u8] = b"MATLAB 5.0 MAT-file Platform:";

/// Return a copy of `data` with the `SciPy` MAT-5 description prefix adjusted so `matrw` recognizes it.
fn normalize_scipy_mat5_header_for_matrw(data: &[u8]) -> Cow<'_, [u8]> {
    if data.len() < 128 {
        return Cow::Borrowed(data);
    }
    let desc = &data[..MAT5_HEADER_DESC_BYTES];
    if !desc.starts_with(SCIPY_MAT5_DESCRIPTION_PREFIX) {
        return Cow::Borrowed(data);
    }
    // Insert `,` after `MAT-file` at index 19: shift `desc[19..115]` → `desc[20..116]`, drop last padding byte.
    let mut buf = data.to_vec();
    for i in (20..116).rev() {
        buf[i] = buf[i - 1];
    }
    buf[19] = b',';
    Cow::Owned(buf)
}

fn mat_metadata_v73_hdf5_unsupported(stats: &ParseResult) -> MatMetadata {
    MatMetadata {
        byte_count: stats.byte_count,
        mat_format: Some("v7.3".to_string()),
        variable_count: None,
        variables_scanned: None,
        file_parse_error: Some(
            "MAT v7.3 (HDF5) is not parsed here; open as HDF5 or re-save as -v7 in MATLAB or SciPy (format='5')"
                .to_string(),
        ),
        entries: None,
    }
}

fn peel_compressed(mut v: &MatVariable) -> &MatVariable {
    while let MatVariable::Compressed(c) = v {
        v = c.value.as_ref();
    }
    v
}

fn matlab_storage_label(mt: &MatlabType) -> String {
    match mt {
        MatlabType::U8(_) => "uint8",
        MatlabType::I8(_) => "int8",
        MatlabType::U16(_) => "uint16",
        MatlabType::I16(_) => "int16",
        MatlabType::U32(_) => "uint32",
        MatlabType::I32(_) => "int32",
        MatlabType::U64(_) => "uint64",
        MatlabType::I64(_) => "int64",
        MatlabType::F32(_) => "single",
        MatlabType::F64(_) => "double",
        MatlabType::UTF8(_) | MatlabType::UTF16(_) => "char",
        MatlabType::BOOL(_) => "logical",
    }
    .to_string()
}

fn matlab_elem_size(mt: &MatlabType) -> Option<usize> {
    match mt {
        MatlabType::U8(_) | MatlabType::I8(_) | MatlabType::BOOL(_) => Some(1),
        MatlabType::U16(_) | MatlabType::I16(_) => Some(2),
        MatlabType::U32(_) | MatlabType::I32(_) | MatlabType::F32(_) => Some(4),
        MatlabType::U64(_) | MatlabType::I64(_) | MatlabType::F64(_) => Some(8),
        MatlabType::UTF8(_) | MatlabType::UTF16(_) => None,
    }
}

fn shape_num_elements(shape: &[usize]) -> usize {
    shape.iter().product()
}

fn array_layout_for_mat_variable(var: &MatVariable) -> ArrayLayoutSummary {
    let v = peel_compressed(var);
    match v {
        MatVariable::NumericArray(n) => {
            let shape = if n.dim.is_empty() {
                None
            } else {
                Some(n.dim.clone())
            };
            let mut dtype = matlab_storage_label(&n.value);
            if n.value_cmp.is_some() {
                dtype.push_str(" complex");
            }
            let expected_data_bytes_from_dtype = shape.as_ref().and_then(|sh| {
                matlab_elem_size(&n.value).and_then(|el| el.checked_mul(shape_num_elements(sh)))
            });
            ArrayLayoutSummary {
                dtype: Some(dtype),
                shape,
                fortran_order: match n.dim.len() {
                    0 | 1 => None,
                    _ => Some(true),
                },
                header_region_bytes: None,
                data_offset: None,
                data_region_bytes: None,
                expected_data_bytes_from_dtype,
                ..Default::default()
            }
        }
        MatVariable::SparseArray(s) => {
            let shape = Some(s.dim.clone());
            let mut dtype = matlab_storage_label(s.numeric_type());
            if s.value_cmp.is_some() {
                dtype.push_str(" complex");
            }
            ArrayLayoutSummary {
                dtype: Some(dtype),
                shape,
                fortran_order: Some(true),
                header_region_bytes: None,
                data_offset: None,
                data_region_bytes: None,
                expected_data_bytes_from_dtype: None,
                ..Default::default()
            }
        }
        MatVariable::StructureArray(_) | MatVariable::Structure(_) => ArrayLayoutSummary {
            dtype: Some("struct".to_string()),
            shape: Some(v.dim()),
            fortran_order: None,
            header_region_bytes: None,
            data_offset: None,
            data_region_bytes: None,
            expected_data_bytes_from_dtype: None,
            ..Default::default()
        },
        MatVariable::CellArray(_) => ArrayLayoutSummary {
            dtype: Some("cell".to_string()),
            shape: Some(v.dim()),
            fortran_order: None,
            header_region_bytes: None,
            data_offset: None,
            data_region_bytes: None,
            expected_data_bytes_from_dtype: None,
            ..Default::default()
        },
        MatVariable::Unsupported => ArrayLayoutSummary {
            dtype: Some("unsupported".to_string()),
            ..Default::default()
        },
        MatVariable::Null | MatVariable::Compressed(_) => ArrayLayoutSummary::default(),
    }
}

fn tensor3d_for_mat_var(var: &MatVariable) -> Option<Tensor3DPlaneStats> {
    let v = peel_compressed(var);
    let MatVariable::NumericArray(n) = v else {
        return None;
    };
    if n.value_cmp.is_some() {
        return None;
    }
    if n.dim.len() != 3 {
        return None;
    }
    matlab_elem_size(&n.value)?;
    let d0 = n.dim[0];
    let d1 = n.dim[1];
    let d2 = n.dim[2];
    if d0 == 0 || d1 == 0 || d2 == 0 {
        return None;
    }
    tensor3d_plane_stats_for_mat_colmaj(&n.value, d0, d1, d2)
}

fn mat_skip_tabular_stats(var: &MatVariable) -> bool {
    let v = peel_compressed(var);
    match v {
        MatVariable::NumericArray(n) => {
            if n.value_cmp.is_some() {
                return true;
            }
            matches!(
                n.value,
                MatlabType::UTF8(_) | MatlabType::UTF16(_) | MatlabType::BOOL(_)
            )
        }
        _ => true,
    }
}

/// Stringify a 1×1 numeric sample cell.
///
/// `matrw`'s `MatVariable::to_f64` / `to_i32` / … call `MatlabType::get` with a fixed `T`; if `T`
/// does not match the stored variant, `matrw` panics (`unwrap` in `matlab_types.rs`). We branch on
/// [`MatlabType`] instead.
fn mat_scalar_to_string(cell: &MatVariable) -> String {
    let v = peel_compressed(cell);
    let MatVariable::NumericArray(n) = v else {
        return String::new();
    };
    if !n.is_scalar() {
        return String::new();
    }
    match &n.value {
        MatlabType::F64(v) => v.first().map_or(String::new(), |&x| {
            if x.is_nan() {
                String::new()
            } else {
                format!("{x:.15}")
            }
        }),
        MatlabType::F32(v) => v.first().map_or(String::new(), |&x| {
            if x.is_nan() {
                String::new()
            } else {
                format!("{x:.15}")
            }
        }),
        MatlabType::I64(v) => v.first().map_or(String::new(), |x| format!("{x}")),
        MatlabType::I32(v) => v.first().map_or(String::new(), |x| format!("{x}")),
        MatlabType::I16(v) => v.first().map_or(String::new(), |x| format!("{x}")),
        MatlabType::I8(v) => v.first().map_or(String::new(), |x| format!("{x}")),
        MatlabType::U64(v) => v.first().map_or(String::new(), |x| format!("{x}")),
        MatlabType::U32(v) => v.first().map_or(String::new(), |x| format!("{x}")),
        MatlabType::U16(v) => v.first().map_or(String::new(), |x| format!("{x}")),
        MatlabType::U8(v) => v.first().map_or(String::new(), |x| format!("{x}")),
        MatlabType::BOOL(v) => v.first().map_or(String::new(), |x| format!("{x}")),
        MatlabType::UTF8(v) => v.first().map_or(String::new(), |x| format!("{x}")),
        MatlabType::UTF16(v) => v.first().map_or(String::new(), |x| format!("{x}")),
    }
}

fn shape_only_matlab(layout: &ArrayLayoutSummary) -> ColumnarCommonFields {
    let Some((row_count, column_count)) = layout.shape_row_col_counts() else {
        return ColumnarCommonFields::default();
    };
    ColumnarCommonFields {
        row_count,
        column_count,
        ..ColumnarCommonFields::default()
    }
}

fn contiguous_payload_prefix_elems(
    rows: usize,
    cols: usize,
    rank: usize,
    fortran_order: bool,
    sample_rows: usize,
) -> usize {
    let sr = sample_rows.min(rows);
    if sr == 0 {
        return 0;
    }
    match rank {
        0 | 1 => sr.saturating_mul(1),
        2 if !fortran_order => sr.saturating_mul(cols),
        2 => {
            let r = sr - 1;
            let c = cols.saturating_sub(1);
            (r + c * rows).saturating_add(1)
        }
        _ => 0,
    }
}

fn max_sample_rows_in_prefix(
    rows: usize,
    cols: usize,
    rank: usize,
    fortran: bool,
    elem_size: usize,
    avail_elems: usize,
    max_csv: usize,
) -> usize {
    let hi = rows.min(max_csv);
    if hi == 0 || elem_size == 0 || avail_elems < elem_size {
        return 0;
    }
    let mut lo = 0usize;
    let mut hi2 = hi;
    while lo < hi2 {
        let mid = (lo + hi2).div_ceil(2);
        let elems = contiguous_payload_prefix_elems(rows, cols, rank, fortran, mid);
        if elems <= avail_elems {
            lo = mid;
        } else {
            hi2 = mid - 1;
        }
    }
    lo
}

fn column_common_from_mat_numeric(
    var: &MatVariable,
    layout: &ArrayLayoutSummary,
    file_len_bytes: u64,
    config: &RuntimeConfig,
) -> ColumnarCommonFields {
    let v = peel_compressed(var);
    let MatVariable::NumericArray(n) = v else {
        return shape_only_matlab(layout);
    };
    if mat_skip_tabular_stats(var) {
        return shape_only_matlab(layout);
    }
    let Some(shape) = layout.shape.as_ref() else {
        return ColumnarCommonFields::default();
    };
    let Some(elem_size) = matlab_elem_size(&n.value) else {
        return shape_only_matlab(layout);
    };
    let Some((rows, cols)) = layout.table_dims() else {
        return shape_only_matlab(layout);
    };
    if rows == 0 || cols == 0 {
        return shape_only_matlab(layout);
    }

    let rank = shape.len();
    let fortran = layout.fortran_order.unwrap_or(false);
    let total_elems = shape_num_elements(shape);
    let cap = tabular_sample_rows_for_dense_array(file_len_bytes, cols.max(1), rows, config);
    let avail_elems = total_elems;
    let sample_rows =
        max_sample_rows_in_prefix(rows, cols, rank, fortran, elem_size, avail_elems, cap);
    if sample_rows == 0 {
        return shape_only_matlab(layout);
    }

    let mut sample_data: Vec<Vec<String>> = Vec::with_capacity(sample_rows);
    let index_var = peel_compressed(var);

    for r in 0..sample_rows {
        let mut row = Vec::with_capacity(cols);
        for c in 0..cols {
            let cell = if rank <= 1 {
                index_var.elem(r)
            } else {
                index_var.elem([r, c])
            };
            row.push(mat_scalar_to_string(&cell));
        }
        sample_data.push(row);
    }

    let ts = columnar_utils::tabular_stats_from_sample(&sample_data, cols, config);
    let names: Vec<String> = (0..cols).map(|i| format!("col{i}")).collect();
    let columns = columnar_utils::columns_from_tabular_sample(cols, Some(names), ts, None);

    ColumnarCommonFields {
        row_count: rows,
        column_count: cols,
        stats_rows_sampled: Some(sample_data.len()),
        columns,
        ..ColumnarCommonFields::default()
    }
}

fn column_common_for_mat_entry(
    var: &MatVariable,
    layout: &ArrayLayoutSummary,
    file_len_bytes: u64,
    config: &RuntimeConfig,
) -> ColumnarCommonFields {
    if mat_skip_tabular_stats(var) {
        return shape_only_matlab(layout);
    }
    let v = peel_compressed(var);
    match v {
        MatVariable::NumericArray(_) => {
            column_common_from_mat_numeric(var, layout, file_len_bytes, config)
        }
        _ => shape_only_matlab(layout),
    }
}

/// Extract `.mat` metadata: classic v7 variables via `matrw`; v7.3 HDF5 and other load failures return [`MatMetadata::file_parse_error`] instead of failing the extractor (avoids silent [`crate::results::MinimalFallback`] with only `byte_count`).
///
/// # Errors
///
/// Currently does not fail the extractor; load problems are reported in `file_parse_error`.
pub fn extract_mat_metadata(
    mmap: &Mmap,
    stats: &ParseResult,
    config: &RuntimeConfig,
) -> Result<MatMetadata> {
    let bytes: &[u8] = mmap.as_ref();
    if bytes.starts_with(HDF5_FILE_SIGNATURE) {
        return Ok(mat_metadata_v73_hdf5_unsupported(stats));
    }
    let normalized = normalize_scipy_mat5_header_for_matrw(bytes);

    let mat = match load_matfile_from_u8(normalized.as_ref()) {
        Ok(m) => m,
        Err(MatrwError::MatFile73Error) => {
            return Ok(mat_metadata_v73_hdf5_unsupported(stats));
        }
        Err(e) => {
            return Ok(MatMetadata {
                byte_count: stats.byte_count,
                mat_format: None,
                variable_count: None,
                variables_scanned: None,
                file_parse_error: Some(format!("{e:#}")),
                entries: None,
            });
        }
    };

    let variable_count = mat.iter().count();
    let mut variables_scanned = 0usize;
    let mut entries = Vec::new();

    for (name, var) in mat.iter() {
        if entries.len() >= MAX_MAT_VARIABLES {
            break;
        }
        variables_scanned = variables_scanned.saturating_add(1);
        let layout = array_layout_for_mat_variable(var);
        let common = column_common_for_mat_entry(var, &layout, stats.byte_count as u64, config);
        entries.push(MatArrayEntrySummary {
            name: name.clone(),
            layout,
            common,
            tensor3d: tensor3d_for_mat_var(var),
            entry_parse_error: None,
        });
    }

    Ok(MatMetadata {
        byte_count: stats.byte_count,
        mat_format: Some("v7".to_string()),
        variable_count: Some(variable_count),
        variables_scanned: Some(variables_scanned),
        file_parse_error: None,
        entries: Some(entries),
    })
}

crate::no_template_mining!(
    extract_mat_templates,
    "MATLAB `.mat`: variable layouts and bounded column stats for dense real numeric arrays; no template mining."
);
