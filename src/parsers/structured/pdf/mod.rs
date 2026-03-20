//! PDF file metadata extraction

mod utils;

use crate::config::RuntimeConfig;
use crate::parsers::ParseResult;
use crate::results::PdfMetadata;
use anyhow::Result;
use log::warn;
use pdf::file::FileOptions;

/// Extract document info from PDF info dictionary
fn extract_document_info(info: &pdf::object::InfoDict, metadata: &mut PdfMetadata) {
    metadata.title = utils::extract_text_str(info.title.as_ref());
    metadata.author = utils::extract_text_str(info.author.as_ref());
    metadata.subject = utils::extract_text_str(info.subject.as_ref());
    metadata.creator = utils::extract_text_str(info.creator.as_ref());
    metadata.producer = utils::extract_text_str(info.producer.as_ref());

    // Dates are Option<pdf::primitive::Date> - convert to ISO 8601 format
    metadata.creation_date = info
        .creation_date
        .as_ref()
        .and_then(utils::extract_pdf_date_to_iso8601);
    metadata.modification_date = info
        .mod_date
        .as_ref()
        .and_then(utils::extract_pdf_date_to_iso8601);
}

/// Extract PDF metadata
///
/// # Errors
///
/// Currently always returns [`Ok`]; parse failures yield minimal metadata with a warning.
pub fn extract_pdf_metadata(
    content: &[u8],
    stats: &ParseResult,
    _config: &RuntimeConfig,
) -> Result<PdfMetadata> {
    let mut metadata = PdfMetadata {
        file_size: Some(stats.byte_count),
        ..Default::default()
    };

    // Try to parse the PDF
    // Note: FileOptions::load() requires ownership of the data, so to_vec() is necessary.
    // The PDF crate needs to own the data to parse it, similar to how other parsers work.
    let data = content.to_vec();
    let file = match FileOptions::cached().load(data) {
        Ok(f) => f,
        Err(e) => {
            // If parsing fails, log the error and return minimal metadata with file size.
            warn!("Failed to parse PDF {}: {:?}", stats.file_path, e);
            return Ok(metadata);
        }
    };

    // Extract PDF version
    metadata.pdf_version = file.version().ok();

    // Extract page count (convert u32 to usize)
    metadata.page_count = Some(file.num_pages() as usize);

    // Check encryption status
    metadata.is_encrypted = Some(file.trailer.encrypt_dict.is_some());

    // Extract document info (metadata dictionary) if present
    if let Some(ref info) = file.trailer.info_dict {
        extract_document_info(info, &mut metadata);
    }

    Ok(metadata)
}

crate::no_template_mining!(
    extract_pdf_templates,
    "PDFs are binary files, don't have templates, return empty result."
);
