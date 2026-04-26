//! Path filtering and sizing for `Zarr` directory stores (`.zarr/`), used when a walker emits
//! both store roots and inner chunk/metadata paths. Only store-root paths are kept.

use std::path::Path;

pub const ZARR_EXTENSION: &str = ".zarr";

/// True if `p` is a `Zarr` store root: last path segment ends with `.zarr` and there is no further
/// path after that segment (after trimming trailing `/`).
#[must_use]
pub fn is_zarr_store_root_path(p: &str) -> bool {
    let t = p.trim_end_matches(['/', '\\']);
    t.ends_with(ZARR_EXTENSION)
}

/// When the path string contains `.zarr` but is not a store root (e.g. `…/a.zarr/field/0`), drop it.
/// All other paths are kept.
#[must_use]
pub fn filter_zarr_input_paths(paths: impl IntoIterator<Item = String>) -> Vec<String> {
    paths
        .into_iter()
        .filter(|p| {
            if !p.contains(ZARR_EXTENSION) {
                return true;
            }
            is_zarr_store_root_path(p)
        })
        .collect()
}

/// Total size in bytes of all regular files under `root` (recursive). Used for `byte_count` when
/// the store is a directory, not a single mmap.
///
/// # Errors
///
/// Returns I/O errors from directory traversal.
pub fn total_file_bytes_under(root: &Path) -> std::io::Result<usize> {
    fn walk(path: &Path, acc: &mut usize) -> std::io::Result<()> {
        for e in std::fs::read_dir(path)? {
            let e = e?;
            let p = e.path();
            let md = e.metadata()?;
            if md.is_dir() {
                walk(&p, acc)?;
            } else if md.is_file() {
                *acc = acc.saturating_add(md.len() as usize);
            }
        }
        Ok(())
    }
    let mut n = 0usize;
    walk(root, &mut n)?;
    Ok(n)
}
