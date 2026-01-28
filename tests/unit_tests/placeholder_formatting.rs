//! Tests for placeholder formatting functions

use zahirscan::engine::tools::{
    PlaceholderType, format_placeholder, format_placeholder_bracketed,
    format_placeholder_bracketed_typed, format_placeholder_typed,
};

#[test]
fn test_format_placeholder_basic() {
    assert_eq!(format_placeholder("WORD", 0), "WORD_00");
    assert_eq!(format_placeholder("WORD", 1), "WORD_01");
    assert_eq!(format_placeholder("WORD", 10), "WORD_10");
    assert_eq!(format_placeholder("WORD", 99), "WORD_99");
}

#[test]
fn test_format_placeholder_large_index() {
    assert_eq!(format_placeholder("WORD", 100), "WORD_100");
    assert_eq!(format_placeholder("WORD", 999), "WORD_999");
    assert_eq!(format_placeholder("WORD", 1234), "WORD_1234");
}

#[test]
fn test_format_placeholder_typed() {
    assert_eq!(
        format_placeholder_typed(PlaceholderType::Word, 0),
        "WORD_00"
    );
    assert_eq!(
        format_placeholder_typed(PlaceholderType::Word, 5),
        "WORD_05"
    );
    assert_eq!(
        format_placeholder_typed(PlaceholderType::Position, 0),
        "POS_00"
    );
    assert_eq!(
        format_placeholder_typed(PlaceholderType::Position, 12),
        "POS_12"
    );
    assert_eq!(format_placeholder_typed(PlaceholderType::Col, 3), "col_03");
}

#[test]
fn test_format_placeholder_bracketed() {
    assert_eq!(format_placeholder_bracketed("WORD", 0), "[WORD_00]");
    assert_eq!(format_placeholder_bracketed("WORD", 1), "[WORD_01]");
    assert_eq!(format_placeholder_bracketed("POS", 5), "[POS_05]");
}

#[test]
fn test_format_placeholder_bracketed_typed() {
    assert_eq!(
        format_placeholder_bracketed_typed(PlaceholderType::Word, 0),
        "[WORD_00]"
    );
    assert_eq!(
        format_placeholder_bracketed_typed(PlaceholderType::Position, 10),
        "[POS_10]"
    );
    assert_eq!(
        format_placeholder_bracketed_typed(PlaceholderType::Col, 25),
        "[col_25]"
    );
}

#[test]
fn test_placeholder_type_as_str() {
    assert_eq!(PlaceholderType::Word.as_str(), "WORD");
    assert_eq!(PlaceholderType::Position.as_str(), "POS");
    assert_eq!(PlaceholderType::Col.as_str(), "col");
    assert_eq!(PlaceholderType::Pos.as_str(), "pos");
    assert_eq!(PlaceholderType::List.as_str(), "LIST");
    assert_eq!(PlaceholderType::CodeBlock.as_str(), "CODE_BLOCK");
}

#[test]
fn test_format_placeholder_zero_padding() {
    // Should always zero-pad to at least 2 digits
    assert_eq!(format_placeholder("TEST", 0), "TEST_00");
    assert_eq!(format_placeholder("TEST", 9), "TEST_09");
    assert_eq!(format_placeholder("TEST", 10), "TEST_10");
    assert_eq!(format_placeholder("TEST", 100), "TEST_100");
}

#[test]
fn test_format_placeholder_all_types() {
    // Test all placeholder types
    let types = vec![
        PlaceholderType::Word,
        PlaceholderType::Position,
        PlaceholderType::Col,
        PlaceholderType::List,
        PlaceholderType::CodeBlock,
    ];

    for placeholder_type in types {
        let result = format_placeholder_typed(placeholder_type, 42);
        assert!(result.ends_with("_42"));
        assert!(result.len() > 4); // At least "TYPE_42"
    }
}

#[test]
fn test_format_placeholder_edge_cases() {
    // Very large indices
    assert_eq!(format_placeholder("WORD", 999999), "WORD_999999");

    // Zero index
    assert_eq!(format_placeholder("WORD", 0), "WORD_00");
}
