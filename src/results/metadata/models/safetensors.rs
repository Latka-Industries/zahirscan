//! Safetensors (`.safetensors`) — header JSON and tensor index via [`safetensors::SafeTensors`]. No execution.

use serde::{Deserialize, Serialize};

use crate::results::MinimalFallback;

/// One tensor entry in a safetensors file (name, dtype, shape, payload size).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SafetensorTensorSummary {
    pub name: String,
    /// Element type label (same as `Debug` on the on-wire dtype enum).
    pub dtype: String,
    pub shape: Vec<usize>,
    /// Byte length of the tensor payload in the file.
    pub data_bytes: usize,
}

/// Per-dtype occurrence count over all tensors in the file.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SafetensorsDtypeCount {
    pub dtype: String,
    pub count: usize,
}

/// Inspected `.safetensors` file: optional `__metadata__` map, per-tensor layout (no materialized weights).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SafetensorsMetadata {
    pub byte_count: usize,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_ok: Option<bool>,

    /// `__metadata__` from the file header, when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_metadata: Option<serde_json::Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tensor_count: Option<usize>,

    /// First N tensors in offset order.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tensors: Option<Vec<SafetensorTensorSummary>>,

    /// Dtype counts (most common first, capped in the extractor).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dtype_counts: Option<Vec<SafetensorsDtypeCount>>,
}

impl MinimalFallback for SafetensorsMetadata {
    fn minimal_fallback(file_size_bytes: usize) -> Self {
        Self {
            byte_count: file_size_bytes,
            ..Self::default()
        }
    }
}
