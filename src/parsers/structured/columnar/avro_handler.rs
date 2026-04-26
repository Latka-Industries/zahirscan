//! Apache Avro OCF metadata and column statistics.

use std::io::Cursor;

use anyhow::{Context, Result};
use apache_avro::types::Value;
use apache_avro::{Reader, Schema};

use crate::config::RuntimeConfig;
use crate::parsers::{ParseResult, structured::constants::StructuredEncoding};
use crate::results::{AvroMetadata, ColumnarCommonFields};

use super::utils as columnar_utils;

fn avro_field_type_strings(schema: &Schema) -> Vec<String> {
    match schema {
        Schema::Record(rec) => rec.fields.iter().map(|f| format!("{}", f.schema)).collect(),
        _ => vec![format!("{schema}")],
    }
}

fn avro_column_names(schema: &Schema) -> Vec<String> {
    match schema {
        Schema::Record(rec) => rec.fields.iter().map(|f| f.name.clone()).collect(),
        _ => vec!["value".to_string()],
    }
}

fn value_row_strings(value: &Value) -> Vec<String> {
    match value {
        Value::Record(fields) => fields.iter().map(|(_, v)| avro_cell_string(v)).collect(),
        other => vec![avro_cell_string(other)],
    }
}

fn avro_cell_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Boolean(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Long(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Double(f) => f.to_string(),
        Value::Bytes(b) => format!("<{} bytes>", b.len()),
        Value::String(s) => s.clone(),
        Value::Enum(_, sym) => sym.clone(),
        Value::Union(_, inner) => avro_cell_string(inner),
        Value::Array(a) => format!("<array len {}>", a.len()),
        Value::Map(m) => format!("<map len {}>", m.len()),
        Value::Date(d) => d.to_string(),
        Value::Record(_)
        | Value::Fixed(_, _)
        | Value::Decimal(_)
        | Value::BigDecimal(_)
        | Value::TimeMillis(_)
        | Value::TimeMicros(_)
        | Value::TimestampMillis(_)
        | Value::TimestampMicros(_)
        | Value::TimestampNanos(_)
        | Value::LocalTimestampMillis(_)
        | Value::LocalTimestampMicros(_)
        | Value::LocalTimestampNanos(_)
        | Value::Duration(_)
        | Value::Uuid(_) => format!("{value:?}"),
    }
}

/// Extract Avro writer schema, row count, and CSV-like statistics from a bounded sample.
///
/// # Errors
///
/// Returns an error if the container is not a valid Avro OCF file.
pub fn extract_avro_metadata(
    mmap: &[u8],
    _stats: &ParseResult,
    config: &RuntimeConfig,
) -> Result<AvroMetadata> {
    let cursor = Cursor::new(mmap);
    let reader = Reader::new(cursor).context("open Avro OCF")?;
    let writer_schema = reader.writer_schema().clone();
    let schema_canonical = Some(writer_schema.canonical_form());
    let column_names_vec = avro_column_names(&writer_schema);
    let avro_field_types = avro_field_type_strings(&writer_schema);
    let column_count = column_names_vec.len();
    let file_bytes = mmap.len() as u64;

    let max_sample = columnar_utils::tabular_effective_sample_rows(
        config.max_tabular_sample_rows,
        file_bytes,
        column_count.max(1),
        None,
    );
    let mut sample_data: Vec<Vec<String>> = Vec::new();
    let mut row_count: usize = 0;

    for item in reader {
        let v = item.context("decode Avro datum")?;
        row_count += 1;
        if sample_data.len() < max_sample {
            sample_data.push(value_row_strings(&v));
        }
    }

    let ts = columnar_utils::tabular_stats_from_sample(&sample_data, column_count, config);

    let columns = columnar_utils::columns_from_tabular_sample(
        column_count,
        Some(column_names_vec),
        ts,
        Some(avro_field_types),
    );

    Ok(AvroMetadata {
        common: ColumnarCommonFields {
            row_count: Some(row_count),
            column_count: Some(column_count),
            stats_rows_sampled: Some(sample_data.len()),
            encoding: Some(StructuredEncoding::TABULAR_BINARY.to_string()),
            columns,
        },
        schema_canonical,
    })
}

crate::no_template_mining!(
    extract_avro_templates,
    "Avro is binary with embedded schema; no text template mining."
);
