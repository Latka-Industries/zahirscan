//! TensorFlow Lite (`.tflite`) — [`tract_tflite::tflite::root_as_model`] over mmap. No `Interpreter`; metadata only.

use std::collections::HashMap;

use anyhow::Result;
use memmap2::Mmap;
use tract_tflite::tflite;

use crate::config::RuntimeConfig;
use crate::parsers::ParseResult;
use crate::results::{TfliteMetadata, TfliteOpTypeCount, TfliteSubgraphSummary};

const MAX_OP_TYPE_STATS: usize = 48;
const MAX_SUBGRAPH_REPORT: usize = 32;

/// Parse `TFLite` `FlatBuffer`; histogram walks all [`tflite::SubGraph`] operators.
///
/// # Errors
///
/// Infallible: always returns `Ok` with `parse_error` / `parse_ok` on invalid flatbuffers.
pub fn extract_tflite_metadata(
    mmap: &Mmap,
    stats: &ParseResult,
    _config: &RuntimeConfig,
) -> Result<TfliteMetadata> {
    let byte_count = stats.byte_count;
    let mut meta = TfliteMetadata {
        byte_count,
        ..TfliteMetadata::default()
    };

    let model = match tflite::root_as_model(mmap.as_ref()) {
        Ok(m) => m,
        Err(e) => {
            let msg = e.to_string();
            log::debug!("tflite root_as_model failed for {}: {msg}", stats.file_path);
            meta.parse_error = Some(msg);
            meta.parse_ok = Some(false);
            return Ok(meta);
        }
    };

    meta.parse_ok = Some(true);
    meta.parse_error = None;
    meta.schema_version = Some(model.version());
    meta.description = model.description().map(std::string::ToString::to_string);

    if let Some(codes) = model.operator_codes() {
        meta.operator_code_count = Some(codes.len());
    }
    if let Some(bufs) = model.buffers() {
        meta.buffer_count = Some(bufs.len());
    }
    if let Some(md) = model.metadata() {
        meta.model_metadata_count = Some(md.len());
    }

    if let Some(sgs) = model.subgraphs() {
        meta.subgraph_count = Some(sgs.len());
        let mut out: Vec<TfliteSubgraphSummary> = Vec::new();
        for i in 0..sgs.len().min(MAX_SUBGRAPH_REPORT) {
            let sg = sgs.get(i);
            // `as_ref()` yields `&flatbuffers::vector::Vector<…>`; clippy’s `map(Vector::len)` wants
            // `flatbuffers::Vector::len` — a different function item, so keep the closure.
            #[allow(clippy::redundant_closure_for_method_calls)]
            out.push(TfliteSubgraphSummary {
                index: i,
                input_tensor_indices: sg.inputs().as_ref().map(|v| v.len()),
                output_tensor_indices: sg.outputs().as_ref().map(|v| v.len()),
                tensor_count: sg.tensors().as_ref().map(|v| v.len()),
                operator_count: sg.operators().as_ref().map(|v| v.len()),
                name: sg.name().map(std::string::ToString::to_string),
            });
        }
        if !out.is_empty() {
            meta.subgraphs = Some(out);
        }
    }

    if let (Some(codes), Some(sgs)) = (model.operator_codes(), model.subgraphs()) {
        let mut m: HashMap<String, usize> = HashMap::new();
        for sgi in 0..sgs.len() {
            let sg = sgs.get(sgi);
            if let Some(ops) = sg.operators() {
                for oi in 0..ops.len() {
                    let op = ops.get(oi);
                    let idx = op.opcode_index() as usize;
                    let name = if idx < codes.len() {
                        format!("{:?}", codes.get(idx).builtin_code())
                    } else {
                        format!("(bad_opcode_index:{})", op.opcode_index())
                    };
                    *m.entry(name).or_insert(0) += 1;
                }
            }
        }
        if !m.is_empty() {
            let mut v: Vec<TfliteOpTypeCount> = m
                .into_iter()
                .map(|(op_type, count)| TfliteOpTypeCount { op_type, count })
                .collect();
            v.sort_by(|a, b| {
                b.count
                    .cmp(&a.count)
                    .then_with(|| a.op_type.cmp(&b.op_type))
            });
            v.truncate(MAX_OP_TYPE_STATS);
            meta.op_type_counts = Some(v);
        }
    }

    Ok(meta)
}

crate::no_template_mining!(
    extract_tflite_templates,
    "`TFLite` is a flatbuffer graph; no line-oriented template mining."
);
