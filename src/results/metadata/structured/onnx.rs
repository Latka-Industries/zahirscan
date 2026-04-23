//! Open Neural Network Exchange (`.onnx`) — wire summary from `oxionnx-proto` and typed I/O from `onnx-ir`. No execution.

use serde::{Deserialize, Serialize};

use crate::results::MinimalFallback;

/// One opset import entry (ONNX / extension domain + version). Set when a future wire parser fills it.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct OnnxOpsetEntry {
    /// Empty string denotes the default `onnx` domain.
    pub domain: String,
    pub version: i64,
}

/// Key/value model metadata. Set when a future wire parser fills it.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct OnnxStringPair {
    pub key: String,
    pub value: String,
}

/// `op_type` with occurrence count in the main graph.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct OnnxOpTypeCount {
    pub op_type: String,
    pub count: usize,
}

/// One graph input or output value (name plus inferred type information when present).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct OnnxValueSummary {
    /// ONNX value name.
    pub name: String,
    /// Short description of the argument type (e.g. tensor dtype / rank, shape).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_summary: Option<String>,
}

/// Inspected ONNX model: protobuf file header + raw graph, plus optional IR view.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct OnnxMetadata {
    pub byte_count: usize,

    // --- `oxionnx-proto` (`parse_model`; not all optional fields are populated) ---
    /// Fails if [`oxionnx_proto::parse_model`] could not decode the file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proto_parse_error: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ir_version: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub producer_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub producer_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_version: Option<i64>,
    /// Model `doc_string` (may be long; still bounded at source if desired).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_doc_string: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub opset_import: Option<Vec<OnnxOpsetEntry>>,

    /// First N model metadata key/value pairs (only if a wire parser provides them).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_metadata_props: Option<Vec<OnnxStringPair>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub training_info_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub functions_count: Option<usize>,

    /// From the structural parser (not necessarily raw `GraphProto` only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_node_count: Option<usize>,
    /// Count of `initializer` entries in the parsed `GraphProto`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initializer_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sparse_initializer_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_info_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph_input_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph_output_count: Option<usize>,

    /// Initializer tensors that store data outside the protobuf (external files).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_initializer_count: Option<usize>,

    /// `op_type` counts, most frequent first, capped in the extractor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub op_type_counts: Option<Vec<OnnxOpTypeCount>>,

    // --- [`onnx_ir`] (type inference; may fail independently of wire parse) ---
    /// Number of computational nodes in the main graph after IR conversion.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph_node_count: Option<usize>,
    /// Graph inputs (model interface) with inferred types.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph_inputs: Option<Vec<OnnxValueSummary>>,
    /// Graph outputs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph_outputs: Option<Vec<OnnxValueSummary>>,

    /// Error from [`onnx_ir::OnnxGraphBuilder`], if IR conversion failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_error: Option<String>,
    /// `true` when IR conversion succeeded and interface fields are populated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_ok: Option<bool>,
}

impl MinimalFallback for OnnxMetadata {
    fn minimal_fallback(file_size_bytes: usize) -> Self {
        Self {
            byte_count: file_size_bytes,
            ..Self::default()
        }
    }
}
