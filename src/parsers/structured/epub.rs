//! EPUB (e-book) metadata and body text extraction

use anyhow::Result;
use log::{debug, warn};
use quick_xml::Reader;
use quick_xml::events::{BytesEnd, BytesStart, Event};
use std::collections::HashMap;
use std::io::Cursor;
use zip::ZipArchive;

use crate::config::RuntimeConfig;
use crate::parsers::{
    ParseResult, structured::html, text::plain_text::extract_text_templates,
    traits::empty_mining_result,
};
use crate::results::{EpubMetadata, MiningResult};
use crate::utils::zip_read::read_zip_entry_to_string_limited;

/// Return the value of the first attribute whose name is in `names`, or None.
fn get_first_attr_value(e: &BytesStart, names: &[&[u8]]) -> Option<String> {
    for name in names {
        if let Ok(Some(attr)) = e.try_get_attribute(*name) {
            return Some(String::from_utf8_lossy(&attr.value).into_owned());
        }
    }
    None
}

/// True if this Start/Empty element's local name equals `name`.
fn start_local_name_eq(e: &BytesStart, name: &[u8]) -> bool {
    let local = e.name().local_name();
    local.as_ref() == name
}

/// True if this End element's local name equals `name`.
fn end_local_name_eq(e: &BytesEnd, name: &[u8]) -> bool {
    let local = e.name().local_name();
    local.as_ref() == name
}

/// EPUB file paths and locations
struct EpubPaths;

impl EpubPaths {
    const CONTAINER_XML: &'static str = "META-INF/container.xml";
    /// Present when the EPUB contains encrypted resources (DRM); skip parsing/writing analysis.
    const ENCRYPTION_XML: &'static str = "META-INF/encryption.xml";
    const COMMON_OPF_PATHS: [&'static str; 3] =
        ["OEBPS/content.opf", "content.opf", "EPUB/package.opf"];
}

/// True if the EPUB has META-INF/encryption.xml (encrypted/DRM content). We skip parsing and writing analysis.
fn epub_has_encryption(archive: &mut ZipArchive<Cursor<&[u8]>>) -> bool {
    archive.by_name(EpubPaths::ENCRYPTION_XML).is_ok()
}

/// EPUB XML element and attribute names (as byte slices for `quick_xml`)
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
    const ITEM: &'static [u8] = b"item";
    const ID: &'static [u8] = b"id";
    const HREF: &'static [u8] = b"href";
    const IDREF: &'static [u8] = b"idref";
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
///
/// # Errors
///
/// Currently always returns [`Ok`]; ZIP/OPF failures yield partial metadata and warnings.
pub fn extract_epub_metadata(
    content: &[u8],
    stats: &ParseResult,
    config: &RuntimeConfig,
) -> Result<EpubMetadata> {
    let metadata = EpubMetadata {
        file_size: Some(stats.byte_count),
        ..Default::default()
    };

    let mut archive = match ZipArchive::new(Cursor::new(content)) {
        Ok(a) => a,
        Err(e) => {
            warn!("EPUB: failed to open as ZIP: {e}");
            return Ok(metadata);
        }
    };

    if epub_has_encryption(&mut archive) {
        debug!("EPUB: encryption.xml present, skipping metadata extraction");
        return Ok(metadata);
    }

    let max = config.max_zip_entry_uncompressed_bytes;
    let opf_path = read_container_rootfile(&mut archive, max, &stats.file_path)
        .or_else(|| try_common_opf_paths(&mut archive));

    let Some(opf_path) = opf_path else {
        return Ok(metadata);
    };

    Ok(parse_opf(
        &mut archive,
        &opf_path,
        metadata,
        max,
        &stats.file_path,
    ))
}

/// Read container.xml file and parse the rootfile path
/// Returns the rootfile path or None if not found
fn read_container_rootfile(
    archive: &mut ZipArchive<Cursor<&[u8]>>,
    max_uncompressed_bytes: usize,
    epub_path: &str,
) -> Option<String> {
    let f = archive.by_name(EpubPaths::CONTAINER_XML).ok()?;
    let xml = read_zip_entry_to_string_limited(
        f,
        max_uncompressed_bytes,
        EpubPaths::CONTAINER_XML,
        epub_path,
    )
    .ok()?;
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
            Ok(Event::Empty(ref e) | Event::Start(ref e)) => {
                if start_local_name_eq(e, EpubElements::ROOTFILE)
                    && let Some(path) = get_first_attr_value(
                        e,
                        &[EpubElements::FULL_PATH, EpubElements::FULL_PATH_ALT],
                    )
                {
                    return Some(path);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    None
}

/// Parse OPF (e-book package) file, populate metadata (including chapter count), and return it.
fn parse_opf(
    archive: &mut ZipArchive<Cursor<&[u8]>>,
    path: &str,
    mut metadata: EpubMetadata,
    max_uncompressed_bytes: usize,
    epub_path: &str,
) -> EpubMetadata {
    let Ok(f) = archive.by_name(path) else {
        return metadata;
    };

    let Ok(xml) = read_zip_entry_to_string_limited(f, max_uncompressed_bytes, path, epub_path)
    else {
        return metadata;
    };

    extract_metadata_from_opf(&xml, &mut metadata);
    metadata.chapter_count = Some(count_spine_itemrefs(&xml));

    metadata
}

/// Extract metadata from OPF XML and set fields on the given metadata.
fn extract_metadata_from_opf(xml: &str, m: &mut EpubMetadata) {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut text = String::new();
    let mut in_meta = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                in_meta = start_local_name_eq(e, EpubElements::METADATA) || in_meta;
                if in_meta {
                    text.clear();
                }
            }
            Ok(Event::End(ref e)) => {
                if end_local_name_eq(e, EpubElements::METADATA) {
                    in_meta = false;
                }
                if in_meta {
                    let v = text.trim();
                    if !v.is_empty() {
                        let local_name = e.name().local_name();
                        let local_slice = local_name.as_ref();
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
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
}

/// Count the number of itemrefs in the spine
/// Returns the count of itemrefs
fn count_spine_itemrefs(xml: &str) -> usize {
    parse_spine_order_from_opf(xml).len()
}

/// Parse OPF manifest: map item id -> href (path relative to OPF).
fn parse_manifest_from_opf(xml: &str) -> HashMap<String, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut manifest = HashMap::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e) | Event::Start(ref e)) => {
                if !start_local_name_eq(e, EpubElements::ITEM) {
                    buf.clear();
                    continue;
                }
                let id = get_first_attr_value(e, &[EpubElements::ID]);
                let href = get_first_attr_value(e, &[EpubElements::HREF]);
                if let (Some(id), Some(href)) = (id, href) {
                    manifest.insert(id, href);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    manifest
}

/// Parse OPF spine: ordered list of item idrefs (reading order).
fn parse_spine_order_from_opf(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut in_spine = false;
    let mut order = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e) | Event::Start(ref e)) => {
                if start_local_name_eq(e, EpubElements::SPINE) {
                    in_spine = true;
                } else if in_spine
                    && start_local_name_eq(e, EpubElements::ITEMREF)
                    && let Some(idref) = get_first_attr_value(e, &[EpubElements::IDREF])
                {
                    order.push(idref);
                }
            }
            Ok(Event::End(ref e)) => {
                if end_local_name_eq(e, EpubElements::SPINE) {
                    in_spine = false;
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    order
}

/// Resolve content document path: OPF dir + href (handles relative hrefs).
fn resolve_content_path(opf_path: &str, href: &str) -> String {
    let opf_dir = opf_path.rfind('/').map_or("", |i| &opf_path[..=i]);
    if opf_dir.is_empty() {
        href.to_string()
    } else {
        format!("{opf_dir}{href}")
    }
}

/// Extract full body text from EPUB in spine order (all content documents concatenated).
fn extract_epub_body_text(
    content: &[u8],
    max_uncompressed_bytes: usize,
    epub_path: &str,
) -> Result<String> {
    let mut archive = ZipArchive::new(Cursor::new(content))
        .map_err(|e| anyhow::anyhow!("EPUB: failed to open as ZIP: {e}"))?;

    if epub_has_encryption(&mut archive) {
        debug!("EPUB: encryption.xml present, skipping body text extraction");
        return Ok(String::new());
    }

    let opf_path = read_container_rootfile(&mut archive, max_uncompressed_bytes, epub_path)
        .or_else(|| try_common_opf_paths(&mut archive));
    let opf_path = opf_path.ok_or_else(|| anyhow::anyhow!("EPUB: no package document found"))?;

    let opf_f = archive
        .by_name(&opf_path)
        .map_err(|e| anyhow::anyhow!("EPUB: cannot read OPF: {e}"))?;
    let opf_xml =
        read_zip_entry_to_string_limited(opf_f, max_uncompressed_bytes, &opf_path, epub_path)
            .map_err(|e| anyhow::anyhow!("EPUB: OPF read error: {e}"))?;

    let manifest = parse_manifest_from_opf(&opf_xml);
    let spine_order = parse_spine_order_from_opf(&opf_xml);
    if spine_order.is_empty() {
        return Ok(String::new());
    }

    let mut body_parts = Vec::with_capacity(spine_order.len());
    for idref in spine_order {
        let href = if let Some(h) = manifest.get(&idref) {
            h.as_str()
        } else {
            debug!("EPUB: spine idref '{idref}' not in manifest, skipping");
            continue;
        };
        let content_path = resolve_content_path(&opf_path, href);
        let entry = match archive.by_name(&content_path) {
            Ok(e) => e,
            Err(e) => {
                warn!("EPUB: cannot open content document '{content_path}': {e}");
                continue;
            }
        };
        let Ok(html_content) = read_zip_entry_to_string_limited(
            entry,
            max_uncompressed_bytes,
            &content_path,
            epub_path,
        ) else {
            continue;
        };
        let text = html::extract_plain_text_from_html(&html_content);
        if !text.is_empty() {
            body_parts.push(text);
        }
    }
    Ok(body_parts.join("\n\n"))
}

/// Extract templates and writing footprint from EPUB by extracting body text from all spine
/// content documents (in reading order) and running the plain-text pipeline.
///
/// # Errors
///
/// Propagates errors from [`crate::parsers::text::plain_text::extract_text_templates`] when body text was extracted successfully.
pub fn extract_epub_templates(
    content: &[u8],
    stats: &ParseResult,
    config: &RuntimeConfig,
) -> Result<MiningResult> {
    let body_text = match extract_epub_body_text(
        content,
        config.max_zip_entry_uncompressed_bytes,
        &stats.file_path,
    ) {
        Ok(t) => t,
        Err(e) => {
            warn!("EPUB body text extraction failed: {e}");
            return Ok(empty_mining_result(stats));
        }
    };
    if body_text.trim().is_empty() {
        return Ok(empty_mining_result(stats));
    }
    extract_text_templates(&body_text, stats, config)
}
