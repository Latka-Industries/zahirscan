//! `GGUF` (`.gguf`) — `ggml` universal format: metadata + tensor table via [`gguf_rs`]. No execution.

use serde::{Deserialize, Serialize};

use crate::results::MinimalFallback;

/// One tensor entry in a GGUF weight file (name, quantization kind, size, shape).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct GgufTensorSummary {
    pub name: String,
    /// Raw GGML type id in the file.
    pub kind: u32,
    /// Short name for [`kind`], if it maps to a known [`gguf_rs::GGMLType`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind_name: Option<String>,
    /// Declared on-disk / logical size in bytes (from the format reader).
    pub size: u64,
    /// Tensor dimensions (padded in source to four slots; we keep the full vector as reported).
    pub shape: Vec<u64>,
}

/// Inspected `GGUF` file: high-level `general.*` style fields, capped KV and tensor tables.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct GgufMetadata {
    pub byte_count: usize,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_error: Option<String>,
    /// `true` when [`gguf_rs::GGUFContainer::decode`] succeeded and summary fields are populated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_ok: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// From `general.architecture` (or `unknown` when missing).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_family: Option<String>,
    /// From `general.file_type` (quantization / layout family label in GGUF, not a filename extension).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gguf_file_type: Option<String>,
    /// Human-readable parameter count (from reader accumulation).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_parameters: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_kv: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_tensor: Option<u64>,

    /// Subset of string-keyed metadata (capped) as JSON, safe for `tokenizer.*` and similar arrays (reader caps array length).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kv: Option<serde_json::Value>,

    /// First N tensors, for layout / naming (weights not loaded).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tensor_summaries: Option<Vec<GgufTensorSummary>>,
}

impl MinimalFallback for GgufMetadata {
    fn minimal_fallback(file_size_bytes: usize) -> Self {
        Self {
            byte_count: file_size_bytes,
            ..Self::default()
        }
    }
}
