//! `NumPy` `.npy` / `.npz` on-disk layout metadata plus optional CSV-like column statistics.

use serde::{Deserialize, Serialize};

use super::array::ArrayLayoutSummary;
use super::tensor3d::Tensor3DPlaneStats;
use crate::results::ColumnarCommonFields;
use crate::results::MinimalFallback;

/// Metadata for a single `NumPy` `.npy` array file (header, layout, and sampled column stats when applicable).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct NpyMetadata {
    pub byte_count: usize,
    #[serde(flatten)]
    pub layout: ArrayLayoutSummary,
    /// CSV-like column stats when dtype is not object/structured and rank ≤ 2. Array dimensions are in [`ArrayLayoutSummary::shape`], not duplicated as `row_count` / `column_count` on [`ColumnarCommonFields`].
    #[serde(flatten)]
    pub common: ColumnarCommonFields,
    /// Rank-3 only: min / max / mean / stdev per 2D plane (capped; see parser limits).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tensor3d: Option<Tensor3DPlaneStats>,
}

impl MinimalFallback for NpyMetadata {
    fn minimal_fallback(file_size_bytes: usize) -> Self {
        Self {
            byte_count: file_size_bytes,
            common: ColumnarCommonFields::default(),
            ..Self::default()
        }
    }
}

/// One `.npy` member inside an `.npz` archive.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct NpzNpyEntrySummary {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uncompressed_size: Option<u64>,
    #[serde(flatten)]
    pub layout: ArrayLayoutSummary,
    #[serde(flatten)]
    pub common: ColumnarCommonFields,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tensor3d: Option<Tensor3DPlaneStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_parse_error: Option<String>,
}

/// Metadata for a `NumPy` `.npz` archive (ZIP listing + per-`.npy` header summaries).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct NpzMetadata {
    pub byte_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zip_entry_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub npy_entries_scanned: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub npy_entries: Option<Vec<NpzNpyEntrySummary>>,
}

impl MinimalFallback for NpzMetadata {
    fn minimal_fallback(file_size_bytes: usize) -> Self {
        Self {
            byte_count: file_size_bytes,
            ..Self::default()
        }
    }
}
