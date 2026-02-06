//! Tests for filename sanitization rules

use zahirscan::utils::path_string_helper::sanitize_filename;

#[test]
fn test_sanitize_removes_whitespace() {
    assert_eq!(sanitize_filename("file name.txt"), "filename.txt");
    assert_eq!(sanitize_filename("file\tname.txt"), "filename.txt");
    assert_eq!(sanitize_filename("file\nname.txt"), "filename.txt");
    assert_eq!(sanitize_filename("file\rname.txt"), "filename.txt");
    assert_eq!(
        sanitize_filename("file name with spaces.txt"),
        "filenamewithspaces.txt"
    );
}

#[test]
fn test_sanitize_removes_apostrophes_and_commas() {
    assert_eq!(sanitize_filename("don't.txt"), "dont.txt");
    assert_eq!(sanitize_filename("file, name.txt"), "filename.txt");
    assert_eq!(sanitize_filename("it's, a test.txt"), "itsatest.txt");
}

#[test]
fn test_sanitize_replaces_brackets_with_underscore() {
    assert_eq!(sanitize_filename("file[name].txt"), "file_name_.txt");
    assert_eq!(sanitize_filename("file{name}.txt"), "file_name_.txt");
    assert_eq!(sanitize_filename("file(name).txt"), "file_name_.txt");
    assert_eq!(sanitize_filename("[file].txt"), "_file_.txt");
    assert_eq!(sanitize_filename("{file}.txt"), "_file_.txt");
    assert_eq!(sanitize_filename("(file).txt"), "_file_.txt");
}

#[test]
fn test_sanitize_preserves_valid_characters() {
    assert_eq!(sanitize_filename("file-name.txt"), "file-name.txt");
    assert_eq!(sanitize_filename("file_name.txt"), "file_name.txt");
    assert_eq!(sanitize_filename("file123.txt"), "file123.txt");
    assert_eq!(sanitize_filename("File.Name.txt"), "File.Name.txt");
    assert_eq!(
        sanitize_filename("file@name#test.txt"),
        "file@name#test.txt"
    );
}

#[test]
fn test_sanitize_empty_string() {
    assert_eq!(sanitize_filename(""), "");
}

#[test]
fn test_sanitize_only_whitespace() {
    assert_eq!(sanitize_filename("   "), "");
    assert_eq!(sanitize_filename("\t\n\r"), "");
}

#[test]
fn test_sanitize_complex_filename() {
    assert_eq!(
        sanitize_filename("My File (2024) [v1.0], test.txt"),
        "MyFile_2024__v1.0_test.txt"
    );
}

#[test]
fn test_sanitize_unicode_characters() {
    // Unicode characters should be preserved
    assert_eq!(sanitize_filename("café.txt"), "café.txt");
    assert_eq!(sanitize_filename("文件.txt"), "文件.txt");
    assert_eq!(sanitize_filename("файл.txt"), "файл.txt");
}

#[test]
fn test_sanitize_multiple_brackets() {
    assert_eq!(
        sanitize_filename("file[test](v1).txt"),
        "file_test__v1_.txt"
    );
    assert_eq!(sanitize_filename("{file}[test].txt"), "_file__test_.txt");
}
