//! Matrix Market `.mtx` — format-specific fields plus column stats (numeric pipeline; no CSV string inference).

use serde::{Deserialize, Serialize};

use crate::results::ColumnarCommonFields;
use crate::results::MinimalFallback;

/// Parsed Matrix Market summary (sparse coordinate or dense array) and column metadata.
///
/// Logical matrix size is [`ColumnarCommonFields::row_count`] and [`ColumnarCommonFields::column_count`]
/// (serialized at the top level via `common`).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct MtxMetadata {
    pub byte_count: usize,
    /// `coordinate` or `array` in Matrix Market terms; exposed as `sparse` / `dense` for JSON.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage: Option<String>,
    /// Number of stored entries (nonzeros for sparse; `row_count` × `column_count` for dense).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_stored_values: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symmetry: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_error: Option<String>,
    /// `true` when [`crate::results::ColumnStat::num`] reflects **full logical columns** (implicit zeros included in sparse mean/min/max/stdev, or exact sorted stats when the matrix is materialized).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub numeric_stats_include_implicit_zeros: Option<bool>,
    #[serde(flatten)]
    pub common: ColumnarCommonFields,
}

impl MinimalFallback for MtxMetadata {
    fn minimal_fallback(file_size_bytes: usize) -> Self {
        Self {
            byte_count: file_size_bytes,
            common: ColumnarCommonFields::default(),
            ..Self::default()
        }
    }
}
