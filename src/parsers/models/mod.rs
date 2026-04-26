//! Model file formats: ONNX (`.onnx`), GGUF (`.gguf`), TensorFlow Lite (`.tflite`), and safetensors (`.safetensors`).

pub mod gguf;
pub mod onnx;
pub mod safetensors;
pub mod tflite;

pub use gguf::{extract_gguf_metadata, extract_gguf_templates};
pub use onnx::{extract_onnx_metadata, extract_onnx_templates};
pub use safetensors::{extract_safetensors_metadata, extract_safetensors_templates};
pub use tflite::{extract_tflite_metadata, extract_tflite_templates};

use anyhow::Result;
use memmap2::Mmap;

use crate::config::RuntimeConfig;
use crate::parsers::{FileType, ParseResult};
use crate::results::MiningResult;

/// Dispatch by file type; metadata only (no line-oriented template mining).
///
/// # Errors
///
/// Propagates format or I/O errors from the underlying extractor or template-mining hook (stubs for model formats return `Ok`).
pub fn process(
    stats: &mut ParseResult,
    mmap: &Mmap,
    config: &RuntimeConfig,
) -> Result<MiningResult> {
    match stats.file_type {
        FileType::Onnx => {
            crate::process_with_metadata!(
                stats,
                mmap,
                config,
                onnx_metadata,
                extract_onnx_metadata(mmap, stats, config),
                crate::results::OnnxMetadata,
                FileType::Onnx,
                extract_onnx_templates(mmap, stats, config)
            )
        }
        FileType::Gguf => {
            crate::process_with_metadata!(
                stats,
                mmap,
                config,
                gguf_metadata,
                extract_gguf_metadata(mmap, stats, config),
                crate::results::GgufMetadata,
                FileType::Gguf,
                extract_gguf_templates(mmap, stats, config)
            )
        }
        FileType::Tflite => {
            crate::process_with_metadata!(
                stats,
                mmap,
                config,
                tflite_metadata,
                extract_tflite_metadata(mmap, stats, config),
                crate::results::TfliteMetadata,
                FileType::Tflite,
                extract_tflite_templates(mmap, stats, config)
            )
        }
        FileType::Safetensors => {
            crate::process_with_metadata!(
                stats,
                mmap,
                config,
                safetensors_metadata,
                extract_safetensors_metadata(mmap, stats, config),
                crate::results::SafetensorsMetadata,
                FileType::Safetensors,
                extract_safetensors_templates(mmap, stats, config)
            )
        }
        _ => unreachable!("models::process called with {:?}", stats.file_type),
    }
}
