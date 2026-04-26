//! Writing footprint and SVO analysis utilities.
//! Shared analysis functions for text and markdown parsers.

use dashmap::DashMap;
use log::debug;
use rayon::prelude::*;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::config::RuntimeConfig;
use crate::parsers::traits::AdaptiveParallel;
use crate::results::{PunctuationMetrics, SVOAnalysis, Template, WritingFootprint};

/// Calculate entropy (variation) for a template
/// Higher entropy = more variation in word choice (more creative/diverse writing)
/// Lower entropy = less variation (more repetitive/structured writing)
/// Uses frequency-weighted diversity metric
#[must_use]
pub fn calculate_template_entropy(
    examples: &BTreeMap<String, Vec<String>>,
    total_count: usize,
    config: &RuntimeConfig,
) -> f64 {
    if examples.is_empty() || total_count == 0 {
        return 0.0;
    }

    let mut total_entropy = 0.0;
    let mut placeholder_count = 0;

    for example_values in examples.values() {
        if example_values.is_empty() {
            continue;
        }

        // Count frequency of each unique value
        let mut value_freq: HashMap<&String, usize> = HashMap::new();
        for value in example_values {
            *value_freq.entry(value).or_insert(0) += 1;
        }

        let unique_count = value_freq.len();
        let sample_size = example_values.len();

        if sample_size == 0 {
            continue;
        }

        // Calculate entropy based on actual distribution
        let mut entropy = 0.0;

        if unique_count == 1 {
            // Only one unique value = no variation = 0 entropy
            entropy = 0.0;
        } else {
            // Calculate Shannon entropy: -Σ(p * log2(p))
            for count in value_freq.values() {
                let probability = *count as f64 / sample_size as f64;
                if probability > 0.0 {
                    entropy -= probability * probability.log2();
                }
            }

            // Normalize: entropy ranges from 0 to log2(unique_count)
            let max_entropy = (unique_count as f64).log2();

            if max_entropy > 0.0 {
                // Normalize to 0-1 range
                entropy /= max_entropy;

                // For small samples, adjust entropy based on sample size
                // Smaller samples are less reliable, so we scale entropy down
                // This prevents all templates from showing the same entropy
                let sample_reliability = if sample_size < config.entropy_small_sample_threshold {
                    // Scale reliability based on sample size
                    // Sample of 3 = 0.6 reliability, sample of 5 = 0.8 reliability
                    (sample_size as f64 / config.entropy_small_sample_threshold as f64).min(1.0)
                } else {
                    1.0
                };

                // Apply reliability scaling: smaller samples get lower entropy scores
                // This creates variation instead of all showing 0.85
                entropy *= sample_reliability;
            }
        }

        total_entropy += entropy;
        placeholder_count += 1;
    }

    if placeholder_count > 0 {
        total_entropy / f64::from(placeholder_count)
    } else {
        0.0
    }
}

/// Extract pivot points (language-agnostic structural elements)
/// Pivot points are words that appear frequently at the same position across sentences
/// and have high variation in what follows them (indicating structural importance)
/// Returns empty `DashMap` if there are too many sentences to process efficiently
#[must_use]
pub fn extract_pivot_points(
    sentences: &[String],
    config: &RuntimeConfig,
) -> DashMap<String, usize> {
    if sentences.is_empty() {
        return DashMap::new();
    }

    let sentences_to_process = sentences;
    let pivot_freq: DashMap<String, usize> = DashMap::new();
    let position_word_freq: DashMap<(usize, String), usize> = DashMap::new();
    let word_position_variation_flat: DashMap<(usize, String, String), usize> = DashMap::new();

    sentences_to_process
        .par_iter_adaptive(config)
        .enumerate()
        .for_each(|(idx, sentence)| {
            if idx > 0 && idx % 10_000 == 0 {
                debug!("Processed {idx} sentences for pivot extraction");
            }
            let tokens: Vec<&str> = sentence.split_whitespace().collect();

            for (pos, token) in tokens.iter().enumerate() {
                let token_clean: String = token
                    .chars()
                    .filter(|c| c.is_alphanumeric())
                    .collect::<String>()
                    .to_lowercase();

                if !token_clean.is_empty() {
                    let key = (pos, token_clean.clone());
                    position_word_freq
                        .entry(key.clone())
                        .and_modify(|c| *c += 1)
                        .or_insert(1);

                    if pos + 1 < tokens.len() {
                        let next_token = tokens[pos + 1];
                        let next_clean: String = next_token
                            .chars()
                            .filter(|c| c.is_alphanumeric())
                            .collect::<String>()
                            .to_lowercase();

                        if !next_clean.is_empty() {
                            let variation_key = (pos, token_clean, next_clean);
                            word_position_variation_flat
                                .entry(variation_key)
                                .and_modify(|c| *c += 1)
                                .or_insert(1);
                        }
                    }
                }
            }
        });

    let total_sentences = sentences_to_process.len();
    let min_frequency = (total_sentences as f64 * config.text_threshold).max(2.0) as usize;

    let mut variation_counts_map: HashMap<(usize, String), HashSet<String>> = HashMap::new();
    for entry in &word_position_variation_flat {
        let (pos, word, next_word) = entry.key();
        let key = (*pos, word.clone());
        variation_counts_map
            .entry(key)
            .or_default()
            .insert(next_word.clone());
    }

    let variation_counts: HashMap<(usize, String), usize> = variation_counts_map
        .into_iter()
        .map(|(k, v)| (k, v.len()))
        .collect();

    for entry in &position_word_freq {
        let (pos, word) = entry.key();
        let count = entry.value();
        if *count >= min_frequency {
            let variation_score = variation_counts
                .get(&(*pos, word.clone()))
                .copied()
                .unwrap_or(0);

            if variation_score >= config.min_pivot_variation
                && variation_score <= config.max_pivot_variation
            {
                let pattern_key = format!("P_{pos}_{word}");
                pivot_freq
                    .entry(pattern_key)
                    .and_modify(|c| *c += *count)
                    .or_insert(*count);
            }
        }
    }
    debug!(
        "Extracted {} pivot points from {} sentences",
        pivot_freq.len(),
        total_sentences
    );

    pivot_freq
}

/// Calculate writing footprint metrics for text/markdown analysis
#[must_use]
pub fn calculate_writing_footprint(
    sentences: &[String],
    templates: &[Template],
    content: &str,
    config: &RuntimeConfig,
) -> WritingFootprint {
    debug!(
        "Calculating vocabulary richness (content length: {})",
        content.len()
    );
    let all_words: Vec<&str> = content
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| !w.is_empty())
        .collect();
    let unique_words: HashSet<&str> = all_words.iter().copied().collect();
    let vocabulary_richness = if all_words.is_empty() {
        0.0
    } else {
        unique_words.len() as f64 / all_words.len() as f64
    };

    debug!(
        "Calculating average sentence length from {} sentences",
        sentences.len()
    );
    let total_words: usize = sentences
        .par_iter_adaptive(config)
        .map(|s| s.split_whitespace().count())
        .sum();
    let avg_sentence_length = if sentences.is_empty() {
        0.0
    } else {
        total_words as f64 / sentences.len() as f64
    };

    let mut period_count = 0;
    let mut question_count = 0;
    let mut exclamation_count = 0;
    let mut dialogue_count = 0;
    let mut total_commas = 0;

    for sentence in sentences {
        let trimmed = sentence.trim();
        if trimmed.ends_with('.') {
            period_count += 1;
        } else if trimmed.ends_with('?') {
            question_count += 1;
        } else if trimmed.ends_with('!') {
            exclamation_count += 1;
        }

        if sentence.contains('"') || sentence.contains('\'') {
            dialogue_count += 1;
        }

        total_commas += sentence.matches(',').count();
    }

    let total_sentences = sentences.len();
    let total_sentences_f64 = total_sentences as f64;
    let percent = |count: usize| {
        if total_sentences > 0 {
            count as f64 / total_sentences_f64 * 100.0
        } else {
            0.0
        }
    };
    let avg = |count: usize| {
        if total_sentences > 0 {
            count as f64 / total_sentences_f64
        } else {
            0.0
        }
    };

    let punctuation = PunctuationMetrics {
        period_percent: percent(period_count),
        question_percent: percent(question_count),
        exclamation_percent: percent(exclamation_count),
        dialogue_percent: percent(dialogue_count),
        avg_commas_per_sentence: avg(total_commas),
    };

    let template_diversity = templates.len();

    // Average entropy across templates (extract from pattern strings)
    let mut total_entropy = 0.0;
    let mut entropy_count = 0;
    for template in templates {
        if let Some(entropy_start) = template.pattern.find("[entropy=") {
            let entropy_str = &template.pattern[entropy_start + 9..];
            if let Some(entropy_end) = entropy_str.find(']')
                && let Ok(entropy) = entropy_str[..entropy_end].parse::<f64>()
            {
                total_entropy += entropy;
                entropy_count += 1;
            }
        }
    }
    let avg_entropy = if entropy_count > 0 {
        total_entropy / f64::from(entropy_count)
    } else {
        0.0
    };

    WritingFootprint {
        vocabulary_richness,
        avg_sentence_length,
        punctuation,
        template_diversity,
        avg_entropy,
        svo_analysis: None, // Set by caller
    }
}

fn normalize_token(token: &str) -> String {
    token
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_lowercase()
}

fn find_pivot_position(
    tokens: &[&str],
    pivot_patterns: &DashMap<String, usize>,
) -> Option<(usize, String)> {
    tokens.iter().enumerate().find_map(|(pos, token)| {
        let token_clean = normalize_token(token);
        if token_clean.is_empty() {
            return None;
        }
        let pattern_key = format!("P_{pos}_{token_clean}");
        if pivot_patterns.contains_key(&pattern_key) {
            Some((pos, token_clean))
        } else {
            None
        }
    })
}

/// Uses pivot points to infer subject-verb-object relationships.
#[must_use]
pub fn analyze_svo_structure(
    sentences: &[String],
    pivot_patterns: &DashMap<String, usize>,
    config: &RuntimeConfig,
) -> SVOAnalysis {
    let sentences_with_pivots = AtomicUsize::new(0);
    let total_subject_length = AtomicUsize::new(0);
    let total_object_length = AtomicUsize::new(0);
    let subject_count = AtomicUsize::new(0);
    let object_count = AtomicUsize::new(0);
    let pivot_words: DashMap<String, usize> = DashMap::new();

    sentences.par_iter_adaptive(config).for_each(|sentence| {
        let tokens: Vec<&str> = sentence.split_whitespace().collect();

        if let Some((pivot_pos, pivot_word)) = find_pivot_position(&tokens, pivot_patterns) {
            sentences_with_pivots.fetch_add(1, Ordering::Relaxed);

            pivot_words
                .entry(pivot_word.clone())
                .and_modify(|c| *c += 1)
                .or_insert(1);

            if pivot_pos > 0 {
                total_subject_length.fetch_add(pivot_pos, Ordering::Relaxed);
                subject_count.fetch_add(1, Ordering::Relaxed);
            }

            let words_after_pivot = tokens.len().saturating_sub(pivot_pos + 1);
            if words_after_pivot > 0 {
                total_object_length.fetch_add(words_after_pivot, Ordering::Relaxed);
                object_count.fetch_add(1, Ordering::Relaxed);
            }
        }
    });

    let sentences_with_pivots = sentences_with_pivots.load(Ordering::Relaxed);
    let total_subject_length = total_subject_length.load(Ordering::Relaxed);
    let total_object_length = total_object_length.load(Ordering::Relaxed);
    let subject_count = subject_count.load(Ordering::Relaxed);
    let object_count = object_count.load(Ordering::Relaxed);
    let pivot_words: HashMap<String, usize> = pivot_words
        .iter()
        .map(|e| (e.key().clone(), *e.value()))
        .collect();

    let total_sentences = sentences.len();
    let svo_structure_percent = if total_sentences > 0 {
        sentences_with_pivots as f64 / total_sentences as f64 * 100.0
    } else {
        0.0
    };

    let avg_subject_length = if subject_count > 0 {
        total_subject_length as f64 / subject_count as f64
    } else {
        0.0
    };

    let avg_object_length = if object_count > 0 {
        total_object_length as f64 / object_count as f64
    } else {
        0.0
    };

    let mut pivot_vec: Vec<(String, usize)> = pivot_words.into_iter().collect();
    pivot_vec.sort_by_key(|item| std::cmp::Reverse(item.1));
    let common_pivots: Vec<String> = pivot_vec
        .into_iter()
        .take(config.max_common_pivots)
        .map(|(word, _)| word)
        .collect();

    debug!(
        "SVO analysis: {:.2}% of sentences with {} common pivots and avg subject length of {:.2} and avg object length of {:.2}",
        svo_structure_percent,
        common_pivots.len(),
        avg_subject_length,
        avg_object_length
    );

    SVOAnalysis {
        svo_structure_percent,
        avg_subject_length,
        avg_object_length,
        common_pivots,
    }
}
