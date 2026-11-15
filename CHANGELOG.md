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
- CHANGELOG, README / rustdoc notes for the 0.4 cut; release workflow installs `libhdf5-dev` for crates.io publish ([#20](https://github.com/Latka-Industries/zahirscan/pull/20), [THI-696](https://linear.app/thicclatka/issue/THI-696))

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
- Tag `v0.4.0` triggers the release workflow (draft GH release + crates.io publish).

## [0.3.6] - 2026-07-21

### Fixed

- Parse SVG width/height (and viewBox fallback); omit unknown size from JSON instead of writing `0`×`0` ([#17](https://github.com/Latka-Industries/zahirscan/pull/17), [THI-163](https://linear.app/thicclatka/issue/THI-163))

### Notes

- Last 0.3 line on crates.io. **Do not use for new dependents** — tetration `0.1.x` is yanked; use **0.4.0+**.

[Unreleased]: https://github.com/Latka-Industries/zahirscan/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/Latka-Industries/zahirscan/compare/v0.3.6...v0.4.0
[0.3.6]: https://github.com/Latka-Industries/zahirscan/releases/tag/v0.3.6
