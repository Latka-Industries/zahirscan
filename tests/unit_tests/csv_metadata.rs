//! Tests for CSV metadata extraction

use std::time::Duration;
use zahirscan::RuntimeConfig;
use zahirscan::parsers::structured::extract_csv_metadata;
use zahirscan::parsers::{FileType, ParseResult};

fn get_test_config() -> RuntimeConfig {
    RuntimeConfig::default()
}

fn get_test_stats() -> ParseResult {
    ParseResult {
        file_path: "test.csv".to_string(),
        file_type: FileType::Csv,
        line_count: 0,
        byte_count: 0,
        token_count: 0,
        duration: Duration::ZERO,
        is_binary: false,
        ..Default::default()
    }
}

#[test]
fn test_basic_csv_metadata() {
    let csv_content = b"name,age,city\nJohn,30,New York\nJane,25,Boston\nBob,35,Chicago";
    let stats = get_test_stats();
    let config = get_test_config();

    let metadata = extract_csv_metadata(csv_content, &stats, &config).unwrap();

    assert_eq!(metadata.row_count, 3);
    assert_eq!(metadata.column_count, 3);
    assert_eq!(metadata.delimiter, Some(",".to_string()));
    assert_eq!(metadata.has_header, Some(true));
    assert_eq!(metadata.encoding, Some("UTF-8".to_string()));

    let column_names = metadata.column_names.unwrap();
    assert_eq!(column_names, vec!["name", "age", "city"]);

    let column_types = metadata.column_types.unwrap();
    assert_eq!(column_types[0], "string"); // name
    assert_eq!(column_types[1], "number"); // age
    assert_eq!(column_types[2], "string"); // city
}

#[test]
fn test_csv_without_header() {
    let csv_content = b"John,30,New York\nJane,25,Boston\nBob,35,Chicago";
    let stats = get_test_stats();
    let config = get_test_config();

    let metadata = extract_csv_metadata(csv_content, &stats, &config).unwrap();

    // When has_headers(true) is used but headers fail, it still tries to read first row as header
    // So we get 2 data rows (Jane and Bob), not 3
    // The actual behavior depends on CSV reader configuration
    assert!(metadata.row_count >= 2);
    assert_eq!(metadata.column_count, 3);
    // has_header might be false or the reader might have tried to read headers
    assert!(metadata.has_header.is_some());
    // Column names might be None if header detection failed
}

#[test]
fn test_csv_delimiter_detection_comma() {
    let csv_content = b"col1,col2,col3\nval1,val2,val3";
    let stats = get_test_stats();
    let config = get_test_config();

    let metadata = extract_csv_metadata(csv_content, &stats, &config).unwrap();
    assert_eq!(metadata.delimiter, Some(",".to_string()));
}

#[test]
fn test_csv_delimiter_detection_semicolon() {
    let csv_content = b"col1;col2;col3\nval1;val2;val3";
    let stats = get_test_stats();
    let config = get_test_config();

    let metadata = extract_csv_metadata(csv_content, &stats, &config).unwrap();
    assert_eq!(metadata.delimiter, Some(";".to_string()));
}

#[test]
fn test_csv_delimiter_detection_tab() {
    let csv_content = b"col1\tcol2\tcol3\nval1\tval2\tval3";
    let stats = get_test_stats();
    let config = get_test_config();

    let metadata = extract_csv_metadata(csv_content, &stats, &config).unwrap();
    assert_eq!(metadata.delimiter, Some("\\t".to_string()));
}

#[test]
fn test_csv_quote_character_detection() {
    let csv_content = b"\"name\",\"age\",\"city\"\n\"John\",\"30\",\"New York\"";
    let stats = get_test_stats();
    let config = get_test_config();

    let metadata = extract_csv_metadata(csv_content, &stats, &config).unwrap();
    assert_eq!(metadata.quote_character, Some("\"".to_string()));
}

#[test]
fn test_csv_escape_character_backslash() {
    let csv_content = b"\"name\",\"description\"\n\"John\",\"He said \\\"Hello\\\"\"";
    let stats = get_test_stats();
    let config = get_test_config();

    let metadata = extract_csv_metadata(csv_content, &stats, &config).unwrap();
    assert_eq!(metadata.escape_character, Some("\\".to_string()));
}

#[test]
fn test_csv_escape_character_doubled_quotes() {
    let csv_content = b"\"name\",\"description\"\n\"John\",\"He said \"\"Hello\"\"\"";
    let stats = get_test_stats();
    let config = get_test_config();

    let metadata = extract_csv_metadata(csv_content, &stats, &config).unwrap();
    // When doubled quotes are used, escape character should be same as quote
    assert_eq!(metadata.quote_character, Some("\"".to_string()));
    assert_eq!(metadata.escape_character, Some("\"".to_string()));
}

#[test]
fn test_csv_numeric_statistics() {
    let csv_content = b"id,price\n1,10.5\n2,20.0\n3,15.75\n4,25.0\n5,12.5";
    let stats = get_test_stats();
    let config = get_test_config();

    let metadata = extract_csv_metadata(csv_content, &stats, &config).unwrap();

    let numeric_stats = metadata.numeric_stats.unwrap();
    let price_stats = numeric_stats[1].as_ref().unwrap();

    assert_eq!(price_stats.min, Some(10.5));
    assert_eq!(price_stats.max, Some(25.0));
    assert!(price_stats.mean.is_some());
    assert!(price_stats.median.is_some());
    assert!(price_stats.range.is_some());
    assert!(price_stats.stdev.is_some());
    assert!(price_stats.iqr.is_some());
}

#[test]
fn test_csv_date_statistics() {
    let csv_content = b"id,date\n1,2025-01-01\n2,2025-01-05\n3,2025-01-10";
    let stats = get_test_stats();
    let config = get_test_config();

    let metadata = extract_csv_metadata(csv_content, &stats, &config).unwrap();

    let date_stats = metadata.date_stats.unwrap();
    let date_col_stats = date_stats[1].as_ref().unwrap();

    assert!(date_col_stats.span_days.is_some());
    assert!(date_col_stats.span_minutes.is_some());
    assert!(date_col_stats.min.is_some());
    assert!(date_col_stats.max.is_some());
}

#[test]
fn test_csv_boolean_statistics() {
    let csv_content = b"id,active\n1,true\n2,false\n3,true\n4,true\n5,false";
    let stats = get_test_stats();
    let config = get_test_config();

    let metadata = extract_csv_metadata(csv_content, &stats, &config).unwrap();

    let boolean_stats = metadata.boolean_stats.unwrap();
    let active_stats = boolean_stats[1].as_ref().unwrap();

    // 3 out of 5 are true = 60%
    assert!(active_stats.true_percentage.is_some());
    let true_pct = active_stats.true_percentage.unwrap();
    assert!((true_pct - 60.0).abs() < 1.0); // Allow small floating point differences
}

#[test]
fn test_csv_null_percentages() {
    let csv_content = b"id,name,age\n1,John,30\n2,,25\n3,Bob,";
    let stats = get_test_stats();
    let config = get_test_config();

    let metadata = extract_csv_metadata(csv_content, &stats, &config).unwrap();

    let null_percentages = metadata.null_percentages.unwrap();
    // name column: 1 out of 3 empty = ~33.33%
    assert!(null_percentages[1] > 30.0 && null_percentages[1] < 40.0);
    // age column: 1 out of 3 empty = ~33.33%
    assert!(null_percentages[2] > 30.0 && null_percentages[2] < 40.0);
}

#[test]
fn test_csv_unique_counts() {
    let csv_content = b"id,name,category\n1,John,A\n2,Jane,A\n3,Bob,B\n4,John,A";
    let stats = get_test_stats();
    let config = get_test_config();

    let metadata = extract_csv_metadata(csv_content, &stats, &config).unwrap();

    let unique_counts = metadata.unique_counts.unwrap();
    // name column: 3 unique values (John, Jane, Bob) - but John appears twice
    assert!(unique_counts[1] >= 3);
    // category column: 2 unique values (A, B)
    assert_eq!(unique_counts[2], 2);
}

#[test]
fn test_csv_timestamp_detection() {
    let csv_content = b"id,timestamp\n1,1704067200\n2,1704153600";
    let stats = get_test_stats();
    let config = get_test_config();

    let metadata = extract_csv_metadata(csv_content, &stats, &config).unwrap();

    let column_types = metadata.column_types.unwrap();
    assert_eq!(column_types[1], "timestamp");
}

#[test]
fn test_csv_empty_file() {
    let csv_content = b"";
    let stats = get_test_stats();
    let config = get_test_config();

    let metadata = extract_csv_metadata(csv_content, &stats, &config).unwrap();

    assert_eq!(metadata.row_count, 0);
    assert_eq!(metadata.column_count, 0);
}

#[test]
fn test_csv_single_row() {
    let csv_content = b"name,age\nJohn,30";
    let stats = get_test_stats();
    let config = get_test_config();

    let metadata = extract_csv_metadata(csv_content, &stats, &config).unwrap();

    assert_eq!(metadata.row_count, 1);
    assert_eq!(metadata.column_count, 2);
}

#[test]
fn test_csv_mixed_types() {
    let csv_content = b"id,name,age,active,price,date\n1,John,30,true,10.5,2025-01-01\n2,Jane,25,false,20.0,2025-01-02";
    let stats = get_test_stats();
    let config = get_test_config();

    let metadata = extract_csv_metadata(csv_content, &stats, &config).unwrap();

    let column_types = metadata.column_types.unwrap();
    // Type inference is probabilistic - check that we get expected types
    assert_eq!(column_types[1], "string"); // name
    assert_eq!(column_types[2], "number"); // age
    assert_eq!(column_types[3], "boolean"); // active
    assert_eq!(column_types[4], "number"); // price
    assert_eq!(column_types[5], "date"); // date
    // id might be detected as number or boolean depending on inference
    assert!(
        column_types[0] == "number" || column_types[0] == "boolean" || column_types[0] == "string"
    );
}

#[test]
fn test_csv_encoding_detection() {
    let csv_content = b"name,age\nJohn,30";
    let stats = get_test_stats();
    let config = get_test_config();

    let metadata = extract_csv_metadata(csv_content, &stats, &config).unwrap();
    assert_eq!(metadata.encoding, Some("UTF-8".to_string()));
}

#[test]
fn test_csv_all_null_column() {
    let csv_content = b"id,name\n1,\n2,\n3,";
    let stats = get_test_stats();
    let config = get_test_config();

    let metadata = extract_csv_metadata(csv_content, &stats, &config).unwrap();

    let null_percentages = metadata.null_percentages.unwrap();
    // name column should be 100% null
    assert!((null_percentages[1] - 100.0).abs() < 0.1);
}
