//! Markdown file template extraction with markdown structure awareness and sentence analysis

use crate::analysis::calculate_writing_footprint;
use crate::analysis::{DefaultSentenceAnalyzer, SentenceAnalyzer};
use crate::engine::config::Config;
use crate::engine::tools::{PlaceholderType, format_placeholder_bracketed_typed};
use crate::parsers::ParseResult;
use crate::parsers::traits::AdaptiveParallel;
use crate::results::{MiningResult, Template};
use anyhow::Result;
use dashmap::DashMap;
use rayon::prelude::*;
use regex::Regex;
use std::collections::BTreeMap;

/// Markdown element types
#[derive(Debug, Clone, PartialEq, Eq)]
enum MarkdownElement {
    Header {
        level: usize,
        text: String,
    },
    Paragraph {
        text: String,
    },
    ListItem {
        text: String,
        ordered: bool,
    },
    CodeBlock {
        language: Option<String>,
        content: String,
    },
    #[allow(dead_code)]
    InlineCode {
        content: String,
    },
    #[allow(dead_code)]
    Link {
        text: String,
        url: String,
    },
    #[allow(dead_code)]
    Bold {
        text: String,
    },
    #[allow(dead_code)]
    Italic {
        text: String,
    },
    HorizontalRule,
    BlockQuote {
        text: String,
    },
}

/// Extract templates from markdown files with structure awareness
pub fn extract_markdown_templates(
    content: &str,
    stats: &ParseResult,
    config: &Config,
) -> Result<MiningResult> {
    if content.trim().is_empty() {
        return Ok(crate::parsers::traits::empty_mining_result(stats));
    }

    // Parse markdown structure
    let elements = parse_markdown_structure(content);

    if elements.is_empty() {
        return Ok(crate::parsers::traits::empty_mining_result(stats));
    }

    // Extract markdown structure patterns
    let structure_patterns = extract_structure_patterns(&elements, config);

    // Build templates from markdown elements
    let templates = build_markdown_templates(&elements, &structure_patterns, config);

    // Count total items (markdown elements)
    let total_items = elements.len();

    // Extract sentences from paragraphs for writing footprint
    // Parallelize sentence extraction for better performance
    let sentences: Vec<String> = elements
        .as_slice()
        .par_iter_adaptive(config)
        .filter_map(|elem| match elem {
            MarkdownElement::Paragraph { text } => Some(text.clone()),
            MarkdownElement::BlockQuote { text } => Some(text.clone()),
            _ => None,
        })
        .flat_map(|text| DefaultSentenceAnalyzer::extract_sentences(&text))
        .collect();

    // Calculate writing footprint metrics (using shared function)
    let writing_footprint = calculate_writing_footprint(&sentences, &templates, content, config);

    // Build MiningResult using shared utility, including writing footprint in compression calculation
    let result = crate::parsers::traits::build_mining_result_with_footprint(
        templates,
        total_items,
        stats,
        config,
        Some(&writing_footprint),
    );

    Ok(result)
}

/// Parse markdown content into structured elements
fn parse_markdown_structure(content: &str) -> Vec<MarkdownElement> {
    let mut elements = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let horizontal_rule_re = Regex::new(r"^[-*_]{3,}$").unwrap();
    let list_re = Regex::new(r"^(\s*)([-*+]|\d+\.)\s+").unwrap();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();

        // Skip empty lines
        if line.is_empty() {
            i += 1;
            continue;
        }

        // Try parsing each element type in priority order
        match try_parse_element(&lines, i, line, &horizontal_rule_re, &list_re) {
            ElementParseResult::Single(elem, next_idx) => {
                elements.push(elem);
                i = next_idx;
            }
            ElementParseResult::Multiple(elems, next_idx) => {
                elements.extend(elems);
                i = next_idx;
            }
            ElementParseResult::None => {
                i += 1;
            }
        }
    }

    elements
}

/// Result of attempting to parse a markdown element
enum ElementParseResult {
    /// Single element parsed, advance to next_idx
    Single(MarkdownElement, usize),
    /// Multiple elements parsed (e.g., list items), advance to next_idx
    Multiple(Vec<MarkdownElement>, usize),
    /// No element matched, advance by 1
    None,
}

/// Try to parse a markdown element at the given index
fn try_parse_element(
    lines: &[&str],
    idx: usize,
    line: &str,
    horizontal_rule_re: &Regex,
    list_re: &Regex,
) -> ElementParseResult {
    // Headers (# ## ### etc.) - highest priority
    parse_header(line)
        .map(|header| ElementParseResult::Single(header, idx + 1))
        .or_else(|| {
            // Horizontal rule
            if horizontal_rule_re.is_match(line) {
                Some(ElementParseResult::Single(
                    MarkdownElement::HorizontalRule,
                    idx + 1,
                ))
            } else {
                None
            }
        })
        .or_else(|| {
            // Code blocks (```) - multi-line
            line.starts_with("```")
                .then(|| parse_code_block(lines, idx))
                .flatten()
                .map(|(code_block, next_idx)| ElementParseResult::Single(code_block, next_idx))
        })
        .or_else(|| {
            // Block quotes (>) - multi-line
            line.starts_with('>')
                .then(|| parse_block_quote(lines, idx, horizontal_rule_re, list_re))
                .flatten()
                .map(|(block_quote, next_idx)| ElementParseResult::Single(block_quote, next_idx))
        })
        .or_else(|| {
            // Lists (-, *, +, or numbered) - multi-line, can produce multiple elements
            parse_list(lines, idx)
                .map(|(list_items, next_idx)| ElementParseResult::Multiple(list_items, next_idx))
        })
        .or_else(|| {
            // Paragraph (collect consecutive non-special lines) - multi-line
            parse_paragraph(lines, idx, horizontal_rule_re, list_re)
                .map(|(paragraph, next_idx)| ElementParseResult::Single(paragraph, next_idx))
        })
        .unwrap_or(ElementParseResult::None)
}

/// Parse markdown header (# ## ### etc.)
fn parse_header(line: &str) -> Option<MarkdownElement> {
    let header_re = Regex::new(r"^(#{1,6})\s+(.+)$").ok()?;
    if let Some(caps) = header_re.captures(line) {
        let level = caps.get(1)?.as_str().len();
        let text = caps.get(2)?.as_str().to_string();
        return Some(MarkdownElement::Header { level, text });
    }
    None
}

/// Parse code block (```language\ncontent\n```)
fn parse_code_block(lines: &[&str], start_idx: usize) -> Option<(MarkdownElement, usize)> {
    let first_line = lines[start_idx].trim();
    let language = if first_line.len() > 3 {
        Some(first_line[3..].trim().to_string())
    } else {
        None
    };

    let mut content = String::new();
    let mut i = start_idx + 1;

    while i < lines.len() {
        if lines[i].trim().starts_with("```") {
            return Some((
                MarkdownElement::CodeBlock {
                    language,
                    content: content.trim().to_string(),
                },
                i + 1,
            ));
        }
        if i > start_idx {
            content.push('\n');
        }
        content.push_str(lines[i]);
        i += 1;
    }

    None // Unclosed code block
}

/// Parse block quote (>)
fn parse_block_quote(
    lines: &[&str],
    start_idx: usize,
    _horizontal_rule_re: &Regex,
    _list_re: &Regex,
) -> Option<(MarkdownElement, usize)> {
    let mut text_parts = Vec::new();
    let mut i = start_idx;

    while i < lines.len() && lines[i].trim().starts_with('>') {
        let line = lines[i].trim();
        let quote_text = if line.len() > 1 { line[1..].trim() } else { "" };
        if !quote_text.is_empty() {
            text_parts.push(quote_text);
        }
        i += 1;
    }

    if text_parts.is_empty() {
        None
    } else {
        Some((
            MarkdownElement::BlockQuote {
                text: text_parts.join(" "),
            },
            i,
        ))
    }
}

/// Parse list items (-, *, +, or numbered)
fn parse_list(lines: &[&str], start_idx: usize) -> Option<(Vec<MarkdownElement>, usize)> {
    let list_re = Regex::new(r"^(\s*)([-*+]|\d+\.)\s+(.+)$").ok()?;
    let first_line = lines[start_idx].trim();

    if !list_re.is_match(first_line) {
        return None;
    }

    let mut items = Vec::new();
    let mut i = start_idx;

    while i < lines.len() {
        let line = lines[i];
        if let Some(caps) = list_re.captures(line) {
            let marker = caps.get(2)?.as_str();
            let text = caps.get(3)?.as_str().to_string();
            let ordered = marker.parse::<usize>().is_ok();
            items.push(MarkdownElement::ListItem { text, ordered });
            i += 1;
        } else if line.trim().is_empty() {
            i += 1;
            break;
        } else {
            break;
        }
    }

    if items.is_empty() {
        None
    } else {
        Some((items, i))
    }
}

/// Parse paragraph (consecutive non-special lines)
fn parse_paragraph(
    lines: &[&str],
    start_idx: usize,
    horizontal_rule_re: &Regex,
    list_re: &Regex,
) -> Option<(MarkdownElement, usize)> {
    let mut text_parts = Vec::new();
    let mut i = start_idx;

    while i < lines.len() {
        let line = lines[i].trim();

        if line.is_empty() {
            break;
        }

        // Stop at special markdown elements
        if line.starts_with('#')
            || line.starts_with("```")
            || line.starts_with('>')
            || horizontal_rule_re.is_match(line)
            || list_re.is_match(line)
        {
            break;
        }

        text_parts.push(line);
        i += 1;
    }

    if text_parts.is_empty() {
        None
    } else {
        Some((
            MarkdownElement::Paragraph {
                text: text_parts.join(" "),
            },
            i,
        ))
    }
}

/// Increment frequency counter for a pattern in DashMap
#[inline]
fn increment_pattern_freq(pattern_freq: &DashMap<String, usize>, pattern: String) {
    pattern_freq
        .entry(pattern)
        .and_modify(|c| *c += 1)
        .or_insert(1);
}

/// Generate pattern string for a markdown element
fn element_to_pattern(elem: &MarkdownElement) -> Option<String> {
    match elem {
        MarkdownElement::Header { level, .. } => Some(format_placeholder_bracketed_typed(
            PlaceholderType::Header,
            *level,
        )),
        MarkdownElement::ListItem { ordered, text } => {
            let word_count = text.split_whitespace().count();
            let list_type = if *ordered { "ordered" } else { "unordered" };
            let base_pattern =
                format_placeholder_bracketed_typed(PlaceholderType::List, word_count);
            Some(format!("{}:type={}", base_pattern, list_type))
        }
        MarkdownElement::CodeBlock { language, .. } => {
            let lang_str = language.as_deref().unwrap_or("unknown");
            let base_pattern = format_placeholder_bracketed_typed(PlaceholderType::CodeBlock, 0);
            Some(format!("{}:lang={}", base_pattern, lang_str))
        }
        _ => None,
    }
}

/// Extract patterns from markdown structure (headers, lists, etc.)
fn extract_structure_patterns(
    elements: &[MarkdownElement],
    config: &Config,
) -> Vec<(String, usize)> {
    let pattern_freq: DashMap<String, usize> = DashMap::new();

    // Process elements in parallel to build frequency map
    elements.par_iter_adaptive(config).for_each(|elem| {
        if let Some(pattern) = element_to_pattern(elem) {
            increment_pattern_freq(&pattern_freq, pattern);
        }
    });

    let threshold = (elements.len() as f64 * config.text_threshold) as usize;
    pattern_freq
        .iter()
        .filter(|entry| *entry.value() >= threshold)
        .map(|entry| (entry.key().clone(), *entry.value()))
        .collect()
}

/// Build templates from markdown elements and patterns
fn build_markdown_templates(
    elements: &[MarkdownElement],
    _structure_patterns: &[(String, usize)],
    config: &Config,
) -> Vec<Template> {
    let mut templates = Vec::new();

    // Group elements by structure pattern
    // Process in parallel with adaptive chunking
    let element_groups: DashMap<String, Vec<&MarkdownElement>> = DashMap::new();

    elements.par_iter_adaptive(config).for_each(|elem| {
        let pattern = match elem {
            MarkdownElement::Header { level, .. } => {
                // Group headers by level - all H1 together, all H2 together, etc.
                // This makes header hierarchy more visible in templates
                Some(format_placeholder_bracketed_typed(
                    PlaceholderType::Header,
                    *level,
                ))
            }
            MarkdownElement::ListItem { ordered, text } => {
                let word_count = text.split_whitespace().count();
                let list_type = if *ordered { "ordered" } else { "unordered" };
                // Use type-safe placeholder for list, with metadata suffix
                let base_pattern =
                    format_placeholder_bracketed_typed(PlaceholderType::List, word_count);
                Some(format!("{}:type={}", base_pattern, list_type))
            }
            MarkdownElement::Paragraph { text } => {
                // Use sentence patterns for paragraphs
                let stats = DefaultSentenceAnalyzer::analyze_sentence_structure(text);
                // Use type-safe placeholder for paragraph, with metadata suffix
                let base_pattern = format_placeholder_bracketed_typed(
                    PlaceholderType::Paragraph,
                    stats.word_count,
                );
                Some(format!("{}:quotes={}", base_pattern, stats.has_quotes))
            }
            _ => None,
        };
        if let Some(pattern) = pattern {
            element_groups.entry(pattern).or_default().push(elem);
        }
    });

    // Convert groups to templates
    for entry in element_groups.iter() {
        let pattern = entry.key().clone();
        let matching_elements = entry.value();

        let mut examples: BTreeMap<String, Vec<String>> = BTreeMap::new();

        for elem in matching_elements.iter().take(config.max_sample_lines) {
            match elem {
                MarkdownElement::Header { text, .. } => {
                    let entry = examples.entry("header_text".to_string()).or_default();
                    if !entry.contains(text) && entry.len() < config.max_examples_per_placeholder {
                        entry.push(text.clone());
                    }
                }
                MarkdownElement::ListItem { text, .. } => {
                    let entry = examples.entry("list_text".to_string()).or_default();
                    if !entry.contains(text) && entry.len() < config.max_examples_per_placeholder {
                        entry.push(text.clone());
                    }
                }
                MarkdownElement::Paragraph { text } => {
                    let entry = examples.entry("paragraph_text".to_string()).or_default();
                    let preview = text
                        .chars()
                        .take(config.markdown_preview_length)
                        .collect::<String>();
                    if !entry.contains(&preview)
                        && entry.len() < config.max_examples_per_placeholder
                    {
                        entry.push(preview);
                    }
                }
                _ => {}
            }
        }

        templates.push(Template {
            pattern,
            count: matching_elements.len(),
            examples,
        });
    }

    templates
}
