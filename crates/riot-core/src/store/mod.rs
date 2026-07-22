//! Durable SQLite lifecycle primitives.
//!
//! This module owns opening, configuring, migrating, checking, backing up, and
//! restoring the physical database. Willow storage is layered on top in later
//! modules; native callers never receive a raw SQLite connection.

#[cfg(feature = "sqlite")]
mod backup;
#[cfg(feature = "sqlite")]
mod database;
pub(crate) mod evidence;
mod memory;
#[cfg(feature = "sqlite")]
mod schema;

#[cfg(feature = "sqlite")]
pub use backup::BackupManifest;
#[cfg(feature = "sqlite")]
pub(crate) use database::{map_sqlite_error, WriteEstimate};
#[cfg(feature = "sqlite")]
pub use database::{
    CheckpointMode, CheckpointResult, DatabaseConfig, DatabaseError, DatabaseSettings, Durability,
    JournalMode, RiotDatabase, RiotReadSnapshot,
};
#[cfg(feature = "sqlite")]
pub use schema::CURRENT_SCHEMA_VERSION;
