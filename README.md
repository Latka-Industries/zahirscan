# ZahirScan: Template-Based Content Compression & Metadata Extraction

[![Crates.io](https://img.shields.io/crates/v/zahirscan.svg)](https://crates.io/crates/zahirscan)
[![docs.rs](https://img.shields.io/docsrs/zahirscan)](https://docs.rs/zahirscan)
![Build](https://github.com/thicclatka/zahirscan/workflows/Build/badge.svg)
![Rust](https://img.shields.io/badge/rust-1.93-orange.svg)

> _"Others will dream that I am mad, while I dream of the Zahir."_ — [JL Borges, Labyrinths](https://bookshop.org/p/books/labyrinths-jorge-luis-borges/f14b472a366ed106?ean=9780811216999&next=t&)

A high-performance Rust CLI that uses probabilistic template mining to extract structure and patterns from content, and metadata for the formats below.

**Supported formats**:

- **Logs**: Plain text logs, JSON-formatted logs, structured log files
- **Text Documents**: TXT, Markdown (MD), plain text content
- **Documents**: DOCX, XLSX, PPTX, PDF
- **Databases**: SQLite (.db, .sqlite, .sqlite3)
- **Settings**: INI (.ini, .cfg), TOML (.toml, .lock), YAML (.yaml, .yml), XML (.xml)
- **Structured**: CSV, HTML (.html, .htm)
- **Archives**: ZIP (.zip); TAR and compressed TAR (`.tar`, `.tar.gz`, `.tgz`, `.tar.bz2`, `.tar.xz`).
- **Code/Scripts**: Detected via [linguist](https://github.com/github-linguist/linguist) (e.g. .py, .rs, .js, .ts, .sh, Makefile, Dockerfile).
- **Images**: JPEG, PNG, GIF, WebP, BMP, TIFF
- **Videos**: MP4, MKV, AVI, MOV, WMV, FLV, WebM, M4V, 3GP, OGV
- **Audio**: MP3, FLAC, WAV, M4A, AAC, OGG, Opus, WMA, APE, DSD, DSF

## Key Features

- **Template mining**: Identifies repeated patterns in logs/text and extracts them as templates with placeholders
- **Memory-mapped I/O**: Uses `memmap2` for large files; each path is opened once (Phase 1 mmap reused in Phase 2)
- **Adaptive parallelization**: Chunk sizes and worker usage tuned from Phase 1 statistics
- **Size reduction**: Typically 80–95% smaller than raw content while preserving structure and metadata

### Metadata extraction by format

| **Metadata**         | **Extracts**                                                                                                                                                                                                                                                                                                                                                        |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Media                | Dimensions, codecs, bitrates for images, videos, audio                                                                                                                                                                                                                                                                                                              |
| Document             | **DOCX**: word count, character count, paragraph count, title, author, creation/modification dates, revision. **XLSX**: sheet count, sheet names, row/column counts per sheet, core properties. **PPTX**: slide count, core properties. **PDF**: page count, title, author, subject, creator, producer, creation/modification dates, PDF version, encryption status |
| CSV                  | Row/column counts, column names, data types, delimiter, quote/escape characters, null percentages, unique counts; type-specific statistics (numeric: min/max/mean/median/IQR/stdev, date: span/min/max, boolean: true percentage)                                                                                                                                   |
| SQLite               | Schema (tables, columns, types, constraints), primary keys, foreign keys, indexes, row counts, column statistics (null percentages, unique counts, numeric/text/boolean/blob/date)                                                                                                                                                                                  |
| TOML, YAML, INI, CFG | Recursive schema (scalar, table/mapping, array/sequence; INI: section→key→scalar, multi-line values), key count, max depth. TOML: section count. YAML: scalar/sequence/map counts. INI/.cfg: section count, comment count                                                                                                                                           |
| Code/Scripts         | script_type (linguist + optional shebang), byte_count, line_count; BOM, line_ending, trailing_newline, max_line_length, blank_line_count, indentation (single-pass scan)                                                                                                                                                                                            |
| ZIP                  | File count, entries (path, uncompressed/compressed size, detected type, modified, compression method), entry_type_counts; filters hidden OS files (e.g. \_\_MACOSX, .DS_Store, Thumbs.db)                                                                                                                                                                           |
| Archive (TAR family) | File count, entries (path, size), compressed_size, uncompressed_size                                                                                                                                                                                                                                                                                                |
| XML                  | Recursive schema (root→children with attributes; repeated siblings as arrays with union of all children), element count, attribute count, max depth, has_namespaces                                                                                                                                                                                                 |
| HTML                 | Title, meta description, lang, charset, viewport; link/stylesheet/script/style counts; heading (h1–h6) and element counts (img, table, form, p, ul, ol, iframe, article, nav, section, header, footer, main); plain_text_len, word_count; writing footprint from body text                                                                                          |
| Writing Footprint    | For text/markdown/html: vocabulary richness, sentence structure, template diversity, punctuation metrics. Uses **two writing-analysis passes**: (1) exact-pattern grouping (n-gram/phrase-based); (2) shape fallback (group by sentence length + end punctuation) when pass 1 yields no templates                                                                   |

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

## Usage

### CLI

```bash
$ zahirscan --help
Template mining for text/logs and metadata extraction for media, documents, archives, and more

Usage: zahirscan [OPTIONS] [COMMAND]

Commands:
  init  Write default config to XDG config dir (~/.config/zahirscan/zahirscan.toml or equivalent)
  help  Print this message or the help of the given subcommand(s)

Options:
  -i, --input <INPUT>...  Input file(s) to parse (can specify multiple)
  -o, --output <OUTPUT>   Output folder path (defaults to temp file if not specified). Creates filename.zahirscan.out in the folder for each input file
  -f, --full              Output mode: full metadata (for development/debugging). Default is templates-only mode (minimal JSON with templates & writing footprint)
  -d, --dev               Development mode: enables debug logging. Default is production mode (info level only). This disables progress bars if enabled
  -r, --redact            Redact file paths in output (show only filename as ***/filename.ext). Useful for privacy when sharing output JSON
  -n, --no-media          Skip media metadata extraction (audio, video, image). Faster processing when metadata is not needed
  -p, --progress          Show progress bars during processing. This is ignored if dev mode is enabled
  -h, --help              Print help
  -V, --version           Print version
```

**Output formats:**

- **Mode 1 (Templates)**: Minimal JSON with template patterns & schema, writing footprint (for text/markdown), media metadata (for images/videos/audio), code metadata (for code/script files), and document metadata (for DOCX/XLSX/PPTX)
- **Mode 2 (Full)**: Mode 1 output plus:
  - File statistics (size, line count, processing time)
  - Size comparison (before/after)

### Library Usage

ZahirScan can be used as a Rust library to extract schemas (templates and metadata) from files programmatically.

```rust
use zahirscan::{RuntimeConfig, OutputMode, extract_schema, extract_schema_with_config};

// Simple API: embedded default config, same result shape as extract_schema_with_config
let result = extract_schema("file.log", OutputMode::Full)?;
let outputs = result.outputs;

// Advanced API: same ZahirScanResult { outputs, phase1_failed, phase2_failed }, with your config
let config = RuntimeConfig::new();
let result = extract_schema_with_config(files, OutputMode::Full, &config)?;
// result.outputs, result.phase1_failed, result.phase2_failed
```

**Supported input types** (via `ToPathIter` trait):

- Single file: `&str`, `String`, `&String`
- Multiple files: `&[&str]`, `&[String]`, `Vec<String>`, `[&str; N]`

#### Return types

Both **`extract_schema()`** and **`extract_schema_with_config()`** return `Result<ZahirScanResult>`. **`ZahirScanResult`** has:

- **`outputs: Vec<Output>`** — successful results (one per file that passed Phase 1 and Phase 2)
- **`phase1_failed: Vec<(String, String)>`** — paths that failed initial scan `(path, error_message)`
- **`phase2_failed: Vec<(String, String)>`** — paths that failed template mining or write `(path, error_message)`

Use `phase1_failed` and `phase2_failed` for TUI/reporting when some paths fail; partial success is returned, not an error. **`extract_schema()`** uses embedded default config only; **`extract_schema_with_config()`** uses the config you pass.

Each **`Output`** object contains:

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

**Conditional fields** (per-format metadata when applicable): `writing_footprint`, `image_metadata`, `video_metadata`, `audio_metadata`, `code_metadata`, `csv_metadata`, `sqlite_metadata`, `toml_metadata`, `zip_metadata`, `archive_metadata`, `xml_metadata`, `html_metadata`, `yaml_metadata`, `ini_metadata`, `pdf_metadata`, `docx_metadata`, `pptx_metadata`, `epub_metadata`. See [Metadata extraction by format](#metadata-extraction-by-format) and [docs.rs](https://docs.rs/zahirscan) for field details.

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

Runtime config is `RuntimeConfig`. How it’s loaded:

- **CLI**: Embedded default (`config.toml`), merged with a user config in the app data dir if present. Only keys in the user file override.
  - User config path: Unix `~/.config/zahirscan/zahirscan.toml` (or `$XDG_CONFIG_HOME/zahirscan/zahirscan.toml`), Windows `%APPDATA%\zahirscan\zahirscan.toml`.
  - Run **`zahirscan init`** to write the embedded default to edit; the CLI will use it as the overlay.
- **Library**: `extract_schema()` uses the embedded default only (no user config file). For custom config: `RuntimeConfig::new()` is the embedded default (same as config.toml, no file I/O); `RuntimeConfig::load_from_path(path)` to load from a file; `RuntimeConfig::load_with_overlay(base_path, overlay_path)` for base + overlay.

Full schema: [config.toml](config.toml).

**Adaptive defaults:**

- `max_workers = 0` uses a sensible default based on CPU cores
- Phase 2 uses **adaptive chunking** based on Phase 1 file statistics (count/bytes/variance) and targets a neat multiple of `max_workers`
- No manual batching configuration is required for typical workloads

**File filtering (`[filter]`):**

- `ignore_patterns`: skip files whose basename matches (exact: `.DS_Store`, `Thumbs.db`; suffix: `*.swp`, `*~`; prefix: `prefix*`)
- `ignore_hidden_files = true`: skip Unix hidden files (basename starts with `.`)

## Architecture

### Phase 1: Initial File Scan

- File format detection and statistics collection (line count, byte count, token count)
- Memory-mapped file access for text files (`memmap2`); mmap reused in Phase 2 (single open per path)
- Content type determination (log vs. text/markdown vs. media)
- Prepares tasks for Phase 2

### Phase 2: Template Mining and Metadata Extraction

- **Metadata extraction** (media, document, database, settings, structured, archives, code): see the [Metadata extraction by format](#metadata-extraction-by-format) table above for what is extracted per format.
- **Template Mining**: Frequency-based analysis to identify static vs. dynamic fields, extracts patterns as templates
- **Tokenization**: Content-aware (whitespace for logs, structure for JSON logs, sentence/paragraph for text/markdown)
- **Writing Footprint**: Two writing-analysis passes for text/markdown:
  1. **Exact-pattern pass**: Groups sentences by n-gram/phrase-derived pattern; used when repetition is sufficient to yield templates.
  2. **Shape fallback**: If pass 1 yields no templates, groups by sentence shape (word count + end punctuation). Produces stable, interpretable templates for short or highly varied text.
     Footprint metrics: vocabulary richness, sentence structure, punctuation, template diversity, SVO analysis.
- **Parallel Processing**: Single Rayon thread pool with adaptive chunk sizing based on Phase 1 statistics

## Security

ZahirScan implements non-invasive file operations:

- Path sanitization to prevent directory traversal attacks
- File existence validation before processing
- Read-only file access (never modifies source files)

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE).
