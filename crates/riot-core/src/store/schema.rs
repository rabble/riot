use super::database::{map_sqlite_error, DatabaseError};
use rusqlite::{Connection, TransactionBehavior};

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

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

    validate_structure(&transaction)?;

    transaction
        .commit()
        .map_err(|_| DatabaseError::MigrationFailed)
}

pub(crate) fn validate_supported(connection: &Connection) -> Result<u32, DatabaseError> {
    validate_structure(connection)
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
    if found < CURRENT_SCHEMA_VERSION {
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
