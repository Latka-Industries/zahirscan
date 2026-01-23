//! Basic example of using ZahirScan as a library
//!
//! Run with: `cargo run --example basic_usage -- <input-file>`
//!
//! This example demonstrates the simple `extract_schema()` API for library usage.

use std::env;
use zahirscan::{OutputMode, extract_schema};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get input file from command line args
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: cargo run --example basic_usage -- <input-file>");
        eprintln!("Example: cargo run --example basic_usage -- README.md");
        std::process::exit(1);
    }
    let input_file = &args[1];

    println!("Processing file: {}\n", input_file);

    // Simple API: extract_schema handles all the complexity internally
    // Use OutputMode::Full to get complete metadata, or OutputMode::Templates for minimal output
    let outputs = extract_schema(input_file, OutputMode::Full)?;
    println!("Processing complete: {} file(s) processed\n", outputs.len());

    // Access Output objects directly
    let Some(output) = outputs.first() else {
        println!("No output generated");
        return Ok(());
    };

    println!("Output summary:");
    println!("  Templates: {}", output.templates.len());

    // Show template patterns
    if !output.templates.is_empty() {
        println!("\n  Template patterns:");
        for (i, template) in output.templates.iter().take(5).enumerate() {
            println!(
                "    {}. {} ({} matches)",
                i + 1,
                template.pattern,
                template.count
            );
        }
        let remaining = output.templates.len().saturating_sub(5);
        if remaining > 0 {
            println!("    ... and {} more", remaining);
        }
    }

    // Show compression stats
    if let Some(compression) = &output.compression {
        println!(
            "\n  Compression: {:.2}% reduction ({} -> {} tokens)",
            compression.reduction_percent,
            compression.original_tokens,
            compression.compressed_tokens
        );
    }

    // Convert to pretty JSON - this shows all metadata automatically
    println!("\n--- Full Output (JSON) ---");
    for (i, output) in outputs.iter().enumerate() {
        let json = serde_json::to_string_pretty(output)?;
        if outputs.len() > 1 {
            println!("=== Output {} ===", i + 1);
        }
        println!("{}", json);
        if i < outputs.len() - 1 {
            println!();
        }
    }

    Ok(())
}
