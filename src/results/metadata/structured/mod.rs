//! Structured data metadata models (CSV, EPUB, HTML, JSON, PDF, columnar,
//! `NumPy`, MATLAB, Matrix Market, HDF5, `NetCDF`, and Zarr).

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
pub mod tetration;
pub mod zarr;

pub use array::{ArrayLayoutSummary, NpyLayoutSummary};
pub use columnar::{
    ArrowIpcMetadata, AvroMetadata, ColumnStat, ColumnarCommonFields, CsvMetadata,
    MergeColumnStatsInput, OrcMetadata, ParquetMetadata, merge_column_stats,
};
pub use epub::EpubMetadata;
pub use hdf5::{Hdf5DatasetSummary, Hdf5Metadata};
pub use html::HtmlMetadata;
pub use json::JsonMetadata;
pub use matlab::{MatArrayEntrySummary, MatGlobalSummary, MatMetadata, MatStructWalkSummary};
pub use mtx::MtxMetadata;
pub use netcdf::{NetCdfAttributeEntry, NetCdfMetadata, NetCdfVariableSummary};
pub use numpy::{NpyMetadata, NpzMetadata, NpzNpyEntrySummary};
pub use pdf::PdfMetadata;
pub use tensor3d::{Tensor3DGlobalStats, Tensor3DPlaneStatEntry, Tensor3DPlaneStats};
pub use tetration::{TetDatasetSummary, TetrationMetadata};
pub use zarr::{ZarrArrayEntrySummary, ZarrMetadata};
