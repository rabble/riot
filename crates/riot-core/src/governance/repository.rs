//! Durable authority repository: governance journal and derived indexes.

#[cfg(feature = "sqlite")]
use std::collections::BTreeSet;
use std::sync::Mutex;

#[cfg(feature = "sqlite")]
use super::body::{target_id_of, Body};
use super::evaluator::{evaluate, PolicySnapshot};
#[cfg(feature = "sqlite")]
use super::record::{decode_record, encode_record};
use super::record::{record_id, GovernanceRecordV1};
use super::GovernanceError;
#[cfg(feature = "sqlite")]
use super::{Fingerprint, RecordKind};

#[cfg(feature = "sqlite")]
use crate::store::{map_sqlite_error, RiotDatabase, WriteEstimate};
#[cfg(feature = "sqlite")]
use rusqlite::OptionalExtension;

pub enum AuthorityRepository {
    Memory(Mutex<Vec<GovernanceRecordV1>>),
    #[cfg(feature = "sqlite")]
    Sqlite(RiotDatabase),
}

impl AuthorityRepository {
    pub fn memory() -> Self {
        Self::Memory(Mutex::new(Vec::new()))
    }
    #[cfg(feature = "sqlite")]
    pub fn sqlite(database: RiotDatabase) -> Self {
        Self::Sqlite(database)
    }

    pub fn ingest(&self, record: &GovernanceRecordV1) -> Result<(), GovernanceError> {
        match self {
            Self::Memory(records) => {
                records
                    .lock()
                    .map_err(|_| GovernanceError::Storage)?
                    .push(record.clone());
                Ok(())
            }
            #[cfg(feature = "sqlite")]
            Self::Sqlite(database) => {
                let bytes = encode_record(record);
                let rid = record_id(record);
                let target = target_id_of(record.kind, &record.body, &record.actor_id);
                let action_head = match &record.body {
                    Body::ActionReceipt { receipt } => {
                        let receipt = super::action::decode_receipt(&receipt.0)?;
                        Some((
                            receipt.actor_id,
                            receipt.receiver,
                            super::action::action_hash(&receipt),
                            receipt.actor_sequence.to_be_bytes(),
                        ))
                    }
                    _ => None,
                };
                database.write_transaction(WriteEstimate::new(bytes.len(), 128), |tx| {
                    tx.execute("INSERT OR IGNORE INTO governance_journal(namespace_id, record_id, kind, actor_id, sequence_be, authorizing_fingerprint, record_bytes, accepted_generation) VALUES (?1,?2,?3,?4,?5,?6,?7,(SELECT generation FROM database_meta WHERE singleton=1))",
                        rusqlite::params![&record.namespace[..], &rid[..], record.kind.tag() as i64, &record.actor_id[..], &record.sequence.to_be_bytes()[..], &record.authorizing_fingerprint[..], &bytes[..]]).map_err(map_sqlite_error)?;
                    tx.execute("INSERT OR IGNORE INTO governance_by_target(namespace_id, kind, target_id, record_id) VALUES (?1,?2,?3,?4)", rusqlite::params![&record.namespace[..], record.kind.tag() as i64, &target[..], &rid[..]]).map_err(map_sqlite_error)?;
                    match &record.body {
                        Body::CapabilityIssued { covering_parent_fingerprint, child_fingerprint, .. }
                        | Body::CapabilityRenewed { covering_parent_fingerprint, child_fingerprint, .. } => {
                            tx.execute("INSERT OR IGNORE INTO capability_lineage(namespace_id, child_fingerprint, parent_fingerprint) VALUES (?1,?2,?3)", rusqlite::params![&record.namespace[..], &child_fingerprint[..], &covering_parent_fingerprint[..]]).map_err(map_sqlite_error)?;
                        }
                        Body::CapabilityRevoked { target_fingerprint, .. } => {
                            tx.execute("INSERT OR IGNORE INTO revocation_index(namespace_id, target_fingerprint, record_id) VALUES (?1,?2,?3)", rusqlite::params![&record.namespace[..], &target_fingerprint[..], &rid[..]]).map_err(map_sqlite_error)?;
                        }
                        _ => {}
                    }
                    if let Some((actor, receiver, hash, sequence)) = action_head {
                        tx.execute("INSERT OR REPLACE INTO action_heads(namespace_id, actor_id, receiver_id, action_hash, sequence_be) VALUES (?1,?2,?3,?4,?5)", rusqlite::params![&record.namespace[..], &actor[..], &receiver[..], &hash[..], &sequence[..]]).map_err(map_sqlite_error)?;
                    }
                    Ok(())
                }).map_err(|_| GovernanceError::Storage)
            }
        }
    }

    #[cfg(feature = "sqlite")]
    pub fn records_for_target(
        &self,
        kind: RecordKind,
        target: &[u8; 32],
    ) -> Result<Vec<[u8; 32]>, GovernanceError> {
        match self {
            Self::Sqlite(database) => database.read_connection(|c| {
                let mut s = c.prepare("SELECT record_id FROM governance_by_target WHERE kind=?1 AND target_id=?2 ORDER BY record_id").map_err(map_sqlite_error)?;
                let rows = s.query_map(rusqlite::params![kind.tag() as i64, &target[..]], |r| r.get::<_, Vec<u8>>(0)).map_err(map_sqlite_error)?;
                rows.map(|row| row.map_err(map_sqlite_error)?.try_into().map_err(|_| crate::store::DatabaseError::CorruptDatabase)).collect()
            }).map_err(|_| GovernanceError::Storage),
            Self::Memory(_) => Ok(Vec::new()),
        }
    }

    #[cfg(feature = "sqlite")]
    pub fn revocations_for(
        &self,
        fingerprint: &Fingerprint,
    ) -> Result<Vec<[u8; 32]>, GovernanceError> {
        match self {
            Self::Sqlite(database) => database.read_connection(|c| {
                let mut s = c.prepare("SELECT record_id FROM revocation_index WHERE target_fingerprint=?1 ORDER BY record_id").map_err(map_sqlite_error)?;
                let rows = s.query_map(rusqlite::params![&fingerprint[..]], |r| r.get::<_, Vec<u8>>(0)).map_err(map_sqlite_error)?;
                rows.map(|row| row.map_err(map_sqlite_error)?.try_into().map_err(|_| crate::store::DatabaseError::CorruptDatabase)).collect()
            }).map_err(|_| GovernanceError::Storage),
            Self::Memory(_) => Ok(Vec::new()),
        }
    }

    #[cfg(feature = "sqlite")]
    pub fn action_head_for(
        &self,
        actor: &[u8; 32],
        receiver: &[u8; 32],
    ) -> Result<Option<[u8; 32]>, GovernanceError> {
        match self {
            Self::Sqlite(database) => database.read_connection(|c| {
                let head: Option<Vec<u8>> = c.query_row("SELECT action_hash FROM action_heads WHERE actor_id=?1 AND receiver_id=?2", rusqlite::params![&actor[..], &receiver[..]], |r| r.get(0)).optional().map_err(map_sqlite_error)?;
                head.map(|bytes| bytes.try_into().map_err(|_| crate::store::DatabaseError::CorruptDatabase)).transpose()
            }).map_err(|_| GovernanceError::Storage),
            Self::Memory(_) => Ok(None),
        }
    }

    pub fn load_journal(&self) -> Result<Vec<GovernanceRecordV1>, GovernanceError> {
        let mut records = match self {
            Self::Memory(records) => records
                .lock()
                .map_err(|_| GovernanceError::Storage)?
                .clone(),
            #[cfg(feature = "sqlite")]
            Self::Sqlite(database) => database
                .read_connection(|c| {
                    let mut s = c
                        .prepare("SELECT record_bytes FROM governance_journal ORDER BY record_id")
                        .map_err(map_sqlite_error)?;
                    let rows = s
                        .query_map([], |r| r.get::<_, Vec<u8>>(0))
                        .map_err(map_sqlite_error)?;
                    rows.map(|row| {
                        decode_record(&row.map_err(map_sqlite_error)?)
                            .map_err(|_| crate::store::DatabaseError::CorruptDatabase)
                    })
                    .collect::<Result<Vec<_>, _>>()
                })
                .map_err(|_| GovernanceError::Storage)?,
        };
        records.sort_by_key(record_id);
        Ok(records)
    }

    pub fn snapshot(&self, now_micros: u64) -> Result<PolicySnapshot, GovernanceError> {
        Ok(evaluate(&self.load_journal()?, Some(now_micros)))
    }

    #[cfg(feature = "sqlite")]
    pub fn snapshot_respecting_quarantine(
        &self,
        now_micros: u64,
    ) -> Result<PolicySnapshot, GovernanceError> {
        match self {
            Self::Sqlite(database)
                if database
                    .authority_quarantined()
                    .map_err(|_| GovernanceError::Storage)? =>
            {
                let snapshot = self.snapshot(now_micros)?;
                Ok(PolicySnapshot {
                    active_fingerprints: BTreeSet::new(),
                    ..snapshot
                })
            }
            _ => self.snapshot(now_micros),
        }
    }
}

#[cfg(test)]
mod memory_tests {
    use super::*;
    use crate::governance::test_support::genesis_record;
    #[test]
    fn a_poisoned_memory_mutex_yields_a_storage_error() {
        let repository = AuthorityRepository::memory();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let AuthorityRepository::Memory(records) = &repository;
            let _guard = records.lock().unwrap();
            panic!("poison the lock");
        }));
        assert_eq!(
            repository.ingest(&genesis_record([9u8; 32])),
            Err(GovernanceError::Storage)
        );
    }
}
