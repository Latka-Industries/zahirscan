# ZahirScan: Template-Based Content Compression & Media Metadata Extraction

![Build](https://github.com/thicclatka/zahirscan/workflows/Build/badge.svg)
![Rust](https://img.shields.io/badge/rust-1.92.0-orange.svg)

> _"Others will dream that I am mad, while I dream of the Zahir."_ — [JL Borges, Labyrinths](https://bookshop.org/p/books/labyrinths-jorge-luis-borges/f14b472a366ed106?ean=9780811216999&next=t&)

A high-performance Rust CLI tool that extracts templates and patterns from unstructured content, converting them into compact structured formats while preserving essential information. Additionally provides comprehensive metadata extraction for media files.

> **Note**: This project is currently a work in progress, so use with caution.

## Overview

ZahirScan uses probabilistic template mining to extract essential structure and patterns from content. The tool automatically adapts to different content types:

- **Logs & Text**: Identifies static vs. dynamic tokens, groups similar log lines into templates, extracts structural patterns and repeated phrases
- **Media Files**: Automatically detects and extracts comprehensive metadata for images, videos, and audio

**Supported Formats**:

- **Logs**: Plain text logs, JSON-formatted logs, structured log files
- **Text Documents**: TXT, Markdown (MD), plain text content
- **CSV Files**: CSV
- **Images**: JPEG, PNG, GIF, WebP, BMP, TIFF
- **Videos**: MP4, MKV, AVI, MOV, WMV, FLV, WebM, M4V, 3GP, OGV
- **Audio**: MP3, FLAC, WAV, M4A, AAC, OGG, Opus, WMA, APE, DSD, DSF

All outputs reduce size by 80-95% compared to raw content while preserving essential information.

## Key Features

- **Template Mining**: Automatically identifies repeated patterns in logs/text and extracts them as templates with placeholders
- **Media Metadata**: Extracts comprehensive metadata for images, videos, and audio (dimensions, codecs, bitrates, etc.)
- **CSV Metadata**: Extracts row/column counts, column names, data types, delimiter, quote/escape characters, null percentages, unique counts, and type-specific statistics (numeric: min/max/mean/median/IQR/stdev, date: span/min/max, boolean: true percentage)
- **Writing Footprint**: For text/markdown files, provides vocabulary richness, sentence structure, and template diversity metrics
- **Zero-Copy Processing**: Uses memory-mapped files (`memmap2`) to handle files larger than available RAM
- **Adaptive Parallelization**: Automatically optimizes chunk sizes based on file statistics and CPU resources
- **Size Reduction**: Typically reduces content size by 80-95% while preserving essential information

## Installation

### Prerequisites

- **Rust** (stable toolchain)
- **ffprobe** (optional, for video/audio metadata extraction): `ffprobe` is distributed with FFmpeg. Install FFmpeg: `https://ffmpeg.org/download.html`

  > **Note**: If `ffprobe` is not installed, ZahirScan will still work for text, log, and image files. Video and audio files will be processed but metadata extraction will be skipped.

### Build

```bash
# Build from source
cargo build --release
```

## Usage

### Quickstart Examples

```bash
# Process log files
zahirscan -i app.log -o output/
zahirscan -i logs/*.log -o output/
zahirscan -i app.log -o output/ -f  # Full metadata mode

# Process text/markdown files (extracts templates and writing footprint)
zahirscan -i document.md -o output/
zahirscan -i docs/*.txt docs/*.md -o output/

# Extract image metadata (dimensions, format, compression, chroma subsampling)
zahirscan -i images/*.jpg images/*.png -o output/ -f

# Extract video metadata (requires ffprobe: codec, resolution, bitrate, frame_rate, etc.)
zahirscan -i videos/*.mp4 -o output/ -f

# Extract audio metadata (codec, bitrate, sample_rate, channels, bit_rate_mode for MP3)
zahirscan -i audio/*.mp3 -o output/ -f

# Extract CSV metadata (row/column counts, data types, statistics)
zahirscan -i data/*.csv -o output/ -f

# Process multiple file types at once
zahirscan -i logs/*.log docs/*.md images/*.jpg data/*.csv -o output/ -f

# Skip media metadata for faster processing
zahirscan -i logs/*.log -o output/ -n

# Redact file paths in output (privacy)
zahirscan -i sensitive.log -o output/ -f -r
```

### Command-Line Options

```bash
$ zahirscan --help
Text file and log file parser using probabilistic template mining

Usage: zahirscan [OPTIONS]

Options:
  -i, --input <INPUT>...
          Input file(s) to parse (can specify multiple)

  -o, --output <OUTPUT>
          Output folder path (defaults to temp file if not specified).
          Creates filename.zahirscan.out in the folder for each input file

  -f, --full
          Output mode: full metadata (for development/debugging).
          Default is templates-only mode (minimal JSON with templates, writing footprint, and media metadata)

  -d, --dev
          Development mode: enables debug logging.
          Default is production mode (info level only)

  -r, --redact
          Redact file paths in output (show only filename as ***/filename.ext).
          Useful for privacy when sharing output JSON

  -n, --no-media
          Skip media metadata extraction (audio, video, image).
          Faster processing when metadata is not needed

  -h, --help
          Print help
```

**Output formats:**

- **Mode 1 (Templates)**: Minimal JSON with template patterns & schema, writing footprint (for text/markdown), and media metadata (for images/videos/audio)
- **Mode 2 (Full)**: Mode 1 output plus:
  - File statistics (size, line count, processing time)
  - Size comparison (before/after)

### Configuration

See [`config.toml`](config.toml) for configuration.

**Adaptive Defaults:**

- `max_workers = 0` uses a sensible default based on CPU cores
- Phase 2 uses **adaptive chunking** based on Phase 1 file statistics (count/bytes/variance) and targets a neat multiple of `max_workers`
- No manual batching configuration is required for typical workloads

## Architecture

### Phase 1: Initial File Scan

- File format detection and statistics collection (line count, byte count, token count)
- Memory-mapped file access for text files (`memmap2`)
- Content type determination (log vs. text/markdown vs. media)
- Prepares tasks for Phase 2

### Phase 2: Template Mining and Metadata Extraction

- **Media Metadata**: Extracts metadata for images (via `image` crate), videos/audio (via `ffprobe`)
- **Template Mining**: Frequency-based analysis to identify static vs. dynamic fields, extracts patterns as templates
- **Tokenization**: Content-aware (whitespace for logs, JSON structure for JSON logs, sentence/paragraph for text/markdown)
- **Writing Footprint**: Calculates vocabulary richness, sentence structure, template diversity for text/markdown
- **Parallel Processing**: Single Rayon thread pool with adaptive chunk sizing based on Phase 1 statistics

## Security

ZahirScan implements non-invasive file operations:

- Path sanitization to prevent directory traversal attacks
- File existence validation before processing
- Read-only file access (never modifies source files)

## TODO

- [ ] Publish to crates.io
- [x] CSV metadata extraction (row/column counts, data types, delimiter detection, quote/escape characters, type-specific statistics)
- [ ] PDF metadata extraction (page count, document properties, PDF version)
- [ ] DOCX & Pages text extraction (plain text without formatting)

## License

This project is licensed under the MIT OR Apache-2.0 dual license - see the [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE) files for details.
