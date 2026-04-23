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
- **Documents**: DOCX, XLSX, PPTX, PDF, EPUB
- **Databases**: SQLite (.db, .sqlite, .sqlite3)
- **Settings**: INI (.ini, .cfg), TOML (.toml, .lock), YAML (.yaml, .yml), XML (.xml)
- **Structured text**: CSV/TSV/tab/psv, JSON, HTML (.html, .htm)
- **Tabular / columnar**: Parquet; Arrow IPC / Feather (`.arrow`, `.feather`, `.ipc`); Avro; ORC; NumPy (`.npy`, `.npz`); HDF5 (`.h5`, `.hdf5`); NetCDF (`.nc`, `.cdf`); Matrix Market (`.mtx`); MATLAB (`.mat`); ONNX (`.onnx`)
- **Archives**: ZIP (.zip); TAR and compressed TAR (`.tar`, `.tar.gz`, `.tgz`, `.tar.bz2`, `.tar.xz`).
- **Code/Scripts**: e.g. .py, .rs, .js, .ts, .sh, Makefile, Dockerfile, etc
- **Images**: JPEG, PNG, GIF, WebP, BMP, TIFF
- **Videos**: MP4, MKV, AVI, MOV, WMV, FLV, WebM, M4V, 3GP, OGV
- **Audio**: MP3, FLAC, WAV, M4A, AAC, OGG, Opus, WMA, APE, DSD, DSF

## Key Features

- **Template mining**: Repeated patterns in logs/text → templates with placeholders
- **Memory-mapped I/O**: `memmap2`; single open per path
- **Adaptive parallelization**: Phase 2 chunk sizes and Rayon batching follow Phase 1 stats; see **Architecture** and **Configuration** → adaptive batching
- **Path batching**: Large path lists run in fd-limit-sized batches so mmaps stay bounded; see **Architecture**
- **Streamable output**: `OutputSink::Collect` (default), `OutputSink::StreamOnly` (callback per file, no collection), or `OutputSink::Channel` (send on channel); compatible with batched scans
- **Size reduction**: Typically 80–95% smaller than raw while preserving structure and metadata

### Metadata extraction by format

| **Metadata**          | **Extracts**                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| --------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Media                 | Dimensions, codecs, bitrates for images, videos, audio                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Document              | **DOCX**: word count, character count, paragraph count, title, author, creation/modification dates, revision. **XLSX**: sheet count, sheet names, row/column counts per sheet, core properties. **PPTX**: slide count, core properties. **PDF**: page count, title, author, subject, creator, producer, creation/modification dates, PDF version, encryption status. **EPUB**: title, author, language, identifier, chapter count; writing footprint from spine body text. DRM-protected EPUBs (META-INF/encryption.xml present) are skipped for parsing and writing analysis.                                                                                                                                                                                   |
| Log                   | Byte count, line count, line ending (lf/crlf/cr), max line length, blank line count, has_timestamps (sample of first lines for ISO8601/epoch)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| JSON                  | Byte count, line count, line ending, max line length, blank line count; root_type (array/object), root_array_length or root_object_key_count, max_depth, pretty_printed heuristic                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| CSV / TSV / tab / psv | Row/column counts (dedicated count pass), column names, data types, delimiter, quote/escape characters, null percentages, unique string counts; type-specific statistics (numeric: min/max/mean/median/IQR/stdev, date: span/min/max, boolean: true percentage). Sample size for stats uses shared tabular rules (file-size scaling, wide-table scaling) plus **bytes-per-row** when row count is known before sampling.                                                                                                                                                                                                                                                                                                                                         |
| Tabular / columnar    | **Parquet, Arrow IPC/Feather, Avro, ORC**: schema column names, physical types, row counts, `stats_rows_sampled`, CSV-like column stats from a bounded row sample. **NumPy** (`.npy`/`.npz`), **HDF5**, **NetCDF**, **Matrix Market** (`.mtx`), **MATLAB** (`.mat`): shape, dtypes, and format-specific variable or column summaries. **ONNX** (`.onnx`): wire-model summary (e.g. producer, opset imports, op-type counts, initializer and I/O counts); typed graph inputs/outputs and node count. Sample caps use the same tabular scaling; **bytes-per-row** applies when row count is known before stats (footer/metadata for Parquet/ORC; two-pass row count for Arrow IPC; array shape for NumPy; Avro has no cheap pre-scan count, so byte scaling only). |
| SQLite                | Schema (tables, columns, types, constraints), primary keys, foreign keys, indexes, row counts, column statistics (null percentages, unique counts, numeric/text/boolean/blob/date)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| TOML, YAML, INI, CFG  | Recursive schema (scalar, table/mapping, array/sequence; INI: section→key→scalar, multi-line values), key count, max depth. TOML: section count. YAML: scalar/sequence/map counts. INI/.cfg: section count, comment count                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| Code/Scripts          | script_type (linguist + optional shebang), byte_count, line_count; BOM, line_ending, trailing_newline, max_line_length, blank_line_count, indentation (single-pass scan)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| ZIP                   | File count, entries (path, uncompressed/compressed size, detected type, modified, compression method), entry_type_counts; filters hidden OS files (e.g. \_\_MACOSX, .DS_Store, Thumbs.db)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| Archive (TAR family)  | File count, entries (path, size), compressed_size, uncompressed_size                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| XML                   | Recursive schema (root→children with attributes; repeated siblings as arrays with union of all children), element count, attribute count, max depth, has_namespaces                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| HTML                  | Title, meta description, lang, charset, viewport; link/stylesheet/script/style counts; heading (h1–h6) and element counts (img, table, form, p, ul, ol, iframe, article, nav, section, header, footer, main); plain_text_len, word_count; writing footprint from body text                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| Writing Footprint     | For text/markdown/html: vocabulary richness, sentence structure, template diversity, punctuation metrics. Uses **two writing-analysis passes**: (1) exact-pattern grouping (n-gram/phrase-based); (2) shape fallback (group by sentence length + end punctuation) when pass 1 yields no templates                                                                                                                                                                                                                                                                                                                                                                                                                                                                |

## Installation

```bash
cargo add zahirscan        # library
cargo install zahirscan    # CLI
```

**Optional system tools / libraries**:

- **`ffprobe`** (FFmpeg): needed for rich video/audio metadata
- **`libnetcdf`** (NetCDF C): the `netcdf` crate links it at build time

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

- **Mode 1 (Templates)**: Minimal JSON with templates, writing footprint (text/markdown), and per-format metadata where applicable—see **Supported formats** (media, code, logs, JSON, documents, tabular/columnar, archives, …)
- **Mode 2 (Full)**: Mode 1 output plus:
  - File statistics (size, line count, processing time)
  - Size comparison (before/after)

### Library Usage

ZahirScan can be used as a Rust library to extract schemas (templates and metadata) from files programmatically.

#### Basic: collect all outputs

```rust
use zahirscan::{extract_zahir, OutputMode, OutputSink};

// Default config, no file write; all results in result.outputs
let result = extract_zahir(
    "file.log",
    OutputMode::Full,
    None,
    None,
    &OutputSink::Collect,
)?;
// result.outputs, result.phase1_failed, result.phase2_failed
```

#### Stream-only: callback or channel (no collection, bounded memory)

```rust
use std::sync::{Arc, Mutex};
use zahirscan::{extract_zahir, Output, OutputMode, OutputSink};

let collected = Arc::new(Mutex::new(Vec::<(String, zahirscan::Output)>::new()));
let c = Arc::clone(&collected);
let sink = OutputSink::StreamOnly(Box::new(move |path, out| {
    c.lock().unwrap().push((path, out));
}));
let result = extract_zahir(
    ["a.log", "b.log"],
    OutputMode::Full,
    None,
    None,
    &sink,
)?;
// result.outputs is empty; results are in collected
```

#### Streaming input (paths from a channel)

```rust
use std::sync::mpsc;
use zahirscan::{extract_zahir_from_stream, OutputMode, OutputSink};

let (tx, rx) = mpsc::channel();
// Producer sends paths, then drops tx
let result = extract_zahir_from_stream(&rx, OutputMode::Full, None, None, &OutputSink::Collect)?;
```

**Inputs**: single path (`&str`, `String`) or multiple (`&[&str]`, `Vec<String>`, etc.). **Config**: pass `Some(&config)` for custom `RuntimeConfig`; `None` uses embedded default. Full API: `ZahirScanResult`, `Output`, `OutputSink`, `Template`, `WritingFootprint`, and per-format metadata: [docs.rs](https://docs.rs/zahirscan).

### Configuration

- **CLI**: Embedded default (`config.toml`), merged with a user config in the app data dir if present. Only keys in the user file override.
  - User config path: Unix `~/.config/zahirscan/zahirscan.toml` (or `$XDG_CONFIG_HOME/zahirscan/zahirscan.toml`), Windows `%APPDATA%\zahirscan\zahirscan.toml`.
  - Run **`zahirscan init`** to write the embedded default to edit; the CLI will use it as the overlay.
- **Library**: `extract_zahir(..., config: None, output_dir: None)` uses the embedded default only (no overlay). For custom config pass `Some(&config)`; `RuntimeConfig::new()` is the embedded default (no file I/O). Overlay is only used by the CLI via `setup::load_config()`.

Full schema: [config.toml](config.toml).

**Adaptive batching and parallelization** (see **Architecture** for how Phase 1 / Phase 2 interact with batches):

- **Phase 2 adaptive chunking**: Chunk sizes and “chunks per file” come from Phase 1 stats (file count, mean bytes, variance); targets a neat multiple of `max_workers` for load balancing.
- **Phase 2 parallel batching**: When task count exceeds `workers × threshold_multiplier`, Rayon uses `with_min_len(batch_size)` to avoid thread-pool saturation; otherwise full parallelism.
- `max_workers = 0` uses a sensible default (e.g. num_cpus - 1). No manual tuning is required for typical workloads.

**File filtering (`[filter]`):**

- `ignore_patterns`: skip files whose basename matches (exact: `.DS_Store`, `Thumbs.db`; suffix: `*.swp`, `*~`; prefix: `prefix*`)
- `ignore_hidden_files = true`: skip Unix hidden files (basename starts with `.`)

## Architecture

**Phase 1**: Format detection, stats (lines/bytes/tokens), mmap per path, content-type classification. Runs in parallel over paths (Rayon).

**Path batching**: When there are more paths than the batch size (from the process fd limit), each chunk runs Phase 1 then Phase 2; the chunk and its mmaps are dropped before the next chunk.

**Phase 2**: Metadata extraction per format, template mining, writing footprint (exact-pattern then shape fallback for text/markdown). One Rayon pool; chunk sizing and `with_min_len` batching follow Phase 1 stats—tunable fields are under **Configuration** → adaptive batching.

## Security

Read-only, non-invasive: path sanitization, existence checks, no source modification.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE).
