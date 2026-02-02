//! TOML file metadata extraction

use crate::config::RuntimeConfig;
use crate::parsers::ParseResult;
use crate::results::TomlMetadata;
use crate::results::metadata::settings::toml::TomlTypeInfo;
use anyhow::Result;
use toml::Value;

fn value_to_type_info(v: &Value) -> TomlTypeInfo {
    match v {
        Value::String(_) => TomlTypeInfo::Scalar("string".to_string()),
        Value::Integer(_) => TomlTypeInfo::Scalar("integer".to_string()),
        Value::Float(_) => TomlTypeInfo::Scalar("float".to_string()),
        Value::Boolean(_) => TomlTypeInfo::Scalar("boolean".to_string()),
        Value::Datetime(_) => TomlTypeInfo::Scalar("datetime".to_string()),
        Value::Array(arr) => {
            let element = arr
                .first()
                .map(value_to_type_info)
                .unwrap_or_else(|| TomlTypeInfo::Scalar("unknown".to_string()));
            TomlTypeInfo::Array(Box::new(element))
        }
        Value::Table(t) => TomlTypeInfo::Table(
            t.iter()
                .map(|(k, v)| (k.clone(), value_to_type_info(v)))
                .collect(),
        ),
    }
}

/// Walk `toml::Value` to count tables (sections), total keys, and max depth.
fn count_tables(v: &Value) -> usize {
    match v {
        Value::Table(t) => 1 + t.values().map(count_tables).sum::<usize>(),
        _ => 0,
    }
}

/// Walk `toml::Value` to count total keys across all tables.
fn count_keys(v: &Value) -> usize {
    match v {
        Value::Table(t) => t.len() + t.values().map(count_keys).sum::<usize>(),
        _ => 0,
    }
}

/// Recursively walk `toml::Value` to find maximum nesting depth.
fn max_depth_rec(d: usize, v: &Value) -> usize {
    match v {
        Value::Table(t) => t
            .values()
            .map(|c| max_depth_rec(d + 1, c))
            .max()
            .unwrap_or(d),
        _ => d,
    }
}

/// Extract TOML metadata from file content.
pub fn extract_toml_metadata(
    content: &[u8],
    stats: &ParseResult,
    _config: &RuntimeConfig,
) -> Result<TomlMetadata> {
    let s = std::str::from_utf8(content)
        .map_err(|e| anyhow::anyhow!("TOML must be valid UTF-8: {}", e))?;
    let value: Value = toml::from_str(s).map_err(|e| anyhow::anyhow!("TOML parse error: {}", e))?;

    let section_count = Some(count_tables(&value));
    let key_count = Some(count_keys(&value));
    let max_depth = Some(max_depth_rec(0, &value));
    let schema = value.as_table().map(|t| {
        t.iter()
            .map(|(k, v)| (k.clone(), value_to_type_info(v)))
            .collect()
    });

    Ok(TomlMetadata {
        file_size: Some(stats.byte_count),
        section_count,
        key_count,
        max_depth,
        schema,
    })
}

crate::no_template_mining!(
    extract_toml_templates,
    "TOML is config; schema covers structure. No template mining."
);
