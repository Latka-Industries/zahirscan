//! Log file template extraction using position-based analysis

use crate::config::RuntimeConfig;
use crate::parsers::ParseResult;
use crate::parsers::traits::AdaptiveParallel;
use crate::results::{LogMetadata, MiningResult, Template};
use crate::utils::path_string_helper::{
    PlaceholderType, format_placeholder_bracketed_typed, format_placeholder_typed,
};
use crate::utils::typecheck::{parse_date_to_timestamp, parse_timestamp_to_seconds};
use anyhow::Result;
use dashmap::DashMap;
use rayon::prelude::*;
use std::collections::BTreeMap;

/// Maximum number of lines to sample when detecting timestamps in log files.
const TIMESTAMP_SAMPLE_LINES: usize = 10;

/// Extract log metadata: line stats (line_ending, max_line_length, blank_line_count) and has_timestamps.
pub fn extract_log_metadata(content: &str, stats: &ParseResult) -> LogMetadata {
    let line_count = stats.line_count;
    let byte_count = stats.byte_count;

    // Line ending: scan raw bytes for \r\n, \n, \r
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

    let max_line_length = content.lines().map(|l| l.len()).max();
    let blank_line_count = content
        .lines()
        .filter(|l| l.trim().is_empty())
        .count();

    // Sample first N lines: if any line has a token that parses as timestamp/date, set has_timestamps
    let has_timestamps = content
        .lines()
        .take(TIMESTAMP_SAMPLE_LINES)
        .any(|line| {
            line.split_whitespace()
                .next()
                .map(|first_token| {
                    parse_timestamp_to_seconds(first_token).is_some()
                        || parse_date_to_timestamp(first_token).is_some()
                })
                .unwrap_or(false)
        });

    LogMetadata {
        byte_count,
        line_count,
        line_ending,
        max_line_length,
        blank_line_count: if blank_line_count > 0 {
            Some(blank_line_count)
        } else {
            None
        },
        has_timestamps: Some(has_timestamps),
    }
}

/// Extract templates from log files (position-based analysis)
pub fn extract_log_templates(
    content: &str,
    stats: &ParseResult,
    config: &RuntimeConfig,
) -> Result<MiningResult> {
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    if total_lines == 0 {
        return Ok(crate::parsers::traits::empty_mining_result(stats));
    }

    // Build frequency map: position → token → count
    let freq_map: DashMap<usize, DashMap<String, usize>> = DashMap::new();

    // Process lines in parallel to build frequency map
    lines.as_slice().par_iter_adaptive(config).for_each(|line| {
        let tokens: Vec<&str> = line.split_whitespace().collect();

        for (pos, token) in tokens.iter().enumerate() {
            freq_map
                .entry(pos)
                .or_default()
                .entry(token.to_string())
                .and_modify(|c| *c += 1)
                .or_insert(1);
        }
    });

    // Determine max position (longest line)
    let max_pos = freq_map.iter().map(|entry| *entry.key()).max().unwrap_or(0);

    // Classify each position as static or dynamic
    let threshold = (total_lines as f64 * config.static_threshold) as usize;
    let mut classifications: Vec<(usize, bool, Option<String>)> = Vec::new();

    for pos in 0..=max_pos {
        if let Some(token_map) = freq_map.get(&pos) {
            // Find most common token at this position
            let (most_common, count) = token_map
                .iter()
                .max_by_key(|e| *e.value())
                .map(|e| (e.key().clone(), *e.value()))
                .unwrap_or_else(|| ("".to_string(), 0));

            let is_static = count >= threshold && token_map.len() == 1;
            classifications.push((
                pos,
                is_static,
                if is_static { Some(most_common) } else { None },
            ));
        } else {
            classifications.push((pos, false, None));
        }
    }

    // Group lines into templates based on static positions
    // Process in parallel with adaptive chunking
    let template_groups: DashMap<String, Vec<&str>> = DashMap::new();
    lines.as_slice().par_iter_adaptive(config).for_each(|line| {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        let pattern = build_pattern(&tokens, &classifications);
        template_groups.entry(pattern).or_default().push(line);
    });

    // Convert groups to Template structs
    let templates: Vec<Template> = template_groups
        .iter()
        .map(|entry| {
            let pattern = entry.key().clone();
            let matching_lines = entry.value();

            // Extract examples for each placeholder position
            let mut examples: BTreeMap<String, Vec<String>> = BTreeMap::new();

            // Collect unique values for each dynamic position
            for line in matching_lines.iter().take(config.max_sample_lines) {
                let tokens: Vec<&str> = line.split_whitespace().collect();

                for (pos, token) in tokens.iter().enumerate() {
                    if let Some((_, is_static, _)) = classifications.get(pos)
                        && !is_static
                    {
                        let placeholder = format_placeholder_typed(PlaceholderType::Position, pos);
                        let entry = examples.entry(placeholder).or_insert_with(|| {
                            Vec::with_capacity(config.max_examples_per_placeholder.min(10))
                        });
                        let token_str = token.to_string();
                        if !entry.contains(&token_str) {
                            entry.push(token_str);
                        }
                        // Limit examples per placeholder
                        if entry.len() > config.max_examples_per_placeholder {
                            break;
                        }
                    }
                }
            }

            Template {
                pattern,
                count: matching_lines.len(),
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

/// Build pattern string from tokens and classifications
fn build_pattern(tokens: &[&str], classifications: &[(usize, bool, Option<String>)]) -> String {
    let mut pattern_parts = Vec::new();

    for (pos, token) in tokens.iter().enumerate() {
        if let Some((_, is_static, static_token)) = classifications.get(pos) {
            if *is_static {
                // Use the static token (most common at this position)
                pattern_parts.push(static_token.clone().unwrap_or_else(|| token.to_string()));
            } else {
                // Dynamic token - use placeholder
                pattern_parts.push(format_placeholder_bracketed_typed(
                    PlaceholderType::Position,
                    pos,
                ));
            }
        } else {
            // Position not in classifications - treat as dynamic
            pattern_parts
                .push(crate::utils::path_string_helper::format_placeholder_bracketed("POS", pos));
        }
    }

    pattern_parts.join(" ")
}
