//! Media metadata structures (images, videos, audio, documents, etc.)

mod code;
mod container;
mod logging;
mod media;
mod models;
mod office;
mod settings;
mod sqlite;
mod stats;
mod structured;

// Re-export all metadata types for convenience
pub use code::CodeMetadata;
pub use container::{ArchiveEntry, ArchiveMetadata, ZipEntry, ZipMetadata};
pub use logging::LogMetadata;
pub use media::{AudioMetadata, ImageMetadata, VideoMetadata};
pub use models::*;
pub use office::{DocumentMetadata, PptxMetadata};
pub use settings::*;
pub use sqlite::{ColumnInfo, ForeignKeyInfo, IndexInfo, SqliteMetadata, TableInfo};
pub use stats::{BlobStats, BooleanStats, DateStats, NumericStats, TextStats};
pub use structured::*;
