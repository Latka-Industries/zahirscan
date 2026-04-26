//! `Zarr` directory store: hierarchy summary and per-array layout (V2 / V3 via `zarrs`).

use serde::{Deserialize, Serialize};

use super::array::ArrayLayoutSummary;
use super::tensor3d::Tensor3DPlaneStats;
use crate::results::ColumnarCommonFields;
use crate::results::MinimalFallback;

/// One array under a `Zarr` store (path + layout + optional tabular / 3D stats, mirroring [`super::numpy::NpzNpyEntrySummary`]).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ZarrArrayEntrySummary {
    /// In-store path (e.g. `/field`, `/group/x`).
    pub name: String,
    /// `2` or `3` for Zarr *array* metadata kind (from the opened array).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zarr_array_metadata: Option<String>,
    #[serde(flatten)]
    pub layout: ArrayLayoutSummary,
    #[serde(flatten)]
    pub common: ColumnarCommonFields,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tensor3d: Option<Tensor3DPlaneStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_chunks: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_parse_error: Option<String>,
}

/// Metadata for a `Zarr` store (V2 and V3 supported by the `zarrs` stack).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ZarrMetadata {
    pub byte_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_group_metadata: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub array_entries_scanned: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub array_entries: Option<Vec<ZarrArrayEntrySummary>>,
}

impl MinimalFallback for ZarrMetadata {
    fn minimal_fallback(file_size_bytes: usize) -> Self {
        Self {
            byte_count: file_size_bytes,
            ..Default::default()
        }
    }
}
