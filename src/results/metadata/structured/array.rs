//! Layout metadata shared by homogeneous array containers (`NumPy` `.npy` / `.npz`, MATLAB `.mat`, etc.).

use serde::{Deserialize, Serialize};

/// Parsed header / layout for a single dense array variable (dtype, shape, data region bounds).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ArrayLayoutSummary {
    /// Container-specific format version (e.g. `NumPy` `1.0` / `2.0` header, MAT level).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format_version: Option<String>,
    /// Element type description: `NumPy` `descr` string, MATLAB class name, HDF5 dtype label, etc.
    #[serde(skip_serializing_if = "Option::is_none", alias = "descr")]
    pub dtype: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape: Option<Vec<usize>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fortran_order: Option<bool>,
    /// Size in bytes of any header/prefix region before raw array bytes (`NumPy` header dict; optional elsewhere).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_region_bytes: Option<usize>,
    /// Byte offset where raw array data begins.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_offset: Option<usize>,
    /// Bytes from `data_offset` through end of the logical payload (file or archive member size).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_region_bytes: Option<usize>,
    /// `itemsize * num_elements` when both can be inferred from `dtype` and `shape`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_data_bytes_from_dtype: Option<usize>,
}

impl ArrayLayoutSummary {
    /// Rank 0–2 shapes as `(rows, cols)` for tabular sampling; `None` if rank is above two.
    #[must_use]
    pub fn table_dims(&self) -> Option<(usize, usize)> {
        let shape = self.shape.as_deref()?;
        match shape.len() {
            0 => Some((1, 1)),
            1 => Some((shape[0], 1)),
            2 => Some((shape[0], shape[1])),
            _ => None,
        }
    }

    /// `(row_count, column_count)` for stats: rank ≤ 2 uses [`Self::table_dims`]; higher rank uses flat `prod` × 0.
    #[must_use]
    pub fn shape_row_col_counts(&self) -> Option<(usize, usize)> {
        let shape = self.shape.as_deref()?;
        if let Some((r, c)) = self.table_dims() {
            Some((r, c))
        } else {
            let n: usize = shape.iter().product();
            Some((n, 0))
        }
    }
}

/// Backward-compatible alias used when the name “`NumPy` layout” reads clearer than “array layout”.
pub type NpyLayoutSummary = ArrayLayoutSummary;
