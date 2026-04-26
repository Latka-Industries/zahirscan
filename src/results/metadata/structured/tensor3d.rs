//! Summaries for rank-3 dense arrays: min / max / mean / stdev per 2D plane along one axis (capped).

use serde::{Deserialize, Serialize};

// Serde calls this with `&usize`; signature must match `fn(&T) -> bool`.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero_usize(n: &usize) -> bool {
    *n == 0
}

/// One plane (fixed index along `along_axis`): global stats over that 2D slice (possibly subsampled).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Tensor3DPlaneStatEntry {
    /// Index of this plane along the chosen axis (0-based).
    pub plane: usize,
    /// Values included in the estimate (after subsampling).
    pub n_pos: usize,
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    /// Population stdev; omitted when `n < 2` or undefined.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdev: Option<f64>,
}

/// Min / max / mean / stdev over a **strided** linear pass of the full rank-3 buffer (or logical
/// length in MATLAB), not tied to a single 2D plane. `n` counts **finite** samples; `n` may be
/// zero when only non-finite values were seen in the strided set.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Tensor3DGlobalStats {
    /// Count of strided positions that decoded to a finite value (Welford input).
    pub n_pos: usize,
    /// `NaN` in the strided set (NPY f4/f8 and MATLAB f32/f64; always `0` for integer dtypes here).
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub n_nan: usize,
    /// `±∞` in the strided set (NPY f4/f8, MATLAB f32/f64; always `0` for integer dtypes).
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub n_inf: usize,
    /// If `n == 0` (all non-finite in the strided set), `min`/`max`/`mean` are `0.0` and stdev is omitted.
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdev: Option<f64>,
}

/// Per-plane stats for a 3D tensor. Planes are orthogonal to `along_axis`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Tensor3DPlaneStats {
    /// Stacked dimension: smallest extent among `d0..d2` (ties → lowest axis index); if all three
    /// are equal, the layout contiguous axis (C → 0, Fortran / MATLAB → 2).
    pub along_axis: u8,
    /// Element values visited (post subsampling) over all reported planes.
    pub elements_sampled: usize,
    /// Subsampled stats over the **entire** (available) 3D volume, same striding cap as the plane
    /// overview where applicable. Omitted if no non-NaN samples were seen.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub global: Option<Tensor3DGlobalStats>,
    /// N planes (evenly subsampled when `n_along` is large), spread across the axis. The cap
    /// scales with `n_along` (see `tensor3d_max_reported_planes` in the structured parser).
    pub planes: Vec<Tensor3DPlaneStatEntry>,
}
