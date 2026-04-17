//! Structured formats: CSV, HTML, JSON, EPUB, PDF, columnar binaries (Parquet, Arrow IPC, Avro, ORC), `NumPy` (NPY/NPZ), HDF5, and NetCDF.

mod columnar;
mod csv;
mod epub;
mod hdf5;
mod html;
mod json;
mod netcdf;
mod numpy;
mod pdf;
mod table_sample_profile;

pub use columnar::*;
pub use csv::{
    delimiter_byte_for_reader, detect_delimiter_byte, extract_csv_metadata, extract_csv_templates,
};
pub use epub::{extract_epub_metadata, extract_epub_templates};
pub use hdf5::{extract_hdf5_metadata, extract_hdf5_templates};
pub use html::{extract_html_metadata, extract_html_templates};
pub use json::{extract_json_metadata, extract_json_templates};
pub use netcdf::{extract_netcdf_metadata, extract_netcdf_templates};
pub use numpy::{
    extract_npy_metadata, extract_npy_templates, extract_npz_metadata, extract_npz_templates,
};
pub use pdf::{extract_pdf_metadata, extract_pdf_templates};
pub use table_sample_profile::*;

use anyhow::Result;
use memmap2::Mmap;

use crate::config::RuntimeConfig;
use crate::parsers::{FileType, ParseResult};
use crate::results::MiningResult;

/// Dispatch by file type; fills structured metadata fields and returns templates.
///
/// # Errors
///
/// Returns an error if the mmap is not valid UTF-8 where required, or if a structured parser fails.
pub fn process(
    stats: &mut ParseResult,
    mmap: &Mmap,
    config: &RuntimeConfig,
) -> Result<MiningResult> {
    match stats.file_type {
        FileType::Csv => crate::process_with_metadata!(
            stats,
            mmap,
            config,
            csv_metadata,
            extract_csv_metadata(mmap, stats, config),
            crate::results::CsvMetadata,
            FileType::Csv,
            extract_csv_templates(mmap, stats, config)
        ),
        FileType::Json => {
            let content = std::str::from_utf8(mmap)?;
            stats.json_metadata = Some(extract_json_metadata(content, stats));
            extract_json_templates(content, stats, config)
        }
        FileType::Epub => crate::process_with_metadata!(
            stats,
            mmap,
            config,
            epub_metadata,
            extract_epub_metadata(mmap, stats, config),
            crate::results::EpubMetadata,
            FileType::Epub,
            extract_epub_templates(mmap, stats, config)
        ),
        FileType::Html => crate::process_with_metadata!(
            stats,
            mmap,
            config,
            html_metadata,
            extract_html_metadata(mmap, stats, config),
            crate::results::HtmlMetadata,
            FileType::Html,
            extract_html_templates(mmap, stats, config)
        ),
        FileType::Pdf => crate::process_with_metadata!(
            stats,
            mmap,
            config,
            pdf_metadata,
            extract_pdf_metadata(mmap, stats, config),
            crate::results::PdfMetadata,
            FileType::Pdf,
            extract_pdf_templates(mmap, stats, config)
        ),
        FileType::Parquet => crate::process_with_metadata!(
            stats,
            mmap,
            config,
            parquet_metadata,
            extract_parquet_metadata(mmap, stats, config),
            crate::results::ParquetMetadata,
            FileType::Parquet,
            extract_parquet_templates(mmap, stats, config)
        ),
        FileType::ArrowIpc => crate::process_with_metadata!(
            stats,
            mmap,
            config,
            arrow_ipc_metadata,
            extract_arrow_ipc_metadata(mmap, stats, config),
            crate::results::ArrowIpcMetadata,
            FileType::ArrowIpc,
            extract_arrow_ipc_templates(mmap, stats, config)
        ),
        FileType::Avro => crate::process_with_metadata!(
            stats,
            mmap,
            config,
            avro_metadata,
            extract_avro_metadata(mmap, stats, config),
            crate::results::AvroMetadata,
            FileType::Avro,
            extract_avro_templates(mmap, stats, config)
        ),
        FileType::Orc => crate::process_with_metadata!(
            stats,
            mmap,
            config,
            orc_metadata,
            extract_orc_metadata(mmap, stats, config),
            crate::results::OrcMetadata,
            FileType::Orc,
            extract_orc_templates(mmap, stats, config)
        ),
        FileType::Npy => crate::process_with_metadata!(
            stats,
            mmap,
            config,
            npy_metadata,
            extract_npy_metadata(mmap, stats, config),
            crate::results::NpyMetadata,
            FileType::Npy,
            extract_npy_templates(mmap, stats, config)
        ),
        FileType::Npz => crate::process_with_metadata!(
            stats,
            mmap,
            config,
            npz_metadata,
            extract_npz_metadata(mmap, stats, config),
            crate::results::NpzMetadata,
            FileType::Npz,
            extract_npz_templates(mmap, stats, config)
        ),
        FileType::Hdf5 => crate::process_with_metadata!(
            stats,
            mmap,
            config,
            hdf5_metadata,
            extract_hdf5_metadata(mmap, stats, config),
            crate::results::Hdf5Metadata,
            FileType::Hdf5,
            extract_hdf5_templates(mmap, stats, config)
        ),
        FileType::NetCdf => crate::process_with_metadata!(
            stats,
            mmap,
            config,
            netcdf_metadata,
            extract_netcdf_metadata(mmap, stats, config),
            crate::results::NetCdfMetadata,
            FileType::NetCdf,
            extract_netcdf_templates(mmap, stats, config)
        ),
        _ => unreachable!("structured::process called with {:?}", stats.file_type),
    }
}
