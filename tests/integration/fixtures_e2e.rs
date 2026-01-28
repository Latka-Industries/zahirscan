//! End-to-end integration tests for simple fixture files: file on disk → extract_schema → Output.
use std::path::PathBuf;
use zahirscan::{OutputMode, extract_schema};

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

/// (fixture filename, expected file_type string from Output)
const SIMPLE_FIXTURES: &[(&str, &str)] = &[
    ("sample.log", "Log"),
    ("sample.txt", "Text"),
    ("sample.csv", "Csv"),
    ("sample.json", "Json"),
    ("sample.md", "Markdown"),
    ("sample.toml", "Toml"),
    ("sample.yaml", "Yaml"),
];

#[test]
fn simple_fixtures_full_mode() {
    for (filename, expected_type) in SIMPLE_FIXTURES {
        let path = fixture_path(filename);
        let path_str = path.to_str().expect("path is valid UTF-8");
        let outputs = extract_schema(path_str, OutputMode::Full).expect("extract_schema");
        assert_eq!(outputs.len(), 1, "{}", filename);
        assert_eq!(
            outputs[0].file_type.as_deref(),
            Some(*expected_type),
            "{}",
            filename
        );
    }
}

#[test]
fn simple_fixtures_templates_mode() {
    for (filename, expected_type) in SIMPLE_FIXTURES {
        let path = fixture_path(filename);
        let path_str = path.to_str().expect("path is valid UTF-8");
        let outputs = extract_schema(path_str, OutputMode::Templates).expect("extract_schema");
        assert_eq!(outputs.len(), 1, "{}", filename);
        assert_eq!(
            outputs[0].file_type.as_deref(),
            Some(*expected_type),
            "{}",
            filename
        );
    }
}

#[test]
fn multi_file_extract_schema_returns_one_output_per_file() {
    let p1 = fixture_path("sample.txt")
        .into_os_string()
        .into_string()
        .unwrap();
    let p2 = fixture_path("sample.json")
        .into_os_string()
        .into_string()
        .unwrap();
    let paths: Vec<String> = vec![p1, p2];
    let outputs = extract_schema(paths.as_slice(), OutputMode::Full).expect("extract_schema");
    assert_eq!(outputs.len(), 2);
    assert_eq!(outputs[0].file_type.as_deref(), Some("Text"));
    assert_eq!(outputs[1].file_type.as_deref(), Some("Json"));
}

#[test]
fn extract_schema_empty_paths_returns_err() {
    let empty: &[&str] = &[];
    let err = extract_schema(empty, OutputMode::Full).unwrap_err();
    assert!(
        err.to_string().contains("No file paths provided"),
        "expected 'No file paths provided', got: {}",
        err
    );
}

#[test]
fn extract_schema_nonexistent_path_returns_err() {
    let err = extract_schema(
        "/nonexistent/path/that/does/not/exist.txt",
        OutputMode::Full,
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("No valid files found"),
        "expected 'No valid files found', got: {}",
        err
    );
}
