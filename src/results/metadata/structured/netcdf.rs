//! NetCDF (`.nc`, `.cdf`) — global attributes and per-variable metadata (no array decode).

use serde::{Deserialize, Serialize};

use crate::results::MinimalFallback;

/// One attribute name and a short string representation of its value.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct NetCdfAttributeEntry {
    pub name: String,
    pub value: String,
}

/// One variable: name, shape, type summary, and a bounded attribute list.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct NetCdfVariableSummary {
    /// Path-style name (e.g. `temperature` or `group/sub/var` for NetCDF-4 groups).
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape: Option<Vec<usize>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimension_names: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vartype: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Vec<NetCdfAttributeEntry>>,
}

/// Metadata for a NetCDF file: globals, dimensions, variables (bounded).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct NetCdfMetadata {
    pub byte_count: usize,
    /// `true` when the file uses the NetCDF-4/HDF5 model ([`netcdf::File::root`] is present).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub netcdf4_model: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_attributes: Option<Vec<NetCdfAttributeEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimension_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions_sample: Option<Vec<(String, usize)>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variable_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<Vec<NetCdfVariableSummary>>,
    /// True when variable or attribute caps were hit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata_truncated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_error: Option<String>,
}

impl MinimalFallback for NetCdfMetadata {
    fn minimal_fallback(file_size_bytes: usize) -> Self {
        Self {
            byte_count: file_size_bytes,
            ..Self::default()
        }
    }
}
