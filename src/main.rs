use clap::Parser;
use log::{debug, warn};
use std::time::Instant;
use zahirscan::{Config, calculate_adaptive_chunking, format_duration, phase1_scan, phase2_mining};

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
    /// Default is production mode (info level only)
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
}

/// Processed arguments with computed values
struct ProcessedArgs {
    paths: Vec<String>,
    output: Option<String>,
    output_is_dir: bool,
}

fn process_args(args: &Args) -> anyhow::Result<ProcessedArgs> {
    if args.input.is_empty() {
        return Err(anyhow::anyhow!("At least one input file is required"));
    }

    // Output path is always treated as a directory
    let output_is_dir = args.output.is_some();

    // If output is a directory, ensure it exists
    if let Some(ref output) = args.output
        && output_is_dir
    {
        std::fs::create_dir_all(output)?;
    }

    Ok(ProcessedArgs {
        paths: args.input.clone(),
        output: args.output.clone(),
        output_is_dir,
    })
}

fn main() -> anyhow::Result<()> {
    let start = Instant::now();

    let args = Args::parse();

    // Initialize logger based on dev mode
    if args.dev {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Debug)
            .init();
    } else {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Info)
            .init();
    }

    let mut config = Config::load().unwrap_or_else(|_| Config::default());

    // Set output mode based on CLI flag
    if args.full {
        config.output_mode = zahirscan::results::OutputMode::Full;
    }

    // Set path redaction based on CLI flag
    config.redact_paths = args.redact;
    // Set skip media metadata based on CLI flag
    config.skip_media_metadata = args.no_media;

    let processed = process_args(&args)?;

    // Set up rayon thread pool
    rayon::ThreadPoolBuilder::new()
        .num_threads(config.max_workers)
        .build_global()
        .unwrap_or_else(|_| {
            warn!("Failed to set thread pool, using default");
        });

    // Phase 1: Initial scan to prepare for template mining
    let tasks = phase1_scan(
        &processed.paths,
        processed.output.as_deref(),
        processed.output_is_dir,
        &config,
    );

    // Calculate adaptive chunking based on Phase 1 stats
    let adaptive = calculate_adaptive_chunking(&tasks, config.max_workers, &config);

    // Phase 2: Template mining and write output
    // skip_file_write=false (write files)
    let _outputs = phase2_mining(tasks, &config, &adaptive, config.max_workers, false)?;

    let total_duration = start.elapsed();
    debug!("Total time: {}", format_duration(total_duration));

    Ok(())
}
