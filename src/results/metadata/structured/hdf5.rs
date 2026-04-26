//! HDF5 (`.h5`, `.hdf5`) hierarchical layout metadata (no `libhdf5`).

use serde::{Deserialize, Serialize};

use crate::results::MinimalFallback;

/// One dataset discovered while walking the file.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Hdf5DatasetSummary {
    /// Absolute path from root (e.g. `/matrix`, `/group1/aux`).
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape: Option<Vec<u64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub datatype_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inspect_error: Option<String>,
}

/// Metadata for an HDF5 file: superblock, root listing, and a bounded tree walk.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Hdf5Metadata {
    pub byte_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub superblock_version: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_member_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_dataset_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_attribute_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub groups_visited: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub datasets_visited: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub datasets: Option<Vec<Hdf5DatasetSummary>>,
    /// True when depth or dataset caps were hit while walking.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub walk_truncated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_error: Option<String>,
}

impl MinimalFallback for Hdf5Metadata {
    fn minimal_fallback(file_size_bytes: usize) -> Self {
        Self {
            byte_count: file_size_bytes,
            ..Self::default()
        }
    }
}
