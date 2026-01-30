//! CSV metadata structures

use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

use crate::results::MinimalFallback;
use crate::results::{BooleanStats, DateStats, NumericStats};

/// CSV metadata (Mode 2 only, for CSV files)
#[derive(Debug, Clone, Deserialize, Default)]
pub struct CsvMetadata {
    // Basic structure
    /// Number of rows (excluding header if present)
    pub row_count: usize,
    /// Number of columns
    pub column_count: usize,
    /// Column names (if header row exists)
    pub column_names: Option<Vec<String>>,
    /// File encoding (e.g., "UTF-8", "Latin-1")
    pub encoding: Option<String>,
    /// Inferred data types per column (e.g., "string", "number", "date", "boolean", "null")
    pub column_types: Option<Vec<String>>,

    // Format detection
    /// Detected delimiter character (e.g., ",", ";", "\t", "|")
    pub delimiter: Option<String>,
    /// Detected quote character (e.g., "\"", "'")
    pub quote_character: Option<String>,
    /// Detected escape character (e.g., "\\", "\"")
    pub escape_character: Option<String>,
    /// Whether the CSV has a header row
    pub has_header: Option<bool>,

    // Column statistics
    /// Percentage of null/empty values per column (0.0-100.0)
    pub null_percentages: Option<Vec<f64>>,
    /// Number of unique values per column (based on sample)
    pub unique_counts: Option<Vec<usize>>,
    /// Numeric statistics per column (only for numeric columns)
    pub numeric_stats: Option<Vec<Option<NumericStats>>>,
    /// Date statistics per column (only for date columns)
    pub date_stats: Option<Vec<Option<DateStats>>>,
    /// Boolean statistics per column (only for boolean columns)
    pub boolean_stats: Option<Vec<Option<BooleanStats>>>,
}

impl Serialize for CsvMetadata {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("CsvMetadata", 14)?;
        state.serialize_field("row_count", &self.row_count)?;
        state.serialize_field("column_count", &self.column_count)?;
        crate::serialize_optional!(state, self.column_names, "column_names");
        crate::serialize_optional!(state, self.encoding, "encoding");
        crate::serialize_optional!(state, self.column_types, "column_types");
        crate::serialize_optional!(state, self.delimiter, "delimiter");
        crate::serialize_optional!(state, self.quote_character, "quote_character");
        crate::serialize_optional!(state, self.escape_character, "escape_character");
        crate::serialize_optional!(state, self.has_header, "has_header");
        crate::serialize_optional!(state, self.null_percentages, "null_percentages");
        crate::serialize_optional!(state, self.unique_counts, "unique_counts");
        crate::serialize_optional!(state, self.numeric_stats, "numeric_stats");
        crate::serialize_optional!(state, self.date_stats, "date_stats");
        crate::serialize_optional!(state, self.boolean_stats, "boolean_stats");
        state.end()
    }
}

crate::impl_minimal_fallback!(CsvMetadata, _);
