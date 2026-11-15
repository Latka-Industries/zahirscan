# Changelog

## [0.4.0] - 2026-11-15

### Changed

- Depend on **tetration 0.8.3** (`default-features = false`). Published 0.3.x required yanked tetration `0.1.x` and could not be linked from a fresh crates.io resolve.
- CI: path filters; NetCDF coverage on Ubuntu; macOS/Windows exercise `--no-default-features` (Homebrew HDF5 2.x is incompatible with `hdf5-metno-sys`).

### Added

- Richer `tetration_metadata` for `.tet` files against tetration 0.8:
  - File footer (`tool` / `library_version` / `created_at`)
  - Per-dataset declare fields (`dim_names`, `attrs`, coord label summaries)
  - Capped history rows with PROV `agent` / `activity` / `entity`
  - Chunk-index `file_execution` prefs when non-default
  - Shallow verify summary (`ok`, finding counts; no deep decode)

### Notes

- Semver minor: tetration major jump for dependents and expanded `.tet` JSON shape.
- Library consumers should use `zahirscan = { version = "0.4", default-features = false }` when `libnetcdf` is unavailable (Tesscriptor / Hostinger).

## [0.3.6] - 2026-07-21

- Parse SVG dimensions; omit unknown size from JSON.
