use super::database::{map_sqlite_error, DatabaseError};
use rusqlite::{Connection, TransactionBehavior};

pub const CURRENT_SCHEMA_VERSION: u32 = 3;

const SCHEMA_MIGRATIONS_SQL: &str = "CREATE TABLE schema_migrations (
    version INTEGER PRIMARY KEY NOT NULL CHECK (version > 0)
) STRICT";
const DATABASE_META_SQL: &str = "CREATE TABLE database_meta (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    database_id BLOB NOT NULL CHECK (length(database_id) = 16),
    database_generation BLOB NOT NULL CHECK (length(database_generation) = 16),
    generation INTEGER NOT NULL CHECK (generation >= 0),
    authority_quarantined INTEGER NOT NULL CHECK (authority_quarantined IN (0, 1))
) STRICT";
const LOCAL_STATE_SQL: &str = "CREATE TABLE local_state (
    key TEXT PRIMARY KEY NOT NULL CHECK (length(key) BETWEEN 1 AND 128),
    value BLOB NOT NULL CHECK (length(value) <= 1048576)
) STRICT, WITHOUT ROWID";
const EVIDENCE_META_SQL: &str = "CREATE TABLE evidence_meta (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    generation INTEGER NOT NULL CHECK (generation >= 0),
    next_receipt_id INTEGER NOT NULL CHECK (next_receipt_id > 0),
    retained_receipt_charge_bytes INTEGER NOT NULL CHECK (retained_receipt_charge_bytes >= 0)
) STRICT";
const ACCEPTED_ENTRIES_SQL: &str = "CREATE TABLE accepted_entries (
    namespace_id BLOB NOT NULL CHECK (length(namespace_id) = 32),
    entry_id BLOB NOT NULL CHECK (length(entry_id) = 32),
    subspace_id BLOB NOT NULL CHECK (length(subspace_id) = 32),
    path_bytes BLOB NOT NULL,
    timestamp_be BLOB NOT NULL CHECK (length(timestamp_be) = 8),
    payload_digest BLOB NOT NULL CHECK (length(payload_digest) = 32),
    payload_length INTEGER NOT NULL CHECK (payload_length >= 0),
    entry_bytes BLOB NOT NULL CHECK (length(entry_bytes) > 0),
    capability_bytes BLOB NOT NULL CHECK (length(capability_bytes) > 0),
    signature_bytes BLOB NOT NULL CHECK (length(signature_bytes) = 64),
    first_receipt_id INTEGER NOT NULL CHECK (first_receipt_id > 0),
    dominated_on_arrival INTEGER NOT NULL CHECK (dominated_on_arrival IN (0, 1)),
    PRIMARY KEY(namespace_id, entry_id),
    FOREIGN KEY(first_receipt_id) REFERENCES import_receipts(receipt_id)
        DEFERRABLE INITIALLY DEFERRED
) STRICT, WITHOUT ROWID";
const LIVE_ENTRIES_SQL: &str = "CREATE TABLE live_entries (
    namespace_id BLOB NOT NULL CHECK (length(namespace_id) = 32),
    entry_id BLOB NOT NULL CHECK (length(entry_id) = 32),
    subspace_id BLOB NOT NULL CHECK (length(subspace_id) = 32),
    path_bytes BLOB NOT NULL,
    timestamp_be BLOB NOT NULL CHECK (length(timestamp_be) = 8),
    payload_digest BLOB NOT NULL CHECK (length(payload_digest) = 32),
    payload_length INTEGER NOT NULL CHECK (payload_length >= 0),
    payload BLOB,
    PRIMARY KEY(namespace_id, entry_id),
    FOREIGN KEY(namespace_id, entry_id)
        REFERENCES accepted_entries(namespace_id, entry_id) ON DELETE CASCADE
) STRICT, WITHOUT ROWID";
const ENTRY_PATH_PREFIXES_SQL: &str = "CREATE TABLE entry_path_prefixes (
    namespace_id BLOB NOT NULL CHECK (length(namespace_id) = 32),
    entry_id BLOB NOT NULL CHECK (length(entry_id) = 32),
    depth INTEGER NOT NULL CHECK (depth >= 0),
    prefix_bytes BLOB NOT NULL,
    PRIMARY KEY(namespace_id, entry_id, depth),
    FOREIGN KEY(namespace_id, entry_id)
        REFERENCES live_entries(namespace_id, entry_id) ON DELETE CASCADE
) STRICT, WITHOUT ROWID";
const ENTRY_PATH_PREFIX_LOOKUP_SQL: &str = "CREATE INDEX entry_path_prefix_lookup
    ON entry_path_prefixes(namespace_id, depth, prefix_bytes, entry_id)";
const IMPORT_RECEIPTS_SQL: &str = "CREATE TABLE import_receipts (
    receipt_id INTEGER PRIMARY KEY NOT NULL CHECK (receipt_id > 0),
    route TEXT NOT NULL,
    before_generation INTEGER NOT NULL CHECK (before_generation >= 0),
    after_generation INTEGER NOT NULL CHECK (after_generation = before_generation + 1)
) STRICT";
const IMPORT_DISPOSITIONS_SQL: &str = "CREATE TABLE import_dispositions (
    namespace_id BLOB NOT NULL CHECK (length(namespace_id) = 32),
    receipt_id INTEGER NOT NULL,
    position INTEGER NOT NULL CHECK (position >= 0),
    entry_id BLOB NOT NULL CHECK (length(entry_id) = 32),
    kind INTEGER NOT NULL CHECK (kind BETWEEN 0 AND 2),
    insertion_receipt_id INTEGER,
    PRIMARY KEY(namespace_id, receipt_id, position),
    FOREIGN KEY(receipt_id)
        REFERENCES import_receipts(receipt_id) ON DELETE CASCADE,
    FOREIGN KEY(namespace_id, entry_id)
        REFERENCES accepted_entries(namespace_id, entry_id),
    FOREIGN KEY(insertion_receipt_id) REFERENCES import_receipts(receipt_id)
) STRICT, WITHOUT ROWID";
const IMPORT_REFERENCES_SQL: &str = "CREATE TABLE import_references (
    namespace_id BLOB NOT NULL CHECK (length(namespace_id) = 32),
    receipt_id INTEGER NOT NULL,
    disposition_position INTEGER NOT NULL,
    reference_position INTEGER NOT NULL CHECK (reference_position >= 0),
    entry_id BLOB NOT NULL CHECK (length(entry_id) = 32),
    PRIMARY KEY(namespace_id, receipt_id, disposition_position, reference_position),
    FOREIGN KEY(namespace_id, receipt_id, disposition_position)
        REFERENCES import_dispositions(namespace_id, receipt_id, position) ON DELETE CASCADE,
    FOREIGN KEY(namespace_id, entry_id)
        REFERENCES accepted_entries(namespace_id, entry_id)
) STRICT, WITHOUT ROWID";
const FORGET_EVENTS_SQL: &str = "CREATE TABLE forget_events (
    namespace_id BLOB NOT NULL CHECK (length(namespace_id) = 32),
    entry_id BLOB NOT NULL CHECK (length(entry_id) = 32),
    forgotten_generation INTEGER PRIMARY KEY NOT NULL CHECK (forgotten_generation > 0),
    restored_generation INTEGER CHECK (
        restored_generation IS NULL OR restored_generation > forgotten_generation
    ),
    UNIQUE(namespace_id, entry_id, forgotten_generation),
    FOREIGN KEY(namespace_id, entry_id)
        REFERENCES accepted_entries(namespace_id, entry_id) ON DELETE CASCADE
) STRICT";
const FORGET_EVENTS_OPEN_ENTRY_SQL: &str = "CREATE UNIQUE INDEX forget_events_open_entry
    ON forget_events(namespace_id, entry_id) WHERE restored_generation IS NULL";
const FORGOTTEN_ENTRIES_SQL: &str = "CREATE TABLE forgotten_entries (
    namespace_id BLOB NOT NULL CHECK (length(namespace_id) = 32),
    entry_id BLOB NOT NULL CHECK (length(entry_id) = 32),
    forgotten_generation INTEGER NOT NULL CHECK (forgotten_generation > 0),
    PRIMARY KEY(namespace_id, entry_id),
    FOREIGN KEY(namespace_id, entry_id)
        REFERENCES accepted_entries(namespace_id, entry_id) ON DELETE CASCADE,
    FOREIGN KEY(namespace_id, entry_id, forgotten_generation)
        REFERENCES forget_events(namespace_id, entry_id, forgotten_generation)
) STRICT, WITHOUT ROWID";

const GOVERNANCE_JOURNAL_SQL: &str = "CREATE TABLE governance_journal (
    namespace_id BLOB NOT NULL CHECK (length(namespace_id) = 32),
    record_id BLOB NOT NULL CHECK (length(record_id) = 32),
    kind INTEGER NOT NULL CHECK (kind BETWEEN 0 AND 21),
    actor_id BLOB NOT NULL CHECK (length(actor_id) = 32),
    sequence_be BLOB NOT NULL CHECK (length(sequence_be) = 8),
    authorizing_fingerprint BLOB NOT NULL CHECK (length(authorizing_fingerprint) = 32),
    record_bytes BLOB NOT NULL CHECK (length(record_bytes) > 0),
    accepted_generation INTEGER NOT NULL CHECK (accepted_generation >= 0),
    PRIMARY KEY(namespace_id, record_id)
) STRICT, WITHOUT ROWID";
const GOVERNANCE_BY_TARGET_SQL: &str = "CREATE TABLE governance_by_target (
    namespace_id BLOB NOT NULL CHECK (length(namespace_id) = 32),
    kind INTEGER NOT NULL CHECK (kind BETWEEN 0 AND 21),
    target_id BLOB NOT NULL CHECK (length(target_id) = 32),
    record_id BLOB NOT NULL CHECK (length(record_id) = 32),
    PRIMARY KEY(namespace_id, kind, target_id, record_id),
    FOREIGN KEY(namespace_id, record_id) REFERENCES governance_journal(namespace_id, record_id) ON DELETE CASCADE
) STRICT, WITHOUT ROWID";
const CAPABILITY_LINEAGE_SQL: &str = "CREATE TABLE capability_lineage (
    namespace_id BLOB NOT NULL CHECK (length(namespace_id) = 32),
    child_fingerprint BLOB NOT NULL CHECK (length(child_fingerprint) = 32),
    parent_fingerprint BLOB NOT NULL CHECK (length(parent_fingerprint) = 32),
    PRIMARY KEY(namespace_id, child_fingerprint)
) STRICT, WITHOUT ROWID";
const REVOCATION_INDEX_SQL: &str = "CREATE TABLE revocation_index (
    namespace_id BLOB NOT NULL CHECK (length(namespace_id) = 32),
    target_fingerprint BLOB NOT NULL CHECK (length(target_fingerprint) = 32),
    record_id BLOB NOT NULL CHECK (length(record_id) = 32),
    PRIMARY KEY(namespace_id, target_fingerprint, record_id),
    FOREIGN KEY(namespace_id, record_id) REFERENCES governance_journal(namespace_id, record_id) ON DELETE CASCADE
) STRICT, WITHOUT ROWID";
const ACTION_HEADS_SQL: &str = "CREATE TABLE action_heads (
    namespace_id BLOB NOT NULL CHECK (length(namespace_id) = 32),
    actor_id BLOB NOT NULL CHECK (length(actor_id) = 32),
    receiver_id BLOB NOT NULL CHECK (length(receiver_id) = 32),
    action_hash BLOB NOT NULL CHECK (length(action_hash) = 32),
    sequence_be BLOB NOT NULL CHECK (length(sequence_be) = 8),
    PRIMARY KEY(namespace_id, actor_id, receiver_id)
) STRICT, WITHOUT ROWID";

const MIGRATION_ONE: &str = r#"
    CREATE TABLE database_meta (
        singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
        database_id BLOB NOT NULL CHECK (length(database_id) = 16),
        database_generation BLOB NOT NULL CHECK (length(database_generation) = 16),
        generation INTEGER NOT NULL CHECK (generation >= 0),
        authority_quarantined INTEGER NOT NULL CHECK (authority_quarantined IN (0, 1))
    ) STRICT;

    INSERT INTO database_meta(
        singleton, database_id, database_generation, generation, authority_quarantined
    ) VALUES (1, randomblob(16), randomblob(16), 0, 0);

    CREATE TABLE local_state (
        key TEXT PRIMARY KEY NOT NULL CHECK (length(key) BETWEEN 1 AND 128),
        value BLOB NOT NULL CHECK (length(value) <= 1048576)
    ) STRICT, WITHOUT ROWID;

    INSERT INTO schema_migrations(version) VALUES (1);
    PRAGMA user_version = 1;
"#;

const MIGRATION_TWO: &str = r#"
    CREATE TABLE evidence_meta (
        singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
        generation INTEGER NOT NULL CHECK (generation >= 0),
        next_receipt_id INTEGER NOT NULL CHECK (next_receipt_id > 0),
        retained_receipt_charge_bytes INTEGER NOT NULL CHECK (retained_receipt_charge_bytes >= 0)
    ) STRICT;
    INSERT INTO evidence_meta(singleton, generation, next_receipt_id, retained_receipt_charge_bytes)
        VALUES (1, 0, 1, 0);
    CREATE TABLE accepted_entries (
        namespace_id BLOB NOT NULL CHECK (length(namespace_id) = 32),
        entry_id BLOB NOT NULL CHECK (length(entry_id) = 32),
        subspace_id BLOB NOT NULL CHECK (length(subspace_id) = 32),
        path_bytes BLOB NOT NULL,
        timestamp_be BLOB NOT NULL CHECK (length(timestamp_be) = 8),
        payload_digest BLOB NOT NULL CHECK (length(payload_digest) = 32),
        payload_length INTEGER NOT NULL CHECK (payload_length >= 0),
        entry_bytes BLOB NOT NULL CHECK (length(entry_bytes) > 0),
        capability_bytes BLOB NOT NULL CHECK (length(capability_bytes) > 0),
        signature_bytes BLOB NOT NULL CHECK (length(signature_bytes) = 64),
        first_receipt_id INTEGER NOT NULL CHECK (first_receipt_id > 0),
        dominated_on_arrival INTEGER NOT NULL CHECK (dominated_on_arrival IN (0, 1)),
        PRIMARY KEY(namespace_id, entry_id),
        FOREIGN KEY(first_receipt_id) REFERENCES import_receipts(receipt_id)
            DEFERRABLE INITIALLY DEFERRED
    ) STRICT, WITHOUT ROWID;
    CREATE TABLE live_entries (
        namespace_id BLOB NOT NULL CHECK (length(namespace_id) = 32),
        entry_id BLOB NOT NULL CHECK (length(entry_id) = 32),
        subspace_id BLOB NOT NULL CHECK (length(subspace_id) = 32),
        path_bytes BLOB NOT NULL,
        timestamp_be BLOB NOT NULL CHECK (length(timestamp_be) = 8),
        payload_digest BLOB NOT NULL CHECK (length(payload_digest) = 32),
        payload_length INTEGER NOT NULL CHECK (payload_length >= 0),
        payload BLOB,
        PRIMARY KEY(namespace_id, entry_id),
        FOREIGN KEY(namespace_id, entry_id)
            REFERENCES accepted_entries(namespace_id, entry_id) ON DELETE CASCADE
    ) STRICT, WITHOUT ROWID;
    CREATE TABLE entry_path_prefixes (
        namespace_id BLOB NOT NULL CHECK (length(namespace_id) = 32),
        entry_id BLOB NOT NULL CHECK (length(entry_id) = 32),
        depth INTEGER NOT NULL CHECK (depth >= 0),
        prefix_bytes BLOB NOT NULL,
        PRIMARY KEY(namespace_id, entry_id, depth),
        FOREIGN KEY(namespace_id, entry_id)
            REFERENCES live_entries(namespace_id, entry_id) ON DELETE CASCADE
    ) STRICT, WITHOUT ROWID;
    CREATE INDEX entry_path_prefix_lookup
        ON entry_path_prefixes(namespace_id, depth, prefix_bytes, entry_id);
    CREATE TABLE import_receipts (
        receipt_id INTEGER PRIMARY KEY NOT NULL CHECK (receipt_id > 0),
        route TEXT NOT NULL,
        before_generation INTEGER NOT NULL CHECK (before_generation >= 0),
        after_generation INTEGER NOT NULL CHECK (after_generation = before_generation + 1)
    ) STRICT;
    CREATE TABLE import_dispositions (
        namespace_id BLOB NOT NULL CHECK (length(namespace_id) = 32),
        receipt_id INTEGER NOT NULL,
        position INTEGER NOT NULL CHECK (position >= 0),
        entry_id BLOB NOT NULL CHECK (length(entry_id) = 32),
        kind INTEGER NOT NULL CHECK (kind BETWEEN 0 AND 2),
        insertion_receipt_id INTEGER,
        PRIMARY KEY(namespace_id, receipt_id, position),
        FOREIGN KEY(receipt_id)
            REFERENCES import_receipts(receipt_id) ON DELETE CASCADE,
        FOREIGN KEY(namespace_id, entry_id)
            REFERENCES accepted_entries(namespace_id, entry_id),
        FOREIGN KEY(insertion_receipt_id) REFERENCES import_receipts(receipt_id)
    ) STRICT, WITHOUT ROWID;
    CREATE TABLE import_references (
        namespace_id BLOB NOT NULL CHECK (length(namespace_id) = 32),
        receipt_id INTEGER NOT NULL,
        disposition_position INTEGER NOT NULL,
        reference_position INTEGER NOT NULL CHECK (reference_position >= 0),
        entry_id BLOB NOT NULL CHECK (length(entry_id) = 32),
        PRIMARY KEY(namespace_id, receipt_id, disposition_position, reference_position),
        FOREIGN KEY(namespace_id, receipt_id, disposition_position)
            REFERENCES import_dispositions(namespace_id, receipt_id, position) ON DELETE CASCADE,
        FOREIGN KEY(namespace_id, entry_id)
            REFERENCES accepted_entries(namespace_id, entry_id)
    ) STRICT, WITHOUT ROWID;
    CREATE TABLE forget_events (
        namespace_id BLOB NOT NULL CHECK (length(namespace_id) = 32),
        entry_id BLOB NOT NULL CHECK (length(entry_id) = 32),
        forgotten_generation INTEGER PRIMARY KEY NOT NULL CHECK (forgotten_generation > 0),
        restored_generation INTEGER CHECK (
            restored_generation IS NULL OR restored_generation > forgotten_generation
        ),
        UNIQUE(namespace_id, entry_id, forgotten_generation),
        FOREIGN KEY(namespace_id, entry_id)
            REFERENCES accepted_entries(namespace_id, entry_id) ON DELETE CASCADE
    ) STRICT;
    CREATE UNIQUE INDEX forget_events_open_entry
        ON forget_events(namespace_id, entry_id) WHERE restored_generation IS NULL;
    CREATE TABLE forgotten_entries (
        namespace_id BLOB NOT NULL CHECK (length(namespace_id) = 32),
        entry_id BLOB NOT NULL CHECK (length(entry_id) = 32),
        forgotten_generation INTEGER NOT NULL CHECK (forgotten_generation > 0),
        PRIMARY KEY(namespace_id, entry_id),
        FOREIGN KEY(namespace_id, entry_id)
            REFERENCES accepted_entries(namespace_id, entry_id) ON DELETE CASCADE,
        FOREIGN KEY(namespace_id, entry_id, forgotten_generation)
            REFERENCES forget_events(namespace_id, entry_id, forgotten_generation)
    ) STRICT, WITHOUT ROWID;
    INSERT INTO schema_migrations(version) VALUES (2);
    PRAGMA user_version = 2;
"#;

const MIGRATION_THREE: &str = r#"
    CREATE TABLE governance_journal (
        namespace_id BLOB NOT NULL CHECK (length(namespace_id) = 32),
        record_id BLOB NOT NULL CHECK (length(record_id) = 32),
        kind INTEGER NOT NULL CHECK (kind BETWEEN 0 AND 21),
        actor_id BLOB NOT NULL CHECK (length(actor_id) = 32),
        sequence_be BLOB NOT NULL CHECK (length(sequence_be) = 8),
        authorizing_fingerprint BLOB NOT NULL CHECK (length(authorizing_fingerprint) = 32),
        record_bytes BLOB NOT NULL CHECK (length(record_bytes) > 0),
        accepted_generation INTEGER NOT NULL CHECK (accepted_generation >= 0),
        PRIMARY KEY(namespace_id, record_id)
    ) STRICT, WITHOUT ROWID;
    CREATE TABLE governance_by_target (
        namespace_id BLOB NOT NULL CHECK (length(namespace_id) = 32),
        kind INTEGER NOT NULL CHECK (kind BETWEEN 0 AND 21),
        target_id BLOB NOT NULL CHECK (length(target_id) = 32),
        record_id BLOB NOT NULL CHECK (length(record_id) = 32),
        PRIMARY KEY(namespace_id, kind, target_id, record_id),
        FOREIGN KEY(namespace_id, record_id) REFERENCES governance_journal(namespace_id, record_id) ON DELETE CASCADE
    ) STRICT, WITHOUT ROWID;
    CREATE TABLE capability_lineage (
        namespace_id BLOB NOT NULL CHECK (length(namespace_id) = 32),
        child_fingerprint BLOB NOT NULL CHECK (length(child_fingerprint) = 32),
        parent_fingerprint BLOB NOT NULL CHECK (length(parent_fingerprint) = 32),
        PRIMARY KEY(namespace_id, child_fingerprint)
    ) STRICT, WITHOUT ROWID;
    CREATE TABLE revocation_index (
        namespace_id BLOB NOT NULL CHECK (length(namespace_id) = 32),
        target_fingerprint BLOB NOT NULL CHECK (length(target_fingerprint) = 32),
        record_id BLOB NOT NULL CHECK (length(record_id) = 32),
        PRIMARY KEY(namespace_id, target_fingerprint, record_id),
        FOREIGN KEY(namespace_id, record_id) REFERENCES governance_journal(namespace_id, record_id) ON DELETE CASCADE
    ) STRICT, WITHOUT ROWID;
    CREATE TABLE action_heads (
        namespace_id BLOB NOT NULL CHECK (length(namespace_id) = 32),
        actor_id BLOB NOT NULL CHECK (length(actor_id) = 32),
        receiver_id BLOB NOT NULL CHECK (length(receiver_id) = 32),
        action_hash BLOB NOT NULL CHECK (length(action_hash) = 32),
        sequence_be BLOB NOT NULL CHECK (length(sequence_be) = 8),
        PRIMARY KEY(namespace_id, actor_id, receiver_id)
    ) STRICT, WITHOUT ROWID;
    INSERT INTO schema_migrations(version) VALUES (3);
    PRAGMA user_version = 3;
"#;

pub(crate) fn migrate(connection: &mut Connection) -> Result<(), DatabaseError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_sqlite_error)?;
    transaction
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (\
                 version INTEGER PRIMARY KEY NOT NULL CHECK (version > 0)\
             ) STRICT;",
        )
        .map_err(|_| DatabaseError::MigrationFailed)?;

    let found = schema_version_in(&transaction).map_err(|_| DatabaseError::MigrationFailed)?;
    if found > CURRENT_SCHEMA_VERSION {
        return Err(DatabaseError::MigrationRequired {
            found,
            supported: CURRENT_SCHEMA_VERSION,
        });
    }

    if found < 1 {
        transaction
            .execute_batch(MIGRATION_ONE)
            .map_err(|_| DatabaseError::MigrationFailed)?;
        canonicalize_version_zero_ledger(&transaction)?;
    }
    if found < 2 {
        transaction
            .execute_batch(MIGRATION_TWO)
            .map_err(|_| DatabaseError::MigrationFailed)?;
    }
    if found < 3 {
        transaction
            .execute_batch(MIGRATION_THREE)
            .map_err(|_| DatabaseError::MigrationFailed)?;
    }

    validate_structure(&transaction)?;

    transaction
        .commit()
        .map_err(|_| DatabaseError::MigrationFailed)
}

pub(crate) fn validate_supported(connection: &Connection) -> Result<u32, DatabaseError> {
    let found = validate_structure(connection)?;
    if found == CURRENT_SCHEMA_VERSION {
        Ok(found)
    } else {
        Err(DatabaseError::MigrationRequired {
            found,
            supported: CURRENT_SCHEMA_VERSION,
        })
    }
}

/// Checks an existing database before connection pragmas are allowed to
/// mutate it. An empty migration ledger is accepted as version zero; nonzero
/// version markers without their required structure fail closed.
pub(crate) fn preflight_existing(connection: &Connection) -> Result<(), DatabaseError> {
    let object_count: u32 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema WHERE type IN ('table', 'index', 'trigger', 'view')",
            [],
            |row| row.get(0),
        )
        .map_err(map_sqlite_error)?;
    if object_count == 0 {
        return Ok(());
    }
    if schema_version_in(connection).unwrap_or(u32::MAX) == 0 {
        let user_version: u32 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(|_| DatabaseError::CorruptDatabase)?;
        if user_version != 0 {
            return Err(DatabaseError::CorruptDatabase);
        }
        return Ok(());
    }
    validate_structure(connection).map(|_| ())
}

fn validate_structure(connection: &Connection) -> Result<u32, DatabaseError> {
    let mut statement = connection
        .prepare("SELECT version FROM schema_migrations ORDER BY version")
        .map_err(|_| DatabaseError::CorruptDatabase)?;
    let versions = statement
        .query_map([], |row| row.get::<_, i64>(0))
        .map_err(|_| DatabaseError::CorruptDatabase)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| DatabaseError::CorruptDatabase)?;
    drop(statement);
    if versions.is_empty()
        || versions
            .iter()
            .enumerate()
            .any(|(index, version)| *version != (index + 1) as i64)
    {
        return Err(DatabaseError::CorruptDatabase);
    }
    let found = u32::try_from(*versions.last().ok_or(DatabaseError::CorruptDatabase)?)
        .map_err(|_| DatabaseError::CorruptDatabase)?;
    if found > CURRENT_SCHEMA_VERSION {
        return Err(DatabaseError::MigrationRequired {
            found,
            supported: CURRENT_SCHEMA_VERSION,
        });
    }

    let user_version: u32 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|_| DatabaseError::CorruptDatabase)?;
    if user_version != found {
        return Err(DatabaseError::CorruptDatabase);
    }
    validate_columns(
        connection,
        "schema_migrations",
        &[("version", "INTEGER", true, 1)],
    )?;
    validate_columns(
        connection,
        "database_meta",
        &[
            ("singleton", "INTEGER", false, 1),
            ("database_id", "BLOB", true, 0),
            ("database_generation", "BLOB", true, 0),
            ("generation", "INTEGER", true, 0),
            ("authority_quarantined", "INTEGER", true, 0),
        ],
    )?;
    validate_columns(
        connection,
        "local_state",
        &[("key", "TEXT", true, 1), ("value", "BLOB", true, 0)],
    )?;
    validate_schema_definition(
        connection,
        "schema_migrations",
        SCHEMA_MIGRATIONS_SQL,
        false,
    )?;
    validate_schema_definition(connection, "database_meta", DATABASE_META_SQL, false)?;
    validate_schema_definition(connection, "local_state", LOCAL_STATE_SQL, true)?;
    if found >= 2 {
        validate_evidence_structure(connection)?;
    }
    if found >= 3 {
        validate_governance_structure(connection)?;
    }
    let meta: (u32, u32, u32, i64, i64) = connection
        .query_row(
            "SELECT COUNT(*),
                    COALESCE(SUM(singleton = 1), 0),
                    COALESCE(SUM(length(database_id) = 16 AND length(database_generation) = 16), 0),
                    COALESCE(MIN(generation), -1),
                    COALESCE(MIN(authority_quarantined), -1)
             FROM database_meta",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .map_err(|_| DatabaseError::CorruptDatabase)?;
    if meta.0 != 1 || meta.1 != 1 || meta.2 != 1 || meta.3 < 0 || !matches!(meta.4, 0 | 1) {
        return Err(DatabaseError::CorruptDatabase);
    }
    Ok(found)
}

fn validate_evidence_structure(connection: &Connection) -> Result<(), DatabaseError> {
    let tables = [
        ("evidence_meta", EVIDENCE_META_SQL, false),
        ("accepted_entries", ACCEPTED_ENTRIES_SQL, true),
        ("live_entries", LIVE_ENTRIES_SQL, true),
        ("entry_path_prefixes", ENTRY_PATH_PREFIXES_SQL, true),
        ("import_receipts", IMPORT_RECEIPTS_SQL, false),
        ("import_dispositions", IMPORT_DISPOSITIONS_SQL, true),
        ("import_references", IMPORT_REFERENCES_SQL, true),
        ("forget_events", FORGET_EVENTS_SQL, false),
        ("forgotten_entries", FORGOTTEN_ENTRIES_SQL, true),
    ];
    for (table, sql, without_rowid) in tables {
        validate_schema_definition(connection, table, sql, without_rowid)?;
    }
    validate_index_definition(
        connection,
        "entry_path_prefix_lookup",
        ENTRY_PATH_PREFIX_LOOKUP_SQL,
    )?;
    validate_index_definition(
        connection,
        "forget_events_open_entry",
        FORGET_EVENTS_OPEN_ENTRY_SQL,
    )?;
    let meta: (u32, i64, i64, i64) = connection
        .query_row(
            "SELECT COUNT(*), COALESCE(MIN(generation), -1),
                    COALESCE(MIN(next_receipt_id), 0),
                    COALESCE(MIN(retained_receipt_charge_bytes), -1)
             FROM evidence_meta WHERE singleton = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|_| DatabaseError::CorruptDatabase)?;
    if meta.0 != 1 || meta.1 < 0 || meta.2 <= 0 || meta.3 < 0 {
        return Err(DatabaseError::CorruptDatabase);
    }
    Ok(())
}

fn validate_governance_structure(connection: &Connection) -> Result<(), DatabaseError> {
    for (table, sql) in [
        ("governance_journal", GOVERNANCE_JOURNAL_SQL),
        ("governance_by_target", GOVERNANCE_BY_TARGET_SQL),
        ("capability_lineage", CAPABILITY_LINEAGE_SQL),
        ("revocation_index", REVOCATION_INDEX_SQL),
        ("action_heads", ACTION_HEADS_SQL),
    ] {
        validate_schema_definition(connection, table, sql, true)?;
    }
    Ok(())
}

fn validate_index_definition(
    connection: &Connection,
    index: &str,
    expected_sql: &str,
) -> Result<(), DatabaseError> {
    let sql: String = connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type = 'index' AND name = ?1",
            [index],
            |row| row.get(0),
        )
        .map_err(|_| DatabaseError::CorruptDatabase)?;
    if normalize_schema_sql(&sql) != normalize_schema_sql(expected_sql) {
        return Err(DatabaseError::CorruptDatabase);
    }
    Ok(())
}

fn canonicalize_version_zero_ledger(connection: &Connection) -> Result<(), DatabaseError> {
    let sql: String = connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = 'schema_migrations'",
            [],
            |row| row.get(0),
        )
        .map_err(|_| DatabaseError::MigrationFailed)?;
    if normalize_schema_sql(&sql) == normalize_schema_sql(SCHEMA_MIGRATIONS_SQL) {
        return Ok(());
    }
    connection
        .execute_batch(
            "ALTER TABLE schema_migrations RENAME TO schema_migrations_version_zero;
             CREATE TABLE schema_migrations (
                 version INTEGER PRIMARY KEY NOT NULL CHECK (version > 0)
             ) STRICT;
             INSERT INTO schema_migrations(version)
                 SELECT version FROM schema_migrations_version_zero;
             DROP TABLE schema_migrations_version_zero;",
        )
        .map_err(|_| DatabaseError::MigrationFailed)
}

fn validate_schema_definition(
    connection: &Connection,
    table: &str,
    expected_sql: &str,
    expected_without_rowid: bool,
) -> Result<(), DatabaseError> {
    let sql: String = connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = ?1",
            [table],
            |row| row.get(0),
        )
        .map_err(|_| DatabaseError::CorruptDatabase)?;
    if normalize_schema_sql(&sql) != normalize_schema_sql(expected_sql) {
        return Err(DatabaseError::CorruptDatabase);
    }
    let (without_rowid, strict): (i64, i64) = connection
        .query_row(
            "SELECT wr, strict
             FROM pragma_table_list
             WHERE schema = 'main' AND name = ?1 AND type = 'table'",
            [table],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| DatabaseError::CorruptDatabase)?;
    if (without_rowid != 0) != expected_without_rowid || strict != 1 {
        return Err(DatabaseError::CorruptDatabase);
    }
    Ok(())
}

fn normalize_schema_sql(sql: &str) -> String {
    sql.chars()
        .filter(|character| !character.is_ascii_whitespace())
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

fn validate_columns(
    connection: &Connection,
    table: &str,
    expected: &[(&str, &str, bool, i64)],
) -> Result<(), DatabaseError> {
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|_| DatabaseError::CorruptDatabase)?;
    let columns = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)? != 0,
                row.get::<_, i64>(5)?,
            ))
        })
        .map_err(|_| DatabaseError::CorruptDatabase)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| DatabaseError::CorruptDatabase)?;
    if columns.len() != expected.len()
        || columns.iter().zip(expected).any(
            |((name, column_type, not_null, primary_key), expected)| {
                name != expected.0
                    || !column_type.eq_ignore_ascii_case(expected.1)
                    || *not_null != expected.2
                    || *primary_key != expected.3
            },
        )
    {
        return Err(DatabaseError::CorruptDatabase);
    }
    Ok(())
}

pub(crate) fn schema_version_in(connection: &Connection) -> rusqlite::Result<u32> {
    connection.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )
}

#[cfg(test)]
mod tests {
    use crate::store::{DatabaseConfig, RiotDatabase};

    #[test]
    fn fresh_database_is_governance_schema_v3() {
        let path = std::env::temp_dir().join(format!(
            "riot-governance-schema-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let database = RiotDatabase::open(&path, DatabaseConfig::default()).unwrap();
        assert_eq!(database.schema_version().unwrap(), 3);
        drop(database);
        let _ = std::fs::remove_file(path);
    }
}
