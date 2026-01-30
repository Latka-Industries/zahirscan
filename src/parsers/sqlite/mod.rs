//! SQLite database metadata extraction

mod type_stats;
mod utils;

use anyhow::Result;
use log::debug;
use rusqlite::Connection;

use crate::engine::config::Config;
use crate::parsers::{FileType, ParseResult, column_stats};
use crate::results::{ColumnInfo, MiningResult, SqliteMetadata, TableInfo};
use memmap2::Mmap;
use type_stats::{compute_stats_for_type, fetch_column_as_strings};

/// Log a SQLite error and set it in metadata, then return the metadata.
/// Helper to reduce repetition of error handling pattern.
fn handle_sqlite_error(
    metadata: &mut SqliteMetadata,
    file_path: &str,
    error_msg: String,
) -> Result<SqliteMetadata> {
    debug!("SQLite error for '{}': {}", file_path, error_msg);
    metadata.error = Some(error_msg);
    Ok(metadata.clone())
}

/// Extract SQLite metadata from database file
pub fn extract_sqlite_metadata(
    content: &[u8],
    stats: &ParseResult,
    config: &Config,
) -> Result<SqliteMetadata> {
    let (conn_opt, mut metadata) =
        utils::open_sqlite_connection(content, stats.byte_count, &stats.file_path)?;
    let (conn, _temp_file) = match conn_opt {
        Some((c, t)) => (c, t),
        None => return Ok(metadata),
    };

    // Extract database statistics (best-effort; failures remain None)
    metadata.page_size = utils::try_query_one(&conn, "PRAGMA page_size;", |r| r.get::<_, i64>(0))
        .map(|v| v as usize);
    metadata.encoding = utils::try_query_one(&conn, "PRAGMA encoding;", |r| r.get::<_, String>(0));
    metadata.sqlite_version =
        utils::try_query_one(&conn, "SELECT sqlite_version();", |r| r.get::<_, String>(0));

    // Extract schema information
    let mut tables = Vec::new();
    let mut total_rows = 0usize;

    // Get all table names (excluding system tables)
    let table_names: Vec<String> = match utils::get_table_names(&conn) {
        Ok(names) => names,
        Err(error_msg) => return handle_sqlite_error(&mut metadata, &stats.file_path, error_msg),
    };

    metadata.table_count = Some(table_names.len());

    for table_name in &table_names {
        let quoted_table = utils::quote_sql_identifier(table_name);

        let (columns, primary_keys) = match utils::get_table_columns(&conn, table_name) {
            Ok(x) => x,
            Err(msg) => return handle_sqlite_error(&mut metadata, &stats.file_path, msg),
        };

        let mut table_info = TableInfo {
            name: table_name.clone(),
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

        // Row count (best-effort; views may fail)
        if let Some(n) = utils::get_table_row_count(&conn, &quoted_table) {
            table_info.row_count = Some(n);
            total_rows += n;
        }

        let (foreign_keys, fk_columns) =
            match utils::get_foreign_keys_for_table(&conn, &quoted_table) {
                Ok(x) => x,
                Err(msg) => return handle_sqlite_error(&mut metadata, &stats.file_path, msg),
            };
        if !foreign_keys.is_empty() {
            table_info.foreign_keys = Some(foreign_keys);
        }
        if let Some(ref mut cols) = table_info.columns {
            for col in cols.iter_mut() {
                if fk_columns.contains(&col.name) {
                    col.is_foreign_key = Some(true);
                }
            }
        }

        ensure_column_stats(
            &conn,
            table_name,
            table_info.columns.as_deref_mut(),
            table_info.row_count.unwrap_or(0),
            config,
        );

        let indexes = match utils::get_indexes_for_table(&conn, table_name, &quoted_table) {
            Ok(ix) => ix,
            Err(msg) => return handle_sqlite_error(&mut metadata, &stats.file_path, msg),
        };
        if !indexes.is_empty() {
            table_info.indexes = Some(indexes);
        }

        tables.push(table_info);
    }

    metadata.total_rows = Some(total_rows);
    if !tables.is_empty() {
        metadata.tables = Some(tables);
    }

    Ok(metadata)
}

/// Fills null_percentage/unique_count and type-specific stats. No-op if `columns` is None.
/// Empty tables get no-data defaults; otherwise delegates to `compute_column_statistics`.
fn ensure_column_stats(
    conn: &Connection,
    table_name: &str,
    columns: Option<&mut [ColumnInfo]>,
    row_count: usize,
    config: &Config,
) {
    let Some(cols) = columns else { return };
    if row_count == 0 {
        for col in cols.iter_mut() {
            utils::set_no_data_column_stats(col);
        }
    } else {
        compute_column_statistics(conn, table_name, cols, config);
    }
}

/// Compute column statistics (null percentage, unique count, type-specific stats)
fn compute_column_statistics(
    conn: &Connection,
    table_name: &str,
    columns: &mut [ColumnInfo],
    config: &Config,
) {
    let quoted_table = utils::quote_sql_identifier(table_name);

    for col in columns.iter_mut() {
        let quoted_col = utils::quote_sql_identifier(&col.name);
        let values =
            fetch_column_as_strings(conn, &quoted_table, &quoted_col, table_name, &col.name);
        if values.is_empty() {
            utils::set_no_data_column_stats(col);
            continue;
        }

        let (null_pct, unique_count) = column_stats::compute_null_and_unique_stats(&values, config);
        col.null_percentage = Some(null_pct);
        col.unique_count = Some(unique_count);

        compute_stats_for_type(conn, &quoted_table, &quoted_col, col, &values, config);
    }
}

crate::no_template_mining!(
    extract_sqlite_templates,
    "SQLite files don't need template mining - return empty result. Only metadata is extracted (schema, tables, columns, etc.)."
);

/// Extract metadata and templates for SQLite; single file type in this module.
pub fn process(stats: &mut ParseResult, mmap: &Mmap, config: &Config) -> Result<MiningResult> {
    crate::process_with_metadata!(
        stats,
        mmap,
        config,
        sqlite_metadata,
        extract_sqlite_metadata(mmap, stats, config),
        crate::results::SqliteMetadata,
        FileType::Sqlite,
        extract_sqlite_templates(mmap, stats, config)
    )
}
