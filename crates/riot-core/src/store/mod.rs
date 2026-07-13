//! Durable SQLite lifecycle primitives.
//!
//! This module owns opening, configuring, migrating, checking, backing up, and
//! restoring the physical database. Willow storage is layered on top in later
//! modules; native callers never receive a raw SQLite connection.

mod backup;
mod database;
mod schema;

pub use backup::BackupManifest;
pub use database::{
    CheckpointMode, CheckpointResult, DatabaseConfig, DatabaseError, DatabaseSettings, Durability,
    JournalMode, RiotDatabase, RiotReadSnapshot,
};
pub use schema::CURRENT_SCHEMA_VERSION;
