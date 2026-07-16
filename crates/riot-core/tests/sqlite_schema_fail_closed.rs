//! Fail-closed coverage for the schema validator (`store/schema.rs`).
//!
//! Every test hand-builds a physically plausible database that violates one
//! structural invariant and asserts the managed open path rejects it (or, for
//! the supported-but-older case, reports the exact migration requirement)
//! rather than trusting it. Databases are built through a raw `rusqlite`
//! connection so the forgeries can bypass the migration engine.

use riot_core::store::{DatabaseConfig, DatabaseError, RiotDatabase, CURRENT_SCHEMA_VERSION};
use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(1);

struct TestDir(PathBuf);

impl TestDir {
    fn new(label: &str) -> Self {
        let sequence = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "riot-schema-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create test directory");
        Self(path)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// Canonical v1 table definitions (byte-for-byte equivalent, after whitespace
// normalization, to the constants the validator compares against).
const V1_SCHEMA_MIGRATIONS: &str = "CREATE TABLE schema_migrations (
    version INTEGER PRIMARY KEY NOT NULL CHECK (version > 0)
) STRICT";
const V1_DATABASE_META: &str = "CREATE TABLE database_meta (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    database_id BLOB NOT NULL CHECK (length(database_id) = 16),
    database_generation BLOB NOT NULL CHECK (length(database_generation) = 16),
    generation INTEGER NOT NULL CHECK (generation >= 0),
    authority_quarantined INTEGER NOT NULL CHECK (authority_quarantined IN (0, 1))
) STRICT";
const V1_LOCAL_STATE: &str = "CREATE TABLE local_state (
    key TEXT PRIMARY KEY NOT NULL CHECK (length(key) BETWEEN 1 AND 128),
    value BLOB NOT NULL CHECK (length(value) <= 1048576)
) STRICT, WITHOUT ROWID";

/// Builds a structurally valid *version 1* database in WAL mode. The managed
/// writable open path would migrate it to v2, so these fixtures are opened
/// read-only to preserve the older version and exercise `validate_supported`.
fn build_valid_v1_wal_database(path: &Path) {
    let connection = Connection::open(path).expect("v1 database");
    connection
        .pragma_update(None, "journal_mode", "WAL")
        .expect("wal mode");
    connection
        .execute_batch(&format!(
            "{V1_SCHEMA_MIGRATIONS};
             {V1_DATABASE_META};
             {V1_LOCAL_STATE};
             INSERT INTO schema_migrations(version) VALUES (1);
             INSERT INTO database_meta(
                 singleton, database_id, database_generation, generation, authority_quarantined
             ) VALUES (1, randomblob(16), randomblob(16), 0, 0);
             PRAGMA user_version = 1;"
        ))
        .expect("write v1 schema");
    connection
        .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_row| Ok(()))
        .expect("fold WAL");
    drop(connection);
}

/// Builds a fresh, fully valid current-version database and returns its path.
fn build_valid_current_database(path: &Path) {
    let database = RiotDatabase::open(path, DatabaseConfig::default()).expect("valid database");
    drop(database);
}

#[test]
fn read_only_open_of_a_supported_older_version_reports_the_migration_requirement() {
    let directory = TestDir::new("older-version");
    let path = directory.path("v1.sqlite");
    build_valid_v1_wal_database(&path);

    let error = RiotDatabase::open_read_only(&path, DatabaseConfig::default())
        .expect_err("a v1 database is not the current supported version");
    assert!(matches!(
        error,
        DatabaseError::MigrationRequired { found, supported }
            if found == 1 && supported == CURRENT_SCHEMA_VERSION
    ));
}

#[test]
fn a_nonempty_file_with_no_schema_objects_is_treated_as_fresh() {
    let directory = TestDir::new("nonempty-no-objects");
    let path = directory.path("empty.sqlite");
    // Create a nonempty database file that carries no schema objects: create a
    // table, then drop it, leaving allocated pages but an empty schema.
    let connection = Connection::open(&path).expect("bootstrap");
    connection
        .execute_batch("CREATE TABLE scratch(x); DROP TABLE scratch;")
        .expect("leave a nonempty but objectless file");
    drop(connection);
    assert!(fs::metadata(&path).expect("metadata").len() > 0);

    // The managed open path must treat it as fresh and migrate it cleanly.
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("fresh migrate");
    assert_eq!(
        database.schema_version().expect("schema version"),
        CURRENT_SCHEMA_VERSION
    );
}

#[test]
fn an_empty_ledger_with_a_nonzero_user_version_fails_closed() {
    let directory = TestDir::new("empty-ledger-bad-version");
    let path = directory.path("bad.sqlite");
    let connection = Connection::open(&path).expect("bootstrap");
    // A migration ledger table exists but is empty, while the user_version
    // marker claims a nonzero version. This is an inconsistent, untrusted state.
    connection
        .execute_batch(&format!(
            "{V1_SCHEMA_MIGRATIONS};
             PRAGMA user_version = 5;"
        ))
        .expect("write inconsistent markers");
    drop(connection);
    let before = fs::read(&path).expect("bytes before open");

    let error = RiotDatabase::open(&path, DatabaseConfig::default())
        .expect_err("empty ledger with nonzero user_version must fail closed");
    assert!(matches!(error, DatabaseError::CorruptDatabase));
    assert_eq!(fs::read(&path).expect("bytes after open"), before);
}

#[test]
fn a_user_version_that_disagrees_with_the_ledger_fails_closed() {
    let directory = TestDir::new("user-version-mismatch");
    let path = directory.path("valid.sqlite");
    build_valid_current_database(&path);

    // A contiguous [1, 2] ledger but a user_version that does not match the
    // recorded maximum: a torn or tampered upgrade marker.
    let connection = Connection::open(&path).expect("tamper");
    connection
        .pragma_update(None, "user_version", 1)
        .expect("mismatch user_version");
    drop(connection);
    let before = fs::read(&path).expect("bytes before open");

    let error = RiotDatabase::open(&path, DatabaseConfig::default())
        .expect_err("user_version/ledger mismatch must fail closed");
    assert!(matches!(error, DatabaseError::CorruptDatabase));
    assert_eq!(fs::read(&path).expect("bytes after open"), before);
}

#[test]
fn a_missing_database_meta_row_fails_closed() {
    let directory = TestDir::new("missing-meta-row");
    let path = directory.path("valid.sqlite");
    build_valid_current_database(&path);

    let connection = Connection::open(&path).expect("tamper");
    connection
        .execute_batch("PRAGMA foreign_keys = OFF; DELETE FROM database_meta;")
        .expect("remove the singleton meta row");
    drop(connection);

    let error = RiotDatabase::open(&path, DatabaseConfig::default())
        .expect_err("a missing database_meta row must fail closed");
    assert!(matches!(error, DatabaseError::CorruptDatabase));
}

#[test]
fn a_missing_evidence_meta_row_fails_closed() {
    let directory = TestDir::new("missing-evidence-meta");
    let path = directory.path("valid.sqlite");
    build_valid_current_database(&path);

    let connection = Connection::open(&path).expect("tamper");
    connection
        .execute_batch("PRAGMA foreign_keys = OFF; DELETE FROM evidence_meta;")
        .expect("remove the evidence meta row");
    drop(connection);

    let error = RiotDatabase::open(&path, DatabaseConfig::default())
        .expect_err("a missing evidence_meta row must fail closed");
    assert!(matches!(error, DatabaseError::CorruptDatabase));
}

#[test]
fn a_tampered_index_definition_fails_closed() {
    let directory = TestDir::new("tampered-index");
    let path = directory.path("valid.sqlite");
    build_valid_current_database(&path);

    // Replace a validated index with a differently-shaped one under the same
    // name: the structure fingerprint no longer matches.
    let connection = Connection::open(&path).expect("tamper");
    connection
        .execute_batch(
            "DROP INDEX entry_path_prefix_lookup;
             CREATE INDEX entry_path_prefix_lookup ON entry_path_prefixes(namespace_id);",
        )
        .expect("reshape index");
    drop(connection);

    let error = RiotDatabase::open(&path, DatabaseConfig::default())
        .expect_err("a reshaped index must fail closed");
    assert!(matches!(error, DatabaseError::CorruptDatabase));
}
