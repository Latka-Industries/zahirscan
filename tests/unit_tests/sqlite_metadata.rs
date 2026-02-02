//! Tests for SQLite metadata extraction

use crate::get_test_config;
use rusqlite::Connection;
use std::fs;
use std::time::Duration;
use tempfile::NamedTempFile;
use zahirscan::parsers::sqlite::extract_sqlite_metadata;
use zahirscan::parsers::{FileType, ParseResult};

fn get_test_stats(file_path: &str, byte_count: usize) -> ParseResult {
    ParseResult {
        file_path: file_path.to_string(),
        file_type: FileType::Sqlite,
        line_count: 0,
        byte_count,
        token_count: 0,
        duration: Duration::ZERO,
        is_binary: true,
        ..Default::default()
    }
}

/// Create a simple database with a users table
fn create_simple_db() -> Vec<u8> {
    let temp_file = NamedTempFile::new().unwrap();
    let temp_path = temp_file.path();

    let conn = Connection::open(temp_path).unwrap();
    conn.execute(
        "CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT,
            age INTEGER,
            created_at TEXT
        )",
        [],
    )
    .unwrap();

    // Insert test data
    for i in 1..=10 {
        conn.execute(
            "INSERT INTO users (name, email, age, created_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                format!("User {}", i),
                format!("user{}@example.com", i),
                20 + i,
                format!("2025-01-{:02}", i)
            ],
        )
        .unwrap();
    }

    drop(conn);
    fs::read(temp_path).unwrap()
}

/// Create a relational database with foreign keys
fn create_relational_db() -> Vec<u8> {
    let temp_file = NamedTempFile::new().unwrap();
    let temp_path = temp_file.path();

    let conn = Connection::open(temp_path).unwrap();

    // Enable foreign keys
    conn.execute("PRAGMA foreign_keys = ON", []).unwrap();

    // Create customers table
    conn.execute(
        "CREATE TABLE customers (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT
        )",
        [],
    )
    .unwrap();

    // Create orders table with foreign key
    conn.execute(
        "CREATE TABLE orders (
            id INTEGER PRIMARY KEY,
            customer_id INTEGER NOT NULL,
            order_date TEXT,
            total REAL,
            FOREIGN KEY (customer_id) REFERENCES customers(id)
        )",
        [],
    )
    .unwrap();

    // Create order_items table
    conn.execute(
        "CREATE TABLE order_items (
            id INTEGER PRIMARY KEY,
            order_id INTEGER NOT NULL,
            product_name TEXT,
            quantity INTEGER,
            price REAL,
            FOREIGN KEY (order_id) REFERENCES orders(id)
        )",
        [],
    )
    .unwrap();

    // Insert test data
    conn.execute(
        "INSERT INTO customers (name, email) VALUES ('Alice', 'alice@example.com')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO customers (name, email) VALUES ('Bob', 'bob@example.com')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO orders (customer_id, order_date, total) VALUES (1, '2025-01-01', 100.0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO orders (customer_id, order_date, total) VALUES (2, '2025-01-02', 200.0)",
        [],
    )
    .unwrap();

    conn.execute("INSERT INTO order_items (order_id, product_name, quantity, price) VALUES (1, 'Widget', 2, 50.0)", []).unwrap();
    conn.execute("INSERT INTO order_items (order_id, product_name, quantity, price) VALUES (2, 'Gadget', 1, 200.0)", []).unwrap();

    drop(conn);
    fs::read(temp_path).unwrap()
}

/// Create a database with indexes
fn create_indexed_db() -> Vec<u8> {
    let temp_file = NamedTempFile::new().unwrap();
    let temp_path = temp_file.path();

    let conn = Connection::open(temp_path).unwrap();

    conn.execute(
        "CREATE TABLE products (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            sku TEXT NOT NULL,
            price REAL,
            category TEXT
        )",
        [],
    )
    .unwrap();

    // Create unique index on sku
    conn.execute("CREATE UNIQUE INDEX idx_sku ON products(sku)", [])
        .unwrap();

    // Create non-unique index on category
    conn.execute("CREATE INDEX idx_category ON products(category)", [])
        .unwrap();

    // Insert test data
    for i in 1..=20 {
        conn.execute(
            "INSERT INTO products (name, sku, price, category) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                format!("Product {}", i),
                format!("SKU-{:04}", i),
                10.0 * i as f64,
                if i % 2 == 0 {
                    "Electronics"
                } else {
                    "Clothing"
                }
            ],
        )
        .unwrap();
    }

    drop(conn);
    fs::read(temp_path).unwrap()
}

/// Create a database with various data types and statistics
fn create_types_db() -> Vec<u8> {
    let temp_file = NamedTempFile::new().unwrap();
    let temp_path = temp_file.path();

    let conn = Connection::open(temp_path).unwrap();

    conn.execute(
        "CREATE TABLE test_types (
            integer_field INTEGER,
            real_field REAL,
            text_field TEXT,
            blob_field BLOB,
            boolean_field INTEGER,
            date_field TEXT,
            null_field TEXT
        )",
        [],
    )
    .unwrap();

    // Insert test data with various types
    for i in 1..=10 {
        let blob_data = format!("blob data {}", i).into_bytes();
        conn.execute(
            "INSERT INTO test_types (integer_field, real_field, text_field, blob_field, boolean_field, date_field, null_field)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                i,
                i as f64 * 1.5,
                format!("text {}", i),
                blob_data,
                i % 2, // 0 or 1 for boolean
                format!("2025-01-{:02}", i),
                if i % 3 == 0 { None::<String> } else { Some(format!("value {}", i)) }
            ],
        ).unwrap();
    }

    drop(conn);
    fs::read(temp_path).unwrap()
}

/// Create a complex database with multiple tables and constraints
fn create_complex_db() -> Vec<u8> {
    let temp_file = NamedTempFile::new().unwrap();
    let temp_path = temp_file.path();

    let conn = Connection::open(temp_path).unwrap();
    conn.execute("PRAGMA foreign_keys = ON", []).unwrap();

    // Departments table
    conn.execute(
        "CREATE TABLE departments (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            budget REAL
        )",
        [],
    )
    .unwrap();

    // Employees table with foreign key
    conn.execute(
        "CREATE TABLE employees (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            department_id INTEGER,
            salary REAL,
            FOREIGN KEY (department_id) REFERENCES departments(id)
        )",
        [],
    )
    .unwrap();

    // Projects table
    conn.execute(
        "CREATE TABLE projects (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            start_date TEXT,
            end_date TEXT
        )",
        [],
    )
    .unwrap();

    // Insert test data
    conn.execute(
        "INSERT INTO departments (name, budget) VALUES ('Engineering', 100000.0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO departments (name, budget) VALUES ('Sales', 50000.0)",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO employees (name, department_id, salary) VALUES ('Alice', 1, 75000.0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO employees (name, department_id, salary) VALUES ('Bob', 1, 80000.0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO employees (name, department_id, salary) VALUES ('Charlie', 2, 60000.0)",
        [],
    )
    .unwrap();

    conn.execute("INSERT INTO projects (name, start_date, end_date) VALUES ('Project A', '2025-01-01', '2025-06-30')", []).unwrap();
    conn.execute("INSERT INTO projects (name, start_date, end_date) VALUES ('Project B', '2025-02-01', '2025-07-31')", []).unwrap();

    drop(conn);
    fs::read(temp_path).unwrap()
}

#[test]
fn test_simple_database_metadata() {
    let db_content = create_simple_db();
    let stats = get_test_stats("simple.db", db_content.len());
    let config = get_test_config();

    let metadata = extract_sqlite_metadata(&db_content, &stats, &config).unwrap();

    // Basic database statistics
    assert!(metadata.file_size.is_some());
    assert!(metadata.page_size.is_some());
    assert!(metadata.sqlite_version.is_some());
    assert!(metadata.encoding.is_some());
    assert!(metadata.table_count.is_some());
    assert!(metadata.total_rows.is_some());

    // Should have one table
    assert_eq!(metadata.table_count, Some(1));
    assert!(metadata.tables.is_some());

    let tables = metadata.tables.unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].name, "users");
}

#[test]
fn test_simple_database_schema() {
    let db_content = create_simple_db();
    let stats = get_test_stats("simple.db", db_content.len());
    let config = get_test_config();

    let metadata = extract_sqlite_metadata(&db_content, &stats, &config).unwrap();
    let tables = metadata.tables.unwrap();
    let users_table = &tables[0];

    // Table metadata
    assert_eq!(users_table.name, "users");
    assert!(users_table.row_count.is_some());
    assert!(users_table.column_count.is_some());
    assert!(users_table.columns.is_some());

    let columns = users_table.columns.as_ref().unwrap();
    assert!(columns.len() >= 5); // id, name, email, age, created_at

    // Check column names
    let column_names: Vec<&str> = columns.iter().map(|c| c.name.as_str()).collect();
    assert!(column_names.contains(&"id"));
    assert!(column_names.contains(&"name"));
    assert!(column_names.contains(&"email"));
}

#[test]
fn test_simple_database_column_types() {
    let db_content = create_simple_db();
    let stats = get_test_stats("simple.db", db_content.len());
    let config = get_test_config();

    let metadata = extract_sqlite_metadata(&db_content, &stats, &config).unwrap();
    let tables = metadata.tables.unwrap();
    let columns = tables[0].columns.as_ref().unwrap();

    // Find id column (should be INTEGER)
    let id_col = columns.iter().find(|c| c.name == "id").unwrap();
    assert!(id_col.type_name.is_some());
    let type_name = id_col.type_name.as_ref().unwrap();
    assert!(type_name == "INTEGER" || type_name == "INT");

    // Find name column (should be TEXT)
    let name_col = columns.iter().find(|c| c.name == "name").unwrap();
    assert!(name_col.type_name.is_some());
    let type_name = name_col.type_name.as_ref().unwrap();
    assert!(type_name == "TEXT" || type_name == "VARCHAR");
}

#[test]
fn test_simple_database_primary_key() {
    let db_content = create_simple_db();
    let stats = get_test_stats("simple.db", db_content.len());
    let config = get_test_config();

    let metadata = extract_sqlite_metadata(&db_content, &stats, &config).unwrap();
    let tables = metadata.tables.unwrap();
    let users_table = &tables[0];

    // Check primary key
    if let Some(primary_keys) = &users_table.primary_keys {
        assert!(primary_keys.contains(&"id".to_string()));
    }

    // Check is_primary_key flag on columns
    let columns = users_table.columns.as_ref().unwrap();
    let id_col = columns.iter().find(|c| c.name == "id").unwrap();
    assert!(id_col.is_primary_key.is_some());
    assert_eq!(id_col.is_primary_key, Some(true));
}

#[test]
fn test_relational_database_foreign_keys() {
    let db_content = create_relational_db();
    let stats = get_test_stats("relational.db", db_content.len());
    let config = get_test_config();

    let metadata = extract_sqlite_metadata(&db_content, &stats, &config).unwrap();

    // Should have multiple tables
    assert!(metadata.table_count.is_some());
    let table_count = metadata.table_count.unwrap();
    assert!(table_count >= 3); // customers, orders, order_items

    let tables = metadata.tables.unwrap();

    // Find orders table (should have foreign key to customers)
    let orders_table = tables.iter().find(|t| t.name == "orders");
    if let Some(orders) = orders_table {
        if let Some(foreign_keys) = &orders.foreign_keys {
            assert!(!foreign_keys.is_empty());
            let fk = foreign_keys.iter().find(|fk| fk.column == "customer_id");
            assert!(fk.is_some());
            if let Some(fk) = fk {
                assert_eq!(fk.references_table, "customers");
            }
        }

        // Check is_foreign_key flag on columns
        if let Some(columns) = &orders.columns {
            let customer_id_col = columns.iter().find(|c| c.name == "customer_id");
            if let Some(col) = customer_id_col {
                assert!(col.is_foreign_key.is_some());
                assert_eq!(col.is_foreign_key, Some(true));
            }
        }
    }
}

#[test]
fn test_indexed_database_indexes() {
    let db_content = create_indexed_db();
    let stats = get_test_stats("indexed.db", db_content.len());
    let config = get_test_config();

    let metadata = extract_sqlite_metadata(&db_content, &stats, &config).unwrap();
    let tables = metadata.tables.unwrap();

    // Find products table
    let products_table = tables.iter().find(|t| t.name == "products");
    if let Some(products) = products_table
        && let Some(indexes) = &products.indexes
    {
        assert!(!indexes.is_empty());

        // Check for unique index on sku
        let sku_index = indexes
            .iter()
            .find(|idx| idx.columns.contains(&"sku".to_string()) && idx.unique == Some(true));
        assert!(sku_index.is_some());
    }
}

#[test]
fn test_types_database_column_statistics() {
    let db_content = create_types_db();
    let stats = get_test_stats("types.db", db_content.len());
    let config = get_test_config();

    let metadata = extract_sqlite_metadata(&db_content, &stats, &config).unwrap();
    let tables = metadata.tables.unwrap();
    let columns = tables[0].columns.as_ref().unwrap();

    // Find integer column
    let integer_col = columns.iter().find(|c| c.name == "integer_field");
    if let Some(col) = integer_col {
        // Should have numeric stats if column has values
        if col.null_percentage != Some(100.0) {
            assert!(col.numeric_stats.is_some());
            let stats = col.numeric_stats.as_ref().unwrap();
            assert!(stats.min.is_some() || stats.max.is_some());
        }
    }

    // Find text column
    let text_col = columns.iter().find(|c| c.name == "text_field");
    if let Some(col) = text_col {
        // Should have text stats if column has values
        if col.null_percentage != Some(100.0) {
            assert!(col.text_stats.is_some());
            let stats = col.text_stats.as_ref().unwrap();
            assert!(stats.min_length.is_some() || stats.max_length.is_some());
        }
    }

    // Find boolean column
    let boolean_col = columns.iter().find(|c| c.name == "boolean_field");
    if let Some(col) = boolean_col {
        // Should have boolean stats if column has values
        if col.null_percentage != Some(100.0) {
            assert!(col.boolean_stats.is_some());
        }
    }

    // Find blob column
    let blob_col = columns.iter().find(|c| c.name == "blob_field");
    if let Some(col) = blob_col {
        // Should have blob stats if column has values
        if col.null_percentage != Some(100.0) {
            assert!(col.blob_stats.is_some());
            let stats = col.blob_stats.as_ref().unwrap();
            assert!(stats.min_size.is_some() || stats.max_size.is_some());
        }
    }
}

#[test]
fn test_types_database_null_percentages() {
    let db_content = create_types_db();
    let stats = get_test_stats("types.db", db_content.len());
    let config = get_test_config();

    let metadata = extract_sqlite_metadata(&db_content, &stats, &config).unwrap();
    let tables = metadata.tables.unwrap();
    let columns = tables[0].columns.as_ref().unwrap();

    // All columns should have null_percentage set
    for col in columns {
        assert!(col.null_percentage.is_some());
        let null_pct = col.null_percentage.unwrap();
        assert!((0.0..=100.0).contains(&null_pct));
    }
}

#[test]
fn test_types_database_unique_counts() {
    let db_content = create_types_db();
    let stats = get_test_stats("types.db", db_content.len());
    let config = get_test_config();

    let metadata = extract_sqlite_metadata(&db_content, &stats, &config).unwrap();
    let tables = metadata.tables.unwrap();
    let columns = tables[0].columns.as_ref().unwrap();

    // All columns should have unique_count set
    for col in columns {
        assert!(col.unique_count.is_some());
        let _unique_count = col.unique_count.unwrap();
        // unique_count is usize, so it's always >= 0
    }
}

#[test]
fn test_complex_database_multiple_tables() {
    let db_content = create_complex_db();
    let stats = get_test_stats("complex.db", db_content.len());
    let config = get_test_config();

    let metadata = extract_sqlite_metadata(&db_content, &stats, &config).unwrap();

    assert!(metadata.table_count.is_some());
    let table_count = metadata.table_count.unwrap();
    assert!(table_count >= 3); // departments, employees, projects

    let tables = metadata.tables.unwrap();
    let table_names: Vec<&str> = tables.iter().map(|t| t.name.as_str()).collect();
    assert!(table_names.contains(&"departments"));
    assert!(table_names.contains(&"employees"));
    assert!(table_names.contains(&"projects"));
}

#[test]
fn test_complex_database_constraints() {
    let db_content = create_complex_db();
    let stats = get_test_stats("complex.db", db_content.len());
    let config = get_test_config();

    let metadata = extract_sqlite_metadata(&db_content, &stats, &config).unwrap();
    let tables = metadata.tables.unwrap();

    // Check for NOT NULL constraints
    for table in &tables {
        if let Some(columns) = &table.columns {
            for col in columns {
                if col.not_null == Some(true) {
                    // NOT NULL columns should have null_percentage = 0.0 (or very close)
                    if let Some(null_pct) = col.null_percentage {
                        assert!(
                            null_pct < 1.0,
                            "NOT NULL column should have null_percentage < 1.0"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn test_database_statistics() {
    let db_content = create_simple_db();
    let stats = get_test_stats("simple.db", db_content.len());
    let config = get_test_config();

    let metadata = extract_sqlite_metadata(&db_content, &stats, &config).unwrap();

    // Page size should be a power of 2 (typically 4096)
    if let Some(page_size) = metadata.page_size {
        assert!(page_size > 0);
        assert!(page_size % 2 == 0); // Should be even
    }

    // Encoding should be UTF-8 or UTF-16
    if let Some(encoding) = metadata.encoding {
        assert!(encoding == "UTF-8" || encoding == "UTF-16le" || encoding == "UTF-16be");
    }

    // SQLite version should be present
    if let Some(version) = metadata.sqlite_version {
        assert!(!version.is_empty());
        // Version format is typically "3.x.x"
        assert!(version.starts_with("3."));
    }
}

#[test]
fn test_database_row_counts() {
    let db_content = create_simple_db();
    let stats = get_test_stats("simple.db", db_content.len());
    let config = get_test_config();

    let metadata = extract_sqlite_metadata(&db_content, &stats, &config).unwrap();

    // Total rows should be sum of all table row counts
    if let (Some(total_rows), Some(tables)) = (metadata.total_rows, &metadata.tables) {
        let calculated_total: usize = tables.iter().map(|t| t.row_count.unwrap_or(0)).sum();
        assert_eq!(total_rows, calculated_total);
    }

    // Each table should have row_count set
    if let Some(tables) = &metadata.tables {
        for table in tables {
            assert!(table.row_count.is_some());
            let _row_count = table.row_count.unwrap();
            // row_count is usize, so it's always >= 0
        }
    }
}

#[test]
fn test_database_column_counts() {
    let db_content = create_simple_db();
    let stats = get_test_stats("simple.db", db_content.len());
    let config = get_test_config();

    let metadata = extract_sqlite_metadata(&db_content, &stats, &config).unwrap();
    let tables = metadata.tables.unwrap();

    for table in &tables {
        if let (Some(column_count), Some(columns)) = (table.column_count, &table.columns) {
            assert_eq!(column_count, columns.len());
        }
    }
}

#[test]
fn test_empty_database() {
    let temp_file = NamedTempFile::new().unwrap();
    let temp_path = temp_file.path();

    // Create empty database
    let conn = Connection::open(temp_path).unwrap();
    drop(conn);

    let db_content = fs::read(temp_path).unwrap();
    let stats = get_test_stats("empty.db", db_content.len());
    let config = get_test_config();

    let metadata = extract_sqlite_metadata(&db_content, &stats, &config).unwrap();

    // Should have no tables
    assert_eq!(metadata.table_count, Some(0));
    assert!(metadata.tables.is_none() || metadata.tables.as_ref().unwrap().is_empty());
    assert_eq!(metadata.total_rows, Some(0));
}

#[test]
fn test_invalid_database_handling() {
    // Use invalid SQLite content
    let invalid_content = b"This is not a SQLite database";
    let stats = get_test_stats("invalid.db", invalid_content.len());
    let config = get_test_config();

    let metadata = extract_sqlite_metadata(invalid_content, &stats, &config).unwrap();

    // Should have file_size but error should be set
    assert!(metadata.file_size.is_some());
    assert!(metadata.error.is_some());
    assert!(!metadata.error.as_ref().unwrap().is_empty());
}

#[test]
fn test_database_error_in_output() {
    // Use corrupted/invalid content
    let invalid_content = b"SQLite format 3\x00\x00\x00\x00\x00\x00\x00\x00INVALID";
    let stats = get_test_stats("corrupted.db", invalid_content.len());
    let config = get_test_config();

    let metadata = extract_sqlite_metadata(invalid_content, &stats, &config).unwrap();

    // Error should be captured in metadata
    if metadata.error.is_some() {
        let error_msg = metadata.error.as_ref().unwrap();
        assert!(!error_msg.is_empty());
    }
}

#[test]
fn test_numeric_statistics_calculation() {
    let db_content = create_types_db();
    let stats = get_test_stats("types.db", db_content.len());
    let config = get_test_config();

    let metadata = extract_sqlite_metadata(&db_content, &stats, &config).unwrap();
    let tables = metadata.tables.unwrap();
    let columns = tables[0].columns.as_ref().unwrap();

    // Find a numeric column with data
    let numeric_col = columns.iter().find(|c| {
        c.type_name
            .as_ref()
            .map(|t| t == "INTEGER" || t == "REAL")
            .unwrap_or(false)
            && c.null_percentage != Some(100.0)
            && c.numeric_stats.is_some()
    });

    if let Some(col) = numeric_col {
        let stats = col.numeric_stats.as_ref().unwrap();

        // If we have min and max, max should be >= min
        if let (Some(min), Some(max)) = (stats.min, stats.max) {
            assert!(max >= min);
        }

        // If we have mean, it should be between min and max (if both exist)
        if let (Some(mean), Some(min), Some(max)) = (stats.mean, stats.min, stats.max) {
            assert!(mean >= min);
            assert!(mean <= max);
        }

        // Range should be max - min if both exist
        if let (Some(range), Some(min), Some(max)) = (stats.range, stats.min, stats.max) {
            assert!((range - (max - min)).abs() < 0.001); // Allow small floating point differences
        }
    }
}

#[test]
fn test_text_statistics_calculation() {
    let db_content = create_types_db();
    let stats = get_test_stats("types.db", db_content.len());
    let config = get_test_config();

    let metadata = extract_sqlite_metadata(&db_content, &stats, &config).unwrap();
    let tables = metadata.tables.unwrap();
    let columns = tables[0].columns.as_ref().unwrap();

    // Find a text column with data
    let text_col = columns.iter().find(|c| {
        c.type_name.as_ref().map(|t| t == "TEXT").unwrap_or(false)
            && c.null_percentage != Some(100.0)
            && c.text_stats.is_some()
    });

    if let Some(col) = text_col {
        let stats = col.text_stats.as_ref().unwrap();

        // If we have min and max length, max should be >= min
        if let (Some(min_len), Some(max_len)) = (stats.min_length, stats.max_length) {
            assert!(max_len >= min_len);
        }

        // Average length should be between min and max (if both exist)
        if let (Some(avg_len), Some(min_len), Some(max_len)) =
            (stats.avg_length, stats.min_length, stats.max_length)
        {
            assert!(avg_len >= min_len as f64);
            assert!(avg_len <= max_len as f64);
        }
    }
}

#[test]
fn test_blob_statistics_calculation() {
    let db_content = create_types_db();
    let stats = get_test_stats("types.db", db_content.len());
    let config = get_test_config();

    let metadata = extract_sqlite_metadata(&db_content, &stats, &config).unwrap();
    let tables = metadata.tables.unwrap();
    let columns = tables[0].columns.as_ref().unwrap();

    // Find a blob column with data
    let blob_col = columns.iter().find(|c| {
        c.type_name.as_ref().map(|t| t == "BLOB").unwrap_or(false)
            && c.null_percentage != Some(100.0)
            && c.blob_stats.is_some()
    });

    if let Some(col) = blob_col {
        let stats = col.blob_stats.as_ref().unwrap();

        // If we have min and max size, max should be >= min
        if let (Some(min_size), Some(max_size)) = (stats.min_size, stats.max_size) {
            assert!(max_size >= min_size);
        }

        // Average size should be between min and max (if both exist)
        if let (Some(avg_size), Some(min_size), Some(max_size)) =
            (stats.avg_size, stats.min_size, stats.max_size)
        {
            assert!(avg_size >= min_size as f64);
            assert!(avg_size <= max_size as f64);
        }
    }
}

#[test]
fn test_boolean_statistics_calculation() {
    let db_content = create_types_db();
    let stats = get_test_stats("types.db", db_content.len());
    let config = get_test_config();

    let metadata = extract_sqlite_metadata(&db_content, &stats, &config).unwrap();
    let tables = metadata.tables.unwrap();
    let columns = tables[0].columns.as_ref().unwrap();

    // Find a boolean column with data
    let boolean_col = columns
        .iter()
        .find(|c| c.boolean_stats.is_some() && c.null_percentage != Some(100.0));

    if let Some(col) = boolean_col {
        let stats = col.boolean_stats.as_ref().unwrap();

        // true_percentage should be between 0 and 100
        if let Some(true_pct) = stats.true_percentage {
            assert!(true_pct >= 0.0);
            assert!(true_pct <= 100.0);
        }
    }
}

#[test]
fn test_empty_table_statistics() {
    let temp_file = NamedTempFile::new().unwrap();
    let temp_path = temp_file.path();

    let conn = Connection::open(temp_path).unwrap();
    conn.execute(
        "CREATE TABLE empty_table (id INTEGER PRIMARY KEY, name TEXT)",
        [],
    )
    .unwrap();
    drop(conn);

    let db_content = fs::read(temp_path).unwrap();
    let stats = get_test_stats("empty_table.db", db_content.len());
    let config = get_test_config();

    let metadata = extract_sqlite_metadata(&db_content, &stats, &config).unwrap();

    if let Some(tables) = &metadata.tables {
        let empty_table = tables.iter().find(|t| t.name == "empty_table");
        if let Some(table) = empty_table {
            assert_eq!(table.row_count, Some(0));

            if let Some(columns) = &table.columns {
                for col in columns {
                    // Empty tables should have null_percentage = 100.0 and unique_count = 0
                    assert_eq!(col.null_percentage, Some(100.0));
                    assert_eq!(col.unique_count, Some(0));
                }
            }
        }
    }
}

#[test]
fn test_minimal_fallback_on_error() {
    // Test that we get minimal fallback metadata when extraction fails
    let invalid_content = b"Not a valid SQLite file";
    let stats = get_test_stats("invalid.db", invalid_content.len());
    let config = get_test_config();

    let metadata = extract_sqlite_metadata(invalid_content, &stats, &config).unwrap();

    // Should have at least file_size
    assert!(metadata.file_size.is_some());
    assert_eq!(metadata.file_size, Some(invalid_content.len()));

    // Error should be set
    assert!(metadata.error.is_some());
}

#[test]
fn test_index_unique_flag() {
    let db_content = create_indexed_db();
    let stats = get_test_stats("indexed.db", db_content.len());
    let config = get_test_config();

    let metadata = extract_sqlite_metadata(&db_content, &stats, &config).unwrap();
    let tables = metadata.tables.unwrap();

    for table in &tables {
        if let Some(indexes) = &table.indexes {
            for index in indexes {
                // unique flag should be set (Some(true) or Some(false))
                assert!(index.unique.is_some());
            }
        }
    }
}

#[test]
fn test_foreign_key_references() {
    let db_content = create_relational_db();
    let stats = get_test_stats("relational.db", db_content.len());
    let config = get_test_config();

    let metadata = extract_sqlite_metadata(&db_content, &stats, &config).unwrap();
    let tables = metadata.tables.unwrap();

    for table in &tables {
        if let Some(foreign_keys) = &table.foreign_keys {
            for fk in foreign_keys {
                // Foreign key should reference a valid table and column
                assert!(!fk.column.is_empty());
                assert!(!fk.references_table.is_empty());
                assert!(!fk.references_column.is_empty());

                // Referenced table should exist in the database
                let referenced_table_exists = tables.iter().any(|t| t.name == fk.references_table);
                assert!(
                    referenced_table_exists,
                    "Foreign key references non-existent table: {}",
                    fk.references_table
                );
            }
        }
    }
}
