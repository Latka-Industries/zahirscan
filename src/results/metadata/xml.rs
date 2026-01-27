//! XML file metadata structures

use std::collections::BTreeMap;

use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

use crate::results::MinimalFallback;

/// Recursive type info for an XML element: attributes (name → "string") and children (tag → type).
/// For repeated same-name siblings, the child is `Array(Element{...})` using the first occurrence.
#[derive(Debug, Clone)]
pub enum XmlTypeInfo {
    /// Element: attributes and children (recursive). Tag name is the key in the parent's children.
    Element {
        attributes: BTreeMap<String, String>,
        children: BTreeMap<String, XmlTypeInfo>,
    },
    /// Array of elements: used when the same tag appears multiple times as a sibling.
    Array(Box<XmlTypeInfo>),
}

impl Serialize for XmlTypeInfo {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            XmlTypeInfo::Element {
                attributes,
                children,
            } => {
                #[derive(Serialize)]
                struct El<'a> {
                    attributes: &'a BTreeMap<String, String>,
                    children: &'a BTreeMap<String, XmlTypeInfo>,
                }
                El {
                    attributes,
                    children,
                }
                .serialize(s)
            }
            XmlTypeInfo::Array(inner) => {
                #[derive(Serialize)]
                struct Arr<'a> {
                    #[serde(rename = "type")]
                    typ: &'static str,
                    element: &'a XmlTypeInfo,
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

impl<'de> Deserialize<'de> for XmlTypeInfo {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(XmlTypeInfoVisitor)
    }
}

struct XmlTypeInfoVisitor;

impl<'de> Visitor<'de> for XmlTypeInfoVisitor {
    type Value = XmlTypeInfo;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("an object (element with attributes/children or array with type and element)")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut attributes = BTreeMap::new();
        let mut children = BTreeMap::new();
        let mut type_holder: Option<String> = None;
        let mut element_holder: Option<XmlTypeInfo> = None;

        while let Some(k) = map.next_key::<String>()? {
            match k.as_str() {
                "type" => type_holder = Some(map.next_value::<String>()?),
                "element" => element_holder = Some(map.next_value::<XmlTypeInfo>()?),
                "attributes" => attributes = map.next_value()?,
                "children" => children = map.next_value()?,
                _ => {
                    let _ = map.next_value::<serde::de::IgnoredAny>()?;
                }
            }
        }

        if type_holder.as_deref() == Some("array")
            && let Some(e) = element_holder
        {
            return Ok(XmlTypeInfo::Array(Box::new(e)));
        }
        Ok(XmlTypeInfo::Element {
            attributes,
            children,
        })
    }
}

/// XML file metadata
#[derive(Debug, Clone, Deserialize, Default)]
pub struct XmlMetadata {
    /// File size in bytes
    pub file_size: Option<usize>,
    /// Number of elements (tags)
    pub element_count: Option<usize>,
    /// Total number of attributes
    pub attribute_count: Option<usize>,
    /// Maximum nesting depth
    pub max_depth: Option<usize>,
    /// Whether the document uses XML namespaces (xmlns)
    pub has_namespaces: Option<bool>,
    /// Root-level schema: element tag → { attributes, children } (recursive); repeated siblings become array
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<BTreeMap<String, XmlTypeInfo>>,
}

impl Serialize for XmlMetadata {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("XmlMetadata", 6)?;
        crate::serialize_optional!(state, self.file_size, "file_size");
        crate::serialize_optional!(state, self.element_count, "element_count");
        crate::serialize_optional!(state, self.attribute_count, "attribute_count");
        crate::serialize_optional!(state, self.max_depth, "max_depth");
        crate::serialize_optional!(state, self.has_namespaces, "has_namespaces");
        crate::serialize_optional!(state, self.schema, "schema");
        state.end()
    }
}

crate::impl_minimal_fallback!(XmlMetadata, file_size);
