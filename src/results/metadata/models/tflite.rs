//! TensorFlow Lite (`.tflite`) — wire summary from `tract_tflite`’s bundled `FlatBuffer` schema. No execution.

use serde::{Deserialize, Serialize};

use crate::results::MinimalFallback;

/// `BuiltinOperator` occurrence counts (most frequent first, capped in the extractor).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TfliteOpTypeCount {
    pub op_type: String,
    pub count: usize,
}

/// Per-subgraph size summary (main graph is typically index 0).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TfliteSubgraphSummary {
    pub index: usize,
    pub input_tensor_indices: Option<usize>,
    pub output_tensor_indices: Option<usize>,
    pub tensor_count: Option<usize>,
    pub operator_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Inspected `TFLite` `FlatBuffer`: schema version, table sizes, op histogram (no inference).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TfliteMetadata {
    pub byte_count: usize,

    /// `FlatBuffer` parse / schema validation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_ok: Option<bool>,

    /// `TFLite` model `version` field (see TensorFlow Lite schema).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<u32>,
    /// Optional `description` string from the file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub operator_code_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subgraph_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buffer_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_metadata_count: Option<usize>,

    /// First N subgraphs with tensor/op counts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subgraphs: Option<Vec<TfliteSubgraphSummary>>,

    /// Builtin op counts over all operators in all subgraphs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub op_type_counts: Option<Vec<TfliteOpTypeCount>>,
}

impl MinimalFallback for TfliteMetadata {
    fn minimal_fallback(file_size_bytes: usize) -> Self {
        Self {
            byte_count: file_size_bytes,
            ..Self::default()
        }
    }
}
