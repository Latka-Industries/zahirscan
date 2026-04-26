//! Shared limits, header fixups, layout, scalar JSON values, and NaN/Inf budget scans.

use std::borrow::Cow;
use std::collections::HashSet;

use matrw::interface::types::sparse_array::SparseArray;
use matrw::{MatVariable, MatlabType};
use serde_json::{Value, json};

use crate::parsers::ParseResult;
use crate::results::{ArrayLayoutSummary, MatMetadata};

// --- limits -----------------------------------------------------------------

/// Tunable limits for `.mat` metadata extraction.
pub(crate) struct MaxMatVars;

impl MaxMatVars {
    pub const STRUCT_NEST: usize = 32;
    pub const CELLS_LINEAR: usize = 64;
    pub const STRUCT_ARRAY_ELEMS: usize = 4_096;
    pub const FIELD_NAMES: usize = 256;
    pub const NAN_INF_SCAN_FLOATS: usize = 1_000_000;
    /// Cap on top-level workspace variable rows in `entries`.
    pub const TOP_LEVEL_VARIABLES: usize = 128;
    /// Enable parallel top-level entry builds when this many variables are scanned.
    pub const PARALLEL_TOP_LEVEL_MIN: usize = 16;
    /// Cap sampled numeric values retained for global numeric stats (memory guardrail).
    pub const NUMERIC_STATS_VALUES: usize = 200_000;
    /// Cap unique char tracking size (memory guardrail).
    pub const CHAR_UNIQ_VALUES: usize = 50_000;
}

// --- MAT-5 header -----------------------------------------------------------

/// First 116 bytes of a MAT 5.x file are the human-readable description (see MATLAB MAT-file spec).
pub(crate) const MAT5_HEADER_DESC_BYTES: usize = 116;
/// HDF5 superblock signature. MAT v7.3 does not start with the v5/v7 text header.
pub(crate) const HDF5_FILE_SIGNATURE: &[u8] = &[0x89, b'H', b'D', b'F', b'\r', b'\n', 0x1a, b'\n'];
/// `SciPy` savemat may omit the comma after `MAT-file`; `matrw` expects the official header.
pub(crate) const SCIPY_MAT5_DESCRIPTION_PREFIX: &[u8] = b"MATLAB 5.0 MAT-file Platform:";

pub(crate) fn normalize_scipy_mat5_header_for_matrw(data: &[u8]) -> Cow<'_, [u8]> {
    if data.len() < 128 {
        return Cow::Borrowed(data);
    }
    let desc = &data[..MAT5_HEADER_DESC_BYTES];
    if !desc.starts_with(SCIPY_MAT5_DESCRIPTION_PREFIX) {
        return Cow::Borrowed(data);
    }
    let mut buf = data.to_vec();
    for i in (20..116).rev() {
        buf[i] = buf[i - 1];
    }
    buf[19] = b',';
    Cow::Owned(buf)
}

pub(crate) fn mat_metadata_v73_hdf5_unsupported(stats: &ParseResult) -> MatMetadata {
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

// --- layout -----------------------------------------------------------------

pub(crate) fn peel_compressed(mut v: &MatVariable) -> &MatVariable {
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

fn matlab_real_dtype_string(mt: &MatlabType, is_complex: bool) -> String {
    let mut s = matlab_storage_label(mt);
    if is_complex {
        s.push_str(" complex");
    }
    s
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

fn mat_container_layout(v: &MatVariable, dtype: &str) -> ArrayLayoutSummary {
    ArrayLayoutSummary {
        dtype: Some(dtype.to_string()),
        shape: Some(v.dim()),
        fortran_order: None,
        header_region_bytes: None,
        data_offset: None,
        data_region_bytes: None,
        expected_data_bytes_from_dtype: None,
        ..Default::default()
    }
}

pub(crate) fn array_layout_for_mat_variable(var: &MatVariable) -> ArrayLayoutSummary {
    let v = peel_compressed(var);
    match v {
        MatVariable::NumericArray(n) => {
            let shape = if n.dim.is_empty() {
                None
            } else {
                Some(n.dim.clone())
            };
            let dtype = matlab_real_dtype_string(&n.value, n.value_cmp.is_some());
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
            let dtype = matlab_real_dtype_string(s.numeric_type(), s.value_cmp.is_some());
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
        MatVariable::StructureArray(_) | MatVariable::Structure(_) => {
            mat_container_layout(v, "struct")
        }
        MatVariable::CellArray(_) => mat_container_layout(v, "cell"),
        MatVariable::Unsupported => ArrayLayoutSummary {
            dtype: Some("unsupported".to_string()),
            ..Default::default()
        },
        MatVariable::Null | MatVariable::Compressed(_) => ArrayLayoutSummary::default(),
    }
}

// --- scalar `value` ---------------------------------------------------------

pub(crate) fn is_mat_scalar_1x1(layout: &ArrayLayoutSummary) -> bool {
    matches!(layout.shape.as_deref(), Some([1, 1]))
}

pub(crate) fn is_char_vector_shape(layout: &ArrayLayoutSummary) -> bool {
    let Some(shape) = layout.shape.as_ref() else {
        return false;
    };
    !shape.is_empty() && shape.contains(&1)
}

fn chars_to_string(chars: &[char]) -> String {
    chars.iter().collect()
}

pub(crate) fn bounded_unique_chars(chars: &[char], cap: usize) -> usize {
    let mut uniq = HashSet::with_capacity(cap.min(chars.len()));
    for &ch in chars {
        if uniq.len() >= cap {
            break;
        }
        uniq.insert(ch);
    }
    uniq.len()
}

fn first_scalar_value(value: &MatlabType) -> Option<Value> {
    match value {
        MatlabType::U8(v) => v.first().copied().map(Value::from),
        MatlabType::I8(v) => v.first().copied().map(Value::from),
        MatlabType::U16(v) => v.first().copied().map(Value::from),
        MatlabType::I16(v) => v.first().copied().map(Value::from),
        MatlabType::U32(v) => v.first().copied().map(Value::from),
        MatlabType::I32(v) => v.first().copied().map(Value::from),
        MatlabType::U64(v) => v.first().copied().map(Value::from),
        MatlabType::I64(v) => v.first().copied().map(Value::from),
        MatlabType::F32(v) => v.first().copied().map(f64::from).map(Value::from),
        MatlabType::F64(v) => v.first().copied().map(Value::from),
        MatlabType::UTF8(v) | MatlabType::UTF16(v) => {
            v.first().copied().map(|c| Value::String(c.to_string()))
        }
        MatlabType::BOOL(v) => v.first().copied().map(Value::from),
    }
}

pub(crate) fn scalar_value_for_entry(
    var: &MatVariable,
    layout: &ArrayLayoutSummary,
) -> Option<Value> {
    match peel_compressed(var) {
        MatVariable::NumericArray(n) => {
            match &n.value {
                MatlabType::UTF8(v) | MatlabType::UTF16(v) if is_char_vector_shape(layout) => {
                    return Some(Value::String(chars_to_string(v)));
                }
                _ => {}
            }
            if !is_mat_scalar_1x1(layout) {
                return None;
            }
            if let Some(imag) = n.value_cmp.as_ref() {
                let re = first_scalar_value(&n.value)?;
                let im = first_scalar_value(imag)?;
                Some(json!({ "re": re, "im": im }))
            } else {
                first_scalar_value(&n.value)
            }
        }
        MatVariable::SparseArray(s) => {
            if !is_mat_scalar_1x1(layout) {
                return None;
            }
            first_scalar_value(&s.value)
        }
        _ => None,
    }
}

// --- NaN / Inf (budgeted) ---------------------------------------------------

pub(crate) fn count_nan_inf_in_matlab_type(
    value: &MatlabType,
    comp: Option<&MatlabType>,
    budget: &mut usize,
    n_nan: &mut u64,
    n_inf: &mut u64,
) {
    let tick = |f: f64, b: &mut usize, nn: &mut u64, ni: &mut u64| {
        if *b == 0 {
            return;
        }
        *b -= 1;
        *nn += u64::from(f.is_nan());
        *ni += u64::from(f.is_infinite());
    };
    let tick32 = |f: f32, b: &mut usize, nn: &mut u64, ni: &mut u64| {
        tick(f64::from(f), b, nn, ni);
    };
    if *budget == 0 {
        return;
    }
    match (value, comp) {
        (MatlabType::F32(re), None) => {
            for &x in re {
                if *budget == 0 {
                    return;
                }
                tick32(x, budget, n_nan, n_inf);
            }
        }
        (MatlabType::F64(re), None) => {
            for &x in re {
                if *budget == 0 {
                    return;
                }
                tick(x, budget, n_nan, n_inf);
            }
        }
        (MatlabType::F32(re), Some(MatlabType::F32(im))) => {
            let m = re.len().min(im.len());
            for i in 0..m {
                if *budget == 0 {
                    return;
                }
                tick32(re[i], budget, n_nan, n_inf);
                if *budget == 0 {
                    return;
                }
                tick32(im[i], budget, n_nan, n_inf);
            }
        }
        (MatlabType::F64(re), Some(MatlabType::F64(im))) => {
            let m = re.len().min(im.len());
            for i in 0..m {
                if *budget == 0 {
                    return;
                }
                tick(re[i], budget, n_nan, n_inf);
                if *budget == 0 {
                    return;
                }
                tick(im[i], budget, n_nan, n_inf);
            }
        }
        _ => {}
    }
}

pub(crate) fn count_nan_inf_in_numeric_var(
    v: &MatVariable,
    budget: &mut usize,
    n_nan: &mut u64,
    n_inf: &mut u64,
) {
    if *budget == 0 {
        return;
    }
    let p = peel_compressed(v);
    let MatVariable::NumericArray(n) = p else {
        return;
    };
    count_nan_inf_in_matlab_type(&n.value, n.value_cmp.as_ref(), budget, n_nan, n_inf);
}

pub(crate) fn count_nan_inf_in_sparse_var(
    s: &SparseArray,
    budget: &mut usize,
    n_nan: &mut u64,
    n_inf: &mut u64,
) {
    if *budget == 0 {
        return;
    }
    count_nan_inf_in_matlab_type(&s.value, s.value_cmp.as_ref(), budget, n_nan, n_inf);
}
