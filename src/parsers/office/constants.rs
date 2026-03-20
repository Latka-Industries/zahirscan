//! Shared constants for Office document parsers (DOCX, PPTX, XLSX).
//! docProps/core.xml property definitions and paths.

use crate::results::{DocumentMetadata, PptxMetadata};

/// Property definition for docProps/core.xml extraction.
/// Generic over the metadata type so DOCX and PPTX can share the pattern.
pub struct CoreProperty<M> {
    pub element: &'static [u8],
    pub namespace: &'static [u8],
    pub setter: fn(&mut M, String),
}

/// Core properties for DOCX (`DocumentMetadata`). Used by `docx::extract_core_properties`.
pub const DOCX_CORE_PROPERTIES: &[CoreProperty<DocumentMetadata>] = &[
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

/// Core properties for PPTX (`PptxMetadata`). Subset of DOCX: title, author, created, modified.
pub const PPTX_CORE_PROPERTIES: &[CoreProperty<PptxMetadata>] = &[
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
        element: b"created",
        namespace: b"dcterms",
        setter: |m, v| m.creation_date = Some(v),
    },
    CoreProperty {
        element: b"modified",
        namespace: b"dcterms",
        setter: |m, v| m.modification_date = Some(v),
    },
];

// --- Shared: revision special case (DOCX, XLSX) ---
pub const REVISION_ELEMENT: &[u8] = b"revision";
pub const CP_NAMESPACE: &[u8] = b"cp";

// --- Office paths ---
pub const OFFICE_CORE_XML: &str = "docProps/core.xml";

// --- XLSX-specific ---
pub const XLSX_APP_XML: &str = "docProps/app.xml";
pub const XLSX_WORKBOOK_XML: &str = "xl/workbook.xml";
pub const XLSX_SHEET: &[u8] = b"sheet";
pub const XLSX_LPSTR: &[u8] = b"lpstr";
pub const XLSX_ATTR_NAME: &[u8] = b"name";
