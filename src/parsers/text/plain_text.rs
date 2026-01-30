//! Text file template extraction using sentence-level analysis and n-gram/phrase-based patterns

use crate::engine::config::Config;
use crate::engine::tools::{
    PlaceholderType, format_placeholder_bracketed_typed, format_placeholder_typed,
};
use crate::parsers::ParseResult;
use crate::parsers::text::writing_analysis::{
    analyze_svo_structure, calculate_template_entropy, calculate_writing_footprint,
    extract_pivot_points,
};
use crate::parsers::traits::{
    AdaptiveParallel, DefaultSentenceAnalyzer, SentenceAnalyzer,
    build_mining_result_with_footprint, empty_mining_result,
};
use crate::results::{MiningResult, Template};
use anyhow::Result;
use dashmap::DashMap;
use log::debug;
use rayon::prelude::*;
use std::collections::BTreeMap;

/// (item, count) list for frequent n-grams or phrases; used to avoid complex inline types.
type FrequentEntries = Vec<(String, usize)>;

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

/// Collect entries from a frequency map that meet the threshold (count >= threshold).
fn collect_frequent_entries(freq: &DashMap<String, usize>, threshold: usize) -> FrequentEntries {
    freq.iter()
        .filter(|entry| *entry.value() >= threshold)
        .map(|entry| (entry.key().clone(), *entry.value()))
        .collect()
}

/// Filter n-gram and phrase maps by threshold, return frequent lists, and log the result.
fn filter_frequent_ngrams_and_phrases(
    ngram_freq: &DashMap<String, usize>,
    phrase_freq: &DashMap<String, usize>,
    threshold: usize,
    total_sentences: usize,
    config: &Config,
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

/// Build n-gram and phrase frequency maps from sentences (parallel). Fills ngram_freq and phrase_freq in place.
fn build_ngram_and_phrase_freq(
    sentences: &[String],
    ngram_freq: &DashMap<String, usize>,
    phrase_freq: &DashMap<String, usize>,
    config: &Config,
) {
    sentences
        .par_iter_adaptive(config)
        .enumerate()
        .for_each(|(idx, sentence)| {
            if idx > 0 && idx % 10_000 == 0 {
                debug!("Processed {} sentences for n-grams", idx);
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

/// Group sentences by structural pattern (parallel). Fills template_groups: pattern → list of sentences.
fn group_sentences_by_pattern(
    sentences: &[String],
    sentence_tokens: &[Vec<String>],
    template_groups: &DashMap<String, Vec<String>>,
    pivot_patterns: &DashMap<String, usize>,
    frequent_ngrams: &[(String, usize)],
    frequent_phrases: &[(String, usize)],
    config: &Config,
) {
    sentences
        .par_iter_adaptive(config)
        .zip(sentence_tokens.par_iter_adaptive(config))
        .for_each(|(sentence, tokens)| {
            let token_refs: Vec<&str> = tokens.iter().map(|s| s.as_str()).collect();
            let pattern = build_enriched_structural_pattern(
                &token_refs,
                pivot_patterns,
                frequent_ngrams,
                frequent_phrases,
                config,
            )
            .unwrap_or_else(|| {
                build_text_pattern(&token_refs, frequent_ngrams, frequent_phrases, config)
            });

            template_groups
                .entry(pattern)
                .or_default()
                .push(sentence.to_string());
        });
    debug!(
        "Template grouping: {} sentences in {} pattern groups (each pattern → list of sentences)",
        sentences.len(),
        template_groups.len()
    );
}

/// Convert template_groups (pattern → sentences) into Vec<Template>. Keeps only groups with
/// count >= min_template_count; skips patterns longer than avg_sentence_length * 2; adds examples and entropy.
fn build_templates_from_groups(
    template_groups: &DashMap<String, Vec<String>>,
    min_template_count: usize,
    avg_sentence_length: usize,
    frequent_ngrams: &[(String, usize)],
    frequent_phrases: &[(String, usize)],
    config: &Config,
) -> Vec<Template> {
    let templates: Vec<Template> = template_groups
        .iter()
        .filter(|entry| entry.value().len() >= min_template_count)
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

            for sentence in matching_sentences.iter().take(sample_size) {
                let tokens: Vec<&str> = sentence.split_whitespace().collect();
                extract_text_examples(
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

            let entropy = if matching_sentences.len() >= config.min_entropy_sample_size {
                calculate_template_entropy(&examples, matching_sentences.len(), config)
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
        min_template_count
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
    let pivot_patterns = extract_pivot_points(&original_sentences, config);

    // SVO analysis: run on unfiltered sentences + pivot_patterns (right after pivot extraction)
    let svo_analysis = Some(analyze_svo_structure(
        &original_sentences,
        &pivot_patterns,
        config,
    ));

    // Filter before n-grams: keep only sentence-like segments (min words, length, alphanumeric)
    filter_sentences_before_ngrams(&mut sentences, config);

    let total_sentences_after_filter = sentences.len();

    if total_sentences_after_filter == 0 {
        return Ok(crate::parsers::traits::empty_mining_result(stats));
    }

    let ngram_freq: DashMap<String, usize> = DashMap::new();
    let phrase_freq: DashMap<String, usize> = DashMap::new();

    build_ngram_and_phrase_freq(sentences.as_slice(), &ngram_freq, &phrase_freq, config);

    // Find frequent n-grams and phrases (appear in at least threshold% of sentences)
    let threshold = (total_sentences_after_filter as f64 * config.text_threshold) as usize;
    let (frequent_ngrams, frequent_phrases) = filter_frequent_ngrams_and_phrases(
        &ngram_freq,
        &phrase_freq,
        threshold,
        total_sentences_after_filter,
        config,
    );

    // Pre-compute tokens for each sentence
    let sentence_tokens = precompute_sentence_tokens(sentences.as_slice(), config);

    let template_groups: DashMap<String, Vec<String>> = DashMap::new();

    group_sentences_by_pattern(
        sentences.as_slice(),
        &sentence_tokens,
        &template_groups,
        &pivot_patterns,
        &frequent_ngrams,
        &frequent_phrases,
        config,
    );

    // Convert groups to Template structs
    // Only keep templates that appear multiple times (better compression)
    let min_template_count =
        (total_sentences_after_filter as f64 * config.text_threshold).max(2.0) as usize;

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

    let templates = build_templates_from_groups(
        &template_groups,
        min_template_count,
        avg_sentence_length,
        &frequent_ngrams,
        &frequent_phrases,
        config,
    );

    // Calculate writing footprint metrics
    let mut writing_footprint =
        calculate_writing_footprint(&sentences, &templates, content, config);

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

/// Build structural pattern (pivot-based) enriched with n-grams for writing footprint
/// Language-agnostic: finds structural pivot points (words with high variation after them)
/// Combines pivot structure with frequent n-grams to create richer patterns
fn build_enriched_structural_pattern(
    tokens: &[&str],
    pivot_patterns: &DashMap<String, usize>,
    frequent_ngrams: &[(String, usize)],
    _frequent_phrases: &[(String, usize)],
    config: &Config,
) -> Option<String> {
    if tokens.is_empty() {
        return None;
    }

    // Find pivot point (word at position that appears frequently in that position)
    // Pivot points are structural elements that appear consistently in similar positions
    let mut best_pivot: Option<(usize, &str)> = None;
    let mut best_score = 0;

    for (pos, token) in tokens.iter().enumerate() {
        // Check if this word at this position is a frequent pivot
        let pattern_key = format!("P_{}_{}", pos, token);
        if let Some(count) = pivot_patterns.get(&pattern_key) {
            let score = *count.value();
            if score > best_score {
                best_score = score;
                best_pivot = Some((pos, token));
            }
        }
    }

    // If we found a good pivot, create enriched structural pattern
    if let Some((p_pos, p_word)) = best_pivot {
        let threshold = (tokens.len() as f64 * config.text_threshold) as usize;

        if best_score >= threshold {
            let mut pattern_parts = Vec::new();

            // Before pivot: prefix area - try to match n-grams first
            if p_pos > 0 {
                let prefix_tokens = &tokens[0..p_pos];

                // Try to match frequent n-grams in prefix
                let mut matched_ngram = false;
                for n in (config.min_ngram_size..=config.max_ngram_size.min(p_pos)).rev() {
                    if prefix_tokens.len() >= n {
                        let candidate = prefix_tokens[prefix_tokens.len() - n..].join(" ");
                        if frequent_ngrams.iter().any(|(ngram, _)| ngram == &candidate) {
                            // Found n-gram at end of prefix
                            if prefix_tokens.len() > n {
                                pattern_parts.push("[PREFIX]".to_string());
                            }
                            pattern_parts.push(candidate);
                            matched_ngram = true;
                            break;
                        }
                    }
                }

                if !matched_ngram {
                    if p_pos <= config.short_prefix_threshold {
                        // Short prefix - include it
                        pattern_parts.push(prefix_tokens.join(" "));
                    } else {
                        // Long prefix - use placeholder
                        pattern_parts.push("[PREFIX]".to_string());
                    }
                }
            }

            // Pivot word (structural element)
            pattern_parts.push(p_word.to_string());

            // After pivot: suffix area - try to match n-grams first
            if p_pos + 1 < tokens.len() {
                let suffix_tokens = &tokens[p_pos + 1..];
                let remaining = suffix_tokens.len();

                // Try to match frequent n-grams in suffix
                let mut matched_ngram = false;
                for n in (config.min_ngram_size..=config.max_ngram_size.min(remaining)).rev() {
                    if suffix_tokens.len() >= n {
                        let candidate = suffix_tokens[0..n].join(" ");
                        if frequent_ngrams.iter().any(|(ngram, _)| ngram == &candidate) {
                            // Found n-gram at start of suffix
                            pattern_parts.push(candidate);
                            if suffix_tokens.len() > n {
                                pattern_parts.push("[SUFFIX]".to_string());
                            }
                            matched_ngram = true;
                            break;
                        }
                    }
                }

                if !matched_ngram {
                    if remaining <= config.short_prefix_threshold {
                        // Short suffix - include it
                        pattern_parts.push(suffix_tokens.join(" "));
                    } else {
                        // Long suffix - use placeholder
                        pattern_parts.push("[SUFFIX]".to_string());
                    }
                }
            }

            return Some(pattern_parts.join(" "));
        }
    }

    None
}

/// Build pattern string for text using n-gram matching
fn build_text_pattern(
    tokens: &[&str],
    frequent_ngrams: &[(String, usize)],
    frequent_phrases: &[(String, usize)],
    config: &crate::engine::config::Config,
) -> String {
    if tokens.is_empty() {
        return String::new();
    }

    let mut pattern_parts = Vec::new();
    let mut i = 0;

    while i < tokens.len() {
        let mut matched = false;

        // Try to match longest n-grams first
        for n in (config.min_ngram_size..=config.max_ngram_size).rev() {
            if i + n <= tokens.len() {
                let candidate = tokens[i..i + n].join(" ");

                // Check if this n-gram is frequent
                if frequent_ngrams.iter().any(|(ngram, _)| ngram == &candidate) {
                    pattern_parts.push(candidate);
                    i += n;
                    matched = true;
                    break;
                }
            }
        }

        if !matched {
            // Check for phrase patterns
            if i + config.min_phrase_length <= tokens.len() {
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
        }

        if !matched {
            // Single token - mark as dynamic (zero-padded for proper sorting)
            pattern_parts.push(format_placeholder_bracketed_typed(PlaceholderType::Word, i));
            i += 1;
        }
    }

    pattern_parts.join(" ")
}

/// Extract examples for text templates
fn extract_text_examples(
    tokens: &[&str],
    frequent_ngrams: &[(String, usize)],
    frequent_phrases: &[(String, usize)],
    examples: &mut BTreeMap<String, Vec<String>>,
    config: &crate::engine::config::Config,
) {
    let mut i = 0;
    let mut placeholder_idx = 0;

    while i < tokens.len() {
        let mut matched = false;

        // Try to match n-grams
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
            // This is a dynamic word (zero-padded for proper sorting)
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
