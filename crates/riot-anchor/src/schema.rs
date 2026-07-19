//! Forward-only SQLite schema for the anchor `AnchorRepository`.
//!
//! [`migrate`] applies the versioned, forward-only schema: every logical table,
//! index, and constraint the design's Anchor Repository defines, plus the
//! preprovisioned `2 * L` removal slots and the seeded emergency-reserve rows.
//! Schema versioning is explicit and fails closed — a database stamped with a
//! version newer than [`CURRENT_SCHEMA_VERSION`] is refused
//! ([`SchemaError::VersionTooNew`]) rather than downgraded or migrated backward.

use rusqlite::{Connection, TransactionBehavior};

/// Current schema version this binary declares.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Default configured listing ceiling `L`. The preprovisioned removal table
/// holds exactly `2 * L` slots.
pub const DEFAULT_LISTING_CEILING: u32 = 10_000;

/// Errors from opening/migrating an anchor database.
#[derive(Debug)]
#[non_exhaustive]
pub enum SchemaError {
    /// A SQLite error occurred while migrating.
    Sqlite(rusqlite::Error),
    /// The database was created by a NEWER binary. Fail closed.
    VersionTooNew {
        /// Version stamped in the database.
        found: u32,
        /// Highest version this binary declares.
        supported: u32,
    },
    /// The database structure is inconsistent with its version marker.
    Corrupt,
}

impl core::fmt::Display for SchemaError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Sqlite(error) => write!(formatter, "anchor schema sqlite error: {error}"),
            Self::VersionTooNew { found, supported } => write!(
                formatter,
                "anchor database schema version {found} is newer than supported {supported}"
            ),
            Self::Corrupt => write!(formatter, "anchor database schema is corrupt"),
        }
    }
}

impl std::error::Error for SchemaError {}

impl From<rusqlite::Error> for SchemaError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

/// Forward-only migration 1: the full `AnchorRepository` schema. Tables are
/// created parents-before-children so foreign keys resolve at insert time.
/// Singleton and reserve rows are seeded here; the preprovisioned removal slots
/// are inserted separately (their count derives from [`DEFAULT_LISTING_CEILING`]).
const MIGRATION_ONE: &str = r#"
    CREATE TABLE operator_state (
        singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
        operator_key_id BLOB CHECK (operator_key_id IS NULL OR length(operator_key_id) = 32),
        schema_version INTEGER NOT NULL CHECK (schema_version > 0),
        record_version INTEGER NOT NULL CHECK (record_version > 0),
        descriptor_bytes BLOB,
        descriptor_floor_generation INTEGER NOT NULL CHECK (descriptor_floor_generation >= 0)
    ) STRICT;

    CREATE TABLE deployment_lease (
        singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
        deployment_instance_token BLOB CHECK (deployment_instance_token IS NULL OR length(deployment_instance_token) = 32),
        lease_holder_id BLOB CHECK (lease_holder_id IS NULL OR length(lease_holder_id) = 32),
        lease_epoch INTEGER NOT NULL DEFAULT 0 CHECK (lease_epoch >= 0),
        lease_expires_at INTEGER NOT NULL DEFAULT 0 CHECK (lease_expires_at >= 0)
    ) STRICT;

    CREATE TABLE cursor_secret_epochs (
        epoch INTEGER PRIMARY KEY CHECK (epoch >= 0),
        secret_bytes BLOB NOT NULL CHECK (length(secret_bytes) = 32),
        created_at INTEGER NOT NULL CHECK (created_at >= 0),
        retired_at INTEGER CHECK (retired_at IS NULL OR retired_at >= created_at)
    ) STRICT;

    CREATE TABLE communities (
        community_id BLOB PRIMARY KEY NOT NULL CHECK (length(community_id) = 32),
        created_at INTEGER NOT NULL CHECK (created_at >= 0),
        logical_bytes INTEGER NOT NULL CHECK (logical_bytes >= 0)
    ) STRICT;

    CREATE TABLE payloads (
        payload_digest BLOB PRIMARY KEY NOT NULL CHECK (length(payload_digest) = 32),
        payload_length INTEGER NOT NULL CHECK (payload_length >= 0),
        payload_bytes BLOB,
        reference_count INTEGER NOT NULL CHECK (reference_count >= 0)
    ) STRICT;

    CREATE TABLE manifests (
        community_id BLOB NOT NULL CHECK (length(community_id) = 32),
        manifest_generation INTEGER NOT NULL CHECK (manifest_generation >= 0),
        manifest_digest BLOB NOT NULL CHECK (length(manifest_digest) = 32),
        manifest_bytes BLOB NOT NULL CHECK (length(manifest_bytes) > 0 AND length(manifest_bytes) <= 16384),
        PRIMARY KEY (community_id, manifest_generation),
        FOREIGN KEY (community_id) REFERENCES communities(community_id) ON DELETE CASCADE
    ) STRICT;

    CREATE TABLE manifest_floors (
        community_id BLOB PRIMARY KEY NOT NULL CHECK (length(community_id) = 32),
        min_manifest_generation INTEGER NOT NULL CHECK (min_manifest_generation >= 0),
        min_manifest_digest BLOB NOT NULL CHECK (length(min_manifest_digest) = 32),
        FOREIGN KEY (community_id) REFERENCES communities(community_id) ON DELETE CASCADE
    ) STRICT;

    CREATE TABLE public_site_tickets (
        ticket_digest BLOB PRIMARY KEY NOT NULL CHECK (length(ticket_digest) = 32),
        community_id BLOB NOT NULL CHECK (length(community_id) = 32),
        root_key BLOB NOT NULL CHECK (length(root_key) = 32),
        admitted_at INTEGER NOT NULL CHECK (admitted_at >= 0),
        expires_at INTEGER NOT NULL CHECK (expires_at > admitted_at),
        ticket_bytes BLOB NOT NULL CHECK (length(ticket_bytes) > 0 AND length(ticket_bytes) <= 768),
        FOREIGN KEY (community_id) REFERENCES communities(community_id) ON DELETE CASCADE
    ) STRICT;

    CREATE TABLE namespaces (
        namespace_id BLOB PRIMARY KEY NOT NULL CHECK (length(namespace_id) = 32),
        community_id BLOB NOT NULL CHECK (length(community_id) = 32),
        kind INTEGER NOT NULL CHECK (kind BETWEEN 0 AND 3),
        live_entry_count INTEGER NOT NULL CHECK (live_entry_count >= 0),
        FOREIGN KEY (community_id) REFERENCES communities(community_id) ON DELETE CASCADE
    ) STRICT;

    CREATE TABLE entries (
        namespace_id BLOB NOT NULL CHECK (length(namespace_id) = 32),
        entry_id BLOB NOT NULL CHECK (length(entry_id) = 32),
        subspace_id BLOB NOT NULL CHECK (length(subspace_id) = 32),
        path_bytes BLOB NOT NULL,
        timestamp_be BLOB NOT NULL CHECK (length(timestamp_be) = 8),
        payload_digest BLOB NOT NULL CHECK (length(payload_digest) = 32),
        payload_length INTEGER NOT NULL CHECK (payload_length >= 0),
        entry_bytes BLOB NOT NULL CHECK (length(entry_bytes) > 0),
        PRIMARY KEY (namespace_id, entry_id),
        FOREIGN KEY (namespace_id) REFERENCES namespaces(namespace_id) ON DELETE CASCADE,
        FOREIGN KEY (payload_digest) REFERENCES payloads(payload_digest)
    ) STRICT;

    CREATE TABLE community_payload_refs (
        community_id BLOB NOT NULL CHECK (length(community_id) = 32),
        payload_digest BLOB NOT NULL CHECK (length(payload_digest) = 32),
        logical_bytes INTEGER NOT NULL CHECK (logical_bytes >= 0),
        PRIMARY KEY (community_id, payload_digest),
        FOREIGN KEY (community_id) REFERENCES communities(community_id) ON DELETE CASCADE,
        FOREIGN KEY (payload_digest) REFERENCES payloads(payload_digest)
    ) STRICT;

    CREATE TABLE removal_slots (
        slot_index INTEGER PRIMARY KEY NOT NULL CHECK (slot_index >= 0),
        claimed_by_community BLOB CHECK (claimed_by_community IS NULL OR length(claimed_by_community) = 32),
        claimed_root_key BLOB CHECK (claimed_root_key IS NULL OR length(claimed_root_key) = 32),
        request_digest BLOB CHECK (request_digest IS NULL OR length(request_digest) = 32),
        removal_state INTEGER NOT NULL DEFAULT 0 CHECK (removal_state BETWEEN 0 AND 3),
        removal_result BLOB CHECK (removal_result IS NULL OR length(removal_result) <= 4096),
        FOREIGN KEY (claimed_by_community) REFERENCES communities(community_id)
    ) STRICT;

    CREATE TABLE listings (
        community_id BLOB PRIMARY KEY NOT NULL CHECK (length(community_id) = 32),
        root_key BLOB NOT NULL CHECK (length(root_key) = 32),
        listed_at INTEGER NOT NULL CHECK (listed_at >= 0),
        expires_at INTEGER NOT NULL CHECK (expires_at > listed_at),
        last_host_refresh_at INTEGER NOT NULL CHECK (last_host_refresh_at >= 0),
        removal_slot_index INTEGER NOT NULL UNIQUE,
        FOREIGN KEY (community_id) REFERENCES communities(community_id) ON DELETE CASCADE,
        FOREIGN KEY (removal_slot_index) REFERENCES removal_slots(slot_index)
    ) STRICT;

    CREATE TABLE listing_conflict_floors (
        community_id BLOB NOT NULL CHECK (length(community_id) = 32),
        subject_key BLOB NOT NULL CHECK (length(subject_key) = 32),
        floor_generation INTEGER NOT NULL CHECK (floor_generation >= 0),
        proof_count INTEGER NOT NULL CHECK (proof_count BETWEEN 0 AND 4),
        PRIMARY KEY (community_id, subject_key),
        FOREIGN KEY (community_id) REFERENCES communities(community_id) ON DELETE CASCADE
    ) STRICT;

    CREATE TABLE directory_inclusions (
        inclusion_id BLOB PRIMARY KEY NOT NULL CHECK (length(inclusion_id) = 32),
        community_id BLOB NOT NULL CHECK (length(community_id) = 32),
        included_at INTEGER NOT NULL CHECK (included_at >= 0),
        record_bytes BLOB NOT NULL CHECK (length(record_bytes) > 0 AND length(record_bytes) <= 49152),
        FOREIGN KEY (community_id) REFERENCES communities(community_id) ON DELETE CASCADE
    ) STRICT;

    CREATE TABLE directory_feed_heads (
        feed_id INTEGER PRIMARY KEY CHECK (feed_id = 1),
        head_digest BLOB NOT NULL CHECK (length(head_digest) = 32),
        feed_length INTEGER NOT NULL CHECK (feed_length >= 0),
        updated_at INTEGER NOT NULL CHECK (updated_at >= 0)
    ) STRICT;

    CREATE TABLE directory_checkpoints (
        checkpoint_generation INTEGER PRIMARY KEY NOT NULL CHECK (checkpoint_generation >= 0),
        signed_bytes BLOB NOT NULL CHECK (length(signed_bytes) > 0),
        created_at INTEGER NOT NULL CHECK (created_at >= 0)
    ) STRICT;

    CREATE TABLE snapshot_generations (
        generation INTEGER PRIMARY KEY NOT NULL CHECK (generation >= 0),
        frozen_at INTEGER NOT NULL CHECK (frozen_at >= 0),
        member_count INTEGER NOT NULL CHECK (member_count >= 0),
        snapshot_digest BLOB NOT NULL CHECK (length(snapshot_digest) = 32)
    ) STRICT;

    CREATE TABLE snapshot_members (
        generation INTEGER NOT NULL,
        member_position INTEGER NOT NULL CHECK (member_position >= 0),
        community_id BLOB NOT NULL CHECK (length(community_id) = 32),
        PRIMARY KEY (generation, member_position),
        FOREIGN KEY (generation) REFERENCES snapshot_generations(generation) ON DELETE CASCADE,
        FOREIGN KEY (community_id) REFERENCES communities(community_id)
    ) STRICT;

    CREATE TABLE checkpoint_work (
        work_id BLOB PRIMARY KEY NOT NULL CHECK (length(work_id) = 32),
        publication_phase INTEGER NOT NULL CHECK (publication_phase BETWEEN 0 AND 6),
        temp_filename TEXT CHECK (temp_filename IS NULL OR length(temp_filename) > 0),
        published_filename TEXT CHECK (published_filename IS NULL OR length(published_filename) > 0),
        created_at INTEGER NOT NULL CHECK (created_at >= 0)
    ) STRICT;

    CREATE TABLE checkpoint_work_members (
        work_id BLOB NOT NULL CHECK (length(work_id) = 32),
        member_position INTEGER NOT NULL CHECK (member_position >= 0),
        community_id BLOB NOT NULL CHECK (length(community_id) = 32),
        frozen_head_digest BLOB NOT NULL CHECK (length(frozen_head_digest) = 32),
        PRIMARY KEY (work_id, member_position),
        FOREIGN KEY (work_id) REFERENCES checkpoint_work(work_id) ON DELETE CASCADE
    ) STRICT;

    CREATE TABLE hosting_receipts (
        receipt_id BLOB PRIMARY KEY NOT NULL CHECK (length(receipt_id) = 32),
        community_id BLOB CHECK (community_id IS NULL OR length(community_id) = 32),
        created_at INTEGER NOT NULL CHECK (created_at >= 0),
        receipt_bytes BLOB NOT NULL CHECK (length(receipt_bytes) > 0),
        FOREIGN KEY (community_id) REFERENCES communities(community_id)
    ) STRICT;

    CREATE TABLE staged_operations (
        operation_id BLOB PRIMARY KEY NOT NULL CHECK (length(operation_id) = 32),
        source_key BLOB NOT NULL CHECK (length(source_key) > 0 AND length(source_key) <= 64),
        staged_at INTEGER NOT NULL CHECK (staged_at >= 0),
        stage_deadline INTEGER NOT NULL CHECK (stage_deadline > staged_at),
        staged_bytes INTEGER NOT NULL CHECK (staged_bytes >= 0)
    ) STRICT;

    CREATE TABLE idempotency_key_index (
        control_request_digest BLOB PRIMARY KEY NOT NULL CHECK (length(control_request_digest) = 32),
        idempotency_key BLOB NOT NULL UNIQUE CHECK (length(idempotency_key) = 16),
        result_class INTEGER NOT NULL CHECK (result_class IN (0, 1)),
        claim_state INTEGER NOT NULL DEFAULT 0 CHECK (claim_state BETWEEN 0 AND 2),
        operation_id BLOB CHECK (operation_id IS NULL OR length(operation_id) = 32),
        lease_expires_at INTEGER CHECK (lease_expires_at IS NULL OR lease_expires_at >= 0),
        created_at INTEGER NOT NULL CHECK (created_at >= 0),
        expires_at INTEGER NOT NULL CHECK (expires_at >= created_at)
    ) STRICT;

    CREATE TABLE operations (
        operation_id BLOB PRIMARY KEY NOT NULL CHECK (length(operation_id) = 32),
        originating_kind INTEGER NOT NULL CHECK (originating_kind IN (0, 1)),
        token_secret_epoch INTEGER NOT NULL CHECK (token_secret_epoch >= 0),
        base_generation INTEGER NOT NULL CHECK (base_generation >= 0),
        operation_status INTEGER NOT NULL DEFAULT 0 CHECK (operation_status BETWEEN 0 AND 2),
        created_at INTEGER NOT NULL CHECK (created_at >= 0),
        operation_expiry INTEGER NOT NULL CHECK (operation_expiry > created_at),
        retention_deadline INTEGER NOT NULL CHECK (retention_deadline >= operation_expiry),
        prepare_response_bytes BLOB NOT NULL CHECK (length(prepare_response_bytes) > 0 AND length(prepare_response_bytes) <= 16384),
        terminal_result_bytes BLOB CHECK (terminal_result_bytes IS NULL OR (length(terminal_result_bytes) > 0 AND length(terminal_result_bytes) <= 16384))
    ) STRICT;

    CREATE TABLE ordinary_results (
        control_request_digest BLOB PRIMARY KEY NOT NULL CHECK (length(control_request_digest) = 32),
        result_bytes BLOB NOT NULL CHECK (length(result_bytes) > 0),
        FOREIGN KEY (control_request_digest)
            REFERENCES idempotency_key_index(control_request_digest) ON DELETE CASCADE
    ) STRICT;

    CREATE TABLE reserved_results (
        control_request_digest BLOB PRIMARY KEY NOT NULL CHECK (length(control_request_digest) = 32),
        removal_slot_index INTEGER NOT NULL,
        result_bytes BLOB NOT NULL CHECK (length(result_bytes) > 0 AND length(result_bytes) <= 4096),
        FOREIGN KEY (control_request_digest)
            REFERENCES idempotency_key_index(control_request_digest) ON DELETE CASCADE,
        FOREIGN KEY (removal_slot_index) REFERENCES removal_slots(slot_index)
    ) STRICT;

    CREATE TABLE anchor_peers (
        peer_id BLOB PRIMARY KEY NOT NULL CHECK (length(peer_id) = 32),
        endpoint_bytes BLOB NOT NULL CHECK (length(endpoint_bytes) > 0),
        added_at INTEGER NOT NULL CHECK (added_at >= 0)
    ) STRICT;

    CREATE TABLE peer_session_generations (
        peer_id BLOB NOT NULL CHECK (length(peer_id) = 32),
        generation INTEGER NOT NULL CHECK (generation >= 0),
        established_at INTEGER NOT NULL CHECK (established_at >= 0),
        PRIMARY KEY (peer_id, generation),
        FOREIGN KEY (peer_id) REFERENCES anchor_peers(peer_id) ON DELETE CASCADE
    ) STRICT;

    CREATE TABLE replica_challenges (
        challenge_id BLOB PRIMARY KEY NOT NULL CHECK (length(challenge_id) = 32),
        peer_id BLOB NOT NULL CHECK (length(peer_id) = 32),
        challenge_bytes BLOB NOT NULL CHECK (length(challenge_bytes) > 0),
        issued_at INTEGER NOT NULL CHECK (issued_at >= 0),
        FOREIGN KEY (peer_id) REFERENCES anchor_peers(peer_id) ON DELETE CASCADE
    ) STRICT;

    CREATE TABLE consumed_attestations (
        attestation_digest BLOB PRIMARY KEY NOT NULL CHECK (length(attestation_digest) = 32),
        peer_id BLOB NOT NULL CHECK (length(peer_id) = 32),
        consumed_at INTEGER NOT NULL CHECK (consumed_at >= 0),
        FOREIGN KEY (peer_id) REFERENCES anchor_peers(peer_id) ON DELETE CASCADE
    ) STRICT;

    CREATE TABLE emergency_reserves (
        reserve_name TEXT PRIMARY KEY NOT NULL CHECK (length(reserve_name) BETWEEN 1 AND 64),
        default_value INTEGER NOT NULL CHECK (default_value >= 0),
        absolute_ceiling INTEGER NOT NULL CHECK (absolute_ceiling >= default_value),
        is_fixed INTEGER NOT NULL CHECK (is_fixed IN (0, 1))
    ) STRICT;

    CREATE INDEX idx_entries_payload_digest ON entries(payload_digest);
    CREATE INDEX idx_community_payload_refs_payload ON community_payload_refs(payload_digest);
    CREATE INDEX idx_listings_host_refresh ON listings(last_host_refresh_at);
    CREATE INDEX idx_removal_slots_unclaimed
        ON removal_slots(slot_index) WHERE claimed_by_community IS NULL;
    CREATE INDEX idx_staged_operations_deadline ON staged_operations(stage_deadline);
    CREATE INDEX idx_idempotency_expires ON idempotency_key_index(expires_at);
    CREATE INDEX idx_idempotency_key ON idempotency_key_index(idempotency_key);
    CREATE INDEX idx_operations_retention ON operations(retention_deadline);
    CREATE INDEX idx_directory_inclusions_community ON directory_inclusions(community_id);

    INSERT INTO operator_state(
        singleton, operator_key_id, schema_version, record_version,
        descriptor_bytes, descriptor_floor_generation
    ) VALUES (1, NULL, 1, 1, NULL, 0);

    INSERT INTO deployment_lease(singleton) VALUES (1);

    INSERT INTO emergency_reserves(reserve_name, default_value, absolute_ceiling, is_fixed) VALUES
        ('removal_metadata_reserve', 805306368, 3221225472, 0),
        ('removal_wal_fsync_reserve', 805306368, 3221225472, 0),
        ('owner_removal_verification_permits', 4, 4, 1),
        ('valid_removal_writer_permits', 2, 2, 1),
        ('emergency_checkpoint_worker', 1, 1, 1);

    PRAGMA user_version = 1;
"#;

/// SQL that preprovisions `2 * L` free removal slots via a bounded recursive
/// sequence. Every slot starts unclaimed.
fn preprovision_removal_slots_sql() -> String {
    let count = 2 * DEFAULT_LISTING_CEILING;
    format!(
        "INSERT INTO removal_slots(slot_index)
         WITH RECURSIVE slot_seq(i) AS (
             SELECT 0
             UNION ALL
             SELECT i + 1 FROM slot_seq WHERE i + 1 < {count}
         )
         SELECT i FROM slot_seq;"
    )
}

/// Applies the forward-only schema to `connection`.
///
/// The migration ledger (`schema_migrations`) is the version marker. Opening a
/// database whose ledger records a version NEWER than [`CURRENT_SCHEMA_VERSION`]
/// fails closed with [`SchemaError::VersionTooNew`]; this binary never migrates
/// backward or silently downgrades. Migrations only ever move forward. Running
/// `migrate` on an already-current database is a no-op. Returns the version the
/// database is at on success.
pub fn migrate(connection: &mut Connection) -> Result<u32, SchemaError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
             version INTEGER PRIMARY KEY NOT NULL CHECK (version > 0)
         ) STRICT;",
    )?;

    let found: u32 = transaction.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )?;
    if found > CURRENT_SCHEMA_VERSION {
        return Err(SchemaError::VersionTooNew {
            found,
            supported: CURRENT_SCHEMA_VERSION,
        });
    }

    if found < 1 {
        transaction.execute_batch(MIGRATION_ONE)?;
        transaction.execute_batch(&preprovision_removal_slots_sql())?;
        transaction.execute("INSERT INTO schema_migrations(version) VALUES (1)", [])?;
    }

    transaction.commit()?;
    Ok(CURRENT_SCHEMA_VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    /// Every logical table the design's Anchor Repository defines.
    const EXPECTED_TABLES: &[&str] = &[
        "schema_migrations",
        "operator_state",
        "deployment_lease",
        "cursor_secret_epochs",
        "communities",
        "manifests",
        "manifest_floors",
        "public_site_tickets",
        "namespaces",
        "entries",
        "payloads",
        "community_payload_refs",
        "removal_slots",
        "listings",
        "listing_conflict_floors",
        "directory_inclusions",
        "directory_feed_heads",
        "directory_checkpoints",
        "snapshot_generations",
        "snapshot_members",
        "checkpoint_work",
        "checkpoint_work_members",
        "hosting_receipts",
        "staged_operations",
        "idempotency_key_index",
        "operations",
        "ordinary_results",
        "reserved_results",
        "anchor_peers",
        "peer_session_generations",
        "replica_challenges",
        "consumed_attestations",
        "emergency_reserves",
    ];

    /// Every index the schema defines.
    const EXPECTED_INDEXES: &[&str] = &[
        "idx_entries_payload_digest",
        "idx_community_payload_refs_payload",
        "idx_listings_host_refresh",
        "idx_removal_slots_unclaimed",
        "idx_staged_operations_deadline",
        "idx_idempotency_expires",
        "idx_idempotency_key",
        "idx_operations_retention",
        "idx_directory_inclusions_community",
    ];

    fn fresh() -> Connection {
        let mut connection = Connection::open_in_memory().expect("open in-memory");
        connection
            .pragma_update(None, "foreign_keys", true)
            .expect("enable foreign keys");
        migrate(&mut connection).expect("migrate fresh database");
        connection
    }

    fn object_exists(connection: &Connection, kind: &str, name: &str) -> bool {
        connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_schema WHERE type = ?1 AND name = ?2",
                rusqlite::params![kind, name],
                |row| row.get::<_, i64>(0),
            )
            .expect("query sqlite_schema")
            > 0
    }

    #[test]
    fn migration_creates_every_logical_table() {
        let connection = fresh();
        for table in EXPECTED_TABLES {
            assert!(
                object_exists(&connection, "table", table),
                "expected table `{table}` to exist after migration"
            );
        }
    }

    #[test]
    fn every_content_table_is_strict() {
        let connection = fresh();
        for table in EXPECTED_TABLES {
            let strict: i64 = connection
                .query_row(
                    "SELECT strict FROM pragma_table_list WHERE schema = 'main' AND name = ?1",
                    [table],
                    |row| row.get(0),
                )
                .unwrap_or_else(|_| panic!("table_list for `{table}`"));
            assert_eq!(strict, 1, "table `{table}` must be STRICT");
        }
    }

    #[test]
    fn migration_creates_every_index() {
        let connection = fresh();
        for index in EXPECTED_INDEXES {
            assert!(
                object_exists(&connection, "index", index),
                "expected index `{index}` to exist after migration"
            );
        }
    }

    #[test]
    fn migration_enforces_foreign_keys() {
        let connection = fresh();
        // `manifests.community_id` references `communities`; inserting a
        // manifest for a nonexistent community must be rejected.
        let result = connection.execute(
            "INSERT INTO manifests(community_id, manifest_generation, manifest_digest, manifest_bytes) \
             VALUES (?1, 1, ?2, ?3)",
            rusqlite::params![vec![7u8; 32], vec![9u8; 32], vec![1u8; 4]],
        );
        assert!(
            result.is_err(),
            "foreign key on manifests.community_id must be enforced"
        );
    }

    #[test]
    fn migration_enforces_blob_length_checks() {
        let connection = fresh();
        // `communities.community_id` must be 32 bytes.
        let result = connection.execute(
            "INSERT INTO communities(community_id, created_at, logical_bytes) VALUES (?1, 0, 0)",
            rusqlite::params![vec![1u8; 8]],
        );
        assert!(
            result.is_err(),
            "length CHECK on communities.community_id must be enforced"
        );
    }

    #[test]
    fn migration_enforces_quota_ceiling_checks() {
        let connection = fresh();
        // A negative logical byte count is impossible; the CHECK must reject it.
        let result = connection.execute(
            "INSERT INTO communities(community_id, created_at, logical_bytes) VALUES (?1, 0, -1)",
            rusqlite::params![vec![2u8; 32]],
        );
        assert!(
            result.is_err(),
            "non-negative CHECK on communities.logical_bytes must be enforced"
        );
    }

    #[test]
    fn fresh_database_reports_current_version() {
        let mut connection = Connection::open_in_memory().expect("open");
        let applied = migrate(&mut connection).expect("migrate");
        assert_eq!(applied, CURRENT_SCHEMA_VERSION);
        let user_version: u32 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("user_version");
        assert_eq!(user_version, CURRENT_SCHEMA_VERSION);
        let ledger: u32 = connection
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .expect("ledger");
        assert_eq!(ledger, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn forward_migration_is_idempotent() {
        let mut connection = Connection::open_in_memory().expect("open");
        let first = migrate(&mut connection).expect("first migrate");
        let table_count_first: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table'",
                [],
                |row| row.get(0),
            )
            .expect("count");
        let slots_first: i64 = connection
            .query_row("SELECT COUNT(*) FROM removal_slots", [], |row| row.get(0))
            .expect("slots");
        // Re-running migrate must be a no-op: no error, same version, same
        // structure, and no duplicated preprovisioned rows.
        let second = migrate(&mut connection).expect("second migrate");
        assert_eq!(first, second);
        let table_count_second: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table'",
                [],
                |row| row.get(0),
            )
            .expect("count");
        let slots_second: i64 = connection
            .query_row("SELECT COUNT(*) FROM removal_slots", [], |row| row.get(0))
            .expect("slots");
        assert_eq!(table_count_first, table_count_second);
        assert_eq!(slots_first, slots_second);
    }

    #[test]
    fn newer_version_database_is_refused() {
        let mut connection = Connection::open_in_memory().expect("open");
        migrate(&mut connection).expect("migrate to current");
        // Simulate a database written by a NEWER binary.
        let future = CURRENT_SCHEMA_VERSION + 1;
        connection
            .execute(
                "INSERT INTO schema_migrations(version) VALUES (?1)",
                [future],
            )
            .expect("stamp future migration row");
        connection
            .pragma_update(None, "user_version", future)
            .expect("bump user_version");
        let error = migrate(&mut connection).expect_err("must fail closed");
        match error {
            SchemaError::VersionTooNew { found, supported } => {
                assert_eq!(found, future);
                assert_eq!(supported, CURRENT_SCHEMA_VERSION);
            }
            other => panic!("expected VersionTooNew, got {other:?}"),
        }
    }

    #[test]
    fn preprovisioned_removal_slots_present() {
        let connection = fresh();
        let count: u32 = connection
            .query_row("SELECT COUNT(*) FROM removal_slots", [], |row| row.get(0))
            .expect("count removal slots");
        assert_eq!(count, 2 * DEFAULT_LISTING_CEILING);
    }

    #[test]
    fn preprovisioned_removal_slots_start_unclaimed() {
        let connection = fresh();
        let claimed: u32 = connection
            .query_row(
                "SELECT COUNT(*) FROM removal_slots WHERE claimed_by_community IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .expect("count claimed slots");
        assert_eq!(claimed, 0, "all preprovisioned removal slots start free");
    }

    #[test]
    fn emergency_reserve_rows_present() {
        let connection = fresh();
        // Both emergency reserves (metadata + WAL/fsync) and the three fixed
        // reservation partitions must be seeded.
        for name in [
            "removal_metadata_reserve",
            "removal_wal_fsync_reserve",
            "owner_removal_verification_permits",
            "valid_removal_writer_permits",
            "emergency_checkpoint_worker",
        ] {
            let present: u32 = connection
                .query_row(
                    "SELECT COUNT(*) FROM emergency_reserves WHERE reserve_name = ?1",
                    [name],
                    |row| row.get(0),
                )
                .expect("query reserve row");
            assert_eq!(present, 1, "emergency reserve row `{name}` must be seeded");
        }
    }

    #[test]
    fn operator_state_is_singleton_seeded_at_current_versions() {
        let connection = fresh();
        let (count, schema_version): (u32, u32) = connection
            .query_row(
                "SELECT COUNT(*), COALESCE(MIN(schema_version), 0) FROM operator_state WHERE singleton = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("operator_state");
        assert_eq!(count, 1);
        assert_eq!(schema_version, CURRENT_SCHEMA_VERSION);
    }
}
