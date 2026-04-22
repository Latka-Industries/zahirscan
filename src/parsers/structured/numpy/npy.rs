//! `NumPy` `.npy` header parsing (magic, version, header dict, data offset).

use anyhow::{Context, Result, bail};
use memmap2::Mmap;
use regex::Regex;
use std::sync::LazyLock;

use crate::config::RuntimeConfig;
use crate::parsers::ParseResult;
use crate::parsers::structured::tensor3d::tensor3d_plane_stats_for_npy_bytes;
use crate::results::{ArrayLayoutSummary, NpyMetadata};

const NPY_MAGIC: &[u8; 6] = b"\x93NUMPY";
const MAX_HEADER_REGION: usize = 512 * 1024;

static RE_DESCR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"'descr'\s*:\s*'((?:[^'\\]|\\.)*)'").expect("descr regex"));
static RE_SHAPE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"'shape'\s*:\s*\(([^)]*)\)").expect("shape regex"));
static RE_FORTRAN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"'fortran_order'\s*:\s*(True|False)").expect("fortran_order regex")
});
static RE_U: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[<>=|]?U(\d+)$").expect("U regex"));
static RE_S: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[<>=|]?S(\d+)$").expect("S regex"));
static RE_V: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[<>=|]?V(\d+)$").expect("V regex"));
static RE_STD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[<>=|]?[a-zA-Z]+\d+$").expect("std dtype regex"));

/// Best-effort element size for simple `descr` strings (e.g. `<i4`, `>f8`, `|S10`, `U20`).
#[must_use]
pub fn numpy_descr_element_nbytes(descr: &str) -> Option<usize> {
    let t = descr.trim();
    if t.is_empty() || t.starts_with('[') || t.starts_with('{') {
        return None;
    }

    if let Some(c) = RE_U.captures(t) {
        let n: usize = c.get(1)?.as_str().parse().ok()?;
        return n.checked_mul(4);
    }
    if let Some(c) = RE_S.captures(t) {
        return c.get(1)?.as_str().parse().ok();
    }
    if let Some(c) = RE_V.captures(t) {
        return c.get(1)?.as_str().parse().ok();
    }
    if RE_STD.is_match(t) {
        let digits: String = t
            .chars()
            .rev()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        return digits.parse().ok();
    }
    None
}

fn latin1_to_string(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

fn parse_shape_tuple(inner: &str) -> Option<Vec<usize>> {
    let inner = inner.trim();
    if inner.is_empty() {
        return Some(Vec::new());
    }
    let mut out = Vec::new();
    for part in inner.split(',') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        out.push(p.parse().ok()?);
    }
    Some(out)
}

fn shape_num_elements(shape: &[usize]) -> usize {
    shape.iter().product()
}

/// Parse the `.npy` header from a prefix of bytes; `logical_len` is the full on-disk size of this array (file or zip entry).
///
/// # Errors
///
/// Returns [`anyhow::Error`] when the buffer is too short, magic is wrong, the version is not
/// supported (only 1.0 and 2.0), the declared header length exceeds [`MAX_HEADER_REGION`], the
/// prefix does not contain the full header dict, or the header cannot be read as Latin-1 (same
/// slice bounds as the length checks above).
pub fn parse_npy_prefix(bytes: &[u8], logical_len: usize) -> Result<ArrayLayoutSummary> {
    if bytes.len() < 10 {
        bail!("NPY too short for header prefix");
    }
    if &bytes[..6] != NPY_MAGIC {
        bail!("missing \\x93NUMPY magic");
    }
    let major = bytes[6];
    let minor = bytes[7];
    let (header_region, data_offset) = match (major, minor) {
        (1, 0) => {
            let hlen = u16::from_le_bytes([bytes[8], bytes[9]]) as usize;
            if hlen > MAX_HEADER_REGION {
                bail!("NPY v1 header length {hlen} exceeds cap");
            }
            if bytes.len() < 10 + hlen {
                bail!(
                    "NPY v1 truncated: need {} header bytes, have {}",
                    hlen,
                    bytes.len() - 10
                );
            }
            (hlen, 10 + hlen)
        }
        (2, 0) => {
            if bytes.len() < 12 {
                bail!("NPY v2 too short for header length");
            }
            let hlen = u32::from_le_bytes([
                bytes[8], bytes[9], bytes[10], bytes[11],
            ]) as usize;
            if hlen > MAX_HEADER_REGION {
                bail!("NPY v2 header length {hlen} exceeds cap");
            }
            if bytes.len() < 12 + hlen {
                bail!(
                    "NPY v2 truncated: need {} header bytes, have {}",
                    hlen,
                    bytes.len() - 12
                );
            }
            (hlen, 12 + hlen)
        }
        _ => bail!("unsupported NPY version {major}.{minor}"),
    };

    let header_start = data_offset - header_region;
    let header_bytes = &bytes[header_start..data_offset];
    let header_str = latin1_to_string(header_bytes);

    let descr = RE_DESCR
        .captures(&header_str)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().replace("\\'", "'"));

    let shape = RE_SHAPE
        .captures(&header_str)
        .and_then(|c| c.get(1))
        .and_then(|m| parse_shape_tuple(m.as_str()));

    let fortran_order = RE_FORTRAN
        .captures(&header_str)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str() == "True");

    let format_version = match (major, minor) {
        (1, 0) => Some("1.0".to_string()),
        (2, 0) => Some("2.0".to_string()),
        _ => None,
    };

    let data_region_bytes = logical_len.saturating_sub(data_offset);

    let expected_data_bytes_from_dtype = match (&descr, &shape) {
        (Some(d), Some(sh)) => {
            numpy_descr_element_nbytes(d).and_then(|el| el.checked_mul(shape_num_elements(sh)))
        }
        _ => None,
    };

    Ok(ArrayLayoutSummary {
        format_version,
        dtype: descr,
        shape,
        fortran_order,
        header_region_bytes: Some(header_region),
        data_offset: Some(data_offset),
        data_region_bytes: Some(data_region_bytes),
        expected_data_bytes_from_dtype,
    })
}

/// Extract `NumPy` `.npy` layout metadata and CSV-like column stats (bounded sample; mmap-backed).
///
/// # Errors
///
/// Returns an error if magic/version/header cannot be read or parsed within caps.
pub fn extract_npy_metadata(
    mmap: &Mmap,
    stats: &ParseResult,
    config: &RuntimeConfig,
) -> Result<NpyMetadata> {
    let bytes: &[u8] = mmap.as_ref();
    let layout = parse_npy_prefix(bytes, stats.byte_count).context("parse NPY")?;
    let common = super::sample::column_common_from_npy_bytes(bytes, &layout, config);
    let tensor3d = tensor3d_plane_stats_for_npy_bytes(bytes, &layout);
    Ok(NpyMetadata {
        byte_count: stats.byte_count,
        layout,
        common,
        tensor3d,
    })
}

crate::no_template_mining!(
    extract_npy_templates,
    "`NumPy` `.npy` is handled for header/layout metadata only; no template mining."
);
