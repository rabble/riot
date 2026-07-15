//! Two refusal surfaces that only a damaged or interrupted installation can
//! reach: schema validation (a database whose structure does not match the one
//! this build compiled against) and install-journal recovery (a restore that
//! was cut in half by a crash).
//!
//! Both are tested against real files on disk — forged schemas built with a raw
//! SQLite connection, and install journals planted exactly as a crash would
//! leave them — because both exist precisely to survive states that no healthy
//! code path can produce.

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
            "riot-store-recovery-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create test directory");
        Self(path)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn database(&self) -> PathBuf {
        self.path("riot.sqlite")
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The sidecar paths the installer uses, named exactly as `backup::install_path`
/// names them. A crash leaves these behind; recovery is what reads them.
fn install_path(destination: &Path, suffix: &str) -> PathBuf {
    let parent = destination.parent().expect("parent");
    let name = destination
        .file_name()
        .expect("file name")
        .to_string_lossy();
    parent.join(format!(".{name}.install-{suffix}"))
}

/// Builds a healthy current-schema database at `path` holding one known value,
/// then closes it so the file can be inspected or moved.
fn seed_database(path: &Path, value: &[u8]) {
    let database = RiotDatabase::open(path, DatabaseConfig::default()).expect("open to seed");
    database.set_local_state("document", value).expect("seed");
    // Fold the WAL back in so the single main file is self-contained and can be
    // renamed around by the recovery tests.
    database
        .checkpoint(riot_core::store::CheckpointMode::Truncate)
        .expect("truncate the WAL");
}

/// Opens the file with a raw connection and applies `sql`, defeating every
/// guard the real API has — which is the only way to manufacture the damage
/// these guards exist to catch.
fn forge(path: &Path, sql: &str) {
    let connection = Connection::open(path).expect("raw connection");
    connection.execute_batch(sql).expect("apply forgery");
}

// ─── Schema validation ───────────────────────────────────────────────────────

/// A database written by an older build is not silently upgraded behind a
/// read-only handle: it is refused, naming the version found and the version
/// this build supports, so the caller can decide to migrate.
#[test]
fn an_older_schema_is_refused_read_only_naming_both_versions() {
    let directory = TestDir::new("older-schema");
    let path = directory.database();

    // A structurally complete *version 1* database: the three v1 tables in their
    // canonical shape, a valid singleton meta row, the ledger and user_version
    // both at 1, and WAL journaling (which the read-only path requires).
    let connection = Connection::open(&path).expect("raw connection");
    connection
        .pragma_update(None, "journal_mode", "WAL")
        .expect("wal");
    connection
        .execute_batch(
            "CREATE TABLE schema_migrations (
                 version INTEGER PRIMARY KEY NOT NULL CHECK (version > 0)
             ) STRICT;
             CREATE TABLE database_meta (
                 singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                 database_id BLOB NOT NULL CHECK (length(database_id) = 16),
                 database_generation BLOB NOT NULL CHECK (length(database_generation) = 16),
                 generation INTEGER NOT NULL CHECK (generation >= 0),
                 authority_quarantined INTEGER NOT NULL CHECK (authority_quarantined IN (0, 1))
             ) STRICT;
             CREATE TABLE local_state (
                 key TEXT PRIMARY KEY NOT NULL CHECK (length(key) BETWEEN 1 AND 128),
                 value BLOB NOT NULL CHECK (length(value) <= 1048576)
             ) STRICT, WITHOUT ROWID;
             INSERT INTO database_meta(
                 singleton, database_id, database_generation, generation, authority_quarantined
             ) VALUES (1, randomblob(16), randomblob(16), 0, 0);
             INSERT INTO schema_migrations(version) VALUES (1);
             PRAGMA user_version = 1;",
        )
        .expect("build a version 1 database");
    drop(connection);

    let error = RiotDatabase::open_read_only(&path, DatabaseConfig::default())
        .expect_err("an old schema cannot be served read-only");
    assert_eq!(
        error,
        DatabaseError::MigrationRequired {
            found: 1,
            supported: CURRENT_SCHEMA_VERSION,
        }
    );

    // Opened writably, the very same file migrates forward instead of failing —
    // the refusal above is about not mutating a database a read-only caller
    // asked only to read.
    let database = RiotDatabase::open(&path, DatabaseConfig::default())
        .expect("a writable open migrates it forward");
    assert_eq!(
        database.schema_version().expect("schema version"),
        CURRENT_SCHEMA_VERSION
    );
}

/// A file with bytes but no schema objects at all (a table created and dropped)
/// is not corrupt — it is empty, and it migrates cleanly.
#[test]
fn a_file_with_no_schema_objects_is_treated_as_empty_and_migrates() {
    let directory = TestDir::new("no-objects");
    let path = directory.database();
    forge(
        &path,
        "CREATE TABLE scratch (x INTEGER); DROP TABLE scratch;",
    );
    assert!(
        fs::metadata(&path).expect("metadata").len() > 0,
        "the file must carry bytes, or it is not the case under test"
    );

    let database =
        RiotDatabase::open(&path, DatabaseConfig::default()).expect("an object-free file migrates");
    assert_eq!(
        database.schema_version().expect("schema version"),
        CURRENT_SCHEMA_VERSION
    );
    database.set_local_state("key", b"value").expect("usable");
    assert_eq!(
        database.local_state("key").expect("read back"),
        Some(b"value".to_vec())
    );
}

/// Every way a schema can lie about itself fails closed with `CorruptDatabase`,
/// and — this is the load-bearing half — leaves the rejected file byte-for-byte
/// unchanged. A validator that repaired what it rejected would be indisting-
/// uishable from one that accepted it.
#[test]
fn every_schema_forgery_fails_closed_without_touching_the_file() {
    // Each case: a label, and the forgery applied to an otherwise-healthy
    // current-schema database.
    let cases: [(&str, &str); 4] = [
        (
            // The ledger says version 2 but the file's own version marker says 1.
            "user_version disagrees with the ledger",
            "PRAGMA user_version = 1;",
        ),
        (
            // The singleton identity row is gone.
            "database_meta has no singleton row",
            "DELETE FROM database_meta;",
        ),
        (
            // The evidence generation counter is gone.
            "evidence_meta has no singleton row",
            "DELETE FROM evidence_meta;",
        ),
        (
            // The prefix-lookup index covers the wrong columns, so prefix
            // queries would silently return the wrong rows.
            "entry_path_prefix_lookup indexes the wrong columns",
            "DROP INDEX entry_path_prefix_lookup;
             CREATE INDEX entry_path_prefix_lookup
                 ON entry_path_prefixes(namespace_id, depth);",
        ),
    ];

    for (label, forgery) in cases {
        let directory = TestDir::new("forgery");
        let path = directory.database();
        seed_database(&path, b"original");
        forge(&path, forgery);
        let before = fs::read(&path).expect("bytes before the rejected open");

        let error = RiotDatabase::open(&path, DatabaseConfig::default())
            .expect_err(&format!("{label}: forgery was accepted"));
        assert_eq!(error, DatabaseError::CorruptDatabase, "{label}");
        assert_eq!(
            fs::read(&path).expect("bytes after the rejected open"),
            before,
            "{label}: the rejected database was mutated"
        );
    }
}

/// An empty migration ledger paired with a non-zero version marker is a lie
/// about a database that was never migrated, and is refused before any pragma
/// is allowed to touch the file.
#[test]
fn an_empty_ledger_with_a_version_marker_is_refused() {
    let directory = TestDir::new("empty-ledger");
    let path = directory.database();
    forge(
        &path,
        "CREATE TABLE schema_migrations (
             version INTEGER PRIMARY KEY NOT NULL CHECK (version > 0)
         ) STRICT;
         PRAGMA user_version = 1;",
    );
    let before = fs::read(&path).expect("bytes before");

    let error = RiotDatabase::open(&path, DatabaseConfig::default())
        .expect_err("an unmigrated database claiming a version is refused");
    assert_eq!(error, DatabaseError::CorruptDatabase);
    assert_eq!(fs::read(&path).expect("bytes after"), before);
}

// ─── Install-journal recovery ────────────────────────────────────────────────

/// A crash after the replacement was installed but before the journal was
/// cleaned up: the destination is gone, and the replacement is sitting in the
/// `new` slot. Opening the database finishes the interrupted install rather
/// than starting from nothing.
#[test]
fn an_interrupted_install_completes_from_the_new_slot() {
    let directory = TestDir::new("install-new");
    let path = directory.database();

    // Build the replacement where the installer would have left it, and leave
    // no destination — exactly the window between the rename and the cleanup.
    let replacement = install_path(&path, "new");
    seed_database(&replacement, b"the-replacement");
    fs::write(install_path(&path, "journal"), "installed\n").expect("plant the journal");
    assert!(!path.exists(), "the destination must be absent");

    let database = RiotDatabase::open(&path, DatabaseConfig::default())
        .expect("the interrupted install is completed");
    assert_eq!(
        database.local_state("document").expect("read back"),
        Some(b"the-replacement".to_vec()),
        "the replacement's contents must be what we ended up with"
    );

    // The install finished: every sidecar is cleaned up.
    assert!(!install_path(&path, "journal").exists());
    assert!(!install_path(&path, "new").exists());
    assert!(!install_path(&path, "old").exists());
}

/// The same crash window, but the rename never happened: the destination is
/// gone and only the *old* copy survives. Recovery must put the old database
/// back rather than leaving the caller with no database at all.
#[test]
fn an_interrupted_install_falls_back_to_the_old_slot() {
    let directory = TestDir::new("install-old");
    let path = directory.database();

    let old = install_path(&path, "old");
    seed_database(&old, b"the-original");
    fs::write(install_path(&path, "journal"), "installed\n").expect("plant the journal");
    assert!(!path.exists(), "the destination must be absent");

    let database =
        RiotDatabase::open(&path, DatabaseConfig::default()).expect("the old database is restored");
    assert_eq!(
        database.local_state("document").expect("read back"),
        Some(b"the-original".to_vec())
    );
    assert!(!install_path(&path, "journal").exists());
    assert!(!install_path(&path, "old").exists());
}

/// A journal with no database anywhere — neither destination, nor new, nor old
/// — is unrecoverable. It is reported as an I/O failure, not papered over by
/// silently creating a fresh empty database in its place.
#[test]
fn an_install_journal_with_no_database_anywhere_is_an_io_failure() {
    let directory = TestDir::new("install-nothing");
    let path = directory.database();
    fs::write(install_path(&path, "journal"), "installed\n").expect("plant the journal");

    let error = RiotDatabase::open(&path, DatabaseConfig::default())
        .expect_err("a journal with nothing to recover cannot be papered over");
    assert_eq!(error, DatabaseError::StorageIo);
    assert!(
        !path.exists(),
        "no empty database may be conjured in place of the lost one"
    );
}

/// A crash *before* the install (the `prepared` marker) rolls the old database
/// back into place and discards the half-written replacement.
#[test]
fn a_prepared_install_rolls_back_to_the_old_database() {
    let directory = TestDir::new("install-prepared");
    let path = directory.database();

    let old = install_path(&path, "old");
    seed_database(&old, b"the-original");
    // A half-written replacement that must be discarded, not installed.
    let replacement = install_path(&path, "new");
    seed_database(&replacement, b"the-half-written-replacement");
    fs::write(install_path(&path, "journal"), "prepared\n").expect("plant the journal");

    let database = RiotDatabase::open(&path, DatabaseConfig::default())
        .expect("a prepared install rolls back");
    assert_eq!(
        database.local_state("document").expect("read back"),
        Some(b"the-original".to_vec()),
        "a prepared (not installed) replacement must never win"
    );
    assert!(!install_path(&path, "new").exists());
    assert!(!install_path(&path, "old").exists());
    assert!(!install_path(&path, "journal").exists());
}

/// No journal at all, but an `old` copy left behind and no destination: the old
/// database is put back. This is the crash window before the journal became
/// durable, and it must not lose the only database that exists.
#[test]
fn an_orphaned_old_copy_is_restored_when_no_journal_survives() {
    let directory = TestDir::new("install-orphan");
    let path = directory.database();

    let old = install_path(&path, "old");
    seed_database(&old, b"the-original");
    assert!(!install_path(&path, "journal").exists());
    assert!(!path.exists());

    let database = RiotDatabase::open(&path, DatabaseConfig::default())
        .expect("the orphaned old database is restored");
    assert_eq!(
        database.local_state("document").expect("read back"),
        Some(b"the-original".to_vec())
    );
    assert!(!install_path(&path, "old").exists());
}

/// A journal holding a phase this build does not know how to interpret is a
/// database in an unknown state. It fails closed rather than guessing which
/// half of an install it was in.
#[test]
fn an_unknown_install_phase_fails_closed() {
    let directory = TestDir::new("install-unknown");
    let path = directory.database();
    seed_database(&path, b"the-original");
    fs::write(install_path(&path, "journal"), "reticulating-splines\n")
        .expect("plant an unknown phase");
    let before = fs::read(&path).expect("bytes before");

    let error = RiotDatabase::open(&path, DatabaseConfig::default())
        .expect_err("an unknown install phase is not guessed at");
    assert_eq!(error, DatabaseError::CorruptDatabase);
    assert_eq!(fs::read(&path).expect("bytes after"), before);
}

/// An install journal that cannot be read at all (here: it is a directory, not
/// a file) is an I/O failure, distinct from "there is no journal".
#[test]
fn an_unreadable_install_journal_is_an_io_failure() {
    let directory = TestDir::new("install-unreadable");
    let path = directory.database();
    seed_database(&path, b"the-original");
    fs::create_dir(install_path(&path, "journal")).expect("plant an unreadable journal");

    let error = RiotDatabase::open(&path, DatabaseConfig::default())
        .expect_err("an unreadable journal is not treated as absent");
    assert_eq!(error, DatabaseError::StorageIo);
}

/// A leftover install slot that cannot be cleared (here: it is a directory) is
/// reported rather than ignored — an install that cannot clean up its own
/// sidecars must not proceed to open the database over them.
#[test]
fn an_unclearable_install_slot_is_an_io_failure() {
    let directory = TestDir::new("install-unclearable");
    let path = directory.database();
    seed_database(&path, b"the-original");
    fs::create_dir(install_path(&path, "new")).expect("plant an unclearable new slot");

    let error = RiotDatabase::open(&path, DatabaseConfig::default())
        .expect_err("an unclearable sidecar is not ignored");
    assert_eq!(error, DatabaseError::StorageIo);
}

// ─── Restore refusals ────────────────────────────────────────────────────────

/// Restoring from a source that is not a file is refused before the destination
/// is touched.
#[test]
fn a_restore_from_a_missing_source_is_refused() {
    let directory = TestDir::new("restore-missing");
    let path = directory.database();
    let backup = directory.path("riot.backup.sqlite");

    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("open");
    database.set_local_state("document", b"live").expect("seed");
    let manifest = database.backup_to(&backup).expect("backup");
    drop(database);

    let before = fs::read(&path).expect("bytes before");
    let error = RiotDatabase::restore_from(
        &path,
        directory.path("no-such-backup.sqlite"),
        &manifest,
        DatabaseConfig::default(),
    )
    .expect_err("a missing source cannot be restored from");
    assert_eq!(error, DatabaseError::StorageIo);
    assert_eq!(
        fs::read(&path).expect("bytes after"),
        before,
        "the destination was touched by a refused restore"
    );
}

/// A restore that finds an install already mid-flight refuses as retryable
/// rather than racing it — two installers writing the same destination is the
/// one way the copy-on-write swap could lose data.
#[test]
fn a_restore_over_an_unfinished_install_is_retryable() {
    let directory = TestDir::new("restore-busy");
    let path = directory.database();
    let backup = directory.path("riot.backup.sqlite");

    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("open");
    database.set_local_state("document", b"live").expect("seed");
    let manifest = database.backup_to(&backup).expect("backup");
    drop(database);

    // An install that reached the installed phase and never cleaned up.
    fs::write(install_path(&path, "journal"), "installed\n").expect("plant the journal");

    let error = RiotDatabase::restore_from(&path, &backup, &manifest, DatabaseConfig::default())
        .expect_err("a restore must not race an unfinished install");
    assert_eq!(error, DatabaseError::BusyRetryable);
}
