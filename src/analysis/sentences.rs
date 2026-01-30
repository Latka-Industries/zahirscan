//! Sentence-level text analysis: extraction, structure, patterns.
//! Used by markdown and text parsers for sentence boundaries and structure.

use crate::analysis::Punctuation;
use crate::engine::config::Config;
use dashmap::DashMap;
use log::debug;

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
            has_quotes: sentence.contains(Punctuation::is_quote),
            has_question: sentence.contains(Punctuation::QUESTION),
            has_exclamation: sentence.contains(Punctuation::EXCLAMATION),
            ends_with_period: sentence.trim_end().ends_with(Punctuation::PERIOD),
        }
    }
}

/// Default implementation of SentenceAnalyzer
pub struct DefaultSentenceAnalyzer;

impl SentenceAnalyzer for DefaultSentenceAnalyzer {
    /// Extract sentences from content. Normalizes input (trim lines, drop empty/non-alphanumeric,
    /// join with space) then splits on sentence-ending punctuation (. ! ?) with heuristics to skip:
    /// - Abbreviations (single letter before punctuation)
    /// - Decimals (digit before punctuation)
    /// - Lowercase after punctuation (abbreviation)
    fn extract_sentences(text: &str) -> Vec<String> {
        let normalized = text
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty() && line.chars().any(|c| c.is_alphanumeric()))
            .collect::<Vec<_>>()
            .join(" ");

        if normalized.is_empty() {
            return Vec::new();
        }

        let mut sentences = Vec::new();
        let mut current_sentence = String::new();
        let chars: Vec<char> = normalized.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let ch = chars[i];
            current_sentence.push(ch);

            // Check if this is sentence-ending punctuation
            if Punctuation::is_end_punct(ch) {
                // Ellipsis: don't split on . when part of ... (two or more consecutive periods)
                if ch == Punctuation::PERIOD {
                    let next_is_period = chars.get(i + 1).copied() == Some(Punctuation::PERIOD);
                    let prev_is_period =
                        i > 0 && chars.get(i - 1).copied() == Some(Punctuation::PERIOD);
                    if next_is_period || prev_is_period {
                        i += 1;
                        continue;
                    }
                }

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

                // Don't split when immediately followed by ')' — we're inside a parenthetical (e.g. "(pinkies up!)")
                if next_char == Some(Punctuation::CLOSE_PAREN) {
                    i += 1;
                    continue;
                }

                let should_split = match next_char {
                    // End of text - definitely split
                    None => true,
                    // Lowercase after punctuation - likely abbreviation, don't split
                    Some(c) if c.is_lowercase() => false,
                    // Whitespace - check what comes after whitespace
                    Some(Punctuation::SPACE)
                    | Some(Punctuation::NEWLINE)
                    | Some(Punctuation::TAB) => {
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

        debug!("Extracted {} sentences from text", sentences.len());

        sentences
    }

    /// Analyze sentence structure
    fn analyze_sentence_structure(sentence: &str) -> SentenceStats {
        SentenceStats::new(sentence)
    }

    /// Extract common sentence patterns (simplified patterns based on structure)
    fn extract_sentence_patterns(sentences: &[String], config: &Config) -> Vec<String> {
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
