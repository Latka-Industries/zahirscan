//! Office Open XML formats: DOCX, PPTX, XLSX.
//! Shared utilities in `utils`, constants in `constants`.

use crate::engine::config::Config;
use crate::parsers::{FileType, ParseResult};
use crate::results::MiningResult;
use anyhow::Result;
use memmap2::Mmap;

mod constants;
mod docx;
mod pptx;
mod utils;
mod xlsx;

pub use docx::{extract_docx_metadata, extract_docx_templates};
pub use pptx::{extract_pptx_metadata, extract_pptx_templates};
pub use xlsx::{extract_xlsx_metadata, extract_xlsx_templates};

/// Dispatch by file type; fills docx_metadata or pptx_metadata and returns templates.
pub fn process(stats: &mut ParseResult, mmap: &Mmap, config: &Config) -> Result<MiningResult> {
    match stats.file_type {
        FileType::Docx => crate::process_with_metadata!(
            stats,
            mmap,
            config,
            docx_metadata,
            extract_docx_metadata(mmap, stats, config),
            crate::results::DocumentMetadata,
            FileType::Docx,
            extract_docx_templates(mmap, stats, config)
        ),
        FileType::Xlsx => crate::process_with_metadata!(
            stats,
            mmap,
            config,
            docx_metadata,
            extract_xlsx_metadata(mmap, stats, config),
            crate::results::DocumentMetadata,
            FileType::Xlsx,
            extract_xlsx_templates(mmap, stats, config)
        ),
        FileType::Pptx => crate::process_with_metadata!(
            stats,
            mmap,
            config,
            pptx_metadata,
            extract_pptx_metadata(mmap, stats, config),
            crate::results::PptxMetadata,
            FileType::Pptx,
            extract_pptx_templates(mmap, stats, config)
        ),
        _ => unreachable!("office::process called with {:?}", stats.file_type),
    }
}
