# ZahirScan

[![Crates.io](https://img.shields.io/crates/v/zahirscan.svg)](https://crates.io/crates/zahirscan)
[![docs.rs](https://img.shields.io/docsrs/zahirscan)](https://docs.rs/zahirscan)
![Build](https://github.com/Latka-Industries/zahirscan/workflows/Build/badge.svg)
![Rust](https://img.shields.io/badge/rust-1.95-orange.svg)

> _"Others will dream that I am mad, while I dream of the Zahir."_ — [JL Borges, Labyrinths](https://bookshop.org/p/books/labyrinths-jorge-luis-borges/f14b472a366ed106?ean=9780811216999&next=t&)

**Template mining and metadata extraction** across logs, documents, tabular/columnar data, media, archives, models, and more. CLI and library; streamable output for large path lists.

## Quick start

```bash
cargo install zahirscan
zahirscan init   # optional: write ~/.config/zahirscan/zahirscan.toml
zahirscan -i application.log -o ./out
```

Without NetCDF: `cargo install zahirscan --no-default-features`. Optional **`ffprobe`** for rich A/V metadata.

## Documentation

|                                                                                                                     |                                     |
| ------------------------------------------------------------------------------------------------------------------- | ----------------------------------- |
| **[Overview](https://ublx.dev/zahirscan/)**                                                                         | Features, supported formats summary |
| [Install](https://ublx.dev/zahirscan/install)                                                                       | Cargo, features, `init`             |
| [CLI](https://ublx.dev/zahirscan/cli)                                                                               | Flags, output modes, examples       |
| [Supported formats](https://ublx.dev/zahirscan/formats)                                                             | Per-format metadata detail          |
| [Configuration](https://ublx.dev/zahirscan/configuration) · [Architecture](https://ublx.dev/zahirscan/architecture) | Tuning, phases, batching            |
| [Library](https://ublx.dev/zahirscan/library)                                                                       | `extract_zahir`, sinks, streaming   |
| **[API (docs.rs)](https://docs.rs/zahirscan)**                                                                      | Rust types and functions            |
| [config.toml](config.toml)                                                                                          | Full config schema (repo)           |

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE).
