//! SQLite foundation: proves the pinned, *bundled* SQLite is what riot-core
//! links, on every target the conference package ships.
//!
//! This is the de-risking test for the multi-space store. The store design
//! (`docs/superpowers/specs/2026-07-12-multi-space-sqlite-store-design.md`)
//! depends on SQLite's JSON functions for document projections and on the
//! `backup`/`blob`/`hooks`/`limits` surfaces for recovery, payload access,
//! change feeds, and resource ceilings. None of that is safe to build on until
//! we know the library is ours.
//!
//! "Ours" is the load-bearing word. Apple ships a *system* libsqlite3 that
//! changes between OS releases and between iOS versions on users' phones, and
//! its build differs from upstream (an SEE codec, `THREADSAFE=2`). Depending on
//! it would make our storage semantics a function of whose phone we are on.
//! These tests pin the identity of the library itself, not just its version.

use rusqlite::Connection;

/// The exact SQLite that `rusqlite =0.40.1` + `bundled` compiles in.
///
/// Bump these together with the `rusqlite` pin, deliberately — a surprise
/// change here means the storage engine moved under us.
const EXPECTED_VERSION: &str = "3.53.2";
const EXPECTED_VERSION_NUMBER: i32 = 3_053_002;
/// Upstream sqlite.org's source ID for the amalgamation vendored by
/// `libsqlite3-sys` under `bundled`. This is the sharpest available proof that
/// we linked the vendored source and not the platform library — on the macOS
/// dev host the system SQLite is a *different* build (3.51.0, source ID ending
/// `aapl`), so the mismatch also demonstrates we are not silently falling
/// through to it.
const EXPECTED_SOURCE_ID: &str =
    "2026-06-03 19:12:13 d6e03d8c777cfa2d35e3b60d8ec3e0187f3e9f99d8e2ee9cac695fd6fcdf1a24";

#[test]
fn bundled_sqlite_opens_and_round_trips_a_row() {
    let db = Connection::open_in_memory().expect("open in-memory database");
    db.execute_batch(
        "CREATE TABLE probe (id INTEGER PRIMARY KEY, namespace BLOB NOT NULL, label TEXT NOT NULL);",
    )
    .expect("create table");

    let namespace = vec![0xABu8; 32];
    db.execute(
        "INSERT INTO probe (id, namespace, label) VALUES (?1, ?2, ?3)",
        rusqlite::params![1i64, namespace, "first space"],
    )
    .expect("insert row");

    let (got_namespace, got_label): (Vec<u8>, String) = db
        .query_row("SELECT namespace, label FROM probe WHERE id = ?1", [1i64], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .expect("read the row back");

    assert_eq!(got_namespace, namespace, "fixed-width blob keys must round-trip byte-exactly");
    assert_eq!(got_label, "first space");
}

/// The document projection layer is defined in terms of SQLite's JSON
/// functions. If SQLite were compiled with `SQLITE_OMIT_JSON`, every document
/// query in the design would fail at runtime rather than at build time.
#[test]
fn bundled_sqlite_has_json_support_compiled_in() {
    let db = Connection::open_in_memory().expect("open in-memory database");

    let extracted: i64 = db
        .query_row(r#"SELECT json_extract('{"a":1}', '$.a')"#, [], |row| row.get(0))
        .expect("json_extract must be compiled in");
    assert_eq!(extracted, 1);

    // The projection stores documents as JSON text; validity checking and
    // structural queries are both on the critical path.
    let valid: i64 = db
        .query_row(r#"SELECT json_valid('{"ok":true}')"#, [], |row| row.get(0))
        .expect("json_valid must be compiled in");
    assert_eq!(valid, 1);

    let nested: String = db
        .query_row(
            r#"SELECT json_extract('{"doc":{"title":"Roll Call"}}', '$.doc.title')"#,
            [],
            |row| row.get(0),
        )
        .expect("nested json_extract");
    assert_eq!(nested, "Roll Call");
}

/// The whole point of `bundled`: one SQLite, identical on an iPhone, an
/// Android handset, and a developer's Mac.
#[test]
fn sqlite_is_the_pinned_bundled_library_not_the_platform_one() {
    assert_eq!(rusqlite::version(), EXPECTED_VERSION, "bundled SQLite version drifted");
    assert_eq!(rusqlite::version_number(), EXPECTED_VERSION_NUMBER);

    let db = Connection::open_in_memory().expect("open in-memory database");
    let source_id: String = db
        .query_row("SELECT sqlite_source_id()", [], |row| row.get(0))
        .expect("read source id");
    assert_eq!(
        source_id, EXPECTED_SOURCE_ID,
        "source ID is not the vendored amalgamation — we linked a platform SQLite"
    );

    let mut stmt = db.prepare("PRAGMA compile_options").expect("prepare pragma");
    let options: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query compile options")
        .collect::<Result<_, _>>()
        .expect("collect compile options");

    // Markers that exist only in Apple's system build. Their presence would mean
    // we are running on the platform library, whose behavior varies per OS.
    for apple_marker in ["CODEC=see-cccrypt", "THREADSAFE=2"] {
        assert!(
            !options.iter().any(|o| o == apple_marker),
            "compile option `{apple_marker}` means this is Apple's system SQLite, not the bundled one"
        );
    }

    // The bundled build is serialized-threadsafe; the system build reports
    // THREADSAFE=2 instead.
    assert!(
        options.iter().any(|o| o == "THREADSAFE=1"),
        "bundled SQLite must be serialized-threadsafe; got: {options:?}"
    );
}

/// The design pins an exact feature set. A silently dropped feature would only
/// surface many tasks later, in the code that needs it; this fails now instead.
#[test]
fn the_pinned_rusqlite_feature_surface_is_reachable() {
    let db = Connection::open_in_memory().expect("open in-memory database");

    // `limits`: resource ceilings stay enforceable on disk.
    let sql_length_limit = db
        .limit(rusqlite::limits::Limit::SQLITE_LIMIT_SQL_LENGTH)
        .expect("query SQL length limit (rusqlite `limits` feature)");
    assert!(sql_length_limit > 0);

    // `hooks`: the change feed publishes only after commit.
    let committed = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let seen = std::sync::Arc::clone(&committed);
    // `update_hook` returns the previously installed hook; there is none.
    let _previous = db.update_hook(Some(move |_action, _db: &str, _tbl: &str, _row: i64| {
        seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }));

    // `blob`: retained payloads are read incrementally, not slurped.
    db.execute_batch("CREATE TABLE payloads (id INTEGER PRIMARY KEY, bytes BLOB NOT NULL);")
        .expect("create payload table");
    db.execute("INSERT INTO payloads (id, bytes) VALUES (1, ZEROBLOB(64))", [])
        .expect("insert zeroblob");
    let blob = db
        .blob_open("main", "payloads", "bytes", 1, false)
        .expect("blob_open must exist (rusqlite `blob` feature)");
    assert_eq!(blob.len(), 64);
    drop(blob);

    assert!(
        committed.load(std::sync::atomic::Ordering::SeqCst) > 0,
        "update_hook must fire (rusqlite `hooks` feature)"
    );

    // `backup`: recovery copies through SQLite's backup API, never by copying
    // a live file.
    let mut destination = Connection::open_in_memory().expect("open backup destination");
    {
        let backup = rusqlite::backup::Backup::new(&db, &mut destination)
            .expect("Backup::new must exist (rusqlite `backup` feature)");
        backup
            .run_to_completion(5, std::time::Duration::from_millis(0), None)
            .expect("run backup to completion");
    }
    let copied: i64 = destination
        .query_row("SELECT COUNT(*) FROM payloads", [], |row| row.get(0))
        .expect("backed-up database must contain the source rows");
    assert_eq!(copied, 1);
}
