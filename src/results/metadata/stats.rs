//! Shared statistics types used by CSV and `SQLite` metadata

use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

/// Numeric column statistics
#[derive(Debug, Clone, Deserialize, PartialEq, Default)]
pub struct NumericStats {
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub mean: Option<f64>,
    pub median: Option<f64>,
    pub range: Option<f64>,
    pub iqr: Option<f64>,
    pub stdev: Option<f64>,
}

impl Serialize for NumericStats {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("NumericStats", 7)?;
        crate::serialize_optional!(state, self.min, "min");
        crate::serialize_optional!(state, self.max, "max");
        crate::serialize_optional!(state, self.mean, "mean");
        crate::serialize_optional!(state, self.median, "median");
        crate::serialize_optional!(state, self.range, "range");
        crate::serialize_optional!(state, self.iqr, "iqr");
        crate::serialize_optional!(state, self.stdev, "stdev");
        state.end()
    }
}

/// Date column statistics
#[derive(Debug, Clone, Deserialize, PartialEq, Default)]
pub struct DateStats {
    pub span_days: Option<f64>,
    pub span_minutes: Option<f64>,
    pub min: Option<String>,
    pub max: Option<String>,
}

impl Serialize for DateStats {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("DateStats", 4)?;
        crate::serialize_optional!(state, self.span_days, "span_days");
        crate::serialize_optional!(state, self.span_minutes, "span_minutes");
        crate::serialize_optional!(state, self.min, "min");
        crate::serialize_optional!(state, self.max, "max");
        state.end()
    }
}

/// Boolean column statistics
#[derive(Debug, Clone, Deserialize, PartialEq, Default)]
pub struct BooleanStats {
    /// Serialized as `true_pct`; JSON key `true_percentage` is also accepted on deserialize.
    #[serde(default, alias = "true_pct")]
    pub true_percentage: Option<f64>,
}

impl Serialize for BooleanStats {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("BooleanStats", 1)?;
        crate::serialize_optional!(state, self.true_percentage, "true_pct");
        state.end()
    }
}

/// Text column statistics
#[derive(Debug, Clone, Deserialize, Default, Serialize)]
pub struct TextStats {
    /// Minimum text length
    pub min_length: Option<usize>,
    /// Maximum text length
    pub max_length: Option<usize>,
    /// Average text length
    pub avg_length: Option<f64>,
}

/// BLOB column statistics
#[derive(Debug, Clone, Deserialize, Default, Serialize)]
pub struct BlobStats {
    /// Minimum BLOB size in bytes
    pub min_size: Option<usize>,
    /// Maximum BLOB size in bytes
    pub max_size: Option<usize>,
    /// Average BLOB size in bytes
    pub avg_size: Option<f64>,
}
