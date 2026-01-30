//! Coarse / shape-based sentence grouping for text templates.
//! Groups by (word count, end punctuation) so more sentences share a template than exact-pattern.

use crate::analysis::Punctuation;
use crate::engine::config::Config;
use crate::engine::tools::{
    PlaceholderType, format_placeholder_bracketed_typed, format_placeholder_typed,
};
use dashmap::DashMap;
use log::debug;
use std::collections::{BTreeMap, HashMap};

/// How the sentence ends (period, question, exclamation). Used as part of shape for coarser grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EndType {
    Period,
    Question,
    Exclamation,
}

/// Coarse descriptor for a sentence: length + end punctuation. Used to group sentences into templates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SentenceShape {
    pub word_count: usize,
    pub end_type: EndType,
}

/// Compute shape (word count + end type) for a sentence.
pub fn sentence_shape(sentence: &str) -> SentenceShape {
    let trimmed = sentence.trim();
    let word_count = trimmed.split_whitespace().count();
    let end_type = if trimmed.ends_with(Punctuation::QUESTION) {
        EndType::Question
    } else if trimmed.ends_with(Punctuation::EXCLAMATION) {
        EndType::Exclamation
    } else {
        EndType::Period
    };
    SentenceShape {
        word_count,
        end_type,
    }
}

/// Build pattern string for a shape: [WORD_00] [WORD_01] ... [WORD_(n-1)].
pub fn pattern_for_shape(shape: &SentenceShape) -> String {
    (0..shape.word_count)
        .map(|i| format_placeholder_bracketed_typed(PlaceholderType::Word, i))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Group sentences by shape (word count + end type). Fills template_groups: pattern → list of sentences.
/// Coarser than exact-pattern: more sentences share a template.
pub fn group_sentences_by_shape(
    sentences: &[String],
    template_groups: &DashMap<String, Vec<String>>,
    _config: &Config,
) {
    let mut by_shape: HashMap<SentenceShape, Vec<String>> = HashMap::new();
    for s in sentences {
        let sh = sentence_shape(s);
        by_shape.entry(sh).or_default().push(s.clone());
    }
    for (shape, list) in by_shape {
        let pattern = pattern_for_shape(&shape);
        template_groups.entry(pattern).or_default().extend(list);
    }
    debug!(
        "Shape fallback: {} pattern groups (by word count + end type) from {} sentences",
        template_groups.len(),
        sentences.len()
    );
}

/// True if pattern is only placeholders (e.g. "[WORD_00] [WORD_01] ...") with no literal tokens.
pub fn pattern_is_all_placeholders(pattern: &str) -> bool {
    pattern
        .split_whitespace()
        .all(|w| w.starts_with('[') && w.ends_with(']'))
}

/// True if the token is only punctuation/symbols (no alphanumeric). Used to avoid adding stray
/// tokens like ")" or "(" as example words.
pub fn token_has_alphanumeric(token: &str) -> bool {
    token.chars().any(|c| c.is_alphanumeric())
}

/// Fill examples by position: WORD_00 = first word from each sentence, WORD_01 = second, etc.
/// Used for shape/coarse fallback so we show actual words at each slot.
/// Skips punctuation-only tokens so they do not appear as example words.
pub fn extract_examples_by_position(
    matching_sentences: &[String],
    sample_size: usize,
    example_limit: usize,
    examples: &mut BTreeMap<String, Vec<String>>,
) {
    for sentence in matching_sentences.iter().take(sample_size) {
        let tokens: Vec<&str> = sentence.split_whitespace().collect();
        for (i, token) in tokens.iter().enumerate() {
            if !token_has_alphanumeric(token) {
                continue;
            }
            let key = format_placeholder_typed(PlaceholderType::Word, i);
            let entry = examples.entry(key).or_default();
            let s = (*token).to_string();
            if !entry.contains(&s) {
                entry.push(s);
            }
        }
    }
    for examples_list in examples.values_mut() {
        if examples_list.len() > example_limit {
            examples_list.truncate(example_limit);
        }
    }
}
