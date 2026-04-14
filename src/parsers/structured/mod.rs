//! Structured formats: CSV, HTML, JSON, EPUB, PDF.

mod csv;
mod epub;
mod html;
mod json;
mod pdf;

pub use csv::{
    delimiter_byte_for_reader, detect_delimiter_byte, extract_csv_metadata, extract_csv_templates,
    infer_value_type,
};
pub use epub::{extract_epub_metadata, extract_epub_templates};
pub use html::{extract_html_metadata, extract_html_templates};
pub use json::{extract_json_metadata, extract_json_templates};
pub use pdf::{extract_pdf_metadata, extract_pdf_templates};

use anyhow::Result;
use memmap2::Mmap;

use crate::config::RuntimeConfig;
use crate::parsers::{FileType, ParseResult};
use crate::results::MiningResult;
/// Dispatch by file type; fills `csv_metadata` or `html_metadata` and returns templates.
/// For text-based formats we pass content (&str) so UTF-8 is validated once at the boundary.
///
/// # Errors
///
/// Returns an error if the mmap is not valid UTF-8 where required, or if a structured parser fails.
pub fn process(
    stats: &mut ParseResult,
    mmap: &Mmap,
    config: &RuntimeConfig,
) -> Result<MiningResult> {
    match stats.file_type {
        FileType::Csv => crate::process_with_metadata!(
            stats,
            mmap,
            config,
            csv_metadata,
            extract_csv_metadata(mmap, stats, config),
            crate::results::CsvMetadata,
            FileType::Csv,
            extract_csv_templates(mmap, stats, config)
        ),
        FileType::Json => {
            let content = std::str::from_utf8(mmap)?;
            stats.json_metadata = Some(extract_json_metadata(content, stats));
            extract_json_templates(content, stats, config)
        }
        FileType::Epub => crate::process_with_metadata!(
            stats,
            mmap,
            config,
            epub_metadata,
            extract_epub_metadata(mmap, stats, config),
            crate::results::EpubMetadata,
            FileType::Epub,
            extract_epub_templates(mmap, stats, config)
        ),
        FileType::Html => crate::process_with_metadata!(
            stats,
            mmap,
            config,
            html_metadata,
            extract_html_metadata(mmap, stats, config),
            crate::results::HtmlMetadata,
            FileType::Html,
            extract_html_templates(mmap, stats, config)
        ),
        FileType::Pdf => crate::process_with_metadata!(
            stats,
            mmap,
            config,
            pdf_metadata,
            extract_pdf_metadata(mmap, stats, config),
            crate::results::PdfMetadata,
            FileType::Pdf,
            extract_pdf_templates(mmap, stats, config)
        ),
        _ => unreachable!("structured::process called with {:?}", stats.file_type),
    }
}
