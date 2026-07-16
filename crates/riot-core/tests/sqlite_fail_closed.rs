//! Fail-closed / tamper-detection coverage for the durable SQLite store.
//!
//! Every test here provokes a *real* failure — a genuine SQLite error, a
//! genuine invalid input, or a physically tampered ledger row — and asserts
//! that the store surfaces the typed error (or `SessionError`) instead of
//! trusting corrupt state or panicking. The evidence-layer tests exercise the
//! security property "a tampered evidence ledger is detected, not trusted":
//! `SqliteEvidenceStore::load` re-audits the whole ledger on every read, and
//! the persist path re-audits the on-disk rows against the in-memory join.

use riot_core::apps::entry::build_app_data_entry;
use riot_core::import::encode_bundle;
use riot_core::session::{CommitOutcome, ImportContext, RiotSession, SessionError};
use riot_core::store::{CheckpointMode, DatabaseConfig, DatabaseError, RiotDatabase};
use riot_core::willow::{
    authorise_entry, encode_capability, encode_entry, EvidenceAuthor, SignedWillowEntry,
};
use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(1);

struct TestDir(PathBuf);

impl TestDir {
    fn new(label: &str) -> Self {
        let sequence = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "riot-failclosed-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create test directory");
        Self(path)
    }

    fn database(&self) -> PathBuf {
        self.0.join("riot.sqlite")
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

fn install_path(path: &Path, suffix: &str) -> PathBuf {
    let parent = path.parent().expect("database parent");
    let name = path.file_name().expect("database name").to_string_lossy();
    parent.join(format!(".{name}.install-{suffix}"))
}

/// Builds a standalone single-file database whose newest state is folded into
/// the main file (WAL truncated) so it can be moved wholesale into an install
/// family slot without carrying uncommitted sidecar frames.
fn seed_single_file_db(path: &Path, value: &[u8]) {
    let database = RiotDatabase::open(path, DatabaseConfig::default()).expect("seed database");
    database
        .set_local_state("value", value)
        .expect("seed value");
    database
        .checkpoint(CheckpointMode::Truncate)
        .expect("fold WAL into main");
    drop(database);
}

/// Moves an entire family (main + -wal + -shm) from `from` to a base path.
fn move_family(from: &Path, to_base: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let src = PathBuf::from(format!("{}{suffix}", from.display()));
        let dst = PathBuf::from(format!("{}{suffix}", to_base.display()));
        if src.exists() {
            fs::rename(&src, &dst).expect("move family member");
        }
    }
}

fn author() -> EvidenceAuthor {
    riot_core::willow::generate_communal_author().expect("production author")
}

fn signed_app(
    author: &EvidenceAuthor,
    app_id: [u8; 32],
    key: &str,
    timestamp: u64,
    payload: &[u8],
) -> SignedWillowEntry {
    let entry = build_app_data_entry(author, &app_id, key, timestamp, payload).expect("entry");
    let authorised = authorise_entry(author, entry).expect("authorise");
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    SignedWillowEntry {
        entry_bytes: encode_entry(authorised.entry()),
        capability_bytes: encode_capability(token.capability()),
        signature: signature.to_bytes(),
        payload_bytes: payload.to_vec(),
    }
}

fn commit(store: &riot_core::session::EvidenceStore, entry: &SignedWillowEntry) -> CommitOutcome {
    store
        .inspect(
            &encode_bundle(std::slice::from_ref(entry)).expect("bundle"),
            ImportContext::new("failclosed-test"),
        )
        .expect("inspect")
        .expect_preview()
        .plan_all()
        .expect("plan")
        .commit()
        .expect("commit")
}

fn open_store(path: &Path) -> (RiotSession, riot_core::session::EvidenceStore) {
    let database = RiotDatabase::open(path, DatabaseConfig::default()).expect("open database");
    let session = RiotSession::open_sqlite(database).expect("session");
    let store = session.create_store().expect("store");
    (session, store)
}

/// Reopens through the ordinary API and asserts the evidence audit fails
/// closed at load time (corrupt ledger surfaces as `SessionError::Internal`).
fn assert_reopen_fails_closed(path: &Path) {
    let database = RiotDatabase::open(path, DatabaseConfig::default()).expect("physical open");
    let session = RiotSession::open_sqlite(database).expect("session handle");
    assert!(matches!(
        session.create_store(),
        Err(SessionError::Internal)
    ));
}

// ---------------------------------------------------------------------------
// database.rs: config, Display/Debug, checkpoint arms, typed refusals
// ---------------------------------------------------------------------------

#[test]
fn config_getter_reports_the_configured_busy_timeout() {
    let timeout = Duration::from_millis(1234);
    let config = DatabaseConfig::default().with_busy_timeout(timeout);
    assert_eq!(config.busy_timeout(), timeout);
}

#[test]
fn every_database_error_has_a_distinct_stable_display_message() {
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
                found: 3,
                supported: 2,
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
    for (error, message) in cases {
        assert_eq!(error.to_string(), message);
        // `std::error::Error` is implemented; exercise it as a trait object.
        let dynamic: &dyn std::error::Error = &error;
        assert_eq!(dynamic.to_string(), message);
    }
}

#[test]
fn database_and_snapshot_debug_impls_are_non_exhaustive_and_safe() {
    let directory = TestDir::new("debug");
    let path = directory.database();
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("open");
    let rendered = format!("{database:?}");
    assert!(rendered.contains("RiotDatabase"));
    assert!(rendered.contains("read_only"));

    let snapshot = database.read_snapshot().expect("snapshot");
    let rendered = format!("{snapshot:?}");
    assert!(rendered.contains("RiotReadSnapshot"));
}

#[test]
fn full_and_restart_checkpoint_modes_execute_their_own_sql() {
    let directory = TestDir::new("checkpoint-modes");
    let path = directory.database();
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("open");
    database.set_local_state("k", b"v").expect("write");

    let full = database
        .checkpoint(CheckpointMode::Full)
        .expect("full checkpoint");
    assert!(!full.busy);
    let restart = database
        .checkpoint(CheckpointMode::Restart)
        .expect("restart checkpoint");
    assert!(!restart.busy);
}

#[test]
fn automatic_restart_checkpoint_fires_once_the_soft_threshold_is_reached() {
    let directory = TestDir::new("auto-restart");
    let path = directory.database();
    // A tiny soft threshold with no pinned reader lets the passive checkpoint
    // fully drain the log, so the post-commit automatic RESTART arm runs.
    let config = DatabaseConfig::default().with_checkpoint_pages(2, 64);
    let database = RiotDatabase::open(&path, config).expect("open");
    for index in 0..20 {
        database
            .set_local_state(&format!("key-{index}"), &vec![index as u8; 2048])
            .expect("write");
    }
    assert_eq!(database.generation().expect("generation"), 20);
}

#[test]
fn invalid_local_state_key_and_value_sizes_fail_closed() {
    let directory = TestDir::new("invalid-local-state");
    let path = directory.database();
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("open");

    assert!(matches!(
        database.set_local_state("", b"value"),
        Err(DatabaseError::InvalidInput)
    ));
    let long_key = "k".repeat(129);
    assert!(matches!(
        database.set_local_state(&long_key, b"value"),
        Err(DatabaseError::InvalidInput)
    ));
    let huge_value = vec![0_u8; 1024 * 1024 + 1];
    assert!(matches!(
        database.set_local_state("key", &huge_value),
        Err(DatabaseError::InvalidInput)
    ));
    // The read path validates the key too (a distinct guard).
    assert!(matches!(
        database.local_state(""),
        Err(DatabaseError::InvalidInput)
    ));
}

#[test]
fn read_only_database_refuses_checkpoint_and_backup_refuses_self_target() {
    let directory = TestDir::new("readonly-refusals");
    let path = directory.database();
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("open");
    database.set_local_state("k", b"v").expect("seed");
    // backup onto its own path is refused as read-only.
    assert!(matches!(
        database.backup_to(&path),
        Err(DatabaseError::StorageReadOnly)
    ));
    drop(database);

    let read_only =
        RiotDatabase::open_read_only(&path, DatabaseConfig::default()).expect("read-only");
    assert!(matches!(
        read_only.checkpoint(CheckpointMode::Passive),
        Err(DatabaseError::StorageReadOnly)
    ));
    // A read-only source cannot be backed up.
    let elsewhere = directory.path("copy.sqlite");
    assert!(matches!(
        read_only.backup_to(&elsewhere),
        Err(DatabaseError::StorageReadOnly)
    ));
}

#[test]
fn invalid_and_unopenable_paths_fail_closed() {
    // Empty path: no file name at all.
    assert!(matches!(
        RiotDatabase::open("", DatabaseConfig::default()),
        Err(DatabaseError::InvalidInput)
    ));

    let directory = TestDir::new("bad-paths");
    // Read-only open of a file that does not exist.
    let missing = directory.path("missing.sqlite");
    assert!(matches!(
        RiotDatabase::open_read_only(&missing, DatabaseConfig::default()),
        Err(DatabaseError::StorageIo)
    ));

    // A path whose parent is not a directory.
    let not_a_dir = directory.path("regular-file");
    fs::write(&not_a_dir, b"x").expect("write file");
    let nested = not_a_dir.join("child.sqlite");
    assert!(matches!(
        RiotDatabase::open(&nested, DatabaseConfig::default()),
        Err(DatabaseError::StorageIo)
    ));

    // A path that passes validation but is a directory: SQLite cannot open it.
    let dir_as_db = directory.path("is-a-directory");
    fs::create_dir(&dir_as_db).expect("make directory");
    assert!(matches!(
        RiotDatabase::open(&dir_as_db, DatabaseConfig::default()),
        Err(DatabaseError::StorageIo)
    ));
}

#[test]
fn exhausted_reader_pool_with_zero_timeout_is_retryable_without_waiting() {
    let directory = TestDir::new("pool-zero-timeout");
    let path = directory.database();
    let config = DatabaseConfig::default()
        .with_reader_pool_size(1)
        .with_busy_timeout(Duration::ZERO);
    let database = RiotDatabase::open(&path, config).expect("open");
    database.set_local_state("k", b"v").expect("seed");
    let snapshot = database.read_snapshot().expect("pin the only reader");
    // With a zero timeout the deadline is already elapsed on the first probe.
    assert!(matches!(
        database.local_state("k"),
        Err(DatabaseError::BusyRetryable)
    ));
    drop(snapshot);
    assert_eq!(
        database.local_state("k").expect("reader returned"),
        Some(b"v".to_vec())
    );
}

#[test]
fn read_only_open_of_a_non_wal_database_fails_closed() {
    let directory = TestDir::new("non-wal-readonly");
    let path = directory.database();
    seed_single_file_db(&path, b"value");

    // Downgrade the persistent journal mode out of WAL with a raw connection.
    let raw = Connection::open(&path).expect("raw open");
    let mode: String = raw
        .query_row("PRAGMA journal_mode = DELETE", [], |row| row.get(0))
        .expect("set delete mode");
    assert_eq!(mode.to_ascii_lowercase(), "delete");
    drop(raw);

    // The read-only open path requires WAL and must refuse a non-WAL file.
    assert!(matches!(
        RiotDatabase::open_read_only(&path, DatabaseConfig::default()),
        Err(DatabaseError::CorruptDatabase)
    ));
}

#[test]
fn real_sqlite_page_limit_surfaces_storage_full() {
    let directory = TestDir::new("storage-full");
    let path = directory.database();
    let config = DatabaseConfig::default().with_max_page_count(12);
    let database = RiotDatabase::open(&path, config).expect("open");
    let mut observed_full = false;
    for index in 0..64 {
        match database.set_local_state(&format!("fill-{index}"), &vec![7; 8 * 1024]) {
            Ok(()) => {}
            Err(DatabaseError::StorageFull) => {
                observed_full = true;
                break;
            }
            Err(other) => panic!("unexpected error: {other:?}"),
        }
    }
    assert!(observed_full, "real page limit must surface StorageFull");
}

// ---------------------------------------------------------------------------
// backup.rs: interrupted-install recovery and IO refusals
// ---------------------------------------------------------------------------

#[test]
fn garbage_install_journal_fails_closed() {
    let directory = TestDir::new("garbage-journal");
    let path = directory.database();
    seed_single_file_db(&path, b"value");
    fs::write(install_path(&path, "journal"), b"not-a-phase\n").expect("write garbage marker");
    assert!(matches!(
        RiotDatabase::open(&path, DatabaseConfig::default()),
        Err(DatabaseError::CorruptDatabase)
    ));
}

#[test]
fn unreadable_install_journal_surfaces_storage_io() {
    let directory = TestDir::new("unreadable-journal");
    let path = directory.database();
    seed_single_file_db(&path, b"value");
    // A directory where the journal file is expected makes read_to_string fail
    // with a non-NotFound error.
    fs::create_dir(install_path(&path, "journal")).expect("journal directory");
    assert!(matches!(
        RiotDatabase::open(&path, DatabaseConfig::default()),
        Err(DatabaseError::StorageIo)
    ));
}

#[test]
fn installed_phase_recovers_the_new_family_when_destination_is_absent() {
    let directory = TestDir::new("installed-new");
    let path = directory.database();
    // Build the prepared replacement in the "new" install slot.
    let staging = directory.path("staging.sqlite");
    seed_single_file_db(&staging, b"from-new");
    move_family(&staging, &install_path(&path, "new"));
    fs::write(install_path(&path, "journal"), b"installed\n").expect("installed marker");
    assert!(!path.exists());

    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("recover new");
    assert_eq!(
        database.local_state("value").expect("recovered value"),
        Some(b"from-new".to_vec())
    );
    assert!(!install_path(&path, "new").exists());
    assert!(!install_path(&path, "journal").exists());
}

#[test]
fn installed_phase_recovers_the_old_family_when_new_is_missing() {
    let directory = TestDir::new("installed-old");
    let path = directory.database();
    let staging = directory.path("staging.sqlite");
    seed_single_file_db(&staging, b"from-old");
    move_family(&staging, &install_path(&path, "old"));
    fs::write(install_path(&path, "journal"), b"installed\n").expect("installed marker");
    assert!(!path.exists());

    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("recover old");
    assert_eq!(
        database.local_state("value").expect("recovered value"),
        Some(b"from-old".to_vec())
    );
    assert!(!install_path(&path, "old").exists());
    assert!(!install_path(&path, "journal").exists());
}

#[test]
fn installed_phase_with_no_recoverable_family_fails_closed() {
    let directory = TestDir::new("installed-none");
    let path = directory.database();
    fs::write(install_path(&path, "journal"), b"installed\n").expect("installed marker");
    assert!(!path.exists());
    assert!(matches!(
        RiotDatabase::open(&path, DatabaseConfig::default()),
        Err(DatabaseError::StorageIo)
    ));
}

#[test]
fn crash_before_prepared_marker_recovers_the_old_family() {
    let directory = TestDir::new("no-journal-old");
    let path = directory.database();
    let staging = directory.path("staging.sqlite");
    seed_single_file_db(&staging, b"pre-prepared-old");
    move_family(&staging, &install_path(&path, "old"));
    // No journal at all, destination absent, only the old family survives.
    assert!(!install_path(&path, "journal").exists());
    assert!(!path.exists());

    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("recover old");
    assert_eq!(
        database.local_state("value").expect("recovered value"),
        Some(b"pre-prepared-old".to_vec())
    );
}

#[test]
fn restore_from_missing_source_fails_closed() {
    let directory = TestDir::new("restore-missing-source");
    let source_path = directory.path("source.sqlite");
    let backup_path = directory.path("source.backup.sqlite");
    let source = RiotDatabase::open(&source_path, DatabaseConfig::default()).expect("source");
    source.set_local_state("value", b"backup").expect("seed");
    let manifest = source.backup_to(&backup_path).expect("backup");
    drop(source);

    let destination = directory.path("destination.sqlite");
    let missing_source = directory.path("does-not-exist.sqlite");
    assert!(matches!(
        RiotDatabase::restore_from(
            &destination,
            &missing_source,
            &manifest,
            DatabaseConfig::default()
        ),
        Err(DatabaseError::StorageIo)
    ));
}

#[test]
fn restore_over_a_pending_installed_destination_is_retryable() {
    let directory = TestDir::new("restore-pending-install");
    let source_path = directory.path("source.sqlite");
    let backup_path = directory.path("source.backup.sqlite");
    let source = RiotDatabase::open(&source_path, DatabaseConfig::default()).expect("source");
    source.set_local_state("value", b"backup").expect("seed");
    let manifest = source.backup_to(&backup_path).expect("backup");
    drop(source);

    let destination = directory.path("destination.sqlite");
    seed_single_file_db(&destination, b"live");
    // A durable "installed" marker means a prior install is still pending; a
    // restore must not race it.
    fs::write(install_path(&destination, "journal"), b"installed\n").expect("installed marker");
    assert!(matches!(
        RiotDatabase::restore_from(
            &destination,
            &backup_path,
            &manifest,
            DatabaseConfig::default()
        ),
        Err(DatabaseError::BusyRetryable)
    ));
}

#[test]
fn restore_surfaces_storage_io_when_the_replacement_slot_is_unremovable() {
    let directory = TestDir::new("restore-unremovable-new");
    let source_path = directory.path("source.sqlite");
    let backup_path = directory.path("source.backup.sqlite");
    let source = RiotDatabase::open(&source_path, DatabaseConfig::default()).expect("source");
    source.set_local_state("value", b"backup").expect("seed");
    let manifest = source.backup_to(&backup_path).expect("backup");
    drop(source);

    let destination = directory.path("destination.sqlite");
    seed_single_file_db(&destination, b"live");
    // A non-empty directory in the "new" install slot cannot be removed with
    // remove_file, forcing the replacement preparation to fail closed.
    let new_slot = install_path(&destination, "new");
    fs::create_dir(&new_slot).expect("new slot directory");
    fs::write(new_slot.join("occupant"), b"x").expect("occupy directory");
    assert!(matches!(
        RiotDatabase::restore_from(
            &destination,
            &backup_path,
            &manifest,
            DatabaseConfig::default()
        ),
        Err(DatabaseError::StorageIo)
    ));
    // The live destination survives the failed restore.
    let _ = fs::remove_dir_all(&new_slot);
    let database = RiotDatabase::open(&destination, DatabaseConfig::default()).expect("reopen");
    assert_eq!(
        database.local_state("value").expect("value"),
        Some(b"live".to_vec())
    );
}

// ---------------------------------------------------------------------------
// evidence.rs: load-path re-audit fails closed on a tampered ledger
// ---------------------------------------------------------------------------

fn make_one_entry_db(label: &str) -> (TestDir, PathBuf, [u8; 32], SignedWillowEntry) {
    let directory = TestDir::new(label);
    let path = directory.database();
    let app_id = [11; 32];
    let author = author();
    let signed = signed_app(&author, app_id, "items/a", 1, b"payload");
    let (session, store) = open_store(&path);
    commit(&store, &signed);
    drop(store);
    drop(session);
    (directory, path, app_id, signed)
}

#[test]
fn reopen_rejects_an_entry_that_is_both_live_and_forgotten() {
    let (_directory, path, _app, _signed) = make_one_entry_db("live-and-forgotten");
    let raw = Connection::open(&path).expect("tamper");
    raw.execute_batch("PRAGMA foreign_keys = OFF").expect("off");
    // Insert a forgotten marker for the entry that is still live: the ledger
    // now claims one entry is simultaneously live and forgotten.
    raw.execute_batch(
        "INSERT INTO forget_events(namespace_id, entry_id, forgotten_generation, restored_generation)
             SELECT namespace_id, entry_id, 1, NULL FROM accepted_entries;
         INSERT INTO forgotten_entries(namespace_id, entry_id, forgotten_generation)
             SELECT namespace_id, entry_id, 1 FROM accepted_entries;",
    )
    .expect("forge live+forgotten");
    drop(raw);
    assert_reopen_fails_closed(&path);
}

#[test]
fn reopen_rejects_a_forget_event_stamped_on_a_receipt_generation() {
    let (_directory, path, _app, _signed) = make_one_entry_db("forget-on-receipt-generation");
    let raw = Connection::open(&path).expect("tamper");
    raw.execute_batch("PRAGMA foreign_keys = OFF").expect("off");
    // Forge a forget event whose forgotten_generation equals the after
    // generation of receipt #1 (generation 1) — an immutable forget can never
    // share a generation with a receipt.
    raw.execute_batch(
        "INSERT INTO forget_events(namespace_id, entry_id, forgotten_generation, restored_generation)
             SELECT namespace_id, entry_id, 1, NULL FROM accepted_entries;",
    )
    .expect("forge forget-at-receipt");
    drop(raw);
    assert_reopen_fails_closed(&path);
}

fn make_reference_db(label: &str) -> (TestDir, PathBuf) {
    let directory = TestDir::new(label);
    let path = directory.database();
    let author = author();
    let older = signed_app(&author, [13; 32], "items/a", 1, b"older");
    let newer = signed_app(&author, [13; 32], "items/a", 2, b"newer");
    let (session, store) = open_store(&path);
    commit(&store, &older);
    let mixed = encode_bundle(&[older, newer]).expect("duplicate plus pruner");
    store
        .inspect(&mixed, ImportContext::new("references"))
        .expect("inspect")
        .expect_preview()
        .plan_all()
        .expect("plan")
        .commit()
        .expect("commit");
    drop(store);
    drop(session);
    (directory, path)
}

#[test]
fn reopen_rejects_a_disposition_with_a_duplicated_reference() {
    let (_directory, path) = make_reference_db("duplicate-reference");
    let raw = Connection::open(&path).expect("tamper");
    raw.execute_batch("PRAGMA foreign_keys = OFF").expect("off");
    // Duplicate the single reference row under a new reference_position, so one
    // pruned id appears twice inside one disposition.
    raw.execute_batch(
        "INSERT INTO import_references(
                namespace_id, receipt_id, disposition_position, reference_position, entry_id
             )
             SELECT namespace_id, receipt_id, disposition_position, reference_position + 1, entry_id
             FROM import_references;",
    )
    .expect("forge duplicate reference");
    drop(raw);
    assert_reopen_fails_closed(&path);
}

#[test]
fn reopen_rejects_a_reference_position_that_is_not_contiguous_from_zero() {
    let (_directory, path) = make_reference_db("noncontiguous-reference-position");
    let raw = Connection::open(&path).expect("tamper");
    raw.execute_batch("PRAGMA foreign_keys = OFF").expect("off");
    raw.execute("UPDATE import_references SET reference_position = 5", [])
        .expect("forge reference gap");
    drop(raw);
    assert_reopen_fails_closed(&path);
}

fn make_same_batch_dominated_db(label: &str) -> (TestDir, PathBuf) {
    let directory = TestDir::new(label);
    let path = directory.database();
    let author = author();
    let newer = signed_app(&author, [14; 32], "items/a", 2, b"newer");
    let older = signed_app(&author, [14; 32], "items/a", 1, b"older");
    let (session, store) = open_store(&path);
    let batch = encode_bundle(&[newer, older]).expect("winner and dominated");
    store
        .inspect(&batch, ImportContext::new("same-batch"))
        .expect("inspect")
        .expect_preview()
        .plan_all()
        .expect("plan")
        .commit()
        .expect("commit");
    drop(store);
    drop(session);
    (directory, path)
}

#[test]
fn reopen_rejects_a_dominated_disposition_missing_its_dominated_flag() {
    let (_directory, path) = make_same_batch_dominated_db("dominated-flag-cleared");
    let raw = Connection::open(&path).expect("tamper");
    raw.execute_batch("PRAGMA foreign_keys = OFF").expect("off");
    // The dominated-on-arrival entry is recorded with kind = 1 (DominatedAtCommit)
    // but its accepted row now claims it was never dominated.
    raw.execute(
        "UPDATE accepted_entries SET dominated_on_arrival = 0
         WHERE entry_id IN (SELECT entry_id FROM import_dispositions WHERE kind = 1)",
        [],
    )
    .expect("clear dominated flag");
    drop(raw);
    assert_reopen_fails_closed(&path);
}

#[test]
fn reopen_rejects_a_forget_event_restored_beyond_the_current_generation() {
    let directory = TestDir::new("restore-beyond-generation");
    let path = directory.database();
    let author = author();
    let entry = signed_app(&author, [15; 32], "items/a", 1, b"payload");
    let entry_id = riot_core::willow::entry_id(&entry.entry_bytes);
    let (session, store) = open_store(&path);
    commit(&store, &entry);
    store.forget_entry(&entry_id).expect("forget");
    commit(&store, &entry); // restore
    drop(store);
    drop(session);

    let raw = Connection::open(&path).expect("tamper");
    raw.execute_batch("PRAGMA foreign_keys = OFF").expect("off");
    // Push the restoration generation past the ledger's current generation.
    raw.execute("UPDATE forget_events SET restored_generation = 9999", [])
        .expect("forge impossible restoration");
    drop(raw);
    assert_reopen_fails_closed(&path);
}

#[test]
fn reopen_rejects_an_active_forgotten_marker_past_the_current_generation() {
    let directory = TestDir::new("forgotten-past-generation");
    let path = directory.database();
    let author = author();
    let entry = signed_app(&author, [16; 32], "items/a", 1, b"payload");
    let entry_id = riot_core::willow::entry_id(&entry.entry_bytes);
    let (session, store) = open_store(&path);
    commit(&store, &entry);
    store.forget_entry(&entry_id).expect("forget");
    drop(store);
    drop(session);

    let raw = Connection::open(&path).expect("tamper");
    raw.execute_batch("PRAGMA foreign_keys = OFF").expect("off");
    // The active forgotten marker now claims a generation that never occurred.
    raw.execute(
        "UPDATE forgotten_entries SET forgotten_generation = 9999",
        [],
    )
    .expect("forge impossible active marker");
    drop(raw);
    assert_reopen_fails_closed(&path);
}

#[test]
fn reopen_rejects_a_live_app_entry_whose_required_payload_was_dropped() {
    let (_directory, path, _app, _signed) = make_one_entry_db("dropped-required-payload");
    let raw = Connection::open(&path).expect("tamper");
    raw.execute_batch("PRAGMA foreign_keys = OFF").expect("off");
    // App-data entries must retain their payload; NULL it out while leaving the
    // accepted entry (and its digest) intact.
    raw.execute("UPDATE live_entries SET payload = NULL", [])
        .expect("drop payload");
    drop(raw);
    assert_reopen_fails_closed(&path);
}

#[test]
fn reopen_rejects_a_live_projection_that_lost_an_expected_row() {
    let directory = TestDir::new("missing-live-row");
    let path = directory.database();
    let author = author();
    let first = signed_app(&author, [17; 32], "items/a", 1, b"a");
    let second = signed_app(&author, [17; 32], "items/b", 1, b"b");
    let (session, store) = open_store(&path);
    commit(&store, &first);
    commit(&store, &second);
    drop(store);
    drop(session);

    let raw = Connection::open(&path).expect("tamper");
    raw.execute_batch("PRAGMA foreign_keys = OFF").expect("off");
    // Delete one live row (and its prefixes) that the receipt replay expects.
    raw.execute_batch(
        "DELETE FROM entry_path_prefixes
             WHERE entry_id = (SELECT MIN(entry_id) FROM live_entries);
         DELETE FROM live_entries WHERE entry_id = (SELECT MIN(entry_id) FROM live_entries);",
    )
    .expect("drop a live row");
    drop(raw);
    assert_reopen_fails_closed(&path);
}

// ---------------------------------------------------------------------------
// evidence.rs: persist-path re-audit fails closed against tampered rows
// ---------------------------------------------------------------------------

#[test]
fn persist_rejects_reaccepting_an_entry_whose_stored_bytes_were_altered() {
    // forget then re-import re-admits the entry (mutation.accepted is non-empty).
    let directory = TestDir::new("persist-accepted-bytes");
    let path = directory.database();
    let author = author();
    let entry = signed_app(&author, [18; 32], "items/a", 1, b"payload");
    let entry_id = riot_core::willow::entry_id(&entry.entry_bytes);
    let (session, store) = open_store(&path);
    commit(&store, &entry);
    store.forget_entry(&entry_id).expect("forget");

    let raw = Connection::open(&path).expect("tamper");
    raw.execute_batch("PRAGMA foreign_keys = OFF").expect("off");
    raw.execute("UPDATE accepted_entries SET entry_bytes = X'00FF00FF'", [])
        .expect("alter stored entry bytes");
    drop(raw);

    // Re-importing the exact entry must detect that the stored bytes no longer
    // match the accepted identity.
    let outcome = store
        .inspect(
            &encode_bundle(std::slice::from_ref(&entry)).expect("bundle"),
            ImportContext::new("reaccept"),
        )
        .expect("inspect")
        .expect_preview()
        .plan_all()
        .expect("plan")
        .commit();
    assert_eq!(outcome, Err(SessionError::Internal));
    let _ = session;
}

#[test]
fn persist_rejects_a_live_row_disagreeing_with_its_accepted_entry() {
    let directory = TestDir::new("persist-live-bytes");
    let path = directory.database();
    let author = author();
    let a = signed_app(&author, [19; 32], "items/a", 1, b"a");
    let b = signed_app(&author, [19; 32], "items/b", 1, b"b");
    let a_id = riot_core::willow::entry_id(&a.entry_bytes);
    let b_id = riot_core::willow::entry_id(&b.entry_bytes);
    let (session, store) = open_store(&path);
    commit(&store, &a);
    commit(&store, &b);

    let raw = Connection::open(&path).expect("tamper");
    raw.execute_batch("PRAGMA foreign_keys = OFF").expect("off");
    // Alter A's accepted bytes; forgetting B rewrites the live projection for A
    // and re-reads A's now-inconsistent accepted bytes.
    raw.execute(
        "UPDATE accepted_entries SET entry_bytes = X'00FF00FF' WHERE entry_id = ?1",
        [a_id],
    )
    .expect("alter A bytes");
    drop(raw);

    assert_eq!(store.forget_entry(&b_id), Err(SessionError::Internal));
    let _ = session;
}

#[test]
fn persist_rejects_a_forget_when_the_ledger_holds_an_unexpected_marker() {
    let directory = TestDir::new("persist-unexpected-marker");
    let path = directory.database();
    let author = author();
    let a = signed_app(&author, [20; 32], "items/a", 1, b"a");
    let b = signed_app(&author, [20; 32], "items/b", 1, b"b");
    let a_id = riot_core::willow::entry_id(&a.entry_bytes);
    let (session, store) = open_store(&path);
    commit(&store, &a);
    commit(&store, &b);

    let raw = Connection::open(&path).expect("tamper");
    raw.execute_batch("PRAGMA foreign_keys = OFF").expect("off");
    // A stray active forgotten marker for the still-live B: the in-memory join
    // does not consider B forgotten, so the DB is not a subset of the desired
    // forgotten set.
    raw.execute_batch(
        "INSERT INTO forgotten_entries(namespace_id, entry_id, forgotten_generation)
             SELECT namespace_id, entry_id, 1 FROM accepted_entries
             WHERE entry_id = (SELECT MAX(entry_id) FROM live_entries);",
    )
    .expect("forge stray marker");
    drop(raw);

    assert_eq!(store.forget_entry(&a_id), Err(SessionError::Internal));
    let _ = session;
}

#[test]
fn persist_rejects_a_forget_when_an_entry_id_spans_two_namespaces() {
    let (_directory, path, _app, _signed) = make_one_entry_db("persist-two-namespaces");
    let (session, store) = open_store(&path);
    let entry_id = store.live_entry_ids().expect("live")[0];

    let raw = Connection::open(&path).expect("tamper");
    raw.execute_batch("PRAGMA foreign_keys = OFF").expect("off");
    // Duplicate the accepted row under a second namespace_id, so the single
    // newly-forgotten entry id resolves to two namespaces.
    raw.execute_batch(
        "INSERT INTO accepted_entries(
                namespace_id, entry_id, subspace_id, path_bytes, timestamp_be,
                payload_digest, payload_length, entry_bytes, capability_bytes,
                signature_bytes, first_receipt_id, dominated_on_arrival
             )
             SELECT X'00000000000000000000000000000000000000000000000000000000000000FF',
                    entry_id, subspace_id, path_bytes, timestamp_be, payload_digest,
                    payload_length, entry_bytes, capability_bytes, signature_bytes,
                    first_receipt_id, dominated_on_arrival
             FROM accepted_entries;",
    )
    .expect("forge second namespace row");
    drop(raw);

    assert_eq!(store.forget_entry(&entry_id), Err(SessionError::Internal));
    let _ = session;
}

#[test]
fn persist_rejects_a_commit_whose_desired_forgets_are_not_on_disk() {
    let directory = TestDir::new("persist-desired-not-subset");
    let path = directory.database();
    let author = author();
    let a = signed_app(&author, [21; 32], "items/a", 1, b"a");
    let b = signed_app(&author, [21; 32], "items/b", 1, b"b");
    let a_id = riot_core::willow::entry_id(&a.entry_bytes);
    let b_id = riot_core::willow::entry_id(&b.entry_bytes);
    let (session, store) = open_store(&path);
    commit(&store, &a);
    commit(&store, &b);
    store.forget_entry(&a_id).expect("forget a");
    store.forget_entry(&b_id).expect("forget b");

    let raw = Connection::open(&path).expect("tamper");
    raw.execute_batch("PRAGMA foreign_keys = OFF").expect("off");
    // Remove B's active marker from disk while the join still considers B
    // forgotten. Restoring A (a receipt-bearing commit) carries B as a desired
    // forget that no longer exists on disk.
    raw.execute("DELETE FROM forgotten_entries WHERE entry_id = ?1", [b_id])
        .expect("drop b marker");
    drop(raw);

    let outcome = store
        .inspect(
            &encode_bundle(std::slice::from_ref(&a)).expect("bundle"),
            ImportContext::new("restore-a"),
        )
        .expect("inspect")
        .expect_preview()
        .plan_all()
        .expect("plan")
        .commit();
    assert_eq!(outcome, Err(SessionError::Internal));
    let _ = session;
}

#[test]
fn persist_rejects_a_commit_facing_a_phantom_forgotten_entry() {
    let directory = TestDir::new("persist-phantom-forgotten");
    let path = directory.database();
    let author = author();
    let a = signed_app(&author, [22; 32], "items/a", 1, b"a");
    let b = signed_app(&author, [22; 32], "items/b", 1, b"b");
    let (session, store) = open_store(&path);
    commit(&store, &a);

    let raw = Connection::open(&path).expect("tamper");
    raw.execute_batch("PRAGMA foreign_keys = OFF").expect("off");
    // Plant an accepted+forgotten entry the in-memory join has never seen; a
    // later receipt-bearing commit must find it neither restored nor desired.
    raw.execute_batch(
        "INSERT INTO accepted_entries(
                namespace_id, entry_id, subspace_id, path_bytes, timestamp_be,
                payload_digest, payload_length, entry_bytes, capability_bytes,
                signature_bytes, first_receipt_id, dominated_on_arrival
             )
             SELECT namespace_id,
                    X'00000000000000000000000000000000000000000000000000000000000000AA',
                    subspace_id, path_bytes, timestamp_be, payload_digest, payload_length,
                    entry_bytes, capability_bytes, signature_bytes, first_receipt_id,
                    dominated_on_arrival
             FROM accepted_entries;
         INSERT INTO forgotten_entries(namespace_id, entry_id, forgotten_generation)
             SELECT namespace_id,
                    X'00000000000000000000000000000000000000000000000000000000000000AA',
                    1
             FROM accepted_entries
             WHERE entry_id = X'00000000000000000000000000000000000000000000000000000000000000AA';",
    )
    .expect("forge phantom forgotten entry");
    drop(raw);

    let outcome = store
        .inspect(
            &encode_bundle(std::slice::from_ref(&b)).expect("bundle"),
            ImportContext::new("phantom"),
        )
        .expect("inspect")
        .expect_preview()
        .plan_all()
        .expect("plan")
        .commit();
    assert_eq!(outcome, Err(SessionError::Internal));
    let _ = session;
}

#[test]
fn persist_rejects_a_restore_whose_open_forget_event_was_already_closed() {
    let directory = TestDir::new("persist-already-closed-event");
    let path = directory.database();
    let author = author();
    let entry = signed_app(&author, [23; 32], "items/a", 1, b"payload");
    let entry_id = riot_core::willow::entry_id(&entry.entry_bytes);
    let (session, store) = open_store(&path);
    commit(&store, &entry);
    store.forget_entry(&entry_id).expect("forget");

    let raw = Connection::open(&path).expect("tamper");
    raw.execute_batch("PRAGMA foreign_keys = OFF").expect("off");
    // Pre-close the open forget event; the restoring commit's UPDATE ... WHERE
    // restored_generation IS NULL then affects zero rows.
    raw.execute(
        "UPDATE forget_events SET restored_generation = forgotten_generation + 1",
        [],
    )
    .expect("pre-close event");
    drop(raw);

    let outcome = store
        .inspect(
            &encode_bundle(std::slice::from_ref(&entry)).expect("bundle"),
            ImportContext::new("restore"),
        )
        .expect("inspect")
        .expect_preview()
        .plan_all()
        .expect("plan")
        .commit();
    assert_eq!(outcome, Err(SessionError::Internal));
    let _ = session;
}

#[test]
fn commit_surfaces_store_full_when_the_page_limit_is_reached() {
    let directory = TestDir::new("commit-store-full");
    let path = directory.database();
    let config = DatabaseConfig::default().with_max_page_count(20);
    let database = RiotDatabase::open(&path, config).expect("open");
    let session = RiotSession::open_sqlite(database).expect("session");
    let store = session.create_store().expect("store");
    let author = author();

    let mut observed_full = false;
    for index in 0..64 {
        let entry = signed_app(
            &author,
            [24; 32],
            &format!("items/{index}"),
            1,
            &vec![7; 4096],
        );
        match store
            .inspect(
                &encode_bundle(std::slice::from_ref(&entry)).expect("bundle"),
                ImportContext::new("fill"),
            )
            .expect("inspect")
            .expect_preview()
            .plan_all()
            .expect("plan")
            .commit()
        {
            Ok(_) => {}
            Err(SessionError::StoreFull) => {
                observed_full = true;
                break;
            }
            Err(other) => panic!("unexpected commit error: {other:?}"),
        }
    }
    assert!(observed_full, "page limit must surface StoreFull");
}
