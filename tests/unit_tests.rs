//! Unit tests for ZahirScan core functionality

#[path = "unit_tests/jpeg_subsampling.rs"]
mod jpeg_subsampling;

#[path = "unit_tests/filename_sanitization.rs"]
mod filename_sanitization;

#[path = "unit_tests/adaptive_chunking.rs"]
mod adaptive_chunking;

#[path = "unit_tests/minimal_fallback.rs"]
mod minimal_fallback;

#[path = "unit_tests/serialization.rs"]
mod serialization;

#[path = "unit_tests/placeholder_formatting.rs"]
mod placeholder_formatting;

#[path = "unit_tests/csv_metadata.rs"]
mod csv_metadata;

#[path = "unit_tests/image_metadata.rs"]
mod image_metadata;

#[path = "unit_tests/sqlite_metadata.rs"]
mod sqlite_metadata;

#[path = "unit_tests/should_ignore_path.rs"]
mod should_ignore_path;
