//! Tetration `.tet` mmap-friendly chunked tensor container (layout v1).

use serde::{Deserialize, Serialize};

use super::array::ArrayLayoutSummary;
use super::tensor3d::Tensor3DPlaneStats;
use crate::results::{ColumnarCommonFields, MinimalFallback};

/// One dataset entry from the on-disk catalog, with optional query-derived stats.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TetDatasetSummary {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dtype: Option<String>,
    pub shape: Vec<u64>,
    pub chunk_shape: Vec<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_count: Option<u64>,
    #[serde(flatten)]
    pub layout: ArrayLayoutSummary,
    #[serde(flatten)]
    pub common: ColumnarCommonFields,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tensor3d: Option<Tensor3DPlaneStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_error: Option<String>,
}

/// Metadata for a Tetration `.tet` file: superblock, dataset catalog, and chunk index summary.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TetrationMetadata {
    pub byte_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout_version: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dataset_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_chunk_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zstd_chunk_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_event_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub datasets: Option<Vec<TetDatasetSummary>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub datasets_stats_skipped: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_error: Option<String>,
}

impl MinimalFallback for TetrationMetadata {
    fn minimal_fallback(file_size_bytes: usize) -> Self {
        Self {
            byte_count: file_size_bytes,
            ..Self::default()
        }
    }
}
