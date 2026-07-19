//! Durable service layer over the anchor's SQLite store.
//!
//! [`AnchorRepository`] is the *only* type that touches raw SQL. Later units
//! (control, sync, listing, removal, and directory handlers) go through this
//! service; they never open a bare [`rusqlite::Connection`]. The repository
//! owns four durable guarantees the design's "Anchor Repository" section
//! requires:
//!
//! * **Durable transactions.** Connections open in WAL journal mode with
//!   foreign keys enforced and `synchronous = FULL`, so a crash before a
//!   commit leaves only expirable staging and a crash after a commit is
//!   byte-recoverable (see [`AnchorRepository::open`]).
//! * **Independent accounting classes.** Every one of the nine
//!   [`AccountingClass`] budgets is tracked separately with its own ceiling; a
//!   charge in one class can never mask headroom in another (see
//!   [`RepoTransaction::charge`]).
//! * **Dedup that never discounts logical charge.** Physical payload bytes
//!   deduplicate by digest, but each community pays the *full* logical size —
//!   deduplication only ever affects the physical class (see
//!   [`RepoTransaction::add_payload`]).
//! * **Single-writer deployment lease.** Restoring one backup into two live
//!   deployments is identity cloning, not scaling; a clone or a steal is
//!   detected and fails closed (see
//!   [`AnchorRepository::acquire_deployment_lease`]).
//!
//! Immutable point-in-time reads are served by [`ReadSnapshot`], which WAL
//! makes consistent for the reader even while the writer commits.

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OpenFlags, OptionalExtension, TransactionBehavior};

use crate::schema::{self, SchemaError};

/// Number of independent accounting classes.
pub const ACCOUNTING_CLASS_COUNT: usize = 9;

/// The independent quota classes the anchor accounts for.
///
/// Each class has its own ceiling and its own running usage. The load-bearing
/// property is *independence*: charging one class to its ceiling never consumes
/// or masks another class's headroom. The variants map one-for-one onto stable
/// limit IDs in the design's canonical limit registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccountingClass {
    /// `logical_retained_bytes_whole_anchor` (limit 1): per-community logical
    /// bytes summed across the anchor. Never discounted by deduplication.
    Logical,
    /// `physical_retained_bytes` (limit 2): on-disk payload bytes after
    /// digest deduplication.
    Physical,
    /// `non_payload_metadata_bytes` (limit 4): metadata not counted as payload.
    Metadata,
    /// `sqlite_wal_bytes` (limit 5): write-ahead-log bytes.
    Wal,
    /// `staged_bytes` (limit 8): operation-private staging not yet promoted.
    Staging,
    /// `idempotency_rows` (limit 10): retained idempotency index rows.
    Idempotency,
    /// `incident_conflict_records` (limit 13): retained conflict/incident rows.
    Conflict,
    /// `local_operational_log_bytes_all_classes` (limit 65): operational log.
    Log,
    /// `static_projection_bytes` (limit 59): rendered static projection bytes.
    Static,
}

impl AccountingClass {
    /// Every accounting class, in stable index order.
    pub const ALL: [AccountingClass; ACCOUNTING_CLASS_COUNT] = [
        AccountingClass::Logical,
        AccountingClass::Physical,
        AccountingClass::Metadata,
        AccountingClass::Wal,
        AccountingClass::Staging,
        AccountingClass::Idempotency,
        AccountingClass::Conflict,
        AccountingClass::Log,
        AccountingClass::Static,
    ];

    /// This class's stable index into a per-class array.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// The per-class ceilings a repository enforces.
///
/// Byte classes are expressed in bytes; count classes ([`AccountingClass::Idempotency`]
/// and [`AccountingClass::Conflict`]) are expressed in row counts. An operator
/// may only lower an effective value below the compiled default.
#[derive(Debug, Clone, Copy)]
pub struct AccountingCeilings {
    values: [u64; ACCOUNTING_CLASS_COUNT],
}

impl AccountingCeilings {
    /// The compiled MVP default effective ceilings from the design's limit
    /// table.
    #[must_use]
    pub const fn mvp_defaults() -> Self {
        const GIB: u64 = 1024 * 1024 * 1024;
        const MIB: u64 = 1024 * 1024;
        let mut values = [0u64; ACCOUNTING_CLASS_COUNT];
        values[AccountingClass::Logical.index()] = 20 * GIB;
        values[AccountingClass::Physical.index()] = 20 * GIB;
        values[AccountingClass::Metadata.index()] = 2 * GIB;
        values[AccountingClass::Wal.index()] = 256 * MIB;
        values[AccountingClass::Staging.index()] = 256 * MIB;
        values[AccountingClass::Idempotency.index()] = 100_000;
        values[AccountingClass::Conflict.index()] = 10_000;
        values[AccountingClass::Log.index()] = 512 * MIB;
        values[AccountingClass::Static.index()] = 5 * GIB;
        Self { values }
    }

    /// Construct ceilings from an explicit per-class array (test/operator use).
    #[must_use]
    pub const fn from_array(values: [u64; ACCOUNTING_CLASS_COUNT]) -> Self {
        Self { values }
    }

    /// The ceiling configured for `class`.
    #[must_use]
    pub const fn ceiling(&self, class: AccountingClass) -> u64 {
        self.values[class.index()]
    }
}

impl Default for AccountingCeilings {
    fn default() -> Self {
        Self::mvp_defaults()
    }
}

/// Running per-class usage plus its ceilings.
#[derive(Debug, Clone)]
struct Ledger {
    used: [u64; ACCOUNTING_CLASS_COUNT],
    ceiling: [u64; ACCOUNTING_CLASS_COUNT],
}

impl Ledger {
    fn new(ceilings: AccountingCeilings) -> Self {
        Self {
            used: [0; ACCOUNTING_CLASS_COUNT],
            ceiling: ceilings.values,
        }
    }

    /// Rebuild persisted usage from committed rows so accounting survives a
    /// restart. Byte classes with no content table (metadata, WAL, log, static)
    /// start at zero and are tracked purely through runtime charges.
    fn rehydrate(&mut self, connection: &Connection) -> Result<(), AnchorRepositoryError> {
        // SQLite stores INTEGER as i64; these aggregates are non-negative by
        // construction (CHECK constraints), so the cast to u64 is lossless.
        let read = |sql: &str| -> Result<u64, AnchorRepositoryError> {
            let value: i64 = connection.query_row(sql, [], |row| row.get(0))?;
            Ok(value.max(0) as u64)
        };
        self.used[AccountingClass::Logical.index()] =
            read("SELECT COALESCE(SUM(logical_bytes), 0) FROM communities")?;
        self.used[AccountingClass::Physical.index()] =
            read("SELECT COALESCE(SUM(payload_length), 0) FROM payloads")?;
        self.used[AccountingClass::Staging.index()] =
            read("SELECT COALESCE(SUM(staged_bytes), 0) FROM staged_operations")?;
        self.used[AccountingClass::Idempotency.index()] =
            read("SELECT COUNT(*) FROM idempotency_key_index")?;
        self.used[AccountingClass::Conflict.index()] =
            read("SELECT COUNT(*) FROM listing_conflict_floors")?;
        Ok(())
    }
}

/// Errors from the anchor repository service layer.
#[derive(Debug)]
#[non_exhaustive]
pub enum AnchorRepositoryError {
    /// A raw SQLite error.
    Sqlite(rusqlite::Error),
    /// Opening/migrating the schema failed.
    Schema(SchemaError),
    /// A charge would push a class over its ceiling. No other class is touched.
    ClassExceeded {
        /// The class whose ceiling would be exceeded.
        class: AccountingClass,
        /// The configured ceiling for that class.
        ceiling: u64,
        /// The class usage before this charge.
        used: u64,
        /// The additional amount the charge requested.
        requested: u64,
    },
    /// No free preprovisioned removal slot remained.
    RemovalSlotsExhausted,
    /// The single-writer lease is currently held by a different, unexpired
    /// holder — a clone/steal attempt that fails closed.
    LeaseHeld {
        /// The current holder that blocks acquisition.
        holder_id: [u8; 32],
        /// When the current holder's lease expires.
        expires_at: u64,
    },
    /// The presented deployment-instance token does not match the one bound to
    /// this database — potential anchor equivocation.
    LeaseTokenMismatch,
    /// A previously held lease is no longer ours (a newer holder/epoch took it).
    LeaseLost,
    /// The verified lease has expired.
    LeaseExpired,
    /// An immutable read snapshot is unavailable (the repository is not backed
    /// by a shareable database file).
    SnapshotUnavailable,
}

impl core::fmt::Display for AnchorRepositoryError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Sqlite(error) => write!(formatter, "anchor repository sqlite error: {error}"),
            Self::Schema(error) => write!(formatter, "anchor repository schema error: {error}"),
            Self::ClassExceeded {
                class,
                ceiling,
                used,
                requested,
            } => write!(
                formatter,
                "accounting class {class:?} would exceed ceiling {ceiling} (used {used} + {requested})"
            ),
            Self::RemovalSlotsExhausted => write!(formatter, "no free removal slot available"),
            Self::LeaseHeld {
                holder_id,
                expires_at,
            } => write!(
                formatter,
                "deployment lease held by another holder {holder_id:02x?} until {expires_at}"
            ),
            Self::LeaseTokenMismatch => {
                write!(formatter, "deployment instance token mismatch (possible clone)")
            }
            Self::LeaseLost => write!(formatter, "deployment lease was taken by another holder"),
            Self::LeaseExpired => write!(formatter, "deployment lease has expired"),
            Self::SnapshotUnavailable => {
                write!(formatter, "immutable read snapshot requires a file-backed repository")
            }
        }
    }
}

impl std::error::Error for AnchorRepositoryError {}

impl From<rusqlite::Error> for AnchorRepositoryError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

impl From<SchemaError> for AnchorRepositoryError {
    fn from(error: SchemaError) -> Self {
        Self::Schema(error)
    }
}

/// A held single-writer deployment lease.
///
/// A lease binds a `holder_id` at a monotonically increasing `epoch` to the
/// database's `deployment_instance_token`. If another holder steals the lease
/// (after expiry) the epoch advances, so the original holder's
/// [`AnchorRepository::verify_deployment_lease`] fails closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeploymentLease {
    /// The deployment-instance token this lease is bound to.
    pub token: [u8; 32],
    /// The identity that currently holds the lease.
    pub holder_id: [u8; 32],
    /// The lease epoch; advances every time a fresh holder takes the lease.
    pub epoch: u64,
    /// When this lease expires (unix seconds).
    pub expires_at: u64,
}

/// Outcome of a startup readiness-recovery pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoveryReport {
    /// How many expired staging operations were cleared.
    pub cleared_staging_operations: u64,
    /// How many staged bytes were reclaimed to the staging class.
    pub reclaimed_staging_bytes: u64,
}

/// A deterministic eviction tier, highest priority first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvictionTier {
    /// Expired or unlisted directory projections.
    ExpiredProjection,
    /// Incomplete, abandoned staging past its signed retention horizon.
    AbandonedStaging,
    /// Unlisted sites, oldest first.
    UnlistedSite,
    /// Listed sites, oldest successful host refresh first.
    ListedSite,
}

/// One deterministic eviction candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvictionCandidate {
    /// The tier that selected this candidate.
    pub tier: EvictionTier,
    /// The identifying key of the row to evict (inclusion id, operation id, or
    /// community id).
    pub key: Vec<u8>,
}

/// Outcome of adding a payload reference for a community.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PayloadOutcome {
    /// Whether this call charged the logical class (a new per-community ref).
    pub logical_charged: bool,
    /// Whether this call charged the physical class (a first-seen digest). When
    /// `false`, the payload deduplicated against an existing physical copy.
    pub physical_charged: bool,
}

/// The durable anchor repository: the single owner of raw SQL access.
pub struct AnchorRepository {
    connection: Connection,
    ledger: Ledger,
    db_path: Option<PathBuf>,
}

impl AnchorRepository {
    fn configure(connection: &Connection) -> Result<(), AnchorRepositoryError> {
        // WAL journal + foreign keys + full synchronous durability are
        // mandatory (design "Transactions"). `journal_mode = WAL` resolves to
        // "memory" for an in-memory database, which is expected.
        connection.pragma_update(None, "foreign_keys", true)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "FULL")?;
        Ok(())
    }

    fn from_connection(
        mut connection: Connection,
        ceilings: AccountingCeilings,
        db_path: Option<PathBuf>,
    ) -> Result<Self, AnchorRepositoryError> {
        Self::configure(&connection)?;
        schema::migrate(&mut connection)?;
        let mut ledger = Ledger::new(ceilings);
        ledger.rehydrate(&connection)?;
        Ok(Self {
            connection,
            ledger,
            db_path,
        })
    }

    /// Open (creating if needed) a file-backed repository with MVP-default
    /// ceilings. WAL, foreign keys, full synchronous durability, and the
    /// forward-only schema are all applied; per-class usage is rehydrated from
    /// committed rows.
    pub fn open(path: &Path) -> Result<Self, AnchorRepositoryError> {
        Self::open_with_ceilings(path, AccountingCeilings::mvp_defaults())
    }

    /// Open a file-backed repository with explicit ceilings.
    pub fn open_with_ceilings(
        path: &Path,
        ceilings: AccountingCeilings,
    ) -> Result<Self, AnchorRepositoryError> {
        let connection = Connection::open(path)?;
        Self::from_connection(connection, ceilings, Some(path.to_path_buf()))
    }

    /// Open an in-memory repository (no shareable snapshots) with MVP defaults.
    pub fn open_in_memory() -> Result<Self, AnchorRepositoryError> {
        Self::open_in_memory_with_ceilings(AccountingCeilings::mvp_defaults())
    }

    /// Open an in-memory repository with explicit ceilings.
    pub fn open_in_memory_with_ceilings(
        ceilings: AccountingCeilings,
    ) -> Result<Self, AnchorRepositoryError> {
        let connection = Connection::open_in_memory()?;
        Self::from_connection(connection, ceilings, None)
    }

    /// Current usage of an accounting class.
    #[must_use]
    pub fn used(&self, class: AccountingClass) -> u64 {
        self.ledger.used[class.index()]
    }

    /// Configured ceiling of an accounting class.
    #[must_use]
    pub fn ceiling(&self, class: AccountingClass) -> u64 {
        self.ledger.ceiling[class.index()]
    }

    /// Begin an immediate write transaction. Accounting charges accumulate as
    /// pending deltas and are applied to the durable ledger only on
    /// [`RepoTransaction::commit`]; a dropped transaction rolls back both the
    /// SQLite changes and the pending accounting.
    pub fn begin(&mut self) -> Result<RepoTransaction<'_>, AnchorRepositoryError> {
        // Disjoint field borrows: the SQLite transaction borrows the
        // connection, the pending deltas mutate the ledger only on commit.
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        Ok(RepoTransaction {
            transaction,
            ledger: &mut self.ledger,
            pending: [0; ACCOUNTING_CLASS_COUNT],
        })
    }

    /// Acquire the single-writer deployment lease for `holder_id`.
    ///
    /// On first acquisition the database is bound to `token`. A subsequent
    /// acquisition with a different token is rejected as
    /// [`AnchorRepositoryError::LeaseTokenMismatch`] (potential clone). If the
    /// lease is currently held by a *different* holder whose term has not
    /// expired, acquisition fails closed with
    /// [`AnchorRepositoryError::LeaseHeld`]. A free/expired lease is taken with
    /// an advanced epoch (steal detection); the same holder renewing keeps its
    /// epoch.
    pub fn acquire_deployment_lease(
        &mut self,
        holder_id: &[u8; 32],
        token: &[u8; 32],
        now: u64,
        ttl: u64,
    ) -> Result<DeploymentLease, AnchorRepositoryError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (stored_token, holder, epoch, expires): (Option<Vec<u8>>, Option<Vec<u8>>, i64, i64) =
            transaction.query_row(
                "SELECT deployment_instance_token, lease_holder_id, lease_epoch, lease_expires_at \
                 FROM deployment_lease WHERE singleton = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )?;
        let epoch = epoch.max(0) as u64;
        let expires = expires.max(0) as u64;

        match &stored_token {
            None => {
                transaction.execute(
                    "UPDATE deployment_lease SET deployment_instance_token = ?1 WHERE singleton = 1",
                    params![token.as_slice()],
                )?;
            }
            Some(existing) if existing.as_slice() == token.as_slice() => {}
            Some(_) => return Err(AnchorRepositoryError::LeaseTokenMismatch),
        }

        let holder_active = expires > now && holder.is_some();
        let same_holder = holder
            .as_ref()
            .is_some_and(|held| held.as_slice() == holder_id.as_slice());

        if holder_active && !same_holder {
            let mut current = [0u8; 32];
            current.copy_from_slice(holder.as_ref().expect("holder present"));
            return Err(AnchorRepositoryError::LeaseHeld {
                holder_id: current,
                expires_at: expires,
            });
        }

        // Same active holder renews in place; any free/expired lease is a fresh
        // take that advances the epoch (so a prior holder's verify fails).
        let new_epoch = if holder_active && same_holder {
            epoch
        } else {
            epoch + 1
        };
        let new_expires = now + ttl;
        transaction.execute(
            "UPDATE deployment_lease \
             SET lease_holder_id = ?1, lease_epoch = ?2, lease_expires_at = ?3 WHERE singleton = 1",
            params![holder_id.as_slice(), new_epoch as i64, new_expires as i64],
        )?;
        transaction.commit()?;

        Ok(DeploymentLease {
            token: *token,
            holder_id: *holder_id,
            epoch: new_epoch,
            expires_at: new_expires,
        })
    }

    /// Verify that a previously acquired lease is still valid, unstolen, and
    /// unexpired. A newer holder or epoch yields [`AnchorRepositoryError::LeaseLost`].
    pub fn verify_deployment_lease(
        &self,
        lease: &DeploymentLease,
        now: u64,
    ) -> Result<(), AnchorRepositoryError> {
        let (stored_token, holder, epoch, expires): (Option<Vec<u8>>, Option<Vec<u8>>, i64, i64) =
            self.connection.query_row(
                "SELECT deployment_instance_token, lease_holder_id, lease_epoch, lease_expires_at \
                 FROM deployment_lease WHERE singleton = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )?;
        let epoch = epoch.max(0) as u64;
        let expires = expires.max(0) as u64;

        match &stored_token {
            Some(existing) if existing.as_slice() == lease.token.as_slice() => {}
            _ => return Err(AnchorRepositoryError::LeaseTokenMismatch),
        }
        let holder_matches = holder
            .as_ref()
            .is_some_and(|held| held.as_slice() == lease.holder_id.as_slice());
        if !holder_matches || epoch != lease.epoch {
            return Err(AnchorRepositoryError::LeaseLost);
        }
        if expires <= now {
            return Err(AnchorRepositoryError::LeaseExpired);
        }
        Ok(())
    }

    /// Startup readiness recovery: clear staging whose signed retention horizon
    /// (`stage_deadline`) has passed, returning what was reclaimed. A crash
    /// before commit leaves only such expirable staging rows.
    pub fn recover_readiness(&mut self, now: u64) -> Result<RecoveryReport, AnchorRepositoryError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (count, bytes): (i64, i64) = transaction.query_row(
            "SELECT COUNT(*), COALESCE(SUM(staged_bytes), 0) FROM staged_operations \
             WHERE stage_deadline <= ?1",
            params![now as i64],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        transaction.execute(
            "DELETE FROM staged_operations WHERE stage_deadline <= ?1",
            params![now as i64],
        )?;
        transaction.commit()?;

        let count = count.max(0) as u64;
        let bytes = bytes.max(0) as u64;
        let staging = &mut self.ledger.used[AccountingClass::Staging.index()];
        *staging = staging.saturating_sub(bytes);
        Ok(RecoveryReport {
            cleared_staging_operations: count,
            reclaimed_staging_bytes: bytes,
        })
    }

    /// Compute the deterministic eviction order once every signed retention
    /// horizon in scope has expired. Candidates are returned tier-by-tier, and
    /// deterministically ordered within each tier. Staging still inside its
    /// signed horizon (`stage_deadline > now`) is never a candidate.
    pub fn plan_eviction(&self, now: u64) -> Result<Vec<EvictionCandidate>, AnchorRepositoryError> {
        let mut candidates = Vec::new();

        // Tier 1: directory projections for communities with no live listing.
        let mut statement = self.connection.prepare(
            "SELECT di.inclusion_id FROM directory_inclusions di \
             LEFT JOIN listings l ON l.community_id = di.community_id \
             WHERE l.community_id IS NULL \
             ORDER BY di.included_at ASC, di.inclusion_id ASC",
        )?;
        for key in statement.query_map([], |row| row.get::<_, Vec<u8>>(0))? {
            candidates.push(EvictionCandidate {
                tier: EvictionTier::ExpiredProjection,
                key: key?,
            });
        }

        // Tier 2: abandoned staging past its retention horizon.
        let mut statement = self.connection.prepare(
            "SELECT operation_id FROM staged_operations WHERE stage_deadline <= ?1 \
             ORDER BY stage_deadline ASC, operation_id ASC",
        )?;
        for key in statement.query_map(params![now as i64], |row| row.get::<_, Vec<u8>>(0))? {
            candidates.push(EvictionCandidate {
                tier: EvictionTier::AbandonedStaging,
                key: key?,
            });
        }

        // Tier 3: unlisted sites, oldest first.
        let mut statement = self.connection.prepare(
            "SELECT c.community_id FROM communities c \
             LEFT JOIN listings l ON l.community_id = c.community_id \
             WHERE l.community_id IS NULL \
             ORDER BY c.created_at ASC, c.community_id ASC",
        )?;
        for key in statement.query_map([], |row| row.get::<_, Vec<u8>>(0))? {
            candidates.push(EvictionCandidate {
                tier: EvictionTier::UnlistedSite,
                key: key?,
            });
        }

        // Tier 4: listed sites, oldest successful host refresh first.
        let mut statement = self.connection.prepare(
            "SELECT community_id FROM listings \
             ORDER BY last_host_refresh_at ASC, community_id ASC",
        )?;
        for key in statement.query_map([], |row| row.get::<_, Vec<u8>>(0))? {
            candidates.push(EvictionCandidate {
                tier: EvictionTier::ListedSite,
                key: key?,
            });
        }

        Ok(candidates)
    }

    /// Open an immutable, point-in-time read snapshot. WAL keeps the reader's
    /// view consistent even while the writer commits. Only available for
    /// file-backed repositories.
    pub fn snapshot(&self) -> Result<ReadSnapshot, AnchorRepositoryError> {
        let path = self
            .db_path
            .as_ref()
            .ok_or(AnchorRepositoryError::SnapshotUnavailable)?;
        ReadSnapshot::open(path)
    }
}

/// A durable write transaction that also accumulates per-class accounting.
///
/// Charges are validated against ceilings at charge time and applied to the
/// repository ledger only on [`Self::commit`]. Dropping without committing rolls
/// back the SQLite transaction and discards the pending accounting.
pub struct RepoTransaction<'conn> {
    transaction: rusqlite::Transaction<'conn>,
    ledger: &'conn mut Ledger,
    pending: [i64; ACCOUNTING_CLASS_COUNT],
}

impl RepoTransaction<'_> {
    /// The usage a class would have if this transaction committed now
    /// (committed usage plus this transaction's pending delta).
    #[must_use]
    pub fn projected_used(&self, class: AccountingClass) -> u64 {
        let index = class.index();
        (self.ledger.used[index] as i128 + self.pending[index] as i128).max(0) as u64
    }

    /// Charge `amount` units to `class`, enforcing that class's ceiling
    /// independently. Exceeding the ceiling returns
    /// [`AnchorRepositoryError::ClassExceeded`] and touches no other class.
    pub fn charge(
        &mut self,
        class: AccountingClass,
        amount: u64,
    ) -> Result<(), AnchorRepositoryError> {
        let index = class.index();
        let ceiling = self.ledger.ceiling[index];
        let projected =
            self.ledger.used[index] as i128 + self.pending[index] as i128 + amount as i128;
        if projected > ceiling as i128 {
            return Err(AnchorRepositoryError::ClassExceeded {
                class,
                ceiling,
                used: self.projected_used(class),
                requested: amount,
            });
        }
        self.pending[index] += amount as i64;
        Ok(())
    }

    /// Release `amount` units previously charged to `class`.
    pub fn uncharge(&mut self, class: AccountingClass, amount: u64) {
        let index = class.index();
        self.pending[index] -= amount as i64;
    }

    /// Insert a community with zero logical bytes (payloads add to it).
    pub fn insert_community(
        &mut self,
        community_id: &[u8; 32],
        created_at: u64,
    ) -> Result<(), AnchorRepositoryError> {
        self.transaction.execute(
            "INSERT INTO communities(community_id, created_at, logical_bytes) VALUES (?1, ?2, 0)",
            params![community_id.as_slice(), created_at as i64],
        )?;
        Ok(())
    }

    /// Add a payload reference for a community.
    ///
    /// Physical bytes deduplicate by digest: the first community to reference a
    /// digest charges [`AccountingClass::Physical`]; later communities only bump
    /// the shared reference count. The **logical** charge is never discounted —
    /// every new per-community reference charges the full payload length to
    /// [`AccountingClass::Logical`] and to that community's `logical_bytes`.
    pub fn add_payload(
        &mut self,
        community_id: &[u8; 32],
        payload_digest: &[u8; 32],
        length: u64,
    ) -> Result<PayloadOutcome, AnchorRepositoryError> {
        let ref_exists = self
            .transaction
            .query_row(
                "SELECT 1 FROM community_payload_refs WHERE community_id = ?1 AND payload_digest = ?2",
                params![community_id.as_slice(), payload_digest.as_slice()],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        let payload_exists = self
            .transaction
            .query_row(
                "SELECT 1 FROM payloads WHERE payload_digest = ?1",
                params![payload_digest.as_slice()],
                |_| Ok(()),
            )
            .optional()?
            .is_some();

        // Physical first: the payload row must exist before any
        // `community_payload_refs` row can reference it (foreign key).
        let mut physical_charged = false;
        if payload_exists {
            self.transaction.execute(
                "UPDATE payloads SET reference_count = reference_count + 1 WHERE payload_digest = ?1",
                params![payload_digest.as_slice()],
            )?;
        } else {
            self.charge(AccountingClass::Physical, length)?;
            self.transaction.execute(
                "INSERT INTO payloads(payload_digest, payload_length, payload_bytes, reference_count) \
                 VALUES (?1, ?2, NULL, 1)",
                params![payload_digest.as_slice(), length as i64],
            )?;
            physical_charged = true;
        }

        let mut logical_charged = false;
        if !ref_exists {
            self.charge(AccountingClass::Logical, length)?;
            self.transaction.execute(
                "INSERT INTO community_payload_refs(community_id, payload_digest, logical_bytes) \
                 VALUES (?1, ?2, ?3)",
                params![
                    community_id.as_slice(),
                    payload_digest.as_slice(),
                    length as i64
                ],
            )?;
            self.transaction.execute(
                "UPDATE communities SET logical_bytes = logical_bytes + ?1 WHERE community_id = ?2",
                params![length as i64, community_id.as_slice()],
            )?;
            logical_charged = true;
        }

        Ok(PayloadOutcome {
            logical_charged,
            physical_charged,
        })
    }

    /// Claim the lowest-indexed free removal slot (deterministic), reserving it
    /// for a community/root. Returns the claimed slot index.
    pub fn claim_removal_slot(
        &mut self,
        community_id: &[u8; 32],
        root_key: &[u8; 32],
        request_digest: &[u8; 32],
    ) -> Result<u32, AnchorRepositoryError> {
        let slot: Option<u32> = self
            .transaction
            .query_row(
                "SELECT slot_index FROM removal_slots WHERE claimed_by_community IS NULL \
                 ORDER BY slot_index ASC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        let slot = slot.ok_or(AnchorRepositoryError::RemovalSlotsExhausted)?;
        self.transaction.execute(
            "UPDATE removal_slots \
             SET claimed_by_community = ?1, claimed_root_key = ?2, request_digest = ?3, removal_state = 1 \
             WHERE slot_index = ?4",
            params![
                community_id.as_slice(),
                root_key.as_slice(),
                request_digest.as_slice(),
                slot
            ],
        )?;
        Ok(slot)
    }

    /// Append an operation-private staging row and charge its bytes to
    /// [`AccountingClass::Staging`]. Staging is never query-visible.
    pub fn stage_operation(
        &mut self,
        operation_id: &[u8; 32],
        source_key: &[u8],
        staged_at: u64,
        stage_deadline: u64,
        staged_bytes: u64,
    ) -> Result<(), AnchorRepositoryError> {
        self.charge(AccountingClass::Staging, staged_bytes)?;
        self.transaction.execute(
            "INSERT INTO staged_operations(operation_id, source_key, staged_at, stage_deadline, staged_bytes) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                operation_id.as_slice(),
                source_key,
                staged_at as i64,
                stage_deadline as i64,
                staged_bytes as i64
            ],
        )?;
        Ok(())
    }

    /// Insert a current listing row bound to a claimed removal slot.
    #[allow(clippy::too_many_arguments)]
    pub fn insert_listing(
        &mut self,
        community_id: &[u8; 32],
        root_key: &[u8; 32],
        listed_at: u64,
        expires_at: u64,
        last_host_refresh_at: u64,
        removal_slot_index: u32,
    ) -> Result<(), AnchorRepositoryError> {
        self.transaction.execute(
            "INSERT INTO listings(community_id, root_key, listed_at, expires_at, last_host_refresh_at, removal_slot_index) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                community_id.as_slice(),
                root_key.as_slice(),
                listed_at as i64,
                expires_at as i64,
                last_host_refresh_at as i64,
                removal_slot_index
            ],
        )?;
        Ok(())
    }

    /// Insert a signed directory inclusion record for a community.
    pub fn insert_directory_inclusion(
        &mut self,
        inclusion_id: &[u8; 32],
        community_id: &[u8; 32],
        included_at: u64,
        record_bytes: &[u8],
    ) -> Result<(), AnchorRepositoryError> {
        self.transaction.execute(
            "INSERT INTO directory_inclusions(inclusion_id, community_id, included_at, record_bytes) \
             VALUES (?1, ?2, ?3, ?4)",
            params![
                inclusion_id.as_slice(),
                community_id.as_slice(),
                included_at as i64,
                record_bytes
            ],
        )?;
        Ok(())
    }

    /// Commit the SQLite transaction and durably apply the pending accounting.
    pub fn commit(self) -> Result<(), AnchorRepositoryError> {
        let RepoTransaction {
            transaction,
            ledger,
            pending,
        } = self;
        transaction.commit()?;
        for (used, delta) in ledger.used.iter_mut().zip(pending.iter()) {
            *used = (*used as i64 + *delta).max(0) as u64;
        }
        Ok(())
    }
}

/// An immutable, point-in-time read snapshot backed by its own read-only
/// connection and an open deferred transaction. WAL keeps the reader's view
/// fixed even while the writer commits new state.
pub struct ReadSnapshot {
    connection: Connection,
}

impl ReadSnapshot {
    fn open(path: &Path) -> Result<Self, AnchorRepositoryError> {
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
        )?;
        connection.pragma_update(None, "foreign_keys", true)?;
        // Start the read transaction and prime it so WAL fixes the snapshot at
        // this instant; later writer commits will not be visible to it.
        connection.execute_batch("BEGIN DEFERRED")?;
        let _: i64 =
            connection.query_row("SELECT COUNT(*) FROM communities", [], |row| row.get(0))?;
        Ok(Self { connection })
    }

    /// The community's logical bytes as of this snapshot, or `None` if the
    /// community did not exist when the snapshot was taken.
    pub fn community_logical_bytes(
        &self,
        community_id: &[u8; 32],
    ) -> Result<Option<u64>, AnchorRepositoryError> {
        let value: Option<i64> = self
            .connection
            .query_row(
                "SELECT logical_bytes FROM communities WHERE community_id = ?1",
                params![community_id.as_slice()],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value.map(|bytes| bytes.max(0) as u64))
    }

    /// The number of communities visible in this snapshot.
    pub fn community_count(&self) -> Result<u64, AnchorRepositoryError> {
        let count: i64 =
            self.connection
                .query_row("SELECT COUNT(*) FROM communities", [], |row| row.get(0))?;
        Ok(count.max(0) as u64)
    }
}

impl Drop for ReadSnapshot {
    fn drop(&mut self) {
        // End the read transaction; ignore errors during teardown.
        let _ = self.connection.execute_batch("ROLLBACK");
    }
}
