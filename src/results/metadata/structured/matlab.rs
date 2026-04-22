//! MATLAB `.mat` (classic v7) container metadata: named variables and per-array layout + optional column stats.

use serde::{Deserialize, Serialize};

use crate::results::ColumnarCommonFields;
use crate::results::MinimalFallback;

use super::array::ArrayLayoutSummary;
use super::tensor3d::Tensor3DPlaneStats;

/// One variable inside a `.mat` file (mirrors [`super::numpy::NpzNpyEntrySummary`] shape).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct MatArrayEntrySummary {
    pub name: String,
    #[serde(flatten)]
    pub layout: ArrayLayoutSummary,
    #[serde(flatten)]
    pub common: ColumnarCommonFields,
    /// Rank-3 dense numeric: per-plane stats (capped).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tensor3d: Option<Tensor3DPlaneStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_parse_error: Option<String>,
}

/// Metadata for a MATLAB `.mat` file (v7 classic; v7.3 HDF5 is flagged without variable listings).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct MatMetadata {
    pub byte_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mat_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variable_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables_scanned: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_parse_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entries: Option<Vec<MatArrayEntrySummary>>,
}

impl MinimalFallback for MatMetadata {
    fn minimal_fallback(file_size_bytes: usize) -> Self {
        Self {
            byte_count: file_size_bytes,
            ..Self::default()
        }
    }
}
