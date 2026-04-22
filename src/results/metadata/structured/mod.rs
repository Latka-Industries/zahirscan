//! Structured data metadata (CSV, EPUB, HTML, JSON, PDF, columnar formats, `NumPy`, HDF5, `NetCDF`)

pub mod array;
pub mod columnar;
pub mod epub;
pub mod hdf5;
pub mod html;
pub mod json;
pub mod matlab;
pub mod mtx;
pub mod netcdf;
pub mod numpy;
pub mod pdf;
pub mod tensor3d;

pub use array::{ArrayLayoutSummary, NpyLayoutSummary};
pub use columnar::{
    ArrowIpcMetadata, AvroMetadata, ColumnStat, ColumnarCommonFields, CsvMetadata,
    MergeColumnStatsInput, OrcMetadata, ParquetMetadata, merge_column_stats,
};
pub use epub::EpubMetadata;
pub use hdf5::{Hdf5DatasetSummary, Hdf5Metadata};
pub use html::HtmlMetadata;
pub use json::JsonMetadata;
pub use matlab::{MatArrayEntrySummary, MatMetadata};
pub use mtx::MtxMetadata;
pub use netcdf::{NetCdfAttributeEntry, NetCdfMetadata, NetCdfVariableSummary};
pub use numpy::{NpyMetadata, NpzMetadata, NpzNpyEntrySummary};
pub use pdf::PdfMetadata;
pub use tensor3d::{Tensor3DPlaneStatEntry, Tensor3DPlaneStats};
