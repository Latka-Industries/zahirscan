//! Rank-3 min / max / mean / (population) stdev per 2D plane, with caps and optional Rayon over planes.

use std::collections::{HashMap, HashSet};

use matrw::MatlabType;
use rayon::prelude::*;

use super::numpy::{numpy_descr_element_nbytes, numpy_descr_skips_tabular_stats};
use crate::parsers::structured::constants::limits::{
    TENSOR3D_MAX_LINEAR_SAMPLES, TENSOR3D_MAX_PLANE_LINEAR_SAMPLES,
};
use crate::results::{
    ArrayLayoutSummary, Tensor3DGlobalStats, Tensor3DPlaneStatEntry, Tensor3DPlaneStats,
};

pub use super::constants::tensor3d_max_reported_planes;

/// Memory-contiguous stack axis for a full dense buffer: C order → 0, Fortran / MATLAB → 2.
/// Used when all three dimensions are equal (tie-break for I/O).
#[must_use]
pub const fn contiguous_3d_stack_axis(fortran: bool) -> u8 {
    if fortran { 2 } else { 0 }
}

fn dim_at(d0: usize, d1: usize, d2: usize, along: u8) -> usize {
    match along {
        0 => d0,
        1 => d1,
        _ => d2,
    }
}

/// Prefer the axis with the **smallest** extent (fewest planes). Ties break on the **lowest** axis
/// index. If `d0 == d1 == d2`, use [`contiguous_3d_stack_axis`] instead.
#[must_use]
pub fn stack_axis_preferred(d0: usize, d1: usize, d2: usize, fortran: bool) -> u8 {
    if d0 == d1 && d1 == d2 {
        contiguous_3d_stack_axis(fortran)
    } else {
        [(0u8, d0), (1u8, d1), (2u8, d2)]
            .into_iter()
            .min_by_key(|(axis, len)| (*len, *axis))
            .map_or(0, |(axis, _)| axis)
    }
}

fn evenly_spaced_indices(n: usize, max: usize) -> Vec<usize> {
    if n == 0 {
        return Vec::new();
    }
    if n <= max {
        return (0..n).collect();
    }
    let max = max.max(1);
    (0..max)
        .map(|k| {
            if max == 1 {
                0
            } else {
                (k * (n - 1)) / (max - 1)
            }
        })
        .collect()
}

fn is_little_endian_descr(descr: &str) -> bool {
    !descr.trim_start().starts_with('>')
}

/// One f64 (or non-finite / missing) from an NPY element window or a MATLAB cell.
#[derive(Copy, Clone, Debug, PartialEq)]
enum F64Sample {
    Fin(f64),
    Nan,
    Inf,
    /// Short buffer, unknown `descr`, or (MATLAB) out of range / non-numeric.
    Missing,
}

/// NPY `descr` at one element window: finite, NaN, ±∞ (float only), or missing.
fn npy_f64_classify(descr: &str, data: &[u8]) -> F64Sample {
    let le = is_little_endian_descr(descr);
    let t = descr.trim();
    let rest = t
        .trim_start_matches(|c: char| "<>|=".contains(c))
        .trim_start_matches('|');

    macro_rules! r_i {
        ($ty:ty, $n:expr) => {{
            if data.len() < $n {
                F64Sample::Missing
            } else if let Ok(a) = <[u8; $n]>::try_from(&data[..$n]) {
                let v: $ty = if le {
                    <$ty>::from_le_bytes(a)
                } else {
                    <$ty>::from_be_bytes(a)
                };
                F64Sample::Fin(v as f64)
            } else {
                F64Sample::Missing
            }
        }};
    }

    if rest.starts_with('?') {
        if data.is_empty() {
            return F64Sample::Missing;
        }
        return F64Sample::Fin(if data[0] == 0 { 0.0 } else { 1.0 });
    }
    if rest.starts_with("f4") {
        if data.len() < 4 {
            return F64Sample::Missing;
        }
        if let Ok(a) = <[u8; 4]>::try_from(&data[..4]) {
            let v = if le {
                f32::from_le_bytes(a)
            } else {
                f32::from_be_bytes(a)
            };
            return if v.is_nan() {
                F64Sample::Nan
            } else if v.is_infinite() {
                F64Sample::Inf
            } else {
                F64Sample::Fin(f64::from(v))
            };
        }
        return F64Sample::Missing;
    }
    if rest.starts_with("f8") {
        if data.len() < 8 {
            return F64Sample::Missing;
        }
        if let Ok(a) = <[u8; 8]>::try_from(&data[..8]) {
            let v = if le {
                f64::from_le_bytes(a)
            } else {
                f64::from_be_bytes(a)
            };
            return if v.is_nan() {
                F64Sample::Nan
            } else if v.is_infinite() {
                F64Sample::Inf
            } else {
                F64Sample::Fin(v)
            };
        }
        return F64Sample::Missing;
    }
    match rest {
        r if r.starts_with("i1") => r_i!(i8, 1),
        r if r.starts_with("u1") | r.starts_with("b1") | (r.starts_with('b') && r.len() == 1) => {
            r_i!(u8, 1)
        }
        r if r.starts_with("i2") => r_i!(i16, 2),
        r if r.starts_with("u2") => r_i!(u16, 2),
        r if r.starts_with("i4") => r_i!(i32, 4),
        r if r.starts_with("u4") => r_i!(u32, 4),
        r if r.starts_with("i8") => r_i!(i64, 8),
        r if r.starts_with("u8") => r_i!(u64, 8),
        _ => F64Sample::Missing,
    }
}

/// NPY `descr` → f64; `None` for NaN, ±∞, and short buffers.
fn npy_f64_at(descr: &str, data: &[u8]) -> Option<f64> {
    match npy_f64_classify(descr, data) {
        F64Sample::Fin(x) => Some(x),
        _ => None,
    }
}

fn mat_f64_classify(v: &MatlabType, idx: usize) -> F64Sample {
    match v {
        MatlabType::F32(x) => x.get(idx).map_or(F64Sample::Missing, |&u| {
            let t = f64::from(u);
            if t.is_nan() {
                F64Sample::Nan
            } else if t.is_infinite() {
                F64Sample::Inf
            } else {
                F64Sample::Fin(t)
            }
        }),
        MatlabType::F64(x) => x.get(idx).map_or(F64Sample::Missing, |&u| {
            if u.is_nan() {
                F64Sample::Nan
            } else if u.is_infinite() {
                F64Sample::Inf
            } else {
                F64Sample::Fin(u)
            }
        }),
        _ => f64_at_mat(v, idx).map_or(F64Sample::Missing, F64Sample::Fin),
    }
}

fn mat_strided_stats_global_f64(value: &MatlabType, to_len: usize) -> Option<Tensor3DGlobalStats> {
    if to_len == 0 {
        return None;
    }
    let stride = to_len.div_ceil(TENSOR3D_MAX_LINEAR_SAMPLES);
    let stride = stride.max(1);
    let mut w = Welford::default();
    let mut n_nan: usize = 0;
    let mut n_inf: usize = 0;
    for linear in (0..to_len).step_by(stride) {
        match mat_f64_classify(value, linear) {
            F64Sample::Fin(x) => w.update(x),
            F64Sample::Nan => n_nan += 1,
            F64Sample::Inf => n_inf += 1,
            F64Sample::Missing => {}
        }
    }
    w.into_global(n_nan, n_inf)
}

struct Welford {
    n: u64,
    min: f64,
    max: f64,
    mean: f64,
    m2: f64,
    first: bool,
}

impl Default for Welford {
    /// Empty aggregate (no updates yet). `first: true` is required before the first `update`.
    fn default() -> Self {
        Self {
            n: 0,
            min: 0.0,
            max: 0.0,
            mean: 0.0,
            m2: 0.0,
            first: true,
        }
    }
}

impl Welford {
    fn update(&mut self, x: f64) {
        if x.is_nan() || x.is_infinite() {
            return;
        }
        self.n += 1;
        if self.first {
            self.min = x;
            self.max = x;
            self.mean = x;
            self.m2 = 0.0;
            self.first = false;
            return;
        }
        self.min = self.min.min(x);
        self.max = self.max.max(x);
        let d = x - self.mean;
        self.mean += d / self.n as f64;
        let d2 = x - self.mean;
        self.m2 += d * d2;
    }

    fn into_entry(self, plane: usize) -> Option<Tensor3DPlaneStatEntry> {
        if self.n == 0 {
            return None;
        }
        let n = self.n as usize;
        let stdev = if self.n > 1 {
            Some((self.m2 / self.n as f64).sqrt())
        } else {
            None
        };
        Some(Tensor3DPlaneStatEntry {
            plane,
            n,
            min: self.min,
            max: self.max,
            mean: self.mean,
            stdev,
        })
    }

    fn into_global(self, n_nan: usize, n_inf: usize) -> Option<Tensor3DGlobalStats> {
        if self.n == 0 && n_nan == 0 && n_inf == 0 {
            return None;
        }
        let n = self.n as usize;
        if self.n > 0 {
            let stdev = if self.n > 1 {
                Some((self.m2 / self.n as f64).sqrt())
            } else {
                None
            };
            return Some(Tensor3DGlobalStats {
                n,
                n_nan,
                n_inf,
                min: self.min,
                max: self.max,
                mean: self.mean,
                stdev,
            });
        }
        Some(Tensor3DGlobalStats {
            n: 0,
            n_nan,
            n_inf,
            min: 0.0,
            max: 0.0,
            mean: 0.0,
            stdev: None,
        })
    }
}

/// Column-major: first dim fastest (MATLAB / NPY F).
fn unravel_col_maj_3d(idx: usize, d0: usize, d1: usize) -> (usize, usize, usize) {
    let i0 = idx % d0;
    let t = idx / d0;
    let i1 = t % d1;
    let i2 = t / d1;
    (i0, i1, i2)
}

/// C order: last dim fastest (`NumPy` default).
#[must_use]
pub fn unravel_c_3d(idx: usize, _d0: usize, d1: usize, d2: usize) -> (usize, usize, usize) {
    let i2 = idx % d2;
    let t = idx / d2;
    let i1 = t % d1;
    let i0 = t / d1;
    (i0, i1, i2)
}

fn plane_index(coord: (usize, usize, usize), along: u8) -> usize {
    match along {
        0 => coord.0,
        1 => coord.1,
        _ => coord.2,
    }
}

/// Strided linear min/max/mean/stdev over `to_visit` elements in memory order (C or F).
fn npy_strided_stats_global_f64(
    file_bytes: &[u8],
    data_offset: usize,
    to_visit: usize,
    elem_size: usize,
    descr: &str,
) -> Option<Tensor3DGlobalStats> {
    if to_visit == 0 {
        return None;
    }
    let stride = to_visit.div_ceil(TENSOR3D_MAX_LINEAR_SAMPLES);
    let stride = stride.max(1);
    let mut w = Welford::default();
    let mut n_nan: usize = 0;
    let mut n_inf: usize = 0;
    for linear in (0..to_visit).step_by(stride) {
        let off = data_offset + linear * elem_size;
        let end = off + elem_size;
        if end > file_bytes.len() {
            break;
        }
        match npy_f64_classify(descr, &file_bytes[off..end]) {
            F64Sample::Fin(x) => w.update(x),
            F64Sample::Nan => n_nan += 1,
            F64Sample::Inf => n_inf += 1,
            F64Sample::Missing => {}
        }
    }
    w.into_global(n_nan, n_inf)
}

/// Rank-3 NPY view + dtype when the payload is only partially available (strided linear scan).
struct NpyStridedInput<'a> {
    file_bytes: &'a [u8],
    data_offset: usize,
    d0: usize,
    d1: usize,
    d2: usize,
    fortran: bool,
    along: u8,
    elem_size: usize,
    descr: &'a str,
    available_elems: usize,
}

/// Rank-3 NPY view + dtype when the full array is in `file_bytes` (contiguous plane slabs).
struct NpyContiguousInput<'a> {
    file_bytes: &'a [u8],
    data_offset: usize,
    d0: usize,
    d1: usize,
    d2: usize,
    fortran: bool,
    along: u8,
    elem_size: usize,
    descr: &'a str,
}

/// `npy_f64_at` is used with `data_offset + linear*elem` window.
fn tensor3d_npy_strided(ctx: &NpyStridedInput<'_>) -> Option<Tensor3DPlaneStats> {
    let &NpyStridedInput {
        file_bytes,
        data_offset,
        d0,
        d1,
        d2,
        fortran,
        along,
        elem_size,
        descr,
        available_elems,
    } = ctx;
    let total: usize = d0.checked_mul(d1)?.checked_mul(d2)?;
    let to_visit = available_elems.min(total);
    if to_visit == 0 {
        return None;
    }
    let n_along = dim_at(d0, d1, d2, along);
    let max_planes = tensor3d_max_reported_planes(n_along);
    let pick = evenly_spaced_indices(n_along, max_planes);
    if pick.is_empty() {
        return None;
    }
    let set: HashSet<usize> = pick.iter().copied().collect();

    let stride = to_visit.div_ceil(TENSOR3D_MAX_LINEAR_SAMPLES);
    let stride = stride.max(1);

    let mut map: HashMap<usize, Welford> = pick.iter().map(|&p| (p, Welford::default())).collect();
    let mut global = Welford::default();
    let mut n_nan: usize = 0;
    let mut n_inf: usize = 0;
    for linear in (0..to_visit).step_by(stride) {
        let (a0, a1, a2) = if fortran {
            unravel_col_maj_3d(linear, d0, d1)
        } else {
            unravel_c_3d(linear, d0, d1, d2)
        };
        let p = plane_index((a0, a1, a2), along);
        let off = data_offset + linear * elem_size;
        let end = off + elem_size;
        if end > file_bytes.len() {
            break;
        }
        match npy_f64_classify(descr, &file_bytes[off..end]) {
            F64Sample::Fin(x) => {
                global.update(x);
                if set.contains(&p)
                    && let Some(ww) = map.get_mut(&p)
                {
                    ww.update(x);
                }
            }
            F64Sample::Nan => n_nan += 1,
            F64Sample::Inf => n_inf += 1,
            F64Sample::Missing => {}
        }
    }

    let planes: Vec<_> = pick
        .into_iter()
        .filter_map(|p| map.remove(&p).and_then(|w| w.into_entry(p)))
        .collect();
    if planes.is_empty() {
        return None;
    }
    let elements_sampled: usize = planes.iter().map(|e| e.n).sum();
    let global = global.into_global(n_nan, n_inf);
    Some(Tensor3DPlaneStats {
        along_axis: along,
        elements_sampled,
        global,
        planes,
    })
}

/// Returns start linear index and plane length in elements (contiguous) for the stack axis.
fn npy_contiguous_block(
    d0: usize,
    d1: usize,
    d2: usize,
    fortran: bool,
    along: u8,
    p: usize,
) -> Option<(usize, usize)> {
    match (fortran, along) {
        (false, 0) => {
            if p >= d0 {
                return None;
            }
            Some((p * d1 * d2, d1 * d2))
        }
        (true, 2) => {
            if p >= d2 {
                return None;
            }
            Some((p * d0 * d1, d0 * d1))
        }
        _ => None,
    }
}

fn welford_byte_slice_f64(
    data: &[u8],
    elem_size: usize,
    descr: &str,
    plane: usize,
    cap: usize,
) -> Option<Tensor3DPlaneStatEntry> {
    if data.is_empty() || elem_size == 0 {
        return None;
    }
    let n = data.len() / elem_size;
    let step = n.div_ceil(cap);
    let step = step.max(1);
    let mut w = Welford::default();
    for j in (0..n).step_by(step) {
        let s = j * elem_size;
        if let Some(x) = npy_f64_at(descr, &data[s..s + elem_size]) {
            w.update(x);
        }
    }
    w.into_entry(plane)
}

fn npy_contiguous_path(ctx: &NpyContiguousInput<'_>) -> Option<Tensor3DPlaneStats> {
    let &NpyContiguousInput {
        file_bytes,
        data_offset,
        d0,
        d1,
        d2,
        fortran,
        along,
        elem_size,
        descr,
    } = ctx;
    let n_along = dim_at(d0, d1, d2, along);
    let max_planes = tensor3d_max_reported_planes(n_along);
    let pick = evenly_spaced_indices(n_along, max_planes);
    if pick.is_empty() {
        return None;
    }

    let out: Option<Vec<Tensor3DPlaneStatEntry>> = if n_along < 2 || pick.len() == 1 {
        // Sequential when tiny.
        let mut v = Vec::new();
        for p in &pick {
            if let Some((start, plen)) = npy_contiguous_block(d0, d1, d2, fortran, along, *p) {
                let s = data_offset + start * elem_size;
                let e = s + plen * elem_size;
                if e > file_bytes.len() {
                    continue;
                }
                if let Some(entry) = welford_byte_slice_f64(
                    &file_bytes[s..e],
                    elem_size,
                    descr,
                    *p,
                    TENSOR3D_MAX_PLANE_LINEAR_SAMPLES,
                ) {
                    v.push(entry);
                }
            }
        }
        if v.is_empty() { None } else { Some(v) }
    } else {
        let results: Vec<_> = pick
            .par_iter()
            .filter_map(|&p| {
                let (start, plen) = npy_contiguous_block(d0, d1, d2, fortran, along, p)?;
                let s = data_offset + start * elem_size;
                let e = s + plen * elem_size;
                if e > file_bytes.len() {
                    return None;
                }
                welford_byte_slice_f64(
                    &file_bytes[s..e],
                    elem_size,
                    descr,
                    p,
                    TENSOR3D_MAX_PLANE_LINEAR_SAMPLES,
                )
            })
            .collect();
        if results.is_empty() {
            None
        } else {
            Some(results)
        }
    };

    let mut planes = out?;
    if planes.is_empty() {
        return None;
    }
    planes.sort_by_key(|e| e.plane);
    let elements_sampled: usize = planes.iter().map(|e| e.n).sum();
    let total = d0 * d1 * d2;
    let global = npy_strided_stats_global_f64(file_bytes, data_offset, total, elem_size, descr);
    Some(Tensor3DPlaneStats {
        along_axis: along,
        elements_sampled,
        global,
        planes,
    })
}

/// True if we can read numeric samples for 3D tensor stats from this `descr`.
fn npy_dtype_ok_for_tensor3d(descr: &str) -> bool {
    !numpy_descr_skips_tabular_stats(descr)
}

/// NPY / NPZ: rank-3 dense; uses contiguous slab axis when the buffer holds the full array, else a strided linear pass.
#[must_use]
pub fn tensor3d_plane_stats_for_npy_bytes(
    file_bytes: &[u8],
    layout: &ArrayLayoutSummary,
) -> Option<Tensor3DPlaneStats> {
    let shape = layout.shape.as_deref()?;
    if shape.len() != 3 {
        return None;
    }
    let descr = layout.dtype.as_deref()?;
    if !npy_dtype_ok_for_tensor3d(descr) {
        return None;
    }
    let d0 = shape[0];
    let d1 = shape[1];
    let d2 = shape[2];
    if d0 == 0 || d1 == 0 || d2 == 0 {
        return None;
    }
    let elem = numpy_descr_element_nbytes(descr)?;
    let data_offset = layout.data_offset?;
    if data_offset > file_bytes.len() {
        return None;
    }
    let avail = file_bytes.len().saturating_sub(data_offset);
    let available_elems = avail / elem;
    if available_elems == 0 {
        return None;
    }
    let total = d0 * d1 * d2;
    let fortran = layout.fortran_order.unwrap_or(false);
    let along = stack_axis_preferred(d0, d1, d2, fortran);
    let along_contig = contiguous_3d_stack_axis(fortran);
    let can_contiguous = available_elems >= total
        && along == along_contig
        && npy_contiguous_block(d0, d1, d2, fortran, along, 0).is_some();

    if can_contiguous {
        npy_contiguous_path(&NpyContiguousInput {
            file_bytes,
            data_offset,
            d0,
            d1,
            d2,
            fortran,
            along,
            elem_size: elem,
            descr,
        })
    } else {
        tensor3d_npy_strided(&NpyStridedInput {
            file_bytes,
            data_offset,
            d0,
            d1,
            d2,
            fortran,
            along,
            elem_size: elem,
            descr,
            available_elems,
        })
    }
}

// --- MATLAB column-major, stack axis 2, same contiguous blocks as F order NPY. ---

fn matlab_type_len(v: &MatlabType) -> usize {
    match v {
        MatlabType::U8(x) => x.len(),
        MatlabType::I8(x) => x.len(),
        MatlabType::U16(x) => x.len(),
        MatlabType::I16(x) => x.len(),
        MatlabType::U32(x) => x.len(),
        MatlabType::I32(x) => x.len(),
        MatlabType::U64(x) => x.len(),
        MatlabType::I64(x) => x.len(),
        MatlabType::F32(x) => x.len(),
        MatlabType::F64(x) => x.len(),
        MatlabType::UTF8(x) | MatlabType::UTF16(x) => x.len(),
        MatlabType::BOOL(x) => x.len(),
    }
}

fn f64_at_mat(v: &MatlabType, idx: usize) -> Option<f64> {
    match v {
        MatlabType::F32(x) => x.get(idx).copied().map(f64::from).filter(|v| !v.is_nan()),
        MatlabType::F64(x) => x.get(idx).copied().filter(|v| !v.is_nan()),
        MatlabType::U8(x) => x.get(idx).copied().map(f64::from),
        MatlabType::I8(x) => x.get(idx).copied().map(f64::from),
        MatlabType::U16(x) => x.get(idx).copied().map(f64::from),
        MatlabType::I16(x) => x.get(idx).copied().map(f64::from),
        MatlabType::U32(x) => x.get(idx).copied().map(f64::from),
        MatlabType::I32(x) => x.get(idx).copied().map(f64::from),
        MatlabType::U64(x) => x.get(idx).copied().map(|v| v as f64),
        MatlabType::I64(x) => x.get(idx).copied().map(|v| v as f64),
        _ => None,
    }
}

fn welford_mat_slice(
    v: &MatlabType,
    start: usize,
    len: usize,
    cap: usize,
    plane: usize,
) -> Option<Tensor3DPlaneStatEntry> {
    if len == 0 {
        return None;
    }
    let step = len.div_ceil(cap);
    let step = step.max(1);
    let mut w = Welford::default();
    for j in (0..len).step_by(step) {
        if let Some(x) = f64_at_mat(v, start + j) {
            w.update(x);
        }
    }
    w.into_entry(plane)
}

/// 3D numeric MATLAB array (no complex), column-major storage.
#[must_use]
pub fn tensor3d_plane_stats_for_mat_colmaj(
    value: &MatlabType,
    d0: usize,
    d1: usize,
    d2: usize,
) -> Option<Tensor3DPlaneStats> {
    if d0 == 0 || d1 == 0 || d2 == 0 {
        return None;
    }
    let total = d0 * d1 * d2;
    let to_len = matlab_type_len(value).min(total);
    if to_len == 0 {
        return None;
    }

    let fortran = true;
    let along = stack_axis_preferred(d0, d1, d2, fortran);
    let along_contig = contiguous_3d_stack_axis(fortran);
    let n_along = dim_at(d0, d1, d2, along);
    let max_planes = tensor3d_max_reported_planes(n_along);
    let pick = evenly_spaced_indices(n_along, max_planes);
    if pick.is_empty() {
        return None;
    }

    let can_parallel = to_len == total && pick.len() > 1 && along == along_contig;

    let out: Option<Vec<Tensor3DPlaneStatEntry>> = if can_parallel {
        let results: Vec<_> = pick
            .par_iter()
            .filter_map(|&p| {
                let (start, plen) = npy_contiguous_block(d0, d1, d2, fortran, along, p)?;
                welford_mat_slice(value, start, plen, TENSOR3D_MAX_PLANE_LINEAR_SAMPLES, p)
            })
            .collect();
        if results.is_empty() {
            None
        } else {
            Some(results)
        }
    } else {
        let mut v = Vec::new();
        for p in &pick {
            if along == along_contig {
                if let Some((start, plen)) = npy_contiguous_block(d0, d1, d2, fortran, along, *p) {
                    if start + plen <= to_len {
                        if let Some(e) = welford_mat_slice(
                            value,
                            start,
                            plen,
                            TENSOR3D_MAX_PLANE_LINEAR_SAMPLES,
                            *p,
                        ) {
                            v.push(e);
                        }
                    } else if let Some(e) =
                        welford_mat_colmaj_strided(value, d0, d1, d2, *p, to_len, along)
                    {
                        v.push(e);
                    }
                }
            } else if let Some(e) = welford_mat_colmaj_strided(value, d0, d1, d2, *p, to_len, along)
            {
                v.push(e);
            }
        }
        if v.is_empty() { None } else { Some(v) }
    };

    let mut planes = out?;
    if planes.is_empty() {
        return None;
    }
    planes.sort_by_key(|e| e.plane);
    let elements_sampled: usize = planes.iter().map(|e| e.n).sum();
    let global = mat_strided_stats_global_f64(value, to_len);
    Some(Tensor3DPlaneStats {
        along_axis: along,
        elements_sampled,
        global,
        planes,
    })
}

fn welford_mat_colmaj_strided(
    value: &MatlabType,
    d0: usize,
    d1: usize,
    d2: usize,
    plane: usize,
    to_len: usize,
    along: u8,
) -> Option<Tensor3DPlaneStatEntry> {
    let n_along = dim_at(d0, d1, d2, along);
    if plane >= n_along {
        return None;
    }
    let to_visit = to_len;
    let stride = to_visit.div_ceil(TENSOR3D_MAX_LINEAR_SAMPLES);
    let stride = stride.max(1);
    let mut w = Welford::default();
    for linear in (0..to_visit).step_by(stride) {
        let (a0, a1, a2) = unravel_col_maj_3d(linear, d0, d1);
        if plane_index((a0, a1, a2), along) == plane
            && let Some(x) = f64_at_mat(value, linear)
        {
            w.update(x);
        }
    }
    if w.n == 0 { None } else { w.into_entry(plane) }
}
