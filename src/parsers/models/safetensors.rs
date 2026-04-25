//! Hugging Face safetensors (`.safetensors`) — [`SafeTensors::deserialize`](safetensors::SafeTensors::deserialize) over mmap. Weights are not executed.

use std::collections::HashMap;

use anyhow::Result;
use memmap2::Mmap;
use safetensors::SafeTensors;

use crate::config::RuntimeConfig;
use crate::parsers::ParseResult;
use crate::results::{SafetensorTensorSummary, SafetensorsDtypeCount, SafetensorsMetadata};

const MAX_TENSOR_REPORT: usize = 128;
const MAX_DTYPE_COUNTS: usize = 32;

/// Parse a single `.safetensors` shard. For multi-shard models, the hub index `JSON` is a separate file (not this extension).
///
/// # Errors
///
/// Infallible: always returns `Ok` with `parse_error` / `parse_ok` on bad buffers.
pub fn extract_safetensors_metadata(
    mmap: &Mmap,
    stats: &ParseResult,
    _config: &RuntimeConfig,
) -> Result<SafetensorsMetadata> {
    let byte_count = stats.byte_count;
    let mut meta = SafetensorsMetadata {
        byte_count,
        ..SafetensorsMetadata::default()
    };

    let st = match SafeTensors::deserialize(mmap.as_ref()) {
        Ok(s) => s,
        Err(e) => {
            let msg = e.to_string();
            log::debug!(
                "SafeTensors::deserialize failed for {}: {msg}",
                stats.file_path
            );
            meta.parse_error = Some(msg);
            meta.parse_ok = Some(false);
            return Ok(meta);
        }
    };

    meta.parse_ok = Some(true);
    meta.parse_error = None;
    meta.tensor_count = Some(st.len());

    if let Ok((_, file_meta)) = SafeTensors::read_metadata(mmap.as_ref())
        && let Some(m) = file_meta.metadata().as_ref()
    {
        meta.header_metadata = serde_json::to_value(m).ok();
    }

    let mut dtype_hist: HashMap<String, usize> = HashMap::new();
    let mut summaries: Vec<SafetensorTensorSummary> = Vec::new();
    for (i, (name, view)) in st.iter().enumerate() {
        let dtype_label = format!("{:?}", view.dtype());
        *dtype_hist.entry(dtype_label.clone()).or_insert(0) += 1;
        if i < MAX_TENSOR_REPORT {
            summaries.push(SafetensorTensorSummary {
                name: name.to_string(),
                dtype: dtype_label,
                shape: view.shape().to_vec(),
                data_bytes: view.data().len(),
            });
        }
    }

    if !summaries.is_empty() {
        meta.tensors = Some(summaries);
    }

    if !dtype_hist.is_empty() {
        let mut v: Vec<SafetensorsDtypeCount> = dtype_hist
            .into_iter()
            .map(|(dtype, count)| SafetensorsDtypeCount { dtype, count })
            .collect();
        v.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.dtype.cmp(&b.dtype)));
        v.truncate(MAX_DTYPE_COUNTS);
        meta.dtype_counts = Some(v);
    }

    Ok(meta)
}

crate::no_template_mining!(
    extract_safetensors_templates,
    "Safetensors stores dense tensors; no line-oriented template mining."
);
