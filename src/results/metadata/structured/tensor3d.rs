//! Summaries for rank-3 dense arrays: min / max / mean / stdev per 2D plane along one axis (capped).

use serde::{Deserialize, Serialize};

/// One plane (fixed index along `along_axis`): global stats over that 2D slice (possibly subsampled).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Tensor3DPlaneStatEntry {
    /// Index of this plane along the chosen axis (0-based).
    pub plane: usize,
    /// Values included in the estimate (after subsampling).
    pub n: usize,
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    /// Population stdev; omitted when `n < 2` or undefined.
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
    /// At most N planes (evenly subsampled when there are more in the file) spread across the axis.
    pub planes: Vec<Tensor3DPlaneStatEntry>,
}
