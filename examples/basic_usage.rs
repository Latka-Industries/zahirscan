//! Basic example of using ZahirScan as a library

use zahirscan::parser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Example: Parse a log file
    let path = "test-data/data/logs/system.log";

    match parser::parse_file(path) {
        Ok(_) => println!("Successfully parsed {}", path),
        Err(e) => eprintln!("Error: {}", e),
    }

    Ok(())
}
