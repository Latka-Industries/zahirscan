//! Media metadata structures (images, videos, audio, documents, etc.)

pub mod code;
pub mod container;
pub mod logging;
pub mod media;
pub mod office;
pub mod settings;
pub mod sqlite;
pub mod stats;
pub mod structured;

// Re-export all metadata types for convenience
pub use code::CodeMetadata;
pub use container::{ArchiveEntry, ArchiveMetadata, ZipEntry, ZipMetadata};
pub use logging::LogMetadata;
pub use media::{AudioMetadata, ImageMetadata, VideoMetadata};
pub use office::{DocumentMetadata, PptxMetadata};
pub use settings::{IniMetadata, TomlMetadata, XmlMetadata, YamlMetadata};
pub use sqlite::{ColumnInfo, ForeignKeyInfo, IndexInfo, SqliteMetadata, TableInfo};
pub use stats::{BlobStats, BooleanStats, DateStats, NumericStats, TextStats};
pub use structured::{
    ArrowIpcMetadata, AvroMetadata, ColumnStat, ColumnarCommonFields, CsvMetadata, EpubMetadata,
    Hdf5DatasetSummary, Hdf5Metadata, HtmlMetadata, JsonMetadata, MergeColumnStatsInput,
    MtxMetadata, NetCdfAttributeEntry, NetCdfMetadata, NetCdfVariableSummary, NpyLayoutSummary,
    NpyMetadata, NpzMetadata, NpzNpyEntrySummary, OrcMetadata, ParquetMetadata, PdfMetadata,
    merge_column_stats,
};
