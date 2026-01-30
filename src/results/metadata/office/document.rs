//! Document metadata structures (DOCX, XLSX, Pages)

use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

use crate::results::MinimalFallback;

/// Document metadata (for DOCX and Pages files)
#[derive(Debug, Clone, Deserialize, Default)]
pub struct DocumentMetadata {
    /// Number of pages (if available)
    pub page_count: Option<usize>,
    /// Word count
    pub word_count: Option<usize>,
    /// Character count (including spaces)
    pub character_count: Option<usize>,
    /// Character count without spaces
    pub character_count_no_spaces: Option<usize>,
    /// Paragraph count
    pub paragraph_count: Option<usize>,
    /// Sheet count (for XLSX files)
    pub sheet_count: Option<usize>,
    /// Per-sheet statistics (for XLSX files, nested object with sheet name -> {rows, columns})
    pub sheet_stats: Option<serde_json::Value>,
    /// File size in bytes
    pub file_size: Option<usize>,
    /// Document title
    pub title: Option<String>,
    /// Document author/creator
    pub author: Option<String>,
    /// Document subject
    pub subject: Option<String>,
    /// Document description
    pub description: Option<String>,
    /// Creation date (ISO 8601 format)
    pub creation_date: Option<String>,
    /// Modification date (ISO 8601 format)
    pub modified_date: Option<String>,
    /// Last modified by
    pub last_modified_by: Option<String>,
    /// Revision number
    pub revision: Option<i64>,
}

impl Serialize for DocumentMetadata {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("DocumentMetadata", 17)?;
        crate::serialize_optional!(state, self.page_count, "page_count");
        crate::serialize_optional!(state, self.word_count, "word_count");
        crate::serialize_optional!(state, self.character_count, "character_count");
        crate::serialize_optional!(
            state,
            self.character_count_no_spaces,
            "character_count_no_spaces"
        );
        crate::serialize_optional!(state, self.paragraph_count, "paragraph_count");
        crate::serialize_optional!(state, self.sheet_count, "sheet_count");
        crate::serialize_optional!(state, self.sheet_stats, "sheet_stats");
        crate::serialize_optional!(state, self.file_size, "file_size");
        crate::serialize_optional!(state, self.title, "title");
        crate::serialize_optional!(state, self.author, "author");
        crate::serialize_optional!(state, self.subject, "subject");
        crate::serialize_optional!(state, self.description, "description");
        crate::serialize_optional!(state, self.creation_date, "creation_date");
        crate::serialize_optional!(state, self.modified_date, "modified_date");
        crate::serialize_optional!(state, self.last_modified_by, "last_modified_by");
        crate::serialize_optional!(state, self.revision, "revision");
        state.end()
    }
}

crate::impl_minimal_fallback!(DocumentMetadata, file_size);
