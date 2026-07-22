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

use riot_anchor_protocol::authority::{AuthorityClass, ListingFloor};

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
    /// The database is already bound to a different operator identity.
    OperatorIdentityMismatch,
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
            Self::OperatorIdentityMismatch => {
                write!(formatter, "anchor database operator identity mismatch")
            }
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

/// The claim state of an idempotency-index row (design "Claimed | Prepared |
/// Terminal").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdempotencyClaimState {
    /// A winning claim with a 30-second lease and no result yet.
    Claimed,
    /// The claim created a long-running operation and points at it.
    Prepared,
    /// The claim reached a byte-identical terminal outcome.
    Terminal,
}

impl IdempotencyClaimState {
    fn to_code(self) -> i64 {
        match self {
            IdempotencyClaimState::Claimed => 0,
            IdempotencyClaimState::Prepared => 1,
            IdempotencyClaimState::Terminal => 2,
        }
    }

    fn from_code(code: i64) -> Option<Self> {
        match code {
            0 => Some(IdempotencyClaimState::Claimed),
            1 => Some(IdempotencyClaimState::Prepared),
            2 => Some(IdempotencyClaimState::Terminal),
            _ => None,
        }
    }
}

/// A retained idempotency-index row. The `control_request_digest` is the retained
/// digest a replay must match exactly; an unequal digest under the same key is
/// `idempotency_conflict` (the caller compares, never this layer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdempotencyRow {
    /// The retained `control_request_digest` bound to this key.
    pub control_request_digest: [u8; 32],
    /// `0` ordinary, `1` reserved-removal partition.
    pub result_class: u8,
    /// The claim's lifecycle state.
    pub claim_state: IdempotencyClaimState,
    /// The long-running operation this claim created, if `Prepared`/`Terminal`.
    pub operation_id: Option<[u8; 32]>,
    /// The `Claimed` lease expiry, if any.
    pub lease_expires_at: Option<u64>,
}

/// The originating long-running Prepare kind of an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationKind {
    /// `prepare_host`.
    Host,
    /// `prepare_replica`.
    Replica,
}

impl OperationKind {
    fn to_code(self) -> i64 {
        match self {
            OperationKind::Host => 0,
            OperationKind::Replica => 1,
        }
    }

    fn from_code(code: i64) -> Option<Self> {
        match code {
            0 => Some(OperationKind::Host),
            1 => Some(OperationKind::Replica),
            _ => None,
        }
    }
}

/// The lifecycle status of a stored operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationStatus {
    /// Prepared and actively staged.
    Prepared,
    /// Committed with a retained receipt.
    Committed,
    /// Terminally refused.
    Refused,
}

impl OperationStatus {
    fn to_code(self) -> i64 {
        match self {
            OperationStatus::Prepared => 0,
            OperationStatus::Committed => 1,
            OperationStatus::Refused => 2,
        }
    }

    fn from_code(code: i64) -> Option<Self> {
        match code {
            0 => Some(OperationStatus::Prepared),
            1 => Some(OperationStatus::Committed),
            2 => Some(OperationStatus::Refused),
            _ => None,
        }
    }
}

/// A durable long-running operation record (the Prepare lifecycle row).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredOperation {
    /// The stable 256-bit operation id.
    pub operation_id: [u8; 32],
    /// The originating Prepare kind.
    pub originating_kind: OperationKind,
    /// The token-secret epoch the namespace tokens were derived under.
    pub token_secret_epoch: u32,
    /// The captured base site generation.
    pub base_generation: u64,
    /// The current lifecycle status.
    pub status: OperationStatus,
    /// The operation expiry (Unix seconds); tokens are accepted only before it.
    pub operation_expiry: u64,
    /// The retention deadline (`operation_expiry + 24h`) after which the mapping
    /// is reclaimable.
    pub retention_deadline: u64,
    /// The exact canonical `ControlResponseV1(prepare success)` bytes.
    pub prepare_response_bytes: Vec<u8>,
    /// The exact canonical terminal outcome bytes, if terminalized.
    pub terminal_result_bytes: Option<Vec<u8>>,
    /// The exact canonical root-signed ticket envelope PrepareHost admitted —
    /// the ONLY ticket source the composite Commit's manifest resolution accepts.
    /// `None` on a pre-migration row (fails closed at commit).
    pub ticket_envelope_bytes: Option<Vec<u8>>,
}

/// The fields required to atomically create a prepared operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPreparedOperation {
    /// The stable operation id.
    pub operation_id: [u8; 32],
    /// The originating Prepare kind.
    pub originating_kind: OperationKind,
    /// The token-secret epoch.
    pub token_secret_epoch: u32,
    /// The captured base site generation.
    pub base_generation: u64,
    /// The creation time (Unix seconds).
    pub created_at: u64,
    /// The operation expiry (Unix seconds).
    pub operation_expiry: u64,
    /// The retention deadline (Unix seconds).
    pub retention_deadline: u64,
    /// The exact canonical prepared response bytes.
    pub prepare_response_bytes: Vec<u8>,
}

/// One committed entry as `(entry_id, item_bytes)` — the sortable sync inventory
/// id and the full anchor-profile item a `sync/2` sender streams.
pub type CommittedEntry = (Vec<u8>, Vec<u8>);

/// A community's committed manifest row as `(manifest_generation,
/// manifest_digest, manifest_bytes)`.
pub type CommittedManifest = (u64, [u8; 32], Vec<u8>);

/// A direction-private staged entry (or, once promoted, a committed entry). The
/// `item_bytes` are the exact anchor-profile encoded item (entry + capability +
/// signature + payload) whose length is the entry's logical contribution to a
/// namespace snapshot digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedEntry {
    /// The namespace this entry belongs to.
    pub namespace_id: [u8; 32],
    /// The full canonical entry id (the sortable sync inventory id).
    pub entry_id: [u8; 32],
    /// The entry's subspace id.
    pub subspace_id: [u8; 32],
    /// The entry's canonical path bytes.
    pub path_bytes: Vec<u8>,
    /// The entry's Willow timestamp, big-endian.
    pub timestamp_be: [u8; 8],
    /// The entry's payload digest.
    pub payload_digest: [u8; 32],
    /// The entry's payload length.
    pub payload_length: u64,
    /// The canonical `Entry` bytes.
    pub entry_bytes: Vec<u8>,
    /// The full anchor-profile item bytes (entry + proofs + payload).
    pub item_bytes: Vec<u8>,
}

/// Outcome of a generation compare-and-swap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerationCas {
    /// The swap won: the base equalled the current generation and it advanced.
    Committed,
    /// The swap lost: the current generation did not equal the operation base.
    Stale {
        /// The current committed generation that blocked the swap.
        current_generation: u64,
    },
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

    /// Return the canonical descriptor already bound to this database, or
    /// atomically persist `proposed_descriptor` on first startup.
    ///
    /// Once initialized, the operator key id is immutable. This prevents a
    /// restored database from silently becoming a different anchor identity.
    pub fn load_or_initialize_descriptor(
        &mut self,
        operator_key_id: &[u8; 32],
        proposed_descriptor: &[u8],
    ) -> Result<Vec<u8>, AnchorRepositoryError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (stored_key, stored_descriptor): (Option<Vec<u8>>, Option<Vec<u8>>) = transaction
            .query_row(
                "SELECT operator_key_id, descriptor_bytes
                 FROM operator_state WHERE singleton = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;

        if let Some(stored_key) = &stored_key {
            if stored_key.as_slice() != operator_key_id {
                return Err(AnchorRepositoryError::OperatorIdentityMismatch);
            }
        }

        if let Some(stored_descriptor) = stored_descriptor {
            transaction.commit()?;
            return Ok(stored_descriptor);
        }

        transaction.execute(
            "UPDATE operator_state
             SET operator_key_id = ?1, descriptor_bytes = ?2
             WHERE singleton = 1",
            params![operator_key_id.as_slice(), proposed_descriptor],
        )?;
        transaction.commit()?;
        Ok(proposed_descriptor.to_vec())
    }

    /// Return the database-durable secret bound to `name`, or atomically
    /// persist `proposed` on first startup (same shape as
    /// [`Self::load_or_initialize_descriptor`]).
    ///
    /// First write wins: once a name is initialized, the persisted value is
    /// returned and the proposal ignored. This binds the anchor's genesis
    /// random and namespace-token secret to the DATABASE, so an operator-key
    /// rotation cannot silently change the anchor id or orphan minted tokens.
    pub fn load_or_initialize_secret(
        &mut self,
        name: &str,
        proposed: &[u8; 32],
    ) -> Result<[u8; 32], AnchorRepositoryError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let stored: Option<Vec<u8>> = transaction
            .query_row(
                "SELECT secret FROM anchor_secrets WHERE name = ?1",
                params![name],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(stored) = stored {
            transaction.commit()?;
            let mut secret = [0u8; 32];
            secret.copy_from_slice(&stored);
            return Ok(secret);
        }
        transaction.execute(
            "INSERT INTO anchor_secrets(name, secret) VALUES (?1, ?2)",
            params![name, proposed.as_slice()],
        )?;
        transaction.commit()?;
        Ok(*proposed)
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

    /// Load a stored operation by its stable id (read-only). `GetOperation` uses
    /// this; it never looks up an idempotency key.
    pub fn load_operation(
        &self,
        operation_id: &[u8; 32],
    ) -> Result<Option<StoredOperation>, AnchorRepositoryError> {
        self.connection
            .query_row(
                "SELECT operation_id, originating_kind, token_secret_epoch, base_generation, \
                 operation_status, operation_expiry, retention_deadline, prepare_response_bytes, \
                 terminal_result_bytes, ticket_envelope_bytes \
                 FROM operations WHERE operation_id = ?1",
                params![operation_id.as_slice()],
                map_stored_operation,
            )
            .optional()
            .map_err(AnchorRepositoryError::from)
    }

    /// The community's current committed `site_generation`, or `None` if the
    /// community has never been hosted (no row). A `None` reads as generation `0`
    /// for a first-host compare-and-swap.
    pub fn site_generation(
        &self,
        community_id: &[u8; 32],
    ) -> Result<Option<u64>, AnchorRepositoryError> {
        let value: Option<i64> = self
            .connection
            .query_row(
                "SELECT site_generation FROM communities WHERE community_id = ?1",
                params![community_id.as_slice()],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value.map(|generation| generation.max(0) as u64))
    }

    /// All direction-private staged entries for an operation's namespace, ordered
    /// by ascending entry id (the sync inventory order).
    pub fn staged_entries(
        &self,
        operation_id: &[u8; 32],
        namespace_id: &[u8; 32],
    ) -> Result<Vec<StagedEntry>, AnchorRepositoryError> {
        let mut statement = self.connection.prepare(
            "SELECT namespace_id, entry_id, subspace_id, path_bytes, timestamp_be, \
             payload_digest, payload_length, entry_bytes, item_bytes FROM staged_entries \
             WHERE operation_id = ?1 AND namespace_id = ?2 ORDER BY entry_id ASC",
        )?;
        let rows = statement.query_map(
            params![operation_id.as_slice(), namespace_id.as_slice()],
            map_staged_entry,
        )?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    /// Every committed entry of a namespace as `(entry_id, item_bytes)`, ordered
    /// by ascending entry id (the sync inventory order). The `item_bytes` are the
    /// full anchor-profile item a `sync/2` sender streams to a reader.
    pub fn committed_entries(
        &self,
        namespace_id: &[u8; 32],
    ) -> Result<Vec<CommittedEntry>, AnchorRepositoryError> {
        let mut statement = self.connection.prepare(
            "SELECT entry_id, item_bytes FROM entries WHERE namespace_id = ?1 ORDER BY entry_id ASC",
        )?;
        let rows = statement.query_map(params![namespace_id.as_slice()], |row| {
            Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Vec<u8>>(1)?))
        })?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    /// The count of committed entries in a namespace (`0` if the namespace row is
    /// absent).
    pub fn committed_entry_count(
        &self,
        namespace_id: &[u8; 32],
    ) -> Result<u64, AnchorRepositoryError> {
        let count: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM entries WHERE namespace_id = ?1",
            params![namespace_id.as_slice()],
            |row| row.get(0),
        )?;
        Ok(count.max(0) as u64)
    }

    /// A stored hosting receipt's exact bytes, by receipt id.
    pub fn hosting_receipt(
        &self,
        receipt_id: &[u8; 32],
    ) -> Result<Option<Vec<u8>>, AnchorRepositoryError> {
        self.connection
            .query_row(
                "SELECT receipt_bytes FROM hosting_receipts WHERE receipt_id = ?1",
                params![receipt_id.as_slice()],
                |row| row.get(0),
            )
            .optional()
            .map_err(AnchorRepositoryError::from)
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

    /// The highest root-signed public-site ticket transport epoch this anchor
    /// has successfully admitted for `root_id`.
    pub fn highest_ticket_transport_epoch(
        &self,
        root_id: &[u8; 32],
    ) -> Result<Option<u32>, AnchorRepositoryError> {
        let stored: Option<i64> = self
            .transaction
            .query_row(
                "SELECT highest_transport_epoch FROM ticket_transport_floors WHERE root_id = ?1",
                params![root_id.as_slice()],
                |row| row.get(0),
            )
            .optional()?;
        Ok(stored.map(|epoch| epoch as u32))
    }

    /// Advance the durable transport epoch floor for `root_id`.
    ///
    /// This is called in the same transaction that persists PrepareHost, so a
    /// crash can expose neither an operation without its rollback floor nor a
    /// floor for an operation that never committed.
    pub fn advance_ticket_transport_epoch(
        &mut self,
        root_id: &[u8; 32],
        transport_epoch: u32,
    ) -> Result<(), AnchorRepositoryError> {
        self.transaction.execute(
            "INSERT INTO ticket_transport_floors(root_id, highest_transport_epoch)
             VALUES (?1, ?2)
             ON CONFLICT(root_id) DO UPDATE SET
                 highest_transport_epoch = MAX(
                     ticket_transport_floors.highest_transport_epoch,
                     excluded.highest_transport_epoch
                 )",
            params![root_id.as_slice(), i64::from(transport_epoch)],
        )?;
        Ok(())
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

    /// Load the durable per-root listing floor (WU-015B), read within this
    /// transaction. `None` means nothing has ever been admitted for this root; the
    /// caller starts from [`ListingFloor::new`].
    pub fn load_listing_floor(
        &self,
        root_id: &[u8; 32],
    ) -> Result<Option<ListingFloor>, AnchorRepositoryError> {
        self.transaction
            .query_row(
                "SELECT listing_epoch, sealed, highest_revision, shown_digest, shown_class, \
                 equivocated FROM listing_floors WHERE root_id = ?1",
                params![root_id.as_slice()],
                |row| map_listing_floor(*root_id, row),
            )
            .optional()
            .map_err(AnchorRepositoryError::from)
    }

    /// Persist the durable per-root listing floor (insert or replace). The floor
    /// never rolls backward; the caller supplies the resolved next floor.
    pub fn upsert_listing_floor(
        &mut self,
        floor: &ListingFloor,
    ) -> Result<(), AnchorRepositoryError> {
        let shown_class = floor.shown_class.map(authority_class_code);
        self.transaction.execute(
            "INSERT INTO listing_floors(root_id, listing_epoch, sealed, highest_revision, \
             shown_digest, shown_class, equivocated) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
             ON CONFLICT(root_id) DO UPDATE SET listing_epoch = excluded.listing_epoch, \
             sealed = excluded.sealed, highest_revision = excluded.highest_revision, \
             shown_digest = excluded.shown_digest, shown_class = excluded.shown_class, \
             equivocated = excluded.equivocated",
            params![
                floor.root_id.as_slice(),
                floor.epoch as i64,
                floor.sealed as i64,
                floor.highest_revision as i64,
                floor.shown_digest.map(|d| d.to_vec()),
                shown_class,
                floor.equivocated as i64
            ],
        )?;
        Ok(())
    }

    /// The current listing row for a community, if it is listed.
    pub fn current_listing(
        &self,
        community_id: &[u8; 32],
    ) -> Result<Option<CurrentListing>, AnchorRepositoryError> {
        self.transaction
            .query_row(
                "SELECT root_key, listed_at, expires_at, last_host_refresh_at, removal_slot_index \
                 FROM listings WHERE community_id = ?1",
                params![community_id.as_slice()],
                map_current_listing,
            )
            .optional()
            .map_err(AnchorRepositoryError::from)
    }

    /// Replace a listing's current visibility state (refresh), retaining its
    /// already-reserved removal slot and the signed feed history.
    pub fn update_listing(
        &mut self,
        community_id: &[u8; 32],
        root_key: &[u8; 32],
        listed_at: u64,
        expires_at: u64,
        last_host_refresh_at: u64,
    ) -> Result<(), AnchorRepositoryError> {
        self.transaction.execute(
            "UPDATE listings SET root_key = ?2, listed_at = ?3, expires_at = ?4, \
             last_host_refresh_at = ?5 WHERE community_id = ?1",
            params![
                community_id.as_slice(),
                root_key.as_slice(),
                listed_at as i64,
                expires_at as i64,
                last_host_refresh_at as i64
            ],
        )?;
        Ok(())
    }

    /// The current directory-feed head `(head_inclusion_digest, feed_length)`.
    /// A never-advanced feed reports `(ZERO, 0)`.
    pub fn feed_head(&self) -> Result<([u8; 32], u64), AnchorRepositoryError> {
        let head = self
            .transaction
            .query_row(
                "SELECT head_digest, feed_length FROM directory_feed_heads WHERE feed_id = 1",
                [],
                |row| {
                    let digest = blob32(row.get::<_, Vec<u8>>(0)?)?;
                    let length: i64 = row.get(1)?;
                    Ok((digest, length.max(0) as u64))
                },
            )
            .optional()?;
        Ok(head.unwrap_or(([0u8; 32], 0)))
    }

    /// Advance the singleton directory-feed head by exactly one inclusion, setting
    /// the new head digest and returning the new monotonic `feed_length` (the
    /// admitted inclusion's sequence / `feed_coordinate`).
    pub fn advance_feed_head(
        &mut self,
        new_head_digest: &[u8; 32],
        updated_at: u64,
    ) -> Result<u64, AnchorRepositoryError> {
        let (_, current_length) = self.feed_head()?;
        let next_length = current_length.saturating_add(1);
        self.transaction.execute(
            "INSERT INTO directory_feed_heads(feed_id, head_digest, feed_length, updated_at) \
             VALUES (1, ?1, ?2, ?3) ON CONFLICT(feed_id) DO UPDATE SET \
             head_digest = excluded.head_digest, feed_length = excluded.feed_length, \
             updated_at = excluded.updated_at",
            params![
                new_head_digest.as_slice(),
                next_length as i64,
                updated_at as i64
            ],
        )?;
        Ok(next_length)
    }

    /// Set the singleton directory-feed head to an explicit digest and length (the
    /// logical floor a checkpoint advances to). Used only by crash-safe checkpoint
    /// advancement, which supplies a frozen head — never a re-invented one.
    pub fn advance_feed_head_to(
        &mut self,
        head_digest: &[u8; 32],
        feed_length: u64,
        updated_at: u64,
    ) -> Result<(), AnchorRepositoryError> {
        self.transaction.execute(
            "INSERT INTO directory_feed_heads(feed_id, head_digest, feed_length, updated_at) \
             VALUES (1, ?1, ?2, ?3) ON CONFLICT(feed_id) DO UPDATE SET \
             head_digest = excluded.head_digest, feed_length = excluded.feed_length, \
             updated_at = excluded.updated_at",
            params![
                head_digest.as_slice(),
                feed_length as i64,
                updated_at as i64
            ],
        )?;
        Ok(())
    }

    /// The current directory/search projection generation (0 if never invalidated).
    pub fn projection_generation(&self) -> Result<u64, AnchorRepositoryError> {
        let generation: Option<i64> = self
            .transaction
            .query_row(
                "SELECT generation FROM directory_projection WHERE projection_id = 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(generation.unwrap_or(0).max(0) as u64)
    }

    /// Invalidate (bump) the directory/search projection generation by exactly one
    /// and return the new value. This is the single projection-invalidation event a
    /// visibility-changing listing transaction performs.
    pub fn invalidate_projection_generation(&mut self) -> Result<u64, AnchorRepositoryError> {
        self.transaction.execute(
            "INSERT INTO directory_projection(projection_id, generation) VALUES (1, 1) \
             ON CONFLICT(projection_id) DO UPDATE SET generation = generation + 1",
            [],
        )?;
        self.projection_generation()
    }

    /// The number of retained signed directory inclusions for a community (its
    /// signed feed history).
    pub fn directory_inclusion_count(
        &self,
        community_id: &[u8; 32],
    ) -> Result<u64, AnchorRepositoryError> {
        let count: i64 = self.transaction.query_row(
            "SELECT COUNT(*) FROM directory_inclusions WHERE community_id = ?1",
            params![community_id.as_slice()],
            |row| row.get(0),
        )?;
        Ok(count.max(0) as u64)
    }

    /// Look up an idempotency-index row by its 128-bit key (read within this
    /// transaction). This is the constant-time-lookup input to the admission
    /// state machine; the caller compares the retained digest.
    pub fn lookup_idempotency(
        &self,
        idempotency_key: &[u8; 16],
    ) -> Result<Option<IdempotencyRow>, AnchorRepositoryError> {
        self.transaction
            .query_row(
                "SELECT control_request_digest, result_class, claim_state, operation_id, \
                 lease_expires_at FROM idempotency_key_index WHERE idempotency_key = ?1",
                params![idempotency_key.as_slice()],
                map_idempotency_row,
            )
            .optional()
            .map_err(AnchorRepositoryError::from)
    }

    /// Atomically claim an idempotency key: insert the retained digest and state,
    /// charging one row to [`AccountingClass::Idempotency`]. This is the durable
    /// claim boundary — every cheap and expensive admission check must have
    /// already passed.
    #[allow(clippy::too_many_arguments)]
    pub fn claim_idempotency(
        &mut self,
        control_request_digest: &[u8; 32],
        idempotency_key: &[u8; 16],
        result_class: u8,
        claim_state: IdempotencyClaimState,
        operation_id: Option<&[u8; 32]>,
        lease_expires_at: Option<u64>,
        created_at: u64,
        expires_at: u64,
    ) -> Result<(), AnchorRepositoryError> {
        self.charge(AccountingClass::Idempotency, 1)?;
        self.transaction.execute(
            "INSERT INTO idempotency_key_index(control_request_digest, idempotency_key, result_class, \
             claim_state, operation_id, lease_expires_at, created_at, expires_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                control_request_digest.as_slice(),
                idempotency_key.as_slice(),
                result_class as i64,
                claim_state.to_code(),
                operation_id.map(|id| id.to_vec()),
                lease_expires_at.map(|value| value as i64),
                created_at as i64,
                expires_at as i64
            ],
        )?;
        Ok(())
    }

    /// Atomically claim an idempotency key in the **reserved removal partition**
    /// (`result_class = 1`). Unlike [`Self::claim_idempotency`], this never charges
    /// the ordinary [`AccountingClass::Idempotency`] row ceiling: the index is sized
    /// for the ordinary ceiling *plus* `2 * L` exclusively reserved entries, so an
    /// owner removal can claim its key even when ordinary idempotency rows are
    /// exhausted. A reserved key and an ordinary key can never collide (both share
    /// the same unique key column) and neither discloses the other's stored result.
    #[allow(clippy::too_many_arguments)]
    pub fn claim_idempotency_reserved(
        &mut self,
        control_request_digest: &[u8; 32],
        idempotency_key: &[u8; 16],
        claim_state: IdempotencyClaimState,
        created_at: u64,
        expires_at: u64,
    ) -> Result<(), AnchorRepositoryError> {
        self.transaction.execute(
            "INSERT INTO idempotency_key_index(control_request_digest, idempotency_key, result_class, \
             claim_state, operation_id, lease_expires_at, created_at, expires_at) \
             VALUES (?1, ?2, 1, ?3, NULL, NULL, ?4, ?5)",
            params![
                control_request_digest.as_slice(),
                idempotency_key.as_slice(),
                claim_state.to_code(),
                created_at as i64,
                expires_at as i64
            ],
        )?;
        Ok(())
    }

    /// Insert a prepared operation row (the Prepare lifecycle record). Charges
    /// nothing to staging here: WU-014 stages no bytes; sync bytes attach later.
    pub fn insert_operation(
        &mut self,
        operation: &NewPreparedOperation,
    ) -> Result<(), AnchorRepositoryError> {
        self.transaction.execute(
            "INSERT INTO operations(operation_id, originating_kind, token_secret_epoch, \
             base_generation, operation_status, created_at, operation_expiry, retention_deadline, \
             prepare_response_bytes, terminal_result_bytes) \
             VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6, ?7, ?8, NULL)",
            params![
                operation.operation_id.as_slice(),
                operation.originating_kind.to_code(),
                operation.token_secret_epoch as i64,
                operation.base_generation as i64,
                operation.created_at as i64,
                operation.operation_expiry as i64,
                operation.retention_deadline as i64,
                operation.prepare_response_bytes.as_slice()
            ],
        )?;
        Ok(())
    }

    /// Load a stored operation within this transaction (used to reconstruct a
    /// byte-identical Prepared replay before rolling back).
    pub fn load_operation(
        &self,
        operation_id: &[u8; 32],
    ) -> Result<Option<StoredOperation>, AnchorRepositoryError> {
        self.transaction
            .query_row(
                "SELECT operation_id, originating_kind, token_secret_epoch, base_generation, \
                 operation_status, operation_expiry, retention_deadline, prepare_response_bytes, \
                 terminal_result_bytes, ticket_envelope_bytes \
                 FROM operations WHERE operation_id = ?1",
                params![operation_id.as_slice()],
                map_stored_operation,
            )
            .optional()
            .map_err(AnchorRepositoryError::from)
    }

    /// Persist the exact canonical root-signed ticket envelope on a prepared
    /// operation row. `PrepareHost` calls this in ITS transaction; the composite
    /// Commit's manifest resolution reads the bytes back as the ONLY legal ticket
    /// source (a client-supplied ticket at commit would enable substitution).
    pub fn store_operation_ticket(
        &mut self,
        operation_id: &[u8; 32],
        ticket_envelope_bytes: &[u8],
    ) -> Result<(), AnchorRepositoryError> {
        self.transaction.execute(
            "UPDATE operations SET ticket_envelope_bytes = ?2 WHERE operation_id = ?1",
            params![operation_id.as_slice(), ticket_envelope_bytes],
        )?;
        Ok(())
    }

    /// Terminalize an operation: record its exact terminal outcome bytes and flip
    /// both the operation status and its idempotency mapping to `Terminal`. Used
    /// by session-close / security-exception handling.
    pub fn terminalize_operation(
        &mut self,
        operation_id: &[u8; 32],
        status: OperationStatus,
        terminal_result_bytes: &[u8],
    ) -> Result<(), AnchorRepositoryError> {
        self.transaction.execute(
            "UPDATE operations SET operation_status = ?2, terminal_result_bytes = ?3 \
             WHERE operation_id = ?1",
            params![
                operation_id.as_slice(),
                status.to_code(),
                terminal_result_bytes
            ],
        )?;
        self.transaction.execute(
            "UPDATE idempotency_key_index SET claim_state = ?2 WHERE operation_id = ?1",
            params![
                operation_id.as_slice(),
                IdempotencyClaimState::Terminal.to_code()
            ],
        )?;
        Ok(())
    }

    /// Ensure a direction-private staging row exists for an operation (idempotent
    /// across the operation's three namespace sessions). Charges nothing; per-entry
    /// bytes are charged by [`Self::stage_entry`].
    pub fn ensure_staged_operation(
        &mut self,
        operation_id: &[u8; 32],
        source_key: &[u8],
        staged_at: u64,
        stage_deadline: u64,
    ) -> Result<(), AnchorRepositoryError> {
        self.transaction.execute(
            "INSERT OR IGNORE INTO staged_operations(operation_id, source_key, staged_at, stage_deadline, staged_bytes) \
             VALUES (?1, ?2, ?3, ?4, 0)",
            params![
                operation_id.as_slice(),
                source_key,
                staged_at as i64,
                stage_deadline as i64
            ],
        )?;
        Ok(())
    }

    /// Admit one entry into an operation's direction-private staging, charging its
    /// full item bytes to [`AccountingClass::Staging`] and bumping the operation's
    /// staged-byte tally. The staged rows are never query-visible outside the
    /// operation until the composite Commit promotes them.
    pub fn stage_entry(
        &mut self,
        operation_id: &[u8; 32],
        entry: &StagedEntry,
    ) -> Result<(), AnchorRepositoryError> {
        let item_len = entry.item_bytes.len() as u64;
        self.charge(AccountingClass::Staging, item_len)?;
        self.transaction.execute(
            "INSERT INTO staged_entries(operation_id, namespace_id, entry_id, subspace_id, \
             path_bytes, timestamp_be, payload_digest, payload_length, entry_bytes, item_bytes) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                operation_id.as_slice(),
                entry.namespace_id.as_slice(),
                entry.entry_id.as_slice(),
                entry.subspace_id.as_slice(),
                entry.path_bytes.as_slice(),
                entry.timestamp_be.as_slice(),
                entry.payload_digest.as_slice(),
                entry.payload_length as i64,
                entry.entry_bytes.as_slice(),
                entry.item_bytes.as_slice()
            ],
        )?;
        self.transaction.execute(
            "UPDATE staged_operations SET staged_bytes = staged_bytes + ?1 WHERE operation_id = ?2",
            params![item_len as i64, operation_id.as_slice()],
        )?;
        Ok(())
    }

    /// The direction-private staged entries for an operation's namespace, ordered
    /// by ascending entry id, read within this transaction.
    pub fn staged_entries(
        &self,
        operation_id: &[u8; 32],
        namespace_id: &[u8; 32],
    ) -> Result<Vec<StagedEntry>, AnchorRepositoryError> {
        let mut statement = self.transaction.prepare(
            "SELECT namespace_id, entry_id, subspace_id, path_bytes, timestamp_be, \
             payload_digest, payload_length, entry_bytes, item_bytes FROM staged_entries \
             WHERE operation_id = ?1 AND namespace_id = ?2 ORDER BY entry_id ASC",
        )?;
        let rows = statement.query_map(
            params![operation_id.as_slice(), namespace_id.as_slice()],
            map_staged_entry,
        )?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    /// Every committed entry of a namespace as `(entry_id, item_bytes)`, ordered by
    /// ascending entry id, read within this transaction.
    pub fn committed_entries(
        &self,
        namespace_id: &[u8; 32],
    ) -> Result<Vec<CommittedEntry>, AnchorRepositoryError> {
        let mut statement = self.transaction.prepare(
            "SELECT entry_id, item_bytes FROM entries WHERE namespace_id = ?1 ORDER BY entry_id ASC",
        )?;
        let rows = statement.query_map(params![namespace_id.as_slice()], |row| {
            Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Vec<u8>>(1)?))
        })?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    /// Every committed entry of a namespace at an exact canonical path, as
    /// `(entry_id, item_bytes)` ordered by ascending entry id. The composite
    /// Commit's manifest resolution unions this with the operation's staged
    /// `/manifest` candidates (a refresh commit resolves from committed `O`).
    pub fn committed_entries_by_path(
        &self,
        namespace_id: &[u8; 32],
        path_bytes: &[u8],
    ) -> Result<Vec<CommittedEntry>, AnchorRepositoryError> {
        let mut statement = self.transaction.prepare(
            "SELECT entry_id, item_bytes FROM entries \
             WHERE namespace_id = ?1 AND path_bytes = ?2 ORDER BY entry_id ASC",
        )?;
        let rows = statement.query_map(params![namespace_id.as_slice(), path_bytes], |row| {
            Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Vec<u8>>(1)?))
        })?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    /// Upsert a community's committed manifest row for one manifest generation
    /// (the manifest's own version). Written inside the composite commit
    /// transaction, after the generation CAS has ensured the community row.
    pub fn upsert_manifest(
        &mut self,
        community_id: &[u8; 32],
        manifest_generation: u64,
        manifest_digest: &[u8; 32],
        manifest_bytes: &[u8],
    ) -> Result<(), AnchorRepositoryError> {
        self.transaction.execute(
            "INSERT INTO manifests(community_id, manifest_generation, manifest_digest, manifest_bytes) \
             VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT(community_id, manifest_generation) DO UPDATE SET \
                 manifest_digest = excluded.manifest_digest, \
                 manifest_bytes = excluded.manifest_bytes",
            params![
                community_id.as_slice(),
                manifest_generation as i64,
                manifest_digest.as_slice(),
                manifest_bytes
            ],
        )?;
        Ok(())
    }

    /// The community's committed manifest with the HIGHEST generation, as
    /// `(manifest_generation, manifest_digest, manifest_bytes)`. `ReadCommitted`
    /// equality and `SubmitListing` read from this.
    pub fn committed_manifest(
        &self,
        community_id: &[u8; 32],
    ) -> Result<Option<CommittedManifest>, AnchorRepositoryError> {
        let row = self
            .transaction
            .query_row(
                "SELECT manifest_generation, manifest_digest, manifest_bytes FROM manifests \
                 WHERE community_id = ?1 ORDER BY manifest_generation DESC LIMIT 1",
                params![community_id.as_slice()],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, Vec<u8>>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                    ))
                },
            )
            .optional()?;
        match row {
            None => Ok(None),
            Some((generation, digest, bytes)) => {
                Ok(Some((generation.max(0) as u64, blob32(digest)?, bytes)))
            }
        }
    }

    /// Advance the durable manifest rollback floor for a community. Monotonic —
    /// mirrors [`Self::advance_ticket_transport_epoch`]: a lower (or equal)
    /// generation never moves the floor or its digest backward.
    pub fn advance_manifest_floor(
        &mut self,
        community_id: &[u8; 32],
        min_manifest_generation: u64,
        min_manifest_digest: &[u8; 32],
    ) -> Result<(), AnchorRepositoryError> {
        self.transaction.execute(
            "INSERT INTO manifest_floors(community_id, min_manifest_generation, min_manifest_digest) \
             VALUES (?1, ?2, ?3) \
             ON CONFLICT(community_id) DO UPDATE SET \
                 min_manifest_digest = CASE \
                     WHEN excluded.min_manifest_generation > manifest_floors.min_manifest_generation \
                     THEN excluded.min_manifest_digest \
                     ELSE manifest_floors.min_manifest_digest END, \
                 min_manifest_generation = MAX( \
                     manifest_floors.min_manifest_generation, \
                     excluded.min_manifest_generation)",
            params![
                community_id.as_slice(),
                min_manifest_generation as i64,
                min_manifest_digest.as_slice()
            ],
        )?;
        Ok(())
    }

    /// The community's durable manifest floor as `(min_manifest_generation,
    /// min_manifest_digest)`, if one has been established.
    pub fn manifest_floor(
        &self,
        community_id: &[u8; 32],
    ) -> Result<Option<(u64, [u8; 32])>, AnchorRepositoryError> {
        let row = self
            .transaction
            .query_row(
                "SELECT min_manifest_generation, min_manifest_digest FROM manifest_floors \
                 WHERE community_id = ?1",
                params![community_id.as_slice()],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?)),
            )
            .optional()?;
        match row {
            None => Ok(None),
            Some((generation, digest)) => Ok(Some((generation.max(0) as u64, blob32(digest)?))),
        }
    }

    /// Delete every staged row for an operation (its staged-operation row and all
    /// cascaded staged entries), releasing the reclaimed bytes to
    /// [`AccountingClass::Staging`]. Used by every terminal Commit disposition.
    pub fn delete_staging_for_operation(
        &mut self,
        operation_id: &[u8; 32],
    ) -> Result<(), AnchorRepositoryError> {
        let staged_bytes: Option<i64> = self
            .transaction
            .query_row(
                "SELECT staged_bytes FROM staged_operations WHERE operation_id = ?1",
                params![operation_id.as_slice()],
                |row| row.get(0),
            )
            .optional()?;
        // ON DELETE CASCADE removes the staged_entries rows.
        self.transaction.execute(
            "DELETE FROM staged_operations WHERE operation_id = ?1",
            params![operation_id.as_slice()],
        )?;
        if let Some(bytes) = staged_bytes {
            self.uncharge(AccountingClass::Staging, bytes.max(0) as u64);
        }
        Ok(())
    }

    /// Compare-and-swap a community's committed `site_generation`. If the community
    /// row is absent it is created at `committed` only when `base == 0` (a first
    /// host). Returns [`GenerationCas::Committed`] on a winning swap, or
    /// [`GenerationCas::Stale`] with the blocking current generation otherwise.
    pub fn commit_generation_cas(
        &mut self,
        community_id: &[u8; 32],
        created_at: u64,
        base_generation: u64,
        committed_generation: u64,
    ) -> Result<GenerationCas, AnchorRepositoryError> {
        let current: Option<i64> = self
            .transaction
            .query_row(
                "SELECT site_generation FROM communities WHERE community_id = ?1",
                params![community_id.as_slice()],
                |row| row.get(0),
            )
            .optional()?;
        match current {
            None => {
                if base_generation != 0 {
                    return Ok(GenerationCas::Stale {
                        current_generation: 0,
                    });
                }
                self.transaction.execute(
                    "INSERT INTO communities(community_id, created_at, logical_bytes, site_generation) \
                     VALUES (?1, ?2, 0, ?3)",
                    params![
                        community_id.as_slice(),
                        created_at as i64,
                        committed_generation as i64
                    ],
                )?;
                Ok(GenerationCas::Committed)
            }
            Some(current) => {
                let current = current.max(0) as u64;
                if current != base_generation {
                    return Ok(GenerationCas::Stale {
                        current_generation: current,
                    });
                }
                self.transaction.execute(
                    "UPDATE communities SET site_generation = ?1 WHERE community_id = ?2",
                    params![committed_generation as i64, community_id.as_slice()],
                )?;
                Ok(GenerationCas::Committed)
            }
        }
    }

    /// Promote one staged entry into committed state: ensure its namespace row,
    /// add the payload reference (full logical charge, deduplicated physical), and
    /// insert the committed entry, bumping the namespace live-entry count.
    pub fn insert_committed_entry(
        &mut self,
        community_id: &[u8; 32],
        namespace_kind: u8,
        entry: &StagedEntry,
    ) -> Result<(), AnchorRepositoryError> {
        self.transaction.execute(
            "INSERT OR IGNORE INTO namespaces(namespace_id, community_id, kind, live_entry_count) \
             VALUES (?1, ?2, ?3, 0)",
            params![
                entry.namespace_id.as_slice(),
                community_id.as_slice(),
                namespace_kind as i64
            ],
        )?;
        // Payload row must exist before the entry's payload_digest foreign key.
        self.add_payload(community_id, &entry.payload_digest, entry.payload_length)?;
        self.transaction.execute(
            "INSERT INTO entries(namespace_id, entry_id, subspace_id, path_bytes, timestamp_be, \
             payload_digest, payload_length, entry_bytes, item_bytes) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                entry.namespace_id.as_slice(),
                entry.entry_id.as_slice(),
                entry.subspace_id.as_slice(),
                entry.path_bytes.as_slice(),
                entry.timestamp_be.as_slice(),
                entry.payload_digest.as_slice(),
                entry.payload_length as i64,
                entry.entry_bytes.as_slice(),
                entry.item_bytes.as_slice()
            ],
        )?;
        self.transaction.execute(
            "UPDATE namespaces SET live_entry_count = live_entry_count + 1 WHERE namespace_id = ?1",
            params![entry.namespace_id.as_slice()],
        )?;
        Ok(())
    }

    /// Insert a signed hosting receipt bound to a community.
    pub fn insert_hosting_receipt(
        &mut self,
        receipt_id: &[u8; 32],
        community_id: &[u8; 32],
        created_at: u64,
        receipt_bytes: &[u8],
    ) -> Result<(), AnchorRepositoryError> {
        self.transaction.execute(
            "INSERT INTO hosting_receipts(receipt_id, community_id, created_at, receipt_bytes) \
             VALUES (?1, ?2, ?3, ?4)",
            params![
                receipt_id.as_slice(),
                community_id.as_slice(),
                created_at as i64,
                receipt_bytes
            ],
        )?;
        Ok(())
    }

    /// Set only an operation's terminal status and outcome bytes (the
    /// operation-lifecycle `GetOperation` result). Unlike
    /// [`Self::terminalize_operation`], this never touches the idempotency index —
    /// a `CommitHost` has its own idempotency row and the originating Prepare row
    /// must keep replaying its prepared response.
    pub fn set_operation_terminal(
        &mut self,
        operation_id: &[u8; 32],
        status: OperationStatus,
        terminal_result_bytes: &[u8],
    ) -> Result<(), AnchorRepositoryError> {
        self.transaction.execute(
            "UPDATE operations SET operation_status = ?2, terminal_result_bytes = ?3 \
             WHERE operation_id = ?1",
            params![
                operation_id.as_slice(),
                status.to_code(),
                terminal_result_bytes
            ],
        )?;
        Ok(())
    }

    /// Store a byte-identical ordinary terminal result keyed by its
    /// `control_request_digest`, for exact same-key replay of a single-call
    /// (e.g. `CommitHost`) request. The idempotency row must already be claimed.
    pub fn store_ordinary_result(
        &mut self,
        control_request_digest: &[u8; 32],
        result_bytes: &[u8],
    ) -> Result<(), AnchorRepositoryError> {
        self.transaction.execute(
            "INSERT INTO ordinary_results(control_request_digest, result_bytes) VALUES (?1, ?2)",
            params![control_request_digest.as_slice(), result_bytes],
        )?;
        Ok(())
    }

    /// The stored ordinary terminal result for a `control_request_digest`, read
    /// within this transaction (exact replay of a terminalized single-call key).
    pub fn ordinary_result(
        &self,
        control_request_digest: &[u8; 32],
    ) -> Result<Option<Vec<u8>>, AnchorRepositoryError> {
        self.transaction
            .query_row(
                "SELECT result_bytes FROM ordinary_results WHERE control_request_digest = ?1",
                params![control_request_digest.as_slice()],
                |row| row.get(0),
            )
            .optional()
            .map_err(AnchorRepositoryError::from)
    }

    // ---- Reserved owner-removal slot lifecycle (WU-016) --------------------

    /// Load one removal slot's durable state, read within this transaction.
    pub fn load_removal_slot(
        &self,
        slot_index: u32,
    ) -> Result<Option<RemovalSlot>, AnchorRepositoryError> {
        self.transaction
            .query_row(
                "SELECT slot_index, removal_state, claimed_by_community, claimed_root_key, \
                 request_digest, removal_idempotency_key, checkpoint_work_id, terminal_expires_at \
                 FROM removal_slots WHERE slot_index = ?1",
                params![slot_index],
                map_removal_slot,
            )
            .optional()
            .map_err(AnchorRepositoryError::from)
    }

    /// The removal slot a community currently owns in the `ReservedForListedRoot`
    /// state (the slot its live listing reserved), if any.
    pub fn reserved_slot_for_community(
        &self,
        community_id: &[u8; 32],
    ) -> Result<Option<RemovalSlot>, AnchorRepositoryError> {
        self.transaction
            .query_row(
                "SELECT slot_index, removal_state, claimed_by_community, claimed_root_key, \
                 request_digest, removal_idempotency_key, checkpoint_work_id, terminal_expires_at \
                 FROM removal_slots WHERE claimed_by_community = ?1 AND removal_state = 1 \
                 ORDER BY slot_index ASC LIMIT 1",
                params![community_id.as_slice()],
                map_removal_slot,
            )
            .optional()
            .map_err(AnchorRepositoryError::from)
    }

    /// Count the removal slots a root currently owns that count toward its
    /// per-root two-slot budget: its live reservation, any committed removal, and
    /// every *unexpired* retained Terminal result. Expired Terminal slots are not
    /// counted (they are reclaimable).
    pub fn count_owned_removal_slots(
        &self,
        root_key: &[u8; 32],
        now: u64,
    ) -> Result<u32, AnchorRepositoryError> {
        let count: i64 = self.transaction.query_row(
            "SELECT COUNT(*) FROM removal_slots WHERE claimed_root_key = ?1 \
             AND removal_state IN (1, 2, 3) \
             AND (terminal_expires_at IS NULL OR terminal_expires_at > ?2)",
            params![root_key.as_slice(), now as i64],
            |row| row.get(0),
        )?;
        Ok(count.max(0) as u32)
    }

    /// The earliest retained-Terminal expiry a root owns (the earliest time a
    /// two-slot-blocked relist could next reserve capacity), if any.
    pub fn earliest_owned_terminal_expiry(
        &self,
        root_key: &[u8; 32],
    ) -> Result<Option<u64>, AnchorRepositoryError> {
        let value: Option<i64> = self
            .transaction
            .query_row(
                "SELECT MIN(terminal_expires_at) FROM removal_slots \
                 WHERE claimed_root_key = ?1 AND removal_state = 3 \
                 AND terminal_expires_at IS NOT NULL",
                params![root_key.as_slice()],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        Ok(value.map(|v| v.max(0) as u64))
    }

    /// Atomically reserve one of the `2 * L` free slots for a root that is
    /// becoming visible, enforcing the exact per-root two-slot rule. Returns
    /// [`SlotReservation::Reserved`] with the claimed index, or
    /// [`SlotReservation::Blocked`] (listing stays hosted-but-invisible and the
    /// caller returns `removal_replay_window`) when the root already owns two
    /// slots or no global slot is free.
    pub fn reserve_visibility_slot(
        &mut self,
        community_id: &[u8; 32],
        root_key: &[u8; 32],
        request_digest: &[u8; 32],
        now: u64,
    ) -> Result<SlotReservation, AnchorRepositoryError> {
        if self.count_owned_removal_slots(root_key, now)? >= 2 {
            return Ok(SlotReservation::Blocked {
                earliest_retry_at: self.earliest_owned_terminal_expiry(root_key)?,
            });
        }
        let free: Option<u32> = self
            .transaction
            .query_row(
                "SELECT slot_index FROM removal_slots WHERE claimed_by_community IS NULL \
                 ORDER BY slot_index ASC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        let Some(slot) = free else {
            return Ok(SlotReservation::Blocked {
                earliest_retry_at: self.earliest_owned_terminal_expiry(root_key)?,
            });
        };
        self.transaction.execute(
            "UPDATE removal_slots SET claimed_by_community = ?1, claimed_root_key = ?2, \
             request_digest = ?3, removal_state = 1, removal_idempotency_key = NULL, \
             removal_operation_id = NULL, checkpoint_work_id = NULL, terminal_expires_at = NULL, \
             removal_result = NULL WHERE slot_index = ?4",
            params![
                community_id.as_slice(),
                root_key.as_slice(),
                request_digest.as_slice(),
                slot
            ],
        )?;
        Ok(SlotReservation::Reserved(slot))
    }

    /// Transition a reserved slot to `Committed`: a removal whose exact terminal
    /// result will be produced by a later covering checkpoint. Binds the removal's
    /// idempotency key, request digest, operation id, and checkpoint work id.
    pub fn commit_removal_slot(
        &mut self,
        slot_index: u32,
        removal_operation_id: &[u8; 32],
        idempotency_key: &[u8; 16],
        request_digest: &[u8; 32],
        checkpoint_work_id: &[u8; 32],
    ) -> Result<(), AnchorRepositoryError> {
        self.transaction.execute(
            "UPDATE removal_slots SET removal_state = 2, removal_operation_id = ?2, \
             removal_idempotency_key = ?3, request_digest = ?4, checkpoint_work_id = ?5 \
             WHERE slot_index = ?1 AND removal_state = 1",
            params![
                slot_index,
                removal_operation_id.as_slice(),
                idempotency_key.as_slice(),
                request_digest.as_slice(),
                checkpoint_work_id.as_slice()
            ],
        )?;
        Ok(())
    }

    /// Transition a reserved-or-committed slot directly to `Terminal`, recording
    /// its exact result bytes and 24-hour retention expiry. This is the
    /// ack-durable transition; physical compaction is not on this path.
    pub fn terminalize_removal_slot(
        &mut self,
        slot_index: u32,
        idempotency_key: &[u8; 16],
        request_digest: &[u8; 32],
        expires_at: u64,
        result_bytes: &[u8],
    ) -> Result<(), AnchorRepositoryError> {
        self.transaction.execute(
            "UPDATE removal_slots SET removal_state = 3, removal_idempotency_key = ?2, \
             request_digest = ?3, terminal_expires_at = ?4, removal_result = ?5, \
             checkpoint_work_id = NULL WHERE slot_index = ?1 AND removal_state IN (1, 2)",
            params![
                slot_index,
                idempotency_key.as_slice(),
                request_digest.as_slice(),
                expires_at as i64,
                result_bytes
            ],
        )?;
        Ok(())
    }

    /// Terminalize a checkpoint-covered `Committed` removal, preserving its
    /// original idempotency key, request digest, and exact frozen result, and only
    /// stamping the 24-hour Terminal retention expiry. Idempotent: a slot already
    /// `Terminal` is left unchanged, so a checkpoint advance can be re-run safely.
    pub fn terminalize_covered_removal(
        &mut self,
        slot_index: u32,
        expires_at: u64,
    ) -> Result<(), AnchorRepositoryError> {
        self.transaction.execute(
            "UPDATE removal_slots SET removal_state = 3, terminal_expires_at = ?2, \
             checkpoint_work_id = NULL WHERE slot_index = ?1 AND removal_state = 2",
            params![slot_index, expires_at as i64],
        )?;
        Ok(())
    }

    /// Release a slot back to `Free`, clearing every binding column. Used by
    /// expiry / terminal-suspension / operator-terminal-removal and idempotent
    /// startup cleanup of abandoned reservations.
    pub fn release_removal_slot(&mut self, slot_index: u32) -> Result<(), AnchorRepositoryError> {
        self.transaction.execute(
            "UPDATE removal_slots SET claimed_by_community = NULL, claimed_root_key = NULL, \
             request_digest = NULL, removal_state = 0, removal_idempotency_key = NULL, \
             removal_operation_id = NULL, checkpoint_work_id = NULL, terminal_expires_at = NULL, \
             removal_result = NULL WHERE slot_index = ?1",
            params![slot_index],
        )?;
        Ok(())
    }

    /// The `ReservedForListedRoot` slot indices whose owning community no longer
    /// has a live listing row — abandoned reservations startup cleanup releases.
    pub fn abandoned_reserved_slots(&self) -> Result<Vec<u32>, AnchorRepositoryError> {
        let mut statement = self.transaction.prepare(
            "SELECT slot_index FROM removal_slots WHERE removal_state = 1 \
             AND claimed_by_community IS NOT NULL \
             AND claimed_by_community NOT IN (SELECT community_id FROM listings) \
             ORDER BY slot_index ASC",
        )?;
        let rows = statement.query_map([], |row| row.get::<_, i64>(0))?;
        let mut slots = Vec::new();
        for row in rows {
            slots.push(row?.max(0) as u32);
        }
        Ok(slots)
    }

    /// Delete a community's current listing row (its visibility), retaining the
    /// removal slot and the signed feed history.
    pub fn delete_listing(&mut self, community_id: &[u8; 32]) -> Result<(), AnchorRepositoryError> {
        self.transaction.execute(
            "DELETE FROM listings WHERE community_id = ?1",
            params![community_id.as_slice()],
        )?;
        Ok(())
    }

    /// Store the byte-identical reserved-removal terminal result, keyed by its
    /// `control_request_digest` and bound to the removal slot. Never consumes
    /// ordinary idempotency capacity.
    pub fn store_reserved_result(
        &mut self,
        control_request_digest: &[u8; 32],
        removal_slot_index: u32,
        result_bytes: &[u8],
    ) -> Result<(), AnchorRepositoryError> {
        self.transaction.execute(
            "INSERT INTO reserved_results(control_request_digest, removal_slot_index, result_bytes) \
             VALUES (?1, ?2, ?3)",
            params![
                control_request_digest.as_slice(),
                removal_slot_index,
                result_bytes
            ],
        )?;
        Ok(())
    }

    /// The stored reserved-removal terminal result for a `control_request_digest`.
    pub fn reserved_result(
        &self,
        control_request_digest: &[u8; 32],
    ) -> Result<Option<Vec<u8>>, AnchorRepositoryError> {
        self.transaction
            .query_row(
                "SELECT result_bytes FROM reserved_results WHERE control_request_digest = ?1",
                params![control_request_digest.as_slice()],
                |row| row.get(0),
            )
            .optional()
            .map_err(AnchorRepositoryError::from)
    }

    // ---- Emergency-reserve permit / limit lookups (WU-016) -----------------

    /// The seeded fixed value of a named emergency reserve (design's
    /// `emergency_reserves` partitions: the reserved verifier / writer permits and
    /// the checkpoint worker).
    pub fn emergency_reserve_value(
        &self,
        reserve_name: &str,
    ) -> Result<Option<u64>, AnchorRepositoryError> {
        let value: Option<i64> = self
            .transaction
            .query_row(
                "SELECT default_value FROM emergency_reserves WHERE reserve_name = ?1",
                params![reserve_name],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value.map(|v| v.max(0) as u64))
    }

    // ---- Checkpoint work lifecycle (WU-016) --------------------------------

    /// Insert a frozen checkpoint-work plan (phase `Planned`) with its ordered,
    /// immutable snapshot members and covered committed-removal slots. Later
    /// site/listing changes cannot mutate this frozen work.
    pub fn insert_checkpoint_work(
        &mut self,
        plan: &CheckpointPlan,
    ) -> Result<(), AnchorRepositoryError> {
        self.transaction.execute(
            "INSERT INTO checkpoint_work(work_id, publication_phase, temp_filename, \
             published_filename, created_at, frozen_state_generation, covered_head_sequence, \
             covered_head_inclusion_digest, previous_checkpoint_digest, snapshot_generation_id, \
             canonical_checkpoint_body, checkpoint_envelope, checkpoint_digest, \
             published_content_hash) \
             VALUES (?1, 0, NULL, NULL, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, NULL, NULL)",
            params![
                plan.work_id.as_slice(),
                plan.created_at as i64,
                plan.frozen_state_generation as i64,
                plan.covered_head_sequence as i64,
                plan.covered_head_inclusion_digest.as_slice(),
                plan.previous_checkpoint_digest.map(|d| d.to_vec()),
                plan.snapshot_generation_id as i64,
                plan.canonical_checkpoint_body.as_slice()
            ],
        )?;
        for (position, member) in plan.ordered_members.iter().enumerate() {
            self.transaction.execute(
                "INSERT INTO checkpoint_work_members(work_id, member_position, community_id, \
                 frozen_head_digest, snapshot_record_bytes) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    plan.work_id.as_slice(),
                    position as i64,
                    member.community_id.as_slice(),
                    member.frozen_head_digest.as_slice(),
                    member.snapshot_record_bytes.as_slice()
                ],
            )?;
        }
        for slot in &plan.covered_removal_slots {
            self.transaction.execute(
                "INSERT INTO checkpoint_covered_removals(work_id, removal_slot_index) \
                 VALUES (?1, ?2)",
                params![plan.work_id.as_slice(), *slot],
            )?;
        }
        Ok(())
    }

    /// Load a checkpoint-work row (its persisted phase and frozen identities),
    /// read within this transaction.
    pub fn load_checkpoint_work(
        &self,
        work_id: &[u8; 32],
    ) -> Result<Option<CheckpointWorkRow>, AnchorRepositoryError> {
        self.transaction
            .query_row(
                "SELECT work_id, publication_phase, temp_filename, published_filename, created_at, \
                 frozen_state_generation, covered_head_sequence, covered_head_inclusion_digest, \
                 previous_checkpoint_digest, snapshot_generation_id, canonical_checkpoint_body, \
                 checkpoint_envelope, checkpoint_digest, published_content_hash \
                 FROM checkpoint_work WHERE work_id = ?1",
                params![work_id.as_slice()],
                map_checkpoint_work,
            )
            .optional()
            .map_err(AnchorRepositoryError::from)
    }

    /// The ordered frozen snapshot members of a checkpoint work.
    pub fn checkpoint_work_members(
        &self,
        work_id: &[u8; 32],
    ) -> Result<Vec<CheckpointMember>, AnchorRepositoryError> {
        let mut statement = self.transaction.prepare(
            "SELECT community_id, frozen_head_digest, snapshot_record_bytes \
             FROM checkpoint_work_members WHERE work_id = ?1 ORDER BY member_position ASC",
        )?;
        let rows = statement.query_map(params![work_id.as_slice()], |row| {
            Ok(CheckpointMember {
                community_id: blob32(row.get::<_, Vec<u8>>(0)?)?,
                frozen_head_digest: blob32(row.get::<_, Vec<u8>>(1)?)?,
                snapshot_record_bytes: row.get::<_, Vec<u8>>(2)?,
            })
        })?;
        let mut members = Vec::new();
        for row in rows {
            members.push(row?);
        }
        Ok(members)
    }

    /// The covered committed-removal slot indices of a checkpoint work, ascending.
    pub fn checkpoint_covered_removals(
        &self,
        work_id: &[u8; 32],
    ) -> Result<Vec<u32>, AnchorRepositoryError> {
        let mut statement = self.transaction.prepare(
            "SELECT removal_slot_index FROM checkpoint_covered_removals \
             WHERE work_id = ?1 ORDER BY removal_slot_index ASC",
        )?;
        let rows = statement.query_map(params![work_id.as_slice()], |row| row.get::<_, i64>(0))?;
        let mut slots = Vec::new();
        for row in rows {
            slots.push(row?.max(0) as u32);
        }
        Ok(slots)
    }

    /// Record the operator-signed checkpoint envelope + digest (phase `Signed`).
    pub fn set_checkpoint_signed(
        &mut self,
        work_id: &[u8; 32],
        checkpoint_envelope: &[u8],
        checkpoint_digest: &[u8; 32],
    ) -> Result<(), AnchorRepositoryError> {
        self.transaction.execute(
            "UPDATE checkpoint_work SET publication_phase = 1, checkpoint_envelope = ?2, \
             checkpoint_digest = ?3 WHERE work_id = ?1 AND publication_phase = 0",
            params![
                work_id.as_slice(),
                checkpoint_envelope,
                checkpoint_digest.as_slice()
            ],
        )?;
        Ok(())
    }

    /// Record the reserved temp/final publication names before any filesystem
    /// write, so recovery can find and reclaim an unpublished temp tree.
    pub fn set_checkpoint_names(
        &mut self,
        work_id: &[u8; 32],
        temp_filename: &str,
        published_filename: &str,
    ) -> Result<(), AnchorRepositoryError> {
        self.transaction.execute(
            "UPDATE checkpoint_work SET temp_filename = ?2, published_filename = ?3 \
             WHERE work_id = ?1",
            params![work_id.as_slice(), temp_filename, published_filename],
        )?;
        Ok(())
    }

    /// Persist `FilesPublished` with the validated final content hash (only after
    /// the atomic filesystem rename + parent fsync completed).
    pub fn set_checkpoint_files_published(
        &mut self,
        work_id: &[u8; 32],
        published_content_hash: &[u8; 32],
    ) -> Result<(), AnchorRepositoryError> {
        self.transaction.execute(
            "UPDATE checkpoint_work SET publication_phase = 2, published_content_hash = ?2 \
             WHERE work_id = ?1 AND publication_phase = 1",
            params![work_id.as_slice(), published_content_hash.as_slice()],
        )?;
        Ok(())
    }

    /// Advance a checkpoint work to `FloorAdvanced` (phase 2 → 3). Called within
    /// the same transaction that switches the checkpoint pointer, advances the
    /// floor, and terminalizes the covered removals.
    pub fn set_checkpoint_phase_floor_advanced(
        &mut self,
        work_id: &[u8; 32],
    ) -> Result<(), AnchorRepositoryError> {
        self.transaction.execute(
            "UPDATE checkpoint_work SET publication_phase = 3 WHERE work_id = ?1 \
             AND publication_phase = 2",
            params![work_id.as_slice()],
        )?;
        Ok(())
    }

    /// Persist the terminal `Reclaimed` phase after all bounded physical
    /// maintenance completes.
    pub fn set_checkpoint_reclaimed(
        &mut self,
        work_id: &[u8; 32],
    ) -> Result<(), AnchorRepositoryError> {
        self.transaction.execute(
            "UPDATE checkpoint_work SET publication_phase = 4 WHERE work_id = ?1 \
             AND publication_phase = 3",
            params![work_id.as_slice()],
        )?;
        Ok(())
    }

    /// Insert a published directory checkpoint (its signed bytes at a generation).
    pub fn insert_directory_checkpoint(
        &mut self,
        checkpoint_generation: u64,
        signed_bytes: &[u8],
        created_at: u64,
    ) -> Result<(), AnchorRepositoryError> {
        self.transaction.execute(
            "INSERT OR IGNORE INTO directory_checkpoints(checkpoint_generation, signed_bytes, created_at) \
             VALUES (?1, ?2, ?3)",
            params![checkpoint_generation as i64, signed_bytes, created_at as i64],
        )?;
        Ok(())
    }

    /// The highest published directory-checkpoint generation (0 if none).
    pub fn latest_checkpoint_generation(&self) -> Result<u64, AnchorRepositoryError> {
        let value: i64 = self.transaction.query_row(
            "SELECT COALESCE(MAX(checkpoint_generation), 0) FROM directory_checkpoints",
            [],
            |row| row.get(0),
        )?;
        Ok(value.max(0) as u64)
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

/// A community's current directory listing row (its bounded visibility state).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentListing {
    /// The listing root key.
    pub root_key: [u8; 32],
    /// When the current listing state was last set.
    pub listed_at: u64,
    /// Listing expiry (Unix seconds).
    pub expires_at: u64,
    /// The most recent host-refresh time.
    pub last_host_refresh_at: u64,
    /// The removal slot this listed root owns.
    pub removal_slot_index: u32,
}

/// The lifecycle state of one of the `2 * L` preprovisioned removal slots
/// (design "RemovalSlot = Free | ReservedForListedRoot | Committed | Terminal").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemovalSlotState {
    /// Unclaimed and available.
    Free,
    /// Reserved for a currently listed root; only that root can consume it.
    ReservedForListedRoot,
    /// A removal is committed and awaiting a checkpoint to terminalize it.
    Committed,
    /// The removal reached its exact terminal result, retained until expiry.
    Terminal,
}

impl RemovalSlotState {
    /// The stable integer code stored in `removal_slots.removal_state`.
    #[must_use]
    pub fn to_code(self) -> i64 {
        match self {
            RemovalSlotState::Free => 0,
            RemovalSlotState::ReservedForListedRoot => 1,
            RemovalSlotState::Committed => 2,
            RemovalSlotState::Terminal => 3,
        }
    }

    fn from_code(code: i64) -> Option<Self> {
        match code {
            0 => Some(RemovalSlotState::Free),
            1 => Some(RemovalSlotState::ReservedForListedRoot),
            2 => Some(RemovalSlotState::Committed),
            3 => Some(RemovalSlotState::Terminal),
            _ => None,
        }
    }
}

/// A read of one removal slot's durable state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovalSlot {
    /// The physical slot index.
    pub slot_index: u32,
    /// The current lifecycle state.
    pub state: RemovalSlotState,
    /// The community that owns the slot, if claimed.
    pub claimed_by_community: Option<[u8; 32]>,
    /// The root key that owns the slot, if claimed.
    pub claimed_root_key: Option<[u8; 32]>,
    /// The bound `control_request_digest`, if claimed.
    pub request_digest: Option<[u8; 32]>,
    /// The bound owner-removal idempotency key, once Committed/Terminal.
    pub removal_idempotency_key: Option<[u8; 16]>,
    /// The linked checkpoint work id, once Committed.
    pub checkpoint_work_id: Option<[u8; 32]>,
    /// The Terminal-state retention expiry (Unix seconds).
    pub terminal_expires_at: Option<u64>,
}

/// The outcome of a per-root visibility slot reservation
/// ([`RepoTransaction::reserve_visibility_slot`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotReservation {
    /// One free slot was atomically reserved for the root; its index.
    Reserved(u32),
    /// The root already owns two slots, or no global slot is free. The listing
    /// stays hosted-but-invisible; the caller returns `removal_replay_window`.
    Blocked {
        /// The earliest retained-Terminal expiry that could free capacity, if any.
        earliest_retry_at: Option<u64>,
    },
}

/// The durable publication phase of a checkpoint work record (design
/// "Planned | Signed | FilesPublished | FloorAdvanced | Reclaimed").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckpointPhase {
    /// The immutable plan is frozen; nothing is signed or published.
    Planned,
    /// The operator signed the frozen body; envelope + digest are stored.
    Signed,
    /// The signed files are validated, fsynced, and atomically renamed into place.
    FilesPublished,
    /// The database pointer/floor advanced and covered removals are terminalized.
    FloorAdvanced,
    /// All bounded physical maintenance completed.
    Reclaimed,
}

impl CheckpointPhase {
    fn from_code(code: i64) -> Option<Self> {
        match code {
            0 => Some(CheckpointPhase::Planned),
            1 => Some(CheckpointPhase::Signed),
            2 => Some(CheckpointPhase::FilesPublished),
            3 => Some(CheckpointPhase::FloorAdvanced),
            4 => Some(CheckpointPhase::Reclaimed),
            _ => None,
        }
    }
}

/// One frozen ordered snapshot member of a checkpoint plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointMember {
    /// The member community.
    pub community_id: [u8; 32],
    /// The community's frozen directory head digest at plan time.
    pub frozen_head_digest: [u8; 32],
    /// The immutable snapshot-member record bytes frozen for publication.
    pub snapshot_record_bytes: Vec<u8>,
}

/// The immutable inputs frozen by one checkpoint planning transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointPlan {
    /// The stable checkpoint-work id.
    pub work_id: [u8; 32],
    /// The wall-clock creation time frozen into the plan (never re-invented).
    pub created_at: u64,
    /// The frozen state generation the checkpoint covers.
    pub frozen_state_generation: u64,
    /// The covered directory-feed head sequence.
    pub covered_head_sequence: u64,
    /// The covered directory-feed head inclusion digest.
    pub covered_head_inclusion_digest: [u8; 32],
    /// The previous published checkpoint digest, if any.
    pub previous_checkpoint_digest: Option<[u8; 32]>,
    /// The snapshot generation id this checkpoint publishes.
    pub snapshot_generation_id: u64,
    /// The canonical checkpoint body bytes (signed later).
    pub canonical_checkpoint_body: Vec<u8>,
    /// The frozen ordered snapshot members.
    pub ordered_members: Vec<CheckpointMember>,
    /// The covered committed-removal slot indices terminalized at advance.
    pub covered_removal_slots: Vec<u32>,
}

/// A read of a checkpoint-work row's persisted phase and frozen identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointWorkRow {
    /// The stable work id.
    pub work_id: [u8; 32],
    /// The current durable phase.
    pub phase: CheckpointPhase,
    /// The reserved temp publication name, once set.
    pub temp_filename: Option<String>,
    /// The reserved final publication name, once set.
    pub published_filename: Option<String>,
    /// The frozen creation time.
    pub created_at: u64,
    /// The frozen state generation.
    pub frozen_state_generation: u64,
    /// The covered feed head sequence.
    pub covered_head_sequence: u64,
    /// The covered feed head inclusion digest.
    pub covered_head_inclusion_digest: Option<[u8; 32]>,
    /// The previous checkpoint digest.
    pub previous_checkpoint_digest: Option<[u8; 32]>,
    /// The snapshot generation id.
    pub snapshot_generation_id: u64,
    /// The canonical checkpoint body bytes.
    pub canonical_checkpoint_body: Option<Vec<u8>>,
    /// The stored operator-signed checkpoint envelope bytes.
    pub checkpoint_envelope: Option<Vec<u8>>,
    /// The stored checkpoint digest.
    pub checkpoint_digest: Option<[u8; 32]>,
    /// The validated published content hash.
    pub published_content_hash: Option<[u8; 32]>,
}

/// The stable integer code for a listing authority class (`0` root-owned, `1`
/// delegated), matching the `listing_floors.shown_class` CHECK.
fn authority_class_code(class: AuthorityClass) -> i64 {
    match class {
        AuthorityClass::RootOwned => 0,
        AuthorityClass::Delegated => 1,
    }
}

fn map_listing_floor(
    root_id: [u8; 32],
    row: &rusqlite::Row<'_>,
) -> Result<ListingFloor, rusqlite::Error> {
    let epoch: i64 = row.get(0)?;
    let sealed: i64 = row.get(1)?;
    let highest_revision: i64 = row.get(2)?;
    let shown_digest = row.get::<_, Option<Vec<u8>>>(3)?.map(blob32).transpose()?;
    let shown_class = match row.get::<_, Option<i64>>(4)? {
        None => None,
        Some(0) => Some(AuthorityClass::RootOwned),
        Some(1) => Some(AuthorityClass::Delegated),
        Some(_) => {
            return Err(rusqlite::Error::FromSqlConversionFailure(
                4,
                rusqlite::types::Type::Integer,
                "invalid shown_class".into(),
            ))
        }
    };
    let equivocated: i64 = row.get(5)?;
    Ok(ListingFloor {
        root_id,
        epoch: epoch.max(0) as u32,
        sealed: sealed != 0,
        highest_revision: highest_revision.max(0) as u32,
        shown_digest,
        shown_class,
        equivocated: equivocated != 0,
    })
}

fn map_current_listing(row: &rusqlite::Row<'_>) -> Result<CurrentListing, rusqlite::Error> {
    let root_key = blob32(row.get::<_, Vec<u8>>(0)?)?;
    let listed_at: i64 = row.get(1)?;
    let expires_at: i64 = row.get(2)?;
    let last_host_refresh_at: i64 = row.get(3)?;
    let removal_slot_index: i64 = row.get(4)?;
    Ok(CurrentListing {
        root_key,
        listed_at: listed_at.max(0) as u64,
        expires_at: expires_at.max(0) as u64,
        last_host_refresh_at: last_host_refresh_at.max(0) as u64,
        removal_slot_index: removal_slot_index.max(0) as u32,
    })
}

fn blob16(value: Vec<u8>) -> Result<[u8; 16], rusqlite::Error> {
    <[u8; 16]>::try_from(value.as_slice()).map_err(|_| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Blob,
            "expected 16-byte blob".into(),
        )
    })
}

fn map_removal_slot(row: &rusqlite::Row<'_>) -> Result<RemovalSlot, rusqlite::Error> {
    let slot_index: i64 = row.get(0)?;
    let state = RemovalSlotState::from_code(row.get(1)?).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            1,
            rusqlite::types::Type::Integer,
            "invalid removal_state".into(),
        )
    })?;
    let claimed_by_community = row.get::<_, Option<Vec<u8>>>(2)?.map(blob32).transpose()?;
    let claimed_root_key = row.get::<_, Option<Vec<u8>>>(3)?.map(blob32).transpose()?;
    let request_digest = row.get::<_, Option<Vec<u8>>>(4)?.map(blob32).transpose()?;
    let removal_idempotency_key = row.get::<_, Option<Vec<u8>>>(5)?.map(blob16).transpose()?;
    let checkpoint_work_id = row.get::<_, Option<Vec<u8>>>(6)?.map(blob32).transpose()?;
    let terminal_expires_at = row.get::<_, Option<i64>>(7)?.map(|v| v.max(0) as u64);
    Ok(RemovalSlot {
        slot_index: slot_index.max(0) as u32,
        state,
        claimed_by_community,
        claimed_root_key,
        request_digest,
        removal_idempotency_key,
        checkpoint_work_id,
        terminal_expires_at,
    })
}

fn map_checkpoint_work(row: &rusqlite::Row<'_>) -> Result<CheckpointWorkRow, rusqlite::Error> {
    let work_id = blob32(row.get::<_, Vec<u8>>(0)?)?;
    let phase = CheckpointPhase::from_code(row.get(1)?).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            1,
            rusqlite::types::Type::Integer,
            "invalid publication_phase".into(),
        )
    })?;
    let temp_filename: Option<String> = row.get(2)?;
    let published_filename: Option<String> = row.get(3)?;
    let created_at: i64 = row.get(4)?;
    let frozen_state_generation: i64 = row.get(5)?;
    let covered_head_sequence: i64 = row.get(6)?;
    let covered_head_inclusion_digest =
        row.get::<_, Option<Vec<u8>>>(7)?.map(blob32).transpose()?;
    let previous_checkpoint_digest = row.get::<_, Option<Vec<u8>>>(8)?.map(blob32).transpose()?;
    let snapshot_generation_id: i64 = row.get(9)?;
    let canonical_checkpoint_body: Option<Vec<u8>> = row.get(10)?;
    let checkpoint_envelope: Option<Vec<u8>> = row.get(11)?;
    let checkpoint_digest = row.get::<_, Option<Vec<u8>>>(12)?.map(blob32).transpose()?;
    let published_content_hash = row.get::<_, Option<Vec<u8>>>(13)?.map(blob32).transpose()?;
    Ok(CheckpointWorkRow {
        work_id,
        phase,
        temp_filename,
        published_filename,
        created_at: created_at.max(0) as u64,
        frozen_state_generation: frozen_state_generation.max(0) as u64,
        covered_head_sequence: covered_head_sequence.max(0) as u64,
        covered_head_inclusion_digest,
        previous_checkpoint_digest,
        snapshot_generation_id: snapshot_generation_id.max(0) as u64,
        canonical_checkpoint_body,
        checkpoint_envelope,
        checkpoint_digest,
        published_content_hash,
    })
}

fn blob32(value: Vec<u8>) -> Result<[u8; 32], rusqlite::Error> {
    <[u8; 32]>::try_from(value.as_slice()).map_err(|_| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Blob,
            "expected 32-byte blob".into(),
        )
    })
}

fn map_idempotency_row(row: &rusqlite::Row<'_>) -> Result<IdempotencyRow, rusqlite::Error> {
    let control_request_digest = blob32(row.get::<_, Vec<u8>>(0)?)?;
    let result_class: i64 = row.get(1)?;
    let claim_state = IdempotencyClaimState::from_code(row.get(2)?).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            2,
            rusqlite::types::Type::Integer,
            "invalid claim_state".into(),
        )
    })?;
    let operation_id = row.get::<_, Option<Vec<u8>>>(3)?.map(blob32).transpose()?;
    let lease_expires_at = row
        .get::<_, Option<i64>>(4)?
        .map(|value| value.max(0) as u64);
    Ok(IdempotencyRow {
        control_request_digest,
        result_class: result_class.max(0) as u8,
        claim_state,
        operation_id,
        lease_expires_at,
    })
}

fn map_staged_entry(row: &rusqlite::Row<'_>) -> Result<StagedEntry, rusqlite::Error> {
    let namespace_id = blob32(row.get::<_, Vec<u8>>(0)?)?;
    let entry_id = blob32(row.get::<_, Vec<u8>>(1)?)?;
    let subspace_id = blob32(row.get::<_, Vec<u8>>(2)?)?;
    let path_bytes: Vec<u8> = row.get(3)?;
    let timestamp_bytes: Vec<u8> = row.get(4)?;
    let timestamp_be = <[u8; 8]>::try_from(timestamp_bytes.as_slice()).map_err(|_| {
        rusqlite::Error::FromSqlConversionFailure(
            4,
            rusqlite::types::Type::Blob,
            "expected 8-byte timestamp".into(),
        )
    })?;
    let payload_digest = blob32(row.get::<_, Vec<u8>>(5)?)?;
    let payload_length: i64 = row.get(6)?;
    let entry_bytes: Vec<u8> = row.get(7)?;
    let item_bytes: Vec<u8> = row.get(8)?;
    Ok(StagedEntry {
        namespace_id,
        entry_id,
        subspace_id,
        path_bytes,
        timestamp_be,
        payload_digest,
        payload_length: payload_length.max(0) as u64,
        entry_bytes,
        item_bytes,
    })
}

fn map_stored_operation(row: &rusqlite::Row<'_>) -> Result<StoredOperation, rusqlite::Error> {
    let operation_id = blob32(row.get::<_, Vec<u8>>(0)?)?;
    let originating_kind = OperationKind::from_code(row.get(1)?).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            1,
            rusqlite::types::Type::Integer,
            "invalid originating_kind".into(),
        )
    })?;
    let token_secret_epoch: i64 = row.get(2)?;
    let base_generation: i64 = row.get(3)?;
    let status = OperationStatus::from_code(row.get(4)?).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            4,
            rusqlite::types::Type::Integer,
            "invalid operation_status".into(),
        )
    })?;
    let operation_expiry: i64 = row.get(5)?;
    let retention_deadline: i64 = row.get(6)?;
    let prepare_response_bytes: Vec<u8> = row.get(7)?;
    let terminal_result_bytes: Option<Vec<u8>> = row.get(8)?;
    let ticket_envelope_bytes: Option<Vec<u8>> = row.get(9)?;
    Ok(StoredOperation {
        operation_id,
        originating_kind,
        token_secret_epoch: token_secret_epoch.max(0) as u32,
        base_generation: base_generation.max(0) as u64,
        status,
        operation_expiry: operation_expiry.max(0) as u64,
        retention_deadline: retention_deadline.max(0) as u64,
        prepare_response_bytes,
        terminal_result_bytes,
        ticket_envelope_bytes,
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_write_wins_and_returns_the_proposal() {
        let mut repo = AnchorRepository::open_in_memory().expect("open");
        let first = repo
            .load_or_initialize_secret("genesis_random", &[7u8; 32])
            .expect("init");
        assert_eq!(first, [7u8; 32]);
    }

    #[test]
    fn persisted_secret_beats_a_different_later_proposal() {
        let mut repo = AnchorRepository::open_in_memory().expect("open");
        repo.load_or_initialize_secret("genesis_random", &[7u8; 32])
            .expect("init");
        // A different proposal (what an operator-key rotation would derive) must
        // NOT displace the persisted value — anchor identity is bound to the DB.
        let second = repo
            .load_or_initialize_secret("genesis_random", &[9u8; 32])
            .expect("load");
        assert_eq!(second, [7u8; 32]);
    }

    #[test]
    fn secrets_are_partitioned_by_name() {
        let mut repo = AnchorRepository::open_in_memory().expect("open");
        repo.load_or_initialize_secret("genesis_random", &[7u8; 32])
            .expect("init genesis");
        let token = repo
            .load_or_initialize_secret("token_secret_v1", &[8u8; 32])
            .expect("init token");
        assert_eq!(token, [8u8; 32], "each name initializes independently");
    }
}
