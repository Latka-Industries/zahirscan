//! Media metadata structures (images, videos, audio, documents, etc.)

pub mod archive;
pub mod audio;
pub mod code;
pub mod csv;
pub mod document;
pub mod epub;
pub mod html;
pub mod image;
pub mod ini;
pub mod pdf;
pub mod pptx;
pub mod sqlite;
pub mod stats;
pub mod toml;
pub mod video;
pub mod xml;
pub mod yaml;
pub mod zip;

// Re-export all metadata types for convenience
pub use archive::{ArchiveEntry, ArchiveMetadata};
pub use audio::AudioMetadata;
pub use code::CodeMetadata;
pub use csv::CsvMetadata;
pub use document::DocumentMetadata;
pub use epub::EpubMetadata;
pub use html::HtmlMetadata;
pub use image::ImageMetadata;
pub use ini::IniMetadata;
pub use pdf::PdfMetadata;
pub use pptx::PptxMetadata;
pub use sqlite::{ColumnInfo, ForeignKeyInfo, IndexInfo, SqliteMetadata, TableInfo};
pub use stats::{BlobStats, BooleanStats, DateStats, NumericStats, TextStats};
pub use toml::TomlMetadata;
pub use video::VideoMetadata;
pub use xml::XmlMetadata;
pub use yaml::YamlMetadata;
pub use zip::{ZipEntry, ZipMetadata};
