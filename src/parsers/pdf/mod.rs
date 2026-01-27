//! PDF file metadata extraction

mod utils;

use crate::config::Config;
use crate::parsers::ParseResult;
use crate::results::PdfMetadata;
use anyhow::Result;
use log::warn;
use pdf::file::FileOptions;

use self::utils::{extract_pdf_date_to_iso8601, extract_text_str};

/// Extract PDF metadata
pub fn extract_pdf_metadata(
    content: &[u8],
    stats: &ParseResult,
    _config: &Config,
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
        metadata.title = extract_text_str(info.title.as_ref());
        metadata.author = extract_text_str(info.author.as_ref());
        metadata.subject = extract_text_str(info.subject.as_ref());
        metadata.creator = extract_text_str(info.creator.as_ref());
        metadata.producer = extract_text_str(info.producer.as_ref());

        // Dates are Option<pdf::primitive::Date> - convert to ISO 8601 format
        metadata.creation_date = info
            .creation_date
            .as_ref()
            .and_then(extract_pdf_date_to_iso8601);
        metadata.modification_date = info.mod_date.as_ref().and_then(extract_pdf_date_to_iso8601);
    }

    Ok(metadata)
}

crate::no_template_mining!(
    extract_pdf_templates,
    "PDFs are binary files, don't have templates, return empty result."
);
