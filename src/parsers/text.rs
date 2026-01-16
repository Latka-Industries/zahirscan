//! Text file template extraction using sentence-level analysis and n-gram/phrase-based patterns

use crate::config::Config;
use crate::parsers::ParseResult;
use crate::parsers::traits::{
    DefaultSentenceAnalyzer, SentenceAnalyzer, WorkComplexity, optimal_chunk_size,
};
use crate::parsers::writing_analysis::{
    analyze_svo_structure, calculate_template_entropy, calculate_writing_footprint,
    extract_pivot_points,
};
use crate::results::{MiningResult, Template};
use anyhow::Result;
use dashmap::DashMap;
use log::debug;
use rayon::prelude::*;
use std::collections::BTreeMap;

/// Extract templates from text files using sentence-level analysis
/// For long-form text, works at sentence level rather than line-by-line
pub fn extract_text_templates(
    content: &str,
    stats: &ParseResult,
    config: &Config,
) -> Result<MiningResult> {
    if content.trim().is_empty() {
        return Ok(crate::parsers::traits::empty_mining_result(stats));
    }

    // Normalize content: handle indentation, collapse whitespace
    // Join non-empty lines with space to preserve sentence structure
    let normalized_content: String = content
        .lines()
        .map(|line| line.trim())
        .filter(|line| {
            // Skip empty lines and lines that are only punctuation/whitespace
            !line.is_empty() && line.chars().any(|c| c.is_alphanumeric())
        })
        .collect::<Vec<_>>()
        .join(" ");

    // Extract sentences from normalized content
    let mut sentences = DefaultSentenceAnalyzer::extract_sentences(&normalized_content);

    // Keep original sentences for SVO analysis (before filtering)
    // Limit to avoid excessive memory usage on very large files
    let max_original_sentences = 100_000;
    let original_sentences: Vec<String> = if sentences.len() > max_original_sentences {
        sentences[..max_original_sentences].to_vec()
    } else {
        sentences.clone()
    };

    // Filter out non-sentences using general criteria
    sentences.retain(|s| {
        let trimmed = s.trim();
        let word_count = trimmed.split_whitespace().count();

        // Must have minimum words to be considered a sentence
        word_count >= config.min_sentence_words
            // Must have alphanumeric content (not just punctuation/symbols)
            && trimmed.chars().any(|c| c.is_alphanumeric())
            // Must meet minimum length requirement (allows short but meaningful sentences)
            && (trimmed.len() >= config.min_sentence_length
                || word_count >= config.min_sentence_words_alt)
    });

    let total_sentences = sentences.len();

    if total_sentences == 0 {
        return Ok(crate::parsers::traits::empty_mining_result(stats));
    }

    // Extract n-grams and phrases from sentences
    debug!(
        "Starting n-gram extraction from {} sentences",
        sentences.len()
    );
    let ngram_freq: DashMap<String, usize> = DashMap::new();
    let phrase_freq: DashMap<String, usize> = DashMap::new();

    // Process sentences in parallel to build n-gram frequency maps
    debug!(
        "Starting n-gram extraction from {} sentences",
        sentences.len()
    );
    let chunk_size = optimal_chunk_size(
        sentences.len(),
        config.max_workers,
        WorkComplexity::Moderate,
    );
    sentences
        .par_iter()
        .with_min_len(chunk_size)
        .enumerate()
        .for_each(|(idx, sentence)| {
            if idx > 0 && idx % 10_000 == 0 {
                debug!("Processed {} sentences for n-grams", idx);
            }

            let tokens: Vec<&str> = sentence.split_whitespace().collect();

            // Extract n-grams (min_ngram_size to max_ngram_size)
            for n in config.min_ngram_size..=config.max_ngram_size {
                for window in tokens.windows(n) {
                    let ngram = window.join(" ");
                    ngram_freq.entry(ngram).and_modify(|c| *c += 1).or_insert(1);
                }
            }

            // Extract phrases (sequences of min_phrase_length+ tokens that appear frequently)
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
        "Completed n-gram extraction. Unique n-grams: {}, Unique phrases: {}",
        ngram_freq.len(),
        phrase_freq.len()
    );

    // Find frequent n-grams (appear in at least threshold% of sentences)
    let threshold = (total_sentences as f64 * config.text_threshold) as usize;
    debug!(
        "Filtering n-grams with threshold: {} ({}% of {} sentences)",
        threshold,
        config.text_threshold * 100.0,
        total_sentences
    );

    // Collect frequent n-grams (DashMap doesn't support par_iter, but we can parallelize the collection)
    let frequent_ngrams: Vec<(String, usize)> = ngram_freq
        .iter()
        .filter(|entry| *entry.value() >= threshold)
        .map(|entry| (entry.key().clone(), *entry.value()))
        .collect();

    // Find frequent phrases
    let frequent_phrases: Vec<(String, usize)> = phrase_freq
        .iter()
        .filter(|entry| *entry.value() >= threshold)
        .map(|entry| (entry.key().clone(), *entry.value()))
        .collect();

    // Extract structural pivot points (language-agnostic: words with high variation after them)
    // Use original sentences (before filtering) for pivot detection to ensure position accuracy
    // This ensures pivot positions match when we analyze SVO structure
    let pivot_patterns = extract_pivot_points(&original_sentences, config);

    // Group sentences by structural patterns enriched with n-grams
    debug!(
        "Starting template grouping for {} sentences",
        sentences.len()
    );
    // Pre-compute tokens for sentences to avoid repeated splitting
    debug!("Pre-computing tokens for {} sentences", sentences.len());
    let sentence_tokens: Vec<Vec<&str>> = sentences
        .iter()
        .enumerate()
        .map(|(_idx, s)| s.split_whitespace().collect::<Vec<&str>>())
        .collect();

    let template_groups: DashMap<String, Vec<String>> = DashMap::new();

    // Process in parallel for better performance
    let chunk_size = optimal_chunk_size(
        sentences.len(),
        config.max_workers,
        WorkComplexity::Moderate,
    );
    sentences
        .par_iter()
        .with_min_len(chunk_size)
        .zip(sentence_tokens.par_iter().with_min_len(chunk_size))
        .for_each(|(sentence, tokens)| {
            // First try structural pattern (pivot-based) enriched with n-grams, then fall back
            let pattern = build_enriched_structural_pattern(
                tokens,
                &pivot_patterns,
                &frequent_ngrams,
                &frequent_phrases,
                config,
            )
            .unwrap_or_else(|| {
                build_text_pattern(tokens, &frequent_ngrams, &frequent_phrases, config)
            });

            template_groups
                .entry(pattern)
                .or_default()
                .push((*sentence).clone());
        });

    // Convert groups to Template structs
    // Only keep templates that appear multiple times (better compression)
    let min_template_count = (total_sentences as f64 * config.text_threshold).max(2.0) as usize;

    // Calculate average sentence length for compression check
    let avg_sentence_length: usize = if !sentences.is_empty() {
        sentences
            .iter()
            .map(|s| s.split_whitespace().count())
            .sum::<usize>()
            / sentences.len()
    } else {
        0
    };

    let templates: Vec<Template> = template_groups
        .iter()
        .filter(|entry| {
            // Only frequent patterns
            entry.value().len() >= min_template_count
        })
        .filter_map(|entry| {
            let pattern = entry.key().clone();
            let matching_sentences = entry.value();

            // Skip if pattern is longer than average sentence (not compressing well)
            let pattern_word_count = pattern.split_whitespace().count();
            if pattern_word_count > avg_sentence_length * 2 {
                return None;
            }

            // Extract examples for placeholders with entropy calculation
            let mut examples: BTreeMap<String, Vec<String>> = BTreeMap::new();

            // Limit to fewer examples for better compression
            let example_limit =
                (config.max_examples_per_placeholder / 2).max(config.min_examples_per_placeholder);
            let sample_size = example_limit.min(config.max_examples_for_entropy);

            // Pre-allocate capacity for examples to reduce reallocations
            for sentence in matching_sentences.iter().take(sample_size) {
                let tokens: Vec<&str> = sentence.split_whitespace().collect();
                extract_text_examples(
                    &tokens,
                    &frequent_ngrams,
                    &frequent_phrases,
                    &mut examples,
                    config,
                );
            }

            // Limit each example placeholder to fewer items
            for examples_list in examples.values_mut() {
                if examples_list.len() > example_limit {
                    examples_list.truncate(example_limit);
                }
            }

            // Calculate entropy for this template (variation metric)
            // Only calculate if we have enough examples for meaningful entropy
            let entropy = if matching_sentences.len() >= config.min_entropy_sample_size {
                calculate_template_entropy(&examples, matching_sentences.len(), config)
            } else {
                0.0 // Skip entropy for small templates
            };

            // Add entropy as metadata in pattern (for writing footprint)
            // Only show if meaningful (not 0.0 and not 1.0 from small samples)
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

    // Calculate writing footprint metrics
    let mut writing_footprint =
        calculate_writing_footprint(&sentences, &templates, content, config);

    // Analyze SVO structure from templates (language-agnostic)
    // Use original sentences for SVO analysis to match pivot pattern positions
    let svo_analysis =
        analyze_svo_structure(&templates, &original_sentences, &pivot_patterns, config);
    writing_footprint.svo_analysis = Some(svo_analysis);

    // Build MiningResult using shared utility, including writing footprint in compression calculation
    let result = crate::parsers::traits::build_mining_result_with_footprint(
        templates,
        total_sentences,
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
    config: &crate::config::Config,
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
            pattern_parts.push(format!("[WORD_{:02}]", i));
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
    config: &crate::config::Config,
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
            let placeholder = format!("WORD_{:02}", placeholder_idx);
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
