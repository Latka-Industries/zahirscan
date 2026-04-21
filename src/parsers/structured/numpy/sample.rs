//! Bounded mmap-friendly sampling of homogeneous `NumPy` arrays for CSV-like column stats.
//!
//! Skips object (`O`), structured, datetime (`M8`/`m8`), complex dtypes, and half-precision (`f2`/`f16`);
//! supports rank 0–2 only.

use crate::config::RuntimeConfig;
use crate::parsers::structured::columnar::utils as columnar_utils;
use crate::parsers::structured::constants::StructuredEncoding;
use crate::results::{ColumnarCommonFields, NpyLayoutSummary};

use super::npy::numpy_descr_element_nbytes;

/// `descr` values for which we do not run string-based tabular stats (object, structured, datetime, complex, half).
#[must_use]
pub fn numpy_descr_skips_tabular_stats(descr: &str) -> bool {
    let t = descr.trim();
    if t.starts_with('[') || t.starts_with('{') {
        return true;
    }
    let rest = t
        .trim_start_matches(|c| "<>|=".contains(c))
        .trim_start_matches('|');
    if rest.is_empty() {
        return true;
    }
    if rest.starts_with("f2") || rest.starts_with("f16") {
        return true;
    }
    let head = rest.as_bytes()[0];
    if head == b'O' {
        return true;
    }
    if head == b'M' || head == b'm' {
        return true;
    }
    if head == b'c' {
        return true;
    }
    false
}

fn table_shape(shape: &[usize]) -> Option<(usize, usize)> {
    match shape.len() {
        0 => Some((1, 1)),
        1 => Some((shape[0], 1)),
        2 => Some((shape[0], shape[1])),
        _ => None,
    }
}

fn elem_offset_2d(row: usize, col: usize, rows: usize, cols: usize, fortran_order: bool) -> usize {
    if fortran_order {
        row + col * rows
    } else {
        col + row * cols
    }
}

/// Contiguous bytes of payload needed from `data_offset` to sample up to `max_sample_rows` (rank 0–2, C or F).
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
        0 | 1 => sr.saturating_mul(1), // scalar / 1-D: first `sr` elements
        2 if !fortran_order => sr.saturating_mul(cols), // C: first `sr` rows × cols
        2 => {
            // F: max linear index among cells (r,c) with r < sr, c < cols
            let r = sr - 1;
            let c = cols.saturating_sub(1);
            elem_offset_2d(r, c, rows, cols, true).saturating_add(1)
        }
        _ => 0,
    }
}

/// Minimum `file_bytes.len()` so `data_offset + payload` covers sampling (not necessarily the full array).
pub(crate) fn min_file_bytes_for_column_stats(
    layout: &NpyLayoutSummary,
    elem_size: usize,
    rows: usize,
    cols: usize,
    file_len_bytes: u64,
    config: &RuntimeConfig,
) -> Option<usize> {
    let data_offset = layout.data_offset?;
    let rank = layout.shape.as_ref()?.len();
    let cap = columnar_utils::tabular_effective_sample_rows(
        config.max_tabular_sample_rows,
        file_len_bytes,
        cols.max(1),
    );
    let sample_rows = rows.min(cap);
    let payload_elems = contiguous_payload_prefix_elems(
        rows,
        cols,
        rank,
        layout.fortran_order.unwrap_or(false),
        sample_rows,
    );
    let prefix = elem_size.saturating_mul(payload_elems);
    Some(data_offset.saturating_add(prefix))
}

fn max_sample_rows_in_prefix(
    rows: usize,
    cols: usize,
    rank: usize,
    fortran: bool,
    elem_size: usize,
    avail: usize,
    max_csv: usize,
) -> usize {
    let hi = rows.min(max_csv);
    if hi == 0 || elem_size == 0 || avail < elem_size {
        return 0;
    }
    let mut lo = 0usize;
    let mut hi2 = hi;
    while lo < hi2 {
        let mid = (lo + hi2).div_ceil(2);
        let elems = contiguous_payload_prefix_elems(rows, cols, rank, fortran, mid);
        let need = elems.saturating_mul(elem_size);
        if need <= avail {
            lo = mid;
        } else {
            hi2 = mid - 1;
        }
    }
    lo
}

/// Bytes to read from a ZIP member so standalone `.npy` parity is possible (header + sample prefix, capped).
pub(crate) fn zip_member_target_read_len(
    layout: &NpyLayoutSummary,
    uncompressed_size: usize,
    config: &RuntimeConfig,
) -> usize {
    let Some(descr) = layout.descr.as_deref() else {
        return uncompressed_size.min(512 * 1024);
    };
    if numpy_descr_skips_tabular_stats(descr) {
        return uncompressed_size.min(512 * 1024);
    }
    let Some(elem) = numpy_descr_element_nbytes(descr) else {
        return uncompressed_size.min(512 * 1024);
    };
    let Some(shape) = layout.shape.as_ref() else {
        return uncompressed_size.min(512 * 1024);
    };
    let Some((rows, cols)) = table_shape(shape) else {
        return uncompressed_size.min(512 * 1024);
    };
    min_file_bytes_for_column_stats(layout, elem, rows, cols, uncompressed_size as u64, config)
        .unwrap_or(512 * 1024)
        .min(uncompressed_size)
}

fn is_little_endian(descr: &str) -> bool {
    !descr.trim_start().starts_with('>')
}

fn decode_cell(descr: &str, chunk: &[u8]) -> Option<String> {
    let t = descr.trim();
    let le = is_little_endian(t);
    let rest = t
        .trim_start_matches(|c| "<>|=".contains(c))
        .trim_start_matches('|');

    macro_rules! r {
        ($ty:ty, $n:expr) => {{
            if chunk.len() < $n {
                return None;
            }
            let a: [u8; $n] = chunk[..$n].try_into().ok()?;
            let v = if le {
                <$ty>::from_le_bytes(a)
            } else {
                <$ty>::from_be_bytes(a)
            };
            Some(format!("{v}"))
        }};
    }

    if rest.starts_with('?') {
        return r!(u8, 1);
    }
    if rest.starts_with("b1") || (rest.starts_with('b') && rest.len() == 1) {
        return r!(u8, 1);
    }
    match rest {
        r if r.starts_with("i8") => r!(i64, 8),
        r if r.starts_with("u8") => r!(u64, 8),
        r if r.starts_with("i4") => r!(i32, 4),
        r if r.starts_with("u4") => r!(u32, 4),
        r if r.starts_with("i2") => r!(i16, 2),
        r if r.starts_with("u2") => r!(u16, 2),
        r if r.starts_with("i1") => r!(i8, 1),
        r if r.starts_with("u1") => r!(u8, 1),
        r if r.starts_with("f4") => {
            if chunk.len() < 4 {
                return None;
            }
            let a: [u8; 4] = chunk[..4].try_into().ok()?;
            let v = if le {
                f32::from_le_bytes(a)
            } else {
                f32::from_be_bytes(a)
            };
            if v.is_nan() {
                Some(String::new())
            } else {
                Some(format!("{v:.15}"))
            }
        }
        r if r.starts_with("f8") => {
            if chunk.len() < 8 {
                return None;
            }
            let a: [u8; 8] = chunk[..8].try_into().ok()?;
            let v = if le {
                f64::from_le_bytes(a)
            } else {
                f64::from_be_bytes(a)
            };
            if v.is_nan() {
                Some(String::new())
            } else {
                Some(format!("{v:.15}"))
            }
        }
        _ => None,
    }
}

/// Build [`ColumnarCommonFields`] from raw `.npy` bytes and a parsed layout (header-only fields).
#[must_use]
pub fn column_common_from_npy_bytes(
    file_bytes: &[u8],
    layout: &NpyLayoutSummary,
    config: &RuntimeConfig,
) -> ColumnarCommonFields {
    let Some(descr) = layout.descr.as_deref() else {
        return ColumnarCommonFields::default();
    };
    if numpy_descr_skips_tabular_stats(descr) {
        return shape_only_common(layout);
    }
    let Some(shape) = layout.shape.as_ref() else {
        return ColumnarCommonFields::default();
    };
    let Some(elem_size) = numpy_descr_element_nbytes(descr) else {
        return shape_only_common(layout);
    };
    let Some(data_offset) = layout.data_offset else {
        return ColumnarCommonFields::default();
    };
    let Some((rows, cols)) = table_shape(shape) else {
        return shape_only_common(layout);
    };
    if rows == 0 || cols == 0 {
        return shape_only_common(layout);
    }

    if data_offset > file_bytes.len() {
        return shape_only_common(layout);
    }

    let avail = file_bytes.len().saturating_sub(data_offset);
    let rank = shape.len();
    let fortran = layout.fortran_order.unwrap_or(false);
    let file_len = file_bytes.len() as u64;
    let cap = columnar_utils::tabular_effective_sample_rows(
        config.max_tabular_sample_rows,
        file_len,
        cols.max(1),
    );
    let sample_rows = max_sample_rows_in_prefix(rows, cols, rank, fortran, elem_size, avail, cap);
    if sample_rows == 0 {
        return shape_only_common(layout);
    }

    let payload_elems = contiguous_payload_prefix_elems(rows, cols, rank, fortran, sample_rows);
    let prefix_bytes = elem_size.saturating_mul(payload_elems);
    if prefix_bytes > avail {
        return shape_only_common(layout);
    }
    let data_end = data_offset.saturating_add(prefix_bytes);
    let data = &file_bytes[data_offset..data_end];

    let mut sample_data: Vec<Vec<String>> = Vec::with_capacity(sample_rows);

    for r in 0..sample_rows {
        let mut row = Vec::with_capacity(cols);
        for c in 0..cols {
            let idx = elem_offset_2d(r, c, rows, cols, layout.fortran_order.unwrap_or(false));
            let start = idx.saturating_mul(elem_size);
            let end = start.saturating_add(elem_size);
            let cell = data
                .get(start..end)
                .and_then(|chunk| decode_cell(descr, chunk))
                .unwrap_or_default();
            row.push(cell);
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
        encoding: Some(StructuredEncoding::NUMPY.to_string()),
        columns,
    }
}

fn shape_only_common(layout: &NpyLayoutSummary) -> ColumnarCommonFields {
    let Some(shape) = layout.shape.as_ref() else {
        return ColumnarCommonFields::default();
    };
    let (row_count, column_count) = if let Some((r, c)) = table_shape(shape) {
        (r, c)
    } else {
        let n: usize = shape.iter().product();
        (n, 0)
    };
    ColumnarCommonFields {
        row_count,
        column_count,
        encoding: Some(StructuredEncoding::NUMPY.to_string()),
        ..ColumnarCommonFields::default()
    }
}
