//! MATLAB `.mat` (classic v7) container metadata: named variables and per-array layout + optional column stats.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::results::MinimalFallback;
use crate::results::{BooleanStats, DateStats, NumericStats};

use super::array::ArrayLayoutSummary;

/// One scan of a top-level or nested `struct` / `struct` array / `cell` value: per-nesting field names, leaf dtype line, and global `NaN` / `Inf` in floating (real) samples under the subtree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatStructWalkSummary {
    /// How many `struct` field-name layers were recorded.
    /// When zero, `field_layers` is `None` (JSON `null`).
    pub n_field_layers: usize,
    /// Index 0: field names of the root `struct` (or struct-template); each later index: union of
    /// field names on `struct` nodes at that depth under the same scan root.
    /// `null` in JSON when [`Self::n_field_layers`] is 0.
    pub field_layers: Option<Vec<Vec<String>>>,
    /// Comma + space separated, sorted, unique `dtype` labels of non-struct leaf values in the walk.
    pub leaf_dtypes: String,
    /// Count of `NaN` in real and imag parts of all floating (and complex) samples scanned, capped in the walker.
    pub n_nan: u64,
    /// Count of infinities (including signed) in those samples.
    pub n_inf: u64,
}

/// Global (entry-wide) statistics for non-scalar MATLAB arrays.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatGlobalSummary {
    /// Inferred value kind (`number`, `string`, `date`, `boolean`, `char`, etc).
    pub t: String,
    /// Number of values included in this global profile.
    pub count: usize,
    /// Distinct value count for string/char-ish entries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uniq: Option<usize>,
    /// Numeric distribution for number-like arrays.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num: Option<NumericStats>,
    /// Date span/min/max when values look like dates/timestamps.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<DateStats>,
    /// Boolean ratio when values are logical.
    #[serde(rename = "bool", skip_serializing_if = "Option::is_none")]
    pub bool_stats: Option<BooleanStats>,
    /// NaN count for floating/complex scans.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n_nan: Option<u64>,
    /// Inf count for floating/complex scans.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n_inf: Option<u64>,
}

/// One variable inside a `.mat` file (mirrors [`super::numpy::NpzNpyEntrySummary`] shape).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct MatArrayEntrySummary {
    pub name: String,
    #[serde(flatten)]
    pub layout: ArrayLayoutSummary,
    /// Present for scalar `shape=[1,1]`: direct value instead of computed stats.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    /// Present when this entry is a `struct`, `struct` array, or `cell`: compact field layers, leaf `dtype` line, `n_nan` / `n_inf`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub struct_subtree: Option<MatStructWalkSummary>,
    /// Global stats for non-scalar arrays.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global: Option<MatGlobalSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_parse_error: Option<String>,
}

/// Metadata for a MATLAB `.mat` file (v7 classic; v7.3 HDF5 is flagged without variable listings).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct MatMetadata {
    pub byte_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mat_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variable_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables_scanned: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_parse_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entries: Option<Vec<MatArrayEntrySummary>>,
}

impl MinimalFallback for MatMetadata {
    fn minimal_fallback(file_size_bytes: usize) -> Self {
        Self {
            byte_count: file_size_bytes,
            ..Self::default()
        }
    }
}
