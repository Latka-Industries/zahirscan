//! Settings-file formats: INI, YAML, TOML, XML.

mod ini;
mod toml;
mod xml;
mod yaml;

pub use ini::{extract_ini_metadata, extract_ini_templates};
pub use toml::{extract_toml_metadata, extract_toml_templates};
pub use xml::{extract_xml_metadata, extract_xml_templates};
pub use yaml::{extract_yaml_metadata, extract_yaml_templates};

use anyhow::Result;
use memmap2::Mmap;

use crate::config::RuntimeConfig;
use crate::parsers::{FileType, ParseResult};
use crate::results::MiningResult;

/// Dispatch by file type; fills the appropriate metadata field and returns templates.
///
/// # Errors
///
/// Propagates errors from the INI, TOML, YAML, or XML metadata/template extractors for the active [`crate::parsers::FileType`].
pub fn process(
    stats: &mut ParseResult,
    mmap: &Mmap,
    config: &RuntimeConfig,
) -> Result<MiningResult> {
    match stats.file_type {
        FileType::Toml => crate::process_with_metadata!(
            stats,
            mmap,
            config,
            toml_metadata,
            extract_toml_metadata(mmap, stats, config),
            crate::results::TomlMetadata,
            FileType::Toml,
            extract_toml_templates(mmap, stats, config)
        ),
        FileType::Yaml => crate::process_with_metadata!(
            stats,
            mmap,
            config,
            yaml_metadata,
            extract_yaml_metadata(mmap, stats, config),
            crate::results::YamlMetadata,
            FileType::Yaml,
            extract_yaml_templates(mmap, stats, config)
        ),
        FileType::Xml => crate::process_with_metadata!(
            stats,
            mmap,
            config,
            xml_metadata,
            extract_xml_metadata(mmap, stats, config),
            crate::results::XmlMetadata,
            FileType::Xml,
            extract_xml_templates(mmap, stats, config)
        ),
        FileType::Ini => crate::process_with_metadata!(
            stats,
            mmap,
            config,
            ini_metadata,
            extract_ini_metadata(mmap, stats, config),
            crate::results::IniMetadata,
            FileType::Ini,
            extract_ini_templates(mmap, stats, config)
        ),
        _ => unreachable!("settings::process called with {:?}", stats.file_type),
    }
}
