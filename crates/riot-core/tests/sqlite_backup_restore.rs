use riot_core::store::{DatabaseConfig, DatabaseError, RiotDatabase};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(1);

struct TestDir(PathBuf);

impl TestDir {
    fn new(label: &str) -> Self {
        let sequence = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "riot-backup-{label}-{}-{sequence}",
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

#[test]
fn online_backup_restores_a_consistent_generation_and_quarantines_authority() {
    let directory = TestDir::new("roundtrip");
    let live_path = directory.path("riot.sqlite");
    let backup_path = directory.path("riot.backup.sqlite");
    let config = DatabaseConfig::default();

    let database = RiotDatabase::open(&live_path, config.clone()).expect("open live database");
    database
        .set_local_state("document", b"backed-up-value")
        .expect("seed backup value");
    let manifest = database.backup_to(&backup_path).expect("online backup");
    assert_eq!(
        manifest.database_id(),
        database.database_id().expect("database id")
    );
    assert_eq!(
        manifest.database_generation(),
        database.database_generation().expect("database generation")
    );
    assert_eq!(manifest.generation(), 1);
    assert_eq!(
        manifest.schema_version(),
        database.schema_version().expect("schema version")
    );

    database
        .set_local_state("document", b"newer-live-value")
        .expect("mutate after backup");
    drop(database);

    let restored = RiotDatabase::restore_from(&live_path, &backup_path, &manifest, config)
        .expect("restore backup");
    assert_eq!(
        restored.local_state("document").expect("restored document"),
        Some(b"backed-up-value".to_vec())
    );
    assert_eq!(
        restored.database_id().expect("restored id"),
        manifest.database_id()
    );
    assert_ne!(
        restored
            .database_generation()
            .expect("restored database generation"),
        manifest.database_generation()
    );
    assert!(restored.generation().expect("restored generation") > manifest.generation());
    assert!(restored.authority_quarantined().expect("quarantine state"));
    assert!(restored.integrity_check().expect("restored integrity"));
}

#[test]
fn restore_requires_exclusive_ownership_and_never_replaces_under_old_handles() {
    let directory = TestDir::new("exclusive-restore");
    let live_path = directory.path("riot.sqlite");
    let source_path = directory.path("source.sqlite");
    let backup_path = directory.path("source.backup.sqlite");
    let config = DatabaseConfig::default();

    let live = RiotDatabase::open(&live_path, config.clone()).expect("live owner");
    live.set_local_state("value", b"live").expect("seed live");
    let old_handle = live.clone();
    let source = RiotDatabase::open(&source_path, config.clone()).expect("source");
    source
        .set_local_state("value", b"backup")
        .expect("seed source");
    let manifest = source.backup_to(&backup_path).expect("backup source");
    drop(source);

    let error = RiotDatabase::restore_from(&live_path, &backup_path, &manifest, config.clone())
        .expect_err("restore cannot race a live writable owner");
    assert!(matches!(error, DatabaseError::BusyRetryable));
    old_handle
        .set_local_state("still-old", b"safe")
        .expect("old owner remains valid because restore did not happen");
    drop(old_handle);
    drop(live);

    let restored = RiotDatabase::restore_from(&live_path, &backup_path, &manifest, config)
        .expect("restore after every old handle closes");
    assert_eq!(
        restored.local_state("value").expect("restored value"),
        Some(b"backup".to_vec())
    );
    assert_eq!(
        restored.local_state("still-old").expect("old-only value"),
        None
    );
}

#[test]
fn read_snapshot_retains_restore_exclusion_after_all_database_handles_drop() {
    let directory = TestDir::new("snapshot-ownership");
    let live_path = directory.path("riot.sqlite");
    let backup_path = directory.path("riot.backup.sqlite");
    let config = DatabaseConfig::default().with_reader_pool_size(1);
    let database = RiotDatabase::open(&live_path, config.clone()).expect("live database");
    database
        .set_local_state("value", b"snapshot-value")
        .expect("seed snapshot");
    let manifest = database
        .backup_to(&backup_path)
        .expect("backup live database");
    let snapshot = database.read_snapshot().expect("open snapshot");
    assert_eq!(
        snapshot.local_state("value").expect("establish snapshot"),
        Some(b"snapshot-value".to_vec())
    );
    drop(database);

    let error = RiotDatabase::restore_from(&live_path, &backup_path, &manifest, config.clone())
        .expect_err("snapshot must retain the database path lease");
    assert!(matches!(error, DatabaseError::BusyRetryable));
    assert_eq!(
        snapshot
            .local_state("value")
            .expect("snapshot remains usable"),
        Some(b"snapshot-value".to_vec())
    );

    drop(snapshot);
    RiotDatabase::restore_from(&live_path, &backup_path, &manifest, config)
        .expect("restore after snapshot drops");
}

#[test]
fn invalid_restore_config_does_not_mutate_any_destination_family_member() {
    let directory = TestDir::new("invalid-restore-config");
    let source_path = directory.path("source.sqlite");
    let backup_path = directory.path("source.backup.sqlite");
    let source = RiotDatabase::open(&source_path, DatabaseConfig::default()).expect("source");
    source
        .set_local_state("value", b"backup")
        .expect("seed source");
    let manifest = source.backup_to(&backup_path).expect("backup source");
    drop(source);

    let invalid_configs = [
        DatabaseConfig::default().with_reader_pool_size(0),
        DatabaseConfig::default().with_checkpoint_pages(8, 8),
    ];
    for (index, invalid_config) in invalid_configs.into_iter().enumerate() {
        let destination = directory.path(&format!("destination-{index}.sqlite"));
        let live = RiotDatabase::open(&destination, DatabaseConfig::default()).expect("live");
        live.set_local_state("value", b"live").expect("seed live");
        drop(live);
        fs::write(wal_path(&destination), b"preserved-wal").expect("write WAL sentinel");
        fs::write(shm_path(&destination), b"preserved-shm").expect("write SHM sentinel");
        let before = family_bytes(&destination);

        let error =
            RiotDatabase::restore_from(&destination, &backup_path, &manifest, invalid_config)
                .expect_err("invalid config must fail before restore starts");
        assert!(matches!(error, DatabaseError::InvalidInput));
        assert_eq!(family_bytes(&destination), before);

        fs::remove_file(wal_path(&destination)).expect("remove WAL sentinel");
        fs::remove_file(shm_path(&destination)).expect("remove SHM sentinel");
        let reopened = RiotDatabase::open(&destination, DatabaseConfig::default())
            .expect("reopen unchanged destination");
        assert_eq!(
            reopened.local_state("value").expect("live value"),
            Some(b"live".to_vec())
        );
    }
}

#[test]
fn restore_ignores_unmanifested_source_wal_and_removes_stale_destination_family() {
    let directory = TestDir::new("wal-family");
    let live_path = directory.path("riot.sqlite");
    let source_path = directory.path("source.sqlite");
    let backup_path = directory.path("source.backup.sqlite");
    let stale_path = directory.path("stale.sqlite");
    let config = DatabaseConfig::default();

    let live = RiotDatabase::open(&live_path, config.clone()).expect("live database");
    live.set_local_state("value", b"live").expect("seed live");
    drop(live);
    let source = RiotDatabase::open(&source_path, config.clone()).expect("source database");
    source
        .set_local_state("value", b"manifested")
        .expect("seed source");
    let manifest = source.backup_to(&backup_path).expect("manifested backup");
    drop(source);

    let backup_main_before = fs::read(&backup_path).expect("backup main bytes");
    let unmanifested = rusqlite::Connection::open(&backup_path).expect("mutate backup WAL");
    unmanifested
        .execute_batch(
            "PRAGMA journal_mode = WAL;
             UPDATE local_state SET value = x'756e6d616e69666573746564' WHERE key = 'value';
             UPDATE database_meta SET generation = generation + 1 WHERE singleton = 1;",
        )
        .expect("write only into unmanifested WAL");
    assert_eq!(
        fs::read(&backup_path).expect("backup main bytes after WAL write"),
        backup_main_before,
        "test requires the unmanifested mutation to remain outside the main file"
    );

    let stale = rusqlite::Connection::open(&stale_path).expect("stale database");
    stale
        .execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE stale(value BLOB);
             INSERT INTO stale(value) VALUES (zeroblob(8192));",
        )
        .expect("create real stale WAL family");
    fs::copy(wal_path(&stale_path), wal_path(&live_path)).expect("copy real stale WAL");
    fs::copy(shm_path(&stale_path), shm_path(&live_path)).expect("copy real stale SHM");

    let restored = RiotDatabase::restore_from(&live_path, &backup_path, &manifest, config)
        .expect("restore main-file snapshot only");
    assert_eq!(
        restored.local_state("value").expect("manifested value"),
        Some(b"manifested".to_vec())
    );
    assert!(
        !wal_path(&live_path).exists() || fs::metadata(wal_path(&live_path)).unwrap().len() == 0
    );
    drop(unmanifested);
    drop(stale);
}

#[test]
fn writable_open_recovers_a_real_prepared_install_after_interruption() {
    let directory = TestDir::new("install-recovery");
    let destination = directory.path("riot.sqlite");
    let replacement = directory.path("replacement.sqlite");
    let config = DatabaseConfig::default();

    let original = RiotDatabase::open(&destination, config.clone()).expect("original database");
    original
        .set_local_state("value", b"original")
        .expect("seed original");
    drop(original);

    // Capture a real database family whose newest committed value lives only
    // in WAL, then recreate that crash image after the connection closes.
    let wal_writer = rusqlite::Connection::open(&destination).expect("WAL writer");
    wal_writer
        .execute_batch(
            "UPDATE local_state SET value = x'77616c2d6f726967696e616c' WHERE key = 'value';
             UPDATE database_meta SET generation = generation + 1 WHERE singleton = 1;",
        )
        .expect("commit value to WAL");
    let captured_main = directory.path("captured-main");
    let captured_wal = directory.path("captured-wal");
    let captured_shm = directory.path("captured-shm");
    fs::copy(&destination, &captured_main).expect("capture main");
    fs::copy(wal_path(&destination), &captured_wal).expect("capture WAL");
    fs::copy(shm_path(&destination), &captured_shm).expect("capture SHM");
    drop(wal_writer);
    let _ = fs::remove_file(wal_path(&destination));
    let _ = fs::remove_file(shm_path(&destination));
    fs::copy(&captured_main, &destination).expect("restore captured main");
    fs::copy(&captured_wal, wal_path(&destination)).expect("restore captured WAL");
    fs::copy(&captured_shm, shm_path(&destination)).expect("restore captured SHM");

    let new_database = RiotDatabase::open(&replacement, config.clone()).expect("replacement");
    new_database
        .set_local_state("value", b"replacement")
        .expect("seed replacement");
    drop(new_database);

    let old_main = install_path(&destination, "old");
    let install_new = install_path(&destination, "new");
    let journal = install_path(&destination, "journal");
    fs::rename(&destination, &old_main).expect("crash phase moved old main");
    fs::copy(&replacement, &install_new).expect("crash phase retained prepared replacement");
    fs::write(&journal, b"prepared\n").expect("durable prepared marker");
    assert!(!destination.exists());

    let recovered = RiotDatabase::open(&destination, config).expect("recover interrupted install");
    assert_eq!(
        recovered.local_state("value").expect("recovered value"),
        Some(b"wal-original".to_vec())
    );
    assert!(!old_main.exists());
    assert!(!install_new.exists());
    assert!(!journal.exists());
}

#[test]
fn manifest_mismatch_or_corrupt_backup_preserves_the_existing_destination() {
    let directory = TestDir::new("preserve");
    let destination_path = directory.path("destination.sqlite");
    let first_path = directory.path("first.sqlite");
    let second_path = directory.path("second.sqlite");
    let first_backup = directory.path("first.backup.sqlite");
    let second_backup = directory.path("second.backup.sqlite");
    let config = DatabaseConfig::default();

    let destination = RiotDatabase::open(&destination_path, config.clone()).expect("destination");
    destination
        .set_local_state("preserved", b"destination-value")
        .expect("seed destination");
    drop(destination);

    let first = RiotDatabase::open(&first_path, config.clone()).expect("first source");
    first
        .set_local_state("source", b"first")
        .expect("seed first");
    let first_manifest = first.backup_to(&first_backup).expect("first backup");
    drop(first);

    let second = RiotDatabase::open(&second_path, config.clone()).expect("second source");
    second
        .set_local_state("source", b"second")
        .expect("seed second");
    let second_manifest = second.backup_to(&second_backup).expect("second backup");
    drop(second);

    let error = RiotDatabase::restore_from(
        &destination_path,
        &first_backup,
        &second_manifest,
        config.clone(),
    )
    .expect_err("mismatched manifest must fail");
    assert!(matches!(error, DatabaseError::BackupMismatch));
    assert_destination_preserved(&destination_path, config.clone());

    let mut corrupt = fs::read(&first_backup).expect("read backup");
    let middle = corrupt.len() / 2;
    corrupt[middle] ^= 0xff;
    fs::write(&first_backup, corrupt).expect("corrupt backup");
    let error = RiotDatabase::restore_from(
        &destination_path,
        &first_backup,
        &first_manifest,
        config.clone(),
    )
    .expect_err("corrupt backup must fail before replacement");
    assert!(matches!(
        error,
        DatabaseError::BackupMismatch | DatabaseError::CorruptDatabase
    ));
    assert_destination_preserved(&destination_path, config);
    assert_no_restore_temporary_files(&directory.0);
}

#[test]
fn a_failed_backup_does_not_replace_an_existing_backup() {
    let directory = TestDir::new("backup-failure");
    let live_path = directory.path("riot.sqlite");
    let backup_path = directory.path("riot.backup.sqlite");
    let config = DatabaseConfig::default();
    let database = RiotDatabase::open(&live_path, config).expect("open database");
    database
        .set_local_state("value", b"one")
        .expect("seed database");
    let first_manifest = database.backup_to(&backup_path).expect("first backup");
    let first_bytes = fs::read(&backup_path).expect("first backup bytes");

    let protected_path = directory.path("protected.sqlite");
    let protected = RiotDatabase::open(&protected_path, DatabaseConfig::default())
        .expect("open protected destination");
    protected
        .set_local_state("protected", b"unchanged")
        .expect("seed protected destination");
    let error = database
        .backup_to(&protected_path)
        .expect_err("backup cannot replace another live database path");
    assert!(matches!(error, DatabaseError::BusyRetryable));
    assert_eq!(
        protected.local_state("protected").expect("protected value"),
        Some(b"unchanged".to_vec())
    );
    drop(protected);

    fs::create_dir(directory.path("blocked-parent")).expect("blocked directory");
    let impossible_destination = directory.path("blocked-parent");
    let error = database
        .backup_to(&impossible_destination)
        .expect_err("directory cannot be replaced by backup file");
    assert!(matches!(error, DatabaseError::StorageIo));

    assert_eq!(
        fs::read(&backup_path).expect("backup preserved"),
        first_bytes
    );
    let reopened = RiotDatabase::restore_from(
        directory.path("restored.sqlite"),
        &backup_path,
        &first_manifest,
        DatabaseConfig::default(),
    )
    .expect("original backup remains restorable");
    assert_eq!(
        reopened.local_state("value").expect("restored state"),
        Some(b"one".to_vec())
    );
}

fn assert_destination_preserved(path: &std::path::Path, config: DatabaseConfig) {
    let database = RiotDatabase::open(path, config).expect("reopen preserved destination");
    assert_eq!(
        database.local_state("preserved").expect("preserved value"),
        Some(b"destination-value".to_vec())
    );
    assert!(!database.authority_quarantined().expect("quarantine state"));
}

fn assert_no_restore_temporary_files(directory: &std::path::Path) {
    let leftovers: Vec<_> = fs::read_dir(directory)
        .expect("list test directory")
        .map(|entry| entry.expect("directory entry").file_name())
        .filter(|name| {
            let name = name.to_string_lossy();
            name.contains("restore-source") || name.contains("install-")
        })
        .collect();
    assert!(
        leftovers.is_empty(),
        "restore temporary files remain: {leftovers:?}"
    );
}

fn wal_path(path: &std::path::Path) -> PathBuf {
    PathBuf::from(format!("{}-wal", path.display()))
}

fn shm_path(path: &std::path::Path) -> PathBuf {
    PathBuf::from(format!("{}-shm", path.display()))
}

fn install_path(path: &std::path::Path, suffix: &str) -> PathBuf {
    let parent = path.parent().expect("database parent");
    let name = path.file_name().expect("database name").to_string_lossy();
    parent.join(format!(".{name}.install-{suffix}"))
}

fn family_bytes(path: &std::path::Path) -> Vec<(PathBuf, Vec<u8>)> {
    [path.to_path_buf(), wal_path(path), shm_path(path)]
        .into_iter()
        .map(|member| {
            let bytes = fs::read(&member).expect("read family member");
            (member, bytes)
        })
        .collect()
}
