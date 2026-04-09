//! JSON file template extraction using JSON-aware parsing

use anyhow::Result;
use dashmap::DashMap;
use rayon::prelude::*;
use serde_json::Value;
use std::collections::BTreeMap;

use crate::config::RuntimeConfig;
use crate::parsers::ParseResult;
use crate::parsers::traits::{AdaptiveParallel, build_mining_result, empty_mining_result};
use crate::results::{JsonMetadata, MiningResult, Template};
use crate::utils::path_string_helper::{PlaceholderType, format_placeholder_typed};

/// JSON placeholder constants and formatting utilities
struct JsonPlaceholders;

impl JsonPlaceholders {
    /// Placeholder name for generic JSON values
    const VALUE: &'static str = "VALUE";

    /// Format a bracketed VALUE placeholder: "[VALUE]"
    fn value_placeholder_bracketed() -> String {
        format!("[{}]", Self::VALUE)
    }

    /// Format a key with VALUE placeholder suffix: "`key_VALUE`"
    fn key_with_value_placeholder(key: &str) -> String {
        format!("{}_{}", key, Self::VALUE)
    }

    /// Format a JSON key-value pair with actual value: "key": `actual_value`
    fn json_key_value_pair(key: &str, value_str: &str) -> String {
        format!("\"{key}\": {value_str}")
    }

    /// Format a JSON object with parts: {part1, part2, ...}
    fn format_object(parts: &[String]) -> String {
        format!("{{{}}}", parts.join(", "))
    }

    /// Format a JSON array with parts: [part1, part2, ...]
    fn format_array(parts: &[String]) -> String {
        format!("[{}]", parts.join(", "))
    }

    /// Format a quoted JSON string: "text"
    fn format_string(s: &str) -> String {
        format!("\"{s}\"")
    }

    /// Format array description: [N items]
    fn format_array_items(count: usize) -> String {
        format!("[{count} items]")
    }

    /// Format object description: {N keys}
    fn format_object_keys(count: usize) -> String {
        format!("{{{count} keys}}")
    }

    /// Format array type and length for frequency tracking: [type:N]
    fn format_array_type_length(value_type: &str, length: usize) -> String {
        format!("[{value_type}:{length}]")
    }
}

/// Compute max nesting depth of a JSON value (1-based: object/array = 1, children add 1).
fn json_max_depth(value: &Value) -> usize {
    match value {
        Value::Array(arr) => arr.iter().map(json_max_depth).max().map_or(1, |d| d + 1),
        Value::Object(map) => map.values().map(json_max_depth).max().map_or(1, |d| d + 1),
        _ => 1,
    }
}

/// Extract JSON metadata: line stats, root type/size, max depth, pretty-printed heuristic.
#[must_use]
pub fn extract_json_metadata(content: &str, stats: &ParseResult) -> JsonMetadata {
    let line_count = stats.line_count;
    let byte_count = stats.byte_count;

    // Line ending from raw bytes
    let bytes = content.as_bytes();
    let mut lf = 0usize;
    let mut crlf = 0usize;
    let mut cr = 0usize;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\r' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                crlf += 1;
                i += 2;
            } else {
                cr += 1;
                i += 1;
            }
        } else if bytes[i] == b'\n' {
            lf += 1;
            i += 1;
        } else {
            i += 1;
        }
    }

    let line_ending = if lf + crlf + cr == 0 {
        None
    } else {
        let total = lf + crlf + cr;
        Some(if crlf == total {
            "crlf".to_string()
        } else if lf == total {
            "lf".to_string()
        } else if cr == total {
            "cr".to_string()
        } else {
            "mixed".to_string()
        })
    };

    let max_line_length = content.lines().map(str::len).max();
    let blank_count = content.lines().filter(|l| l.trim().is_empty()).count();
    let blank_line_count = if blank_count > 0 {
        Some(blank_count)
    } else {
        None
    };

    // Parse root for type/size/depth; if line-by-line JSON, use first line
    let (root_type, root_array_length, root_object_key_count, max_depth) =
        if let Ok(root) = serde_json::from_str::<Value>(content) {
            match &root {
                Value::Array(arr) => (
                    Some("array".to_string()),
                    Some(arr.len()),
                    None,
                    Some(json_max_depth(&root)),
                ),
                Value::Object(map) => (
                    Some("object".to_string()),
                    None,
                    Some(map.len()),
                    Some(json_max_depth(&root)),
                ),
                _ => (None, None, None, Some(1)),
            }
        } else {
            // NDJSON / line-by-line: optional depth from first line
            let first_line = content.lines().next().unwrap_or("");
            if let Ok(v) = serde_json::from_str::<Value>(first_line) {
                (None, None, None, Some(json_max_depth(&v)))
            } else {
                (None, None, None, None)
            }
        };

    // Pretty-printed: has newline followed by space/tab (common in pretty-printed JSON)
    let pretty_printed = Some(content.contains("\n  ") || content.contains("\n\t"));

    JsonMetadata {
        byte_count,
        line_count,
        line_ending,
        max_line_length,
        blank_line_count,
        root_type,
        root_array_length,
        root_object_key_count,
        max_depth,
        pretty_printed,
    }
}

/// Extract templates from JSON files (JSON-aware analysis).
/// Accepts `content: &str` so the same function can be used when JSON is detected via
/// `structured::process` (mmap → str at call site) or via `extract_unknown_templates` (content already available).
///
/// # Errors
///
/// Currently always returns [`Ok`].
pub fn extract_json_templates(
    content: &str,
    stats: &ParseResult,
    config: &RuntimeConfig,
) -> Result<MiningResult> {
    let key_value_freq: DashMap<String, DashMap<String, usize>> = DashMap::new();
    let (parsed_objects, headers) = parse_json_content(content);

    if parsed_objects.is_empty() {
        return Ok(empty_mining_result(stats));
    }

    // First pass: collect frequencies in parallel with adaptive chunking
    let total_lines = parsed_objects.len();
    parsed_objects
        .as_slice()
        .par_iter_adaptive(config)
        .for_each(|value| {
            collect_json_frequencies(value, &key_value_freq, headers.as_ref());
        });

    // Second pass: extract patterns using frequency data in parallel with adaptive chunking
    let template_groups: DashMap<String, Vec<Value>> = DashMap::new();
    parsed_objects
        .as_slice()
        .par_iter_adaptive(config)
        .for_each(|value| {
            let pattern = extract_json_pattern(
                value,
                &key_value_freq,
                total_lines,
                config,
                headers.as_ref(),
            );
            template_groups
                .entry(pattern)
                .or_default()
                .push(value.clone());
        });

    // Convert groups to Template structs
    let templates: Vec<Template> = template_groups
        .iter()
        .map(|entry| {
            let pattern = entry.key().clone();
            let matching_objects = entry.value();

            // Extract examples for each dynamic value
            let mut examples: BTreeMap<String, Vec<String>> = BTreeMap::new();

            for obj in matching_objects.iter().take(config.max_sample_lines) {
                extract_json_examples(obj, &mut examples, config, headers.as_ref());
            }

            Template {
                pattern,
                count: matching_objects.len(),
                examples,
            }
        })
        .collect();

    // Build MiningResult using shared utility
    Ok(build_mining_result(templates, total_lines, stats, config))
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
/// Returns (`parsed_objects`, headers) where headers is Some if first array row contains all strings
fn parse_json_content(content: &str) -> (Vec<Value>, Option<Vec<String>>) {
    // Try parsing entire content as single JSON
    if let Ok(root_value) = serde_json::from_str::<Value>(content) {
        return parse_root_value(root_value);
    }

    // Fallback: try line-by-line parsing
    let parsed_objects: Vec<Value> = content
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect();

    (parsed_objects, None)
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
fn get_array_key(idx: usize, headers: Option<&Vec<String>>) -> String {
    if let Some(header_vec) = headers {
        if idx < header_vec.len() {
            header_vec[idx].clone()
        } else {
            format_placeholder_typed(PlaceholderType::Col, idx)
        }
    } else {
        format_placeholder_typed(PlaceholderType::Pos, idx)
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
    headers: Option<&Vec<String>>,
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
    config: &RuntimeConfig,
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
        JsonPlaceholders::value_placeholder_bracketed()
    };
    JsonPlaceholders::json_key_value_pair(key, &value_str)
}

/// Extract JSON pattern string (keys with placeholders for dynamic values)
fn extract_json_pattern(
    value: &Value,
    key_value_freq: &DashMap<String, DashMap<String, usize>>,
    total_lines: usize,
    config: &RuntimeConfig,
    headers: Option<&Vec<String>>,
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
                        map.get(*key)
                            .expect("key from sorted_keys derived from map.keys()"),
                        key_value_freq,
                        total_lines,
                        config,
                    )
                })
                .collect();
            JsonPlaceholders::format_object(&parts)
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
            JsonPlaceholders::format_array(&parts)
        }
        _ => JsonPlaceholders::value_placeholder_bracketed(),
    }
}

/// Format JSON value for frequency tracking (actual value representation)
fn format_json_value_for_freq(value: &Value) -> String {
    match value {
        Value::String(s) => JsonPlaceholders::format_string(s),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        Value::Array(arr) => {
            if arr.is_empty() {
                "[]".to_string()
            } else {
                // For arrays, use type and length for frequency tracking
                let value_type = get_value_type(
                    arr.first()
                        .expect("array non-empty in else branch of is_empty check"),
                );
                JsonPlaceholders::format_array_type_length(value_type, arr.len())
            }
        }
        Value::Object(obj) => {
            if obj.is_empty() {
                "{}".to_string()
            } else {
                // For objects, use key count for frequency tracking
                JsonPlaceholders::format_object_keys(obj.len())
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
        Value::String(s) => JsonPlaceholders::format_string(s),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        Value::Array(arr) => {
            if arr.is_empty() {
                "[]".to_string()
            } else {
                JsonPlaceholders::format_array_items(arr.len())
            }
        }
        Value::Object(obj) => {
            if obj.is_empty() {
                "{}".to_string()
            } else {
                JsonPlaceholders::format_object_keys(obj.len())
            }
        }
    }
}

/// Extract examples for JSON templates
fn extract_json_examples(
    value: &Value,
    examples: &mut BTreeMap<String, Vec<String>>,
    config: &RuntimeConfig,
    headers: Option<&Vec<String>>,
) {
    match value {
        Value::Object(map) => {
            for (key, val) in map {
                let placeholder = JsonPlaceholders::key_with_value_placeholder(key);
                add_example_value(val, &placeholder, examples, config);
            }
        }
        Value::Array(arr) => {
            for (idx, val) in arr.iter().enumerate() {
                let key = get_array_key(idx, headers);
                let placeholder = JsonPlaceholders::key_with_value_placeholder(&key);
                add_example_value(val, &placeholder, examples, config);
            }
        }
        _ => {
            add_example_value(value, JsonPlaceholders::VALUE, examples, config);
        }
    }
}

/// Add a value to examples if not already present and under limit
fn add_example_value(
    val: &Value,
    placeholder: &str,
    examples: &mut BTreeMap<String, Vec<String>>,
    config: &RuntimeConfig,
) {
    let entry = examples.entry(placeholder.to_string()).or_default();

    let value_str = match val {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        _ => format!("{val:?}"),
    };

    if !entry.contains(&value_str) && entry.len() < config.max_examples_per_placeholder {
        entry.push(value_str);
    }
}
