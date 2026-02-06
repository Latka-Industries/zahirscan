//! Tests for path ignore rules (ignore_patterns, ignore_hidden_files)

use crate::get_test_config;
use zahirscan::{RuntimeConfig, utils::path_string_helper::should_ignore_path};

fn config_with(ignore_patterns: Vec<&str>, ignore_hidden_files: bool) -> RuntimeConfig {
    let mut c = get_test_config();
    c.ignore_patterns = ignore_patterns.into_iter().map(String::from).collect();
    c.ignore_hidden_files = ignore_hidden_files;
    c
}

#[test]
fn should_ignore_path_exact() {
    let c = config_with(vec![".DS_Store", "Thumbs.db"], false);
    assert!(should_ignore_path("/a/.DS_Store", &c));
    assert!(should_ignore_path("/x/y/Thumbs.db", &c));
    assert!(!should_ignore_path("/a/.DS_Storex", &c));
    assert!(!should_ignore_path("/a/DS_Store", &c));
}

#[test]
fn should_ignore_path_exact_case_insensitive() {
    let c = config_with(vec!["desktop.ini"], false);
    assert!(should_ignore_path("/a/desktop.ini", &c));
    assert!(should_ignore_path("/a/Desktop.ini", &c));
}

#[test]
fn should_ignore_path_tilda_dollar_prefix() {
    let c = config_with(vec!["~$*"], false);
    assert!(should_ignore_path("/a/~$foo.xlsx", &c));
}

#[test]
fn should_ignore_path_suffix_glob() {
    let c = config_with(vec!["*.swp", "*~"], false);
    assert!(should_ignore_path("/a/foo.swp", &c));
    assert!(should_ignore_path("/b/bar~", &c));
    assert!(!should_ignore_path("/a/foo.swp.bak", &c));
}

#[test]
fn should_ignore_path_prefix_glob() {
    let c = config_with(vec![".git*"], false);
    assert!(should_ignore_path("/a/.git", &c));
    assert!(should_ignore_path("/a/.gitignore", &c));
    assert!(!should_ignore_path("/a/git", &c));
}

#[test]
fn should_ignore_path_hidden() {
    let c = config_with(vec![], true);
    assert!(should_ignore_path("/a/.hidden", &c));
    assert!(should_ignore_path("/.bashrc", &c));
    assert!(!should_ignore_path("/a/normal", &c));
}

#[test]
fn should_ignore_path_hidden_disabled() {
    let c = config_with(vec![], false);
    assert!(!should_ignore_path("/a/.hidden", &c));
}
