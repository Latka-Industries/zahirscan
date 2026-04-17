//! `NumPy` `.npz` (ZIP of `.npy` members) — list entries and parse each `.npy` header + column stats.

use std::io::{self, Cursor, Read};

use anyhow::{Context, Result};
use memmap2::Mmap;
use zip::ZipArchive;

use crate::config::RuntimeConfig;
use crate::parsers::ParseResult;
use crate::results::{ColumnarCommonFields, NpyLayoutSummary, NpzMetadata, NpzNpyEntrySummary};

use super::npy::parse_npy_prefix;
use super::sample::{column_common_from_npy_bytes, zip_member_target_read_len};

const MAX_NPZ_NPY_ENTRIES: usize = 128;
/// Upper bound on uncompressed bytes read per inner `.npy` (header + sample prefix for stats).
const MAX_NPZ_ENTRY_READ: usize = 64 * 1024 * 1024;
const INITIAL_NPZ_READ: usize = 512 * 1024;

/// Read enough of a ZIP member to parse the header and (when applicable) the contiguous sample prefix.
fn read_npy_zip_member<R: Read + ?Sized>(
    zf: &mut zip::read::ZipFile<'_, R>,
    uncompressed_size: usize,
    config: &RuntimeConfig,
) -> Result<Vec<u8>> {
    let max_read = uncompressed_size.min(MAX_NPZ_ENTRY_READ);
    let mut buf = Vec::new();
    io::copy(
        &mut zf.take(INITIAL_NPZ_READ.min(max_read) as u64),
        &mut buf,
    )
    .context("read npz .npy initial chunk")?;

    let logical_len = uncompressed_size;
    let mut layout = parse_npy_prefix(&buf, logical_len);
    if layout.is_err() && buf.len() < max_read {
        io::copy(&mut zf.take((max_read - buf.len()) as u64), &mut buf)
            .context("read npz .npy remainder for header")?;
        layout = parse_npy_prefix(&buf, logical_len);
    }
    let layout = layout.context("parse .npy header inside .npz")?;

    let need = zip_member_target_read_len(&layout, uncompressed_size, config)
        .min(max_read)
        .max(buf.len());

    if buf.len() < need {
        io::copy(&mut zf.take((need - buf.len()) as u64), &mut buf)
            .context("read npz .npy sample")?;
    }

    Ok(buf)
}

/// Extract `.npz` metadata: ZIP size and per-`.npy` header summaries + column stats when possible.
///
/// # Errors
///
/// Returns an error if the bytes are not a readable ZIP archive.
pub fn extract_npz_metadata(
    mmap: &Mmap,
    stats: &ParseResult,
    config: &RuntimeConfig,
) -> Result<NpzMetadata> {
    let cursor = Cursor::new(mmap.as_ref());
    let mut archive = ZipArchive::new(cursor).context("open NPZ as ZIP")?;
    let zip_entry_count = archive.len();

    let mut npy_entries = Vec::new();
    let mut npy_entries_scanned = 0usize;

    for i in 0..archive.len() {
        if npy_entries.len() >= MAX_NPZ_NPY_ENTRIES {
            break;
        }

        let Ok(mut zf) = archive.by_index(i) else {
            continue;
        };

        let name = zf.name().to_string();
        if !name.to_ascii_lowercase().ends_with(".npy") {
            continue;
        }

        npy_entries_scanned = npy_entries_scanned.saturating_add(1);
        let uncompressed_size = zf.size() as usize;
        let uncompressed_u64 = zf.size();

        let buf = match read_npy_zip_member(&mut zf, uncompressed_size, config) {
            Ok(b) => b,
            Err(e) => {
                npy_entries.push(NpzNpyEntrySummary {
                    name,
                    uncompressed_size: Some(uncompressed_u64),
                    layout: NpyLayoutSummary::default(),
                    common: ColumnarCommonFields::default(),
                    entry_parse_error: Some(format!("{e:#}")),
                });
                continue;
            }
        };

        match parse_npy_prefix(&buf, uncompressed_size) {
            Ok(layout) => {
                let common = column_common_from_npy_bytes(&buf, &layout, config);
                npy_entries.push(NpzNpyEntrySummary {
                    name,
                    uncompressed_size: Some(uncompressed_u64),
                    layout,
                    common,
                    entry_parse_error: None,
                });
            }
            Err(e) => {
                npy_entries.push(NpzNpyEntrySummary {
                    name,
                    uncompressed_size: Some(uncompressed_u64),
                    layout: NpyLayoutSummary::default(),
                    common: ColumnarCommonFields::default(),
                    entry_parse_error: Some(format!("{e:#}")),
                });
            }
        }
    }

    Ok(NpzMetadata {
        byte_count: stats.byte_count,
        zip_entry_count: Some(zip_entry_count),
        npy_entries_scanned: Some(npy_entries_scanned),
        npy_entries: Some(npy_entries),
    })
}

crate::no_template_mining!(
    extract_npz_templates,
    "`NumPy` `.npz`: ZIP listing, per-`.npy` layout, and bounded column stats (same pipeline as `.npy`); no template mining."
);
