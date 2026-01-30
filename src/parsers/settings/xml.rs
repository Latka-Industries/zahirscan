//! XML file metadata extraction

use std::collections::BTreeMap;
use std::io::Cursor;

use crate::engine::config::Config;
use crate::parsers::ParseResult;
use crate::results::XmlMetadata;
use crate::results::metadata::settings::xml::XmlTypeInfo;
use anyhow::Result;
use quick_xml::Reader;
use quick_xml::events::Event;

/// Attribute name "xmlns" and prefix for namespace declarations (e.g. xmlns, xmlns:foo).
const ATTR_XMLNS: &[u8] = b"xmlns";

fn local_name_str(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// Collect attributes into name -> "string", and detect xmlns. Skips xmlns attributes in the map.
fn collect_attrs(
    e: &quick_xml::events::BytesStart<'_>,
    attribute_count: &mut usize,
    has_namespaces: &mut bool,
) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    for a in e.attributes().flatten() {
        *attribute_count += 1;
        if a.key.as_ref().starts_with(ATTR_XMLNS) {
            *has_namespaces = true;
        }
        if a.key.as_namespace_binding().is_some() {
            continue;
        }
        let k = local_name_str(a.key.local_name().as_ref());
        m.insert(k, "string".to_string());
    }
    m
}

/// Helper struct to build XML type information incrementally.
struct ElementBuilder {
    attributes: BTreeMap<String, String>,
    children: BTreeMap<String, XmlTypeInfo>,
}

impl ElementBuilder {
    fn into_type_info(self) -> XmlTypeInfo {
        XmlTypeInfo::Element {
            attributes: self.attributes,
            children: self.children,
        }
    }
}

/// Merges `other` into the element at `into`: union of attributes and recursive
/// merge of children. Used when folding same-tag siblings into an array's element.
/// If `other` is Array, merges the array's inner element.
fn merge_element_into(into: &mut XmlTypeInfo, other: XmlTypeInfo) {
    let (attrs, other_children) = match other {
        XmlTypeInfo::Element {
            attributes,
            children,
        } => (attributes, children),
        XmlTypeInfo::Array(inner) => return merge_element_into(into, *inner),
    };
    if let XmlTypeInfo::Element {
        attributes,
        children,
    } = into
    {
        for (k, v) in attrs {
            attributes.insert(k, v);
        }
        for (k, v) in other_children {
            merge_child(children, k, v);
        }
    }
}

/// Merge child element into parent's children map.
/// If child has same tag name, merge into array (recursive).
fn merge_child(children: &mut BTreeMap<String, XmlTypeInfo>, key: String, value: XmlTypeInfo) {
    if let Some(existing) = children.remove(&key) {
        let mut arr = match existing {
            XmlTypeInfo::Array(inner) => XmlTypeInfo::Array(inner),
            _ => XmlTypeInfo::Array(Box::new(existing)),
        };
        if let XmlTypeInfo::Array(ref mut inner) = arr {
            merge_element_into(inner.as_mut(), value);
        }
        children.insert(key, arr);
    } else {
        children.insert(key, value);
    }
}

/// Extract XML metadata by streaming over events and building a structural schema.
pub fn extract_xml_metadata(
    content: &[u8],
    stats: &ParseResult,
    _config: &Config,
) -> Result<XmlMetadata> {
    let mut reader = Reader::from_reader(Cursor::new(content));
    reader.config_mut().trim_text(true);

    let mut element_count: usize = 0;
    let mut attribute_count: usize = 0;
    let mut depth: usize = 0;
    let mut max_depth: usize = 0;
    let mut has_namespaces = false;

    let mut stack: Vec<ElementBuilder> = Vec::new();
    let mut schema: Option<BTreeMap<String, XmlTypeInfo>> = None;

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                element_count += 1;
                depth += 1;
                if depth > max_depth {
                    max_depth = depth;
                }
                let attrs = collect_attrs(&e, &mut attribute_count, &mut has_namespaces);
                stack.push(ElementBuilder {
                    attributes: attrs,
                    children: BTreeMap::new(),
                });
            }
            Ok(Event::Empty(e)) => {
                element_count += 1;
                if depth + 1 > max_depth {
                    max_depth = depth + 1;
                }
                let attrs = collect_attrs(&e, &mut attribute_count, &mut has_namespaces);
                let local = local_name_str(e.name().local_name().as_ref());
                let node = ElementBuilder {
                    attributes: attrs,
                    children: BTreeMap::new(),
                }
                .into_type_info();
                if let Some(top) = stack.last_mut() {
                    merge_child(&mut top.children, local, node);
                }
            }
            Ok(Event::End(e)) => {
                depth = depth.saturating_sub(1);
                let local = local_name_str(e.name().local_name().as_ref());
                if let Some(node) = stack.pop() {
                    let ty = node.into_type_info();
                    if let Some(top) = stack.last_mut() {
                        merge_child(&mut top.children, local, ty);
                    } else {
                        schema = Some(BTreeMap::from([(local, ty)]));
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML parse error: {}", e)),
            _ => {}
        }
    }

    Ok(XmlMetadata {
        file_size: Some(stats.byte_count),
        element_count: Some(element_count),
        attribute_count: Some(attribute_count),
        max_depth: Some(max_depth),
        has_namespaces: Some(has_namespaces),
        schema,
    })
}

crate::no_template_mining!(
    extract_xml_templates,
    "XML is structured; no template mining (use text/JSON for that)."
);
