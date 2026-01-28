//! EPUB (e-book) metadata extraction

use std::io::{Cursor, Read};

use crate::engine::config::Config;
use crate::parsers::{FileType, ParseResult};
use crate::results::{EpubMetadata, MiningResult};
use anyhow::Result;
use log::warn;
use memmap2::Mmap;
use quick_xml::Reader;
use quick_xml::events::Event;
use zip::ZipArchive;

/// Extract EPUB metadata from file content.
/// EPUB is a ZIP containing META-INF/container.xml (rootfile path) and
/// the rootfile OPF (e.g. content.opf) with <metadata> (dc:title, dc:creator, dc:language, dc:identifier)
/// and <spine> (itemref count for chapters).
pub fn extract_epub_metadata(
    content: &[u8],
    stats: &ParseResult,
    _config: &Config,
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

fn read_container_rootfile(archive: &mut ZipArchive<Cursor<&[u8]>>) -> Option<String> {
    let mut f = archive.by_name("META-INF/container.xml").ok()?;
    let mut xml = String::new();
    f.read_to_string(&mut xml).ok()?;
    parse_container_rootfile(&xml)
}

fn try_common_opf_paths(archive: &mut ZipArchive<Cursor<&[u8]>>) -> Option<String> {
    for name in ["OEBPS/content.opf", "content.opf", "EPUB/package.opf"] {
        if archive.by_name(name).is_ok() {
            return Some(name.to_string());
        }
    }
    None
}

fn parse_container_rootfile(xml: &str) -> Option<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                if e.name().local_name().as_ref() == b"rootfile" {
                    if let Ok(Some(attr)) = e.try_get_attribute(b"full-path") {
                        return Some(String::from_utf8_lossy(&attr.value).into_owned());
                    }
                    if let Ok(Some(attr)) = e.try_get_attribute(b"full_path") {
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
                in_meta = local.as_slice() == b"metadata" || in_meta;
                if in_meta {
                    text.clear();
                }
            }
            Ok(Event::End(ref e)) => {
                let local = e.name().local_name().as_ref().to_vec();
                if local.as_slice() == b"metadata" {
                    in_meta = false;
                }
                if in_meta {
                    let v = text.trim();
                    if !v.is_empty() {
                        match local.as_slice() {
                            b"title" if m.title.is_none() => m.title = Some(v.to_string()),
                            b"creator" if m.author.is_none() => m.author = Some(v.to_string()),
                            b"language" if m.language.is_none() => m.language = Some(v.to_string()),
                            b"identifier" if m.identifier.is_none() => {
                                m.identifier = Some(v.to_string())
                            }
                            _ => {}
                        }
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
                if local.as_slice() == b"spine" {
                    in_spine = true;
                } else if in_spine && local.as_slice() == b"itemref" {
                    count += 1;
                }
            }
            Ok(Event::Empty(ref e)) => {
                if in_spine && e.name().local_name().as_ref() == b"itemref" {
                    count += 1;
                }
            }
            Ok(Event::End(ref e)) => {
                if e.name().local_name().as_ref() == b"spine" {
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

/// Extract metadata and templates for EPUB; single file type in this module.
pub fn process(stats: &mut ParseResult, mmap: &Mmap, config: &Config) -> Result<MiningResult> {
    crate::process_with_metadata!(
        stats,
        mmap,
        config,
        epub_metadata,
        extract_epub_metadata(mmap, stats, config),
        crate::results::EpubMetadata,
        FileType::Epub,
        extract_epub_templates(mmap, stats, config)
    )
}
