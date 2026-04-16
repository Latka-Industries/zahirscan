//! Ensures copy_metadata_fields! and Output stay in sync with ParseResult.
//!
//! When adding a new metadata type you must:
//! 1. Add the field to ParseResult (parsers/mod.rs)
//! 2. Add the field to Output (results/core.rs)
//! 3. Add the copy line to copy_metadata_fields! (parsers/mod.rs)
//! 4. Add a setter below in parse_result_with_all_metadata() and an assertion in test_all_metadata_copied_to_output()
//!
//! If you forget (3), this test fails because the new field will be None in Output.

use zahirscan::config::RuntimeConfig;
use zahirscan::parsers::ParseResult;
use zahirscan::results::{
    ArchiveMetadata, ArrowIpcMetadata, AudioMetadata, AvroMetadata, CodeMetadata, CsvMetadata,
    DocumentMetadata, EpubMetadata, HtmlMetadata, ImageMetadata, IniMetadata, OrcMetadata,
    ParquetMetadata, PdfMetadata, PptxMetadata, SqliteMetadata, TomlMetadata, VideoMetadata,
    XmlMetadata, YamlMetadata, ZipMetadata, create_minimal_fallback,
};
use zahirscan::{FileType, OutputMode};

fn parse_result_with_all_metadata() -> ParseResult {
    let mut stats = ParseResult {
        file_path: "/test".to_string(),
        file_type: FileType::Log,
        line_count: 0,
        byte_count: 0,
        token_count: 0,
        duration: std::time::Duration::ZERO,
        is_binary: false,
        mining_result: None,
        image_metadata: Some(create_minimal_fallback::<ImageMetadata>(0)),
        video_metadata: Some(create_minimal_fallback::<VideoMetadata>(0)),
        audio_metadata: Some(create_minimal_fallback::<AudioMetadata>(0)),
        csv_metadata: Some(CsvMetadata::default()),
        pdf_metadata: Some(create_minimal_fallback::<PdfMetadata>(0)),
        docx_metadata: Some(create_minimal_fallback::<DocumentMetadata>(0)),
        sqlite_metadata: Some(create_minimal_fallback::<SqliteMetadata>(0)),
        toml_metadata: Some(create_minimal_fallback::<TomlMetadata>(0)),
        zip_metadata: Some(create_minimal_fallback::<ZipMetadata>(0)),
        xml_metadata: Some(create_minimal_fallback::<XmlMetadata>(0)),
        html_metadata: Some(create_minimal_fallback::<HtmlMetadata>(0)),
        yaml_metadata: Some(create_minimal_fallback::<YamlMetadata>(0)),
        ini_metadata: Some(create_minimal_fallback::<IniMetadata>(0)),
        pptx_metadata: Some(create_minimal_fallback::<PptxMetadata>(0)),
        epub_metadata: Some(create_minimal_fallback::<EpubMetadata>(0)),
        archive_metadata: Some(create_minimal_fallback::<ArchiveMetadata>(0)),
        code_metadata: Some(CodeMetadata::default()),
        parquet_metadata: Some(ParquetMetadata::default()),
        arrow_ipc_metadata: Some(ArrowIpcMetadata::default()),
        avro_metadata: Some(AvroMetadata::default()),
        orc_metadata: Some(OrcMetadata::default()),
        ..Default::default()
    };
    stats.mining_result = Some(zahirscan::results::MiningResult {
        templates: vec![],
        original_tokens: 0,
        compressed_tokens: 0,
        token_reduction_percent: 0.0,
        writing_footprint: None,
    });
    stats
}

#[test]
fn test_all_metadata_copied_to_output() {
    let config = RuntimeConfig::new();
    let stats = parse_result_with_all_metadata();
    let output = stats.to_output(OutputMode::Full, &config);

    // Canonical list: one assertion per metadata type. If you add a new type, add it here.
    // If copy_metadata_fields! is missing that field, this assertion fails.
    assert!(
        output.image_metadata.is_some(),
        "image_metadata not copied (add to copy_metadata_fields!)"
    );
    assert!(output.video_metadata.is_some(), "video_metadata not copied");
    assert!(output.audio_metadata.is_some(), "audio_metadata not copied");
    assert!(output.csv_metadata.is_some(), "csv_metadata not copied");
    assert!(output.pdf_metadata.is_some(), "pdf_metadata not copied");
    assert!(output.docx_metadata.is_some(), "docx_metadata not copied");
    assert!(
        output.sqlite_metadata.is_some(),
        "sqlite_metadata not copied"
    );
    assert!(output.toml_metadata.is_some(), "toml_metadata not copied");
    assert!(output.zip_metadata.is_some(), "zip_metadata not copied");
    assert!(output.xml_metadata.is_some(), "xml_metadata not copied");
    assert!(output.html_metadata.is_some(), "html_metadata not copied");
    assert!(output.yaml_metadata.is_some(), "yaml_metadata not copied");
    assert!(output.ini_metadata.is_some(), "ini_metadata not copied");
    assert!(output.pptx_metadata.is_some(), "pptx_metadata not copied");
    assert!(output.epub_metadata.is_some(), "epub_metadata not copied");
    assert!(
        output.archive_metadata.is_some(),
        "archive_metadata not copied"
    );
    assert!(output.code_metadata.is_some(), "code_metadata not copied");
    assert!(
        output.parquet_metadata.is_some(),
        "parquet_metadata not copied"
    );
    assert!(
        output.arrow_ipc_metadata.is_some(),
        "arrow_ipc_metadata not copied"
    );
    assert!(output.avro_metadata.is_some(), "avro_metadata not copied");
    assert!(output.orc_metadata.is_some(), "orc_metadata not copied");
}
