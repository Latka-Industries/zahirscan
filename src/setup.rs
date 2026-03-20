//! Setup utilities for `ZahirScan`: logger, config, CLI flag application, input/output path resolution.

use anyhow::Context;
use colored::Colorize;
use env_logger;
use log::{debug, warn};
use std::fs;
use std::io::Write;
use std::sync::mpsc::Sender;

use crate::PKG_NAME;
use crate::config::RuntimeConfig;
use crate::results::{Output, OutputMode};
use crate::utils::typecheck::is_stderr_tty;

/// Controls where scan results go and whether they are collected.
///
/// * [`Collect`](OutputSink::Collect): Engine collects all outputs and returns them in [`ZahirScanResult::outputs`].
/// * [`StreamOnly`](OutputSink::StreamOnly): Each `(path, Output)` is passed to the callback; nothing is collected (bounded memory).
/// * [`Channel`](OutputSink::Channel): Each `(path, Output)` is sent on the channel; nothing is collected.
pub enum OutputSink {
    /// Collect all outputs and return them (default).
    Collect,
    /// Invoke the callback for each file; do not collect.
    StreamOnly(Box<dyn Fn(String, Output) + Send + Sync>),
    /// Send each `(path, Output)` on the channel; do not collect.
    Channel(Sender<(String, Output)>),
}

/// Build logger based on development mode.
/// If `dev_mode` is true, set log level to Debug.
pub fn build_logger(dev_mode: bool) {
    if dev_mode {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Debug)
            .init();
        debug!("Debug mode enabled");
    } else {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Info)
            .format(|buf, record| {
                let level_str = record.level().to_string();
                let level_display = match record.level() {
                    log::Level::Error => level_str.red().to_string(),
                    log::Level::Warn => level_str.yellow().to_string(),
                    _ => level_str,
                };
                writeln!(
                    buf,
                    "{}{} {} {}{} {}",
                    "[".bright_green().bold(),
                    PKG_NAME.bright_green().bold(),
                    level_display,
                    record.target().white(),
                    "]".bright_green().bold(),
                    record.args()
                )
            })
            .init();
    }
}

/// Path to user config file. Unix: ~/.config/zahirscan/zahirscan.toml (`XDG_CONFIG_HOME` or $HOME/.config). Windows: %APPDATA%\zahirscan\zahirscan.toml.
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
        config_dir.map(|d| d.join(PKG_NAME).join(format!("{PKG_NAME}.toml")))
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
///
/// On load failure (invalid TOML overlay or validation), falls back to [`RuntimeConfig::default`].
#[must_use]
pub fn load_config() -> RuntimeConfig {
    let overlay = xdg_config_path();
    match RuntimeConfig::load_config_with_overlay(crate::DEFAULT_CONFIG_TOML, overlay.as_deref()) {
        Ok(config) => {
            if let Some(p) = overlay.as_ref().filter(|p| p.exists()) {
                debug!("Merged config from {}", p.display());
            }
            config
        }
        Err(_) => RuntimeConfig::default(),
    }
}

/// Apply CLI flags to config (progress, output mode, redact, skip media).
/// Validates config after applying.
///
/// # Errors
///
/// Returns [`anyhow::Error`] when [`RuntimeConfig::validate_external`] fails (e.g. invalid numeric ranges).
pub fn apply_cli_to_config(
    config: &mut RuntimeConfig,
    progress: bool,
    dev: bool,
    full: bool,
    redact: bool,
    no_media: bool,
) -> anyhow::Result<()> {
    config.flags.show_progress = match (progress, dev, is_stderr_tty()) {
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
    config.flags.redact_paths = redact;
    config.flags.skip_media_metadata = no_media;
    config
        .validate_external()
        .map_err(|e| anyhow::anyhow!("Invalid config: {e}"))
}

/// Validate input paths and resolve output directory. Returns (paths, `output_dir`).
///
/// # Errors
///
/// Returns [`anyhow::Error`] if `input` is empty, if creating the output directory fails, or if canonicalizing the output path fails.
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
