//! Python pickle (`.pkl`, `.pickle`) — read-only opcode scan; no unpickling.
//!
//! Output is produced by [`crate::parsers::structured::pickle::extract_pickle_metadata`].
//! `content_hint` values: `tabular` (pandas), `ml_model` (sklearn/torch/xgboost),
//! `numeric_array` (numpy), `builtin_containers` (plain dict/list/tuple only).

use serde::{Deserialize, Serialize};

use crate::results::MinimalFallback;

/// Inspected pickle file from header sniff and opcode walk (no unpickling).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct PickleMetadata {
    pub byte_count: usize,
    /// First-seen `\x80` + protocol header, when present at file start.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<u8>,
    /// `binary` (protocol 2+ or non-UTF-8) or `text` (protocol 0/1 ASCII).
    pub encoding: String,
    /// Every `PROTO` opcode observed during scan.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub protocols_seen: Vec<u8>,
    /// Number of `FRAME` (0x95) opcodes and total declared frame payload bytes.
    pub frame_count: usize,
    pub frame_bytes_total: u64,
    /// Deduplicated `module.name` from `GLOBAL`, `INST`, and `STACK_GLOBAL`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub referenced_globals: Vec<String>,
    /// Subset of globals under `builtins.*`, `__builtin__.*`, or `collections.*`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub builtin_types: Vec<String>,
    /// Heuristic label from referenced globals (e.g. `ml_model`, `tabular`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hint: Option<String>,
    /// True when the opcode walk hit its opcode cap or stopped on an unknown/truncated operand.
    pub scan_truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scan_error: Option<String>,
}

impl MinimalFallback for PickleMetadata {
    fn minimal_fallback(file_size_bytes: usize) -> Self {
        Self {
            byte_count: file_size_bytes,
            ..Self::default()
        }
    }
}
