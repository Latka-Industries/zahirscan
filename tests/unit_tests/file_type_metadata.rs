//! `FileType::as_metadata_name` / `from_metadata_name` round-trip and edge cases.

use zahirscan::FileType;

/// Every `FileType` variant in `repr(u8)` order (must match `parsers/mod.rs`).
fn all_file_types() -> [FileType; 23] {
    [
        FileType::Log,
        FileType::Json,
        FileType::Text,
        FileType::Markdown,
        FileType::Image,
        FileType::Video,
        FileType::Audio,
        FileType::Csv,
        FileType::Pdf,
        FileType::Docx,
        FileType::Xlsx,
        FileType::Sqlite,
        FileType::Toml,
        FileType::Zip,
        FileType::Xml,
        FileType::Html,
        FileType::Yaml,
        FileType::Ini,
        FileType::Pptx,
        FileType::Epub,
        FileType::Archive,
        FileType::Code,
        FileType::Unknown,
    ]
}

#[test]
fn from_metadata_name_round_trips_as_metadata_name_for_all_variants() {
    for ft in all_file_types() {
        let name = ft.as_metadata_name();
        assert_eq!(FileType::from_metadata_name(name), Some(ft), "{name}");
        assert_eq!(
            FileType::from_metadata_name(ft.as_metadata_name()),
            Some(ft)
        );
    }
}

#[test]
fn from_metadata_name_returns_none_for_unknown_strings() {
    assert_eq!(FileType::from_metadata_name(""), None);
    assert_eq!(FileType::from_metadata_name("not a type"), None);
    assert_eq!(FileType::from_metadata_name("csv"), None);
    assert_eq!(FileType::from_metadata_name("CSV "), None);
    assert_eq!(FileType::from_metadata_name("JSON "), None);
}

#[test]
fn file_type_discriminant_range_matches_variant_count() {
    assert_eq!(FileType::Unknown as u8, 22);
    assert_eq!(all_file_types().len(), 23);
}
