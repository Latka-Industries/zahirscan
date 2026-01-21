# ZahirScan: Template-Based Content Compression & Media Metadata Extraction

![CI](https://github.com/thicclatka/zahirscan/workflows/CI/badge.svg)
![Rust](https://img.shields.io/badge/rust-stable-orange.svg)

> _"Others will dream that I am mad, while I dream of the Zahir."_

A high-performance Rust CLI tool that extracts templates and patterns from unstructured content (logs, TXT files, Markdown, JSON logs), converting them into compact structured formats while preserving essential information. Additionally provides comprehensive metadata extraction for media files (images, videos, audio).

> **Note**: This project is currently a work in progress and is almost ready for primetime. After a bit more refinement, it will be made publicly available.

## Overview

ZahirScan uses probabilistic template mining to extract essential structure and patterns from content. Whether processing structured logs (plain text, JSON), plain text files, or Markdown documents, it identifies repeated patterns and represents them compactly while preserving essential information.

**Supported Formats**:

- **Logs**: Plain text logs, JSON-formatted logs, structured log files
- **Text Documents**: TXT, Markdown (MD), plain text content
- **Images**: JPEG, PNG, GIF, WebP, BMP, TIFF (extracts dimensions, format, compression, chroma subsampling, aspect ratio)
- **Videos**: MP4, MKV, AVI, MOV, WMV, FLV, WebM, M4V, 3GP, OGV (extracts comprehensive MediaInfo-like metadata: codec, resolution, bitrate, frame rate, audio tracks, etc.)
- **Audio**: MP3, FLAC, WAV, M4A, AAC, OGG, Opus, WMA, APE, DSD, DSF (extracts codec, bitrate, sample rate, channels, duration, etc.)

The tool automatically adapts to different content types:

- **Logs**: Identifies static vs. dynamic tokens, groups similar log lines into templates
- **Long-form Text**: Extracts structural patterns, chapter markers, and repeated phrases
- **Mixed Content**: Handles complex documents with varying structures

All outputs reduce size by 80-95% compared to raw content while preserving essential information.

## Key Features

### Content Analysis

- **Size Reduction**: Reduces content size by 80-95% while preserving essential information
- **Multi-Format Support**: Handles structured logs (plain text, JSON), plain text (TXT), and Markdown (MD)
- **Content-Aware Processing**: Automatically adapts analysis strategies based on detected content type (logs vs. long-form text)
- **Writing Footprint Analysis**: For text and markdown files, provides metrics including vocabulary richness, sentence structure, punctuation patterns, and template diversity
- **Probabilistic Template Mining**: Automatically groups similar patterns into templates by identifying constant vs. dynamic tokens through frequency analysis

### Media Metadata Extraction

- **Image Metadata**: Extracts dimensions, format, compression info, chroma subsampling, aspect ratio, and color information
- **Video Metadata**: Comprehensive MediaInfo-like extraction including codec (profile/level), resolution, bitrate, frame rate, audio tracks, color space, scan type, and more
- **Audio Metadata**: Extracts codec, bitrate, sample rate, channels, duration, container format, and stream information
- **Universal Media Support**: Uses `ffprobe` for video/audio and native Rust crates for images, providing consistent metadata extraction across formats

### Performance & Architecture

- **Zero-Copy Parsing**: Uses `memmap2` for memory-mapped file access, enabling efficient processing of multi-gigabyte files
- **Parallel Processing**: Uses a single Rayon thread pool sized by `max_workers`, with adaptive chunk sizing in Phase 2 to keep all workers busy
- **Adaptive Performance Tuning**: Automatically tunes chunk sizes based on Phase 1 file statistics to reduce contention and improve consistency
- **Structured Output**: Generates human-readable summaries and structured JSON
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
- **Adaptive Parallelization**: Automatically optimizes chunk sizes based on Phase 1 file statistics and available CPU resources
- **Content-Type Handling**: Efficiently processes both structured logs and unstructured long-form text

## Installation

### Prerequisites

- **Rust** (stable toolchain)
- **ffprobe** (optional, for video/audio metadata extraction): Required only if you want to extract metadata from video or audio files. Install via your system package manager:
  - macOS: `brew install ffmpeg`
  - Ubuntu/Debian: `sudo apt-get install ffmpeg`
  - Fedora: `sudo dnf install ffmpeg`

  > **Note**: If `ffprobe` is not installed, ZahirScan will still work for text, log, and image files. Video and audio files will be processed but metadata extraction will be skipped.

### Build

```bash
# Build from source
cargo build --release

# The binary will be available at target/release/zahirscan
```

## Usage

```bash
$ zahirscan --help
Text file and log file parser using probabilistic template mining

Usage: zahirscan [OPTIONS]

Options:
  -p, --path <PATH>...
          Path(s) to the file(s) to parse (can specify multiple)

  -o, --output <OUTPUT>
          Output file path or folder (defaults to temp file if not specified)
          If folder, creates filename.zahirscan.out for each input file

      --full
          Output mode: full metadata (for development/debugging)
          Default is templates-only mode (minimal JSON for AI consumption)

      --dev
          Development mode: enables debug logging
          Default is production mode (info level only)

  -h, --help
          Print help
```

**Output formats:**

- **Mode 1 (Templates)**: Minimal JSON with templates and writing footprint (optimized for AI consumption)
- **Mode 2 (Full)**: Complete metadata including:
  - Template patterns and inferred schema
  - File statistics (size, line count, processing time)
  - Writing footprint metrics (for text/markdown: vocabulary richness, sentence structure, punctuation patterns, template diversity, entropy)
  - Media metadata (for images/videos/audio: comprehensive technical metadata)
- Size comparison (before/after) for text files
- Data integrity score

### Configuration

ZahirScan uses `config.toml` for configuration (optional - works out of the box with sensible defaults):

```toml
[concurrency]
# Maximum number of parallel workers (defaults to a sensible value if set to 0)
max_workers = 0

[adaptive_chunking]
# File size thresholds used by adaptive chunking heuristics
small_file_threshold_bytes = 262144     # 256KB
large_file_threshold_bytes = 1048576    # 1MB
```

**Adaptive Defaults:**

- `max_workers = 0` uses a sensible default based on CPU cores
- Phase 2 uses **adaptive chunking** based on Phase 1 file statistics (count/bytes/variance) and targets a neat multiple of `max_workers`
- No manual batching configuration is required for typical workloads

## Architecture

### Phase 1: Initial File Scan

- Secure file path handling with sanitization
- File format detection and handling:
  - **Plain text (TXT, MD)**: Direct memory-mapped access using `memmap2`
  - **JSON logs**: JSON parsing to extract log entries
  - **Images**: Fast format detection (metadata extraction in Phase 2)
  - **Videos/Audio**: Format detection (metadata extraction in Phase 2 via `ffprobe`)
- Collects file statistics (line count, byte count, token count)
- Determines content type (log vs. text/markdown vs. media)
- Prepares tasks for template mining/metadata extraction phase

### Phase 2: Template Mining and Metadata Extraction

- **Content-Type Detection**: Identifies logs vs. long-form text vs. media files, format detection (plain text, JSON, Markdown, image formats, video/audio containers)
- **Media Metadata Extraction**:
  - **Images**: Extracts metadata using native Rust `image` crate (dimensions, format, compression, chroma subsampling)
  - **Videos**: Comprehensive metadata via `ffprobe` (codec info, resolution, bitrate, frame rate, audio tracks, color space, etc.)
  - **Audio**: Metadata extraction via `ffprobe` (codec, bitrate, sample rate, channels, duration, container format)
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
  - **Single Rayon thread pool** sized by `max_workers`
  - **Adaptive Chunk Sizing (Phase 2)**:
    - Uses Phase 1 stats (file count, total bytes, mean, variance) to pick a target number of chunks
    - Targets a **neat multiple** of `max_workers` to keep all workers busy while minimizing overhead
    - Chunk sizing is based on byte distribution (aiming for roughly equal work per chunk)
  - **Within-file parallelism**: For large text-like files, lines/sentences may be processed in parallel

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
