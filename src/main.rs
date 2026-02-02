use anyhow::Context;
use clap::Parser;
use log::warn;
use std::fs;
use std::time::Instant;
use zahirscan::{
    calculate_adaptive_chunking, format_duration, phase1_scan, phase2_mining,
    print_progress_handler, setup,
};

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

/// Processed arguments with computed values
struct ProcessedArgs {
    paths: Vec<String>,
    output: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if matches!(args.subcommand, Some(SubCmd::Init)) {
        run_init()?;
        return Ok(());
    }

    let start = Instant::now();

    setup::build_logger(args.dev);

    let mut config = setup::load_config();
    setup::apply_cli_to_config(
        &mut config,
        args.progress,
        args.dev,
        args.full,
        args.redact,
        args.no_media,
    )?;

    let (paths, output) = setup::resolve_output_paths(args.input.clone(), args.output.clone())?;
    let processed_paths = ProcessedArgs { paths, output };

    // Set up rayon thread pool
    rayon::ThreadPoolBuilder::new()
        .num_threads(config.max_workers)
        .build_global()
        .unwrap_or_else(|_| {
            warn!("Failed to set thread pool, using default");
        });

    // Phase 1: Initial scan to prepare for template mining
    let phase1 = phase1_scan(
        &processed_paths.paths,
        processed_paths.output.as_deref(),
        &config,
    );

    // Calculate adaptive chunking based on Phase 1 stats
    let adaptive = calculate_adaptive_chunking(&phase1.tasks, config.max_workers, &config);

    // Phase 2: Template mining and write output (skip_file_write=false)
    let phase2 = phase2_mining(phase1.tasks, &config, &adaptive, false);
    let _outputs = phase2.outputs;

    let total_duration = start.elapsed();

    print_progress_handler(
        &format!("Total time: {}", format_duration(total_duration)),
        config.show_progress,
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
