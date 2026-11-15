//! Tetration `.tet` mmap-friendly chunked tensor container (layout v1).
//!
//! Output field: `tetration_metadata` — catalog plus optional query-derived column/tensor stats
//! (`query_error` per dataset when reductions fail).
//!
//! Against tetration **0.8+** this also carries:
//! - file footer (`tool` / `library_version` / `created_at`)
//! - per-dataset declare (`dim_names` / `attrs` / coord label summary)
//! - capped history rows (PROV `agent` / `activity` / `entity`)
//! - non-default `file_execution` prefs
//! - shallow verify (`ok` + finding counts; no deep decode)

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::array::ArrayLayoutSummary;
use super::tensor3d::Tensor3DPlaneStats;
use crate::results::{ColumnarCommonFields, MinimalFallback};

/// File-level footer metadata (`TetMetadataV1.file`).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TetFileFooterMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub library_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

/// One coordinate axis summary (full label lists can be huge — counts + ends only).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TetCoordAxisSummary {
    pub label_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last: Option<String>,
}

/// Declare-only per-dataset footer fields (fold-inert in tetration).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TetDatasetDeclareMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dim_names: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attrs: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub coords: BTreeMap<String, TetCoordAxisSummary>,
}

/// Cap of history rows emitted in JSON (full count stays in `history_event_count`).
pub const MAX_HISTORY_EVENTS_EMITTED: usize = 20;

/// One THST history row (PROV-shaped fields when present).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TetHistoryEventSummary {
    pub op: String,
    pub source: String,
    pub at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity: Option<String>,
}

/// Chunk-index execution prefs (omitted when both fields are engine-default zero).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TetFileExecutionMeta {
    pub memory_budget_percent_bps: u16,
    pub memory_budget_bytes: u32,
}

/// Shallow `tet verify` result (no deep decode).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TetVerifySummary {
    pub ok: bool,
    pub finding_count: usize,
    pub failed_finding_count: usize,
}

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declare: Option<TetDatasetDeclareMeta>,
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
    pub file: Option<TetFileFooterMeta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_execution: Option<TetFileExecutionMeta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history: Option<Vec<TetHistoryEventSummary>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_truncated: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verify: Option<TetVerifySummary>,
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
