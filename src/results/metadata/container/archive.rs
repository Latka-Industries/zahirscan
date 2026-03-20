//! TAR and compressed TAR (tar.gz, tgz, tar.bz2, tar.xz) metadata structures

use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

use crate::results::MinimalFallback;

/// Single entry in a TAR (or compressed TAR) archive
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ArchiveEntry {
    /// Entry path as stored in the archive
    pub path: String,
    /// Uncompressed size in bytes
    pub size: u64,
}

/// TAR / compressed TAR archive metadata
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ArchiveMetadata {
    /// Format: "tar", "tar.gz", "tgz", "tar.bz2", "tar.xz"
    pub format: Option<String>,
    /// Number of file entries (directories may be included in count)
    pub file_count: Option<usize>,
    /// Entry path and size
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entries: Option<Vec<ArchiveEntry>>,
    /// Compressed file size (on-disk) in bytes
    pub compressed_size: Option<u64>,
    /// Total uncompressed size in bytes (sum of entry sizes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uncompressed_size: Option<u64>,
    /// Reserved; not currently set (plain .tar is zero-copy, no truncation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncated: Option<bool>,
    /// Note when full listing is not available (compressed TAR: no decompression; use plain .tar for `file_count/entries`)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl Serialize for ArchiveMetadata {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("ArchiveMetadata", 7)?;
        crate::serialize_optional!(state, self.format, "format");
        crate::serialize_optional!(state, self.file_count, "file_count");
        crate::serialize_optional!(state, self.entries, "entries");
        crate::serialize_optional!(state, self.compressed_size, "compressed_size");
        crate::serialize_optional!(state, self.uncompressed_size, "uncompressed_size");
        crate::serialize_optional!(state, self.truncated, "truncated");
        crate::serialize_optional!(state, self.note, "note");
        state.end()
    }
}

impl MinimalFallback for ArchiveMetadata {
    fn minimal_fallback(file_size_bytes: usize) -> Self {
        Self {
            compressed_size: Some(file_size_bytes as u64),
            ..Default::default()
        }
    }
}
