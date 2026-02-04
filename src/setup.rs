//! Setup utilities for ZahirScan: logger, config, CLI flag application, input/output path resolution.

use anyhow::Context;
use env_logger;
use log::{debug, warn};
use std::fs;

use crate::PKG_NAME;
use crate::config::RuntimeConfig;
use crate::engine::tools::is_stderr_tty;
use crate::results::OutputMode;

/// Build logger based on development mode.
/// If dev_mode is true, set log level to Debug.
pub fn build_logger(dev_mode: bool) {
    if dev_mode {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Debug)
            .init();
        debug!("Debug mode enabled");
    } else {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Info)
            .init();
    }
}

/// Path to user config file. Unix: ~/.config/zahirscan/zahirscan.toml (XDG_CONFIG_HOME or $HOME/.config). Windows: %APPDATA%\zahirscan\zahirscan.toml.
pub fn xdg_config_path() -> Option<std::path::PathBuf> {
    use std::path::PathBuf;
    #[cfg(unix)]
    {
        let config_dir = std::env::var("XDG_CONFIG_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| PathBuf::from(h).join(".config"))
            });
        config_dir.map(|d| d.join(PKG_NAME).join(format!("{}.toml", PKG_NAME)))
    }
    #[cfg(windows)]
    {
        std::env::var("APPDATA").ok().map(|a| {
            PathBuf::from(a)
                .join(PKG_NAME)
                .join(format!("{}.toml", PKG_NAME))
        })
    }
}

/// Load config: embedded default (baked in at build) then overlay user config in app data dir.
/// Only keys present in the user file override; the rest stay from the embedded default.
pub fn load_config() -> RuntimeConfig {
    let overlay = xdg_config_path();
    match RuntimeConfig::load_config_with_overlay(crate::DEFAULT_CONFIG_TOML, overlay.as_deref()) {
        Ok(config) => {
            if overlay.as_ref().is_some_and(|p| p.exists()) {
                debug!("Merged config from {}", overlay.as_ref().unwrap().display());
            }
            config
        }
        Err(_) => RuntimeConfig::default(),
    }
}

/// Apply CLI flags to config (progress, output mode, redact, skip media).
/// Validates config after applying; returns an error if invalid.
pub fn apply_cli_to_config(
    config: &mut RuntimeConfig,
    progress: bool,
    dev: bool,
    full: bool,
    redact: bool,
    no_media: bool,
) -> anyhow::Result<()> {
    config.show_progress = match (progress, dev, is_stderr_tty()) {
        (true, true, _) => {
            debug!("--progress/-p flag was detected but will be disabled (dev mode)");
            false
        }
        (true, false, false) => {
            warn!("--progress/-p flag was detected but will be disabled (not a TTY)");
            false
        }
        (true, false, true) => true,
        (false, _, _) => false,
    };
    if full {
        config.output_mode = OutputMode::Full;
    }
    config.redact_paths = redact;
    config.skip_media_metadata = no_media;
    config
        .validate_external()
        .map_err(|e| anyhow::anyhow!("Invalid config: {}", e))
}

/// Validate input paths and resolve output directory. Returns (paths, output_dir).
pub fn resolve_output_paths(
    input: Vec<String>,
    output: Option<String>,
) -> anyhow::Result<(Vec<String>, Option<String>)> {
    if input.is_empty() {
        return Err(anyhow::anyhow!("At least one input file is required"));
    }
    let output = match output {
        Some(out) => {
            fs::create_dir_all(&out)?;
            let canonical = std::path::Path::new(&out)
                .canonicalize()
                .with_context(|| "Failed to resolve output directory")?;
            Some(canonical.to_string_lossy().into_owned())
        }
        None => None,
    };
    Ok((input, output))
}
