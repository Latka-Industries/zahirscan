//! End-to-end integration tests for SQLite: file on disk → extract_zahir → Output/JSON.
use std::fs;
use std::path::PathBuf;
use zahirscan::{OutputMode, extract_zahir};

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn sqlite_file_type_detection_extensions() {
    let simple_db = include_bytes!("../fixtures/simple.db");
    let dir = tempfile::tempdir().expect("temp dir");
    for ext in [".db", ".sqlite", ".sqlite3"] {
        let path = dir.path().join(format!("test{}", ext));
        fs::write(&path, simple_db.as_slice()).expect("write fixture");
        let path_str = path.to_string_lossy();
        let result = extract_zahir::<&str>(path_str.as_ref(), OutputMode::Full, None, None, None)
            .expect("extract_zahir");
        assert_eq!(result.outputs.len(), 1);
        assert_eq!(result.outputs[0].file_type.as_deref(), Some("Sqlite"));
        assert!(
            result.outputs[0].sqlite_metadata.is_some(),
            "sqlite_metadata should be present for {}",
            ext
        );
    }
}

#[test]
fn sqlite_e2e_full_mode() {
    let path = fixture_path("simple.db");
    let result = extract_zahir(path.to_str().unwrap(), OutputMode::Full, None, None, None)
        .expect("extract_zahir");
    let outputs = &result.outputs;
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].file_type.as_deref(), Some("Sqlite"));
    let meta = outputs[0]
        .sqlite_metadata
        .as_ref()
        .expect("sqlite_metadata");
    assert!(meta.table_count.is_some());
    assert_eq!(meta.table_count, Some(1));
    let tables = meta.tables.as_ref().expect("tables");
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].name, "users");
}

#[test]
fn sqlite_e2e_templates_mode() {
    let path = fixture_path("simple.db");
    let result = extract_zahir(
        path.to_str().unwrap(),
        OutputMode::Templates,
        None,
        None,
        None,
    )
    .expect("extract_zahir");
    let outputs = &result.outputs;
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].file_type.as_deref(), Some("Sqlite"));
    assert!(outputs[0].templates.is_empty());
    assert!(outputs[0].sqlite_metadata.is_some());
}

#[test]
fn sqlite_e2e_json_contains_sqlite_metadata() {
    let path = fixture_path("simple.db");
    let result = extract_zahir(path.to_str().unwrap(), OutputMode::Full, None, None, None)
        .expect("extract_zahir");
    let json = serde_json::to_value(&result.outputs[0]).expect("serialize");
    assert!(json.get("sqlite_metadata").is_some());
    let meta = json.get("sqlite_metadata").unwrap();
    assert!(meta.get("table_count").is_some());
    assert!(meta.get("tables").is_some());
}
