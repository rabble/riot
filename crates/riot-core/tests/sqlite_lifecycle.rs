use riot_core::store::{
    CheckpointMode, DatabaseConfig, DatabaseError, Durability, JournalMode, RiotDatabase,
    CURRENT_SCHEMA_VERSION,
};
use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(1);

struct TestDir(PathBuf);

impl TestDir {
    fn new(label: &str) -> Self {
        let sequence = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "riot-sqlite-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create test directory");
        Self(path)
    }

    fn database(&self) -> PathBuf {
        self.0.join("riot.sqlite")
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn first_open_and_reopen_preserve_identity_generation_and_required_settings() {
    let directory = TestDir::new("reopen");
    let path = directory.database();
    let config = DatabaseConfig::default().with_busy_timeout(Duration::from_millis(250));

    let database = RiotDatabase::open(&path, config.clone()).expect("first open");
    assert_eq!(
        database.schema_version().expect("schema version"),
        CURRENT_SCHEMA_VERSION
    );
    let database_id = database.database_id().expect("database id");
    let database_generation = database.database_generation().expect("database generation");
    assert_ne!(database_id, [0; 16]);
    assert_ne!(database_generation, [0; 16]);
    assert_eq!(database.generation().expect("initial generation"), 0);

    let settings = database.settings().expect("database settings");
    assert_eq!(settings.journal_mode, JournalMode::Wal);
    assert!(settings.foreign_keys);
    assert_eq!(settings.durability, Durability::Full);
    assert_eq!(settings.busy_timeout, Duration::from_millis(250));
    assert_eq!(settings.wal_autocheckpoint_pages, 0);

    database
        .set_local_state("selected-space", b"namespace-a")
        .expect("commit local state");
    assert_eq!(database.generation().expect("advanced generation"), 1);
    drop(database);

    let reopened = RiotDatabase::open(&path, config).expect("reopen");
    assert_eq!(reopened.database_id().expect("database id"), database_id);
    assert_eq!(
        reopened.database_generation().expect("database generation"),
        database_generation
    );
    assert_eq!(reopened.generation().expect("generation"), 1);
    assert_eq!(
        reopened
            .local_state("selected-space")
            .expect("read local state"),
        Some(b"namespace-a".to_vec())
    );
    assert!(reopened.integrity_check().expect("integrity check"));
}

#[test]
fn one_writer_owns_a_path_and_reads_use_the_bounded_query_only_pool() {
    let directory = TestDir::new("ownership-pool");
    let path = directory.database();
    let timeout = Duration::from_millis(60);
    let config = DatabaseConfig::default()
        .with_busy_timeout(timeout)
        .with_reader_pool_size(1);
    let database = RiotDatabase::open(&path, config.clone()).expect("first writable owner");
    database
        .set_local_state("snapshot", b"value")
        .expect("seed snapshot value");

    let duplicate = RiotDatabase::open(&path, config.clone())
        .expect_err("a second writable owner must fail closed");
    assert!(matches!(duplicate, DatabaseError::BusyRetryable));
    assert_eq!(database.reader_pool_capacity(), 1);

    let snapshot = database.read_snapshot().expect("pin the only reader");
    assert_eq!(
        snapshot.local_state("snapshot").expect("snapshot read"),
        Some(b"value".to_vec())
    );
    assert!(snapshot.is_query_only().expect("query-only setting"));

    let started = Instant::now();
    let exhausted = database
        .local_state("snapshot")
        .expect_err("bounded pool must not create an extra reader");
    assert!(matches!(exhausted, DatabaseError::BusyRetryable));
    assert!(started.elapsed() >= Duration::from_millis(40));
    drop(snapshot);
    assert_eq!(
        database.local_state("snapshot").expect("reader returned"),
        Some(b"value".to_vec())
    );
    drop(database);
    RiotDatabase::open(&path, config).expect("ownership releases on final drop");
}

#[test]
fn migration_is_transactional_and_future_schema_is_rejected_without_mutation() {
    let directory = TestDir::new("migration");
    let path = directory.database();
    let bootstrap = Connection::open(&path).expect("bootstrap database");
    bootstrap
        .execute_batch(
            "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY NOT NULL);
             CREATE TRIGGER reject_migration BEFORE INSERT ON schema_migrations
             BEGIN SELECT RAISE(ABORT, 'interrupted migration'); END;",
        )
        .expect("install genuine SQLite failure boundary");
    drop(bootstrap);
    let before_failed_migration = fs::read(&path).expect("bytes before failed migration");

    let error = RiotDatabase::open(&path, DatabaseConfig::default())
        .expect_err("migration trigger must abort open");
    assert!(matches!(error, DatabaseError::MigrationFailed));
    assert_eq!(
        fs::read(&path).expect("bytes after failed migration"),
        before_failed_migration,
        "failed migration changed the main database"
    );
    for sidecar in [wal_path(&path), shm_path(&path), journal_path(&path)] {
        assert!(!sidecar.exists(), "failed migration left {sidecar:?}");
    }
    assert!(!table_exists(&path, "database_meta"));
    assert!(!table_exists(&path, "local_state"));

    let repair = Connection::open(&path).expect("open repair connection");
    repair
        .execute_batch("DROP TRIGGER reject_migration;")
        .expect("remove injected database trigger");
    drop(repair);

    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("retry migration");
    assert_eq!(
        database.schema_version().expect("schema version"),
        CURRENT_SCHEMA_VERSION
    );
    drop(database);

    let future = Connection::open(&path).expect("open future schema connection");
    future
        .execute(
            "INSERT INTO schema_migrations(version) VALUES (?1)",
            [CURRENT_SCHEMA_VERSION + 1],
        )
        .expect("mark unsupported future migration");
    drop(future);
    let before = fs::read(&path).expect("read future database bytes");

    let error = RiotDatabase::open(&path, DatabaseConfig::default())
        .expect_err("future schema must not open");
    assert!(matches!(
        error,
        DatabaseError::MigrationRequired { found, supported }
            if found == CURRENT_SCHEMA_VERSION + 1 && supported == CURRENT_SCHEMA_VERSION
    ));
    assert_eq!(fs::read(&path).expect("reread future database"), before);
}

#[test]
fn marker_only_or_noncontiguous_current_schema_fails_closed_without_mutation() {
    let directory = TestDir::new("structural-schema");
    let marker_only_path = directory.0.join("marker-only.sqlite");
    let marker_only = Connection::open(&marker_only_path).expect("marker-only database");
    marker_only
        .execute_batch(
            "CREATE TABLE schema_migrations (
                 version INTEGER PRIMARY KEY NOT NULL CHECK (version > 0)
             ) STRICT;
             INSERT INTO schema_migrations(version) VALUES (1);
             PRAGMA user_version = 1;",
        )
        .expect("write plausible version markers");
    drop(marker_only);
    let before = fs::read(&marker_only_path).expect("marker-only bytes");

    let error = RiotDatabase::open(&marker_only_path, DatabaseConfig::default())
        .expect_err("version markers cannot substitute for the schema");
    assert!(matches!(error, DatabaseError::CorruptDatabase));
    assert_eq!(
        fs::read(&marker_only_path).expect("marker-only bytes after rejection"),
        before
    );

    let valid_path = directory.0.join("ledger-gap.sqlite");
    let valid = RiotDatabase::open(&valid_path, DatabaseConfig::default()).expect("valid database");
    drop(valid);
    let connection = Connection::open(&valid_path).expect("edit migration ledger");
    connection
        .execute_batch(
            "DELETE FROM schema_migrations;
             INSERT INTO schema_migrations(version) VALUES (2);
             PRAGMA user_version = 1;",
        )
        .expect("make ledger noncontiguous");
    drop(connection);
    let before = fs::read(&valid_path).expect("ledger-gap bytes");
    let error = RiotDatabase::open(&valid_path, DatabaseConfig::default())
        .expect_err("noncontiguous ledger must fail closed");
    assert!(matches!(error, DatabaseError::CorruptDatabase));
    assert_eq!(
        fs::read(&valid_path).expect("ledger bytes after rejection"),
        before
    );
}

#[test]
fn current_schema_with_removed_constraints_or_table_flags_fails_closed_without_mutation() {
    let directory = TestDir::new("schema-fingerprint");
    let valid_ledger = "CREATE TABLE schema_migrations (
             version INTEGER PRIMARY KEY NOT NULL CHECK (version > 0)
         ) STRICT;";
    let ledger_without_check = "CREATE TABLE schema_migrations (
             version INTEGER PRIMARY KEY NOT NULL
         ) STRICT;";
    let valid_meta = "CREATE TABLE database_meta (
             singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
             database_id BLOB NOT NULL CHECK (length(database_id) = 16),
             database_generation BLOB NOT NULL CHECK (length(database_generation) = 16),
             generation INTEGER NOT NULL CHECK (generation >= 0),
             authority_quarantined INTEGER NOT NULL CHECK (authority_quarantined IN (0, 1))
         ) STRICT;";
    let meta_without_strict = "CREATE TABLE database_meta (
             singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
             database_id BLOB NOT NULL CHECK (length(database_id) = 16),
             database_generation BLOB NOT NULL CHECK (length(database_generation) = 16),
             generation INTEGER NOT NULL CHECK (generation >= 0),
             authority_quarantined INTEGER NOT NULL CHECK (authority_quarantined IN (0, 1))
         );";
    let valid_local = "CREATE TABLE local_state (
             key TEXT PRIMARY KEY NOT NULL CHECK (length(key) BETWEEN 1 AND 128),
             value BLOB NOT NULL CHECK (length(value) <= 1048576)
         ) STRICT, WITHOUT ROWID;";
    let local_without_rowid = "CREATE TABLE local_state (
             key TEXT PRIMARY KEY NOT NULL CHECK (length(key) BETWEEN 1 AND 128),
             value BLOB NOT NULL CHECK (length(value) <= 1048576)
         ) STRICT;";
    let forgeries = [
        (
            "missing-check",
            ledger_without_check,
            valid_meta,
            valid_local,
        ),
        (
            "missing-strict",
            valid_ledger,
            meta_without_strict,
            valid_local,
        ),
        (
            "missing-without-rowid",
            valid_ledger,
            valid_meta,
            local_without_rowid,
        ),
    ];

    for (label, ledger, meta, local) in forgeries {
        let path = directory.0.join(format!("{label}.sqlite"));
        let connection = Connection::open(&path).expect("open forged schema");
        connection
            .execute_batch(&format!(
                "{ledger}
                 {meta}
                 {local}
                 INSERT INTO schema_migrations(version) VALUES (1);
                 INSERT INTO database_meta(
                     singleton, database_id, database_generation, generation,
                     authority_quarantined
                 ) VALUES (1, randomblob(16), randomblob(16), 0, 0);
                 PRAGMA user_version = 1;"
            ))
            .expect("create structurally plausible forgery");
        drop(connection);
        let before = fs::read(&path).expect("forged bytes before open");

        let error = RiotDatabase::open(&path, DatabaseConfig::default())
            .expect_err("schema definition forgery must fail closed");
        assert!(
            matches!(error, DatabaseError::CorruptDatabase),
            "unexpected error for {label}: {error:?}"
        );
        assert_eq!(
            fs::read(&path).expect("forged bytes after rejection"),
            before,
            "opening {label} mutated the rejected database"
        );
    }
}

#[test]
fn contended_write_honors_the_bounded_timeout_and_is_retryable() {
    let directory = TestDir::new("busy");
    let path = directory.database();
    let timeout = Duration::from_millis(80);
    let database = RiotDatabase::open(&path, DatabaseConfig::default().with_busy_timeout(timeout))
        .expect("open database");

    let blocker = Connection::open(&path).expect("open contending writer");
    blocker
        .execute_batch("BEGIN IMMEDIATE;")
        .expect("hold real SQLite writer lock");

    let started = Instant::now();
    let error = database
        .set_local_state("blocked", b"value")
        .expect_err("contended mutation must time out");
    let elapsed = started.elapsed();
    assert!(matches!(error, DatabaseError::BusyRetryable));
    assert!(
        elapsed >= Duration::from_millis(60),
        "returned too early: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(1),
        "wait was not bounded: {elapsed:?}"
    );
    assert_eq!(database.generation().expect("unchanged generation"), 0);

    blocker
        .execute_batch("ROLLBACK;")
        .expect("release writer lock");
    database
        .set_local_state("blocked", b"value")
        .expect("retry after contention");
}

#[test]
fn checkpoint_reports_a_long_reader_then_truncates_after_reader_finishes() {
    let directory = TestDir::new("checkpoint");
    let path = directory.database();
    let database = RiotDatabase::open(
        &path,
        DatabaseConfig::default().with_busy_timeout(Duration::from_millis(50)),
    )
    .expect("open database");

    let reader = Connection::open(&path).expect("open long-lived reader");
    reader.execute_batch("BEGIN;").expect("begin read snapshot");
    let _: i64 = reader
        .query_row(
            "SELECT generation FROM database_meta WHERE singleton = 1",
            [],
            |row| row.get(0),
        )
        .expect("establish read snapshot");

    database
        .set_local_state("after-reader", &vec![7; 16 * 1024])
        .expect("write after read snapshot");
    let blocked = database
        .checkpoint(CheckpointMode::Truncate)
        .expect("checkpoint reports reader contention");
    assert!(blocked.busy);
    assert!(blocked.log_frames > blocked.checkpointed_frames);

    reader
        .execute_batch("ROLLBACK;")
        .expect("finish read snapshot");
    let completed = database
        .checkpoint(CheckpointMode::Truncate)
        .expect("truncate checkpoint");
    assert!(!completed.busy);
    assert_eq!(completed.log_frames, 0);
    assert_eq!(completed.checkpointed_frames, 0);
    assert_eq!(
        fs::metadata(wal_path(&path)).expect("WAL metadata").len(),
        0
    );
}

#[test]
fn pinned_reader_triggers_hard_wal_backpressure_and_bounds_repeated_growth() {
    let directory = TestDir::new("wal-backpressure");
    let path = directory.database();
    let config = DatabaseConfig::default()
        .with_busy_timeout(Duration::from_millis(30))
        .with_reader_pool_size(1)
        .with_checkpoint_pages(4, 64);
    let database = RiotDatabase::open(&path, config).expect("open database");
    database
        .set_local_state("pinned", b"before-writes")
        .expect("seed pinned snapshot");
    let snapshot = database.read_snapshot().expect("pin reader");
    assert_eq!(
        snapshot.local_state("pinned").expect("establish snapshot"),
        Some(b"before-writes".to_vec())
    );

    let oversized = database
        .set_local_state("would-cross-hard-limit", &vec![3; 256 * 1024])
        .expect_err("admission must reject a transaction that would cross the hard limit");
    assert!(matches!(oversized, DatabaseError::BusyRetryable));

    let mut observed_backpressure = false;
    for index in 0..100 {
        match database.set_local_state(&format!("write-{index}"), &vec![index as u8; 4096]) {
            Ok(()) => {}
            Err(DatabaseError::BusyRetryable) => {
                observed_backpressure = true;
                break;
            }
            Err(other) => panic!("unexpected write error: {other:?}"),
        }
    }
    assert!(
        observed_backpressure,
        "pinned reader must eventually stop writers"
    );
    let bounded_wal_bytes = fs::metadata(wal_path(&path)).expect("WAL metadata").len();
    assert!(
        bounded_wal_bytes <= wal_absolute_bound(&path, 64),
        "WAL grew beyond documented absolute bound: {bounded_wal_bytes} bytes"
    );

    drop(snapshot);
    assert_eq!(
        database
            .local_state("would-cross-hard-limit")
            .expect("rejected value lookup"),
        None
    );
    database
        .set_local_state("after-pressure", b"accepted")
        .expect("writers resume after pinned reader releases");
    database
        .checkpoint(CheckpointMode::Truncate)
        .expect("final truncate checkpoint");
    assert_eq!(
        fs::metadata(wal_path(&path)).expect("WAL metadata").len(),
        0
    );
}

#[test]
fn read_only_full_and_corrupt_storage_have_typed_fail_closed_errors() {
    let directory = TestDir::new("storage-errors");
    let path = directory.database();
    let config = DatabaseConfig::default().with_max_page_count(10);
    let database = RiotDatabase::open(&path, config.clone()).expect("open database");
    database
        .set_local_state("preserved", b"before-errors")
        .expect("seed preserved state");
    drop(database);

    let read_only =
        RiotDatabase::open_read_only(&path, config.clone()).expect("open read-only database");
    assert_eq!(
        read_only
            .local_state("preserved")
            .expect("read preserved value"),
        Some(b"before-errors".to_vec())
    );
    assert!(matches!(
        read_only.set_local_state("forbidden", b"write"),
        Err(DatabaseError::StorageReadOnly)
    ));
    drop(read_only);

    let database = RiotDatabase::open(&path, config).expect("reopen writable");

    let mut observed_full = false;
    for index in 0..32 {
        match database.set_local_state(&format!("fill-{index}"), &vec![9; 8 * 1024]) {
            Ok(()) => {}
            Err(DatabaseError::StorageFull) => {
                observed_full = true;
                break;
            }
            Err(other) => panic!("unexpected storage error: {other:?}"),
        }
    }
    assert!(
        observed_full,
        "real SQLite page limit must surface StorageFull"
    );
    assert_eq!(
        database
            .local_state("preserved")
            .expect("preserved after full"),
        Some(b"before-errors".to_vec())
    );
    drop(database);

    let corrupt_path = directory.0.join("corrupt.sqlite");
    let corrupt_bytes = b"this is not a sqlite database".to_vec();
    fs::write(&corrupt_path, &corrupt_bytes).expect("write corrupt database");
    let error = RiotDatabase::open(&corrupt_path, DatabaseConfig::default())
        .expect_err("corrupt database must fail closed");
    assert!(matches!(error, DatabaseError::CorruptDatabase));
    assert_eq!(
        fs::read(corrupt_path).expect("corrupt bytes preserved"),
        corrupt_bytes
    );
}

fn table_exists(path: &Path, table: &str) -> bool {
    let database = Connection::open(path).expect("inspect database");
    database
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [table],
            |row| row.get(0),
        )
        .expect("inspect table")
}

fn wal_path(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}-wal", path.display()))
}

fn shm_path(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}-shm", path.display()))
}

fn journal_path(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}-journal", path.display()))
}

fn wal_absolute_bound(path: &Path, hard_frames: u64) -> u64 {
    let connection = Connection::open(path).expect("open for page size");
    let page_size: i64 = connection
        .query_row("PRAGMA page_size", [], |row| row.get(0))
        .expect("page size");
    32 + hard_frames * (24 + u64::try_from(page_size).expect("positive page size"))
}
