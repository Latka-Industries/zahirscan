//! `SQLite` metadata structures

use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

use crate::results::MinimalFallback;

use super::{BlobStats, BooleanStats, DateStats, NumericStats, TextStats};

/// Column information for `SQLite` tables
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ColumnInfo {
    /// Column name
    pub name: String,
    /// `SQLite` type (INTEGER, TEXT, REAL, BLOB, NULL)
    pub type_name: Option<String>,
    /// NOT NULL constraint
    pub not_null: Option<bool>,
    /// Default value
    pub default_value: Option<String>,
    /// Part of primary key
    pub is_primary_key: Option<bool>,
    /// Part of foreign key
    pub is_foreign_key: Option<bool>,
    /// Null percentage (0.0-100.0)
    pub null_percentage: Option<f64>,
    /// Number of unique values
    pub unique_count: Option<usize>,
    /// Numeric statistics (for INTEGER/REAL columns)
    pub numeric_stats: Option<NumericStats>,
    /// Date statistics (for TEXT columns that contain dates)
    pub date_stats: Option<DateStats>,
    /// Boolean statistics (for INTEGER columns used as booleans)
    pub boolean_stats: Option<BooleanStats>,
    /// Text statistics (for TEXT columns: min/max/avg length)
    pub text_stats: Option<TextStats>,
    /// BLOB statistics (for BLOB columns: min/max/avg size in bytes)
    pub blob_stats: Option<BlobStats>,
}

impl Serialize for ColumnInfo {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("ColumnInfo", 12)?;
        state.serialize_field("name", &self.name)?;
        crate::serialize_optional!(state, self.type_name, "type_name");
        crate::serialize_optional!(state, self.not_null, "not_null");
        crate::serialize_optional!(state, self.default_value, "default_value");
        crate::serialize_optional!(state, self.is_primary_key, "is_primary_key");
        crate::serialize_optional!(state, self.is_foreign_key, "is_foreign_key");
        crate::serialize_optional!(state, self.null_percentage, "null_percentage");
        crate::serialize_optional!(state, self.unique_count, "unique_count");
        crate::serialize_optional!(state, self.numeric_stats, "numeric_stats");
        crate::serialize_optional!(state, self.date_stats, "date_stats");
        crate::serialize_optional!(state, self.boolean_stats, "boolean_stats");
        crate::serialize_optional!(state, self.text_stats, "text_stats");
        crate::serialize_optional!(state, self.blob_stats, "blob_stats");
        state.end()
    }
}

/// Foreign key relationship information
#[derive(Debug, Clone, Deserialize, Default, Serialize)]
pub struct ForeignKeyInfo {
    /// Column name
    pub column: String,
    /// Referenced table
    pub references_table: String,
    /// Referenced column
    pub references_column: String,
}

/// Index information
#[derive(Debug, Clone, Deserialize, Default, Serialize)]
pub struct IndexInfo {
    /// Index name
    pub name: String,
    /// Table name
    pub table: String,
    /// Column names in index
    pub columns: Vec<String>,
    /// Whether index is UNIQUE
    pub unique: Option<bool>,
}

/// Table information for `SQLite` databases
#[derive(Debug, Clone, Deserialize, Default)]
pub struct TableInfo {
    /// Table name
    pub name: String,
    /// Number of rows in table
    pub row_count: Option<usize>,
    /// Number of columns
    pub column_count: Option<usize>,
    /// Column details
    pub columns: Option<Vec<ColumnInfo>>,
    /// Primary key column names
    pub primary_keys: Option<Vec<String>>,
    /// Foreign key relationships
    pub foreign_keys: Option<Vec<ForeignKeyInfo>>,
    /// Index information
    pub indexes: Option<Vec<IndexInfo>>,
    /// Approximate table size
    pub table_size: Option<usize>,
}

impl Serialize for TableInfo {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("TableInfo", 8)?;
        state.serialize_field("name", &self.name)?;
        crate::serialize_optional!(state, self.row_count, "row_count");
        crate::serialize_optional!(state, self.column_count, "column_count");
        crate::serialize_optional!(state, self.columns, "columns");
        crate::serialize_optional!(state, self.primary_keys, "primary_keys");
        crate::serialize_optional!(state, self.foreign_keys, "foreign_keys");
        crate::serialize_optional!(state, self.indexes, "indexes");
        crate::serialize_optional!(state, self.table_size, "table_size");
        state.end()
    }
}

/// `SQLite` database metadata (Mode 2 only, for `SQLite` files)
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SqliteMetadata {
    /// Database file size in bytes
    pub file_size: Option<usize>,
    /// `SQLite` page size
    pub page_size: Option<usize>,
    /// `SQLite` version string
    pub sqlite_version: Option<String>,
    /// Database encoding (UTF-8, UTF-16, etc.)
    pub encoding: Option<String>,
    /// Total number of tables
    pub table_count: Option<usize>,
    /// Total rows across all tables
    pub total_rows: Option<usize>,
    /// List of tables with their metadata
    pub tables: Option<Vec<TableInfo>>,
    /// Error message if database could not be opened or parsed
    pub error: Option<String>,
}

impl Serialize for SqliteMetadata {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("SqliteMetadata", 8)?;
        crate::serialize_optional!(state, self.file_size, "file_size");
        crate::serialize_optional!(state, self.page_size, "page_size");
        crate::serialize_optional!(state, self.sqlite_version, "sqlite_version");
        crate::serialize_optional!(state, self.encoding, "encoding");
        crate::serialize_optional!(state, self.table_count, "table_count");
        crate::serialize_optional!(state, self.total_rows, "total_rows");
        crate::serialize_optional!(state, self.tables, "tables");
        crate::serialize_optional!(state, self.error, "error");
        state.end()
    }
}

crate::impl_minimal_fallback!(SqliteMetadata, file_size);
