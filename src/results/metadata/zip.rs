//! ZIP archive metadata structures

use std::collections::BTreeMap;

use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

use crate::results::MinimalFallback;

/// Single entry in a ZIP archive (path, sizes, and header-only metadata; no decompression).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ZipEntry {
    /// Entry path as stored in the archive
    pub path: String,
    /// Uncompressed size in bytes
    pub uncompressed_size: Option<u64>,
    /// Compressed size in bytes (from ZIP header)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compressed_size: Option<u64>,
    /// Detected type from extension (e.g. "Pdf", "Image"); best-effort, no decompression.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detected_type: Option<String>,
    /// Last modified from ZIP header (e.g. "2025-01-15 10:30:00")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<String>,
    /// Compression method (e.g. "Deflate", "Stored")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression_method: Option<String>,
}

/// ZIP archive metadata
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ZipMetadata {
    /// Archive file size in bytes
    pub file_size: Option<usize>,
    /// Number of file entries (directories and filtered OS junk omitted)
    pub file_count: Option<usize>,
    /// Entry paths and header-only metadata (no decompression)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entries: Option<Vec<ZipEntry>>,
    /// Total uncompressed size in bytes (sum of all entry sizes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_uncompressed: Option<u64>,
    /// Total compressed size in bytes (sum of compressed_size of entries)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_compressed: Option<u64>,
    /// Count of entries by detected_type (e.g. {"Pdf": 42, "Image": 1})
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_type_counts: Option<BTreeMap<String, usize>>,
    /// Archive comment if present
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

impl Serialize for ZipMetadata {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("ZipMetadata", 7)?;
        crate::serialize_optional!(state, self.file_size, "file_size");
        crate::serialize_optional!(state, self.file_count, "file_count");
        crate::serialize_optional!(state, self.entries, "entries");
        crate::serialize_optional!(state, self.total_uncompressed, "total_uncompressed");
        crate::serialize_optional!(state, self.total_compressed, "total_compressed");
        crate::serialize_optional!(state, self.entry_type_counts, "entry_type_counts");
        crate::serialize_optional!(state, self.comment, "comment");
        state.end()
    }
}

crate::impl_minimal_fallback!(ZipMetadata, file_size);
