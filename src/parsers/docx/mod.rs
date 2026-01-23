//! DOCX file text extraction and metadata
//!

use super::ParseResult;
use super::traits::empty_mining_result;
use crate::config::Config;
use crate::results::DocumentMetadata;
use anyhow::Result;
use log::warn;
use quick_xml::Reader;
use quick_xml::escape::unescape;
use quick_xml::events::Event;
use std::io::Cursor;
use zip::ZipArchive;

/// Extract DOCX metadata and text content
pub fn extract_docx_metadata(
    content: &[u8],
    stats: &ParseResult,
    _config: &Config,
) -> Result<DocumentMetadata> {
    let mut metadata = DocumentMetadata {
        file_size: Some(stats.byte_count),
        format: Some("DOCX".to_string()),
        ..Default::default()
    };

    // DOCX is a ZIP archive - open it
    let cursor = Cursor::new(content);
    let mut archive = match ZipArchive::new(cursor) {
        Ok(arch) => arch,
        Err(e) => {
            warn!(
                "Failed to open DOCX as ZIP archive {}: {:?}",
                stats.file_path, e
            );
            return Ok(metadata);
        }
    };

    // Read word/document.xml from the archive
    let document_xml = match archive.by_name("word/document.xml") {
        Ok(mut file) => {
            let mut xml_content = String::new();
            if let Err(e) = std::io::Read::read_to_string(&mut file, &mut xml_content) {
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
        if std::io::Read::read_to_string(&mut file, &mut xml_content).is_ok() {
            extract_core_properties(&xml_content, &mut metadata);
        }
    }

    Ok(metadata)
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
                // Match element names (handles namespaces like dc:, dcterms:, cp:)
                if name.ends_with(b"title") && name.windows(2).any(|w| w == b"dc") {
                    if !text_content.trim().is_empty() {
                        metadata.title = Some(text_content.trim().to_string());
                    }
                } else if name.ends_with(b"creator") && name.windows(2).any(|w| w == b"dc") {
                    if !text_content.trim().is_empty() {
                        metadata.author = Some(text_content.trim().to_string());
                    }
                } else if name.ends_with(b"subject") && name.windows(2).any(|w| w == b"dc") {
                    if !text_content.trim().is_empty() {
                        metadata.subject = Some(text_content.trim().to_string());
                    }
                } else if name.ends_with(b"description") && name.windows(2).any(|w| w == b"dc") {
                    if !text_content.trim().is_empty() {
                        metadata.description = Some(text_content.trim().to_string());
                    }
                } else if name.ends_with(b"created") && name.windows(7).any(|w| w == b"dcterms") {
                    if !text_content.trim().is_empty() {
                        metadata.creation_date = Some(text_content.trim().to_string());
                    }
                } else if name.ends_with(b"modified") && name.windows(7).any(|w| w == b"dcterms") {
                    if !text_content.trim().is_empty() {
                        metadata.modified_date = Some(text_content.trim().to_string());
                    }
                } else if name.ends_with(b"lastModifiedBy") && name.windows(2).any(|w| w == b"cp") {
                    if !text_content.trim().is_empty() {
                        metadata.last_modified_by = Some(text_content.trim().to_string());
                    }
                } else if name.ends_with(b"revision")
                    && name.windows(2).any(|w| w == b"cp")
                    && let Ok(rev) = text_content.trim().parse::<i64>()
                {
                    metadata.revision = Some(rev);
                }
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
                    // Use quick-xml's unescape function to decode XML entities
                    if let Ok(utf8_str) = std::str::from_utf8(e.as_ref()) {
                        match unescape(utf8_str) {
                            Ok(decoded) => text.push_str(&decoded),
                            Err(_) => {
                                // Fallback: manual entity replacement if unescape fails
                                let decoded = utf8_str
                                    .replace("&amp;", "&")
                                    .replace("&lt;", "<")
                                    .replace("&gt;", ">")
                                    .replace("&quot;", "\"")
                                    .replace("&apos;", "'");
                                text.push_str(&decoded);
                            }
                        }
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
