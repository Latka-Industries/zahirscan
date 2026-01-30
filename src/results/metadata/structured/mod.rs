//! Structured data metadata (CSV, EPUB, HTML, PDF)

pub mod csv;
pub mod epub;
pub mod html;
pub mod pdf;

pub use csv::CsvMetadata;
pub use epub::EpubMetadata;
pub use html::HtmlMetadata;
pub use pdf::PdfMetadata;
