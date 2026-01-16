//! Basic example of using ZahirScan as a library

use std::time::Instant;
use zahirscan::{Config, extract_templates, initial_file_scan, results::OutputMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration (or use defaults)
    let mut config = Config::load().unwrap_or_else(|_| Config::default());
    println!("Using binary name: {}", config.binary_name);
    println!("Max workers: {}", config.max_workers);

    // Example: Parse a log file
    let path = "test-data/data/logs/system.log";
    let output = "/tmp/example.zahirscan.json";

    // Phase 1: Initial file scan
    let start = Instant::now();
    let mut stats = initial_file_scan(path)?;
    println!(
        "Phase 1 complete: {} lines, {} bytes",
        stats.line_count, stats.byte_count
    );

    // Phase 2: Template mining (if not binary)
    if !stats.is_binary {
        match extract_templates(&stats, &config) {
            Ok(mining_result) => {
                stats.mining_result = Some(mining_result);
                println!(
                    "Template mining complete: {} templates found",
                    stats.mining_result.as_ref().unwrap().templates.len()
                );
            }
            Err(e) => eprintln!("Error extracting templates: {}", e),
        }
    }

    stats.duration = start.elapsed();

    // Write output (Mode 1: templates only by default)
    // To use Mode 2 (full metadata), set: config.output_mode = OutputMode::Full;
    stats.write_to_file(output, config.output_mode)?;
    println!("Successfully wrote output to: {}", output);
    println!("Output mode: {:?}", config.output_mode);

    // Example: Using Mode 2 (full metadata)
    let output_full = "/tmp/example_full.zahirscan.json";
    config.output_mode = OutputMode::Full;
    stats.write_to_file(output_full, config.output_mode)?;
    println!("Full metadata output written to: {}", output_full);

    Ok(())
}
