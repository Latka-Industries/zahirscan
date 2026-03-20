//! N-gram and phrase frequency utilities for text template extraction.
//! Builds structural and token-based patterns, collects frequent n-grams/phrases, extracts examples.

use dashmap::DashMap;
use log::debug;
use rayon::prelude::*;
use std::collections::BTreeMap;

use crate::config::RuntimeConfig;
use crate::parsers::traits::AdaptiveParallel;
use crate::utils::path_string_helper::{
    PlaceholderType, format_placeholder_bracketed_typed, format_placeholder_typed,
};

/// (item, count) list for frequent n-grams or phrases; used to avoid complex inline types.
pub type FrequentEntries = Vec<(String, usize)>;

/// Collect entries from a frequency map that meet the threshold (count >= threshold).
#[must_use]
pub fn collect_frequent_entries(
    freq: &DashMap<String, usize>,
    threshold: usize,
) -> FrequentEntries {
    freq.iter()
        .filter(|entry| *entry.value() >= threshold)
        .map(|entry| (entry.key().clone(), *entry.value()))
        .collect()
}

/// Filter n-gram and phrase maps by threshold, return frequent lists, and log the result.
#[must_use]
pub fn filter_frequent_ngrams_and_phrases(
    ngram_freq: &DashMap<String, usize>,
    phrase_freq: &DashMap<String, usize>,
    threshold: usize,
    total_sentences: usize,
    config: &RuntimeConfig,
) -> (FrequentEntries, FrequentEntries) {
    let frequent_ngrams = collect_frequent_entries(ngram_freq, threshold);
    let frequent_phrases = collect_frequent_entries(phrase_freq, threshold);
    debug!(
        "Filtering n-grams with threshold: {} ({:.2}% of {} sentences); kept {} n-grams, {} phrases",
        threshold,
        config.text_threshold * 100.0,
        total_sentences,
        frequent_ngrams.len(),
        frequent_phrases.len()
    );
    (frequent_ngrams, frequent_phrases)
}

/// Build n-gram and phrase frequency maps from sentences (parallel). Fills `ngram_freq` and `phrase_freq` in place.
pub fn build_ngram_and_phrase_freq(
    sentences: &[String],
    ngram_freq: &DashMap<String, usize>,
    phrase_freq: &DashMap<String, usize>,
    config: &RuntimeConfig,
) {
    sentences
        .par_iter_adaptive(config)
        .enumerate()
        .for_each(|(idx, sentence)| {
            if idx > 0 && idx % 10_000 == 0 {
                debug!("Processed {idx} sentences for n-grams");
            }

            let tokens: Vec<&str> = sentence.split_whitespace().collect();

            for n in config.min_ngram_size..=config.max_ngram_size {
                for window in tokens.windows(n) {
                    let ngram = window.join(" ");
                    ngram_freq.entry(ngram).and_modify(|c| *c += 1).or_insert(1);
                }
            }

            if tokens.len() >= config.min_phrase_length {
                for window in tokens.windows(config.min_phrase_length) {
                    let phrase = window.join(" ");
                    phrase_freq
                        .entry(phrase)
                        .and_modify(|c| *c += 1)
                        .or_insert(1);
                }
            }
        });
    debug!(
        "N-gram results from {} sentences: {} unique n-grams, {} unique phrases",
        sentences.len(),
        ngram_freq.len(),
        phrase_freq.len()
    );
}

/// Walk a token slice left-to-right: match n-grams where possible, else emit [`WORD_idx`] per token.
/// Appends to `pattern_parts` and returns the next placeholder index.
fn tokens_to_pattern_parts(
    tokens: &[&str],
    placeholder_idx: &mut usize,
    pattern_parts: &mut Vec<String>,
    frequent_ngrams: &[(String, usize)],
    config: &RuntimeConfig,
) {
    let mut i = 0;
    while i < tokens.len() {
        let mut matched = false;
        for n in (config.min_ngram_size..=config.max_ngram_size).rev() {
            if i + n <= tokens.len() {
                let candidate = tokens[i..i + n].join(" ");
                if frequent_ngrams.iter().any(|(ngram, _)| ngram == &candidate) {
                    pattern_parts.push(candidate);
                    i += n;
                    matched = true;
                    break;
                }
            }
        }
        if !matched {
            pattern_parts.push(format_placeholder_bracketed_typed(
                PlaceholderType::Word,
                *placeholder_idx,
            ));
            *placeholder_idx += 1;
            i += 1;
        }
    }
}

/// Build structural pattern (pivot-based) enriched with n-grams for writing footprint.
/// Language-agnostic: finds structural pivot points (words with high variation after them).
/// Emits [`WORD_00`] [`WORD_01`] ... pivot ... [`WORD_xx`] [`WORD_xx+1`] ... so pattern aligns with examples.
#[must_use]
pub fn build_enriched_structural_pattern(
    tokens: &[&str],
    pivot_patterns: &DashMap<String, usize>,
    frequent_ngrams: &[(String, usize)],
    _frequent_phrases: &[(String, usize)],
    config: &RuntimeConfig,
) -> Option<String> {
    if tokens.is_empty() {
        return None;
    }

    let mut best_pivot: Option<(usize, &str)> = None;
    let mut best_score = 0;

    for (pos, token) in tokens.iter().enumerate() {
        let pattern_key = format!("P_{pos}_{token}");
        if let Some(count) = pivot_patterns.get(&pattern_key) {
            let score = *count.value();
            if score > best_score {
                best_score = score;
                best_pivot = Some((pos, token));
            }
        }
    }

    if let Some((p_pos, p_word)) = best_pivot {
        let threshold = (tokens.len() as f64 * config.text_threshold) as usize;

        if best_score >= threshold {
            let mut pattern_parts = Vec::new();
            let mut placeholder_idx = 0usize;

            // Prefix: per-token or n-gram, [WORD_00] [WORD_01] ...
            if p_pos > 0 {
                tokens_to_pattern_parts(
                    &tokens[0..p_pos],
                    &mut placeholder_idx,
                    &mut pattern_parts,
                    frequent_ngrams,
                    config,
                );
            }

            pattern_parts.push(p_word.to_string());

            // Suffix: same, continuing [WORD_xx] [WORD_xx+1] ...
            if p_pos + 1 < tokens.len() {
                tokens_to_pattern_parts(
                    &tokens[p_pos + 1..],
                    &mut placeholder_idx,
                    &mut pattern_parts,
                    frequent_ngrams,
                    config,
                );
            }

            return Some(pattern_parts.join(" "));
        }
    }

    None
}

/// Build pattern string for text using n-gram matching.
#[must_use]
pub fn build_text_pattern(
    tokens: &[&str],
    frequent_ngrams: &[(String, usize)],
    frequent_phrases: &[(String, usize)],
    config: &RuntimeConfig,
) -> String {
    if tokens.is_empty() {
        return String::new();
    }

    let mut pattern_parts = Vec::new();
    let mut i = 0;

    while i < tokens.len() {
        let mut matched = false;

        for n in (config.min_ngram_size..=config.max_ngram_size).rev() {
            if i + n <= tokens.len() {
                let candidate = tokens[i..i + n].join(" ");
                if frequent_ngrams.iter().any(|(ngram, _)| ngram == &candidate) {
                    pattern_parts.push(candidate);
                    i += n;
                    matched = true;
                    break;
                }
            }
        }

        if !matched && i + config.min_phrase_length <= tokens.len() {
            let candidate = tokens[i..i + config.min_phrase_length].join(" ");
            if frequent_phrases
                .iter()
                .any(|(phrase, _)| phrase == &candidate)
            {
                pattern_parts.push(candidate);
                i += config.min_phrase_length;
                matched = true;
            }
        }

        if !matched {
            pattern_parts.push(format_placeholder_bracketed_typed(PlaceholderType::Word, i));
            i += 1;
        }
    }

    pattern_parts.join(" ")
}

/// Extract examples for text templates (placeholder → list of example values).
pub fn extract_text_examples(
    tokens: &[&str],
    frequent_ngrams: &[(String, usize)],
    frequent_phrases: &[(String, usize)],
    examples: &mut BTreeMap<String, Vec<String>>,
    config: &RuntimeConfig,
) {
    let mut i = 0;
    let mut placeholder_idx = 0;

    while i < tokens.len() {
        let mut matched = false;

        for n in (config.min_ngram_size..=config.max_ngram_size).rev() {
            if i + n <= tokens.len() {
                let candidate = tokens[i..i + n].join(" ");
                if frequent_ngrams.iter().any(|(ngram, _)| ngram == &candidate) {
                    i += n;
                    matched = true;
                    break;
                }
            }
        }

        if !matched && i + config.min_phrase_length <= tokens.len() {
            let candidate = tokens[i..i + config.min_phrase_length].join(" ");
            if frequent_phrases
                .iter()
                .any(|(phrase, _)| phrase == &candidate)
            {
                i += config.min_phrase_length;
                matched = true;
            }
        }

        if !matched {
            let placeholder = format_placeholder_typed(PlaceholderType::Word, placeholder_idx);
            let entry = examples
                .entry(placeholder)
                .or_insert_with(|| Vec::with_capacity(config.max_examples_per_placeholder.min(10)));
            let token_str = tokens[i].to_string();
            if !entry.contains(&token_str) {
                entry.push(token_str);
            }
            if entry.len() > config.max_examples_per_placeholder {
                break;
            }
            placeholder_idx += 1;
            i += 1;
        }
    }
}
