//! Office document metadata structures (DOCX, XLSX, PPTX)

pub mod document;
pub mod pptx;

pub use document::DocumentMetadata;
pub use pptx::PptxMetadata;
