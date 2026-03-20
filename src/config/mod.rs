//! Configuration for `ZahirScan`: TOML structs, runtime config, loading, and overlay merge.

mod core;
mod helpers;
mod structs;

/// Embedded default config (repo config.toml). Source of truth for defaults; used by `RuntimeConfig::new()`, CLI, and `zahirscan init`.
pub const DEFAULT_CONFIG_TOML: &str = include_str!("../../config.toml");

pub use core::{RuntimeConfig, RuntimeFlags};
