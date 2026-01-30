//! YAML metadata structures

use std::collections::BTreeMap;

use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

use crate::results::MinimalFallback;

/// Recursive type information for a YAML value: scalar name, mapping (keys → types), or sequence (element type).
#[derive(Debug, Clone)]
pub enum YamlTypeInfo {
    /// Scalar: "string", "integer", "float", "boolean", "null"
    Scalar(String),
    /// Mapping: key names → their types (recursive). Only string keys are included.
    Mapping(BTreeMap<String, YamlTypeInfo>),
    /// Sequence: element type (first element's structure, like TOML array-of-tables)
    Sequence(Box<YamlTypeInfo>),
}

impl Serialize for YamlTypeInfo {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            YamlTypeInfo::Scalar(x) => x.serialize(s),
            YamlTypeInfo::Mapping(m) => m.serialize(s),
            YamlTypeInfo::Sequence(inner) => {
                #[derive(Serialize)]
                struct Arr<'a> {
                    #[serde(rename = "type")]
                    typ: &'static str,
                    element: &'a YamlTypeInfo,
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

impl<'de> Deserialize<'de> for YamlTypeInfo {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(YamlTypeInfoVisitor)
    }
}

struct YamlTypeInfoVisitor;

impl<'de> Visitor<'de> for YamlTypeInfoVisitor {
    type Value = YamlTypeInfo;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a string, or an object (mapping or array element)")
    }

    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
        Ok(YamlTypeInfo::Scalar(v.to_string()))
    }

    fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Self::Value, E> {
        Ok(YamlTypeInfo::Scalar(v))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut mapping = BTreeMap::new();
        let mut type_holder: Option<YamlTypeInfo> = None;
        let mut element_holder: Option<YamlTypeInfo> = None;

        while let Some(k) = map.next_key::<String>()? {
            let v = map.next_value::<YamlTypeInfo>()?;
            match k.as_str() {
                "type" => type_holder = Some(v),
                "element" => element_holder = Some(v),
                _ => {
                    mapping.insert(k, v);
                }
            }
        }

        if matches!(type_holder.as_ref(), Some(YamlTypeInfo::Scalar(s)) if s == "array")
            && mapping.is_empty()
            && let Some(e) = element_holder
        {
            return Ok(YamlTypeInfo::Sequence(Box::new(e)));
        }
        if let Some(t) = type_holder {
            mapping.insert("type".to_string(), t);
        }
        if let Some(e) = element_holder {
            mapping.insert("element".to_string(), e);
        }
        Ok(YamlTypeInfo::Mapping(mapping))
    }
}

/// YAML file metadata
#[derive(Debug, Clone, Deserialize, Default)]
pub struct YamlMetadata {
    /// File size in bytes
    pub file_size: Option<usize>,
    /// Total number of keys (in mappings) across the document
    pub key_count: Option<usize>,
    /// Maximum nesting depth
    pub max_depth: Option<usize>,
    /// Number of scalar values (string, number, bool, null)
    pub scalar_count: Option<usize>,
    /// Number of sequence (array) structures
    pub sequence_count: Option<usize>,
    /// Number of mapping (object) structures
    pub map_count: Option<usize>,
    /// Root-level schema: mapping shows key → type; sequence shows element type; scalar shows type name (recursive)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<YamlTypeInfo>,
}

impl Serialize for YamlMetadata {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("YamlMetadata", 7)?;
        crate::serialize_optional!(state, self.file_size, "file_size");
        crate::serialize_optional!(state, self.key_count, "key_count");
        crate::serialize_optional!(state, self.max_depth, "max_depth");
        crate::serialize_optional!(state, self.scalar_count, "scalar_count");
        crate::serialize_optional!(state, self.sequence_count, "sequence_count");
        crate::serialize_optional!(state, self.map_count, "map_count");
        crate::serialize_optional!(state, self.schema, "schema");
        state.end()
    }
}

crate::impl_minimal_fallback!(YamlMetadata, file_size);
