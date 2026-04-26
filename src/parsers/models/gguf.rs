//! `GGUF` (`.gguf`) — key/value metadata and tensor table via [`gguf_rs`]. We open by path (library API) after Phase 1 mmap.

use anyhow::Result;
use memmap2::Mmap;

use crate::config::RuntimeConfig;
use crate::parsers::ParseResult;
use crate::results::{GgufMetadata, GgufTensorSummary};

use gguf_rs::{GGMLType, get_gguf_container_array_size};
use serde_json::Value;

const GGUF_READER_MAX_ARRAY: u64 = 256;
const MAX_KV_JSON_KEYS: usize = 64;
const MAX_TENSOR_SUMMARIES: usize = 128;

fn kind_name(kind: u32) -> Option<String> {
    GGMLType::try_from(kind).ok().map(|k| k.to_string())
}

/// Parse GGUF; uses [`get_gguf_container_array_size`] on disk path (matches `gguf-rs` API; mmap is not used for decode).
///
/// # Errors
///
/// Infallible: always returns `Ok` with `parse_error` / `parse_ok` set on decode failure (same pattern as other model extractors using [`anyhow::Result`] for API consistency).
pub fn extract_gguf_metadata(
    _mmap: &Mmap,
    stats: &ParseResult,
    _config: &RuntimeConfig,
) -> Result<GgufMetadata> {
    let path = &stats.file_path;
    let byte_count = stats.byte_count;

    let mut meta = GgufMetadata {
        byte_count,
        ..GgufMetadata::default()
    };

    let mut container = match get_gguf_container_array_size(path, GGUF_READER_MAX_ARRAY) {
        Ok(c) => c,
        Err(e) => {
            let msg = e.to_string();
            log::debug!("get_gguf_container failed for {path}: {msg}");
            meta.parse_error = Some(msg);
            meta.parse_ok = Some(false);
            return Ok(meta);
        }
    };

    let model = match container.decode() {
        Ok(m) => m,
        Err(e) => {
            let msg = e.to_string();
            log::debug!("gguf decode failed for {path}: {msg}");
            meta.parse_error = Some(msg);
            meta.parse_ok = Some(false);
            return Ok(meta);
        }
    };

    meta.parse_ok = Some(true);
    meta.parse_error = None;
    meta.version = Some(model.get_version());
    meta.model_family = Some(model.model_family());
    meta.gguf_file_type = Some(model.file_type());
    meta.model_parameters = Some(model.model_parameters());
    meta.num_kv = Some(model.num_kv());
    meta.num_tensor = Some(model.num_tensor());

    let kvs = model.metadata();
    let mut m = serde_json::Map::new();
    for (i, (k, v)) in kvs.iter().enumerate() {
        if i >= MAX_KV_JSON_KEYS {
            break;
        }
        m.insert(k.clone(), v.clone());
    }
    meta.kv = if m.is_empty() {
        None
    } else {
        Some(Value::Object(m))
    };

    let summaries: Vec<GgufTensorSummary> = model
        .tensors()
        .iter()
        .take(MAX_TENSOR_SUMMARIES)
        .map(|t| GgufTensorSummary {
            name: t.name.clone(),
            kind: t.kind,
            kind_name: kind_name(t.kind),
            size: t.size,
            shape: t.shape.clone(),
        })
        .collect();
    if !summaries.is_empty() {
        meta.tensor_summaries = Some(summaries);
    }

    Ok(meta)
}

crate::no_template_mining!(
    extract_gguf_templates,
    "GGUF stores tensors; no line-oriented template mining."
);
