//! PDF metadata structures

use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

use crate::results::MinimalFallback;

/// PDF metadata
#[derive(Debug, Clone, Deserialize, Default)]
pub struct PdfMetadata {
    // Document structure
    /// Number of pages
    pub page_count: Option<usize>,
    /// PDF version (e.g., "1.7", "2.0")
    pub pdf_version: Option<String>,
    /// Whether the PDF is encrypted
    pub is_encrypted: Option<bool>,
    /// File size in bytes
    pub file_size: Option<usize>,

    // Document metadata (from InfoDict)
    /// Document title
    pub title: Option<String>,
    /// Document author
    pub author: Option<String>,
    /// Document subject
    pub subject: Option<String>,
    /// Document creator (application that created the PDF)
    pub creator: Option<String>,
    /// Document producer (application that produced the PDF)
    pub producer: Option<String>,
    /// Creation date (ISO 8601 format)
    pub creation_date: Option<String>,
    /// Modification date (ISO 8601 format)
    pub modification_date: Option<String>,
}

impl Serialize for PdfMetadata {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("PdfMetadata", 11)?;
        crate::serialize_optional!(state, self.page_count, "page_count");
        crate::serialize_optional!(state, self.pdf_version, "pdf_version");
        crate::serialize_optional!(state, self.title, "title");
        crate::serialize_optional!(state, self.author, "author");
        crate::serialize_optional!(state, self.subject, "subject");
        crate::serialize_optional!(state, self.creator, "creator");
        crate::serialize_optional!(state, self.producer, "producer");
        crate::serialize_optional!(state, self.creation_date, "creation_date");
        crate::serialize_optional!(state, self.modification_date, "modification_date");
        crate::serialize_optional!(state, self.is_encrypted, "is_encrypted");
        crate::serialize_optional!(state, self.file_size, "file_size");
        state.end()
    }
}

crate::impl_minimal_fallback!(PdfMetadata, file_size);
