//! Structured formats: CSV, HTML.

use crate::engine::config::Config;
use crate::parsers::{FileType, ParseResult};
use crate::results::MiningResult;
use anyhow::Result;
use memmap2::Mmap;

mod csv;
mod html;

pub(crate) use csv::infer_value_type;
pub use csv::{extract_csv_metadata, extract_csv_templates};
pub use html::{extract_html_metadata, extract_html_templates};

/// Dispatch by file type; fills csv_metadata or html_metadata and returns templates.
pub fn process(stats: &mut ParseResult, mmap: &Mmap, config: &Config) -> Result<MiningResult> {
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
        _ => unreachable!("structured::process called with {:?}", stats.file_type),
    }
}
