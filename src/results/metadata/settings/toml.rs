//! TOML metadata structures

use std::collections::BTreeMap;

use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

use crate::results::MinimalFallback;

/// Recursive type information for a TOML value: scalar name, table (fields and their types), or array (element type).
#[derive(Debug, Clone)]
pub enum TomlTypeInfo {
    /// Scalar: "string", "integer", "float", "boolean", "datetime"
    Scalar(String),
    /// Table: field names → their types (recursive)
    Table(BTreeMap<String, TomlTypeInfo>),
    /// Array: element type (for array-of-tables, the first element’s structure is used)
    Array(Box<TomlTypeInfo>),
}

impl Serialize for TomlTypeInfo {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            TomlTypeInfo::Scalar(x) => x.serialize(s),
            TomlTypeInfo::Table(m) => m.serialize(s),
            TomlTypeInfo::Array(inner) => {
                #[derive(Serialize)]
                struct Arr<'a> {
                    #[serde(rename = "type")]
                    typ: &'static str,
                    element: &'a TomlTypeInfo,
                }
                Arr {
                    typ: "array",
                    element: inner,
                }
                .serialize(s)
            }
        }
    }
}

impl<'de> Deserialize<'de> for TomlTypeInfo {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(TomlTypeInfoVisitor)
    }
}

struct TomlTypeInfoVisitor;

impl<'de> Visitor<'de> for TomlTypeInfoVisitor {
    type Value = TomlTypeInfo;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a string, or an object (table or array element)")
    }

    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
        Ok(TomlTypeInfo::Scalar(v.to_string()))
    }

    fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Self::Value, E> {
        Ok(TomlTypeInfo::Scalar(v))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut table = BTreeMap::new();
        let mut type_holder: Option<TomlTypeInfo> = None;
        let mut element_holder: Option<TomlTypeInfo> = None;

        while let Some(k) = map.next_key::<String>()? {
            let v = map.next_value::<TomlTypeInfo>()?;
            match k.as_str() {
                "type" => type_holder = Some(v),
                "element" => element_holder = Some(v),
                _ => {
                    table.insert(k, v);
                }
            }
        }

        if matches!(type_holder.as_ref(), Some(TomlTypeInfo::Scalar(s)) if s == "array")
            && table.is_empty()
            && let Some(e) = element_holder
        {
            return Ok(TomlTypeInfo::Array(Box::new(e)));
        }
        if let Some(t) = type_holder {
            table.insert("type".to_string(), t);
        }
        if let Some(e) = element_holder {
            table.insert("element".to_string(), e);
        }
        Ok(TomlTypeInfo::Table(table))
    }
}

/// TOML file metadata
#[derive(Debug, Clone, Deserialize, Default)]
pub struct TomlMetadata {
    /// File size in bytes
    pub file_size: Option<usize>,
    /// Number of [section] tables (including root)
    pub section_count: Option<usize>,
    /// Total number of keys across all tables
    pub key_count: Option<usize>,
    /// Maximum nesting depth of tables
    pub max_depth: Option<usize>,
    /// Root-level schema: tables show field names and types; arrays show element type (recursive)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<BTreeMap<String, TomlTypeInfo>>,
}

impl Serialize for TomlMetadata {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("TomlMetadata", 5)?;
        crate::serialize_optional!(state, self.file_size, "file_size");
        crate::serialize_optional!(state, self.section_count, "section_count");
        crate::serialize_optional!(state, self.key_count, "key_count");
        crate::serialize_optional!(state, self.max_depth, "max_depth");
        crate::serialize_optional!(state, self.schema, "schema");
        state.end()
    }
}

crate::impl_minimal_fallback!(TomlMetadata, file_size);
