use super::backup::{self, BackupManifest};
use super::schema;
use rusqlite::ffi::ErrorCode;
use rusqlite::{Connection, OpenFlags, OptionalExtension, Transaction, TransactionBehavior};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

const DEFAULT_BUSY_TIMEOUT: Duration = Duration::from_secs(2);
const DEFAULT_READER_POOL_SIZE: usize = 4;
const DEFAULT_CHECKPOINT_SOFT_PAGES: u32 = 256;
const DEFAULT_CHECKPOINT_HARD_PAGES: u32 = 1024;
const MAX_LOCAL_STATE_KEY_BYTES: usize = 128;
const MAX_LOCAL_STATE_VALUE_BYTES: usize = 1024 * 1024;
const LOCAL_STATE_PAGE_HEADROOM: u32 = 32;

#[derive(Clone, Debug)]
pub struct DatabaseConfig {
    busy_timeout: Duration,
    max_page_count: Option<u32>,
    reader_pool_size: usize,
    checkpoint_soft_pages: u32,
    checkpoint_hard_pages: u32,
}

impl DatabaseConfig {
    #[must_use]
    pub fn with_busy_timeout(mut self, timeout: Duration) -> Self {
        self.busy_timeout = timeout;
        self
    }

    #[must_use]
    pub fn busy_timeout(&self) -> Duration {
        self.busy_timeout
    }

    #[must_use]
    pub fn with_reader_pool_size(mut self, size: usize) -> Self {
        self.reader_pool_size = size;
        self
    }

    #[must_use]
    pub fn with_checkpoint_pages(mut self, soft: u32, hard: u32) -> Self {
        self.checkpoint_soft_pages = soft;
        self.checkpoint_hard_pages = hard;
        self
    }

    /// Applies SQLite's real page limit to this connection. Production leaves
    /// it unset; deterministic storage-boundary tests use it to exercise
    /// `SQLITE_FULL` without replacing the filesystem with a fake.
    #[must_use]
    pub fn with_max_page_count(mut self, pages: u32) -> Self {
        self.max_page_count = Some(pages);
        self
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            busy_timeout: DEFAULT_BUSY_TIMEOUT,
            max_page_count: None,
            reader_pool_size: DEFAULT_READER_POOL_SIZE,
            checkpoint_soft_pages: DEFAULT_CHECKPOINT_SOFT_PAGES,
            checkpoint_hard_pages: DEFAULT_CHECKPOINT_HARD_PAGES,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JournalMode {
    Wal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Durability {
    Full,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DatabaseSettings {
    pub journal_mode: JournalMode,
    pub foreign_keys: bool,
    pub durability: Durability,
    pub busy_timeout: Duration,
    pub wal_autocheckpoint_pages: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckpointMode {
    Passive,
    Full,
    Restart,
    Truncate,
}

impl CheckpointMode {
    fn sql(self) -> &'static str {
        match self {
            Self::Passive => "PRAGMA wal_checkpoint(PASSIVE)",
            Self::Full => "PRAGMA wal_checkpoint(FULL)",
            Self::Restart => "PRAGMA wal_checkpoint(RESTART)",
            Self::Truncate => "PRAGMA wal_checkpoint(TRUNCATE)",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckpointResult {
    pub busy: bool,
    pub log_frames: u32,
    pub checkpointed_frames: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum DatabaseError {
    InvalidInput,
    BusyRetryable,
    StorageFull,
    StorageReadOnly,
    CorruptDatabase,
    MigrationRequired { found: u32, supported: u32 },
    MigrationFailed,
    BackupMismatch,
    StorageIo,
    Internal,
}

impl fmt::Display for DatabaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidInput => "invalid database input",
            Self::BusyRetryable => "database is busy",
            Self::StorageFull => "database storage is full",
            Self::StorageReadOnly => "database storage is read-only",
            Self::CorruptDatabase => "database is corrupt",
            Self::MigrationRequired { .. } => "database migration is required",
            Self::MigrationFailed => "database migration failed",
            Self::BackupMismatch => "backup does not match its manifest",
            Self::StorageIo => "database storage operation failed",
            Self::Internal => "internal database failure",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for DatabaseError {}

pub struct RiotDatabase {
    pub(crate) inner: Arc<DatabaseInner>,
}

pub(crate) struct DatabaseInner {
    pub(crate) path: PathBuf,
    pub(crate) writer: Mutex<Connection>,
    pub(crate) read_only: bool,
    config: DatabaseConfig,
    readers: Arc<ReaderPool>,
    _path_lease: PathLease,
}

pub struct RiotReadSnapshot {
    reader: ReaderLease,
    _database: Arc<DatabaseInner>,
}

/// A conservative upper bound supplied by every writer before SQLite begins
/// its transaction. `payload_bytes` covers new/changed payload and overflow
/// storage; `page_headroom` covers B-tree paths, splits, freelist bookkeeping,
/// and fixed metadata pages specific to that transaction shape.
#[derive(Clone, Copy, Debug)]
pub(crate) struct WriteEstimate {
    payload_bytes: usize,
    page_headroom: u32,
}

impl WriteEstimate {
    pub(crate) const fn new(payload_bytes: usize, page_headroom: u32) -> Self {
        Self {
            payload_bytes,
            page_headroom,
        }
    }
}

impl fmt::Debug for RiotReadSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RiotReadSnapshot")
            .finish_non_exhaustive()
    }
}

impl RiotReadSnapshot {
    pub fn local_state(&self, key: &str) -> Result<Option<Vec<u8>>, DatabaseError> {
        validate_local_state_key(key)?;
        self.reader
            .connection()
            .query_row(
                "SELECT value FROM local_state WHERE key = ?1",
                [key],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_sqlite_error)
    }

    pub fn is_query_only(&self) -> Result<bool, DatabaseError> {
        let query_only: i64 = self
            .reader
            .connection()
            .query_row("PRAGMA query_only", [], |row| row.get(0))
            .map_err(map_sqlite_error)?;
        Ok(query_only == 1)
    }
}

impl Drop for RiotReadSnapshot {
    fn drop(&mut self) {
        let _ = self.reader.connection().execute_batch("ROLLBACK");
    }
}

impl Clone for RiotDatabase {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl fmt::Debug for RiotDatabase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RiotDatabase")
            .field("path", &self.inner.path)
            .field("read_only", &self.inner.read_only)
            .finish_non_exhaustive()
    }
}

impl RiotDatabase {
    pub fn open(path: impl AsRef<Path>, config: DatabaseConfig) -> Result<Self, DatabaseError> {
        let path = path.as_ref();
        validate_path(path, false)?;
        let lease = PathLease::acquire(path, LeaseMode::Writer)?;
        Self::open_with_lease(path, config, false, lease)
    }

    pub fn open_read_only(
        path: impl AsRef<Path>,
        config: DatabaseConfig,
    ) -> Result<Self, DatabaseError> {
        let path = path.as_ref();
        validate_path(path, true)?;
        let lease = PathLease::acquire(path, LeaseMode::Reader)?;
        Self::open_with_lease(path, config, true, lease)
    }

    fn open_with_lease(
        path: &Path,
        config: DatabaseConfig,
        read_only: bool,
        lease: PathLease,
    ) -> Result<Self, DatabaseError> {
        validate_config(&config)?;
        let pending_install = if read_only {
            false
        } else {
            backup::recover_install(path)?
        };
        let existing_nonempty = fs::metadata(path)
            .map(|metadata| metadata.len() > 0)
            .unwrap_or(false);
        let flags = if read_only {
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX
        } else {
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
        };
        let mut connection = Connection::open_with_flags(path, flags).map_err(map_sqlite_error)?;
        connection
            .busy_timeout(config.busy_timeout)
            .map_err(map_sqlite_error)?;
        if existing_nonempty {
            schema::preflight_existing(&connection)?;
        }
        if read_only {
            configure_connection(&connection, &config, true)?;
            schema::validate_supported(&connection)?;
        } else {
            configure_migration_connection(&connection, &config)?;
            schema::migrate(&mut connection)?;
            // WAL is persistent database state. Enable it only after the
            // migration transaction has committed successfully.
            configure_connection(&connection, &config, false)?;
            if let Some(max_page_count) = config.max_page_count {
                connection
                    .pragma_update(None, "max_page_count", max_page_count)
                    .map_err(map_sqlite_error)?;
            }
        }
        ensure_integrity(&connection)?;
        let readers = Arc::new(ReaderPool::open(path, &config)?);

        let database = Self {
            inner: Arc::new(DatabaseInner {
                path: path.to_path_buf(),
                writer: Mutex::new(connection),
                read_only,
                config,
                readers,
                _path_lease: lease,
            }),
        };
        if pending_install {
            backup::finish_install(path)?;
        }
        Ok(database)
    }

    pub fn settings(&self) -> Result<DatabaseSettings, DatabaseError> {
        let reader = self.inner.readers.acquire()?;
        let connection = reader.connection();
        let journal_mode: String = connection
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .map_err(map_sqlite_error)?;
        if !journal_mode.eq_ignore_ascii_case("wal") {
            return Err(DatabaseError::Internal);
        }
        let foreign_keys: i64 = connection
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .map_err(map_sqlite_error)?;
        let synchronous: i64 = connection
            .query_row("PRAGMA synchronous", [], |row| row.get(0))
            .map_err(map_sqlite_error)?;
        let busy_timeout: i64 = connection
            .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
            .map_err(map_sqlite_error)?;
        let wal_autocheckpoint: u32 = connection
            .query_row("PRAGMA wal_autocheckpoint", [], |row| row.get(0))
            .map_err(map_sqlite_error)?;
        if foreign_keys != 1 || synchronous != 2 {
            return Err(DatabaseError::Internal);
        }
        Ok(DatabaseSettings {
            journal_mode: JournalMode::Wal,
            foreign_keys: true,
            durability: Durability::Full,
            busy_timeout: Duration::from_millis(
                u64::try_from(busy_timeout).map_err(|_| DatabaseError::CorruptDatabase)?,
            ),
            wal_autocheckpoint_pages: wal_autocheckpoint,
        })
    }

    pub fn schema_version(&self) -> Result<u32, DatabaseError> {
        let reader = self.inner.readers.acquire()?;
        schema::validate_supported(reader.connection())
    }

    pub fn database_id(&self) -> Result<[u8; 16], DatabaseError> {
        let reader = self.inner.readers.acquire()?;
        let connection = reader.connection();
        let bytes: Vec<u8> = connection
            .query_row(
                "SELECT database_id FROM database_meta WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .map_err(map_sqlite_error)?;
        bytes.try_into().map_err(|_| DatabaseError::CorruptDatabase)
    }

    /// Identifies this installed database incarnation. Restore replaces it so
    /// pre-restore cursors and sessions can never compare equal by accident.
    pub fn database_generation(&self) -> Result<[u8; 16], DatabaseError> {
        let reader = self.inner.readers.acquire()?;
        let connection = reader.connection();
        let bytes: Vec<u8> = connection
            .query_row(
                "SELECT database_generation FROM database_meta WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .map_err(map_sqlite_error)?;
        bytes.try_into().map_err(|_| DatabaseError::CorruptDatabase)
    }

    pub fn generation(&self) -> Result<u64, DatabaseError> {
        let reader = self.inner.readers.acquire()?;
        generation_in(reader.connection())
    }

    pub fn authority_quarantined(&self) -> Result<bool, DatabaseError> {
        let reader = self.inner.readers.acquire()?;
        let connection = reader.connection();
        let quarantined: i64 = connection
            .query_row(
                "SELECT authority_quarantined FROM database_meta WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .map_err(map_sqlite_error)?;
        match quarantined {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(DatabaseError::CorruptDatabase),
        }
    }

    pub fn set_local_state(&self, key: &str, value: &[u8]) -> Result<(), DatabaseError> {
        if key.is_empty()
            || key.len() > MAX_LOCAL_STATE_KEY_BYTES
            || value.len() > MAX_LOCAL_STATE_VALUE_BYTES
        {
            return Err(DatabaseError::InvalidInput);
        }
        let estimate = WriteEstimate::new(
            key.len().saturating_add(value.len()),
            LOCAL_STATE_PAGE_HEADROOM,
        );
        self.write_transaction(estimate, |transaction| {
            transaction
                .execute(
                    "INSERT INTO local_state(key, value) VALUES (?1, ?2)\
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    (key, value),
                )
                .map_err(map_sqlite_error)?;
            transaction
                .execute(
                    "UPDATE database_meta SET generation = generation + 1 WHERE singleton = 1",
                    [],
                )
                .map_err(map_sqlite_error)?;
            Ok(())
        })
    }

    pub fn local_state(&self, key: &str) -> Result<Option<Vec<u8>>, DatabaseError> {
        validate_local_state_key(key)?;
        let reader = self.inner.readers.acquire()?;
        reader
            .connection()
            .query_row(
                "SELECT value FROM local_state WHERE key = ?1",
                [key],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_sqlite_error)
    }

    pub fn integrity_check(&self) -> Result<bool, DatabaseError> {
        let reader = self.inner.readers.acquire()?;
        match quick_check(reader.connection()) {
            Ok(()) => Ok(true),
            Err(DatabaseError::CorruptDatabase) => Ok(false),
            Err(error) => Err(error),
        }
    }

    pub fn checkpoint(&self, mode: CheckpointMode) -> Result<CheckpointResult, DatabaseError> {
        if self.inner.read_only {
            return Err(DatabaseError::StorageReadOnly);
        }
        let connection = self.lock_writer()?;
        checkpoint_in(&connection, mode)
    }

    pub fn backup_to(
        &self,
        destination: impl AsRef<Path>,
    ) -> Result<BackupManifest, DatabaseError> {
        let destination = destination.as_ref();
        if destination == self.inner.path {
            return Err(DatabaseError::StorageReadOnly);
        }
        validate_path(destination, false)?;
        let _lease = PathLease::acquire(destination, LeaseMode::Restore)?;
        backup::create(self, destination)
    }

    pub fn restore_from(
        destination: impl AsRef<Path>,
        source: impl AsRef<Path>,
        manifest: &BackupManifest,
        config: DatabaseConfig,
    ) -> Result<Self, DatabaseError> {
        validate_config(&config)?;
        let destination = destination.as_ref();
        validate_path(destination, false)?;
        let lease = PathLease::acquire(destination, LeaseMode::Restore)?;
        backup::restore(destination, source.as_ref(), manifest)?;
        let lease = lease.promote_to_writer()?;
        Self::open_with_lease(destination, config, false, lease)
    }

    #[must_use]
    pub fn reader_pool_capacity(&self) -> usize {
        self.inner.readers.capacity
    }

    pub fn read_snapshot(&self) -> Result<RiotReadSnapshot, DatabaseError> {
        let reader = self.inner.readers.acquire()?;
        reader
            .connection()
            .execute_batch("BEGIN")
            .map_err(map_sqlite_error)?;
        Ok(RiotReadSnapshot {
            reader,
            _database: Arc::clone(&self.inner),
        })
    }

    pub(crate) fn write_transaction<T, F>(
        &self,
        estimate: WriteEstimate,
        operation: F,
    ) -> Result<T, DatabaseError>
    where
        F: FnOnce(&Transaction<'_>) -> Result<T, DatabaseError>,
    {
        if self.inner.read_only {
            return Err(DatabaseError::StorageReadOnly);
        }
        let mut connection = self.lock_writer()?;
        admit_write(&connection, &self.inner.path, &self.inner.config, estimate)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite_error)?;
        let result = operation(&transaction)?;
        transaction.commit().map_err(map_sqlite_error)?;
        automatic_checkpoint(&connection, &self.inner.config);
        Ok(result)
    }

    pub(crate) fn lock_writer(&self) -> Result<MutexGuard<'_, Connection>, DatabaseError> {
        self.inner
            .writer
            .lock()
            .map_err(|_| DatabaseError::Internal)
    }
}

fn validate_local_state_key(key: &str) -> Result<(), DatabaseError> {
    if key.is_empty() || key.len() > MAX_LOCAL_STATE_KEY_BYTES {
        Err(DatabaseError::InvalidInput)
    } else {
        Ok(())
    }
}

fn validate_config(config: &DatabaseConfig) -> Result<(), DatabaseError> {
    if config.reader_pool_size == 0
        || config.checkpoint_soft_pages == 0
        || config.checkpoint_hard_pages <= config.checkpoint_soft_pages
    {
        return Err(DatabaseError::InvalidInput);
    }
    Ok(())
}

struct ReaderPool {
    connections: Mutex<Vec<Connection>>,
    available: Condvar,
    capacity: usize,
    wait_timeout: Duration,
}

impl ReaderPool {
    fn open(path: &Path, config: &DatabaseConfig) -> Result<Self, DatabaseError> {
        let mut connections = Vec::with_capacity(config.reader_pool_size);
        for _ in 0..config.reader_pool_size {
            let connection = Connection::open_with_flags(
                path,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )
            .map_err(map_sqlite_error)?;
            configure_connection(&connection, config, true)?;
            schema::validate_supported(&connection)?;
            connections.push(connection);
        }
        Ok(Self {
            connections: Mutex::new(connections),
            available: Condvar::new(),
            capacity: config.reader_pool_size,
            wait_timeout: config.busy_timeout,
        })
    }

    fn acquire(self: &Arc<Self>) -> Result<ReaderLease, DatabaseError> {
        let deadline = Instant::now() + self.wait_timeout;
        let mut connections = self
            .connections
            .lock()
            .map_err(|_| DatabaseError::Internal)?;
        loop {
            if let Some(connection) = connections.pop() {
                return Ok(ReaderLease {
                    pool: Arc::clone(self),
                    connection: Some(connection),
                });
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(DatabaseError::BusyRetryable);
            }
            let (guard, result) = self
                .available
                .wait_timeout(connections, remaining)
                .map_err(|_| DatabaseError::Internal)?;
            connections = guard;
            if result.timed_out() && connections.is_empty() {
                return Err(DatabaseError::BusyRetryable);
            }
        }
    }
}

struct ReaderLease {
    pool: Arc<ReaderPool>,
    connection: Option<Connection>,
}

impl ReaderLease {
    fn connection(&self) -> &Connection {
        self.connection
            .as_ref()
            .expect("reader lease always owns a connection until drop")
    }
}

impl Drop for ReaderLease {
    fn drop(&mut self) {
        if let Some(connection) = self.connection.take() {
            if let Ok(mut connections) = self.pool.connections.lock() {
                connections.push(connection);
                self.pool.available.notify_one();
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LeaseMode {
    Writer,
    Reader,
    Restore,
}

#[derive(Default)]
struct PathState {
    writer: bool,
    readers: usize,
    restoring: bool,
}

struct PathLease {
    path: PathBuf,
    mode: LeaseMode,
}

impl PathLease {
    fn acquire(path: &Path, mode: LeaseMode) -> Result<Self, DatabaseError> {
        let path = normalized_path(path)?;
        let mut registry = path_registry()
            .lock()
            .map_err(|_| DatabaseError::Internal)?;
        let state = registry.entry(path.clone()).or_default();
        let blocked = match mode {
            LeaseMode::Writer => state.writer || state.restoring,
            LeaseMode::Reader => state.restoring,
            LeaseMode::Restore => state.writer || state.readers != 0 || state.restoring,
        };
        if blocked {
            return Err(DatabaseError::BusyRetryable);
        }
        match mode {
            LeaseMode::Writer => state.writer = true,
            LeaseMode::Reader => state.readers += 1,
            LeaseMode::Restore => state.restoring = true,
        }
        Ok(Self { path, mode })
    }

    fn promote_to_writer(mut self) -> Result<Self, DatabaseError> {
        let mut registry = path_registry()
            .lock()
            .map_err(|_| DatabaseError::Internal)?;
        let state = registry
            .get_mut(&self.path)
            .ok_or(DatabaseError::Internal)?;
        if self.mode != LeaseMode::Restore || !state.restoring || state.writer || state.readers != 0
        {
            return Err(DatabaseError::Internal);
        }
        state.restoring = false;
        state.writer = true;
        self.mode = LeaseMode::Writer;
        Ok(self)
    }
}

impl Drop for PathLease {
    fn drop(&mut self) {
        let Ok(mut registry) = path_registry().lock() else {
            return;
        };
        let Some(state) = registry.get_mut(&self.path) else {
            return;
        };
        match self.mode {
            LeaseMode::Writer => state.writer = false,
            LeaseMode::Reader => state.readers = state.readers.saturating_sub(1),
            LeaseMode::Restore => state.restoring = false,
        }
        if !state.writer && state.readers == 0 && !state.restoring {
            registry.remove(&self.path);
        }
    }
}

fn path_registry() -> &'static Mutex<HashMap<PathBuf, PathState>> {
    static REGISTRY: OnceLock<Mutex<HashMap<PathBuf, PathState>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn normalized_path(path: &Path) -> Result<PathBuf, DatabaseError> {
    if path.exists() {
        return path.canonicalize().map_err(|_| DatabaseError::StorageIo);
    }
    let parent = path.parent().ok_or(DatabaseError::InvalidInput)?;
    let parent = parent
        .canonicalize()
        .map_err(|_| DatabaseError::StorageIo)?;
    Ok(parent.join(path.file_name().ok_or(DatabaseError::InvalidInput)?))
}

fn validate_path(path: &Path, read_only: bool) -> Result<(), DatabaseError> {
    if path.as_os_str().is_empty() || path.file_name().is_none() {
        return Err(DatabaseError::InvalidInput);
    }
    let parent = path.parent().ok_or(DatabaseError::InvalidInput)?;
    if !parent.is_dir() || (read_only && !path.is_file()) {
        return Err(DatabaseError::StorageIo);
    }
    Ok(())
}

fn configure_connection(
    connection: &Connection,
    config: &DatabaseConfig,
    read_only: bool,
) -> Result<(), DatabaseError> {
    configure_migration_connection(connection, config)?;
    connection
        .pragma_update(None, "wal_autocheckpoint", 0)
        .map_err(map_sqlite_error)?;
    if read_only {
        connection
            .pragma_update(None, "query_only", "ON")
            .map_err(map_sqlite_error)?;
        let journal_mode: String = connection
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .map_err(map_sqlite_error)?;
        if !journal_mode.eq_ignore_ascii_case("wal") {
            return Err(DatabaseError::CorruptDatabase);
        }
    } else {
        let journal_mode: String = connection
            .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
            .map_err(map_sqlite_error)?;
        if !journal_mode.eq_ignore_ascii_case("wal") {
            return Err(DatabaseError::Internal);
        }
    }
    Ok(())
}

/// Connection-local safety settings used by migrations. In particular this
/// does not change persistent journal mode, so a rolled-back migration leaves
/// the database file and its sidecar family byte-for-byte untouched.
fn configure_migration_connection(
    connection: &Connection,
    config: &DatabaseConfig,
) -> Result<(), DatabaseError> {
    connection
        .busy_timeout(config.busy_timeout)
        .map_err(map_sqlite_error)?;
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(map_sqlite_error)?;
    connection
        .pragma_update(None, "synchronous", "FULL")
        .map_err(map_sqlite_error)?;
    Ok(())
}

fn checkpoint_in(
    connection: &Connection,
    mode: CheckpointMode,
) -> Result<CheckpointResult, DatabaseError> {
    let (busy, log_frames, checkpointed_frames): (u32, u32, u32) = connection
        .query_row(mode.sql(), [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .map_err(map_sqlite_error)?;
    Ok(CheckpointResult {
        busy: busy != 0,
        log_frames,
        checkpointed_frames,
    })
}

/// Admits a transaction only when its caller-supplied maximum growth fits in
/// the configured WAL ceiling. Thus the physical WAL is bounded by its
/// 32-byte header plus `hard_frames * (24-byte frame header + page_size)`.
/// Future EvidenceStore writers use this same gate and must supply their own
/// payload size and conservative transaction-specific page headroom.
fn admit_write(
    connection: &Connection,
    database_path: &Path,
    config: &DatabaseConfig,
    estimate: WriteEstimate,
) -> Result<(), DatabaseError> {
    let (estimated_frames, page_size) = estimated_wal_frames(connection, estimate)?;
    if estimated_frames > config.checkpoint_hard_pages {
        return Err(DatabaseError::BusyRetryable);
    }
    let status = checkpoint_in(connection, CheckpointMode::Passive)?;
    let absolute_bound = 32_u64.saturating_add(
        u64::from(config.checkpoint_hard_pages).saturating_mul(24_u64.saturating_add(page_size)),
    );
    let physical_wal_bytes = fs::metadata(wal_path(database_path))
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    if physical_wal_bytes <= absolute_bound
        && status.log_frames.saturating_add(estimated_frames) <= config.checkpoint_hard_pages
    {
        return Ok(());
    }
    let truncate = checkpoint_in(connection, CheckpointMode::Truncate)?;
    if truncate.busy {
        return Err(DatabaseError::BusyRetryable);
    }
    let after = checkpoint_in(connection, CheckpointMode::Passive)?;
    if after.log_frames.saturating_add(estimated_frames) > config.checkpoint_hard_pages {
        return Err(DatabaseError::BusyRetryable);
    }
    Ok(())
}

fn estimated_wal_frames(
    connection: &Connection,
    estimate: WriteEstimate,
) -> Result<(u32, u64), DatabaseError> {
    let page_size: i64 = connection
        .query_row("PRAGMA page_size", [], |row| row.get(0))
        .map_err(map_sqlite_error)?;
    let page_size = u64::try_from(page_size).map_err(|_| DatabaseError::CorruptDatabase)?;
    let overflow_payload = page_size
        .checked_sub(4)
        .filter(|bytes| *bytes != 0)
        .ok_or(DatabaseError::CorruptDatabase)?;
    let payload_bytes = u64::try_from(estimate.payload_bytes).unwrap_or(u64::MAX);
    let payload_pages = payload_bytes.saturating_add(overflow_payload - 1) / overflow_payload;
    let frames = payload_pages.saturating_add(u64::from(estimate.page_headroom));
    Ok((u32::try_from(frames).unwrap_or(u32::MAX), page_size))
}

fn wal_path(database_path: &Path) -> PathBuf {
    PathBuf::from(format!("{}-wal", database_path.display()))
}

fn automatic_checkpoint(connection: &Connection, config: &DatabaseConfig) {
    let Ok(status) = checkpoint_in(connection, CheckpointMode::Passive) else {
        return;
    };
    if status.log_frames >= config.checkpoint_soft_pages
        && status.checkpointed_frames == status.log_frames
    {
        let _ = checkpoint_in(connection, CheckpointMode::Restart);
    }
}

pub(crate) fn generation_in(connection: &Connection) -> Result<u64, DatabaseError> {
    let generation: i64 = connection
        .query_row(
            "SELECT generation FROM database_meta WHERE singleton = 1",
            [],
            |row| row.get(0),
        )
        .map_err(map_sqlite_error)?;
    u64::try_from(generation).map_err(|_| DatabaseError::CorruptDatabase)
}

pub(crate) fn ensure_integrity(connection: &Connection) -> Result<(), DatabaseError> {
    quick_check(connection)
}

fn quick_check(connection: &Connection) -> Result<(), DatabaseError> {
    let result: String = connection
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .map_err(map_sqlite_error)?;
    if result == "ok" {
        Ok(())
    } else {
        Err(DatabaseError::CorruptDatabase)
    }
}

pub(crate) fn map_sqlite_error(error: rusqlite::Error) -> DatabaseError {
    match error.sqlite_error_code() {
        Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => DatabaseError::BusyRetryable,
        Some(ErrorCode::DiskFull) => DatabaseError::StorageFull,
        Some(ErrorCode::ReadOnly | ErrorCode::PermissionDenied) => DatabaseError::StorageReadOnly,
        Some(ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase) => {
            DatabaseError::CorruptDatabase
        }
        Some(ErrorCode::CannotOpen | ErrorCode::SystemIoFailure) => DatabaseError::StorageIo,
        _ => DatabaseError::Internal,
    }
}
