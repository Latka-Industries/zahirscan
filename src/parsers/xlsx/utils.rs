//! Utility functions for XLSX parsing
//!
//! Contains reusable helpers for XML parsing and core properties extraction.

use crate::results::DocumentMetadata;
use log::warn;
use std::io::Cursor;
use std::io::Read;
use zip::ZipArchive;

/// Check if an XML element name contains a specific namespace prefix
///
/// Namespaces in Office XML are typically in the format: `{namespace}element`
/// This function checks if the namespace bytes appear anywhere in the element name.
///
/// # Arguments
/// * `name` - The XML element name as a byte slice
/// * `namespace` - The namespace prefix to check for (e.g., b"dc", b"dcterms", b"cp")
pub(crate) fn has_namespace(name: &[u8], namespace: &[u8]) -> bool {
    name.windows(namespace.len()).any(|w| w == namespace)
}

/// Set a metadata field if the value is not empty
///
/// Helper function to reduce repetitive code when setting optional string fields
/// in DocumentMetadata.
pub(crate) fn set_metadata_field(
    metadata: &mut DocumentMetadata,
    value: &str,
    setter: fn(&mut DocumentMetadata, String),
) {
    let trimmed = value.trim();
    if !trimmed.is_empty() {
        setter(metadata, trimmed.to_string());
    }
}

/// Property definition for core properties extraction
pub(crate) struct CoreProperty {
    /// Element name suffix (e.g., b"title", b"creator")
    pub element: &'static [u8],
    /// Namespace prefix (e.g., b"dc", b"dcterms", b"cp")
    pub namespace: &'static [u8],
    /// Setter function to update the metadata field
    pub setter: fn(&mut DocumentMetadata, String),
}

/// All core properties that can be extracted from docProps/core.xml
/// Same structure as DOCX - these are Office Open XML standard properties
pub(crate) const CORE_PROPERTIES: &[CoreProperty] = &[
    CoreProperty {
        element: b"title",
        namespace: b"dc",
        setter: |m, v| m.title = Some(v),
    },
    CoreProperty {
        element: b"creator",
        namespace: b"dc",
        setter: |m, v| m.author = Some(v),
    },
    CoreProperty {
        element: b"subject",
        namespace: b"dc",
        setter: |m, v| m.subject = Some(v),
    },
    CoreProperty {
        element: b"description",
        namespace: b"dc",
        setter: |m, v| m.description = Some(v),
    },
    CoreProperty {
        element: b"created",
        namespace: b"dcterms",
        setter: |m, v| m.creation_date = Some(v),
    },
    CoreProperty {
        element: b"modified",
        namespace: b"dcterms",
        setter: |m, v| m.modified_date = Some(v),
    },
    CoreProperty {
        element: b"lastModifiedBy",
        namespace: b"cp",
        setter: |m, v| m.last_modified_by = Some(v),
    },
];

/// XLSX-specific XML element names
pub(crate) mod elements {
    /// Sheet element name in workbook.xml
    pub const SHEET: &[u8] = b"sheet";
    /// Sheet name list element in app.xml
    pub const LPSTR: &[u8] = b"lpstr";
}

/// XLSX-specific XML attribute names
pub(crate) mod attributes {
    /// Name attribute for sheet elements
    pub const NAME: &[u8] = b"name";
}

/// Core property element names (for special cases)
pub(crate) mod core_elements {
    /// Revision element name
    pub const REVISION: &[u8] = b"revision";
}

/// Core property namespace prefixes (for special cases)
pub(crate) mod core_namespaces {
    /// Core properties namespace
    pub const CP: &[u8] = b"cp";
}

/// XLSX ZIP archive file paths
pub(crate) mod xml_files {
    /// Core properties XML file (document metadata: title, author, dates, etc.)
    pub const CORE_PROPERTIES: &str = "docProps/core.xml";
    /// App properties XML file (sheet names, count, etc.)
    pub const APP_PROPERTIES: &str = "docProps/app.xml";
    /// Workbook XML file (sheet information - more reliable, always exists)
    pub const WORKBOOK: &str = "xl/workbook.xml";
}

/// Open an XLSX file as a ZIP archive
///
/// XLSX files are ZIP archives containing XML files.
/// This helper function opens the content as a ZIP archive with proper error handling.
///
/// # Arguments
/// * `content` - The raw bytes of the XLSX file
/// * `file_path` - The file path (for error messages)
///
/// # Returns
/// * `Ok(ZipArchive)` if the file can be opened as a ZIP archive
/// * `Err` if the file cannot be opened or is not a valid ZIP archive
pub(crate) fn open_office_archive<'a>(
    content: &'a [u8],
    file_path: &str,
) -> Result<ZipArchive<Cursor<&'a [u8]>>, zip::result::ZipError> {
    let cursor = Cursor::new(content);
    ZipArchive::new(cursor).map_err(|e| {
        warn!(
            "Failed to open Office file as ZIP archive {}: {:?}",
            file_path, e
        );
        e
    })
}

/// Read an XML file from a ZIP archive and call an extraction function
///
/// Helper function to reduce repetitive code when reading XML files from ZIP archives.
///
/// # Arguments
/// * `archive` - The ZIP archive
/// * `file_path` - Path to the XML file within the archive
/// * `file_type` - Type of file (for error messages, e.g., "XLSX")
/// * `stats_file_path` - Original file path (for error messages)
/// * `extractor` - Function to call with the XML content
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
                warn!(
                    "Failed to read {} from {} {}",
                    file_path, file_type, stats_file_path
                );
            }
        }
        Err(e) => {
            warn!(
                "{} not found in {} {}: {:?}",
                file_path, file_type, stats_file_path, e
            );
        }
    }
}
