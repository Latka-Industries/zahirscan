//! `NumPy` `.npy` / `.npz` structured binary support (header, layout, and optional column stats).

mod npy;
mod npz;
mod sample;

pub use npy::{extract_npy_metadata, extract_npy_templates};
pub use npz::{extract_npz_metadata, extract_npz_templates};
