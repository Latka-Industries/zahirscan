//! Code/script metadata: script type (from path/content via linguist + optional shebang), byte count, line count.
//! Zero-copy extras: BOM, line endings, trailing newline, max line length, blank lines, indentation (single pass).
//! No tree-sitting, AST, or function analysis.

use anyhow::Result;
use memmap2::Mmap;

use crate::engine::config::Config;
use crate::parsers::ParseResult;
use crate::parsers::traits::empty_mining_result;
use crate::results::{CodeMetadata, MiningResult};

/// Byte constants used in zero-copy scan (LF, CR, space, tab, BOM bytes).
mod bytes {
    pub const LF: u8 = b'\n';
    pub const CR: u8 = b'\r';
    pub const SPACE: u8 = b' ';
    pub const TAB: u8 = b'\t';
    // BOM: UTF-8, UTF-16 LE, UTF-16 BE
    pub const BOM_UTF8_0: u8 = 0xEF;
    pub const BOM_UTF8_1: u8 = 0xBB;
    pub const BOM_UTF8_2: u8 = 0xBF;
    pub const BOM_UTF16LE_0: u8 = 0xFF;
    pub const BOM_UTF16LE_1: u8 = 0xFE;
    pub const BOM_UTF16BE_0: u8 = 0xFE;
    pub const BOM_UTF16BE_1: u8 = 0xFF;
}

/// BOM encoding names (for zero-copy scan).
mod utf_bom {
    pub const UTF_8: &str = "UTF-8";
    pub const UTF_16LE: &str = "UTF-16LE";
    pub const UTF_16BE: &str = "UTF-16BE";
}

mod other_constants {
    pub const LF: &str = "lf";
    pub const CRLF: &str = "crlf";
    pub const CR: &str = "cr";
    pub const MIXED: &str = "mixed";
    pub const SPACE: &str = "spaces";
    pub const TAB: &str = "tabs";
    pub const SHEBANG: &str = "#!";
}

/// Zero-copy scan result from a single pass over bytes.
struct ZeroCopyScan {
    bom: Option<String>,
    line_ending: Option<String>,
    trailing_newline: Option<bool>,
    max_line_length: Option<usize>,
    blank_line_count: Option<usize>,
    indentation: Option<String>,
}

/// Update max line length, blank count, and indentation when a line ends.
fn apply_line_ending(
    line_slice: &[u8],
    current_line_len: usize,
    max_line_len: usize,
    blank_count: usize,
    indentation: &Option<String>,
) -> (usize, usize, Option<String>) {
    let max_line_len = max_line_len.max(current_line_len);
    let (blank_count, indentation) = if current_line_len == 0
        || line_slice
            .iter()
            .all(|&b| b == bytes::SPACE || b == bytes::TAB)
    {
        (blank_count + 1, indentation.clone())
    } else if let Some(lead) = leading_whitespace(line_slice) {
        let indentation = match indentation.as_ref() {
            Some(inhint) if *inhint != lead => Some(other_constants::MIXED.to_string()),
            Some(inhint) => Some(inhint.clone()),
            None => Some(lead),
        };
        (blank_count, indentation)
    } else {
        (blank_count, indentation.clone())
    };
    (max_line_len, blank_count, indentation)
}

/// Single pass over bytes: BOM, line endings, trailing newline, max line length, blank lines, indentation.
fn zero_copy_scan(bytes: &[u8]) -> ZeroCopyScan {
    let mut bom = None;
    let mut lf = 0usize;
    let mut crlf = 0usize;
    let mut cr = 0usize;
    let mut max_line_len = 0usize;
    let mut current_line_len = 0usize;
    let mut blank_count = 0usize;
    let mut indentation: Option<String> = None;
    let mut line_start = 0usize;

    // BOM from first bytes
    if bytes.len() >= 3
        && bytes[0] == bytes::BOM_UTF8_0
        && bytes[1] == bytes::BOM_UTF8_1
        && bytes[2] == bytes::BOM_UTF8_2
    {
        bom = Some(utf_bom::UTF_8.to_string());
    } else if bytes.len() >= 2
        && bytes[0] == bytes::BOM_UTF16LE_0
        && bytes[1] == bytes::BOM_UTF16LE_1
    {
        bom = Some(utf_bom::UTF_16LE.to_string());
    } else if bytes.len() >= 2
        && bytes[0] == bytes::BOM_UTF16BE_0
        && bytes[1] == bytes::BOM_UTF16BE_1
    {
        bom = Some(utf_bom::UTF_16BE.to_string());
    }

    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == bytes::CR {
            if i + 1 < bytes.len() && bytes[i + 1] == bytes::LF {
                crlf += 1;
                i += 2;
            } else {
                cr += 1;
                i += 1;
            }
            let (new_max, new_blank, new_indent) = apply_line_ending(
                &bytes[line_start..line_start + current_line_len],
                current_line_len,
                max_line_len,
                blank_count,
                &indentation,
            );
            max_line_len = new_max;
            blank_count = new_blank;
            indentation = new_indent;
            line_start = i;
            current_line_len = 0;
            continue;
        }
        if bytes[i] == bytes::LF {
            lf += 1;
            i += 1;
            let (new_max, new_blank, new_indent) = apply_line_ending(
                &bytes[line_start..line_start + current_line_len],
                current_line_len,
                max_line_len,
                blank_count,
                &indentation,
            );
            max_line_len = new_max;
            blank_count = new_blank;
            indentation = new_indent;
            line_start = i;
            current_line_len = 0;
            continue;
        }
        current_line_len += 1;
        i += 1;
    }
    // Last line (no trailing newline)
    if current_line_len > 0 {
        if current_line_len > max_line_len {
            max_line_len = current_line_len;
        }
        let line_slice = &bytes[line_start..line_start + current_line_len];
        if line_slice
            .iter()
            .all(|&b| b == bytes::SPACE || b == bytes::TAB)
        {
            blank_count += 1;
        } else if let Some(lead) = leading_whitespace(line_slice) {
            if let Some(inhint) = indentation.as_ref() {
                if *inhint != lead {
                    indentation = Some(other_constants::MIXED.to_string());
                }
            } else {
                indentation = Some(lead);
            }
        }
    }

    let line_ending = if lf + crlf + cr == 0 {
        None
    } else {
        let total = lf + crlf + cr;
        Some(if crlf == total {
            other_constants::CRLF.to_string()
        } else if lf == total {
            other_constants::LF.to_string()
        } else if cr == total {
            other_constants::CR.to_string()
        } else {
            other_constants::MIXED.to_string()
        })
    };

    let trailing_newline = if bytes.is_empty() {
        None
    } else {
        let last = *bytes.last().unwrap();
        Some(last == bytes::LF || last == bytes::CR)
    };

    ZeroCopyScan {
        bom,
        line_ending,
        trailing_newline,
        max_line_length: if max_line_len > 0 {
            Some(max_line_len)
        } else {
            None
        },
        blank_line_count: Some(blank_count),
        indentation,
    }
}

/// Leading whitespace hint: "spaces", "tabs", or "mixed". Returns None when no leading whitespace.
fn leading_whitespace(line: &[u8]) -> Option<String> {
    let mut spaces = 0usize;
    let mut tabs = 0usize;
    for &b in line {
        if b == bytes::SPACE {
            spaces += 1;
        } else if b == bytes::TAB {
            tabs += 1;
        } else {
            break;
        }
    }
    if spaces > 0 && tabs > 0 {
        Some(other_constants::MIXED.to_string())
    } else if tabs > 0 {
        Some(other_constants::TAB.to_string())
    } else if spaces > 0 {
        Some(other_constants::SPACE.to_string())
    } else {
        None
    }
}

/// Extract code metadata: script_type (from linguist + optional shebang), byte_count, line_count, and zero-copy extras.
pub fn extract_code_metadata(
    mmap: &Mmap,
    stats: &ParseResult,
    _config: &Config,
) -> Result<CodeMetadata> {
    let script_type = script_type_from_path_and_content(&stats.file_path, mmap);
    let scan = zero_copy_scan(mmap);
    Ok(CodeMetadata {
        script_type,
        byte_count: stats.byte_count,
        line_count: stats.line_count,
        bom: scan.bom,
        line_ending: scan.line_ending,
        trailing_newline: scan.trailing_newline,
        max_line_length: scan.max_line_length,
        blank_line_count: scan.blank_line_count,
        indentation: scan.indentation,
    })
}

/// Script type from path (linguist: extension + filename, then disambiguate with content) and optional shebang override.
fn script_type_from_path_and_content(path: &str, mmap: &Mmap) -> String {
    // Optional shebang override: if first line is #!..., use interpreter hint first
    if let Ok(s) = std::str::from_utf8(mmap) {
        let first_line = s.lines().next().unwrap_or("").trim();
        if first_line.starts_with(other_constants::SHEBANG)
            && let Some(override_type) = script_type_from_shebang(first_line)
        {
            return override_type;
        }
    }

    let langs = linguist::detect_language_by_extension(path)
        .ok()
        .unwrap_or_default();
    let langs = if langs.is_empty() {
        linguist::detect_language_by_filename(path)
            .ok()
            .unwrap_or_default()
    } else {
        langs
    };
    if langs.is_empty() {
        return "unknown".to_string();
    }
    if langs.len() == 1 {
        return langs[0].name.to_lowercase();
    }
    let content = std::str::from_utf8(mmap).unwrap_or("");
    linguist::disambiguate(path, content)
        .ok()
        .and_then(|v| v.into_iter().next())
        .map(|d| d.name.to_lowercase())
        .unwrap_or_else(|| langs[0].name.to_lowercase())
}

/// Map shebang line to script_type (e.g. "#!/usr/bin/env python3" -> "python").
fn script_type_from_shebang(shebang: &str) -> Option<String> {
    let rest = shebang.strip_prefix(other_constants::SHEBANG)?.trim();
    let interpreter = rest
        .split_whitespace()
        .last()
        .or_else(|| rest.rsplit('/').next())?;
    let base = interpreter
        .split('.')
        .next()
        .unwrap_or(interpreter)
        .to_lowercase();
    let normalized = match base.as_str() {
        "python" | "python2" | "python3" => "python",
        "node" | "nodejs" => "javascript",
        "bash" | "sh" | "zsh" | "ksh" => "shell",
        "rb" | "ruby" => "ruby",
        "perl" => "perl",
        "php" => "php",
        "lua" => "lua",
        "r" => "r",
        "runhaskell" | "runghc" => "haskell",
        _ if !base.is_empty() => base.as_str(),
        _ => return None,
    };
    Some(normalized.to_string())
}

/// Code files: metadata only, no template mining.
pub fn extract_code_templates(
    _content: &[u8],
    stats: &ParseResult,
    _config: &Config,
) -> Result<MiningResult> {
    Ok(empty_mining_result(stats))
}
