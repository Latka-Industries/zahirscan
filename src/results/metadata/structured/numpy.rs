//! `NumPy` `.npy` / `.npz` on-disk layout metadata plus optional CSV-like column statistics.

use serde::{Deserialize, Serialize};

use crate::results::ColumnarCommonFields;
use crate::results::MinimalFallback;

/// Parsed header / layout fields shared by a standalone `.npy` file and each `.npy` member inside an `.npz`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct NpyLayoutSummary {
    /// `1.0` (16-bit header length) or `2.0` (32-bit header length).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub descr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape: Option<Vec<usize>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fortran_order: Option<bool>,
    /// Size in bytes of the header region (dict + padding + newline), as stored after the length field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_region_bytes: Option<usize>,
    /// Byte offset where raw array data begins.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_offset: Option<usize>,
    /// Bytes from `data_offset` through end of the logical `.npy` payload (file or zip entry size).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_region_bytes: Option<usize>,
    /// `itemsize * num_elements` when both can be inferred from `descr` and `shape`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_data_bytes_from_dtype: Option<usize>,
}

/// Metadata for a single `NumPy` `.npy` array file (header, layout, and sampled column stats when applicable).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct NpyMetadata {
    pub byte_count: usize,
    #[serde(flatten)]
    pub layout: NpyLayoutSummary,
    /// Row/column counts and CSV-like stats when dtype is not object/structured and rank ≤ 2.
    #[serde(flatten)]
    pub common: ColumnarCommonFields,
}

impl MinimalFallback for NpyMetadata {
    fn minimal_fallback(file_size_bytes: usize) -> Self {
        Self {
            byte_count: file_size_bytes,
            common: ColumnarCommonFields::default(),
            ..Self::default()
        }
    }
}

/// One `.npy` member inside an `.npz` archive.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct NpzNpyEntrySummary {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uncompressed_size: Option<u64>,
    #[serde(flatten)]
    pub layout: NpyLayoutSummary,
    #[serde(flatten)]
    pub common: ColumnarCommonFields,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_parse_error: Option<String>,
}

/// Metadata for a `NumPy` `.npz` archive (ZIP listing + per-`.npy` header summaries).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct NpzMetadata {
    pub byte_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zip_entry_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub npy_entries_scanned: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub npy_entries: Option<Vec<NpzNpyEntrySummary>>,
}

impl MinimalFallback for NpzMetadata {
    fn minimal_fallback(file_size_bytes: usize) -> Self {
        Self {
            byte_count: file_size_bytes,
            ..Self::default()
        }
    }
}
