use anyhow::Context;
use clap::Parser;
use log::{debug, warn};
use std::fs;
use std::time::Instant;
use zahirscan::{
    Config, calculate_adaptive_chunking, format_duration, is_stderr_tty, phase1_scan,
    phase2_mining, print_progress_handler,
};

#[derive(Parser)]
#[command(name = "zahirscan")]
#[command(about = "Text file and log file parser using probabilistic template mining")]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Args {
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

/// Processed arguments with computed values
struct ProcessedArgs {
    paths: Vec<String>,
    output: Option<String>,
}

fn setup_logging(dev_mode: bool) {
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

fn setup_config_and_paths(args: &Args) -> anyhow::Result<(Config, ProcessedArgs)> {
    let mut config = Config::load().unwrap_or_else(|_| Config::default());

    // Determine if progress bars should be shown
    // Progress bars require: --progress flag, not in dev mode, and stderr is a TTY
    config.show_progress = match (args.progress, args.dev, is_stderr_tty()) {
        (true, true, _) => {
            debug!("--progress/-p flag was detected but will be disabled (debug mode)");
            false
        }
        (true, false, false) => {
            warn!("--progress/-p flag was detected but will be disabled (not a TTY)");
            false
        }
        (true, false, true) => true,
        (false, _, _) => false,
    };

    // Set output mode based on CLI flag
    if args.full {
        config.output_mode = zahirscan::results::OutputMode::Full;
    }

    // Set path redaction based on CLI flag
    config.redact_paths = args.redact;
    // Set skip media metadata based on CLI flag
    config.skip_media_metadata = args.no_media;

    // Process paths
    let processed_paths = process_args_for_paths_params(args)?;

    Ok((config, processed_paths))
}

fn process_args_for_paths_params(args: &Args) -> anyhow::Result<ProcessedArgs> {
    if args.input.is_empty() {
        return Err(anyhow::anyhow!("At least one input file is required"));
    }

    // If args.output is not empty: create the path, confirm it can be resolved
    // Otherwise None — use a temp dir per file.
    let output = match &args.output {
        Some(out) => {
            fs::create_dir_all(out)?;
            let canonical = fs::canonicalize(out).context("Failed to resolve output directory")?;
            Some(canonical.to_string_lossy().into_owned())
        }
        None => None,
    };

    Ok(ProcessedArgs {
        paths: args.input.clone(),
        output,
    })
}

fn main() -> anyhow::Result<()> {
    let start = Instant::now();

    let args = Args::parse();

    setup_logging(args.dev);

    let (config, processed_paths) = setup_config_and_paths(&args)?;

    // Set up rayon thread pool
    rayon::ThreadPoolBuilder::new()
        .num_threads(config.max_workers)
        .build_global()
        .unwrap_or_else(|_| {
            warn!("Failed to set thread pool, using default");
        });

    // Phase 1: Initial scan to prepare for template mining
    let tasks = phase1_scan(
        &processed_paths.paths,
        processed_paths.output.as_deref(),
        &config,
    );

    // Calculate adaptive chunking based on Phase 1 stats
    let adaptive = calculate_adaptive_chunking(&tasks, config.max_workers, &config);

    // Phase 2: Template mining and write output
    // skip_file_write=false (write files)
    let _outputs = phase2_mining(tasks, &config, &adaptive, false)?;

    let total_duration = start.elapsed();

    print_progress_handler(
        &format!("Total time: {}", format_duration(total_duration)),
        config.show_progress,
    );

    Ok(())
}
