//! XLSX file metadata extraction
//!

mod utils;

use super::ParseResult;
use super::traits::empty_mining_result;
use crate::config::Config;
use crate::results::DocumentMetadata;
use anyhow::Result;
use calamine::{Reader, Xlsx, open_workbook};
use log::warn;
use quick_xml::Reader as XmlReader;
use quick_xml::events::Event;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use utils::{has_namespace, open_office_archive, read_xml_from_archive, set_metadata_field};

/// Extract XLSX metadata
pub fn extract_xlsx_metadata(
    content: &[u8],
    stats: &ParseResult,
    _config: &Config,
) -> Result<DocumentMetadata> {
    let mut metadata = DocumentMetadata {
        file_size: Some(stats.byte_count),
        ..Default::default()
    };

    // XLSX is a ZIP archive - open it
    let mut archive = match open_office_archive(content, &stats.file_path) {
        Ok(arch) => arch,
        Err(_) => return Ok(metadata),
    };

    // Extract core properties from docProps/core.xml (same structure as DOCX)
    read_xml_from_archive(
        &mut archive,
        utils::xml_files::CORE_PROPERTIES,
        "XLSX",
        &stats.file_path,
        |xml| extract_core_properties(xml, &mut metadata),
    );

    // Extract app properties from docProps/app.xml (sheet names, count, etc.)
    // Note: This may be empty if no metadata was set
    read_xml_from_archive(
        &mut archive,
        utils::xml_files::APP_PROPERTIES,
        "XLSX",
        &stats.file_path,
        |xml| extract_app_properties(xml, &mut metadata),
    );

    // Extract sheet information from xl/workbook.xml (more reliable - always exists)
    read_xml_from_archive(
        &mut archive,
        utils::xml_files::WORKBOOK,
        "XLSX",
        &stats.file_path,
        |xml| extract_sheet_info(xml, &mut metadata),
    );

    // Use calamine to extract row/column counts from actual worksheet data
    extract_row_column_counts(&stats.file_path, &mut metadata)?;

    Ok(metadata)
}

/// Extract row and column counts using calamine
fn extract_row_column_counts(file_path: &str, metadata: &mut DocumentMetadata) -> Result<()> {
    // Use file path - calamine's open_workbook expects a path
    let mut workbook: Xlsx<BufReader<File>> = match open_workbook(file_path) {
        Ok(wb) => wb,
        Err(e) => {
            warn!("Failed to open XLSX with calamine: {:?}", e);
            return Ok(()); // Non-fatal, just skip row/column extraction
        }
    };

    let sheet_names = workbook.sheet_names();
    let mut sheet_stats = HashMap::new();

    for sheet_name in &sheet_names {
        if let Ok(range) = workbook.worksheet_range(sheet_name) {
            let (rows, cols) = range.get_size();

            // Store per-sheet stats
            sheet_stats.insert(
                sheet_name.clone(),
                serde_json::json!({
                    "rows": rows,
                    "columns": cols
                }),
            );
        }
    }

    // Convert HashMap to JSON Value for proper nested serialization
    if !sheet_stats.is_empty() {
        metadata.sheet_stats = Some(serde_json::to_value(&sheet_stats).unwrap_or_default());
    }

    Ok(())
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
    if name.ends_with(utils::core_elements::REVISION)
        && has_namespace(name, utils::core_namespaces::CP)
        && let Ok(rev) = text_content.trim().parse::<i64>()
    {
        metadata.revision = Some(rev);
    }
}

/// Extract core properties from docProps/core.xml
/// Same structure as DOCX - can reuse the same logic
/// Handles properties like: title, creator, subject, description, created, modified, lastModifiedBy, revision
fn extract_core_properties(xml: &str, metadata: &mut DocumentMetadata) {
    let mut reader = XmlReader::from_str(xml);
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
                warn!("Error parsing XLSX core properties XML: {:?}", e);
                break;
            }
            _ => {}
        }
        buf.clear();
    }
}

/// Extract app properties from docProps/app.xml
/// Extracts sheet names and count from <vt:lpstr> elements in <TitlesOfParts>
fn extract_app_properties(xml: &str, metadata: &mut DocumentMetadata) {
    let mut reader = XmlReader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut text_content = String::new();
    let mut in_sheet_name = false;
    let mut sheet_names = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = e.name().as_ref().to_vec();
                // Check if this is a sheet name element (vt:lpstr within TitlesOfParts)
                if name.ends_with(utils::elements::LPSTR) {
                    in_sheet_name = true;
                    text_content.clear();
                }
            }
            Ok(Event::Text(e)) => {
                if in_sheet_name && let Ok(text) = std::str::from_utf8(e.as_ref()) {
                    text_content.push_str(text);
                }
            }
            Ok(Event::End(ref e)) => {
                let name = e.name().as_ref().to_vec();
                if name.ends_with(utils::elements::LPSTR) && in_sheet_name {
                    if !text_content.trim().is_empty() {
                        sheet_names.push(text_content.trim().to_string());
                    }
                    in_sheet_name = false;
                    text_content.clear();
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                warn!("Error parsing XLSX app properties XML: {:?}", e);
                break;
            }
            _ => {}
        }
        buf.clear();
    }

    // Store sheet count
    if !sheet_names.is_empty() {
        metadata.sheet_count = Some(sheet_names.len());
    }
}

/// Extract sheet information from xl/workbook.xml
/// This is more reliable than app.xml as workbook.xml always exists
/// Extracts sheet names and count from <sheet> elements
fn extract_sheet_info(xml: &str, metadata: &mut DocumentMetadata) {
    let mut reader = XmlReader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut sheet_names = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let name = e.name().as_ref().to_vec();
                // Check if this is a sheet element
                if name.ends_with(utils::elements::SHEET) {
                    // Extract the "name" attribute from the sheet element
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == utils::attributes::NAME
                            && let Ok(sheet_name) = std::str::from_utf8(attr.value.as_ref())
                        {
                            sheet_names.push(sheet_name.to_string());
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                warn!("Error parsing XLSX workbook XML: {:?}", e);
                break;
            }
            _ => {}
        }
        buf.clear();
    }

    // Store sheet count and names
    if !sheet_names.is_empty() {
        metadata.sheet_count = Some(sheet_names.len());
    }
}

/// Extract templates from XLSX files
/// XLSX files don't need template extraction - we only extract metadata
pub fn extract_xlsx_templates(
    _content: &[u8],
    stats: &ParseResult,
    _config: &Config,
) -> Result<super::MiningResult> {
    // XLSX files don't need template mining - return empty result
    // Only metadata is extracted (core properties, sheet info, etc.)
    Ok(empty_mining_result(stats))
}
