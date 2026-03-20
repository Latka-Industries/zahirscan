//! Shared utilities for Office document parsing (DOCX, PPTX, XLSX).
//! XML helpers, entity decoding, ZIP archive handling, and read helpers.

use log::warn;
use quick_xml::escape::unescape;
use std::io::{Cursor, Read};
use zip::ZipArchive;

/// Check if an XML element name contains a specific namespace prefix
///
/// Namespaces in Office XML are typically in the format: `{namespace}element`
/// or `prefix:element`. This checks if the namespace bytes appear in the name.
pub(crate) fn has_namespace(name: &[u8], namespace: &[u8]) -> bool {
    name.windows(namespace.len()).any(|w| w == namespace)
}

/// Open an Office file (DOCX, PPTX, XLSX) as a ZIP archive
pub(crate) fn open_office_archive<'a>(
    content: &'a [u8],
    file_path: &str,
) -> Result<ZipArchive<Cursor<&'a [u8]>>, zip::result::ZipError> {
    let cursor = Cursor::new(content);
    ZipArchive::new(cursor).map_err(|e| {
        warn!("Failed to open Office file as ZIP archive {file_path}: {e:?}");
        e
    })
}

/// XML entity replacements for fallback decoding
const XML_ENTITIES: &[(&str, &str)] = &[
    ("&amp;", "&"),
    ("&lt;", "<"),
    ("&gt;", ">"),
    ("&quot;", "\""),
    ("&apos;", "'"),
];

/// Fallback XML entity decoder when quick-xml's unescape fails
pub(crate) fn decode_xml_entities_fallback(text: &str) -> String {
    let mut result = text.to_string();
    for (entity, replacement) in XML_ENTITIES {
        result = result.replace(entity, replacement);
    }
    result
}

/// Decode XML entities with fallback
pub(crate) fn decode_xml_entities(text: &str) -> String {
    match unescape(text) {
        Ok(decoded) => decoded.into_owned(),
        Err(_) => decode_xml_entities_fallback(text),
    }
}

/// Read an XML file from a ZIP archive and call an extraction function.
pub(crate) fn read_xml_from_archive<F>(
    archive: &mut ZipArchive<Cursor<&[u8]>>,
    file_path: &str,
    file_type: &str,
    stats_file_path: &str,
    mut extractor: F,
) where
    F: FnMut(&str),
{
    match archive.by_name(file_path) {
        Ok(mut file) => {
            let mut xml_content = String::new();
            if file.read_to_string(&mut xml_content).is_ok() {
                extractor(&xml_content);
            } else {
                warn!("Failed to read {file_path} from {file_type} {stats_file_path}");
            }
        }
        Err(e) => {
            warn!("{file_path} not found in {file_type} {stats_file_path}: {e:?}");
        }
    }
}
