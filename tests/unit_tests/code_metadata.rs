//! Unit tests for code/script metadata: detect_file_type fallback and extract_code_metadata

use std::fs::File;
use std::time::Duration;
use tempfile::TempDir;
use zahirscan::engine::config::Config;
use zahirscan::engine::tools::detect_file_type;
use zahirscan::parsers::code::extract_code_metadata;
use zahirscan::parsers::{FileType, ParseResult};

fn get_test_config() -> Config {
    Config::default()
}

fn get_test_stats(file_path: &str, byte_count: usize, line_count: usize) -> ParseResult {
    ParseResult {
        file_path: file_path.to_string(),
        file_type: FileType::Code,
        line_count,
        byte_count,
        token_count: byte_count / 4,
        duration: Duration::ZERO,
        is_binary: false,
        ..Default::default()
    }
}

#[test]
fn detect_file_type_known_types_stay_known() {
    // Our FILE_EXTENSION_MAP first — never hit linguist
    assert_eq!(detect_file_type("foo.txt"), FileType::Text);
    assert_eq!(detect_file_type("foo.zip"), FileType::Zip);
    assert_eq!(detect_file_type("foo.sqlite3"), FileType::Sqlite);
    assert_eq!(detect_file_type("data.json"), FileType::Json);
    assert_eq!(detect_file_type("readme.md"), FileType::Markdown);
}

#[test]
fn detect_file_type_unknown_then_linguist_code() {
    // Unknown to our map → linguist fallback (extension/filename; file must exist) → Code if recognized.
    let dir = TempDir::new().unwrap();
    for (name, content) in [
        ("script.py", b"print(1)\n" as &[u8]),
        ("main.rs", b"fn main() {}\n"),
        ("app.js", b"console.log(1)\n"),
        ("run.sh", b"echo hi\n"),
    ] {
        let path = dir.path().join(name);
        std::fs::write(&path, content).unwrap();
        let path_str = path.to_str().unwrap();
        assert_eq!(detect_file_type(path_str), FileType::Code, "{}", name);
    }
}

#[test]
fn detect_file_type_unknown_stays_unknown() {
    // Unknown to our map and linguist returns empty or path doesn't exist
    assert_eq!(detect_file_type("file.xyz"), FileType::Unknown);
    assert_eq!(detect_file_type("data.unknown"), FileType::Unknown);
}

#[test]
fn extract_code_metadata_script_type_from_path() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("main.rs");
    let content = b"fn main() {}\n";
    std::fs::write(&path, content).unwrap();

    let file = File::open(&path).unwrap();
    let mmap = unsafe { memmap2::Mmap::map(&file).unwrap() };
    let path_str = path.to_str().unwrap();
    let stats = get_test_stats(path_str, content.len(), 1);
    let config = get_test_config();

    let meta = extract_code_metadata(&mmap, &stats, &config).unwrap();
    assert_eq!(meta.script_type, "rust");
    assert_eq!(meta.byte_count, content.len());
    assert_eq!(meta.line_count, 1);
}

#[test]
fn extract_code_metadata_shebang_override() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("script.py");
    let content = b"#!/usr/bin/env python3\nprint(42)\n";
    std::fs::write(&path, content).unwrap();

    let file = File::open(&path).unwrap();
    let mmap = unsafe { memmap2::Mmap::map(&file).unwrap() };
    let path_str = path.to_str().unwrap();
    let stats = get_test_stats(path_str, content.len(), 2);
    let config = get_test_config();

    let meta = extract_code_metadata(&mmap, &stats, &config).unwrap();
    assert_eq!(meta.script_type, "python");
    assert_eq!(meta.byte_count, content.len());
    assert_eq!(meta.line_count, 2);
}
