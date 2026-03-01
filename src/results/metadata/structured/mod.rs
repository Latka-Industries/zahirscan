//! Structured data metadata (CSV, EPUB, HTML, JSON, PDF)

pub mod csv;
pub mod epub;
pub mod html;
pub mod json;
pub mod pdf;

pub use csv::CsvMetadata;
pub use epub::EpubMetadata;
pub use html::HtmlMetadata;
pub use json::JsonMetadata;
pub use pdf::PdfMetadata;
