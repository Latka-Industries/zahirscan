//! Container/archive metadata structures (ZIP, TAR, etc.)

pub mod archive;
pub mod zip;

pub use archive::{ArchiveEntry, ArchiveMetadata};
pub use zip::{ZipEntry, ZipMetadata};
