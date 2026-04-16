//! Columnar binary formats: Parquet, Arrow IPC / Feather, Avro, ORC.
//!
//! Shared table fields live on [`ColumnarCommonFields`]; each format struct composes that
//! (via `#[serde(flatten)]` for a flat JSON shape) and adds only format-specific columns.

use serde::{Deserialize, Serialize};

use crate::results::MinimalFallback;
use crate::results::{BooleanStats, DateStats, NumericStats};

/// Shared table-oriented fields (aligned with [`super::CsvMetadata`] statistics).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ColumnarCommonFields {
    pub row_count: usize,
    pub column_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_names: Option<Vec<String>>,
    /// Inferred CSV-like types from sampled cell strings (`number`, `date`, …).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_types: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub null_percentages: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unique_counts: Option<Vec<usize>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub numeric_stats: Option<Vec<Option<NumericStats>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_stats: Option<Vec<Option<DateStats>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boolean_stats: Option<Vec<Option<BooleanStats>>>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ParquetMetadata {
    #[serde(flatten)]
    pub common: ColumnarCommonFields,
    /// Physical Arrow / Parquet-style type names per field (e.g. `Int32`, `Utf8`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arrow_field_types: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_row_groups: Option<usize>,
}

crate::impl_minimal_fallback!(ParquetMetadata, _);

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ArrowIpcMetadata {
    #[serde(flatten)]
    pub common: ColumnarCommonFields,
    /// `ipc_file`, `ipc_stream`, or `feather` (when magic / path indicates Feather v2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arrow_field_types: Option<Vec<String>>,
}

crate::impl_minimal_fallback!(ArrowIpcMetadata, _);

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct AvroMetadata {
    #[serde(flatten)]
    pub common: ColumnarCommonFields,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avro_field_types: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_canonical: Option<String>,
}

crate::impl_minimal_fallback!(AvroMetadata, _);

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct OrcMetadata {
    #[serde(flatten)]
    pub common: ColumnarCommonFields,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arrow_field_types: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_stripes: Option<usize>,
}

crate::impl_minimal_fallback!(OrcMetadata, _);
