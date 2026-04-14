//! PDF file metadata extraction

mod utils;

use anyhow::Result;
use log::warn;
use lopdf::{Dictionary, Document, decode_text_string};

use crate::config::RuntimeConfig;
use crate::parsers::ParseResult;
use crate::results::PdfMetadata;

/// PDF name objects for the trailer and Info dictionary (PDF 32000-2).
struct PdfName;
impl PdfName {
    const TRAILER_INFO: &'static [u8] = b"Info";
    const TRAILER_ENCRYPT: &'static [u8] = b"Encrypt";

    const TITLE: &'static [u8] = b"Title";
    const AUTHOR: &'static [u8] = b"Author";
    const SUBJECT: &'static [u8] = b"Subject";
    const CREATOR: &'static [u8] = b"Creator";
    const PRODUCER: &'static [u8] = b"Producer";
    const CREATION_DATE: &'static [u8] = b"CreationDate";
    const MOD_DATE: &'static [u8] = b"ModDate";
}

fn info_text(dict: &Dictionary, key: &[u8]) -> Option<String> {
    dict.get(key)
        .ok()
        .and_then(|obj| decode_text_string(obj).ok())
}

fn info_dict(doc: &Document) -> Option<&Dictionary> {
    let info_obj = doc.trailer.get(PdfName::TRAILER_INFO).ok()?;
    let (_, obj) = doc.dereference(info_obj).ok()?;
    obj.as_dict().ok()
}

/// Extract document info from PDF Info dictionary
fn extract_document_info(dict: &Dictionary, metadata: &mut PdfMetadata) {
    metadata.title = info_text(dict, PdfName::TITLE);
    metadata.author = info_text(dict, PdfName::AUTHOR);
    metadata.subject = info_text(dict, PdfName::SUBJECT);
    metadata.creator = info_text(dict, PdfName::CREATOR);
    metadata.producer = info_text(dict, PdfName::PRODUCER);

    metadata.creation_date = info_text(dict, PdfName::CREATION_DATE)
        .as_deref()
        .and_then(utils::format_pdf_date);
    metadata.modification_date = info_text(dict, PdfName::MOD_DATE)
        .as_deref()
        .and_then(utils::format_pdf_date);
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

    let doc = match Document::load_mem(content) {
        Ok(d) => d,
        Err(e) => {
            warn!("Failed to parse PDF {}: {:?}", stats.file_path, e);
            return Ok(metadata);
        }
    };

    metadata.pdf_version = Some(doc.version.clone());
    metadata.page_count = Some(doc.get_pages().len());
    metadata.is_encrypted = Some(doc.trailer.get(PdfName::TRAILER_ENCRYPT).is_ok());

    if let Some(dict) = info_dict(&doc) {
        extract_document_info(dict, &mut metadata);
    }

    Ok(metadata)
}

crate::no_template_mining!(
    extract_pdf_templates,
    "PDFs are binary files, don't have templates, return empty result."
);
