//! ONNX (`.onnx`) — wire `ModelProto` via [`oxionnx_proto::parse_model`], typed I/O via [`onnx_ir::OnnxGraphBuilder`]. No `protoc` build step. IR pass uses the file path.

use std::collections::HashMap;
use std::string::ToString;

use anyhow::Result;
use memmap2::Mmap;
use onnx_ir::ArgType;
use oxionnx_proto::{ModelProto, NodeProto, is_external, parse_model};

use crate::config::RuntimeConfig;
use crate::parsers::ParseResult;
use crate::results::{
    OnnxMetadata, OnnxOpTypeCount, OnnxOpsetEntry, OnnxStringPair, OnnxValueSummary,
};

const MAX_OP_TYPE_STATS: usize = 48;
const MAX_MODEL_METADATA_PROPS: usize = 32;
const MAX_MODEL_DOC_BYTES: usize = 4096;

fn opt_nonempty(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

fn truncate_doc(s: &str) -> String {
    if s.len() <= MAX_MODEL_DOC_BYTES {
        return s.to_string();
    }
    let mut end = MAX_MODEL_DOC_BYTES;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}

fn op_type_histogram(nodes: &[NodeProto]) -> Vec<OnnxOpTypeCount> {
    if nodes.is_empty() {
        return Vec::new();
    }
    let mut m: HashMap<String, usize> = HashMap::new();
    for n in nodes {
        let k = if n.op_type.is_empty() {
            "(empty)".to_string()
        } else {
            n.op_type.clone()
        };
        *m.entry(k).or_insert(0) += 1;
    }
    let mut v: Vec<OnnxOpTypeCount> = m
        .into_iter()
        .map(|(op_type, count)| OnnxOpTypeCount { op_type, count })
        .collect();
    v.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| a.op_type.cmp(&b.op_type))
    });
    v.truncate(MAX_OP_TYPE_STATS);
    v
}

/// Fill fields from a parsed [`ModelProto`]. Does not set IR or `byte_count`.
fn apply_oxionnx_model(out: &mut OnnxMetadata, m: &ModelProto) {
    out.ir_version = Some(m.ir_version);
    out.producer_name = opt_nonempty(m.producer_name.as_str());
    out.producer_version = opt_nonempty(m.producer_version.as_str());
    out.domain = opt_nonempty(m.domain.as_str());
    if m.model_version != 0 {
        out.model_version = Some(m.model_version);
    }
    if !m.doc_string.is_empty() {
        out.model_doc_string = Some(truncate_doc(m.doc_string.as_str()));
    }

    let mut ops: Vec<OnnxOpsetEntry> = m
        .opset_imports
        .iter()
        .map(|o| OnnxOpsetEntry {
            domain: o.domain.clone(),
            version: o.version,
        })
        .collect();
    if ops.is_empty() && m.opset_version != 0 {
        ops.push(OnnxOpsetEntry {
            domain: String::new(),
            version: m.opset_version,
        });
    }
    if !ops.is_empty() {
        out.opset_import = Some(ops);
    }

    if !m.metadata_props.is_empty() {
        out.model_metadata_props = Some(
            m.metadata_props
                .iter()
                .take(MAX_MODEL_METADATA_PROPS)
                .map(|(k, v)| OnnxStringPair {
                    key: k.clone(),
                    value: v.clone(),
                })
                .collect(),
        );
    }

    if !m.training_info.is_empty() {
        out.training_info_count = Some(m.training_info.len());
    }
    // `functions` not represented on oxionnx `ModelProto` — keep `functions_count` unset

    let g = &m.graph;
    if !g.name.is_empty() {
        out.graph_name = Some(g.name.clone());
    }
    out.raw_node_count = Some(g.nodes.len());
    out.initializer_count = Some(g.initializers.len());
    out.graph_input_count = Some(g.inputs.len());
    out.graph_output_count = Some(g.outputs.len());
    out.value_info_count = Some(
        g.input_value_infos
            .len()
            .saturating_add(g.output_value_infos.len()),
    );
    // `GraphProto` here has no sparse-initializer list — keep unset

    let ext_init = g.initializers.iter().filter(|t| is_external(t)).count();
    if ext_init > 0 {
        out.external_initializer_count = Some(ext_init);
    }

    out.op_type_counts = Some(op_type_histogram(&g.nodes));
}

/// Parse wire model + IR: merge when each layer succeeds.
///
/// # Errors
///
/// Infallible: always returns `Ok`; wire and IR issues are reported in the metadata fields.
pub fn extract_onnx_metadata(
    mmap: &Mmap,
    stats: &ParseResult,
    _config: &RuntimeConfig,
) -> Result<OnnxMetadata> {
    let byte_count = stats.byte_count;
    let path = &stats.file_path;

    let mut meta = OnnxMetadata {
        byte_count,
        ..OnnxMetadata::default()
    };

    match parse_model(mmap.as_ref()) {
        Ok(m) => {
            apply_oxionnx_model(&mut meta, &m);
        }
        Err(e) => {
            log::debug!("oxionnx-proto parse_model failed for {path}: {e}");
            meta.proto_parse_error = Some(e);
        }
    }

    match onnx_ir::OnnxGraphBuilder::new().parse_file(path) {
        Ok(graph) => {
            let graph_inputs: Vec<OnnxValueSummary> = graph
                .inputs
                .iter()
                .map(|a| OnnxValueSummary {
                    name: a.name.clone(),
                    type_summary: Some(argtype_summary(&a.ty)),
                })
                .collect();
            let graph_outputs: Vec<OnnxValueSummary> = graph
                .outputs
                .iter()
                .map(|a| OnnxValueSummary {
                    name: a.name.clone(),
                    type_summary: Some(argtype_summary(&a.ty)),
                })
                .collect();

            meta.graph_node_count = Some(graph.nodes.len());
            meta.graph_inputs = Some(graph_inputs);
            meta.graph_outputs = Some(graph_outputs);
            meta.parse_error = None;
            meta.parse_ok = Some(true);
        }
        Err(e) => {
            let msg = e.to_string();
            log::debug!("onnx-ir failed for {path}: {msg}");
            meta.parse_error = Some(msg);
            meta.parse_ok = Some(false);
        }
    }

    Ok(meta)
}

fn argtype_summary(ty: &ArgType) -> String {
    match ty {
        ArgType::ScalarTensor(d) => format!("scalar_tensor({d:?})"),
        ArgType::ScalarNative(d) => format!("scalar_native({d:?})"),
        ArgType::Shape(r) => format!("shape(rank={r})"),
        ArgType::Tensor(t) => {
            let shape = t.static_shape.as_ref().map_or_else(
                || "?".to_string(),
                |s| {
                    s.iter()
                        .map(|dim| {
                            dim.map_or_else(|| "?".to_string(), |d| d.to_string())
                        })
                        .collect::<Vec<_>>()
                        .join("×")
                },
            );
            format!("tensor({:?}, rank {}, [{}])", t.dtype, t.rank, shape)
        }
    }
}

crate::no_template_mining!(
    extract_onnx_templates,
    "ONNX is a serialized computation graph; no line-oriented template mining."
);
