//! Platform proof for Riot's pinned, bundled SQLite build.

use rusqlite::Connection;

const EXPECTED_SQLITE_VERSION: &str = "3.53.2";
const EXPECTED_SQLITE_SOURCE_ID: &str =
    "2026-06-03 19:12:13 d6e03d8c777cfa2d35e3b60d8ec3e0187f3e9f99d8e2ee9cac695fd6fcdf1a24";

#[test]
fn bundled_sqlite_is_the_pinned_engine() {
    assert_eq!(rusqlite::version(), EXPECTED_SQLITE_VERSION);

    let database = Connection::open_in_memory().expect("open bundled SQLite");
    let source_id: String = database
        .query_row("SELECT sqlite_source_id()", [], |row| row.get(0))
        .expect("read bundled SQLite source ID");

    assert_eq!(source_id, EXPECTED_SQLITE_SOURCE_ID);
}

#[test]
fn bundled_sqlite_supports_json_document_queries() {
    let database = Connection::open_in_memory().expect("open bundled SQLite");
    let (valid, title): (bool, String) = database
        .query_row(
            r#"SELECT json_valid(?1), json_extract(?1, '$.document.title')"#,
            [r#"{"document":{"title":"Roll call"}}"#],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("query JSON document");

    assert!(valid);
    assert_eq!(title, "Roll call");
}

#[test]
fn required_rusqlite_surfaces_are_enabled() {
    let database = Connection::open_in_memory().expect("open source database");
    database
        .execute_batch(
            "CREATE TABLE payloads (id INTEGER PRIMARY KEY, bytes BLOB NOT NULL);\
             INSERT INTO payloads (bytes) VALUES (ZEROBLOB(16));",
        )
        .expect("seed source database");

    let sql_limit = database
        .limit(rusqlite::limits::Limit::SQLITE_LIMIT_SQL_LENGTH)
        .expect("read SQL length limit");
    assert!(sql_limit > 0);

    let blob = database
        .blob_open("main", "payloads", "bytes", 1, true)
        .expect("open incremental blob");
    assert_eq!(blob.len(), 16);
    drop(blob);

    let updates = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed_updates = std::sync::Arc::clone(&updates);
    database
        .update_hook(Some(
            move |_action, _database: &str, _table: &str, _row_id| {
                observed_updates.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            },
        ))
        .expect("install update hook");
    database
        .execute("INSERT INTO payloads (bytes) VALUES (ZEROBLOB(1))", [])
        .expect("insert observed row");
    assert_eq!(updates.load(std::sync::atomic::Ordering::SeqCst), 1);

    let mut destination = Connection::open_in_memory().expect("open backup destination");
    let backup =
        rusqlite::backup::Backup::new(&database, &mut destination).expect("start SQLite backup");
    backup
        .run_to_completion(5, std::time::Duration::ZERO, None)
        .expect("finish SQLite backup");
}
