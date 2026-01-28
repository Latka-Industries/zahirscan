//! INI / .cfg metadata structures

use std::collections::BTreeMap;

use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

use crate::results::MinimalFallback;

/// Recursive type information for an INI value: scalar name or table (key → type).
/// INI has no arrays; sections map to key=value with inferred scalar types.
#[derive(Debug, Clone)]
pub enum IniTypeInfo {
    /// Scalar: "string", "integer", "number", "boolean", "date", "timestamp", "null"
    Scalar(String),
    /// Table: key names → their types (section’s key→type map)
    Table(BTreeMap<String, IniTypeInfo>),
}

impl Serialize for IniTypeInfo {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            IniTypeInfo::Scalar(x) => x.serialize(s),
            IniTypeInfo::Table(m) => m.serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for IniTypeInfo {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(IniTypeInfoVisitor)
    }
}

struct IniTypeInfoVisitor;

impl<'de> Visitor<'de> for IniTypeInfoVisitor {
    type Value = IniTypeInfo;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a string or an object (section table)")
    }

    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
        Ok(IniTypeInfo::Scalar(v.to_string()))
    }

    fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Self::Value, E> {
        Ok(IniTypeInfo::Scalar(v))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut table = BTreeMap::new();
        while let Some(k) = map.next_key::<String>()? {
            let v = map.next_value::<IniTypeInfo>()?;
            table.insert(k, v);
        }
        Ok(IniTypeInfo::Table(table))
    }
}

/// INI file metadata
#[derive(Debug, Clone, Deserialize, Default)]
pub struct IniMetadata {
    /// File size in bytes
    pub file_size: Option<usize>,
    /// Number of [section] headers
    pub section_count: Option<usize>,
    /// Total number of key=value pairs
    pub key_count: Option<usize>,
    /// Number of comment lines (; or # to EOL)
    pub comment_count: Option<usize>,
    /// Maximum nesting depth (2 for typical INI: section → keys)
    pub max_depth: Option<usize>,
    /// Schema: section name → Table of key → inferred scalar type (same shape as TOML/YAML schema)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<BTreeMap<String, IniTypeInfo>>,
}

impl Serialize for IniMetadata {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("IniMetadata", 6)?;
        crate::serialize_optional!(state, self.file_size, "file_size");
        crate::serialize_optional!(state, self.section_count, "section_count");
        crate::serialize_optional!(state, self.key_count, "key_count");
        crate::serialize_optional!(state, self.comment_count, "comment_count");
        crate::serialize_optional!(state, self.max_depth, "max_depth");
        crate::serialize_optional!(state, self.schema, "schema");
        state.end()
    }
}

crate::impl_minimal_fallback!(IniMetadata, file_size);
