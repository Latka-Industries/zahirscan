//! Columnar binary formats: Parquet, Arrow IPC / Feather, Avro, ORC, and CSV (shared table fields).
//!
//! Shared table fields live on [`ColumnarCommonFields`]; each format struct composes that
//! (via `#[serde(flatten)]` for a flat JSON shape) and adds only format-specific columns.

use serde::{Deserialize, Serialize};

use crate::results::MinimalFallback;
use crate::results::{BooleanStats, DateStats, NumericStats};

/// One column’s inferred profile and statistics (compact JSON: `columns` array).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ColumnStat {
    pub i: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Inferred value kind from sampling (`number`, `string`, `date`, …).
    pub t: String,
    /// Physical / schema dtype when available (e.g. Arrow `Int32`, Avro field schema text).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pt: Option<String>,
    /// Sample null rate where defined (CSV / Arrow-string pipeline). Omitted when not computed (e.g. MTX).
    #[serde(rename = "null_pct", default, skip_serializing_if = "Option::is_none")]
    pub null_pct: Option<f64>,
    /// Distinct non-null **string** values in the sample (only meaningful when [`Self::t`] is `"string"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uniq: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num: Option<NumericStats>,
    #[serde(rename = "bool", skip_serializing_if = "Option::is_none")]
    pub bool_stats: Option<BooleanStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<DateStats>,
}

/// Parallel per-column vectors merged by [`merge_column_stats`].
#[derive(Debug, Clone)]
pub struct MergeColumnStatsInput {
    pub column_count: usize,
    pub column_names: Option<Vec<String>>,
    pub column_types: Option<Vec<String>>,
    pub null_percentages: Option<Vec<f64>>,
    pub unique_counts: Option<Vec<usize>>,
    pub numeric_stats: Option<Vec<Option<NumericStats>>>,
    pub date_stats: Option<Vec<Option<DateStats>>>,
    pub boolean_stats: Option<Vec<Option<BooleanStats>>>,
    pub physical_types: Option<Vec<String>>,
}

/// Merge legacy parallel per-column vectors into a compact [`ColumnStat`] list.
#[must_use]
pub fn merge_column_stats(input: &MergeColumnStatsInput) -> Option<Vec<ColumnStat>> {
    let column_count = input.column_count;
    if column_count == 0 {
        return None;
    }
    let mut out = Vec::with_capacity(column_count);
    for j in 0..column_count {
        let name = input.column_names.as_ref().and_then(|v| v.get(j).cloned());
        let t = input
            .column_types
            .as_ref()
            .and_then(|v| v.get(j))
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());
        let pt = input
            .physical_types
            .as_ref()
            .and_then(|v| v.get(j).cloned());
        let null_pct = input
            .null_percentages
            .as_ref()
            .and_then(|v| v.get(j).copied());
        let uniq = if t == "string" {
            input.unique_counts.as_ref().and_then(|v| v.get(j).copied())
        } else {
            None
        };
        let num = input
            .numeric_stats
            .as_ref()
            .and_then(|v| v.get(j))
            .cloned()
            .flatten();
        let date = input
            .date_stats
            .as_ref()
            .and_then(|v| v.get(j))
            .cloned()
            .flatten();
        let bool_stats = input
            .boolean_stats
            .as_ref()
            .and_then(|v| v.get(j))
            .cloned()
            .flatten();
        out.push(ColumnStat {
            i: j,
            name,
            t,
            pt,
            null_pct,
            uniq,
            num,
            bool_stats,
            date,
        });
    }
    Some(out)
}

/// Shared table-oriented fields (used by [`CsvMetadata`] and columnar format metadata).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ColumnarCommonFields {
    /// Omitted in JSON when `None` (e.g. non-tabular `.mat` `struct` / `cell` top-level).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column_count: Option<usize>,
    /// Rows used for column stats (sample), when stats are sample-based. Total row count for tabular formats is in `row_count` when set; for `ArrayLayoutSummary` + flattened common, use `shape` instead.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats_rows_sampled: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub columns: Option<Vec<ColumnStat>>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ParquetMetadata {
    #[serde(flatten)]
    pub common: ColumnarCommonFields,
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
}

crate::impl_minimal_fallback!(ArrowIpcMetadata, _);

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct AvroMetadata {
    #[serde(flatten)]
    pub common: ColumnarCommonFields,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_canonical: Option<String>,
}

crate::impl_minimal_fallback!(AvroMetadata, _);

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct OrcMetadata {
    #[serde(flatten)]
    pub common: ColumnarCommonFields,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_stripes: Option<usize>,
}

crate::impl_minimal_fallback!(OrcMetadata, _);

/// CSV metadata (Mode 2 only, for CSV files)
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct CsvMetadata {
    #[serde(flatten)]
    pub common: ColumnarCommonFields,

    // Format detection (CSV-specific)
    /// Detected delimiter character (e.g., ",", ";", "\t", "|")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delimiter: Option<String>,
    /// Detected quote character (e.g., "\"", "'")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_character: Option<String>,
    /// Detected escape character (e.g., "\\", "\"")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub escape_character: Option<String>,
    /// Whether the CSV has a header row
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_header: Option<bool>,
}

crate::impl_minimal_fallback!(CsvMetadata, _);
