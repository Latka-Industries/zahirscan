//! EPUB (e-book) metadata structures

use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

use crate::results::MinimalFallback;

/// EPUB file metadata
#[derive(Debug, Clone, Deserialize, Default)]
pub struct EpubMetadata {
    /// File size in bytes
    pub file_size: Option<usize>,
    /// Book title
    pub title: Option<String>,
    /// Author/creator
    pub author: Option<String>,
    /// Chapter or spine item count
    pub chapter_count: Option<usize>,
    /// Language (e.g. "en")
    pub language: Option<String>,
    /// Identifier (e.g. ISBN, UUID)
    pub identifier: Option<String>,
}

impl Serialize for EpubMetadata {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("EpubMetadata", 6)?;
        crate::serialize_optional!(state, self.file_size, "file_size");
        crate::serialize_optional!(state, self.title, "title");
        crate::serialize_optional!(state, self.author, "author");
        crate::serialize_optional!(state, self.chapter_count, "chapter_count");
        crate::serialize_optional!(state, self.language, "language");
        crate::serialize_optional!(state, self.identifier, "identifier");
        state.end()
    }
}

crate::impl_minimal_fallback!(EpubMetadata, file_size);
