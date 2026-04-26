//! MATLAB `.mat` (classic v7) — one metadata row per top-level variable, global stats for numerics
//! and `struct_subtree` for `struct` / `struct` array / `cell`.

mod common;
mod summary;
mod walk;

pub use walk::extract_mat_metadata;

crate::no_template_mining!(
    extract_mat_templates,
    "MATLAB `.mat`: per-entry `struct_subtree` on `struct`/`cell` variables; `n_nan`/`n_inf` in subtree."
);
