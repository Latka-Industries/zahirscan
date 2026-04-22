//! Shared literals and numeric limits for structured parsers (CSV, columnar binaries, MTX, NPY).

/// Serialized `encoding` and related labels in columnar / CSV metadata JSON.
pub struct StructuredEncoding;

impl StructuredEncoding {
    pub const TABULAR_BINARY: &'static str = "binary";
    pub const NUMPY: &'static str = "numpy";
    pub const MATLAB: &'static str = "matlab";
    pub const MATRIX_MARKET: &'static str = "matrix-market";
}

/// [`crate::results::CsvMetadata::encoding`]-style hints for delimiter-separated text.
pub struct CsvEncodingLabel;

impl CsvEncodingLabel {
    pub const UTF8: &'static str = "UTF-8";
    pub const NON_UTF8: &'static str = "Non-UTF-8";
}

/// [`crate::results::ArrowIpcMetadata::container_kind`] values.
pub struct ArrowIpcContainerKind;

impl ArrowIpcContainerKind {
    pub const IPC_FILE: &'static str = "ipc_file";
    pub const IPC_STREAM: &'static str = "ipc_stream";
    pub const FEATHER: &'static str = "feather";
}

/// Matrix Market [`crate::results::MtxMetadata::symmetry`] strings.
pub struct MtxSymmetryLabel;

impl MtxSymmetryLabel {
    pub const GENERAL: &'static str = "general";
    pub const SYMMETRIC: &'static str = "symmetric";
}

/// Numeric caps and thresholds shared across structured parsers.
pub mod limits {
    /// Column scaling numerator: `effective = base * N / max(cols, N)` for CSV and tabular binaries.
    pub const TABULAR_COL_SCALE_NUMERATOR: usize = 4000;
    /// CSV pass-2 and wide MTX string-inference column chunk width (`HashSet` / buffers per chunk).
    pub const TABULAR_COLUMN_CHUNK: usize = 256;
    /// File size (bytes) above which tabular row samples use per-decade retention scaling (`10^5`).
    pub const TABULAR_SAMPLE_BYTE_THRESHOLD: u64 = 100_000;
    /// Floor on retained sample fraction after file-size decade scaling.
    pub const TABULAR_BYTE_SCALE_MIN_RETAIN_FRAC: f64 = 0.03;
    /// Per decade above [`TABULAR_SAMPLE_BYTE_THRESHOLD`], subtract this fraction from 100% retention (e.g. 2% → 98%, 96%, …).
    pub const TABULAR_BYTE_SCALE_PCT_PER_DECADE: f64 = 0.02;

    /// Average bytes per row (`file_bytes / row_count`) at or below this is treated as **skinny** rows: large
    /// logical tables with small on-disk size skip harsh byte-decade shrink when other full-scan conditions hold.
    pub const TABULAR_BPR_SKINNY_MAX_BYTES: u64 = 2048;
    /// When row count is known, allow scanning **all** rows (subject to [`TABULAR_FULL_SCAN_MAX_ROWS`]) only if
    /// the file is at most this many bytes (cheap full read for stats).
    pub const TABULAR_FULL_SCAN_MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
    /// Max rows for the full-scan path (avoids multi-million-row tabular stats on pathological skinny files).
    pub const TABULAR_FULL_SCAN_MAX_ROWS: usize = 500_000;

    /// Back-compat alias for [`TABULAR_SAMPLE_BYTE_THRESHOLD`].
    pub const CSV_SAMPLE_BYTE_THRESHOLD: u64 = TABULAR_SAMPLE_BYTE_THRESHOLD;
    /// Back-compat alias for [`TABULAR_BYTE_SCALE_MIN_RETAIN_FRAC`].
    pub const CSV_BYTE_SCALE_MIN_RETAIN_FRAC: f64 = TABULAR_BYTE_SCALE_MIN_RETAIN_FRAC;
    /// Back-compat alias for [`TABULAR_BYTE_SCALE_PCT_PER_DECADE`].
    pub const CSV_BYTE_SCALE_PCT_PER_DECADE: f64 = TABULAR_BYTE_SCALE_PCT_PER_DECADE;
    /// Cap on infer-sample `rows × cols` string grid for MTX.
    pub const MAX_MTX_INFERENCE_STRING_CELLS: usize = 256_000;
    /// Max `rows × cols` for full logical sparse materialization in MTX numeric stats.
    pub const MAX_MTX_TABULAR_CELLS: usize = 8_000_000;

    /// Max 2D planes (along the contiguous stack axis) reported for 3D tensor summary stats.
    pub const TENSOR3D_MAX_PLANES: usize = 32;
    /// Max linear element visits across the whole 3D tensor (after subsampling stride).
    pub const TENSOR3D_MAX_LINEAR_SAMPLES: usize = 2_000_000;
    /// Max samples taken within one plane (each plane is strided to stay within this).
    pub const TENSOR3D_MAX_PLANE_LINEAR_SAMPLES: usize = 200_000;
}
