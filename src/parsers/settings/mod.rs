//! Settings-file formats: INI, YAML, TOML, XML.

mod ini;
mod toml;
mod xml;
mod yaml;

pub use ini::{extract_ini_metadata, extract_ini_templates};
pub use toml::{extract_toml_metadata, extract_toml_templates};
pub use xml::{extract_xml_metadata, extract_xml_templates};
pub use yaml::{extract_yaml_metadata, extract_yaml_templates};
