//! Structured data metadata (CSV, EPUB, HTML, JSON, PDF, columnar formats, `NumPy`, HDF5, NetCDF)

pub mod columnar;
pub mod epub;
pub mod hdf5;
pub mod html;
pub mod json;
pub mod netcdf;
pub mod numpy;
pub mod pdf;

pub use columnar::{
    ArrowIpcMetadata, AvroMetadata, ColumnarCommonFields, CsvMetadata, OrcMetadata, ParquetMetadata,
};
pub use epub::EpubMetadata;
pub use hdf5::{Hdf5DatasetSummary, Hdf5Metadata};
pub use html::HtmlMetadata;
pub use json::JsonMetadata;
pub use netcdf::{NetCdfAttributeEntry, NetCdfMetadata, NetCdfVariableSummary};
pub use numpy::{NpyLayoutSummary, NpyMetadata, NpzMetadata, NpzNpyEntrySummary};
pub use pdf::PdfMetadata;
