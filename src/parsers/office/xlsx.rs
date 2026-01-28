//! XLSX file metadata extraction

use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;

use super::constants::{
    CP_NAMESPACE, DOCX_CORE_PROPERTIES, OFFICE_CORE_XML, REVISION_ELEMENT, XLSX_APP_XML,
    XLSX_ATTR_NAME, XLSX_LPSTR, XLSX_SHEET, XLSX_WORKBOOK_XML,
};
use super::utils::{has_namespace, open_office_archive, read_xml_from_archive};
use crate::engine::config::Config;
use crate::parsers::ParseResult;
use crate::results::DocumentMetadata;
use anyhow::Result;
use calamine::{Reader, Xlsx, open_workbook};
use log::warn;
use quick_xml::Reader as XmlReader;
use quick_xml::events::Event;

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

    let mut archive = match open_office_archive(content, &stats.file_path) {
        Ok(arch) => arch,
        Err(_) => return Ok(metadata),
    };

    read_xml_from_archive(
        &mut archive,
        OFFICE_CORE_XML,
        "XLSX",
        &stats.file_path,
        |xml| extract_core_properties(xml, &mut metadata),
    );

    read_xml_from_archive(
        &mut archive,
        XLSX_APP_XML,
        "XLSX",
        &stats.file_path,
        |xml| extract_app_properties(xml, &mut metadata),
    );

    read_xml_from_archive(
        &mut archive,
        XLSX_WORKBOOK_XML,
        "XLSX",
        &stats.file_path,
        |xml| extract_sheet_info(xml, &mut metadata),
    );

    extract_row_column_counts(&stats.file_path, &mut metadata)?;

    Ok(metadata)
}

fn extract_row_column_counts(file_path: &str, metadata: &mut DocumentMetadata) -> Result<()> {
    let mut workbook: Xlsx<BufReader<File>> = match open_workbook(file_path) {
        Ok(wb) => wb,
        Err(e) => {
            warn!("Failed to open XLSX with calamine: {:?}", e);
            return Ok(());
        }
    };

    let sheet_names = workbook.sheet_names();
    let mut sheet_stats = HashMap::new();

    for sheet_name in &sheet_names {
        if let Ok(range) = workbook.worksheet_range(sheet_name) {
            let (rows, cols) = range.get_size();
            sheet_stats.insert(
                sheet_name.clone(),
                serde_json::json!({ "rows": rows, "columns": cols }),
            );
        }
    }

    if !sheet_stats.is_empty() {
        metadata.sheet_stats = Some(serde_json::to_value(&sheet_stats).unwrap_or_default());
    }

    Ok(())
}

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
                process_core_property(&name, text_content.as_str(), metadata);
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
                if name.ends_with(XLSX_LPSTR) {
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
                if name.ends_with(XLSX_LPSTR) && in_sheet_name {
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

    if !sheet_names.is_empty() {
        metadata.sheet_count = Some(sheet_names.len());
    }
}

fn extract_sheet_info(xml: &str, metadata: &mut DocumentMetadata) {
    let mut reader = XmlReader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut sheet_names = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let name = e.name().as_ref().to_vec();
                if name.ends_with(XLSX_SHEET) {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == XLSX_ATTR_NAME
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

    if !sheet_names.is_empty() {
        metadata.sheet_count = Some(sheet_names.len());
    }
}

crate::no_template_mining!(
    extract_xlsx_templates,
    "XLSX files don't need template mining - return empty result. Only metadata is extracted (core properties, sheet info, etc.)."
);
