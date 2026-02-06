//! ZIP archive metadata extraction

use std::collections::BTreeMap;
use std::io::Cursor;

use crate::config::RuntimeConfig;
use crate::parsers::ParseResult;
use crate::results::metadata::{ZipEntry, ZipMetadata};
use crate::utils::filetypes::detect_file_type;
use crate::utils::path_string_helper::should_ignore_path;
use anyhow::Result;
use zip::ZipArchive;

/// Path-prefix rules not covered by `[filter]` (e.g. __MACOSX/). Basename-based
/// filtering uses `should_ignore_path` from config.
pub(crate) struct ZipOmit;

impl ZipOmit {
    /// Path prefixes: entry path starting with any of these is ignored.
    const PATH_PREFIXES: &[&str] = &["__MACOSX/"];

    /// Returns true if the entry path starts with a known junk prefix.
    pub(crate) fn should_ignore_path_prefix(path: &str) -> bool {
        Self::PATH_PREFIXES.iter().any(|p| path.starts_with(p))
    }
}

/// Extract ZIP metadata from archive bytes.
/// Excludes entries via ZipOmit path prefixes (e.g. __MACOSX/) and `[filter]` (should_ignore_path:
/// .DS_Store, Thumbs.db, desktop.ini, ehthumbs.db, *.swp, *~, ~$*, hidden files, etc.).
pub fn extract_zip_metadata(
    content: &[u8],
    stats: &ParseResult,
    config: &RuntimeConfig,
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
        if ZipOmit::should_ignore_path_prefix(&path) || should_ignore_path(&path, config) {
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
        ..Default::default()
    })
}

crate::no_template_mining!(
    extract_zip_templates,
    "ZIP archives are containers; no template mining."
);
