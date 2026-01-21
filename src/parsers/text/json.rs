//! JSON file template extraction using JSON-aware parsing

use crate::config::Config;
use crate::parsers::ParseResult;
use crate::results::MiningResult;
use crate::results::Template;
use anyhow::Result;
use dashmap::DashMap;
use serde_json::Value;
use std::collections::BTreeMap;

/// Extract templates from JSON files (JSON-aware analysis)
pub fn extract_json_templates(
    content: &str,
    stats: &ParseResult,
    config: &Config,
) -> Result<MiningResult> {
    let key_value_freq: DashMap<String, DashMap<String, usize>> = DashMap::new();
    let (parsed_objects, headers) = parse_json_content(content)?;

    if parsed_objects.is_empty() {
        return Ok(crate::parsers::traits::empty_mining_result(stats));
    }

    // First pass: collect frequencies
    for value in &parsed_objects {
        collect_json_frequencies(value, &key_value_freq, &headers);
    }

    let total_lines = parsed_objects.len();

    // Second pass: extract patterns using frequency data
    let template_groups: DashMap<String, Vec<Value>> = DashMap::new();
    for value in parsed_objects {
        let pattern = extract_json_pattern(&value, &key_value_freq, total_lines, config, &headers);
        template_groups.entry(pattern).or_default().push(value);
    }

    // Convert groups to Template structs
    let templates: Vec<Template> = template_groups
        .iter()
        .map(|entry| {
            let pattern = entry.key().clone();
            let matching_objects = entry.value();

            // Extract examples for each dynamic value
            let mut examples: BTreeMap<String, Vec<String>> = BTreeMap::new();

            for obj in matching_objects.iter().take(config.max_sample_lines) {
                extract_json_examples(obj, &mut examples, config, &headers);
            }

            Template {
                pattern,
                count: matching_objects.len(),
                examples,
            }
        })
        .collect();

    // Build MiningResult using shared utility
    Ok(crate::parsers::traits::build_mining_result(
        templates,
        total_lines,
        stats,
        config,
    ))
}

/// Check if an array contains only string values (potential header row)
fn is_header_row(arr: &[Value]) -> bool {
    arr.iter().all(|v| matches!(v, Value::String(_)))
}

/// Extract headers from an array of string values
fn extract_headers(first_arr: &[Value]) -> Vec<String> {
    first_arr
        .iter()
        .filter_map(|v| {
            if let Value::String(s) = v {
                Some(s.clone())
            } else {
                None
            }
        })
        .collect()
}

/// Parse JSON content and detect format (array of arrays, array of objects, single object, or line-by-line)
/// Returns (parsed_objects, headers) where headers is Some if first array row contains all strings
fn parse_json_content(content: &str) -> Result<(Vec<Value>, Option<Vec<String>>)> {
    // Try parsing entire content as single JSON
    if let Ok(root_value) = serde_json::from_str::<Value>(content) {
        return Ok(parse_root_value(root_value));
    }

    // Fallback: try line-by-line parsing
    let parsed_objects: Vec<Value> = content
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect();

    Ok((parsed_objects, None))
}

/// Parse a root JSON value (array or object)
fn parse_root_value(root_value: Value) -> (Vec<Value>, Option<Vec<String>>) {
    match root_value {
        Value::Array(arr) => parse_json_array(arr),
        Value::Object(_) => (vec![root_value], None),
        _ => (vec![], None),
    }
}

/// Parse a JSON array (handles header detection for array-of-arrays)
fn parse_json_array(arr: Vec<Value>) -> (Vec<Value>, Option<Vec<String>>) {
    if arr.is_empty() {
        return (vec![], None);
    }

    // Check if first item is a header row (array of strings)
    if let Value::Array(first_arr) = &arr[0]
        && is_header_row(first_arr)
    {
        let headers = Some(extract_headers(first_arr));
        let parsed_objects = arr.into_iter().skip(1).collect();
        return (parsed_objects, headers);
    }

    // All items are arrays but no header, or array of objects
    (arr, None)
}

/// Get key name for array element at index (uses headers if available)
fn get_array_key(idx: usize, headers: &Option<Vec<String>>) -> String {
    if let Some(header_vec) = headers {
        if idx < header_vec.len() {
            header_vec[idx].clone()
        } else {
            format!("col_{}", idx)
        }
    } else {
        format!("pos_{}", idx)
    }
}

/// Update frequency map for a single key-value pair
fn update_frequency(
    key: String,
    val: &Value,
    key_value_freq: &DashMap<String, DashMap<String, usize>>,
) {
    let value_str = format_json_value_for_freq(val);
    key_value_freq
        .entry(key)
        .or_default()
        .entry(value_str)
        .and_modify(|c| *c += 1)
        .or_insert(1);
}

/// Collect key-value frequencies from JSON object or array
fn collect_json_frequencies(
    value: &Value,
    key_value_freq: &DashMap<String, DashMap<String, usize>>,
    headers: &Option<Vec<String>>,
) {
    match value {
        Value::Object(map) => {
            for (key, val) in map {
                update_frequency(key.clone(), val, key_value_freq);
            }
        }
        Value::Array(arr) => {
            // Treat array as object with positional keys (or header keys if available)
            for (idx, val) in arr.iter().enumerate() {
                let key = get_array_key(idx, headers);
                update_frequency(key, val, key_value_freq);
            }
        }
        _ => {}
    }
}

/// Build a pattern part for a key-value pair
fn build_pattern_part(
    key: &str,
    val: &Value,
    key_value_freq: &DashMap<String, DashMap<String, usize>>,
    total_lines: usize,
    config: &Config,
) -> String {
    let value_str_for_freq = format_json_value_for_freq(val);
    let threshold = (total_lines as f64 * config.static_threshold) as usize;
    let is_static = key_value_freq
        .get(key)
        .and_then(|freq_map| {
            freq_map
                .get(&value_str_for_freq)
                .map(|count| *count >= threshold)
        })
        .unwrap_or(false);

    let value_str = if is_static {
        format_json_value(val)
    } else {
        "[VALUE]".to_string()
    };
    format!("\"{}\": {}", key, value_str)
}

/// Extract JSON pattern string (keys with placeholders for dynamic values)
fn extract_json_pattern(
    value: &Value,
    key_value_freq: &DashMap<String, DashMap<String, usize>>,
    total_lines: usize,
    config: &Config,
    headers: &Option<Vec<String>>,
) -> String {
    match value {
        Value::Object(map) => {
            let mut sorted_keys: Vec<_> = map.keys().collect();
            sorted_keys.sort(); // Consistent ordering

            let parts: Vec<String> = sorted_keys
                .iter()
                .map(|key| {
                    build_pattern_part(
                        key.as_str(),
                        map.get(*key).unwrap(),
                        key_value_freq,
                        total_lines,
                        config,
                    )
                })
                .collect();
            format!("{{{}}}", parts.join(", "))
        }
        Value::Array(arr) => {
            let parts: Vec<String> = arr
                .iter()
                .enumerate()
                .map(|(idx, val)| {
                    let key = get_array_key(idx, headers);
                    build_pattern_part(&key, val, key_value_freq, total_lines, config)
                })
                .collect();
            format!("[{}]", parts.join(", "))
        }
        _ => "[VALUE]".to_string(),
    }
}

/// Format JSON value for frequency tracking (actual value representation)
fn format_json_value_for_freq(value: &Value) -> String {
    match value {
        Value::String(s) => format!("\"{}\"", s),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        Value::Array(arr) => {
            if arr.is_empty() {
                "[]".to_string()
            } else {
                // For arrays, use type and length for frequency tracking
                format!("[{}:{}]", get_value_type(arr.first().unwrap()), arr.len())
            }
        }
        Value::Object(obj) => {
            if obj.is_empty() {
                "{}".to_string()
            } else {
                // For objects, use key count for frequency tracking
                format!("{{{} keys}}", obj.len())
            }
        }
    }
}

/// Get simplified type string for a value
fn get_value_type(value: &Value) -> &str {
    match value {
        Value::String(_) => "str",
        Value::Number(_) => "num",
        Value::Bool(_) => "bool",
        Value::Null => "null",
        Value::Array(_) => "arr",
        Value::Object(_) => "obj",
    }
}

/// Format JSON value for pattern (simplified)
fn format_json_value(value: &Value) -> String {
    match value {
        Value::String(s) => format!("\"{}\"", s),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        Value::Array(arr) => match arr.is_empty() {
            true => "[]".to_string(),
            false => format!("[{} items]", arr.len()),
        },
        Value::Object(obj) => match obj.is_empty() {
            true => "{}".to_string(),
            false => format!("{{{} keys}}", obj.len()),
        },
    }
}

/// Extract examples for JSON templates
fn extract_json_examples(
    value: &Value,
    examples: &mut BTreeMap<String, Vec<String>>,
    config: &Config,
    headers: &Option<Vec<String>>,
) {
    match value {
        Value::Object(map) => {
            for (key, val) in map {
                let placeholder = format!("{}_VALUE", key);
                add_example_value(val, &placeholder, examples, config);
            }
        }
        Value::Array(arr) => {
            for (idx, val) in arr.iter().enumerate() {
                let key = get_array_key(idx, headers);
                let placeholder = format!("{}_VALUE", key);
                add_example_value(val, &placeholder, examples, config);
            }
        }
        _ => {
            add_example_value(value, "VALUE", examples, config);
        }
    }
}

/// Add a value to examples if not already present and under limit
fn add_example_value(
    val: &Value,
    placeholder: &str,
    examples: &mut BTreeMap<String, Vec<String>>,
    config: &Config,
) {
    let entry = examples.entry(placeholder.to_string()).or_default();

    let value_str = match val {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        _ => format!("{:?}", val),
    };

    if !entry.contains(&value_str) && entry.len() < config.max_examples_per_placeholder {
        entry.push(value_str);
    }
}
