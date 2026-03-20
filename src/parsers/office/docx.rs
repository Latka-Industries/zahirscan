//! DOCX file text extraction and metadata

use std::io::Read;

use super::constants::{CP_NAMESPACE, DOCX_CORE_PROPERTIES, OFFICE_CORE_XML, REVISION_ELEMENT};
use super::utils::{decode_xml_entities, has_namespace, open_office_archive};
use crate::config::RuntimeConfig;
use crate::parsers::ParseResult;
use crate::results::DocumentMetadata;
use log::warn;
use quick_xml::Reader;
use quick_xml::events::Event;

/// Extract DOCX metadata and text content
pub fn extract_docx_metadata(
    content: &[u8],
    stats: &ParseResult,
    _config: &RuntimeConfig,
) -> DocumentMetadata {
    let mut metadata = DocumentMetadata {
        file_size: Some(stats.byte_count),
        ..Default::default()
    };

    let Ok(mut archive) = open_office_archive(content, &stats.file_path) else {
        return metadata;
    };

    let document_xml = match archive.by_name("word/document.xml") {
        Ok(mut file) => {
            let mut xml_content = String::new();
            if let Err(e) = file.read_to_string(&mut xml_content) {
                warn!(
                    "Failed to read word/document.xml from DOCX {}: {:?}",
                    stats.file_path, e
                );
                return metadata;
            }
            xml_content
        }
        Err(e) => {
            warn!(
                "word/document.xml not found in DOCX {}: {:?}",
                stats.file_path, e
            );
            return metadata;
        }
    };

    let (_text, word_count, character_count, character_count_no_spaces, paragraph_count) =
        extract_text_from_document_xml(&document_xml);

    metadata.word_count = Some(word_count);
    metadata.character_count = Some(character_count);
    metadata.character_count_no_spaces = Some(character_count_no_spaces);
    metadata.paragraph_count = Some(paragraph_count);

    if let Ok(mut file) = archive.by_name(OFFICE_CORE_XML) {
        let mut xml_content = String::new();
        if file.read_to_string(&mut xml_content).is_ok() {
            extract_core_properties(&xml_content, &mut metadata);
        }
    }

    metadata
}

/// Access core properties XML (docProps/core.xml)
/// Add revision number to metadata if present
fn process_core_property(name: &[u8], text_content: &str, metadata: &mut DocumentMetadata) {
    for prop in DOCX_CORE_PROPERTIES {
        if name.ends_with(prop.element) && has_namespace(name, prop.namespace) {
            let v = text_content.trim();
            if !v.is_empty() {
                (prop.setter)(metadata, v.to_string());
            }
            return;
        }
    }
    if name.ends_with(REVISION_ELEMENT)
        && has_namespace(name, CP_NAMESPACE)
        && let Ok(rev) = text_content.trim().parse::<i64>()
    {
        metadata.revision = Some(rev);
    }
}

/// Extract core properties from DOCX core.xml
/// Extract core properties from DOCX docProps/core.xml.
///
/// Parses Dublin Core and CP (Core Properties) metadata using event-based XML parsing.
/// Extracts: title, author, subject, description, creation/modified dates, last modified by, and revision.
/// Best-effort: continues on parse errors, leaves missing properties as None.
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
                process_core_property(&name, text_content.as_str(), metadata);
                in_element = false;
                text_content.clear();
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                warn!("Error parsing DOCX core properties XML: {e:?}");
                break;
            }
            _ => {}
        }
        buf.clear();
    }
}

/// Extract text content from DOCX document.xml
/// Returns word count, character count, character count without spaces, and paragraph count
fn extract_text_from_document_xml(xml: &str) -> (String, usize, usize, usize, usize) {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut text = String::new();
    let mut buf = Vec::new();
    let mut in_text_element = false;
    let mut paragraph_count = 0;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e) | Event::Empty(ref e)) => {
                if e.name().as_ref() == b"w:t" {
                    in_text_element = true;
                }
            }
            Ok(Event::Text(e)) => {
                if in_text_element && let Ok(utf8_str) = std::str::from_utf8(e.as_ref()) {
                    text.push_str(&decode_xml_entities(utf8_str));
                }
            }
            Ok(Event::End(ref e)) => {
                let name = e.name();
                if name.as_ref() == b"w:t" {
                    in_text_element = false;
                } else if name.as_ref() == b"w:p" {
                    text.push('\n');
                    paragraph_count += 1;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                warn!("Error parsing DOCX XML: {e:?}");
                break;
            }
            _ => {}
        }
        buf.clear();
    }

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

crate::no_template_mining!(
    extract_docx_templates,
    "DOCX files don't need template mining - return empty result. Only metadata is extracted (word count, character count, core properties, etc.)."
);
