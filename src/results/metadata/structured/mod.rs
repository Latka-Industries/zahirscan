//! Structured data metadata (CSV, EPUB, HTML, JSON, PDF, columnar formats, `NumPy`)

pub mod columnar;
pub mod csv;
pub mod epub;
pub mod html;
pub mod json;
pub mod numpy;
pub mod pdf;

pub use columnar::{
    ArrowIpcMetadata, AvroMetadata, ColumnarCommonFields, OrcMetadata, ParquetMetadata,
};
pub use csv::CsvMetadata;
pub use epub::EpubMetadata;
pub use html::HtmlMetadata;
pub use json::JsonMetadata;
pub use numpy::{NpyLayoutSummary, NpyMetadata, NpzMetadata, NpzNpyEntrySummary};
pub use pdf::PdfMetadata;
