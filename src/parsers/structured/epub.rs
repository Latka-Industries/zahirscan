//! EPUB (e-book) metadata extraction

use std::io::{Cursor, Read};

use crate::config::RuntimeConfig;
use crate::parsers::ParseResult;
use crate::results::EpubMetadata;
use anyhow::Result;
use log::warn;
use quick_xml::Reader;
use quick_xml::events::Event;
use zip::ZipArchive;

/// EPUB file paths and locations
struct EpubPaths;

impl EpubPaths {
    const CONTAINER_XML: &'static str = "META-INF/container.xml";
    const COMMON_OPF_PATHS: [&'static str; 3] =
        ["OEBPS/content.opf", "content.opf", "EPUB/package.opf"];
}

/// EPUB XML element and attribute names (as byte slices for quick_xml)
struct EpubElements;

impl EpubElements {
    const ROOTFILE: &'static [u8] = b"rootfile";
    const FULL_PATH: &'static [u8] = b"full-path";
    const FULL_PATH_ALT: &'static [u8] = b"full_path";
    const METADATA: &'static [u8] = b"metadata";
    const TITLE: &'static [u8] = b"title";
    const CREATOR: &'static [u8] = b"creator";
    const LANGUAGE: &'static [u8] = b"language";
    const IDENTIFIER: &'static [u8] = b"identifier";
    const SPINE: &'static [u8] = b"spine";
    const ITEMREF: &'static [u8] = b"itemref";
}

/// Set metadata field if element matches and field is None
macro_rules! set_metadata_field {
    ($local:expr, $value:expr, $metadata:expr, $element:expr => $field:ident) => {
        if $local == $element && $metadata.$field.is_none() {
            $metadata.$field = Some($value.to_string());
        }
    };
}

/// Extract EPUB metadata from file content.
/// EPUB is a ZIP containing META-INF/container.xml (rootfile path) and
/// the rootfile OPF (e.g. content.opf) with <metadata> (dc:title, dc:creator, dc:language, dc:identifier)
/// and <spine> (itemref count for chapters).
pub fn extract_epub_metadata(
    content: &[u8],
    stats: &ParseResult,
    _config: &RuntimeConfig,
) -> Result<EpubMetadata> {
    let metadata = EpubMetadata {
        file_size: Some(stats.byte_count),
        ..Default::default()
    };

    let mut archive = match ZipArchive::new(Cursor::new(content)) {
        Ok(a) => a,
        Err(e) => {
            warn!("EPUB: failed to open as ZIP: {}", e);
            return Ok(metadata);
        }
    };

    let opf_path =
        read_container_rootfile(&mut archive).or_else(|| try_common_opf_paths(&mut archive));

    let opf_path = match opf_path {
        Some(p) => p,
        None => return Ok(metadata),
    };

    parse_opf(&mut archive, &opf_path, metadata)
}

/// Read container.xml file and parse the rootfile path
/// Returns the rootfile path or None if not found
fn read_container_rootfile(archive: &mut ZipArchive<Cursor<&[u8]>>) -> Option<String> {
    let mut f = archive.by_name(EpubPaths::CONTAINER_XML).ok()?;
    let mut xml = String::new();
    f.read_to_string(&mut xml).ok()?;
    parse_container_rootfile(&xml)
}

/// Try common OPF paths in the archive
/// Returns the first path that exists or None if none found
fn try_common_opf_paths(archive: &mut ZipArchive<Cursor<&[u8]>>) -> Option<String> {
    for name in EpubPaths::COMMON_OPF_PATHS {
        if archive.by_name(name).is_ok() {
            return Some(name.to_string());
        }
    }
    None
}

/// Parse container.xml file and extract the rootfile path
/// Returns the rootfile path or None if not found
fn parse_container_rootfile(xml: &str) -> Option<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                if e.name().local_name().as_ref() == EpubElements::ROOTFILE {
                    if let Ok(Some(attr)) = e.try_get_attribute(EpubElements::FULL_PATH) {
                        return Some(String::from_utf8_lossy(&attr.value).into_owned());
                    }
                    if let Ok(Some(attr)) = e.try_get_attribute(EpubElements::FULL_PATH_ALT) {
                        return Some(String::from_utf8_lossy(&attr.value).into_owned());
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    None
}

/// Parse OPF (e-book package) file and extract metadata
/// Returns the metadata and the chapter count
fn parse_opf(
    archive: &mut ZipArchive<Cursor<&[u8]>>,
    path: &str,
    mut metadata: EpubMetadata,
) -> Result<EpubMetadata> {
    let mut f = match archive.by_name(path) {
        Ok(file) => file,
        Err(_) => return Ok(metadata),
    };

    let mut xml = String::new();
    if f.read_to_string(&mut xml).is_err() {
        return Ok(metadata);
    }

    extract_metadata_from_opf(&xml, &mut metadata);
    metadata.chapter_count = Some(count_spine_itemrefs(&xml));

    Ok(metadata)
}

/// Extract metadata from OPF file and set fields in metadata
/// Returns the metadata with extracted fields
fn extract_metadata_from_opf(xml: &str, m: &mut EpubMetadata) {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut text = String::new();
    let mut in_meta = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.name().local_name().as_ref().to_vec();
                in_meta = local.as_slice() == EpubElements::METADATA || in_meta;
                if in_meta {
                    text.clear();
                }
            }
            Ok(Event::End(ref e)) => {
                let local = e.name().local_name().as_ref().to_vec();
                if local.as_slice() == EpubElements::METADATA {
                    in_meta = false;
                }
                if in_meta {
                    let v = text.trim();
                    if !v.is_empty() {
                        let local_slice = local.as_slice();
                        set_metadata_field!(local_slice, v, m, EpubElements::TITLE => title);
                        set_metadata_field!(local_slice, v, m, EpubElements::CREATOR => author);
                        set_metadata_field!(local_slice, v, m, EpubElements::LANGUAGE => language);
                        set_metadata_field!(local_slice, v, m, EpubElements::IDENTIFIER => identifier);
                    }
                    text.clear();
                }
            }
            Ok(Event::Text(e)) => {
                if in_meta && let Ok(s) = std::str::from_utf8(e.as_ref()) {
                    text.push_str(s);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
}

/// Count the number of itemrefs in the spine
/// Returns the count of itemrefs
fn count_spine_itemrefs(xml: &str) -> usize {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut in_spine = false;
    let mut count = 0usize;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.name().local_name().as_ref().to_vec();
                if local.as_slice() == EpubElements::SPINE {
                    in_spine = true;
                } else if in_spine && local.as_slice() == EpubElements::ITEMREF {
                    count += 1;
                }
            }
            Ok(Event::Empty(ref e)) => {
                if in_spine && e.name().local_name().as_ref() == EpubElements::ITEMREF {
                    count += 1;
                }
            }
            Ok(Event::End(ref e)) => {
                if e.name().local_name().as_ref() == EpubElements::SPINE {
                    in_spine = false;
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

crate::no_template_mining!(
    extract_epub_templates,
    "EPUB: book structure; no template mining."
);
