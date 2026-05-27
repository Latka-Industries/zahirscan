//! Shared metadata field registry used to keep ParseResult/Output wiring DRY.

/// Iterate over every metadata field with:
/// - field identifier
/// - `ParseResult` field type
/// - Output field type
/// - serialized JSON key
#[macro_export]
macro_rules! for_each_metadata_field {
    ($callback:ident $(, $args:tt)*) => {
        $callback!($($args,)* image_metadata, $crate::results::ImageMetadata, super::metadata::ImageMetadata, "image_metadata");
        $callback!($($args,)* video_metadata, $crate::results::VideoMetadata, super::metadata::VideoMetadata, "video_metadata");
        $callback!($($args,)* audio_metadata, $crate::results::AudioMetadata, super::metadata::AudioMetadata, "audio_metadata");
        $callback!($($args,)* csv_metadata, $crate::results::CsvMetadata, super::metadata::CsvMetadata, "csv_metadata");
        $callback!($($args,)* pdf_metadata, $crate::results::PdfMetadata, super::metadata::PdfMetadata, "pdf_metadata");
        $callback!($($args,)* docx_metadata, $crate::results::DocumentMetadata, super::metadata::DocumentMetadata, "docx_metadata");
        $callback!($($args,)* sqlite_metadata, $crate::results::SqliteMetadata, super::metadata::SqliteMetadata, "sqlite_metadata");
        $callback!($($args,)* toml_metadata, $crate::results::TomlMetadata, super::metadata::TomlMetadata, "toml_metadata");
        $callback!($($args,)* zip_metadata, $crate::results::ZipMetadata, super::metadata::ZipMetadata, "zip_metadata");
        $callback!($($args,)* xml_metadata, $crate::results::XmlMetadata, super::metadata::XmlMetadata, "xml_metadata");
        $callback!($($args,)* html_metadata, $crate::results::HtmlMetadata, super::metadata::HtmlMetadata, "html_metadata");
        $callback!($($args,)* yaml_metadata, $crate::results::YamlMetadata, super::metadata::YamlMetadata, "yaml_metadata");
        $callback!($($args,)* ini_metadata, $crate::results::IniMetadata, super::metadata::IniMetadata, "ini_metadata");
        $callback!($($args,)* pptx_metadata, $crate::results::PptxMetadata, super::metadata::PptxMetadata, "pptx_metadata");
        $callback!($($args,)* epub_metadata, $crate::results::EpubMetadata, super::metadata::EpubMetadata, "epub_metadata");
        $callback!($($args,)* archive_metadata, $crate::results::ArchiveMetadata, super::metadata::ArchiveMetadata, "archive_metadata");
        $callback!($($args,)* code_metadata, $crate::results::CodeMetadata, super::metadata::CodeMetadata, "code_metadata");
        $callback!($($args,)* log_metadata, $crate::results::LogMetadata, super::metadata::LogMetadata, "log_metadata");
        $callback!($($args,)* json_metadata, $crate::results::JsonMetadata, super::metadata::JsonMetadata, "json_metadata");
        $callback!($($args,)* parquet_metadata, $crate::results::ParquetMetadata, super::metadata::ParquetMetadata, "parquet_metadata");
        $callback!($($args,)* arrow_ipc_metadata, $crate::results::ArrowIpcMetadata, super::metadata::ArrowIpcMetadata, "arrow_ipc_metadata");
        $callback!($($args,)* avro_metadata, $crate::results::AvroMetadata, super::metadata::AvroMetadata, "avro_metadata");
        $callback!($($args,)* orc_metadata, $crate::results::OrcMetadata, super::metadata::OrcMetadata, "orc_metadata");
        $callback!($($args,)* npy_metadata, $crate::results::NpyMetadata, super::metadata::NpyMetadata, "npy_metadata");
        $callback!($($args,)* npz_metadata, $crate::results::NpzMetadata, super::metadata::NpzMetadata, "npz_metadata");
        $callback!($($args,)* hdf5_metadata, $crate::results::Hdf5Metadata, super::metadata::Hdf5Metadata, "hdf5_metadata");
        $callback!($($args,)* netcdf_metadata, $crate::results::NetCdfMetadata, super::metadata::NetCdfMetadata, "netcdf_metadata");
        $callback!($($args,)* mtx_metadata, $crate::results::MtxMetadata, super::metadata::MtxMetadata, "mtx_metadata");
        $callback!($($args,)* mat_metadata, $crate::results::MatMetadata, super::metadata::MatMetadata, "mat_metadata");
        $callback!($($args,)* onnx_metadata, $crate::results::OnnxMetadata, super::metadata::OnnxMetadata, "onnx_metadata");
        $callback!($($args,)* gguf_metadata, $crate::results::GgufMetadata, super::metadata::GgufMetadata, "gguf_metadata");
        $callback!($($args,)* tflite_metadata, $crate::results::TfliteMetadata, super::metadata::TfliteMetadata, "tflite_metadata");
        $callback!($($args,)* safetensors_metadata, $crate::results::SafetensorsMetadata, super::metadata::SafetensorsMetadata, "safetensors_metadata");
        $callback!($($args,)* zarr_metadata, $crate::results::ZarrMetadata, super::metadata::ZarrMetadata, "zarr_metadata");
        $callback!($($args,)* tetration_metadata, $crate::results::TetrationMetadata, super::metadata::TetrationMetadata, "tetration_metadata");
    };
}
