//! Text file template extraction using sentence-level analysis and n-gram/phrase-based patterns

use crate::analysis::{self, DefaultSentenceAnalyzer, SentenceAnalyzer, ngrams};
use crate::engine::config::Config;
use crate::parsers::{
    ParseResult,
    traits::{AdaptiveParallel, build_mining_result_with_footprint, empty_mining_result},
};
use crate::results::{MiningResult, Template};
use anyhow::Result;
use dashmap::DashMap;
use log::debug;
use rayon::prelude::*;
use std::collections::BTreeMap;

/// Filter segments before n-gram extraction and template grouping: keep only those that
/// look like sentences (min words, alphanumeric, min length or min_words_alt). Drops
/// fragments so n-grams and templates are built from sentence-like text only. Mutates in place.
fn filter_sentences_before_ngrams(sentences: &mut Vec<String>, config: &Config) {
    let before_filter = sentences.len();
    sentences.retain(|s| {
        let trimmed = s.trim();
        let word_count = trimmed.split_whitespace().count();
        word_count >= config.min_sentence_words
            && trimmed.chars().any(|c| c.is_alphanumeric())
            && (trimmed.len() >= config.min_sentence_length
                || word_count >= config.min_sentence_words_alt)
    });
    debug!(
        "Filtered sentences: {} kept of {} (min_words={}, min_length={} or min_words_alt={}, alphanumeric required)",
        sentences.len(),
        before_filter,
        config.min_sentence_words,
        config.min_sentence_length,
        config.min_sentence_words_alt
    );
    if sentences.is_empty() {
        debug!("No sentences after filtering, caller will return empty mining result");
    }
}

/// Pre-compute tokens for each sentence (parallel). Returns Vec<Vec<String>>, one token list per sentence.
fn precompute_sentence_tokens(sentences: &[String], config: &Config) -> Vec<Vec<String>> {
    let sentence_tokens: Vec<Vec<String>> = sentences
        .par_iter_adaptive(config)
        .map(|s| {
            s.split_whitespace()
                .map(|t| t.to_string())
                .collect::<Vec<String>>()
        })
        .collect();
    debug!(
        "Pre-computed tokens for {} sentences",
        sentence_tokens.len()
    );
    sentence_tokens
}

/// Group sentences by exact pattern (parallel). Fills template_groups: pattern → list of sentences.
/// Uses n-gram/text pattern only; pivot is kept for SVO/writing footprint, not for grouping.
fn group_sentences_by_pattern(
    sentences: &[String],
    sentence_tokens: &[Vec<String>],
    template_groups: &DashMap<String, Vec<String>>,
    frequent_ngrams: &[(String, usize)],
    frequent_phrases: &[(String, usize)],
    config: &Config,
) {
    sentences
        .par_iter_adaptive(config)
        .zip(sentence_tokens.par_iter_adaptive(config))
        .for_each(|(sentence, tokens)| {
            let token_refs: Vec<&str> = tokens.iter().map(|s| s.as_str()).collect();
            let pattern =
                ngrams::build_text_pattern(&token_refs, frequent_ngrams, frequent_phrases, config);

            template_groups
                .entry(pattern)
                .or_default()
                .push(sentence.to_string());
        });
    debug!(
        "Template grouping (exact pattern grouping): {} sentences in {} pattern groups (each pattern → list of sentences)",
        sentences.len(),
        template_groups.len()
    );
}

/// Convert template_groups (pattern → sentences) into Vec<Template>. Keeps only groups with
/// count >= min_template_count and count >= 2 (never show templates with only one sentence).
/// Skips patterns longer than avg_sentence_length * 2; adds examples and entropy.
fn build_templates_from_groups(
    template_groups: &DashMap<String, Vec<String>>,
    min_template_count: usize,
    avg_sentence_length: usize,
    frequent_ngrams: &[(String, usize)],
    frequent_phrases: &[(String, usize)],
    config: &Config,
) -> Vec<Template> {
    let min_count = min_template_count.max(2);
    let templates: Vec<Template> = template_groups
        .iter()
        .filter(|entry| entry.value().len() >= min_count)
        .filter_map(|entry| {
            let pattern = entry.key().clone();
            let matching_sentences = entry.value();

            let pattern_word_count = pattern.split_whitespace().count();
            if pattern_word_count > avg_sentence_length * 2 {
                return None;
            }

            let mut examples: BTreeMap<String, Vec<String>> = BTreeMap::new();
            let example_limit =
                (config.max_examples_per_placeholder / 2).max(config.min_examples_per_placeholder);
            let sample_size = example_limit.min(config.max_examples_for_entropy);

            if analysis::pattern_is_all_placeholders(&pattern) {
                // Shape/coarse fallback: show actual words at each position (no n-gram matching)
                analysis::extract_examples_by_position(
                    matching_sentences,
                    sample_size,
                    example_limit,
                    &mut examples,
                );
            } else {
                for sentence in matching_sentences.iter().take(sample_size) {
                    let tokens: Vec<&str> = sentence.split_whitespace().collect();
                    ngrams::extract_text_examples(
                        &tokens,
                        frequent_ngrams,
                        frequent_phrases,
                        &mut examples,
                        config,
                    );
                }
                for examples_list in examples.values_mut() {
                    if examples_list.len() > example_limit {
                        examples_list.truncate(example_limit);
                    }
                }
            }

            let entropy = if matching_sentences.len() >= config.min_entropy_sample_size {
                analysis::calculate_template_entropy(&examples, matching_sentences.len(), config)
            } else {
                0.0
            };

            let enriched_pattern =
                if entropy > config.min_entropy_display && entropy < config.max_entropy_display {
                    format!("{} [entropy={:.2}]", pattern, entropy)
                } else {
                    pattern
                };

            Some(Template {
                pattern: enriched_pattern,
                count: matching_sentences.len(),
                examples,
            })
        })
        .collect();

    debug!(
        "Final templates: {} (kept only groups with ≥{} sentences)",
        templates.len(),
        min_count
    );

    templates
}

/// Extract templates from text files using sentence-level analysis
/// For long-form text, works at sentence level rather than line-by-line
pub fn extract_text_templates(
    content: &str,
    stats: &ParseResult,
    config: &Config,
) -> Result<MiningResult> {
    if content.trim().is_empty() {
        return Ok(empty_mining_result(stats));
    }

    // Extract sentences (DefaultSentenceAnalyzer normalizes content internally)
    let mut sentences = DefaultSentenceAnalyzer::extract_sentences(content);

    // Keep unfiltered list for pivot extraction and SVO (position alignment)
    let original_sentences = sentences.clone();

    // Extract pivot points from unfiltered sentences (before min-sentence filter)
    let pivot_patterns = analysis::extract_pivot_points(&original_sentences, config);

    // SVO analysis: run on unfiltered sentences + pivot_patterns (right after pivot extraction)
    let svo_analysis = Some(analysis::analyze_svo_structure(
        &original_sentences,
        &pivot_patterns,
        config,
    ));

    // Filter before n-grams: keep only sentence-like segments (min words, length, alphanumeric)
    filter_sentences_before_ngrams(&mut sentences, config);

    let total_sentences_after_filter = sentences.len();

    if total_sentences_after_filter == 0 {
        return Ok(empty_mining_result(stats));
    }

    let ngram_freq: DashMap<String, usize> = DashMap::new();
    let phrase_freq: DashMap<String, usize> = DashMap::new();

    ngrams::build_ngram_and_phrase_freq(sentences.as_slice(), &ngram_freq, &phrase_freq, config);

    // Find frequent n-grams and phrases (appear in at least threshold% of sentences)
    let threshold = (total_sentences_after_filter as f64 * config.text_threshold) as usize;
    let (frequent_ngrams, frequent_phrases) = ngrams::filter_frequent_ngrams_and_phrases(
        &ngram_freq,
        &phrase_freq,
        threshold,
        total_sentences_after_filter,
        config,
    );

    // Pre-compute tokens for each sentence
    let sentence_tokens = precompute_sentence_tokens(sentences.as_slice(), config);

    // Template groups: pattern → list of sentences
    // First pass: exact-pattern grouping. If that yields no templates, fall back to sentence-length grouping.
    let template_groups: DashMap<String, Vec<String>> = DashMap::new();
    group_sentences_by_pattern(
        sentences.as_slice(),
        &sentence_tokens,
        &template_groups,
        &frequent_ngrams,
        &frequent_phrases,
        config,
    );

    // Convert groups to Template structs
    // Only keep templates that appear multiple times (better compression)
    let min_template_count =
        (total_sentences_after_filter as f64 * config.text_threshold).max(2.0) as usize;

    debug!("Min template count: {}", min_template_count);

    // Calculate average sentence length for compression check
    // Process in parallel with adaptive chunking
    let avg_sentence_length: usize = if !sentences.is_empty() {
        sentences
            .as_slice()
            .par_iter_adaptive(config)
            .map(|s| s.split_whitespace().count())
            .sum::<usize>()
            / sentences.len()
    } else {
        0
    };

    debug!("Average sentence length: {}", avg_sentence_length);

    let mut templates = build_templates_from_groups(
        &template_groups,
        min_template_count,
        avg_sentence_length,
        &frequent_ngrams,
        &frequent_phrases,
        config,
    );

    if templates.is_empty() {
        debug!("Running shape fallback (group by word count + end type)");
        template_groups.clear();
        analysis::group_sentences_by_shape(sentences.as_slice(), &template_groups, config);
        // Allow singletons so every length bucket becomes a template (min_template_count = 1)
        templates = build_templates_from_groups(
            &template_groups,
            min_template_count,
            avg_sentence_length,
            &frequent_ngrams,
            &frequent_phrases,
            config,
        );
    } else {
        debug!(
            "Exact pattern produced {} templates; skipping fallback",
            templates.len()
        );
    }

    // Calculate writing footprint metrics
    let mut writing_footprint =
        analysis::calculate_writing_footprint(&sentences, &templates, content, config);

    // Attach SVO analysis (computed earlier from unfiltered sentences + pivot_patterns)
    writing_footprint.svo_analysis = svo_analysis;

    // Build MiningResult using shared utility, including writing footprint in compression calculation
    let result = build_mining_result_with_footprint(
        templates,
        total_sentences_after_filter,
        stats,
        config,
        Some(&writing_footprint),
    );

    Ok(result)
}
