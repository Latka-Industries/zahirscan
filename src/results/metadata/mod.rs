//! Media metadata structures (images, videos, audio, documents, etc.)

pub mod audio;
pub mod csv;
pub mod document;
pub mod image;
pub mod pdf;
pub mod sqlite;
pub mod stats;
pub mod toml;
pub mod video;
pub mod xml;
pub mod zip;

// Re-export all metadata types for convenience
pub use audio::AudioMetadata;
pub use csv::CsvMetadata;
pub use document::DocumentMetadata;
pub use image::ImageMetadata;
pub use pdf::PdfMetadata;
pub use sqlite::{ColumnInfo, ForeignKeyInfo, IndexInfo, SqliteMetadata, TableInfo};
pub use stats::{BlobStats, BooleanStats, DateStats, NumericStats, TextStats};
pub use toml::TomlMetadata;
pub use video::VideoMetadata;
pub use xml::XmlMetadata;
pub use zip::{ZipEntry, ZipMetadata};
