use crate::config::RuntimeConfig;
use crate::parsers::{FileType, ParseResult};
use crate::results::MiningResult;
use anyhow::Result;
use memmap2::Mmap;

pub mod archive;
pub mod zip;

/// Dispatch by file type; fills zip_metadata or archive_metadata and returns templates.
pub fn process(
    stats: &mut ParseResult,
    mmap: &Mmap,
    config: &RuntimeConfig,
) -> Result<MiningResult> {
    match stats.file_type {
        FileType::Zip => crate::process_with_metadata!(
            stats,
            mmap,
            config,
            zip_metadata,
            zip::extract_zip_metadata(mmap, stats, config),
            crate::results::ZipMetadata,
            FileType::Zip,
            zip::extract_zip_templates(mmap, stats, config)
        ),
        FileType::Archive => crate::process_with_metadata!(
            stats,
            mmap,
            config,
            archive_metadata,
            archive::extract_archive_metadata(mmap, stats, config),
            crate::results::ArchiveMetadata,
            FileType::Archive,
            archive::extract_archive_templates(mmap, stats, config)
        ),
        _ => unreachable!("container::process called with {:?}", stats.file_type),
    }
}
