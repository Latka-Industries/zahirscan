# ZahirScan: Template-Based Content Compression

![CI](https://github.com/thicclatka/zahirscan/workflows/CI/badge.svg)
![Rust](https://img.shields.io/badge/rust-stable-orange.svg)

> _"Others will dream that I am mad, while I dream of the Zahir."_

A high-performance Rust CLI tool that extracts templates and patterns from unstructured content (logs, TXT files, Markdown, JSON logs), converting them into compact structured formats while preserving essential information.

## Overview

ZahirScan uses probabilistic template mining to extract essential structure and patterns from content. Whether processing structured logs (plain text, JSON), plain text files, or Markdown documents, it identifies repeated patterns and represents them compactly while preserving essential information.

**Supported Formats**:

- **Logs**: Plain text logs, JSON-formatted logs, structured log files
- **Text Documents**: TXT, Markdown (MD), plain text content

The tool automatically adapts to different content types:

- **Logs**: Identifies static vs. dynamic tokens, groups similar log lines into templates
- **Long-form Text**: Extracts structural patterns, chapter markers, and repeated phrases
- **Mixed Content**: Handles complex documents with varying structures

All outputs reduce size by 80-95% compared to raw content while preserving essential information.

## Key Features

- **Size Reduction**: Reduces content size by 80-95% while preserving essential information
- **Multi-Format Support**: Handles structured logs (plain text, JSON), plain text (TXT), and Markdown (MD)
- **Content-Aware Processing**: Automatically adapts analysis strategies based on detected content type (logs vs. long-form text)
- **Writing Footprint Analysis**: For text and markdown files, provides metrics including vocabulary richness, sentence structure, punctuation patterns, and template diversity
- **Structured Output**: Generates human-readable summaries and structured JSON
- **Zero-Copy Parsing**: Uses `memmap2` for memory-mapped file access, enabling efficient processing of multi-gigabyte files
- **Probabilistic Template Mining**: Automatically groups similar patterns into templates by identifying constant vs. dynamic tokens through frequency analysis
- **Multi-Level Parallel Processing**: Two-level parallelism with adaptive chunk sizing - files processed in parallel, with sentences/lines within each file also parallelized
- **Adaptive Performance Tuning**: Automatically optimizes concurrent file processing based on system resources to reduce contention and improve consistency
- **Zero-Config Operation**: Automatically infers structure without requiring manual schema definitions
- **Security-First Design**: Implements path sanitization and secure file handling

## Design Philosophy

### Template Extraction

Raw files contain significant redundancy. A 1MB log file may have thousands of similar lines, and long-form text often contains repeated structural patterns. ZahirScan addresses this by:

1. **Template Extraction**: For logs, identifies repeated patterns (e.g., `[TIMESTAMP] ERROR: Process PID failed with code CODE`) and represents them once with placeholders. For text, extracts structural patterns and repeated phrases.
2. **Schema Inference**: Automatically discovers which elements are static (log levels, separators, common phrases) vs. dynamic (timestamps, IDs, messages, unique content)
3. **Content-Type Adaptation**: Uses different strategies for structured logs vs. long-form text, optimizing compression for each content type
4. **Compact Representation**: Outputs structured data that preserves essential information while eliminating redundancy

### Why Probabilistic Analysis?

Regex-based parsers are brittle and require manual tuning for each format. ZahirScan uses probabilistic template mining to automatically identify:

- **For Logs**: Static tokens (log level prefixes, separators) vs. dynamic tokens (timestamps, process IDs, messages)
- **For Text**: Repeated phrases, chapter markers, structural patterns, and content organization
- **Schema patterns**: Common structures across content, whether log lines or text paragraphs

This approach adapts to different content types automatically, making it ideal for diverse use cases from system diagnostics to document analysis.

### Why Zero-Copy?

Traditional parsers load entire files into memory, which becomes impractical for multi-gigabyte files (whether logs or long-form text). ZahirScan uses memory-mapped files (`memmap2`) to access file contents directly from the operating system's page cache, eliminating unnecessary memory copies and enabling efficient processing of files larger than available RAM. This is especially critical when processing large text files or extensive log archives.

## Performance

ZahirScan is designed for both speed and efficiency. Benchmarks show:

- **Processing Speed**: Can analyze 4GB of content (logs or text) in approximately 1.2 seconds on modern hardware
- **Batch Processing**: Processes 200+ files in under 1 minute with adaptive parallelization
- **Size Reduction**: Typically reduces content size by 80-95% while preserving essential information
- **Memory Efficiency**: Uses memory-mapped files to handle files larger than available RAM
- **Adaptive Parallelization**: Automatically optimizes chunk sizes and concurrent file processing based on work complexity and system resources
- **Content-Type Handling**: Efficiently processes both structured logs and unstructured long-form text

## Installation

```bash
# Build from source
cargo build --release

# The binary will be available at target/release/zahirscan
```

## Usage

```bash
# Extract templates from plain text log file
zahirscan --path /path/to/logfile.log

# Extract templates from JSON-formatted logs
zahirscan --path /path/to/logs.json

# Extract templates from plain text file
zahirscan --path /path/to/document.txt

# Extract templates from Markdown file
zahirscan --path /path/to/document.md

# Specify content type explicitly (optional, auto-detected by default)
zahirscan --path /path/to/file.txt --content-type log
zahirscan --path /path/to/file.txt --content-type text
zahirscan --path /path/to/file.json --content-type log
```

**Output formats:**

- Human-readable summary with template patterns
- Structured JSON output
- Size comparison (before/after)
- Inferred schema with field definitions
- Data integrity score
- Writing footprint metrics (for text/markdown files: vocabulary richness, sentence structure, punctuation patterns, template diversity, entropy)

### Configuration

ZahirScan uses `config.toml` for configuration (optional - works out of the box with sensible defaults):

```toml
[concurrency]
# Maximum number of parallel workers (defaults to num_cpus - 1 if set to 0)
max_workers = 0

# Maximum number of files to process concurrently
# Set to 0 for adaptive default: max(4, min(8, max_workers / 2))
# This automatically balances throughput and reduces contention
# Or specify a value (recommended: 4-8 for large batches)
max_concurrent_files = 0
```

**Adaptive Defaults:**

- `max_concurrent_files = 0` automatically calculates optimal batch size based on CPU cores
- Chunk sizes are automatically tuned based on work complexity (light/moderate/heavy operations)
- No configuration needed for optimal performance on most systems

## Architecture

### Phase 1: Initial File Scan

- Secure file path handling with sanitization
- File format detection and handling:
  - **Plain text (TXT, MD)**: Direct memory-mapped access using `memmap2`
  - **JSON logs**: JSON parsing to extract log entries
- Collects file statistics (line count, byte count, token count)
- Determines content type (log vs. text/markdown)
- Prepares tasks for template mining phase

### Phase 2: Template Mining and Output

- **Content-Type Detection**: Identifies logs vs. long-form text, format detection (plain text, JSON, Markdown)
- **Tokenization**: Configurable delimiters:
  - **Plain text logs**: Whitespace delimiters
  - **JSON logs**: JSON structure parsing, then field-level template mining
  - **Markdown/Text**: Sentence/paragraph delimiters with markdown structure awareness
- **Probabilistic Template Mining**:
  - Frequency-based analysis to identify static vs. dynamic fields
  - Automatic categorization of fields:
    - **Logs (plain text or JSON)**: Timestamp, Category, ProcessID, MessageTemplate
    - **Text/Markdown**: Chapter markers, heading patterns, list structures, repeated phrases, content organization
  - Writing footprint calculation for text/markdown files (vocabulary richness, sentence structure, punctuation patterns, template diversity, entropy metrics, SVO analysis)
- **Output Generation**:
  - Structured JSON output matching inferred schema (adapts to content type)
  - Data integrity scoring (1.0 - unparseable_lines / total_lines for logs, coherence metrics for text)
  - Anomaly detection and reporting (error patterns in logs, structural inconsistencies in text)
- **Parallel Processing**:
  - **Two-Level Parallelism**:
    - **File-level**: Multiple files processed concurrently (configurable via `max_concurrent_files`)
    - **Within-file**: Sentences/lines processed in parallel using `rayon`
  - **Adaptive Chunk Sizing**: Automatically optimizes chunk sizes based on:
    - Collection size (number of sentences/lines)
    - Work complexity (light/moderate/heavy operations)
    - Number of available CPU cores
  - **Batched File Processing**: Processes files in optimal batches to reduce thread contention and improve consistency
  - **Work Complexity Classification**:
    - **Light**: Simple tokenization + hash map operations (logs)
    - **Moderate**: Tokenization + n-gram extraction + pattern matching (text/markdown)
    - **Heavy**: Complex operations requiring smaller chunks for better load balancing

## Security

ZahirScan implements non-invasive file operations:

- Path sanitization to prevent directory traversal attacks
- File existence validation before processing
- Read-only file access (never modifies source files)

## Development

```bash
# Run tests
cargo test

# Run with debug output
cargo run --release -- --path example.log

# Format code
cargo fmt

# Lint code
cargo clippy
```

## License

This project is licensed under the MIT OR Apache-2.0 dual license - see the [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE) files for details.
