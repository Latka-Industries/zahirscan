//! `SQLite` parser helpers

use std::collections::HashSet;
use std::error::Error;
use std::fs;

use anyhow::Context;
use log::debug;
use rusqlite::{Connection, OpenFlags};
use tempfile::NamedTempFile;

use crate::results::{ColumnInfo, ForeignKeyInfo, IndexInfo, SqliteMetadata};

/// Writes `content` to a temp file and opens a read-only `SQLite` connection.
/// Returns `(Some((conn, _temp_file)), metadata)` on success; the caller must hold
/// `_temp_file` for the lifetime of `conn`. On connection failure, sets
/// `metadata.error` and returns `(None, metadata)`. Propagates errors from
/// temp file creation or write.
pub(super) fn open_sqlite_connection(
    content_ref: &[u8],
    byte_count: usize,
    file_path_ref: &str,
) -> Result<(Option<(Connection, NamedTempFile)>, SqliteMetadata), anyhow::Error> {
    let mut metadata = SqliteMetadata {
        file_size: Some(byte_count),
        ..Default::default()
    };

    let temp_file = NamedTempFile::new().context("Failed to create temporary file for SQLite")?;
    let temp_path = temp_file.path();
    fs::write(temp_path, content_ref)
        .context("Failed to write SQLite content to temporary file")?;

    match Connection::open_with_flags(temp_path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(conn) => Ok((Some((conn, temp_file)), metadata)),
        Err(e) => {
            let error_msg = if let Some(source) = e.source() {
                format!("{e}: {source}")
            } else {
                format!("{e}")
            };
            debug!("SQLite database '{file_path_ref}' could not be opened: {error_msg}");
            metadata.error = Some(error_msg);
            Ok((None, metadata))
        }
    }
}

const NULL_PERCENTAGE_DEFAULT: f64 = 100.0;
const UNIQUE_COUNT_DEFAULT: usize = 0;

/// Maps `PRAGMA table_info` declared types to canonical names for stats dispatch.
/// Follows [SQLite 3 type affinity](https://www.sqlite.org/datatype3.html#determination_of_column_affinity)
/// (`INTEGER`, `TEXT`, `BLOB`, `REAL`, `NUMERIC`).
pub(super) fn canonical_sqlite_declared_type(type_name_ref: Option<&String>) -> Option<String> {
    let raw = type_name_ref?;
    let t = raw.trim();
    if t.is_empty() {
        return Some("BLOB".to_string());
    }
    let u = t.to_uppercase();
    if u.contains("INT") {
        return Some("INTEGER".to_string());
    }
    if u.contains("CHAR") || u.contains("CLOB") || u.contains("TEXT") {
        return Some("TEXT".to_string());
    }
    if u.contains("BLOB") {
        return Some("BLOB".to_string());
    }
    if u.contains("REAL") || u.contains("FLOA") || u.contains("DOUB") {
        return Some("REAL".to_string());
    }
    Some("NUMERIC".to_string())
}

/// Returns row count for a table (or view). `None` on error (e.g. view with no row count).
pub(super) fn get_table_row_count(conn_ref: &Connection, quoted_table_ref: &str) -> Option<usize> {
    conn_ref
        .query_row(
            &format!("SELECT COUNT(*) FROM {quoted_table_ref};"),
            [],
            |row| row.get::<_, i64>(0),
        )
        .ok()
        .and_then(|n| usize::try_from(n).ok())
}

/// Fetches foreign keys for a table. Returns `(foreign_keys, set of column names that are FKs)`.
pub(super) fn get_foreign_keys_for_table(
    conn_ref: &Connection,
    quoted_table_ref: &str,
) -> Result<(Vec<ForeignKeyInfo>, HashSet<String>), String> {
    let mut fk_columns = HashSet::new();
    let mut foreign_keys = Vec::new();
    let sql = format!("PRAGMA foreign_key_list({quoted_table_ref});");
    let mut stmt = conn_ref
        .prepare(&sql)
        .map_err(|e| format!("Failed to prepare PRAGMA foreign_key_list for table: {e}"))?;
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
        .map_err(|e| format!("Failed to query foreign_key_list: {e}"))?;
    for r in rows {
        let (_id, _seq, ref_table, from_col, to_col) =
            r.map_err(|e| format!("Failed to read foreign_key_list row: {e}"))?;
        fk_columns.insert(from_col.clone());
        foreign_keys.push(ForeignKeyInfo {
            column: from_col,
            references_table: ref_table,
            references_column: to_col,
        });
    }
    Ok((foreign_keys, fk_columns))
}

/// Fetches columns and primary key names for a table via PRAGMA `table_info`.
pub(super) fn get_table_columns(
    conn_ref: &Connection,
    table_name_ref: &str,
) -> Result<(Vec<ColumnInfo>, Vec<String>), String> {
    let quoted = quote_sql_identifier(table_name_ref);
    let sql = format!("PRAGMA table_info({quoted});");
    let mut stmt = conn_ref
        .prepare(&sql)
        .map_err(|e| format!("Failed to prepare PRAGMA query for table '{table_name_ref}': {e}"))?;
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
        .map_err(|e| format!("Failed to query table_info for table '{table_name_ref}': {e}"))?;
    let mut columns = Vec::new();
    let mut pk_order: Vec<(String, i64)> = Vec::new();
    for r in rows {
        let (_cid, name, type_name, notnull, default_value, pk) =
            r.map_err(|e| format!("Failed to read column info for table '{table_name_ref}': {e}"))?;
        let is_pk = pk > 0;
        if is_pk {
            pk_order.push((name.clone(), pk));
        }
        columns.push(ColumnInfo {
            name,
            type_name: canonical_sqlite_declared_type(type_name.as_ref()),
            not_null: Some(notnull != 0),
            default_value,
            is_primary_key: Some(is_pk),
            is_foreign_key: Some(false),
            ..Default::default()
        });
    }
    pk_order.sort_by_key(|(_, pk)| *pk);
    let primary_keys: Vec<String> = pk_order.into_iter().map(|(n, _)| n).collect();
    Ok((columns, primary_keys))
}

/// Fetches indexes for a table (names from `sqlite_master`, columns and unique from PRAGMAs).
pub(super) fn get_indexes_for_table(
    conn_ref: &Connection,
    table_name_ref: &str,
    quoted_table_ref: &str,
) -> Result<Vec<IndexInfo>, String> {
    let mut stmt = conn_ref
        .prepare(
            "SELECT name FROM sqlite_master WHERE type='index' AND tbl_name=? AND name NOT LIKE 'sqlite_%'",
        )
        .map_err(|e| format!("Failed to prepare index names query for table '{table_name_ref}': {e}"))?;
    let index_names: Vec<String> = stmt
        .query_map([table_name_ref], |row| row.get::<_, String>(0))
        .map_err(|e| format!("Failed to query index names for table '{table_name_ref}': {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to collect index names for table '{table_name_ref}': {e}"))?;
    let mut indexes = Vec::new();
    for index_name in &index_names {
        let quoted_index = quote_sql_identifier(index_name);
        let info_sql = format!("PRAGMA index_info({quoted_index});");
        let mut info_stmt = conn_ref
            .prepare(&info_sql)
            .map_err(|e| format!("Failed to prepare PRAGMA index_info for '{index_name}': {e}"))?;
        let info_rows = info_stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| format!("Failed to query index_info for '{index_name}': {e}"))?;
        let index_columns: Vec<String> = info_rows
            .map(|r| r.map(|(_, _, name)| name))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to read index_info for '{index_name}': {e}"))?;
        let list_sql = format!("PRAGMA index_list({quoted_table_ref});");
        let mut list_stmt = conn_ref.prepare(&list_sql).map_err(|e| {
            format!("Failed to prepare PRAGMA index_list for table '{table_name_ref}': {e}")
        })?;
        let list_rows = list_stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })
            .map_err(|e| format!("Failed to query index_list: {e}"))?;
        let mut is_unique = false;
        for r in list_rows {
            let (_seq, name, unique_val) =
                r.map_err(|e| format!("Failed to read index_list row: {e}"))?;
            if name == *index_name {
                is_unique = unique_val != 0;
                break;
            }
        }
        indexes.push(IndexInfo {
            name: index_name.clone(),
            table: table_name_ref.to_string(),
            columns: index_columns,
            unique: Some(is_unique),
        });
    }
    Ok(indexes)
}

/// Fetches table names from `sqlite_master` (excluding system tables).
/// Returns `Ok(names)` or `Err(message)` on prepare/query/collect failure.
pub(super) fn get_table_names(conn_ref: &Connection) -> Result<Vec<String>, String> {
    let mut stmt = conn_ref
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")
        .map_err(|e| format!("Failed to prepare table query: {e}"))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| format!("Failed to query table names: {e}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to collect table names: {e}"))
}

/// Runs a single-row query with no params; returns `Some` on success, `None` on error.
/// Used for best-effort extraction of DB-level metadata (`page_size`, encoding, version).
pub(super) fn try_query_one<F, T>(conn_ref: &Connection, sql_ref: &str, f: F) -> Option<T>
where
    F: FnOnce(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
{
    conn_ref.query_row(sql_ref, [], f).ok()
}

/// Quotes an SQL identifier (table/column/index name) for use in SQL.
/// Doubles any embedded double-quotes per `SQLite` rules.
pub(super) fn quote_sql_identifier(ident_ref: &str) -> String {
    format!("\"{}\"", ident_ref.replace('"', "\"\""))
}

/// Sets `null_percentage` and `unique_count` to "no data" defaults (100% null, 0 unique).
pub(super) fn set_no_data_column_stats(col_mut_ref: &mut ColumnInfo) {
    col_mut_ref.null_percentage = Some(NULL_PERCENTAGE_DEFAULT);
    col_mut_ref.unique_count = Some(UNIQUE_COUNT_DEFAULT);
}
