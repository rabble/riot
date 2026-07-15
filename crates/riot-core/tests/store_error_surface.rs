//! The database layer's refusal surface: every error it can hand a caller,
//! provoked through the real primitive that produces it (a read-only handle, an
//! exhausted reader pool, a byte-level corruption, a path that is not a
//! database) rather than through a fake. The diagnostic impls are asserted too,
//! because native shells log them and a silently-renamed code is a silently
//! broken log.

use riot_core::store::{
    CheckpointMode, DatabaseConfig, DatabaseError, RiotDatabase, CURRENT_SCHEMA_VERSION,
};
use rusqlite::Connection;
use std::fs;
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(1);

struct TestDir(PathBuf);

impl TestDir {
    fn new(label: &str) -> Self {
        let sequence = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "riot-store-surface-{label}-{}-{sequence}",
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

/// Every `DatabaseError` renders a distinct, stable sentence. These strings
/// reach native crash logs, so a variant that renders as another variant's text
/// is a real defect even though nothing panics.
#[test]
fn every_database_error_renders_its_own_message() {
    let cases = [
        (DatabaseError::InvalidInput, "invalid database input"),
        (DatabaseError::BusyRetryable, "database is busy"),
        (DatabaseError::StorageFull, "database storage is full"),
        (
            DatabaseError::StorageReadOnly,
            "database storage is read-only",
        ),
        (DatabaseError::CorruptDatabase, "database is corrupt"),
        (
            DatabaseError::MigrationRequired {
                found: 9,
                supported: CURRENT_SCHEMA_VERSION,
            },
            "database migration is required",
        ),
        (DatabaseError::MigrationFailed, "database migration failed"),
        (
            DatabaseError::BackupMismatch,
            "backup does not match its manifest",
        ),
        (
            DatabaseError::StorageIo,
            "database storage operation failed",
        ),
        (DatabaseError::Internal, "internal database failure"),
    ];
    for (error, expected) in cases {
        assert_eq!(error.to_string(), expected, "{error:?} rendered wrongly");
    }
    // Distinctness: ten variants, ten sentences.
    let mut rendered: Vec<_> = cases.iter().map(|(error, _)| error.to_string()).collect();
    rendered.sort();
    rendered.dedup();
    assert_eq!(rendered.len(), cases.len());

    // `Error` is implemented, so these compose into anyhow/Box<dyn Error> chains.
    let boxed: Box<dyn std::error::Error> = Box::new(DatabaseError::StorageIo);
    assert_eq!(boxed.to_string(), "database storage operation failed");
}

/// The handles are `Debug`, and neither leaks a live SQLite connection into the
/// formatted output. The database names its path; the snapshot names nothing.
#[test]
fn handles_render_debug_without_exposing_connections() {
    let directory = TestDir::new("debug");
    let path = directory.database();
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("open");

    let rendered = format!("{database:?}");
    assert!(rendered.starts_with("RiotDatabase"), "{rendered}");
    assert!(rendered.contains("riot.sqlite"), "{rendered}");
    assert!(rendered.contains("read_only"), "{rendered}");
    assert!(!rendered.contains("Connection"), "{rendered}");

    let snapshot = database.read_snapshot().expect("snapshot");
    let rendered = format!("{snapshot:?}");
    assert!(rendered.starts_with("RiotReadSnapshot"), "{rendered}");
    assert!(!rendered.contains("Connection"), "{rendered}");
    assert!(snapshot.is_query_only().expect("query_only"));
}

/// The config's busy timeout is readable back, so a caller can honour the same
/// deadline it configured when it retries a `BusyRetryable`.
#[test]
fn config_reports_the_busy_timeout_it_was_given() {
    let config = DatabaseConfig::default();
    assert_eq!(config.busy_timeout(), Duration::from_secs(2));
    let config = config.with_busy_timeout(Duration::from_millis(250));
    assert_eq!(config.busy_timeout(), Duration::from_millis(250));

    let directory = TestDir::new("busy-timeout");
    let database = RiotDatabase::open(directory.database(), config).expect("open");
    // The configured deadline reaches SQLite, not just the struct.
    assert_eq!(
        database.settings().expect("settings").busy_timeout,
        Duration::from_millis(250)
    );
}

/// All four checkpoint modes issue their own pragma and report frame counts.
/// `Restart` and `Truncate` are the ones the WAL ceiling depends on, and both
/// were previously never invoked by any test.
#[test]
fn every_checkpoint_mode_runs_and_reports_frames() {
    let directory = TestDir::new("checkpoint-modes");
    let database =
        RiotDatabase::open(directory.database(), DatabaseConfig::default()).expect("open");
    database
        .set_local_state("key", b"value")
        .expect("seed a wal frame");

    // Passive first: it reports the frames the write left behind.
    let passive = database
        .checkpoint(CheckpointMode::Passive)
        .expect("passive checkpoint");
    assert!(!passive.busy);

    for mode in [
        CheckpointMode::Full,
        CheckpointMode::Restart,
        CheckpointMode::Truncate,
    ] {
        let result = database.checkpoint(mode).expect("checkpoint");
        assert!(!result.busy, "{mode:?} reported busy with no other readers");
        assert_eq!(
            result.log_frames, result.checkpointed_frames,
            "{mode:?} left frames behind with no other readers"
        );
    }
    // Truncate is last, so the WAL is now empty on disk.
    let wal = PathBuf::from(format!("{}-wal", directory.database().display()));
    assert_eq!(fs::metadata(&wal).map(|meta| meta.len()).unwrap_or(0), 0);

    // The data survived every checkpoint.
    assert_eq!(
        database.local_state("key").expect("read back"),
        Some(b"value".to_vec())
    );
}

/// A write that fills the WAL past the soft ceiling triggers the automatic
/// restart checkpoint, so the log is recycled instead of growing without bound.
#[test]
fn crossing_the_soft_ceiling_restarts_the_wal_automatically() {
    let directory = TestDir::new("auto-checkpoint");
    // `set_local_state` reserves 32 pages of headroom, so the hard ceiling must
    // admit it; the soft ceiling is set below the frames one write produces so
    // the post-commit restart fires.
    let config = DatabaseConfig::default().with_checkpoint_pages(1, 64);
    let database = RiotDatabase::open(directory.database(), config).expect("open");

    database
        .set_local_state("document", b"first")
        .expect("write over the soft ceiling");

    // The automatic restart ran: the WAL is recycled, so a passive checkpoint
    // now finds nothing left to copy.
    let after = database
        .checkpoint(CheckpointMode::Passive)
        .expect("passive checkpoint");
    assert!(!after.busy);
    assert_eq!(after.log_frames, after.checkpointed_frames);

    // And the write is durable regardless of the recycling.
    assert_eq!(
        database.local_state("document").expect("read back"),
        Some(b"first".to_vec())
    );
    database
        .set_local_state("document", b"second")
        .expect("second write after a restart");
    assert_eq!(
        database.local_state("document").expect("read back"),
        Some(b"second".to_vec())
    );
}

/// Local-state keys and values are bounded, and both bounds are refused as
/// `InvalidInput` before any transaction opens.
#[test]
fn local_state_bounds_are_refused_without_a_write() {
    let directory = TestDir::new("bounds");
    let database =
        RiotDatabase::open(directory.database(), DatabaseConfig::default()).expect("open");
    let before = database.generation().expect("generation");

    let oversized_key = "k".repeat(129);
    let oversized_value = vec![0_u8; 1024 * 1024 + 1];
    let cases: [(&str, &[u8]); 3] = [
        ("", b"value"),
        (oversized_key.as_str(), b"value"),
        ("key", oversized_value.as_slice()),
    ];
    for (key, value) in cases {
        assert_eq!(
            database.set_local_state(key, value),
            Err(DatabaseError::InvalidInput),
            "key {:?} / {} value bytes was admitted",
            &key[..key.len().min(8)],
            value.len()
        );
    }

    // Reads validate the key on the same rule, on both the database and a snapshot.
    assert_eq!(database.local_state(""), Err(DatabaseError::InvalidInput));
    assert_eq!(
        database.local_state(&oversized_key),
        Err(DatabaseError::InvalidInput)
    );
    let snapshot = database.read_snapshot().expect("snapshot");
    assert_eq!(snapshot.local_state(""), Err(DatabaseError::InvalidInput));
    assert_eq!(
        snapshot.local_state(&oversized_key),
        Err(DatabaseError::InvalidInput)
    );

    // Nothing was written: the generation counter never moved.
    assert_eq!(database.generation().expect("generation"), before);
}

/// A read-only handle refuses every mutating primitive, and says read-only
/// rather than pretending the write succeeded.
#[test]
fn a_read_only_handle_refuses_writes_checkpoints_and_backups() {
    let directory = TestDir::new("read-only");
    let path = directory.database();
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("open writable");
    database.set_local_state("seed", b"value").expect("seed");
    drop(database);

    let reader =
        RiotDatabase::open_read_only(&path, DatabaseConfig::default()).expect("open read-only");
    assert_eq!(
        reader.local_state("seed").expect("read-only read"),
        Some(b"value".to_vec())
    );

    assert_eq!(
        reader.set_local_state("seed", b"other"),
        Err(DatabaseError::StorageReadOnly)
    );
    assert_eq!(
        reader.checkpoint(CheckpointMode::Truncate),
        Err(DatabaseError::StorageReadOnly)
    );
    assert_eq!(
        reader
            .backup_to(directory.path("backup.sqlite"))
            .expect_err("read-only backup refused"),
        DatabaseError::StorageReadOnly
    );

    // The value is unchanged after all three refusals.
    assert_eq!(
        reader.local_state("seed").expect("read-only read"),
        Some(b"value".to_vec())
    );
}

/// Backing a database up over itself would destroy the source it is copying, so
/// it is refused before a single byte moves.
#[test]
fn a_backup_onto_the_live_path_is_refused() {
    let directory = TestDir::new("self-backup");
    let path = directory.database();
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("open");
    database.set_local_state("seed", b"value").expect("seed");
    let before = fs::read(&path).expect("bytes before");

    assert_eq!(
        database.backup_to(&path).expect_err("self-backup refused"),
        DatabaseError::StorageReadOnly
    );
    assert_eq!(fs::read(&path).expect("bytes after"), before);
    assert_eq!(
        database.local_state("seed").expect("still readable"),
        Some(b"value".to_vec())
    );
}

/// Paths that cannot name a database are refused before SQLite is asked to open
/// them: an empty path and a root path have no file name, and a parent that is
/// not a directory cannot hold one.
#[test]
fn unusable_paths_are_refused_before_sqlite_sees_them() {
    let directory = TestDir::new("paths");

    assert_eq!(
        RiotDatabase::open("", DatabaseConfig::default()).expect_err("empty path refused"),
        DatabaseError::InvalidInput
    );
    assert_eq!(
        RiotDatabase::open("/", DatabaseConfig::default()).expect_err("root path refused"),
        DatabaseError::InvalidInput
    );

    // The parent exists as a *file*, so it can never hold a database.
    let file_parent = directory.path("not-a-directory");
    fs::write(&file_parent, b"i am a file").expect("write file");
    assert_eq!(
        RiotDatabase::open(file_parent.join("riot.sqlite"), DatabaseConfig::default())
            .expect_err("file parent refused"),
        DatabaseError::StorageIo
    );

    // A read-only open additionally requires the file itself to exist.
    assert_eq!(
        RiotDatabase::open_read_only(directory.path("absent.sqlite"), DatabaseConfig::default())
            .expect_err("absent read-only database refused"),
        DatabaseError::StorageIo
    );
}

/// The reader pool is bounded and the bound is honoured: with one connection and
/// one lease outstanding, the next reader waits its configured timeout and then
/// reports a retryable busy rather than blocking forever or opening a connection
/// outside the pool.
#[test]
fn an_exhausted_reader_pool_times_out_as_retryable() {
    let directory = TestDir::new("reader-pool");
    let config = DatabaseConfig::default()
        .with_reader_pool_size(1)
        .with_busy_timeout(Duration::from_millis(50));
    let database = RiotDatabase::open(directory.database(), config).expect("open");
    assert_eq!(database.reader_pool_capacity(), 1);
    database.set_local_state("key", b"value").expect("seed");

    let snapshot = database.read_snapshot().expect("hold the only reader");

    // Every read path goes through the same pool, so all of them report busy.
    assert_eq!(
        database.local_state("key"),
        Err(DatabaseError::BusyRetryable)
    );
    assert_eq!(database.generation(), Err(DatabaseError::BusyRetryable));
    assert!(matches!(
        database.read_snapshot(),
        Err(DatabaseError::BusyRetryable)
    ));

    // The lease is returned on drop, and the pool recovers.
    drop(snapshot);
    assert_eq!(
        database.local_state("key").expect("pool recovered"),
        Some(b"value".to_vec())
    );

    // A zero timeout is the degenerate case of the same rule: a caller that has
    // asked to never wait is told to retry immediately rather than being parked.
    let directory = TestDir::new("reader-pool-nowait");
    let config = DatabaseConfig::default()
        .with_reader_pool_size(1)
        .with_busy_timeout(Duration::ZERO);
    let database = RiotDatabase::open(directory.database(), config).expect("open");
    database.set_local_state("key", b"value").expect("seed");

    let snapshot = database.read_snapshot().expect("hold the only reader");
    let started = std::time::Instant::now();
    assert_eq!(
        database.local_state("key"),
        Err(DatabaseError::BusyRetryable)
    );
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "a zero timeout must not park the caller"
    );
    drop(snapshot);
    assert_eq!(
        database.local_state("key").expect("pool recovered"),
        Some(b"value".to_vec())
    );
}

/// A database whose journal mode was taken out of WAL behind our back is not a
/// database we will read: WAL is the durability contract, and a rolled-back
/// journal mode silently changes the crash semantics.
#[test]
fn a_database_downgraded_out_of_wal_is_refused_read_only() {
    let directory = TestDir::new("not-wal");
    let path = directory.database();
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("open");
    database.set_local_state("seed", b"value").expect("seed");
    drop(database);

    let connection = Connection::open(&path).expect("raw connection");
    let mode: String = connection
        .query_row("PRAGMA journal_mode = DELETE", [], |row| row.get(0))
        .expect("downgrade the journal mode");
    assert_eq!(mode.to_ascii_lowercase(), "delete");
    drop(connection);

    assert_eq!(
        RiotDatabase::open_read_only(&path, DatabaseConfig::default())
            .expect_err("non-WAL database refused"),
        DatabaseError::CorruptDatabase
    );
}

/// Byte-level damage under a live handle is reported by `integrity_check` as a
/// clean `false`, not as a panic and not as a silent success.
#[test]
fn integrity_check_reports_byte_level_damage_as_false() {
    let directory = TestDir::new("integrity");

    // A healthy database checks out true.
    let healthy = RiotDatabase::open(directory.path("healthy.sqlite"), DatabaseConfig::default())
        .expect("open healthy");
    healthy.set_local_state("key", b"value").expect("seed");
    assert!(healthy.integrity_check().expect("healthy check"));
    drop(healthy);

    let path = directory.database();
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("open");
    // Enough rows to spill past the first page, so there is a b-tree page worth
    // corrupting. Nothing reads these pages back through the reader pool before
    // the corruption lands — a reader that had already scanned them would answer
    // from its own page cache and never see the damage.
    for index in 0..64 {
        database
            .set_local_state(&format!("key-{index:03}"), &[b'v'; 512])
            .expect("seed rows");
    }
    database
        .checkpoint(CheckpointMode::Truncate)
        .expect("fold the WAL back into the main file");

    // Destroy the b-tree page *headers* of the interior pages. Page 1 (the file
    // header and schema) is left intact, so the file still opens and its schema
    // still validates — what is broken is the tree those pages describe, which
    // is exactly what a structural check exists to notice. The reader pool has
    // never read these pages (it only touched the schema at open), so no cached
    // copy hides the damage.
    let page_size = Connection::open(&path)
        .expect("raw connection")
        .query_row("PRAGMA page_size", [], |row| row.get::<_, i64>(0))
        .expect("page size") as u64;
    let length = fs::metadata(&path).expect("metadata").len();
    let pages = length / page_size;
    assert!(pages > 4, "database did not grow past a handful of pages");

    let mut file = fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .expect("open for corruption");
    for page in 2..=pages {
        file.seek(SeekFrom::Start((page - 1) * page_size))
            .expect("seek to a page boundary");
        // 0x07 is not a legal b-tree page type (2, 5, 10 and 13 are).
        file.write_all(&[0x07]).expect("scribble a page header");
    }
    file.sync_all().expect("flush corruption");
    drop(file);

    // The live handle reports the damage as a clean `false` — not a panic, and
    // not a silent success.
    assert!(
        !database.integrity_check().expect("damaged check"),
        "corruption was not reported by the live handle"
    );

    // And a fresh open refuses the file outright rather than serving it.
    drop(database);
    assert_eq!(
        RiotDatabase::open(&path, DatabaseConfig::default()).expect_err("corrupt database refused"),
        DatabaseError::CorruptDatabase
    );
}
