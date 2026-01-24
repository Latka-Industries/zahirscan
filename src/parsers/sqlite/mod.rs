//! SQLite database metadata extraction

use super::ParseResult;
use super::traits::empty_mining_result;
use crate::config::Config;
use crate::parsers::column_stats;
use crate::results::{BlobStats, ColumnInfo, ForeignKeyInfo, IndexInfo, SqliteMetadata, TableInfo, TextStats};
use anyhow::{Context, Result};
use log::debug;
use rusqlite::{Connection, OpenFlags};
use std::error::Error;
use std::fs;
use tempfile::NamedTempFile;

/// Extract SQLite metadata from database file
pub fn extract_sqlite_metadata(
    content: &[u8],
    stats: &ParseResult,
    config: &Config,
) -> Result<SqliteMetadata> {
    let mut metadata = SqliteMetadata {
        file_size: Some(stats.byte_count),
        ..Default::default()
    };

    // SQLite needs a file path, not bytes - write to temporary file
    let temp_file = NamedTempFile::new().context("Failed to create temporary file for SQLite")?;
    let temp_path = temp_file.path();

    // Write content to temp file
    fs::write(temp_path, content).context("Failed to write SQLite content to temporary file")?;

    // Open SQLite connection in read-only mode
    let conn = match Connection::open_with_flags(temp_path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(c) => c,
        Err(e) => {
            // Extract clean error message (just the error code and message, not the full chain)
            let error_msg = if let Some(source) = e.source() {
                format!("{}: {}", e, source)
            } else {
                format!("{}", e)
            };
            debug!(
                "SQLite database '{}' could not be opened: {}",
                stats.file_path, error_msg
            );
            metadata.error = Some(error_msg);
            return Ok(metadata);
        }
    };

    // Extract database statistics
    if let Ok(page_size) = conn.query_row("PRAGMA page_size;", [], |row| row.get::<_, i64>(0)) {
        metadata.page_size = Some(page_size as usize);
    }

    if let Ok(encoding) = conn.query_row("PRAGMA encoding;", [], |row| row.get::<_, String>(0)) {
        metadata.encoding = Some(encoding);
    }

    if let Ok(version) = conn.query_row("SELECT sqlite_version();", [], |row| {
        row.get::<_, String>(0)
    }) {
        metadata.sqlite_version = Some(version);
    }

    // Extract schema information
    let mut tables = Vec::new();
    let mut total_rows = 0usize;

    // Get all table names (excluding system tables)
    let table_names: Vec<String> = match conn.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'") {
        Ok(mut stmt) => {
            match stmt.query_map([], |row| row.get::<_, String>(0)) {
                Ok(rows) => {
                    match rows.collect::<Result<Vec<_>, _>>() {
                        Ok(names) => names,
                        Err(e) => {
                            let error_msg = format!("Failed to collect table names: {}", e);
                            debug!("SQLite error for '{}': {}", stats.file_path, error_msg);
                            metadata.error = Some(error_msg);
                            return Ok(metadata);
                        }
                    }
                },
                Err(e) => {
                    let error_msg = format!("Failed to query table names: {}", e);
                    debug!("SQLite error for '{}': {}", stats.file_path, error_msg);
                    metadata.error = Some(error_msg);
                    return Ok(metadata);
                }
            }
        },
        Err(e) => {
            let error_msg = format!("Failed to prepare table query: {}", e);
            debug!("SQLite error for '{}': {}", stats.file_path, error_msg);
            metadata.error = Some(error_msg);
            return Ok(metadata);
        }
    };

    metadata.table_count = Some(table_names.len());

    for table_name in &table_names {
        let mut table_info = TableInfo {
            name: table_name.clone(),
            ..Default::default()
        };

        // Get column information
        let mut columns = Vec::new();
        let mut pk_order: Vec<(String, i64)> = Vec::new();

        // PRAGMA doesn't support parameters, so we need to use string formatting
        // Table names are from sqlite_master, so they should be safe, but we'll quote them
        let quoted_table = format!("\"{}\"", table_name.replace('"', "\"\""));
        let pragma_query = format!("PRAGMA table_info({});", quoted_table);
        let mut stmt = match conn.prepare(&pragma_query) {
            Ok(s) => s,
            Err(e) => {
                let error_msg = format!("Failed to prepare PRAGMA query for table '{}': {}", table_name, e);
                debug!("SQLite error for '{}': {}", stats.file_path, error_msg);
                metadata.error = Some(error_msg);
                return Ok(metadata);
            }
        };
        let column_rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,            // cid
                row.get::<_, String>(1)?,         // name
                row.get::<_, Option<String>>(2)?, // type
                row.get::<_, i64>(3)?,            // notnull
                row.get::<_, Option<String>>(4)?, // dflt_value
                row.get::<_, i64>(5)?,            // pk
            ))
        })?;

        for row_result in column_rows {
            let (_cid, name, type_name, notnull, default_value, pk) = match row_result {
                Ok(row) => row,
                Err(e) => {
                    let error_msg = format!("Failed to read column info for table '{}': {}", table_name, e);
                    debug!("SQLite error for '{}': {}", stats.file_path, error_msg);
                    metadata.error = Some(error_msg);
                    return Ok(metadata);
                }
            };

            let is_pk = pk > 0;
            if is_pk {
                pk_order.push((name.clone(), pk));
            }

            columns.push(ColumnInfo {
                name: name.clone(),
                type_name,
                not_null: Some(notnull != 0),
                default_value,
                is_primary_key: Some(is_pk),
                is_foreign_key: Some(false), // Will be updated below
                null_percentage: None,       // Will be computed later
                unique_count: None,          // Will be computed later
                numeric_stats: None,         // Will be computed later
                date_stats: None,            // Will be computed later
                boolean_stats: None,         // Will be computed later
                text_stats: None,            // Will be computed later
                blob_stats: None,            // Will be computed later
            });
        }

        // Sort primary keys by pk value to get correct order
        pk_order.sort_by_key(|(_, pk)| *pk);
        let primary_keys: Vec<String> = pk_order.into_iter().map(|(name, _)| name).collect();
        if !primary_keys.is_empty() {
            table_info.primary_keys = Some(primary_keys);
        }

        table_info.column_count = Some(columns.len());
        if !columns.is_empty() {
            table_info.columns = Some(columns.clone());
        }

        // Get row count (handle views gracefully)
        // quoted_table already defined above
        if let Ok(row_count) = conn.query_row(
            &format!("SELECT COUNT(*) FROM {};", quoted_table),
            [],
            |row| row.get::<_, i64>(0),
        ) {
            table_info.row_count = Some(row_count as usize);
            total_rows += row_count as usize;
        }

        // Get foreign keys
        let mut foreign_keys = Vec::new();
        let mut fk_columns = std::collections::HashSet::new();

        // PRAGMA doesn't support parameters
        let fk_pragma_query = format!("PRAGMA foreign_key_list({});", quoted_table);
        let mut fk_stmt = conn.prepare(&fk_pragma_query)?;
        let fk_rows = fk_stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,    // id
                row.get::<_, i64>(1)?,    // seq
                row.get::<_, String>(2)?, // table (referenced)
                row.get::<_, String>(3)?, // from (column in this table)
                row.get::<_, String>(4)?, // to (column in referenced table)
            ))
        })?;

        for fk_result in fk_rows {
            let (_id, _seq, ref_table, from_col, to_col) = fk_result?;
            fk_columns.insert(from_col.clone());
            foreign_keys.push(ForeignKeyInfo {
                column: from_col,
                references_table: ref_table,
                references_column: to_col,
            });
        }

        if !foreign_keys.is_empty() {
            table_info.foreign_keys = Some(foreign_keys);
        }

        // Mark foreign key columns in column info
        if let Some(ref mut cols) = table_info.columns {
            for col in cols.iter_mut() {
                if fk_columns.contains(&col.name) {
                    col.is_foreign_key = Some(true);
                }
            }
        }

        // Compute column statistics (null percentage, unique count, type-specific stats)
        if let Some(ref mut cols) = table_info.columns {
            let row_count = table_info.row_count.unwrap_or(0);
            if row_count == 0 {
                // For empty tables, set basic statistics to indicate no data
                for col in cols.iter_mut() {
                    col.null_percentage = Some(100.0);
                    col.unique_count = Some(0);
                }
            } else {
                // Process all rows - adaptive chunking in statistics computation handles large datasets efficiently
                compute_column_statistics(&conn, table_name, cols, config);
            }
        }

        // Get indexes
        let mut indexes = Vec::new();

        // Get index names for this table (this query can use parameters)
        let index_names: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='index' AND tbl_name=? AND name NOT LIKE 'sqlite_%'")?
            .query_map([table_name], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;

        for index_name in &index_names {
            let mut index_columns = Vec::new();
            let mut is_unique = false;

            // Get index info (columns) - PRAGMA doesn't support parameters
            let quoted_index = format!("\"{}\"", index_name.replace('"', "\"\""));
            let idx_pragma_query = format!("PRAGMA index_info({});", quoted_index);
            let mut idx_stmt = conn.prepare(&idx_pragma_query)?;
            let idx_rows = idx_stmt.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,         // seqno
                    row.get::<_, Option<i64>>(1)?, // cid
                    row.get::<_, String>(2)?,      // name
                ))
            })?;

            for idx_result in idx_rows {
                let (_seqno, _cid, col_name) = idx_result?;
                index_columns.push(col_name);
            }

            // Get index list to check if unique
            // PRAGMA index_list returns: seq, name, unique, origin, partial
            // PRAGMA doesn't support parameters
            let idx_list_pragma_query = format!("PRAGMA index_list({});", quoted_table);
            let mut idx_list_stmt = conn.prepare(&idx_list_pragma_query)?;
            let idx_list_rows = idx_list_stmt.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,    // seq
                    row.get::<_, String>(1)?, // name
                    row.get::<_, i64>(2)?,    // unique
                ))
            })?;

            for (_seq, name, unique_val) in idx_list_rows.flatten() {
                if name == *index_name {
                    is_unique = unique_val != 0;
                    break;
                }
            }

            indexes.push(IndexInfo {
                name: index_name.clone(),
                table: table_name.clone(),
                columns: index_columns,
                unique: Some(is_unique),
            });
        }

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

/// Compute column statistics (null percentage, unique count, type-specific stats)
fn compute_column_statistics(
    conn: &Connection,
    table_name: &str,
    columns: &mut [ColumnInfo],
    config: &Config,
) {
    let quoted_table = format!("\"{}\"", table_name.replace('"', "\"\""));

    for col in columns.iter_mut() {
        // Query column data - adaptive chunking in statistics computation handles large datasets efficiently
        // Use CAST to TEXT to ensure proper string conversion for all types (INTEGER, REAL, TEXT, BLOB)
        let quoted_col = format!("\"{}\"", col.name.replace('"', "\"\""));
        let query = format!("SELECT CAST({} AS TEXT) FROM {};", quoted_col, quoted_table);

        // Query as strings first to get all values including NULLs
        let all_values: Vec<Option<String>> = match conn.prepare(&query) {
            Ok(mut stmt) => stmt
                .query_map([], |row| row.get::<_, Option<String>>(0))
                .ok()
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
                .unwrap_or_default(),
            Err(e) => {
                // Log debug but don't skip - we'll compute stats with empty data
                debug!(
                    "SQLite query failed for column '{}' in table '{}': {}",
                    col.name, table_name, e
                );
                Vec::new()
            }
        };

        let total_rows = all_values.len();
        if total_rows == 0 {
            // Even if no data, set null_percentage to indicate we checked
            col.null_percentage = Some(100.0);
            col.unique_count = Some(0);
            continue;
        }

        // Convert to Vec<String> for statistics (None -> empty string for null handling)
        let values: Vec<String> = all_values
            .iter()
            .map(|v| v.clone().unwrap_or_default())
            .collect();

        // Compute null percentage and unique count
        let (null_pct, unique_count) = column_stats::compute_null_and_unique_stats(&values, config);
        col.null_percentage = Some(null_pct);
        col.unique_count = Some(unique_count);

        // Compute type-specific statistics based on SQLite type
        match col.type_name.as_deref() {
            Some("INTEGER") | Some("REAL") => {
                // For numeric types, query directly as f64 for better performance
                // Use the original column name (not CAST to TEXT) for f64 query
                let f64_query = format!("SELECT \"{}\" FROM {};", quoted_col, quoted_table);

                let numeric_values: Vec<f64> = match conn.prepare(&f64_query) {
                    Ok(mut stmt) => stmt
                        .query_map([], |row| row.get::<_, Option<f64>>(0))
                        .ok()
                        .map(|rows| {
                            rows.filter_map(|r| r.ok().flatten())
                                .filter(|&v| v.is_finite())
                                .collect()
                        })
                        .unwrap_or_default(),
                    Err(_) => {
                        // Fallback: parse from strings if direct query fails
                        // Filter out empty/null strings and parse as f64
                        values
                            .iter()
                            .filter_map(|v| {
                                if v.is_empty()
                                    || v.eq_ignore_ascii_case("null")
                                    || v.eq_ignore_ascii_case("nil")
                                {
                                    None
                                } else {
                                    v.parse::<f64>().ok().filter(|&v| v.is_finite())
                                }
                            })
                            .collect()
                    }
                };

                // Compute numeric statistics if we have any numeric values
                if !numeric_values.is_empty() {
                    col.numeric_stats =
                        column_stats::compute_numeric_stats_from_values(numeric_values);
                }

                // Check if INTEGER column might be boolean-like (0/1 values)
                // Only check if we have non-empty string values
                if col.type_name.as_deref() == Some("INTEGER") && !values.is_empty() {
                    // Check if all non-empty values are boolean-like
                    let non_empty_values: Vec<&String> =
                        values.iter().filter(|v| !v.is_empty()).collect();
                    if !non_empty_values.is_empty() {
                        let bool_like =
                            non_empty_values.iter().all(|v| crate::tools::is_boolean(v));
                        if bool_like {
                            col.boolean_stats =
                                column_stats::compute_boolean_stats_from_strings(&values, config);
                        }
                    }
                }
            }
            Some("TEXT") => {
                // Compute text statistics (min/max/avg length)
                if let Some((min_len, max_len, avg_len, unique_ct)) =
                    column_stats::compute_text_stats_from_strings(&values, config)
                {
                    col.text_stats = Some(TextStats {
                        min_length: Some(min_len),
                        max_length: Some(max_len),
                        avg_length: Some(avg_len),
                    });
                    // Update unique count from text stats (more accurate)
                    col.unique_count = Some(unique_ct);
                }

                // Try to detect if TEXT column contains dates
                let date_like_count = values
                    .iter()
                    .filter(|v| {
                        !v.is_empty()
                            && (crate::tools::parse_timestamp_to_seconds(v).is_some()
                                || crate::tools::parse_date_to_timestamp(v).is_some())
                    })
                    .count();

                // If >50% of values look like dates, compute date stats
                if date_like_count as f64 / values.len() as f64 > 0.5 {
                    col.date_stats = column_stats::compute_date_stats_from_strings(&values);
                }
            }
            Some("BLOB") => {
                // Compute BLOB size statistics (min/max/avg size in bytes)
                // Use length() function to get blob size without loading the actual blob data
                // quoted_col already contains quotes, so use it directly in length()
                let blob_size_query = format!("SELECT length({}) FROM {} WHERE {} IS NOT NULL;", quoted_col, quoted_table, quoted_col);
                let blob_sizes: Vec<usize> = match conn.prepare(&blob_size_query) {
                    Ok(mut stmt) => {
                        stmt.query_map([], |row| row.get::<_, Option<i64>>(0))
                            .ok()
                            .map(|rows| {
                                rows.filter_map(|r| r.ok().flatten().map(|s| s as usize))
                                    .collect()
                            })
                            .unwrap_or_default()
                    }
                    Err(_) => Vec::new(),
                };

                if !blob_sizes.is_empty() {
                    let min_size = *blob_sizes.iter().min().unwrap();
                    let max_size = *blob_sizes.iter().max().unwrap();
                    let avg_size = blob_sizes.iter().sum::<usize>() as f64 / blob_sizes.len() as f64;

                    col.blob_stats = Some(BlobStats {
                        min_size: Some(min_size),
                        max_size: Some(max_size),
                        avg_size: Some(avg_size),
                    });
                }
            }
            _ => {
                // NULL type or unknown - skip type-specific stats
            }
        }
    }
}

/// Extract templates from SQLite files
/// SQLite files don't need template mining - we only extract metadata
pub fn extract_sqlite_templates(
    _content: &[u8],
    stats: &ParseResult,
    _config: &Config,
) -> Result<super::MiningResult> {
    // SQLite files don't need template mining - return empty result
    // Only metadata is extracted (schema, tables, columns, etc.)
    Ok(empty_mining_result(stats))
}
