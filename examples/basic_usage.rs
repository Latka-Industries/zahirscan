//! Basic example of using ZahirScan as a library
//!
//! Run with: `cargo run --example basic_usage -- <input-file>`

use std::env;
use zahirscan::{Config, calculate_adaptive_chunking, phase1_scan, phase2_mining};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get input file from command line args
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: cargo run --example basic_usage -- <input-file>");
        eprintln!("Example: cargo run --example basic_usage -- README.md");
        std::process::exit(1);
    }
    let input_file = &args[1];

    // Load configuration (or use defaults)
    let config = Config::load().unwrap_or_else(|_| Config::default());
    println!("Using binary name: {}", config.binary_name);
    println!("Max workers: {}\n", config.max_workers);

    // Phase 1: Initial scan to prepare for template mining
    println!("Phase 1: Scanning file...");
    let tasks = phase1_scan(&[input_file.clone()], None, false, &config);
    println!("Phase 1 complete: {} file(s) scanned\n", tasks.len());

    // Calculate adaptive chunking based on Phase 1 stats
    let adaptive = calculate_adaptive_chunking(&tasks, config.max_workers, &config);

    // Phase 2: Template mining and processing
    // skip_file_write=true for library usage (no file I/O)
    println!("Phase 2: Mining templates...");
    let outputs = phase2_mining(tasks, &config, &adaptive, config.max_workers, true)?;
    println!("Phase 2 complete: {} file(s) processed\n", outputs.len());

    // Access Output objects directly (programmatic access)
    if let Some(output) = outputs.first() {
        println!("First output summary:");
        println!("  Templates: {}", output.templates.len());
        if let Some(ref compression) = output.compression {
            println!(
                "  Compression: {:.2}% reduction",
                compression.reduction_percent
            );
        }
    }

    // Convert to JSON strings if needed (optional - files are already JSON when written)
    println!("\n--- Converting to JSON strings (optional) ---");
    for (i, output) in outputs.iter().enumerate() {
        let json = serde_json::to_string_pretty(output)?;
        println!("=== JSON Output {} ===", i + 1);
        println!("{}", json);
        println!();
    }

    Ok(())
}
