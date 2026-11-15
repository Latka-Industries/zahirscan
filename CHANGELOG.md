# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0] - 2026-11-15

Semver **minor**: dependents must move off yanked tetration `0.1.x`, and `tetration_metadata` gains new optional JSON fields (additive for readers that ignore unknown keys).

### Added

- **`.tet` / tetration 0.8 metadata** ([#19](https://github.com/Latka-Industries/zahirscan/pull/19), [THI-693](https://linear.app/thicclatka/issue/THI-693)):
  - File footer: `file.tool`, `file.library_version`, `file.created_at`
  - Per-dataset `declare`: `dim_names`, string `attrs` (incl. semantics profile when stamped), coord axis summaries (`label_count` + first/last label; full label lists are not dumped)
  - History: capped event rows (`op` / `source` / `at` + PROV `agent` / `activity` / `entity`) plus `history_truncated` when more events exist; `history_event_count` remains the full count
  - `file_execution` when chunk-index prefs are non-default (`memory_budget_percent_bps`, `memory_budget_bytes`)
  - Shallow verify block: `verify.ok`, `finding_count`, `failed_finding_count` (no deep decode)
- **CI matrix** ([#18](https://github.com/Latka-Industries/zahirscan/pull/18), [THI-695](https://linear.app/thicclatka/issue/THI-695)):
  - Workflow renamed to **CI** with `paths` allowlist (docs-only PRs skip)
  - Ubuntu: default features (NetCDF + HDF5) **and** `--no-default-features`
  - macOS / Windows: `--no-default-features` + ffmpeg (Tesscriptor / Hostinger path)
  - MSRV checks both feature sets; `Swatinem/rust-cache@v2` keyed per OS/label; cargo `--locked`
- CHANGELOG, README / rustdoc notes for the 0.4 cut; release workflow installs `libhdf5-dev` and publishes a non-draft GH release on tag ([#20](https://github.com/Latka-Industries/zahirscan/pull/20), [THI-696](https://linear.app/thicclatka/issue/THI-696))

### Changed

- **tetration** dependency: `0.1.5` → **`0.8.3`** with `default-features = false` ([#19](https://github.com/Latka-Industries/zahirscan/pull/19)). Published **0.3.6** required yanked tetration `0.1.5`–`0.1.12`, so a fresh `cargo add zahirscan` could not resolve.
- Existing query-driven reductions (mean/min/max/std, columnar / tensor3d caps) kept; they now run against the 0.8 catalog/query API.
- NetCDF CI is **Ubuntu-only**: Homebrew’s `hdf5` is 2.x and `hdf5-metno-sys` (via `netcdf-sys`) rejects `H5_VERSION` 2.x.

### Fixed

- Pedantic doc hygiene (non-`cast-*`): NetCDF stub docs, pickle/`CPython` markdown, Apple Pkl link ([#20](https://github.com/Latka-Industries/zahirscan/pull/20))

### Notes

- Library / server installs without `libnetcdf`:

  ```toml
  zahirscan = { version = "0.4", default-features = false }
  ```

- Epic: [THI-694](https://linear.app/thicclatka/issue/THI-694). After this tag, pin UBLX and Tesscriptor; yank older zahirscan (then UBLX) once dependents are on 0.4.
- Tag `v0.4.0` triggers the release workflow (GH release + crates.io publish).

## [0.3.6] - 2026-07-21

### Fixed

- Parse SVG width/height (and viewBox fallback); omit unknown size from JSON instead of writing `0`×`0` ([#17](https://github.com/Latka-Industries/zahirscan/pull/17), [THI-163](https://linear.app/thicclatka/issue/THI-163))

### Notes

- Last 0.3 line on crates.io. **Do not use for new dependents** — tetration `0.1.x` is yanked; use **0.4.0+**.

## [0.3.5] - 2026-06-05

### Added

- `FileType::Pickle` with read-only opcode metadata (no unpickling) ([#15](https://github.com/Latka-Industries/zahirscan/pull/15))

### Changed

- CI: drop redundant build job; use `rust-cache`

## [0.3.4] - 2026-06-03

### Changed

- User docs live on [ublx.dev/zahirscan](https://ublx.dev/zahirscan/); crates.io README slimmed
- Repository / homepage metadata points at Latka-Industries + ublx.dev
- `tract-tflite` → `0.23.0`

## [0.3.3] - 2026-05-27

### Added

- `.tet` parser with query-driven column and tensor stats (tetration)
- Concurrent reduction queries with adaptive memory hints
- Crates.io publish on tag; Actions majors for Node 24

## [0.3.2] - 2026-05-08

### Changed

- Arrow stack → `58.2`; Cargo.lock refresh
- Docs: NetCDF `--no-default-features` install guidance
- CI: dedupe Windows NetCDF setup; simplify release workflow

## [0.3.1] - 2026-04-27

### Added

- Optional NetCDF feature (default on); docs.rs builds with NetCDF off

### Fixed

- CI: Rust **1.95**, rustfmt + clippy in the test suite

## [0.3.0] - 2026-04-26

### Added

- Model metadata parsers (ONNX, GGUF, SafeTensors, TFLite) and centralized metadata wiring
- Zarr metadata parsing; modularized MATLAB parser
- Richer 3D tensor stats; Mat metadata improvements
- SQLite affinity / stats alignment

### Fixed

- Windows CI NetCDF package name (`libnetcdf`)

## [0.2.18] - 2026-04-14

### Changed

- PDF metadata: replace `pdf-rs` with `lopdf`

### Added

- Cap ZIP decompressed reads for Office/EPUB (`zip_read` helper)

## [0.2.17] - 2026-04-13

### Changed

- Bump `lofty` (prior version yanked)

## [0.2.16] - 2026-04-07

### Added

- AIFF / AIFC audio extensions (`aif`, `aiff`, `aifc`)

## [0.2.15] - 2026-03-29

### Changed

- Drop unused deps; refresh CI checkout; clippy-driven parser/setup tweaks

## [0.2.14] - 2026-03-20

### Added

- Delimiter-aware CSV / TSV / PSV parsing
- `FileType` metadata name helpers; `ok_anyhow` macro

### Changed

- Config / engine API cleanup; public CSV helpers

## [0.2.13] - 2026-03-01

### Added

- Broader log timestamp detection (formats + embedded Unix ts)
- Log / JSON metadata improvements

### Changed

- Deps: `lofty` 0.23, `serde_norway`, `toml` 1.0, `zip` 8

## [0.2.12] - 2026-02-07

### Added

- `OutputSink` (stream-only vs collect); fd-limit batching via `rlimit`
- Lib / CLI wired through engine phases

## [0.2.11] - 2026-02-04

### Added

- Single entry point `extract_zahir`; streaming I/O; `output_dir` naming

### Changed

- EPUB: OPF/manifest/spine parsing; DRM skip

## [0.2.10] - 2026-02-03

### Added

- Reuse Phase 1 mmap in Phase 2 (single open per path)
- `metadata_macro_sync` test + `mise run meta`

### Changed

- Rust toolchain **1.92 → 1.93**
- Unify `extract_schema` return type with `extract_schema_with_config`

## [0.2.9] - 2026-01-30

### Added

- `extract_schema_with_config` for config reuse
- Integration tests for schema extraction with config

### Changed

- README examples consolidated around config reuse

## [0.2.8] - 2026-01-30

### Fixed

- Remove `build.rs` so docs.rs can build

### Changed

- Extract analysis module; move EPUB/PDF/JSON under structured
- Canonicalize output dir; extend fixtures / CLI tests

## [0.2.6] - 2026-01-29

### Added

- Archive, EPUB, INI, PPTX metadata
- HTML and YAML parsers
- Linguist-based code language detection
- Integration tests (SQLite e2e, fixtures, multi-file, error paths)

### Fixed

- Filter TAR `._*`, PaxHeader, and directory entries

### Changed

- Category dispatch / `process_with_metadata` macro; scan machinery in `engine/`

## [0.2.3] - 2026-01-28

### Changed

- Installation docs: remove git-clone path from README

[Unreleased]: https://github.com/Latka-Industries/zahirscan/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/Latka-Industries/zahirscan/compare/v0.3.6...v0.4.0
[0.3.6]: https://github.com/Latka-Industries/zahirscan/compare/v0.3.5...v0.3.6
[0.3.5]: https://github.com/Latka-Industries/zahirscan/compare/v0.3.4...v0.3.5
[0.3.4]: https://github.com/Latka-Industries/zahirscan/compare/v0.3.3...v0.3.4
[0.3.3]: https://github.com/Latka-Industries/zahirscan/compare/v0.3.2...v0.3.3
[0.3.2]: https://github.com/Latka-Industries/zahirscan/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/Latka-Industries/zahirscan/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/Latka-Industries/zahirscan/compare/v0.2.18...v0.3.0
[0.2.18]: https://github.com/Latka-Industries/zahirscan/compare/v0.2.17...v0.2.18
[0.2.17]: https://github.com/Latka-Industries/zahirscan/compare/v0.2.16...v0.2.17
[0.2.16]: https://github.com/Latka-Industries/zahirscan/compare/v0.2.15...v0.2.16
[0.2.15]: https://github.com/Latka-Industries/zahirscan/compare/v0.2.14...v0.2.15
[0.2.14]: https://github.com/Latka-Industries/zahirscan/compare/v0.2.13...v0.2.14
[0.2.13]: https://github.com/Latka-Industries/zahirscan/compare/v0.2.12...v0.2.13
[0.2.12]: https://github.com/Latka-Industries/zahirscan/compare/v0.2.11...v0.2.12
[0.2.11]: https://github.com/Latka-Industries/zahirscan/compare/v0.2.10...v0.2.11
[0.2.10]: https://github.com/Latka-Industries/zahirscan/compare/v0.2.9...v0.2.10
[0.2.9]: https://github.com/Latka-Industries/zahirscan/compare/v0.2.8...v0.2.9
[0.2.8]: https://github.com/Latka-Industries/zahirscan/compare/v0.2.6...v0.2.8
[0.2.6]: https://github.com/Latka-Industries/zahirscan/compare/v0.2.3...v0.2.6
[0.2.3]: https://github.com/Latka-Industries/zahirscan/releases/tag/v0.2.3
