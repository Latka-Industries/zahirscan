//! Log file template extraction using position-based analysis

use crate::config::RuntimeConfig;
use crate::parsers::ParseResult;
use crate::parsers::traits::AdaptiveParallel;
use crate::results::{MiningResult, Template};
use crate::utils::path_string_helper::{
    PlaceholderType, format_placeholder_bracketed_typed, format_placeholder_typed,
};
use anyhow::Result;
use dashmap::DashMap;
use rayon::prelude::*;
use std::collections::BTreeMap;

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
