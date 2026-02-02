//! INI / .cfg metadata extraction

use std::collections::BTreeMap;

use crate::config::RuntimeConfig;
use crate::parsers::ParseResult;
use crate::parsers::structured::infer_value_type;
use crate::results::IniMetadata;
use crate::results::metadata::settings::ini::IniTypeInfo;
use anyhow::Result;

/// INI syntax markers/delimiters
struct IniSyntax {
    comment_semicolon: char,
    comment_hash: char,
    section_start: char,
    section_end: char,
    key_value_sep: char,
}

impl IniSyntax {
    /// Create a new instance with INI syntax markers
    const fn new() -> Self {
        Self {
            comment_semicolon: ';',
            comment_hash: '#',
            section_start: '[',
            section_end: ']',
            key_value_sep: '=',
        }
    }

    /// Check if a line is a comment
    fn is_comment(&self, trimmed: &str) -> bool {
        trimmed.starts_with(self.comment_semicolon) || trimmed.starts_with(self.comment_hash)
    }

    /// Check if a line is a section header
    fn is_section(&self, trimmed: &str) -> bool {
        trimmed.starts_with(self.section_start) && trimmed.ends_with(self.section_end)
    }
}

fn is_continuation_line(line: &str) -> bool {
    line.starts_with(' ') || line.starts_with('\t')
}

/// Extract INI metadata from file content.
/// Line-based: `[section]`, `key=value` (or `key = value`), `;` or `#` to EOL = comment.
/// Multi-line values: if `key=` has an empty value, following lines that start with whitespace
/// are concatenated until an empty line, comment, `[section]`, or a non-indented line.
/// Builds a section→key→value map, infers value types, and produces `schema` and `max_depth` (same shape as TOML/YAML).
pub fn extract_ini_metadata(
    content: &[u8],
    stats: &ParseResult,
    _config: &RuntimeConfig,
) -> Result<IniMetadata> {
    let s = std::str::from_utf8(content)
        .map_err(|e| anyhow::anyhow!("INI must be valid UTF-8: {}", e))?;
    let syntax = IniSyntax::new();

    let mut section_count = 0usize;
    let mut key_count = 0usize;
    let mut comment_count = 0usize;

    // section -> (key -> raw value). Use "" for keys before any [section].
    let mut sections: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut current = String::new();

    let mut in_multiline = false;
    let mut multiline_key = String::new();
    let mut multiline_value = String::new();

    let mut lines = s.lines();
    loop {
        let line = match lines.next() {
            None => break,
            Some(l) => l,
        };

        if in_multiline {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                sections
                    .entry(current.clone())
                    .or_default()
                    .insert(multiline_key.clone(), multiline_value.clone());
                in_multiline = false;
                continue;
            }
            if syntax.is_comment(trimmed) {
                sections
                    .entry(current.clone())
                    .or_default()
                    .insert(multiline_key.clone(), multiline_value.clone());
                in_multiline = false;
                comment_count += 1;
                continue;
            }
            if syntax.is_section(trimmed) {
                sections
                    .entry(current.clone())
                    .or_default()
                    .insert(multiline_key.clone(), multiline_value.clone());
                in_multiline = false;
                section_count += 1;
                current = trimmed[1..trimmed.len() - 1].trim().to_string();
                continue;
            }
            if !is_continuation_line(line) {
                sections
                    .entry(current.clone())
                    .or_default()
                    .insert(multiline_key.clone(), multiline_value.clone());
                in_multiline = false;
                // fall through to normal parsing with this line
            } else {
                if !multiline_value.is_empty() {
                    multiline_value.push('\n');
                }
                multiline_value.push_str(line);
                continue;
            }
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if syntax.is_comment(trimmed) {
            comment_count += 1;
            continue;
        }
        if syntax.is_section(trimmed) {
            section_count += 1;
            current = trimmed[1..trimmed.len() - 1].trim().to_string();
            continue;
        }
        if let Some(eq) = trimmed.find(syntax.key_value_sep) {
            key_count += 1;
            let key = trimmed[..eq].trim().to_string();
            let value = trimmed[eq + 1..].trim().to_string();
            if value.is_empty() {
                in_multiline = true;
                multiline_key = key;
                multiline_value = String::new();
                continue;
            }
            sections
                .entry(current.clone())
                .or_default()
                .insert(key, value);
        }
    }

    if in_multiline {
        sections
            .entry(current)
            .or_default()
            .insert(multiline_key, multiline_value);
    }

    // Build schema: section -> Table(key -> Scalar(inferred type))
    let schema: BTreeMap<String, IniTypeInfo> = sections
        .into_iter()
        .map(|(sec, kvs)| {
            let table = kvs
                .into_iter()
                .map(|(k, v)| (k, IniTypeInfo::Scalar(infer_value_type(&v))))
                .collect();
            (sec, IniTypeInfo::Table(table))
        })
        .collect();

    let max_depth = if schema.is_empty() { None } else { Some(2) };

    Ok(IniMetadata {
        file_size: Some(stats.byte_count),
        section_count: Some(section_count),
        key_count: Some(key_count),
        comment_count: Some(comment_count),
        max_depth,
        schema: if schema.is_empty() {
            None
        } else {
            Some(schema)
        },
    })
}

crate::no_template_mining!(extract_ini_templates, "INI: config; no template mining.");
