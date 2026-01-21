//! Common utilities for parsers

use crate::config::Config;
use crate::parsers::ParseResult;
use crate::results::{MiningResult, Template};

/// Create an empty MiningResult (no templates found)
/// This is shared across all parsers for empty content cases
pub fn empty_mining_result(stats: &ParseResult) -> MiningResult {
    MiningResult {
        templates: vec![],
        original_tokens: stats.token_count,
        compressed_tokens: 0,
        token_reduction_percent: 0.0,
        writing_footprint: None,
    }
}

/// Build MiningResult from templates (sorts and calculates compression)
/// This is shared across all parsers
pub fn build_mining_result(
    templates: Vec<Template>,
    total_items: usize,
    stats: &ParseResult,
    config: &Config,
) -> MiningResult {
    build_mining_result_with_footprint(templates, total_items, stats, config, None)
}

/// Build MiningResult from templates including writing footprint in compression calculation
pub fn build_mining_result_with_footprint(
    templates: Vec<Template>,
    total_items: usize,
    stats: &ParseResult,
    config: &Config,
    writing_footprint: Option<&crate::results::WritingFootprint>,
) -> MiningResult {
    // Sort by count (most common first)
    let mut sorted_templates = templates;
    sort_templates_by_count(&mut sorted_templates);

    // Calculate compression including writing footprint tokens
    let (original_tokens, compressed_tokens, token_reduction_percent) =
        calculate_compression_with_footprint(
            &sorted_templates,
            total_items,
            stats,
            config,
            writing_footprint,
        );

    MiningResult {
        templates: sorted_templates,
        original_tokens,
        compressed_tokens,
        token_reduction_percent,
        writing_footprint: writing_footprint.cloned(),
    }
}

/// Calculate compression metrics from templates
/// Returns (original_tokens, compressed_tokens, token_reduction_percent)
pub fn calculate_compression(
    templates: &[Template],
    total_items: usize,
    stats: &ParseResult,
    config: &Config,
) -> (usize, usize, f64) {
    calculate_compression_with_footprint(templates, total_items, stats, config, None)
}

/// Calculate compression metrics from templates including writing footprint
/// Returns (original_tokens, compressed_tokens, token_reduction_percent)
pub fn calculate_compression_with_footprint(
    templates: &[Template],
    total_items: usize,
    stats: &ParseResult,
    config: &Config,
    writing_footprint: Option<&crate::results::WritingFootprint>,
) -> (usize, usize, f64) {
    let original_tokens = stats.token_count;
    use crate::parsers::estimate_compressed_tokens_with_footprint;
    let compressed_tokens = estimate_compressed_tokens_with_footprint(
        templates,
        total_items,
        config,
        writing_footprint,
    );
    let token_reduction_percent = if original_tokens > 0 {
        let reduction = original_tokens.saturating_sub(compressed_tokens);
        (reduction as f64 / original_tokens as f64) * 100.0
    } else {
        0.0
    };
    (original_tokens, compressed_tokens, token_reduction_percent)
}

/// Sort templates by count (most common first)
/// This is shared across all parsers
pub fn sort_templates_by_count(templates: &mut [Template]) {
    templates.sort_by(|a, b| b.count.cmp(&a.count));
}

// ============================================================================
// Parallel Processing Utilities
// ============================================================================

/// Calculate optimal chunk size for parallel processing based on collection size and target number of chunks.
///
/// Creates chunks that approximate `target_chunks` chunks (neat multiple of workers).
/// Uses integer division, so the last chunk may be smaller if there's a remainder.
/// This ensures neat multiples of workers for optimal load balancing.
/// The adaptive chunking calculation already accounts for work complexity, so we respect the target.
pub fn optimal_chunk_size(collection_size: usize, target_chunks: usize) -> usize {
    if collection_size < 1000 || target_chunks == 0 {
        // Small collections or no chunks: no chunking needed
        return 1;
    }

    // Calculate chunk size to approximate target_chunks chunks
    // Integer division: remainder goes to the last chunk (which is fine)
    // Example: 10,000 items / 26 chunks = 384 per chunk, last chunk gets remainder (16 items)
    let chunk_size = collection_size / target_chunks.max(1);

    // Ensure minimum chunk size of 1 (shouldn't happen, but safety check)
    chunk_size.max(1)
}

// ============================================================================
// Sentence Analysis Trait and Utilities
// ============================================================================

/// Trait for sentence-level text analysis
/// Used by markdown and text parsers for analyzing sentence structure and patterns
pub trait SentenceAnalyzer {
    /// Extract sentences from a text block
    fn extract_sentences(text: &str) -> Vec<String>;

    /// Analyze sentence structure (length, word count, punctuation patterns)
    fn analyze_sentence_structure(sentence: &str) -> SentenceStats;

    /// Extract common sentence patterns from a collection of sentences
    fn extract_sentence_patterns(sentences: &[String], config: &Config) -> Vec<String>;
}

/// Statistics about a sentence's structure
#[derive(Debug, Clone)]
pub struct SentenceStats {
    pub word_count: usize,
    pub char_count: usize,
    pub has_quotes: bool,
    pub has_question: bool,
    pub has_exclamation: bool,
    pub ends_with_period: bool,
}

impl SentenceStats {
    pub fn new(sentence: &str) -> Self {
        let words: Vec<&str> = sentence.split_whitespace().collect();
        Self {
            word_count: words.len(),
            char_count: sentence.chars().count(),
            has_quotes: sentence.contains('"') || sentence.contains('\''),
            has_question: sentence.contains('?'),
            has_exclamation: sentence.contains('!'),
            ends_with_period: sentence.trim_end().ends_with('.'),
        }
    }
}

/// Default implementation of SentenceAnalyzer
pub struct DefaultSentenceAnalyzer;

impl SentenceAnalyzer for DefaultSentenceAnalyzer {
    /// Extract sentences from text using heuristic-based splitting
    /// Splits on sentence-ending punctuation (. ! ?) but skips:
    /// - Abbreviations (single letter before punctuation)
    /// - Decimals (digit before punctuation)
    /// - Abbreviations (lowercase letter after punctuation)
    fn extract_sentences(text: &str) -> Vec<String> {
        if text.trim().is_empty() {
            return Vec::new();
        }

        let mut sentences = Vec::new();
        let mut current_sentence = String::new();
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let ch = chars[i];
            current_sentence.push(ch);

            // Check if this is sentence-ending punctuation
            if matches!(ch, '.' | '!' | '?') {
                // First, check heuristics that prevent splitting (abbreviations, decimals)
                let prev_char = if i > 0 {
                    chars.get(i - 1).copied()
                } else {
                    None
                };

                // Skip splitting if preceded by digit (decimal like "3.14")
                if let Some(p) = prev_char
                    && p.is_ascii_digit()
                {
                    i += 1;
                    continue;
                }

                // Skip splitting if preceded by single letter (abbreviation like "Dr.")
                if let Some(p) = prev_char
                    && p.is_alphabetic()
                    && (i == 1 || !chars.get(i - 2).is_some_and(|c| c.is_alphabetic()))
                {
                    // Single letter before - likely abbreviation
                    i += 1;
                    continue;
                }

                // Now check if we should split based on what follows
                let next_char_idx = i + 1;
                let next_char = chars.get(next_char_idx).copied();

                let should_split = match next_char {
                    // End of text - definitely split
                    None => true,
                    // Lowercase after punctuation - likely abbreviation, don't split
                    Some(c) if c.is_lowercase() => false,
                    // Whitespace - check what comes after whitespace
                    Some(' ') | Some('\n') | Some('\t') => {
                        // Look further ahead for the next non-whitespace character
                        let mut j = next_char_idx + 1;
                        while j < chars.len() && chars[j].is_whitespace() {
                            j += 1;
                        }
                        if j >= chars.len() {
                            true // End of text after whitespace
                        } else {
                            // Check if next non-whitespace is uppercase (likely new sentence)
                            chars[j].is_uppercase()
                        }
                    }
                    // Uppercase or other - likely sentence end
                    _ => true,
                };

                if should_split {
                    let sentence = current_sentence.trim().to_string();
                    if !sentence.is_empty() {
                        sentences.push(sentence);
                    }
                    current_sentence.clear();
                }
            }

            i += 1;
        }

        // Add remaining text as final sentence if any
        let remaining = current_sentence.trim().to_string();
        if !remaining.is_empty() {
            sentences.push(remaining);
        }

        sentences
    }

    /// Analyze sentence structure
    fn analyze_sentence_structure(sentence: &str) -> SentenceStats {
        SentenceStats::new(sentence)
    }

    /// Extract common sentence patterns (simplified patterns based on structure)
    fn extract_sentence_patterns(sentences: &[String], config: &Config) -> Vec<String> {
        use dashmap::DashMap;

        // Group sentences by structural pattern
        let pattern_freq: DashMap<String, usize> = DashMap::new();

        for sentence in sentences {
            let stats = Self::analyze_sentence_structure(sentence);
            let pattern = format!(
                "[SENTENCE:words={},quotes={},question={},exclamation={}]",
                stats.word_count, stats.has_quotes, stats.has_question, stats.has_exclamation
            );
            pattern_freq
                .entry(pattern)
                .and_modify(|c| *c += 1)
                .or_insert(1);
        }

        // Return patterns that appear frequently
        let threshold = (sentences.len() as f64 * config.text_threshold) as usize;
        pattern_freq
            .iter()
            .filter(|entry| *entry.value() >= threshold)
            .map(|entry| entry.key().clone())
            .collect()
    }
}
