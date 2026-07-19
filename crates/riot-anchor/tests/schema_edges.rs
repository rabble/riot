//! Edge coverage for `schema.rs`: the forward-only fail-closed refusal to open a
//! database stamped by a newer binary, and the `Display` / `From` surface of the
//! `SchemaError` variants. `SchemaError` is `#[non_exhaustive]`, so each value is
//! obtained through the public API rather than constructed directly.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use riot_anchor::schema::{migrate, SchemaError, CURRENT_SCHEMA_VERSION};
use rusqlite::Connection;

/// A temporary on-disk database that cleans up its `-wal`/`-shm` siblings.
struct TempDb {
    path: PathBuf,
}

impl TempDb {
    fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut path = std::env::temp_dir();
        path.push(format!(
            "riot-anchor-schema-edge-{}-{}.db",
            std::process::id(),
            id
        ));
        let _ = std::fs::remove_file(&path);
        Self { path }
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(self.path.with_extension("db-wal"));
        let _ = std::fs::remove_file(self.path.with_extension("db-shm"));
    }
}

// ---------------------------------------------------------------------------
// forward-only: refuse to open a database from a newer binary, on a real reopen
// ---------------------------------------------------------------------------

#[test]
fn reopening_a_future_stamped_file_database_fails_closed() {
    let temp = TempDb::new();

    // Migrate a fresh file-backed database to the current version, then stamp it
    // as if a NEWER binary had written it and close the connection.
    let future = CURRENT_SCHEMA_VERSION + 1;
    {
        let mut connection = Connection::open(temp.path()).expect("open");
        migrate(&mut connection).expect("migrate to current");
        connection
            .execute(
                "INSERT INTO schema_migrations(version) VALUES (?1)",
                [future],
            )
            .expect("stamp future migration row");
        connection
            .pragma_update(None, "user_version", future)
            .expect("bump user_version");
    }

    // Reopening (a genuine new connection to the persisted file) must refuse to
    // migrate backward: it fails closed with the exact versions and never
    // downgrades the stored marker.
    let mut reopened = Connection::open(temp.path()).expect("reopen");
    let error = migrate(&mut reopened).expect_err("must fail closed on reopen");
    match error {
        SchemaError::VersionTooNew { found, supported } => {
            assert_eq!(found, future);
            assert_eq!(supported, CURRENT_SCHEMA_VERSION);
        }
        other => panic!("expected VersionTooNew, got {other:?}"),
    }

    // State is untouched: the stored marker is still the future version, not
    // silently rewritten to the current one.
    let stored: u32 = reopened
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .expect("stored version");
    assert_eq!(
        stored, future,
        "fail-closed must not mutate the version marker"
    );
}

// ---------------------------------------------------------------------------
// SchemaError Display + From surface
// ---------------------------------------------------------------------------

#[test]
fn display_version_too_new_error() {
    // Obtain a `VersionTooNew` from the public migrate path, then check Display.
    let mut connection = Connection::open_in_memory().expect("open");
    migrate(&mut connection).expect("migrate to current");
    let future = CURRENT_SCHEMA_VERSION + 3;
    connection
        .execute(
            "INSERT INTO schema_migrations(version) VALUES (?1)",
            [future],
        )
        .expect("stamp future row");

    let error = migrate(&mut connection).expect_err("must fail closed");
    let rendered = error.to_string();
    assert!(
        rendered.contains(&format!("version {future}"))
            && rendered.contains(&format!("newer than supported {CURRENT_SCHEMA_VERSION}")),
        "unexpected Display: {rendered}"
    );
}

#[test]
fn display_sqlite_error_via_from_conversion() {
    // Obtain a genuine rusqlite error and convert it — exercising
    // `From<rusqlite::Error>` and the `Sqlite` Display arm.
    let connection = Connection::open_in_memory().expect("open");
    let sqlite_err = connection
        .execute("SELECT this is not valid sql", [])
        .expect_err("malformed sql must error");

    let error: SchemaError = sqlite_err.into();
    assert!(matches!(error, SchemaError::Sqlite(_)));
    assert!(
        error.to_string().starts_with("anchor schema sqlite error:"),
        "unexpected Display: {error}"
    );
}
