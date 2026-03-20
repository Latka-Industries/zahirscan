//! Analysis utilities for text: sentence extraction, n-grams, writing footprint, SVO, pivot extraction.
//! Shape/coarse fallback (by word count + end type) is in `plain_text` when exact-pattern yields no templates.

pub mod ngrams;
pub mod sentences;
pub mod shape;
pub mod writing;

pub use ngrams::{
    FrequentEntries, build_enriched_structural_pattern, build_ngram_and_phrase_freq,
    build_text_pattern, collect_frequent_entries, extract_text_examples,
    filter_frequent_ngrams_and_phrases,
};
pub use sentences::{DefaultSentenceAnalyzer, SentenceAnalyzer, SentenceEmphasis, SentenceStats};
pub use shape::{
    EndType, SentenceShape, extract_examples_by_position, group_sentences_by_shape,
    pattern_for_shape, pattern_is_all_placeholders, sentence_shape,
};
pub use writing::{
    analyze_svo_structure, calculate_template_entropy, calculate_writing_footprint,
    extract_pivot_points,
};

/// Sentence-ending punctuation characters (used for splitting).
pub struct Punctuation;

impl Punctuation {
    pub const PERIOD: char = '.';
    pub const EXCLAMATION: char = '!';
    pub const QUESTION: char = '?';
    pub const SPACE: char = ' ';
    pub const NEWLINE: char = '\n';
    pub const TAB: char = '\t';
    pub const DQUOTE: char = '"';
    pub const SQUOTE: char = '\'';
    pub const COMMA: char = ',';
    pub const OPEN_PAREN: char = '(';
    pub const CLOSE_PAREN: char = ')';

    /// Returns true if `ch` is a sentence-ending punctuation character.
    #[inline]
    #[must_use]
    pub fn is_end_punct(ch: char) -> bool {
        ch == Self::PERIOD || ch == Self::EXCLAMATION || ch == Self::QUESTION
    }

    /// Returns true if `ch` is a quote character.
    #[inline]
    #[must_use]
    pub fn is_quote(ch: char) -> bool {
        ch == Self::DQUOTE || ch == Self::SQUOTE
    }

    /// Returns true if `ch` is an opening or closing parenthesis.
    #[inline]
    #[must_use]
    pub fn is_paren(ch: char) -> bool {
        ch == Self::OPEN_PAREN || ch == Self::CLOSE_PAREN
    }
}
