//! SQLite parser helpers

use std::collections::HashSet;
use std::error::Error;
use std::fs;

use anyhow::Context;
use log::debug;
use rusqlite::{Connection, OpenFlags};
use tempfile::NamedTempFile;

use crate::results::{ColumnInfo, ForeignKeyInfo, IndexInfo, SqliteMetadata};

/// Writes `content` to a temp file and opens a read-only SQLite connection.
/// Returns `(Some((conn, _temp_file)), metadata)` on success; the caller must hold
/// `_temp_file` for the lifetime of `conn`. On connection failure, sets
/// `metadata.error` and returns `(None, metadata)`. Propagates errors from
/// temp file creation or write.
pub(super) fn open_sqlite_connection(
    content: &[u8],
    byte_count: usize,
    file_path: &str,
) -> Result<(Option<(Connection, NamedTempFile)>, SqliteMetadata), anyhow::Error> {
    let mut metadata = SqliteMetadata {
        file_size: Some(byte_count),
        ..Default::default()
    };

    let temp_file = NamedTempFile::new().context("Failed to create temporary file for SQLite")?;
    let temp_path = temp_file.path();
    fs::write(temp_path, content).context("Failed to write SQLite content to temporary file")?;

    match Connection::open_with_flags(temp_path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(conn) => Ok((Some((conn, temp_file)), metadata)),
        Err(e) => {
            let error_msg = if let Some(source) = e.source() {
                format!("{}: {}", e, source)
            } else {
                format!("{}", e)
            };
            debug!(
                "SQLite database '{}' could not be opened: {}",
                file_path, error_msg
            );
            metadata.error = Some(error_msg);
            Ok((None, metadata))
        }
    }
}

const NULL_PERCENTAGE_DEFAULT: f64 = 100.0;
const UNIQUE_COUNT_DEFAULT: usize = 0;

/// Returns row count for a table (or view). `None` on error (e.g. view with no row count).
pub(super) fn get_table_row_count(conn: &Connection, quoted_table: &str) -> Option<usize> {
    conn.query_row(
        &format!("SELECT COUNT(*) FROM {};", quoted_table),
        [],
        |row| row.get::<_, i64>(0),
    )
    .ok()
    .map(|n| n as usize)
}

/// Fetches foreign keys for a table. Returns `(foreign_keys, set of column names that are FKs)`.
pub(super) fn get_foreign_keys_for_table(
    conn: &Connection,
    quoted_table: &str,
) -> Result<(Vec<ForeignKeyInfo>, HashSet<String>), String> {
    let mut fk_columns = HashSet::new();
    let mut foreign_keys = Vec::new();
    let sql = format!("PRAGMA foreign_key_list({});", quoted_table);
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Failed to prepare PRAGMA foreign_key_list for table: {}", e))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(|e| format!("Failed to query foreign_key_list: {}", e))?;
    for r in rows {
        let (_id, _seq, ref_table, from_col, to_col) =
            r.map_err(|e| format!("Failed to read foreign_key_list row: {}", e))?;
        fk_columns.insert(from_col.clone());
        foreign_keys.push(ForeignKeyInfo {
            column: from_col,
            references_table: ref_table,
            references_column: to_col,
        });
    }
    Ok((foreign_keys, fk_columns))
}

/// Fetches columns and primary key names for a table via PRAGMA table_info.
pub(super) fn get_table_columns(
    conn: &Connection,
    table_name: &str,
) -> Result<(Vec<ColumnInfo>, Vec<String>), String> {
    let quoted = quote_sql_identifier(table_name);
    let sql = format!("PRAGMA table_info({});", quoted);
    let mut stmt = conn.prepare(&sql).map_err(|e| {
        format!(
            "Failed to prepare PRAGMA query for table '{}': {}",
            table_name, e
        )
    })?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })
        .map_err(|e| {
            format!(
                "Failed to query table_info for table '{}': {}",
                table_name, e
            )
        })?;
    let mut columns = Vec::new();
    let mut pk_order: Vec<(String, i64)> = Vec::new();
    for r in rows {
        let (_cid, name, type_name, notnull, default_value, pk) = r.map_err(|e| {
            format!(
                "Failed to read column info for table '{}': {}",
                table_name, e
            )
        })?;
        let is_pk = pk > 0;
        if is_pk {
            pk_order.push((name.clone(), pk));
        }
        columns.push(ColumnInfo {
            name,
            type_name,
            not_null: Some(notnull != 0),
            default_value,
            is_primary_key: Some(is_pk),
            is_foreign_key: Some(false),
            null_percentage: None,
            unique_count: None,
            numeric_stats: None,
            date_stats: None,
            boolean_stats: None,
            text_stats: None,
            blob_stats: None,
        });
    }
    pk_order.sort_by_key(|(_, pk)| *pk);
    let primary_keys: Vec<String> = pk_order.into_iter().map(|(n, _)| n).collect();
    Ok((columns, primary_keys))
}

/// Fetches indexes for a table (names from sqlite_master, columns and unique from PRAGMAs).
pub(super) fn get_indexes_for_table(
    conn: &Connection,
    table_name: &str,
    quoted_table: &str,
) -> Result<Vec<IndexInfo>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT name FROM sqlite_master WHERE type='index' AND tbl_name=? AND name NOT LIKE 'sqlite_%'",
        )
        .map_err(|e| format!("Failed to prepare index names query for table '{}': {}", table_name, e))?;
    let index_names: Vec<String> = stmt
        .query_map([table_name], |row| row.get::<_, String>(0))
        .map_err(|e| {
            format!(
                "Failed to query index names for table '{}': {}",
                table_name, e
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| {
            format!(
                "Failed to collect index names for table '{}': {}",
                table_name, e
            )
        })?;
    let mut indexes = Vec::new();
    for index_name in &index_names {
        let quoted_index = quote_sql_identifier(index_name);
        let info_sql = format!("PRAGMA index_info({});", quoted_index);
        let mut info_stmt = conn.prepare(&info_sql).map_err(|e| {
            format!(
                "Failed to prepare PRAGMA index_info for '{}': {}",
                index_name, e
            )
        })?;
        let info_rows = info_stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| format!("Failed to query index_info for '{}': {}", index_name, e))?;
        let index_columns: Vec<String> = info_rows
            .map(|r| r.map(|(_, _, name)| name))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to read index_info for '{}': {}", index_name, e))?;
        let list_sql = format!("PRAGMA index_list({});", quoted_table);
        let mut list_stmt = conn.prepare(&list_sql).map_err(|e| {
            format!(
                "Failed to prepare PRAGMA index_list for table '{}': {}",
                table_name, e
            )
        })?;
        let list_rows = list_stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })
            .map_err(|e| format!("Failed to query index_list: {}", e))?;
        let mut is_unique = false;
        for r in list_rows {
            let (_seq, name, unique_val) =
                r.map_err(|e| format!("Failed to read index_list row: {}", e))?;
            if name == *index_name {
                is_unique = unique_val != 0;
                break;
            }
        }
        indexes.push(IndexInfo {
            name: index_name.clone(),
            table: table_name.to_string(),
            columns: index_columns,
            unique: Some(is_unique),
        });
    }
    Ok(indexes)
}

/// Fetches table names from sqlite_master (excluding system tables).
/// Returns `Ok(names)` or `Err(message)` on prepare/query/collect failure.
pub(super) fn get_table_names(conn: &Connection) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")
        .map_err(|e| format!("Failed to prepare table query: {}", e))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| format!("Failed to query table names: {}", e))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to collect table names: {}", e))
}

/// Runs a single-row query with no params; returns `Some` on success, `None` on error.
/// Used for best-effort extraction of DB-level metadata (page_size, encoding, version).
pub(super) fn try_query_one<F, T>(conn: &Connection, sql: &str, f: F) -> Option<T>
where
    F: FnOnce(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
{
    conn.query_row(sql, [], f).ok()
}

/// Quotes an SQL identifier (table/column/index name) for use in SQL.
/// Doubles any embedded double-quotes per SQLite rules.
pub(super) fn quote_sql_identifier(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// Sets null_percentage and unique_count to "no data" defaults (100% null, 0 unique).
pub(super) fn set_no_data_column_stats(col: &mut ColumnInfo) {
    col.null_percentage = Some(NULL_PERCENTAGE_DEFAULT);
    col.unique_count = Some(UNIQUE_COUNT_DEFAULT);
}
