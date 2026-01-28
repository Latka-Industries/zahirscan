//! Structured formats: CSV, HTML.

mod csv;
mod html;

pub(crate) use csv::infer_value_type;
pub use csv::{extract_csv_metadata, extract_csv_templates};
pub use html::{extract_html_metadata, extract_html_templates};
