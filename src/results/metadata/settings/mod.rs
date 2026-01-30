//! Configuration file metadata structures (INI, TOML, XML, YAML)

pub mod ini;
pub mod toml;
pub mod xml;
pub mod yaml;

pub use ini::IniMetadata;
pub use toml::TomlMetadata;
pub use xml::XmlMetadata;
pub use yaml::YamlMetadata;
