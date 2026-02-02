//! PPTX (PowerPoint) metadata extraction

use std::io::Read;

use super::constants::{OFFICE_CORE_XML, PPTX_CORE_PROPERTIES};
use super::utils::{has_namespace, open_office_archive};
use crate::config::RuntimeConfig;
use crate::parsers::ParseResult;
use crate::results::PptxMetadata;
use anyhow::Result;
use log::warn;
use quick_xml::Reader;
use quick_xml::events::Event;

/// Extract PPTX metadata from file content.
/// PPTX is a ZIP (OOXML) containing docProps/core.xml (title, author, dates) and
/// ppt/presentation.xml (p:sldIdLst with p:sldId per slide).
pub fn extract_pptx_metadata(
    content: &[u8],
    stats: &ParseResult,
    _config: &RuntimeConfig,
) -> Result<PptxMetadata> {
    let mut metadata = PptxMetadata {
        file_size: Some(stats.byte_count),
        ..Default::default()
    };

    let mut archive = match open_office_archive(content, &stats.file_path) {
        Ok(arch) => arch,
        Err(_) => return Ok(metadata),
    };

    if let Ok(mut f) = archive.by_name(OFFICE_CORE_XML) {
        let mut xml = String::new();
        if f.read_to_string(&mut xml).is_ok() {
            extract_core_properties(&xml, &mut metadata);
        }
    }

    if let Ok(mut f) = archive.by_name("ppt/presentation.xml") {
        let mut xml = String::new();
        if f.read_to_string(&mut xml).is_ok() {
            metadata.slide_count = Some(count_sld_id_elements(&xml));
        }
    }

    Ok(metadata)
}

/// Extract core properties from PPTX core.xml
/// Add presentation title, author, creation date, and modification date to metadata
fn extract_core_properties(xml: &str, m: &mut PptxMetadata) {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut text = String::new();
    let mut in_element = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref _e)) => {
                in_element = true;
                text.clear();
            }
            Ok(Event::Text(e)) => {
                if in_element && let Ok(s) = std::str::from_utf8(e.as_ref()) {
                    text.push_str(s);
                }
            }
            Ok(Event::End(ref e)) => {
                let name = e.name().as_ref().to_vec();
                let v = text.trim();
                for prop in PPTX_CORE_PROPERTIES {
                    if name.ends_with(prop.element) && has_namespace(&name, prop.namespace) {
                        if !v.is_empty() {
                            (prop.setter)(m, v.to_string());
                        }
                        break;
                    }
                }
                in_element = false;
                text.clear();
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                warn!("PPTX core.xml parse error: {}", e);
                break;
            }
            _ => {}
        }
        buf.clear();
    }
}

/// Count slides in PPTX presentation.xml
/// Returns number of slide IDs (p:sldId elements)
fn count_sld_id_elements(xml: &str) -> usize {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut count = 0usize;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                if e.name().as_ref().ends_with(b"sldId") {
                    count += 1;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    count
}

crate::no_template_mining!(extract_pptx_templates, "PPTX: slides; no template mining.");
