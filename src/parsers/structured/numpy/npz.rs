//! `NumPy` `.npz` (ZIP of `.npy` members) — list entries and parse each `.npy` header + column stats.
//!
//! ZIP member bodies are read sequentially (one [`ZipArchive`]), then parse + stats run in parallel.

use std::io::{self, Cursor, Read};

use anyhow::{Context, Result};
use memmap2::Mmap;
use rayon::prelude::*;
use zip::ZipArchive;

use crate::config::RuntimeConfig;
use crate::parsers::{ParseResult, structured::tensor3d::tensor3d_plane_stats_for_npy_bytes};
use crate::results::{ArrayLayoutSummary, ColumnarCommonFields, NpzMetadata, NpzNpyEntrySummary};

use super::npy::parse_npy_prefix;
use super::sample::{column_common_from_npy_bytes, zip_member_target_read_len};

const MAX_NPZ_NPY_ENTRIES: usize = 128;
/// Upper bound on uncompressed bytes read per inner `.npy` (header + sample prefix for stats).
///
/// Slightly above 64 MiB on purpose: a member whose total uncompressed size is **64 MiB + header**
/// (e.g. 128-byte header + exactly 64 MiB of array data = 67,108,992 bytes) would otherwise be cut
/// at 64 MiB and lose the tail of the payload—dropping the last row of column stats.
const MAX_NPZ_ENTRY_READ: usize = 64 * 1024 * 1024 + 256 * 1024;
const INITIAL_NPZ_READ: usize = 512 * 1024;

/// One `.npy` member read from the archive (serial phase); parse/stats use this in parallel.
struct NpzEntryRead {
    name: String,
    uncompressed_size: u64,
    bytes: Result<Vec<u8>, String>,
}

fn npy_entry_from_read(entry: &NpzEntryRead, config: &RuntimeConfig) -> NpzNpyEntrySummary {
    let name = entry.name.clone();
    let uncompressed_u64 = entry.uncompressed_size;
    let uncompressed_size = entry.uncompressed_size as usize;

    let buf = match &entry.bytes {
        Err(e) => {
            return NpzNpyEntrySummary {
                name,
                uncompressed_size: Some(uncompressed_u64),
                layout: ArrayLayoutSummary::default(),
                common: ColumnarCommonFields::default(),
                tensor3d: None,
                entry_parse_error: Some(e.clone()),
            };
        }
        Ok(b) => b,
    };

    match parse_npy_prefix(buf, uncompressed_size) {
        Ok(layout) => {
            let common = column_common_from_npy_bytes(buf, &layout, config);
            let tensor3d = tensor3d_plane_stats_for_npy_bytes(buf, &layout);
            NpzNpyEntrySummary {
                name,
                uncompressed_size: Some(uncompressed_u64),
                layout,
                common,
                tensor3d,
                entry_parse_error: None,
            }
        }
        Err(e) => NpzNpyEntrySummary {
            name,
            uncompressed_size: Some(uncompressed_u64),
            layout: ArrayLayoutSummary::default(),
            common: ColumnarCommonFields::default(),
            tensor3d: None,
            entry_parse_error: Some(format!("{e:#}")),
        },
    }
}

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

    let mut raw_entries: Vec<NpzEntryRead> = Vec::new();
    let mut npy_entries_scanned = 0usize;

    for i in 0..archive.len() {
        if raw_entries.len() >= MAX_NPZ_NPY_ENTRIES {
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

        let bytes =
            read_npy_zip_member(&mut zf, uncompressed_size, config).map_err(|e| format!("{e:#}"));
        raw_entries.push(NpzEntryRead {
            name,
            uncompressed_size: uncompressed_u64,
            bytes,
        });
    }

    let npy_entries: Vec<NpzNpyEntrySummary> = raw_entries
        .par_iter()
        .map(|e| npy_entry_from_read(e, config))
        .collect();

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
