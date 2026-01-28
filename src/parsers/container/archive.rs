//! TAR and compressed TAR (tar.gz, tgz, tar.bz2, tar.xz) metadata extraction.
//!
//! **Zero-copy / no decompression:** Plain .tar uses header seek only. Compressed
//! TAR (.tar.gz, .tar.xz, .tar.bz2) never runs a decompressor: we read only the
//! gzip trailer (last 4 bytes) for .tar.gz uncompressed size; entries and
//! file_count are not available for compressed TAR. Use plain .tar for full listing.

use std::io::{Cursor, Read, Seek, SeekFrom};

use crate::engine::chunking::optimal_chunk_size;
use crate::engine::config::Config;
use crate::parsers::ParseResult;
use crate::results::metadata::{ArchiveEntry, ArchiveMetadata};
use anyhow::Result;
use rayon::prelude::*;
use rayon::slice::ParallelSlice;

/// TAR archive format: plain or compressed. Used for detection and to choose the right decoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TarFormat {
    Tar,
    TarGz,
    Tgz,
    TarBz2,
    TarXz,
}

impl TarFormat {
    fn as_str(self) -> &'static str {
        match self {
            TarFormat::Tar => "tar",
            TarFormat::TarGz => "tar.gz",
            TarFormat::Tgz => "tgz",
            TarFormat::TarBz2 => "tar.bz2",
            TarFormat::TarXz => "tar.xz",
        }
    }
}

/// Returns true if the TAR entry path should be omitted from the listing (metadata/overhead or directory).
/// Omits: AppleDouble/resource fork (._*), PAX extended header (PaxHeader), directory entries (path ends with /).
fn should_omit_tar_entry_path(path: &str) -> bool {
    path.contains("/._")
        || path.starts_with("._")
        || path.contains("PaxHeader")
        || path.ends_with('/')
}

/// Detect archive format from file path (lowercased). Check compound extensions first.
fn detect_archive_format(path: &str) -> TarFormat {
    let lo = path.to_lowercase();
    match () {
        _ if lo.ends_with(".tar.xz") => TarFormat::TarXz,
        _ if lo.ends_with(".tar.bz2") => TarFormat::TarBz2,
        _ if lo.ends_with(".tgz") => TarFormat::Tgz,
        _ if lo.ends_with(".tar.gz") => TarFormat::TarGz,
        _ if lo.ends_with(".tar") => TarFormat::Tar,
        _ => TarFormat::Tar,
    }
}

/// Extract TAR (or compressed TAR) metadata. Zero-copy: no decompression.
/// Plain .tar: full listing (file_count, entries) via header seek. Compressed
/// .tar.gz/.tar.xz/.tar.bz2: compressed_size always; .tar.gz also gets
/// uncompressed_size from gzip trailer (last 4 bytes). Entries and file_count
/// are None for compressed TAR.
pub fn extract_archive_metadata(
    content: &[u8],
    stats: &ParseResult,
    config: &Config,
) -> Result<ArchiveMetadata> {
    let format = detect_archive_format(&stats.file_path);
    let compressed_size = stats.byte_count as u64;

    let (file_count, uncompressed_size, entries) = stream_tar_entries(content, format, config)?;

    let note = match format {
        TarFormat::Tar => None,
        TarFormat::TarGz | TarFormat::Tgz | TarFormat::TarBz2 | TarFormat::TarXz => {
            Some("full listing for .tar only.".to_string())
        }
    };

    Ok(ArchiveMetadata {
        format: Some(format.as_str().to_string()),
        file_count,
        entries: if entries.is_empty() {
            None
        } else {
            Some(entries)
        },
        compressed_size: Some(compressed_size),
        uncompressed_size: if uncompressed_size > 0 {
            Some(uncompressed_size)
        } else {
            None
        },
        truncated: None,
        note,
    })
}

fn stream_tar_entries(
    content: &[u8],
    format: TarFormat,
    config: &Config,
) -> Result<(Option<usize>, u64, Vec<ArchiveEntry>)> {
    match format {
        TarFormat::Tar => {
            let (fc, u, e) = stream_tar_entries_seek(content, config)?;
            Ok((Some(fc), u, e))
        }
        _ => compressed_tar_metadata_no_decompress(content, format),
    }
}

/// Compressed .tar.gz / .tar.xz / .tar.bz2: zero-copy. No decompression.
/// We only read the gzip trailer (last 4 bytes) for .tar.gz uncompressed size.
/// Entries and file_count are not available (would require decompression).
fn compressed_tar_metadata_no_decompress(
    content: &[u8],
    format: TarFormat,
) -> Result<(Option<usize>, u64, Vec<ArchiveEntry>)> {
    let uncompressed = match format {
        TarFormat::TarGz | TarFormat::Tgz => {
            // Gzip trailer: last 8 bytes = CRC32 (4) + ISIZE (4, LE). ISIZE = uncompressed size mod 2^32.
            if content.len() >= 8 {
                let i = content.len() - 4;
                u32::from_le_bytes([content[i], content[i + 1], content[i + 2], content[i + 3]])
                    as u64
            } else {
                0
            }
        }
        TarFormat::TarBz2 | TarFormat::TarXz => 0,
        TarFormat::Tar => unreachable!("Tar uses seek path"),
    };
    Ok((None, uncompressed, Vec::new()))
}

/// Parse 12-byte octal size at TAR header offset 124.
fn parse_tar_size(bytes: &[u8]) -> u64 {
    if bytes.len() < 136 {
        return 0;
    }
    let s = match std::str::from_utf8(&bytes[124..136]) {
        Ok(x) => x.trim_end_matches('\0').trim(),
        Err(_) => return 0,
    };
    u64::from_str_radix(s, 8).unwrap_or(0)
}

/// Path from TAR header: name (0..100) and optional USTAR prefix (345..500).
fn parse_tar_path(bytes: &[u8]) -> String {
    let trim_nul = |b: &[u8]| -> String {
        let end = b.iter().position(|&x| x == 0).unwrap_or(b.len());
        std::str::from_utf8(&b[..end])
            .unwrap_or("")
            .trim_end()
            .to_string()
    };
    let name = trim_nul(&bytes[0..100.min(bytes.len())]);
    if bytes.len() >= 500 {
        let prefix = trim_nul(&bytes[345..500]);
        if !prefix.is_empty() {
            return format!("{}/{}", prefix, name);
        }
    }
    name
}

/// Plain .tar: read only 512‑byte headers and seek over bodies. Zero read of file contents.
/// Uses a two-pass strategy when adaptive chunking is beneficial: first pass collects
/// (offset, size, longname) for each entry; second pass resolves paths in parallel over
/// header boundaries.
fn stream_tar_entries_seek(
    content: &[u8],
    config: &Config,
) -> Result<(usize, u64, Vec<ArchiveEntry>)> {
    let mut r = Cursor::new(content);
    let mut _raw_count = 0usize;
    let mut _raw_uncompressed = 0u64;
    let mut info: Vec<(u64, u64, Option<String>)> = Vec::new(); // (offset, size, longname)
    let mut maybe_long_name: Option<String> = None;

    // First pass: collect (offset, size, longname) for normal entries; seek over all bodies
    loop {
        let offset = r.stream_position().unwrap_or(0);
        let mut buf = [0u8; 512];
        if r.read_exact(&mut buf).is_err() {
            break;
        }
        if buf.iter().all(|&b| b == 0) {
            break;
        }

        let size = parse_tar_size(&buf);
        let typeflag = buf.get(156).copied().unwrap_or(0);
        let block_len = size.div_ceil(512) * 512;

        _raw_count += 1;
        _raw_uncompressed = _raw_uncompressed.saturating_add(size);

        if typeflag == b'L' {
            if size > 0 && size <= 4096 {
                let mut name_buf = vec![0u8; size as usize];
                if r.read_exact(&mut name_buf).is_ok() {
                    maybe_long_name = Some(String::from_utf8_lossy(&name_buf).into_owned());
                }
            }
            let skip = block_len.saturating_sub(size);
            if skip > 0 {
                let _ = r.seek(SeekFrom::Current(skip as i64));
            }
        } else if typeflag == b'K' {
            let _ = r.seek(SeekFrom::Current(block_len as i64));
        } else {
            info.push((offset, size, maybe_long_name.take()));
            let _ = r.seek(SeekFrom::Current(block_len as i64));
        }
    }

    // Second pass: resolve paths (from longname or header)
    let to_resolve = &info[..];
    let mut entries = if to_resolve.is_empty() {
        Vec::new()
    } else if to_resolve.len() >= config.min_collection_size_for_chunking
        && config.target_chunks_per_file > 1
    {
        // Adaptive chunking: parallel over header boundaries
        let chunk_size = optimal_chunk_size(
            to_resolve.len(),
            config.target_chunks_per_file,
            config.min_collection_size_for_chunking,
        );
        to_resolve
            .par_chunks(chunk_size)
            .map(|chunk| {
                let mut c = Cursor::new(content);
                chunk
                    .iter()
                    .map(|(o, s, ln)| {
                        let path = ln.clone().unwrap_or_else(|| {
                            let mut buf = [0u8; 512];
                            let _ = c.seek(SeekFrom::Start(*o));
                            let _ = c.read_exact(&mut buf);
                            parse_tar_path(&buf)
                        });
                        ArchiveEntry { path, size: *s }
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
            .into_iter()
            .flatten()
            .collect()
    } else {
        let mut c = Cursor::new(content);
        to_resolve
            .iter()
            .map(|(o, s, ln)| {
                let path = ln.clone().unwrap_or_else(|| {
                    let mut buf = [0u8; 512];
                    let _ = c.seek(SeekFrom::Start(*o));
                    let _ = c.read_exact(&mut buf);
                    parse_tar_path(&buf)
                });
                ArchiveEntry { path, size: *s }
            })
            .collect()
    };

    entries.retain(|e| !should_omit_tar_entry_path(&e.path));
    let file_count = entries.len();
    let uncompressed_size: u64 = entries.iter().map(|e| e.size).sum();

    Ok((file_count, uncompressed_size, entries))
}

crate::no_template_mining!(
    extract_archive_templates,
    "Archive: container; no template mining."
);
