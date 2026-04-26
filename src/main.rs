use anyhow::Context;
use clap::Parser;
use std::fs;
use std::time::Instant;
use zahirscan::{engine, extract_zahir, setup, utils};

#[derive(Parser)]
#[command(name = zahirscan::PKG_NAME)]
#[command(
    about = "Template mining for text/logs and metadata extraction for media, documents, archives, and more"
)]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Args {
    #[command(subcommand)]
    subcommand: Option<SubCmd>,

    /// Input file(s) to parse (can specify multiple)
    #[arg(short = 'i', long, num_args = 1..)]
    input: Vec<String>,

    /// Output folder path (defaults to temp file if not specified).
    /// Creates filename.zahirscan.out in the folder for each input file
    #[arg(short = 'o', long)]
    output: Option<String>,

    #[command(flatten)]
    mode: CliModeArgs,

    #[command(flatten)]
    processing: CliProcessingArgs,
}

#[derive(Parser)]
struct CliModeArgs {
    /// Output mode: full metadata (for development/debugging).
    /// Default is templates-only mode (minimal JSON with templates & writing footprint)
    #[arg(short = 'f', long)]
    full: bool,

    /// Development mode: enables debug logging.
    /// Default is production mode (info level only). This disables progress bars if enabled.
    #[arg(short = 'd', long)]
    dev: bool,

    /// Redact file paths in output (show only filename as ***/filename.ext).
    /// Useful for privacy when sharing output JSON
    #[arg(short = 'r', long)]
    redact: bool,
}

#[derive(Parser)]
struct CliProcessingArgs {
    /// Skip media metadata extraction (audio, video, image).
    /// Faster processing when metadata is not needed
    #[arg(short = 'n', long)]
    no_media: bool,

    /// Show progress bars during processing.
    /// This is ignored if dev mode is enabled.
    #[arg(short = 'p', long)]
    progress: bool,
}

#[derive(clap::Subcommand)]
enum SubCmd {
    /// Write default config to XDG config dir (~/.config/zahirscan/zahirscan.toml or equivalent)
    Init,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if matches!(args.subcommand, Some(SubCmd::Init)) {
        run_init()?;
        return Ok(());
    }

    let start = Instant::now();

    setup::build_logger(args.mode.dev);

    let mut config = setup::load_config();
    setup::apply_cli_to_config(
        &mut config,
        setup::CliRuntimeFlags {
            mode: setup::CliRuntimeMode {
                dev: args.mode.dev,
                full: args.mode.full,
                redact: args.mode.redact,
            },
            processing: setup::CliRuntimeProcessing {
                progress: args.processing.progress,
                no_media: args.processing.no_media,
            },
        },
    )?;

    let (paths, output) = setup::resolve_output_paths(args.input.clone(), args.output.clone())?;

    engine::chunking::setup_rayon_thread_pool(config.max_workers as usize);

    let _result = extract_zahir(
        &paths,
        config.output_mode,
        Some(&config),
        output.as_deref(),
        &zahirscan::OutputSink::Collect,
    )?;

    utils::path_string_helper::print_progress_handler(
        &format!(
            "Total time: {}",
            utils::path_string_helper::format_duration(start.elapsed())
        ),
        config.flags.show_progress,
    );

    Ok(())
}

/// Write default config to XDG config directory.
fn run_init() -> anyhow::Result<()> {
    let path = setup::xdg_config_path()
        .context("Could not determine XDG config directory (HOME or APPDATA not set)")?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create config directory {}", parent.display()))?;
    }
    fs::write(&path, zahirscan::DEFAULT_CONFIG_TOML)
        .with_context(|| format!("Failed to write config to {}", path.display()))?;
    println!("Wrote default config to {}", path.display());
    Ok(())
}
