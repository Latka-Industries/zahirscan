//! PPTX (PowerPoint) metadata structures

use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

use crate::results::MinimalFallback;

/// PPTX file metadata
#[derive(Debug, Clone, Deserialize, Default)]
pub struct PptxMetadata {
    /// File size in bytes
    pub file_size: Option<usize>,
    /// Number of slides
    pub slide_count: Option<usize>,
    /// Presentation title
    pub title: Option<String>,
    /// Author/creator
    pub author: Option<String>,
    /// Creation date (ISO 8601)
    pub creation_date: Option<String>,
    /// Modification date (ISO 8601)
    pub modification_date: Option<String>,
}

impl Serialize for PptxMetadata {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("PptxMetadata", 6)?;
        crate::serialize_optional!(state, self.file_size, "file_size");
        crate::serialize_optional!(state, self.slide_count, "slide_count");
        crate::serialize_optional!(state, self.title, "title");
        crate::serialize_optional!(state, self.author, "author");
        crate::serialize_optional!(state, self.creation_date, "creation_date");
        crate::serialize_optional!(state, self.modification_date, "modification_date");
        state.end()
    }
}

crate::impl_minimal_fallback!(PptxMetadata, file_size);
