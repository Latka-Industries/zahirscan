//! ZIP archive metadata extraction

use std::collections::BTreeMap;
use std::io::Cursor;

use crate::config::Config;
use crate::parsers::ParseResult;
use crate::results::metadata::{ZipEntry, ZipMetadata};
use crate::tools::detect_file_type;
use anyhow::Result;
use zip::ZipArchive;

/// OS hidden/junk paths to omit from ZIP entries and totals.
/// Add path prefixes, filename prefixes, or exact filenames here; matching is case-insensitive for exact filenames.
pub(crate) struct ZipOmit;

impl ZipOmit {
    /// Path prefixes: entry path starting with any of these is ignored.
    const PATH_PREFIXES: &[&str] = &["__MACOSX/"];

    /// Filename prefixes: basename starting with any of these is ignored.
    const FILENAME_PREFIXES: &[&str] = &["._", "~$"];

    /// Exact filenames (case-insensitive): e.g. .DS_Store, Thumbs.db, Desktop.ini.
    const FILENAMES: &[&str] = &[".DS_Store", "Thumbs.db", "Desktop.ini", "ehthumbs.db"];

    /// Returns true if `path` should be excluded from the ZIP listing.
    pub(crate) fn should_ignore(path: &str) -> bool {
        for p in Self::PATH_PREFIXES {
            if path.starts_with(p) {
                return true;
            }
        }
        let name = path.trim_end_matches('/').rsplit('/').next().unwrap_or("");
        for p in Self::FILENAME_PREFIXES {
            if name.starts_with(p) {
                return true;
            }
        }
        for f in Self::FILENAMES {
            if name.eq_ignore_ascii_case(f) {
                return true;
            }
        }
        false
    }
}

/// Extract ZIP metadata from archive bytes.
/// Excludes OS hidden/junk entries (e.g. __MACOSX/, ._*, .DS_Store, Thumbs.db, Desktop.ini, ~$*) from the entries list and totals.
pub fn extract_zip_metadata(
    content: &[u8],
    stats: &ParseResult,
    _config: &Config,
) -> Result<ZipMetadata> {
    let mut archive = ZipArchive::new(Cursor::new(content))
        .map_err(|e| anyhow::anyhow!("ZIP parse error: {}", e))?;

    let mut entries = Vec::new();
    let mut total_uncompressed: u64 = 0;
    let mut total_compressed: u64 = 0;
    let mut entry_type_counts: BTreeMap<String, usize> = BTreeMap::new();

    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| anyhow::anyhow!("ZIP entry {}: {}", i, e))?;
        let path = entry.name().to_string();
        if ZipOmit::should_ignore(&path) {
            continue;
        }
        if !entry.is_file() {
            continue; // omit directory-only entries (e.g. "pdfs/")
        }
        let size = entry.size();
        let comp = entry.compressed_size();
        total_uncompressed = total_uncompressed.saturating_add(size);
        total_compressed = total_compressed.saturating_add(comp);

        let detected_type = detect_file_type(entry.name())
            .as_metadata_name()
            .to_string();
        *entry_type_counts.entry(detected_type.clone()).or_insert(0) += 1;

        let modified = entry.last_modified().map(|dt| format!("{}", dt));
        let compression_method = format!("{:?}", entry.compression());

        entries.push(ZipEntry {
            path,
            uncompressed_size: Some(size),
            compressed_size: Some(comp),
            detected_type: Some(detected_type),
            modified,
            compression_method: Some(compression_method),
        });
    }

    // Archive comment would require reading the End of Central Directory;
    // zip crate's ZipArchive doesn't expose it easily. Leave as None.
    Ok(ZipMetadata {
        file_size: Some(stats.byte_count),
        file_count: Some(entries.len()),
        entries: Some(entries),
        total_uncompressed: Some(total_uncompressed),
        total_compressed: Some(total_compressed),
        entry_type_counts: Some(entry_type_counts),
        comment: None,
    })
}

crate::no_template_mining!(
    extract_zip_templates,
    "ZIP archives are containers; no template mining."
);
