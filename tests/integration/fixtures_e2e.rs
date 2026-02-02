//! End-to-end integration tests for simple fixture files: file on disk → extract_schema → Output.

use crate::get_test_config;
use std::path::PathBuf;
use std::process::Command;
use zahirscan::{OutputMode, extract_schema, extract_schema_with_config};

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

/// Extended fixtures: PDF, DOCX, Image, Zip, Archive, Sqlite, Epub
const EXTENDED_FIXTURES: &[(&str, &str)] = &[
    ("sample.pdf", "Pdf"),
    ("sample.docx", "Docx"),
    ("sample.jpg", "Image"),
    ("sample.epub", "Epub"),
    ("fixtures.zip", "Zip"),
    ("fixtures.tar", "Archive"),
    ("fixtures.tar.gz", "Archive"),
    ("fixtures.tgz", "Archive"),
    ("fixtures.tar.bz2", "Archive"),
    ("fixtures.tar.xz", "Archive"),
    ("simple.db", "Sqlite"),
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

#[test]
fn extended_fixtures_full_mode() {
    for (filename, expected_type) in EXTENDED_FIXTURES {
        let path = fixture_path(filename);
        if !path.exists() {
            continue; // skip if fixture not present (e.g. optional)
        }
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

/// CLI: --output writes one file per input and output is valid JSON
#[test]
fn cli_output_dir_writes_files() {
    let out_dir = tempfile::tempdir().expect("temp dir");
    let out_path = out_dir.path().to_path_buf();
    let input = fixture_path("sample.txt");
    let bin = env!("CARGO_BIN_EXE_zahirscan");
    let status = Command::new(bin)
        .arg("-i")
        .arg(input)
        .arg("-o")
        .arg(out_path.as_os_str())
        .status()
        .expect("run zahirscan");
    assert!(status.success(), "zahirscan should succeed");
    let entries: Vec<_> = std::fs::read_dir(&out_path)
        .expect("read dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains("zahirscan"))
        .collect();
    assert!(
        !entries.is_empty(),
        "expected at least one .zahirscan.out file in {:?}",
        out_path
    );
    let out_file = std::fs::read_to_string(entries[0].path()).expect("read output file");
    let _: serde_json::Value =
        serde_json::from_str(&out_file).expect("output file should be valid JSON");
}

/// CLI: invalid output dir (e.g. non-directory) fails with error
#[test]
fn cli_invalid_output_dir_fails() {
    let input = fixture_path("sample.txt");
    // Use existing file as -o so path exists but is not a directory (works on Windows and Unix)
    let output_path = fixture_path("sample.txt");
    let bin = env!("CARGO_BIN_EXE_zahirscan");
    let output = Command::new(bin)
        .arg("-i")
        .arg(&input)
        .arg("-o")
        .arg(&output_path)
        .output()
        .expect("run zahirscan");
    assert!(
        !output.status.success(),
        "zahirscan should fail when -o is not a directory"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stderr.contains("canonicalization")
            || stderr.contains("resolve")
            || stderr.contains("output")
            || stderr.contains("directory")
            || stderr.contains("Error")
            || stdout.contains("canonicalization")
            || stdout.contains("Error"),
        "error output should mention canonicalization/resolve/output/directory or Error; stderr: {:?}, stdout: {:?}",
        stderr,
        stdout
    );
}

#[test]
fn extract_schema_with_config_reuses_config() {
    // Load config once
    let config = get_test_config();

    // Use same config for multiple calls (simulates TUI usage pattern)
    let files = ["sample.txt", "sample.log", "sample.json"];

    for filename in &files {
        let path = fixture_path(filename);
        let path_str = path.to_str().expect("path is valid UTF-8");

        // This should work without loading config from disk each time
        let outputs = extract_schema_with_config(path_str, OutputMode::Full, &config)
            .expect("extract_schema_with_config should succeed");

        assert_eq!(outputs.len(), 1, "Should process one file: {}", filename);
        assert!(outputs[0].source.is_some(), "Should have source");
    }
}

#[test]
fn extract_schema_with_config_multiple_files_at_once() {
    let config = get_test_config();

    // Process multiple files in a single call
    let paths: Vec<String> = ["sample.txt", "sample.log", "sample.json"]
        .iter()
        .map(|f| fixture_path(f).to_string_lossy().to_string())
        .collect();

    let outputs = extract_schema_with_config(paths.as_slice(), OutputMode::Full, &config)
        .expect("extract_schema_with_config should succeed with multiple files");

    assert_eq!(outputs.len(), 3, "Should process all three files");

    // Verify all outputs have expected data
    for output in &outputs {
        assert!(
            output.source.is_some(),
            "Each output should have a file name"
        );
        assert!(
            output.file_type.is_some(),
            "Each output should have a file type"
        );
    }
}
