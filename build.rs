//! Creates tests/fixtures/simple.db for integration tests (self-contained, no external files).
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let fixture_dir = manifest.join("tests").join("fixtures");
    fs::create_dir_all(&fixture_dir).expect("create tests/fixtures");
    let db_path = fixture_dir.join("simple.db");

    if db_path.exists() {
        fs::remove_file(&db_path).expect("remove existing fixture");
    }

    let conn = rusqlite::Connection::open(&db_path).expect("open db");
    conn.execute(
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT)",
        [],
    )
    .expect("create table");
    conn.execute(
        "INSERT INTO users (name, email) VALUES ('alice', 'alice@example.com')",
        [],
    )
    .expect("insert");
}
