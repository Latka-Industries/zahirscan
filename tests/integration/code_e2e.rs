//! End-to-end integration tests for code/script metadata: file on disk → extract_schema → Output/JSON.

use std::fs;
use zahirscan::{OutputMode, extract_schema};

#[test]
fn code_file_type_detection_py() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("script.py");
    let content = "print(1)\n";
    fs::write(&path, content).expect("write");
    let path_str = path.to_string_lossy();
    let outputs =
        extract_schema::<&str>(path_str.as_ref(), OutputMode::Full).expect("extract_schema");
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].file_type.as_deref(), Some("Code"));
    let meta = outputs[0].code_metadata.as_ref().expect("code_metadata");
    assert_eq!(meta.script_type, "python");
    assert_eq!(meta.byte_count, content.len());
    assert_eq!(meta.line_count, 1);
}

#[test]
fn code_e2e_full_mode() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("main.rs");
    let content = "fn main() {\n    println!(\"hi\");\n}\n";
    fs::write(&path, content).expect("write");
    let path_str = path.to_string_lossy();
    let outputs =
        extract_schema::<&str>(path_str.as_ref(), OutputMode::Full).expect("extract_schema");
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].file_type.as_deref(), Some("Code"));
    let meta = outputs[0].code_metadata.as_ref().expect("code_metadata");
    assert_eq!(meta.script_type, "rust");
    assert_eq!(meta.byte_count, content.len());
    assert_eq!(meta.line_count, 3);
}

#[test]
fn code_e2e_templates_mode() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("app.js");
    fs::write(&path, "console.log('hello');\n").expect("write");
    let path_str = path.to_string_lossy();
    let outputs = extract_schema::<&str>(path_str.as_ref(), OutputMode::Templates)
        .expect("extract_schema");
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].file_type.as_deref(), Some("Code"));
    assert!(outputs[0].templates.is_empty());
    assert!(outputs[0].code_metadata.is_some());
}

#[test]
fn code_e2e_json_contains_code_metadata() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("script.py");
    fs::write(&path, "x = 1\n").expect("write");
    let path_str = path.to_string_lossy();
    let outputs =
        extract_schema::<&str>(path_str.as_ref(), OutputMode::Full).expect("extract_schema");
    let json = serde_json::to_value(&outputs[0]).expect("serialize");
    assert!(json.get("code_metadata").is_some());
    let meta = json.get("code_metadata").unwrap();
    assert_eq!(
        meta.get("script_type").and_then(|v| v.as_str()),
        Some("python")
    );
    assert!(meta.get("byte_count").is_some());
    assert!(meta.get("line_count").is_some());
}
