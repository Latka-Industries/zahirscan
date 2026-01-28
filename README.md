# ZahirScan: Template-Based Content Compression & Metadata Extraction

[![Crates.io](https://img.shields.io/crates/v/zahirscan.svg)](https://crates.io/crates/zahirscan)
[![docs.rs](https://img.shields.io/docsrs/zahirscan)](https://docs.rs/zahirscan)
![Build](https://github.com/thicclatka/zahirscan/workflows/Build/badge.svg)
![Rust](https://img.shields.io/badge/rust-1.92.0-orange.svg)

> _"Others will dream that I am mad, while I dream of the Zahir."_ — [JL Borges, Labyrinths](https://bookshop.org/p/books/labyrinths-jorge-luis-borges/f14b472a366ed106?ean=9780811216999&next=t&)

A high-performance Rust CLI tool that extracts templates and patterns from unstructured content, converting them into compact structured formats while preserving essential information. Provides comprehensive metadata extraction for many file types: media (images, video, audio), documents (DOCX, XLSX, PDF), databases (SQLite), settings (TOML, YAML, INI, XML), archives (ZIP, TAR), and code/scripts (via linguist).

## Overview

ZahirScan uses probabilistic template mining to extract essential structure and patterns from content, and extracts metadata for the formats below.

**Supported Formats**:

- **Logs**: Plain text logs, JSON-formatted logs, structured log files
- **Text Documents**: TXT, Markdown (MD), plain text content
- **Documents**: DOCX, XLSX, PDF
- **Databases**: SQLite (.db, .sqlite, .sqlite3)
- **Settings**: INI (.ini, .cfg), TOML (.toml, .lock), YAML (.yaml, .yml), XML (.xml)
- **Structured**: CSV, HTML (.html, .htm)
- **Archives**: ZIP (.zip); TAR and compressed TAR (`.tar`, `.tar.gz`, `.tgz`, `.tar.bz2`, `.tar.xz`).
- **Code/Scripts**: Detected via [linguist](https://github.com/github-linguist/linguist) (e.g. .py, .rs, .js, .ts, .sh, Makefile, Dockerfile).
- **Images**: JPEG, PNG, GIF, WebP, BMP, TIFF
- **Videos**: MP4, MKV, AVI, MOV, WMV, FLV, WebM, M4V, 3GP, OGV
- **Audio**: MP3, FLAC, WAV, M4A, AAC, OGG, Opus, WMA, APE, DSD, DSF

All outputs reduce size by 80-95% compared to raw content while preserving essential information.

## Key Features

- **Template Mining**: Automatically identifies repeated patterns in logs/text and extracts them as templates with placeholders
- **Zero-Copy Processing**: Uses memory-mapped files (`memmap2`) to handle files larger than available RAM
- **Adaptive Parallelization**: Automatically optimizes chunk sizes based on file statistics and CPU resources
- **Size Reduction**: Typically reduces content size by 80-95% while preserving essential information

### Metadata extraction by format

| **Metadata**         | **Extracts**                                                                                                                                                                                                                                                                                                    |
| -------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Media                | Dimensions, codecs, bitrates for images, videos, audio                                                                                                                                                                                                                                                          |
| Document             | DOCX: word count, character count, paragraph count, title, author, creation/modification dates, revision. XLSX: sheet count, sheet names, row/column counts per sheet, core properties. PDF: page count, title, author, subject, creator, producer, creation/modification dates, PDF version, encryption status |
| CSV                  | Row/column counts, column names, data types, delimiter, quote/escape characters, null percentages, unique counts; type-specific statistics (numeric: min/max/mean/median/IQR/stdev, date: span/min/max, boolean: true percentage)                                                                               |
| SQLite               | Schema (tables, columns, types, constraints), primary keys, foreign keys, indexes, row counts, column statistics (null percentages, unique counts, numeric/text/boolean/blob/date)                                                                                                                              |
| TOML, YAML, INI, CFG | Recursive schema (scalar, table/mapping, array/sequence; INI: section→key→scalar, multi-line values), key count, max depth. TOML: section count. YAML: scalar/sequence/map counts. INI/.cfg: section count, comment count                                                                                       |
| Code/Scripts         | script_type (linguist + optional shebang), byte_count, line_count; BOM, line_ending, trailing_newline, max_line_length, blank_line_count, indentation (single-pass scan)                                                                                                                                        |
| ZIP                  | File count, entries (path, uncompressed/compressed size, detected type, modified, compression method), entry_type_counts; filters hidden OS files (e.g. \_\_MACOSX, .DS_Store, Thumbs.db)                                                                                                                       |
| Archive (TAR family) | File count, entries (path, size), compressed_size, uncompressed_size                                                                                                                                                                                                                                            |
| XML                  | Recursive schema (root→children with attributes; repeated siblings as arrays with union of all children), element count, attribute count, max depth, has_namespaces                                                                                                                                             |
| HTML                 | Title, meta description, lang, charset, viewport; link/stylesheet/script/style counts; heading (h1–h6) and element counts (img, table, form, p, ul, ol, iframe, article, nav, section, header, footer, main); plain_text_len, word_count; writing footprint from body text                                      |
| Writing Footprint    | For text/markdown/html: vocabulary richness, sentence structure, template diversity, punctuation metrics                                                                                                                                                                                                        |

## Installation

```bash
# library
cargo add zahirscan

# CLI (from crates.io)
cargo install zahirscan

# Source archive (from GitHub Releases)
# Download from: https://github.com/thicclatka/zahirscan/releases
```

**Note**: `ffprobe` (from FFmpeg) is optional but required for video/audio metadata extraction.

Documentation: [docs.rs/zahirscan](https://docs.rs/zahirscan)

## Usage

### CLI

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
          Default is production mode (info level only).
          This disables progress bars if enabled

  -r, --redact
          Redact file paths in output (show only filename as ***/filename.ext).
          Useful for privacy when sharing output JSON

  -n, --no-media
          Skip media metadata extraction (audio, video, image).
          Faster processing when metadata is not needed
  -p, --progress
          Show progress bars during processing.
          This is ignored if dev mode is enabled.

  -h, --help
          Print help
```

**Output formats:**

- **Mode 1 (Templates)**: Minimal JSON with template patterns & schema, writing footprint (for text/markdown), media metadata (for images/videos/audio), code metadata (for code/script files), and document metadata (for DOCX/XLSX)
- **Mode 2 (Full)**: Mode 1 output plus:
  - File statistics (size, line count, processing time)
  - Size comparison (before/after)

### Library Usage

ZahirScan can be used as a Rust library to extract schemas (templates and metadata) from files programmatically.

#### Basic Example

The `extract_schema()` function accepts flexible input types via the `ToPathIter` trait:

- Single file: `&str`, `&String`, or `String`
- Multiple files: `&[&str]`, `Vec<&str>`, `&[String]`, `Vec<String>`, or arrays like `[&str; N]`

For a complete working example, see [`examples/basic_usage.rs`](examples/basic_usage.rs). Run it with:

```bash
cargo run --example basic_usage -- <input-file>
```

#### Output Schema

The `extract_schema()` function returns `Result<Vec<Output>>`. Each `Output` object contains:

**Always present (both modes):**

- `templates: Vec<Template>` - Extracted template patterns
- `source: String` - Source file path
- `file_type: String` - Detected file type (e.g., "Log", "Text", "Code", "Sqlite", "Image")

**Mode 2 (Full) only (all optional):**

- `line_count: Option<usize>` - Number of lines in file
- `byte_count: Option<usize>` - File size in bytes
- `token_count: Option<usize>` - Estimated token count
- `processing_time_ms: Option<f64>` - Processing duration
- `is_binary: Option<bool>` - Whether file is binary
- `compression: Option<CompressionStats>` - Compression metrics

**Conditional Fields (present when applicable):**

- `writing_footprint: Option<WritingFootprint>` - Writing analysis for text/markdown files
- `image_metadata: Option<ImageMetadata>` - Image metadata (dimensions, format, etc.)
- `video_metadata: Option<VideoMetadata>` - Video metadata (codec, resolution, bitrate, etc.)
- `audio_metadata: Option<AudioMetadata>` - Audio metadata (codec, bitrate, sample rate, etc.)
- `code_metadata: Option<CodeMetadata>` - Code/script metadata (script_type, byte_count, line_count, BOM, line_ending, trailing_newline, max_line_length, blank_line_count, indentation)
- `csv_metadata: Option<CsvMetadata>` - CSV metadata (row/column counts, data types, statistics)
- `sqlite_metadata: Option<SqliteMetadata>` - SQLite database metadata (schema, tables, columns, indexes, statistics)
- `toml_metadata: Option<TomlMetadata>` - TOML config metadata (recursive schema, section/key counts, depth)
- `zip_metadata: Option<ZipMetadata>` - ZIP archive metadata (entries, sizes, detected types, compression; hidden OS files filtered)
- `archive_metadata: Option<ArchiveMetadata>` - TAR / compressed TAR. Plain `.tar`: format, file_count, entries, compressed_size, uncompressed_size. Compressed (`.tar.gz`/`.xz`/`.bz2`): zero-copy, no decompression—format, compressed_size; `.tar.gz` also has uncompressed_size from gzip trailer; file_count and entries are `None`.
- `xml_metadata: Option<XmlMetadata>` - XML structure metadata (recursive schema, element/attribute counts, namespaces)
- `html_metadata: Option<HtmlMetadata>` - HTML metadata (title, meta, lang, charset, element counts, plain text/word count, writing footprint from body)
- `yaml_metadata: Option<YamlMetadata>` - YAML metadata (recursive schema, key count, max depth, scalar/sequence/map counts)
- `ini_metadata: Option<IniMetadata>` - INI/.cfg metadata (recursive schema section→key→scalar, section/key/comment counts, max depth, multi-line values)
- `pdf_metadata: Option<PdfMetadata>` - PDF metadata (page count, document properties, etc.)
- `docx_metadata: Option<DocumentMetadata>` - DOCX/XLSX metadata (word count, sheet count, title, author, dates, etc.)
- `pptx_metadata: Option<PptxMetadata>` - PPTX metadata (slide count, core properties, etc.)
- `epub_metadata: Option<EpubMetadata>` - EPUB metadata (title, creator, language, chapter count, etc.)

#### Template Structure

Each `Template` contains:

- `pattern: String` - Template pattern with placeholders (e.g., `"[DATE] [TIME] ERROR: [MESSAGE]"`)
- `count: usize` - Number of lines matching this template
- `examples: BTreeMap<String, Vec<String>>` - Example values for each placeholder

#### Writing Footprint Structure

`WritingFootprint` (for text/markdown files) contains:

- `vocabulary_richness: f64` - Unique words / total words (0.0-1.0)
- `avg_sentence_length: f64` - Average sentence length in words
- `punctuation: PunctuationMetrics` - Punctuation usage statistics
- `template_diversity: usize` - Number of unique template patterns
- `avg_entropy: f64` - Average entropy across templates (0.0-1.0)
- `svo_analysis: Option<SVOAnalysis>` - Sentence structure analysis

#### Compression Stats Structure

`CompressionStats` contains:

- `original_tokens: usize` - Original content token count
- `compressed_tokens: usize` - Compressed template token count
- `reduction_percent: f64` - Percentage reduction (0.0-100.0)

### Configuration

See [`config.toml`](config.toml) for configuration.

**Adaptive Defaults:**

- `max_workers = 0` uses a sensible default based on CPU cores
- Phase 2 uses **adaptive chunking** based on Phase 1 file statistics (count/bytes/variance) and targets a neat multiple of `max_workers`
- No manual batching configuration is required for typical workloads

**File filtering (`[filter]`):**

- `ignore_patterns`: skip files whose basename matches (exact: `.DS_Store`, `Thumbs.db`; suffix: `*.swp`, `*~`; prefix: `prefix*`)
- `ignore_hidden_files = true`: skip Unix hidden files (basename starts with `.`)

## Architecture

### Phase 1: Initial File Scan

- File format detection and statistics collection (line count, byte count, token count)
- Memory-mapped file access for text files (`memmap2`)
- Content type determination (log vs. text/markdown vs. media)
- Prepares tasks for Phase 2

### Phase 2: Template Mining and Metadata Extraction

- **Metadata extraction** (media, document, database, settings, structured, archives, code): see the [Metadata extraction by format](#metadata-extraction-by-format) table above for what is extracted per format.
- **Template Mining**: Frequency-based analysis to identify static vs. dynamic fields, extracts patterns as templates
- **Tokenization**: Content-aware (whitespace for logs, JSON structure for JSON logs, sentence/paragraph for text/markdown)
- **Writing Footprint**: Calculates vocabulary richness, sentence structure, punctuation metrics, template diversity for text/markdown
- **Parallel Processing**: Single Rayon thread pool with adaptive chunk sizing based on Phase 1 statistics

## Security

ZahirScan implements non-invasive file operations:

- Path sanitization to prevent directory traversal attacks
- File existence validation before processing
- Read-only file access (never modifies source files)

## TODO

- [ ] Word universe for enhanced writing analysis (per-document vocabulary corpus with frequency distributions, word length statistics, and visualization data)
- [ ] Improve template extraction for short literary texts (adaptive thresholds and pattern similarity merging for better pattern recognition in short documents)
- [ ] Shared lightweight NLP utility layer for logs + writing analysis (normalization/tokenization/stats/redaction; optional similarity/embeddings later)

- [ ] (Optional) Security hardening: output path validation + symlink checks

## License

This project is licensed under the MIT OR Apache-2.0 dual license - see the [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE) files for details.
