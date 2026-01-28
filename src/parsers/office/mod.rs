//! Office Open XML formats: DOCX, PPTX, XLSX.
//! Shared utilities in `utils`, constants in `constants`.

mod constants;
mod docx;
mod pptx;
mod utils;
mod xlsx;

pub use docx::{extract_docx_metadata, extract_docx_templates};
pub use pptx::{extract_pptx_metadata, extract_pptx_templates};
pub use xlsx::{extract_xlsx_metadata, extract_xlsx_templates};
