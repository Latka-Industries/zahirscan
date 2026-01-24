//! DOCX file text extraction and metadata
//!

mod utils;

use super::ParseResult;
use super::traits::empty_mining_result;
use crate::config::Config;
use crate::results::DocumentMetadata;
use anyhow::Result;
use log::warn;
use quick_xml::Reader;
use quick_xml::events::Event;
use std::io::Read;
use utils::{decode_xml_entities, has_namespace, open_office_archive, set_metadata_field};

/// Extract DOCX metadata and text content
pub fn extract_docx_metadata(
    content: &[u8],
    stats: &ParseResult,
    _config: &Config,
) -> Result<DocumentMetadata> {
    let mut metadata = DocumentMetadata {
        file_size: Some(stats.byte_count),
        ..Default::default()
    };

    // DOCX is a ZIP archive - open it
    let mut archive = match open_office_archive(content, &stats.file_path) {
        Ok(arch) => arch,
        Err(_) => return Ok(metadata),
    };

    // Read word/document.xml from the archive
    let document_xml = match archive.by_name("word/document.xml") {
        Ok(mut file) => {
            let mut xml_content = String::new();
            if let Err(e) = file.read_to_string(&mut xml_content) {
                warn!(
                    "Failed to read word/document.xml from DOCX {}: {:?}",
                    stats.file_path, e
                );
                return Ok(metadata);
            }
            xml_content
        }
        Err(e) => {
            warn!(
                "word/document.xml not found in DOCX {}: {:?}",
                stats.file_path, e
            );
            return Ok(metadata);
        }
    };

    // Parse XML and extract text, counting words, characters, paragraphs
    let (_text, word_count, character_count, character_count_no_spaces, paragraph_count) =
        extract_text_from_document_xml(&document_xml);

    metadata.word_count = Some(word_count);
    metadata.character_count = Some(character_count);
    metadata.character_count_no_spaces = Some(character_count_no_spaces);
    metadata.paragraph_count = Some(paragraph_count);

    // Extract core properties from docProps/core.xml
    if let Ok(mut file) = archive.by_name("docProps/core.xml") {
        let mut xml_content = String::new();
        if file.read_to_string(&mut xml_content).is_ok() {
            extract_core_properties(&xml_content, &mut metadata);
        }
    }

    Ok(metadata)
}

/// Process a core property element and update metadata if it matches known properties
fn process_core_property(name: &[u8], text_content: &str, metadata: &mut DocumentMetadata) {
    // Check all defined properties
    for prop in utils::CORE_PROPERTIES {
        if name.ends_with(prop.element) && has_namespace(name, prop.namespace) {
            set_metadata_field(metadata, text_content, prop.setter);
            return;
        }
    }

    // Special case for revision (needs parsing to i64)
    if name.ends_with(b"revision")
        && has_namespace(name, b"cp")
        && let Ok(rev) = text_content.trim().parse::<i64>()
    {
        metadata.revision = Some(rev);
    }
}

/// Extract core properties from docProps/core.xml
/// Handles properties like: title, creator, subject, description, created, modified, lastModifiedBy, revision
fn extract_core_properties(xml: &str, metadata: &mut DocumentMetadata) {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut text_content = String::new();
    let mut in_element = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref _e)) => {
                in_element = true;
                text_content.clear();
            }
            Ok(Event::Text(e)) => {
                if in_element && let Ok(text) = std::str::from_utf8(e.as_ref()) {
                    text_content.push_str(text);
                }
            }
            Ok(Event::End(ref e)) => {
                let name = e.name().as_ref().to_vec();
                let text = text_content.as_str();
                process_core_property(&name, text, metadata);
                in_element = false;
                text_content.clear();
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                warn!("Error parsing DOCX core properties XML: {:?}", e);
                break;
            }
            _ => {}
        }
        buf.clear();
    }
}

/// Extract plain text from DOCX document.xml
///
/// DOCX structure: `<w:p>` (paragraph) -> `<w:r>` (run) -> `<w:t>` (text)
///
/// Simple approach: extract all `<w:t>` text elements, decode XML entities with `unescape()`,
/// concatenate directly, and add newlines at paragraph boundaries.
///
/// Returns (text, word_count, character_count, character_count_no_spaces, paragraph_count)
fn extract_text_from_document_xml(xml: &str) -> (String, usize, usize, usize, usize) {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut text = String::new();
    let mut buf = Vec::new();
    let mut in_text_element = false;
    let mut paragraph_count = 0;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                // Check if this is a text element (w:t)
                if e.name().as_ref() == b"w:t" {
                    in_text_element = true;
                }
            }
            Ok(Event::Text(e)) => {
                if in_text_element {
                    // Decode XML entities (with fallback)
                    if let Ok(utf8_str) = std::str::from_utf8(e.as_ref()) {
                        let decoded = decode_xml_entities(utf8_str);
                        text.push_str(&decoded);
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let name = e.name();
                if name.as_ref() == b"w:t" {
                    in_text_element = false;
                } else if name.as_ref() == b"w:p" {
                    // End of paragraph, add newline
                    text.push('\n');
                    paragraph_count += 1;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                warn!("Error parsing DOCX XML: {:?}", e);
                break;
            }
            _ => {}
        }
        buf.clear();
    }

    // Count words and characters
    let word_count = text.split_whitespace().count();
    let character_count = text.chars().count();
    let character_count_no_spaces = text.chars().filter(|c| !c.is_whitespace()).count();

    (
        text,
        word_count,
        character_count,
        character_count_no_spaces,
        paragraph_count,
    )
}

/// Extract templates from DOCX files
/// DOCX files don't need template extraction - we only extract metadata
pub fn extract_docx_templates(
    _content: &[u8],
    stats: &ParseResult,
    _config: &Config,
) -> Result<super::MiningResult> {
    // DOCX files don't need template mining - return empty result
    // Only metadata is extracted (word count, character count, core properties, etc.)
    Ok(empty_mining_result(stats))
}
