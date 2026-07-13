use super::database::{
    ensure_integrity, generation_in, map_sqlite_error, DatabaseError, RiotDatabase,
};
use super::schema;
use rusqlite::{backup::Backup, Connection, OpenFlags, TransactionBehavior};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static NEXT_TEMPORARY_FILE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackupManifest {
    schema_version: u32,
    database_id: [u8; 16],
    database_generation: [u8; 16],
    generation: u64,
    file_sha256: [u8; 32],
}

impl BackupManifest {
    #[must_use]
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    #[must_use]
    pub fn database_id(&self) -> [u8; 16] {
        self.database_id
    }

    #[must_use]
    pub fn database_generation(&self) -> [u8; 16] {
        self.database_generation
    }

    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

pub(crate) fn create(
    database: &RiotDatabase,
    destination: &Path,
) -> Result<BackupManifest, DatabaseError> {
    if database.inner.read_only || destination == database.inner.path {
        return Err(DatabaseError::StorageReadOnly);
    }
    validate_destination(destination)?;
    let temporary = TemporaryFile::new(destination, "backup-tmp")?;
    let source = database.lock_writer()?;
    let mut target = Connection::open(temporary.path()).map_err(map_sqlite_error)?;
    {
        let backup = Backup::new(&source, &mut target).map_err(map_sqlite_error)?;
        backup
            .run_to_completion(64, Duration::from_millis(1), None)
            .map_err(map_sqlite_error)?;
    }
    ensure_integrity(&target)?;
    drop(target);
    sync_file(temporary.path())?;

    let (schema_version, database_id, database_generation, generation) =
        inspect_database(temporary.path())?;
    let manifest = BackupManifest {
        schema_version,
        database_id,
        database_generation,
        generation,
        file_sha256: sha256_file(temporary.path())?,
    };
    temporary.install(destination)?;
    Ok(manifest)
}

pub(crate) fn restore(
    destination: &Path,
    source: &Path,
    manifest: &BackupManifest,
) -> Result<(), DatabaseError> {
    validate_destination(destination)?;
    if !source.is_file() {
        return Err(DatabaseError::StorageIo);
    }
    if recover_install(destination)? {
        return Err(DatabaseError::BusyRetryable);
    }

    // Copy through one already-opened file identity. Any concurrent mutation
    // either produces the exact manifested bytes or fails the digest/integrity
    // checks; path replacement cannot redirect the copy midway.
    let source_snapshot = TemporaryFile::new(destination, "restore-source")?;
    copy_file_snapshot(source, source_snapshot.path())?;
    if sha256_file(source_snapshot.path())? != manifest.file_sha256 {
        return Err(DatabaseError::BackupMismatch);
    }
    if !manifest_matches(source_snapshot.path(), manifest)? {
        return Err(DatabaseError::BackupMismatch);
    }

    let replacement_path = install_path(destination, "new");
    remove_family(&replacement_path)?;
    let source_connection = Connection::open_with_flags(
        source_snapshot.path(),
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(map_sqlite_error)?;
    ensure_integrity(&source_connection)?;
    let mut replacement = Connection::open(&replacement_path).map_err(map_sqlite_error)?;
    {
        let backup = Backup::new(&source_connection, &mut replacement).map_err(map_sqlite_error)?;
        backup
            .run_to_completion(64, Duration::from_millis(1), None)
            .map_err(map_sqlite_error)?;
    }
    drop(source_connection);

    ensure_integrity(&replacement)?;
    let replacement_identity = inspect_connection(&replacement)?;
    if replacement_identity
        != (
            manifest.schema_version,
            manifest.database_id,
            manifest.database_generation,
            manifest.generation,
        )
    {
        return Err(DatabaseError::BackupMismatch);
    }
    let transaction = replacement
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_sqlite_error)?;
    transaction
        .execute(
            "UPDATE database_meta
             SET database_generation = randomblob(16),
                 generation = generation + 1,
                 authority_quarantined = 1
             WHERE singleton = 1",
            [],
        )
        .map_err(map_sqlite_error)?;
    transaction.commit().map_err(map_sqlite_error)?;
    ensure_integrity(&replacement)?;
    drop(replacement);
    sync_file(&replacement_path)?;
    install_replacement(destination)
}

/// Recovers an interrupted install. `true` means a replacement reached the
/// installed phase and its old family must be retained until the caller has
/// successfully reopened and validated the replacement.
pub(crate) fn recover_install(destination: &Path) -> Result<bool, DatabaseError> {
    let journal = install_path(destination, "journal");
    let old = install_path(destination, "old");
    let new = install_path(destination, "new");
    let phase = match fs::read_to_string(&journal) {
        Ok(value) => Some(value),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(_) => return Err(DatabaseError::StorageIo),
    };

    match phase.as_deref() {
        Some("prepared\n") => {
            if family_exists(&old) {
                move_family(&old, destination)?;
            }
            remove_family(&new)?;
            remove_family(&old)?;
            remove_if_exists(&journal)?;
            sync_parent(destination)?;
            Ok(false)
        }
        Some("installed\n") => {
            if !destination.exists() {
                if new.exists() {
                    move_family(&new, destination)?;
                } else if family_exists(&old) {
                    move_family(&old, destination)?;
                    remove_if_exists(&journal)?;
                    sync_parent(destination)?;
                    return Ok(false);
                } else {
                    return Err(DatabaseError::StorageIo);
                }
            }
            sync_file(destination)?;
            sync_parent(destination)?;
            Ok(true)
        }
        Some(_) => Err(DatabaseError::CorruptDatabase),
        None => {
            // A crash before the prepared marker can only leave the new file.
            remove_family(&new)?;
            // Be conservative if a journal unlink was durable but old cleanup
            // was not: preserve the installed destination and discard old.
            if destination.exists() {
                remove_family(&old)?;
            } else if family_exists(&old) {
                move_family(&old, destination)?;
            }
            sync_parent(destination)?;
            Ok(false)
        }
    }
}

pub(crate) fn finish_install(destination: &Path) -> Result<(), DatabaseError> {
    remove_family(&install_path(destination, "new"))?;
    remove_family(&install_path(destination, "old"))?;
    remove_if_exists(&install_path(destination, "journal"))?;
    sync_parent(destination)
}

fn inspect_database(path: &Path) -> Result<(u32, [u8; 16], [u8; 16], u64), DatabaseError> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(map_sqlite_error)?;
    inspect_connection(&connection)
}

fn inspect_connection(
    connection: &Connection,
) -> Result<(u32, [u8; 16], [u8; 16], u64), DatabaseError> {
    ensure_integrity(connection)?;
    let schema_version = schema::validate_supported(connection)?;
    let database_id: Vec<u8> = connection
        .query_row(
            "SELECT database_id FROM database_meta WHERE singleton = 1",
            [],
            |row| row.get(0),
        )
        .map_err(map_sqlite_error)?;
    let database_id = database_id
        .try_into()
        .map_err(|_| DatabaseError::CorruptDatabase)?;
    let database_generation: Vec<u8> = connection
        .query_row(
            "SELECT database_generation FROM database_meta WHERE singleton = 1",
            [],
            |row| row.get(0),
        )
        .map_err(map_sqlite_error)?;
    let database_generation = database_generation
        .try_into()
        .map_err(|_| DatabaseError::CorruptDatabase)?;
    let generation = generation_in(connection)?;
    Ok((schema_version, database_id, database_generation, generation))
}

fn manifest_matches(path: &Path, manifest: &BackupManifest) -> Result<bool, DatabaseError> {
    Ok(inspect_database(path)?
        == (
            manifest.schema_version,
            manifest.database_id,
            manifest.database_generation,
            manifest.generation,
        ))
}

fn copy_file_snapshot(source: &Path, destination: &Path) -> Result<(), DatabaseError> {
    let mut source = File::open(source).map_err(|_| DatabaseError::StorageIo)?;
    let mut destination = File::create(destination).map_err(|_| DatabaseError::StorageIo)?;
    io::copy(&mut source, &mut destination).map_err(|_| DatabaseError::StorageIo)?;
    destination
        .flush()
        .and_then(|()| destination.sync_all())
        .map_err(|_| DatabaseError::StorageIo)
}

fn install_replacement(destination: &Path) -> Result<(), DatabaseError> {
    let journal = install_path(destination, "journal");
    let old = install_path(destination, "old");
    let new = install_path(destination, "new");
    remove_family(&old)?;
    write_phase(&journal, "prepared\n")?;
    move_family(destination, &old)?;
    sync_parent(destination)?;
    fs::rename(&new, destination).map_err(|_| DatabaseError::StorageIo)?;
    sync_file(destination)?;
    sync_parent(destination)?;
    write_phase(&journal, "installed\n")?;
    Ok(())
}

fn write_phase(path: &Path, phase: &str) -> Result<(), DatabaseError> {
    let mut file = File::create(path).map_err(|_| DatabaseError::StorageIo)?;
    file.write_all(phase.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|_| DatabaseError::StorageIo)?;
    sync_parent(path)
}

fn install_path(destination: &Path, suffix: &str) -> PathBuf {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    let name = destination
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    parent.join(format!(".{name}.install-{suffix}"))
}

fn family_paths(base: &Path) -> [PathBuf; 3] {
    [
        base.to_path_buf(),
        PathBuf::from(format!("{}-wal", base.display())),
        PathBuf::from(format!("{}-shm", base.display())),
    ]
}

fn family_exists(base: &Path) -> bool {
    family_paths(base).iter().any(|path| path.exists())
}

fn move_family(source: &Path, destination: &Path) -> Result<(), DatabaseError> {
    let source_family = family_paths(source);
    let destination_family = family_paths(destination);
    for (source, destination) in source_family.iter().zip(destination_family.iter()) {
        if source.exists() {
            remove_if_exists(destination)?;
            fs::rename(source, destination).map_err(|_| DatabaseError::StorageIo)?;
        }
    }
    Ok(())
}

fn remove_family(base: &Path) -> Result<(), DatabaseError> {
    for path in family_paths(base) {
        remove_if_exists(&path)?;
    }
    Ok(())
}

fn remove_if_exists(path: &Path) -> Result<(), DatabaseError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(DatabaseError::StorageIo),
    }
}

fn validate_destination(destination: &Path) -> Result<(), DatabaseError> {
    if destination.as_os_str().is_empty() || destination.file_name().is_none() {
        return Err(DatabaseError::InvalidInput);
    }
    let parent = destination.parent().ok_or(DatabaseError::InvalidInput)?;
    if !parent.is_dir() {
        return Err(DatabaseError::StorageIo);
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<[u8; 32], DatabaseError> {
    let mut file = File::open(path).map_err(|_| DatabaseError::StorageIo)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| DatabaseError::StorageIo)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(digest.finalize().into())
}

fn sync_file(path: &Path) -> Result<(), DatabaseError> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|_| DatabaseError::StorageIo)
}

fn sync_parent(path: &Path) -> Result<(), DatabaseError> {
    let parent = path.parent().ok_or(DatabaseError::InvalidInput)?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| DatabaseError::StorageIo)
}

struct TemporaryFile {
    path: PathBuf,
    installed: bool,
}

impl TemporaryFile {
    fn new(destination: &Path, marker: &str) -> Result<Self, DatabaseError> {
        let parent = destination.parent().ok_or(DatabaseError::InvalidInput)?;
        let name = destination
            .file_name()
            .ok_or(DatabaseError::InvalidInput)?
            .to_string_lossy();
        let sequence = NEXT_TEMPORARY_FILE.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(
            ".{name}.{marker}-{}-{sequence}",
            std::process::id()
        ));
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return Err(DatabaseError::StorageIo),
        }
        Ok(Self {
            path,
            installed: false,
        })
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn install(mut self, destination: &Path) -> Result<(), DatabaseError> {
        fs::rename(&self.path, destination).map_err(|_| DatabaseError::StorageIo)?;
        sync_file(destination)?;
        sync_parent(destination)?;
        self.installed = true;
        Ok(())
    }
}

impl Drop for TemporaryFile {
    fn drop(&mut self) {
        if !self.installed {
            let _ = fs::remove_file(&self.path);
            let _ = fs::remove_file(PathBuf::from(format!("{}-wal", self.path.display())));
            let _ = fs::remove_file(PathBuf::from(format!("{}-shm", self.path.display())));
        }
    }
}
