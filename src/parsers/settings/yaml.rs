//! YAML file metadata extraction

use std::collections::BTreeMap;

use crate::engine::config::Config;
use crate::parsers::ParseResult;
use crate::results::YamlMetadata;
use crate::results::metadata::settings::yaml::YamlTypeInfo;
use anyhow::Result;
use serde_yaml::Value;

/// Convert a `serde_yaml::Value` to a `YamlTypeInfo`.
fn value_to_yaml_type_info(v: &Value) -> YamlTypeInfo {
    match v {
        Value::Null => YamlTypeInfo::Scalar("null".to_string()),
        Value::Bool(_) => YamlTypeInfo::Scalar("boolean".to_string()),
        Value::Number(n) => {
            // serde_yaml::Number: prefer integer when it fits
            let s = if n.as_i64().is_some() || n.as_u64().is_some() {
                "integer"
            } else if n.as_f64().is_some() {
                "float"
            } else {
                "number"
            };
            YamlTypeInfo::Scalar(s.to_string())
        }
        Value::String(_) => YamlTypeInfo::Scalar("string".to_string()),
        Value::Sequence(seq) => {
            let element = seq
                .first()
                .map(value_to_yaml_type_info)
                .unwrap_or_else(|| YamlTypeInfo::Scalar("unknown".to_string()));
            YamlTypeInfo::Sequence(Box::new(element))
        }
        Value::Mapping(map) => {
            let m: BTreeMap<String, YamlTypeInfo> = map
                .iter()
                .filter_map(|(k, val)| {
                    k.as_str()
                        .map(|s| (s.to_string(), value_to_yaml_type_info(val)))
                })
                .collect();
            YamlTypeInfo::Mapping(m)
        }
        Value::Tagged(b) => value_to_yaml_type_info(&b.value),
    }
}

/// Recursively walk `serde_yaml::Value` to count keys, scalars, sequences, and maps.
fn walk(
    v: &Value,
    depth: usize,
    key_count: &mut usize,
    scalar_count: &mut usize,
    sequence_count: &mut usize,
    map_count: &mut usize,
    max_depth: &mut usize,
) {
    if depth > *max_depth {
        *max_depth = depth;
    }
    match v {
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
            *scalar_count += 1;
        }
        Value::Sequence(seq) => {
            *sequence_count += 1;
            for child in seq {
                walk(
                    child,
                    depth + 1,
                    key_count,
                    scalar_count,
                    sequence_count,
                    map_count,
                    max_depth,
                );
            }
        }
        Value::Mapping(map) => {
            *map_count += 1;
            *key_count += map.len();
            for (k, val) in map.iter() {
                walk(
                    k,
                    depth + 1,
                    key_count,
                    scalar_count,
                    sequence_count,
                    map_count,
                    max_depth,
                );
                walk(
                    val,
                    depth + 1,
                    key_count,
                    scalar_count,
                    sequence_count,
                    map_count,
                    max_depth,
                );
            }
        }
        Value::Tagged(b) => {
            walk(
                &b.value,
                depth,
                key_count,
                scalar_count,
                sequence_count,
                map_count,
                max_depth,
            );
        }
    }
}

/// Extract YAML metadata from document content.
pub fn extract_yaml_metadata(
    content: &[u8],
    stats: &ParseResult,
    _config: &Config,
) -> Result<YamlMetadata> {
    let value: Value =
        serde_yaml::from_slice(content).map_err(|e| anyhow::anyhow!("YAML parse error: {}", e))?;

    let mut key_count = 0;
    let mut scalar_count = 0;
    let mut sequence_count = 0;
    let mut map_count = 0;
    let mut max_depth = 0;

    walk(
        &value,
        0,
        &mut key_count,
        &mut scalar_count,
        &mut sequence_count,
        &mut map_count,
        &mut max_depth,
    );

    let schema = Some(value_to_yaml_type_info(&value));

    Ok(YamlMetadata {
        file_size: Some(stats.byte_count),
        key_count: Some(key_count),
        max_depth: Some(max_depth),
        scalar_count: Some(scalar_count),
        sequence_count: Some(sequence_count),
        map_count: Some(map_count),
        schema,
    })
}

crate::no_template_mining!(
    extract_yaml_templates,
    "YAML: metadata only; no template mining."
);
