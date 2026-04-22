//! `SQLite` database metadata extraction

mod type_stats;
mod utils;

use anyhow::Result;
use log::debug;
use rusqlite::Connection;

use crate::config::RuntimeConfig;
use crate::parsers::{FileType, ParseResult, column_stats};
use crate::results::{ColumnInfo, MiningResult, SqliteMetadata, TableInfo};
use memmap2::Mmap;
use type_stats::{compute_stats_for_type, fetch_column_as_strings};

/// Log a `SQLite` error and set it in metadata, then return the metadata.
/// Helper to reduce repetition of error handling pattern.
fn handle_sqlite_error(
    metadata_mut_ref: &mut SqliteMetadata,
    file_path_ref: &str,
    error_msg: String,
) -> SqliteMetadata {
    debug!("SQLite error for '{file_path_ref}': {error_msg}");
    metadata_mut_ref.error = Some(error_msg);
    metadata_mut_ref.clone()
}

fn apply_connection_pragmas(conn: &Connection, metadata: &mut SqliteMetadata) {
    metadata.page_size = utils::try_query_one(conn, "PRAGMA page_size;", |r| r.get::<_, i64>(0))
        .and_then(|v| usize::try_from(v).ok());
    metadata.encoding = utils::try_query_one(conn, "PRAGMA encoding;", |r| r.get::<_, String>(0));
    metadata.sqlite_version =
        utils::try_query_one(conn, "SELECT sqlite_version();", |r| r.get::<_, String>(0));
}

/// One user table: schema bits, FK flags, column stats, indexes.
fn build_table_info(
    conn: &Connection,
    table_name: &str,
    config: &RuntimeConfig,
) -> Result<TableInfo, String> {
    let quoted_table = utils::quote_sql_identifier(table_name);

    let (columns, primary_keys) = utils::get_table_columns(conn, table_name)?;

    let mut table_info = TableInfo {
        name: table_name.to_string(),
        column_count: Some(columns.len()),
        columns: if columns.is_empty() {
            None
        } else {
            Some(columns)
        },
        primary_keys: if primary_keys.is_empty() {
            None
        } else {
            Some(primary_keys)
        },
        ..Default::default()
    };

    if let Some(n) = utils::get_table_row_count(conn, &quoted_table) {
        table_info.row_count = Some(n);
    }

    let (foreign_keys, fk_columns) = utils::get_foreign_keys_for_table(conn, &quoted_table)?;
    if !foreign_keys.is_empty() {
        table_info.foreign_keys = Some(foreign_keys);
    }
    if let Some(cols) = table_info.columns.as_deref_mut() {
        for col in cols.iter_mut() {
            if fk_columns.contains(&col.name) {
                col.is_foreign_key = Some(true);
            }
        }
    }

    ensure_column_stats(
        conn,
        table_name,
        table_info.columns.as_deref_mut(),
        table_info.row_count.unwrap_or(0),
        config,
    );

    let indexes = utils::get_indexes_for_table(conn, table_name, &quoted_table)?;
    if !indexes.is_empty() {
        table_info.indexes = Some(indexes);
    }

    Ok(table_info)
}

/// Extract `SQLite` metadata from database file
///
/// # Errors
///
/// Returns [`anyhow::Error`] when a temporary file cannot be created or written while opening the database from bytes.
pub fn extract_sqlite_metadata(
    content_ref: &[u8],
    stats_ref: &ParseResult,
    config_ref: &RuntimeConfig,
) -> Result<SqliteMetadata> {
    let (conn_opt, mut metadata) =
        utils::open_sqlite_connection(content_ref, stats_ref.byte_count, &stats_ref.file_path)?;
    let Some((conn, _temp_file)) = conn_opt else {
        return Ok(metadata);
    };

    apply_connection_pragmas(&conn, &mut metadata);

    let mut tables = Vec::new();
    let mut total_rows = 0usize;

    let table_names: Vec<String> = match utils::get_table_names(&conn) {
        Ok(names) => names,
        Err(error_msg) => {
            return Ok(handle_sqlite_error(
                &mut metadata,
                &stats_ref.file_path,
                error_msg,
            ));
        }
    };

    metadata.table_count = Some(table_names.len());

    for table_name in &table_names {
        let table_info = match build_table_info(&conn, table_name, config_ref) {
            Ok(t) => t,
            Err(msg) => {
                return Ok(handle_sqlite_error(
                    &mut metadata,
                    &stats_ref.file_path,
                    msg,
                ));
            }
        };
        if let Some(n) = table_info.row_count {
            total_rows += n;
        }
        tables.push(table_info);
    }

    metadata.total_rows = Some(total_rows);
    if !tables.is_empty() {
        metadata.tables = Some(tables);
    }

    Ok(metadata)
}

/// Fills `null_percentage/unique_count` and type-specific stats. No-op if `columns` is None.
/// Empty tables get no-data defaults; otherwise delegates to `compute_column_statistics`.
fn ensure_column_stats(
    conn_ref: &Connection,
    table_name_ref: &str,
    columns: Option<&mut [ColumnInfo]>,
    row_count: usize,
    config_ref: &RuntimeConfig,
) {
    let Some(cols_mut_ref) = columns else { return };
    if row_count == 0 {
        for col_mut_ref in cols_mut_ref.iter_mut() {
            utils::set_no_data_column_stats(col_mut_ref);
        }
    } else {
        compute_column_statistics(conn_ref, table_name_ref, cols_mut_ref, config_ref);
    }
}

/// Compute column statistics (null percentage, unique count, type-specific stats)
fn compute_column_statistics(
    conn_ref: &Connection,
    table_name_ref: &str,
    columns_mut_ref: &mut [ColumnInfo],
    config_ref: &RuntimeConfig,
) {
    let quoted_table = utils::quote_sql_identifier(table_name_ref);

    for col_mut_ref in columns_mut_ref.iter_mut() {
        let quoted_col = utils::quote_sql_identifier(&col_mut_ref.name);
        let values = fetch_column_as_strings(
            conn_ref,
            &quoted_table,
            &quoted_col,
            table_name_ref,
            &col_mut_ref.name,
        );
        if values.is_empty() {
            utils::set_no_data_column_stats(col_mut_ref);
            continue;
        }

        let (null_pct, unique_count) =
            column_stats::compute_null_and_unique_stats(&values, config_ref);
        col_mut_ref.null_percentage = Some(null_pct);
        col_mut_ref.unique_count = Some(unique_count);

        compute_stats_for_type(
            conn_ref,
            &quoted_table,
            &quoted_col,
            col_mut_ref,
            &values,
            config_ref,
        );
    }
}

crate::no_template_mining!(
    extract_sqlite_templates,
    "`SQLite` files don't need template mining - return empty result. Only metadata is extracted (schema, tables, columns, etc.)."
);

/// Extract metadata and templates for `SQLite`; single file type in this module.
///
/// # Errors
///
/// Propagates errors from [`extract_sqlite_metadata`] or [`extract_sqlite_templates`].
pub fn process(
    stats_mut_ref: &mut ParseResult,
    mmap_ref: &Mmap,
    config_ref: &RuntimeConfig,
) -> Result<MiningResult> {
    crate::process_with_metadata!(
        stats_mut_ref,
        mmap_ref,
        config_ref,
        sqlite_metadata,
        extract_sqlite_metadata(mmap_ref, stats_mut_ref, config_ref),
        crate::results::SqliteMetadata,
        FileType::Sqlite,
        extract_sqlite_templates(mmap_ref, stats_mut_ref, config_ref)
    )
}
