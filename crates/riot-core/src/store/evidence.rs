//! Durable evidence authority layered on the managed SQLite lifecycle.

use super::database::{map_sqlite_error, WriteEstimate};
use super::memory::MemoryEvidenceStore;
use super::RiotDatabase;
use crate::import::join::{JoinState, LiveJoinEntry, PersistedJoinEntry};
use crate::session::{DispositionRow, EntryDisposition, ImportReceipt, SessionError};
use crate::willow::{decode_capability_canonic, decode_entry_canonic, AuthorisationToken, EntryId};
use rusqlite::{params, OptionalExtension, Transaction};
use std::collections::{BTreeMap, BTreeSet};
use willow25::authorisation::PossiblyAuthorisedEntry;
use willow25::entry::{Entrylike, EntrylikeExt, SubspaceSignature};
use willow25::groupings::{Coordinatelike, Keylike, Namespaced};

const EVIDENCE_WRITE_HEADROOM: u32 = 64;
const MAX_ACCEPTED_ENTRIES: usize = 1_024;
const MAX_LIVE_ENTRIES: usize = 1_024;
const MAX_IMPORT_RECEIPTS: usize = 256;
const MAX_NAMESPACE_VIEWS: usize = 64;
const RECEIPT_ROW_CHARGE: u64 = 256;
const REFERENCE_CHARGE: u64 = 32;

pub(crate) struct EvidenceSnapshot {
    pub(crate) generation: u64,
    pub(crate) join: JoinState,
    pub(crate) receipts: Vec<ImportReceipt>,
    pub(crate) first_receipt: Vec<(EntryId, u64, bool)>,
    pub(crate) next_receipt_id: u64,
    pub(crate) retained_receipt_charge_bytes: u64,
    pub(crate) seen_namespaces: Vec<[u8; 32]>,
}

impl EvidenceSnapshot {
    pub(crate) fn empty() -> Self {
        Self {
            generation: 0,
            join: JoinState::new(),
            receipts: Vec::new(),
            first_receipt: Vec::new(),
            next_receipt_id: 1,
            retained_receipt_charge_bytes: 0,
            seen_namespaces: Vec::new(),
        }
    }
}

pub(crate) struct AcceptedEvidence {
    pub(crate) entry_id: EntryId,
    pub(crate) entry_bytes: Vec<u8>,
    pub(crate) capability_bytes: Vec<u8>,
    pub(crate) signature_bytes: [u8; 64],
    pub(crate) first_receipt_id: u64,
    pub(crate) dominated_on_arrival: bool,
}

pub(crate) struct EvidenceMutation {
    pub(crate) expected_generation: u64,
    pub(crate) generation: u64,
    pub(crate) next_receipt_id: u64,
    pub(crate) retained_receipt_charge_bytes: u64,
    pub(crate) accepted: Vec<AcceptedEvidence>,
    pub(crate) live: Vec<LiveJoinEntry>,
    pub(crate) forgotten: Vec<EntryId>,
    pub(crate) receipt: Option<ImportReceipt>,
    pub(crate) disposition_namespaces: Vec<[u8; 32]>,
}

pub(crate) enum EvidenceRepository {
    Memory(MemoryEvidenceStore),
    Sqlite(SqliteEvidenceStore),
}

impl EvidenceRepository {
    pub(crate) fn memory() -> Self {
        Self::Memory(MemoryEvidenceStore)
    }

    pub(crate) fn sqlite(database: RiotDatabase) -> Self {
        Self::Sqlite(SqliteEvidenceStore { database })
    }

    pub(crate) fn load(&self) -> Result<EvidenceSnapshot, SessionError> {
        match self {
            Self::Memory(memory) => memory.load(),
            Self::Sqlite(sqlite) => sqlite.load(),
        }
    }

    pub(crate) fn persist(&self, mutation: &EvidenceMutation) -> Result<(), SessionError> {
        match self {
            Self::Memory(memory) => memory.persist(mutation),
            Self::Sqlite(sqlite) => sqlite.persist(mutation),
        }
    }

    pub(crate) fn entries_with_prefix_in_namespace(
        &self,
        namespace_id: &[u8; 32],
        prefix: &crate::willow::Path,
    ) -> Result<Option<Vec<crate::import::join::PrefixedEntry>>, SessionError> {
        match self {
            Self::Memory(_) => Ok(None),
            Self::Sqlite(sqlite) => sqlite
                .entries_with_prefix_in_namespace(namespace_id, prefix)
                .map(Some),
        }
    }
}

pub(crate) struct SqliteEvidenceStore {
    database: RiotDatabase,
}

impl SqliteEvidenceStore {
    fn load(&self) -> Result<EvidenceSnapshot, SessionError> {
        self.database
            .read_connection(|connection| {
                let (generation, next_receipt_id, retained): (i64, i64, i64) = connection
                    .query_row(
                        "SELECT generation, next_receipt_id, retained_receipt_charge_bytes
                         FROM evidence_meta WHERE singleton = 1",
                        [],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    )
                    .map_err(map_sqlite_error)?;
                let mut statement = connection
                    .prepare(
                        "SELECT a.entry_id, a.namespace_id, a.subspace_id, a.path_bytes,
                                a.timestamp_be, a.payload_digest, a.payload_length,
                                a.entry_bytes, a.capability_bytes, a.signature_bytes,
                                l.payload, l.entry_id IS NOT NULL, f.entry_id IS NOT NULL,
                                a.first_receipt_id, a.dominated_on_arrival
                         FROM accepted_entries a
                         LEFT JOIN live_entries l
                           ON l.namespace_id = a.namespace_id AND l.entry_id = a.entry_id
                         LEFT JOIN forgotten_entries f
                           ON f.namespace_id = a.namespace_id AND f.entry_id = a.entry_id
                         ORDER BY a.namespace_id, a.entry_id",
                    )
                    .map_err(map_sqlite_error)?;
                let rows = statement
                    .query_map([], |row| {
                        Ok((
                            row.get::<_, Vec<u8>>(0)?,
                            row.get::<_, Vec<u8>>(1)?,
                            row.get::<_, Vec<u8>>(2)?,
                            row.get::<_, Vec<u8>>(3)?,
                            row.get::<_, Vec<u8>>(4)?,
                            row.get::<_, Vec<u8>>(5)?,
                            row.get::<_, i64>(6)?,
                            row.get::<_, Vec<u8>>(7)?,
                            row.get::<_, Vec<u8>>(8)?,
                            row.get::<_, Vec<u8>>(9)?,
                            row.get::<_, Option<Vec<u8>>>(10)?,
                            row.get::<_, bool>(11)?,
                            row.get::<_, bool>(12)?,
                            row.get::<_, i64>(13)?,
                            row.get::<_, bool>(14)?,
                        ))
                    })
                    .map_err(map_sqlite_error)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(map_sqlite_error)?;
                let mut persisted = Vec::with_capacity(rows.len());
                let mut first_receipt = Vec::with_capacity(rows.len());
                let mut namespaces = Vec::new();
                for (
                    id,
                    namespace,
                    subspace,
                    path,
                    timestamp,
                    digest,
                    payload_length,
                    entry_bytes,
                    capability,
                    signature,
                    payload,
                    live,
                    forgotten,
                    receipt,
                    dominated,
                ) in rows
                {
                    let id: EntryId = id
                        .try_into()
                        .map_err(|_| super::DatabaseError::CorruptDatabase)?;
                    let namespace: [u8; 32] = namespace
                        .try_into()
                        .map_err(|_| super::DatabaseError::CorruptDatabase)?;
                    validate_stored_entry(
                        &id,
                        &namespace,
                        &subspace,
                        &path,
                        &timestamp,
                        &digest,
                        payload_length,
                        &entry_bytes,
                        &capability,
                        &signature,
                        payload.as_deref(),
                    )?;
                    if live && forgotten {
                        return Err(super::DatabaseError::CorruptDatabase);
                    }
                    if !namespaces.contains(&namespace) {
                        namespaces.push(namespace);
                    }
                    first_receipt.push((
                        id,
                        u64::try_from(receipt)
                            .map_err(|_| super::DatabaseError::CorruptDatabase)?,
                        dominated,
                    ));
                    persisted.push(PersistedJoinEntry {
                        entry_bytes,
                        payload,
                        live,
                        forgotten,
                    });
                }
                let join = JoinState::from_persisted(persisted)
                    .map_err(|_| super::DatabaseError::CorruptDatabase)?;
                let receipts = load_receipts(connection)?;
                validate_relational_state(
                    connection,
                    u64::try_from(generation).map_err(|_| super::DatabaseError::CorruptDatabase)?,
                    u64::try_from(next_receipt_id)
                        .map_err(|_| super::DatabaseError::CorruptDatabase)?,
                    u64::try_from(retained).map_err(|_| super::DatabaseError::CorruptDatabase)?,
                    &receipts,
                )?;
                Ok(EvidenceSnapshot {
                    generation: u64::try_from(generation)
                        .map_err(|_| super::DatabaseError::CorruptDatabase)?,
                    join,
                    receipts,
                    first_receipt,
                    next_receipt_id: u64::try_from(next_receipt_id)
                        .map_err(|_| super::DatabaseError::CorruptDatabase)?,
                    retained_receipt_charge_bytes: u64::try_from(retained)
                        .map_err(|_| super::DatabaseError::CorruptDatabase)?,
                    seen_namespaces: namespaces,
                })
            })
            .map_err(session_error)
    }

    fn persist(&self, mutation: &EvidenceMutation) -> Result<(), SessionError> {
        let (deleted_bytes, deleted_rows) = self
            .database
            .read_connection(current_projection_cost)
            .map_err(session_error)?;
        let accepted_bytes = mutation
            .accepted
            .iter()
            .map(|entry| entry.entry_bytes.len() + entry.capability_bytes.len() + 64)
            .sum::<usize>();
        let live_bytes = mutation.live.iter().try_fold(0usize, |total, live| {
            let entry =
                decode_entry_canonic(&live.entry_bytes).map_err(|_| SessionError::Internal)?;
            let prefix_bytes = entry.path().components().fold(0usize, |sum, component| {
                sum.saturating_add(4).saturating_add(component.len())
            });
            let materialized_prefix_bytes =
                prefix_bytes.saturating_mul(entry.path().component_count().saturating_add(1));
            Ok::<_, SessionError>(
                total
                    .saturating_add(live.entry_bytes.len())
                    .saturating_add(live.payload.as_ref().map_or(0, Vec::len))
                    .saturating_add(materialized_prefix_bytes)
                    .saturating_add(512),
            )
        })?;
        let receipt_bytes = mutation.receipt.as_ref().map_or(0, |receipt| {
            receipt
                .route
                .len()
                .saturating_add(receipt.dispositions.iter().fold(0usize, |sum, row| {
                    let references = match &row.disposition {
                        EntryDisposition::AppliedAtCommit { pruned_entry_ids } => {
                            pruned_entry_ids.len()
                        }
                        EntryDisposition::DominatedAtCommit {
                            dominating_entry_ids,
                        } => dominating_entry_ids.len(),
                        EntryDisposition::AlreadyPresent { .. } => 0,
                    };
                    sum.saturating_add(128)
                        .saturating_add(references.saturating_mul(64))
                }))
        });
        let bytes = accepted_bytes
            .saturating_add(live_bytes)
            .saturating_add(receipt_bytes)
            .saturating_add(mutation.forgotten.len().saturating_mul(96))
            .saturating_add(deleted_bytes);
        let row_count = mutation
            .accepted
            .len()
            .saturating_add(mutation.live.len().saturating_mul(4))
            .saturating_add(mutation.forgotten.len())
            .saturating_add(
                mutation
                    .receipt
                    .as_ref()
                    .map_or(0, |r| r.dispositions.len()),
            )
            .saturating_add(deleted_rows);
        let page_headroom = u32::try_from(row_count.saturating_mul(2))
            .unwrap_or(u32::MAX)
            .saturating_add(EVIDENCE_WRITE_HEADROOM);
        self.database
            .write_transaction(WriteEstimate::new(bytes, page_headroom), |transaction| {
                persist_transaction(transaction, mutation)
            })
            .map_err(session_error)
    }

    fn entries_with_prefix_in_namespace(
        &self,
        namespace_id: &[u8; 32],
        prefix: &crate::willow::Path,
    ) -> Result<Vec<crate::import::join::PrefixedEntry>, SessionError> {
        let prefix_bytes = encode_path(prefix);
        let depth = i64::try_from(prefix.component_count()).map_err(|_| SessionError::Internal)?;
        self.database
            .read_connection(|connection| {
                let mut statement = connection
                    .prepare(
                        "SELECT l.entry_id, a.entry_bytes, l.payload
                         FROM entry_path_prefixes p
                         JOIN live_entries l
                           ON l.namespace_id = p.namespace_id AND l.entry_id = p.entry_id
                         JOIN accepted_entries a
                           ON a.namespace_id = l.namespace_id AND a.entry_id = l.entry_id
                         WHERE p.namespace_id = ?1 AND p.depth = ?2 AND p.prefix_bytes = ?3
                         ORDER BY l.entry_id",
                    )
                    .map_err(map_sqlite_error)?;
                let rows = statement
                    .query_map(params![namespace_id, depth, prefix_bytes], |row| {
                        Ok((
                            row.get::<_, Vec<u8>>(0)?,
                            row.get::<_, Vec<u8>>(1)?,
                            row.get::<_, Option<Vec<u8>>>(2)?,
                        ))
                    })
                    .map_err(map_sqlite_error)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(map_sqlite_error)?;
                rows.into_iter()
                    .map(|(id, entry_bytes, payload)| {
                        let id: EntryId = id
                            .try_into()
                            .map_err(|_| super::DatabaseError::CorruptDatabase)?;
                        let entry = decode_entry_canonic(&entry_bytes)
                            .map_err(|_| super::DatabaseError::CorruptDatabase)?;
                        if crate::willow::entry_id(&entry_bytes) != id
                            || entry.namespace_id().as_bytes() != namespace_id
                            || !prefix.is_prefix_of(entry.path())
                        {
                            return Err(super::DatabaseError::CorruptDatabase);
                        }
                        Ok((id, entry, payload))
                    })
                    .collect()
            })
            .map_err(session_error)
    }
}

fn current_projection_cost(
    connection: &rusqlite::Connection,
) -> Result<(usize, usize), super::DatabaseError> {
    let (live_rows, live_bytes): (i64, i64) = connection
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(
                length(namespace_id) + length(entry_id) + length(subspace_id) +
                length(path_bytes) + length(timestamp_be) + length(payload_digest) +
                8 + COALESCE(length(payload), 0)
             ), 0) FROM live_entries",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(map_sqlite_error)?;
    let (prefix_rows, prefix_bytes): (i64, i64) = connection
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(
                length(namespace_id) + length(entry_id) + 8 + length(prefix_bytes)
             ), 0) FROM entry_path_prefixes",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(map_sqlite_error)?;
    let (forgotten_rows, forgotten_bytes): (i64, i64) = connection
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(length(namespace_id) + length(entry_id) + 8), 0)
             FROM forgotten_entries",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(map_sqlite_error)?;
    let rows = live_rows
        .checked_add(prefix_rows)
        .and_then(|value| value.checked_add(forgotten_rows))
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(super::DatabaseError::CorruptDatabase)?;
    let bytes = live_bytes
        .checked_add(prefix_bytes)
        .and_then(|value| value.checked_add(forgotten_bytes))
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(super::DatabaseError::CorruptDatabase)?
        .saturating_add(rows.saturating_mul(256));
    Ok((bytes, rows))
}

fn persist_transaction(
    transaction: &Transaction<'_>,
    mutation: &EvidenceMutation,
) -> Result<(), super::DatabaseError> {
    let changed = transaction
        .execute(
            "UPDATE evidence_meta
             SET generation = ?1, next_receipt_id = ?2, retained_receipt_charge_bytes = ?3
             WHERE singleton = 1 AND generation = ?4",
            params![
                i64::try_from(mutation.generation)
                    .map_err(|_| super::DatabaseError::StorageFull)?,
                i64::try_from(mutation.next_receipt_id)
                    .map_err(|_| super::DatabaseError::StorageFull)?,
                i64::try_from(mutation.retained_receipt_charge_bytes)
                    .map_err(|_| super::DatabaseError::StorageFull)?,
                i64::try_from(mutation.expected_generation)
                    .map_err(|_| super::DatabaseError::StorageFull)?,
            ],
        )
        .map_err(map_sqlite_error)?;
    if changed != 1 {
        return Err(super::DatabaseError::BusyRetryable);
    }

    for accepted in &mutation.accepted {
        let entry = decode_entry_canonic(&accepted.entry_bytes)
            .map_err(|_| super::DatabaseError::CorruptDatabase)?;
        let namespace = *entry.namespace_id().as_bytes();
        let existing = transaction
            .query_row(
                "SELECT entry_bytes, capability_bytes, signature_bytes
                 FROM accepted_entries WHERE namespace_id = ?1 AND entry_id = ?2",
                params![namespace, accepted.entry_id],
                |row| {
                    Ok((
                        row.get::<_, Vec<u8>>(0)?,
                        row.get::<_, Vec<u8>>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(map_sqlite_error)?;
        if let Some((entry_bytes, capability_bytes, signature_bytes)) = existing {
            if entry_bytes != accepted.entry_bytes
                || capability_bytes != accepted.capability_bytes
                || signature_bytes.as_slice() != accepted.signature_bytes
            {
                return Err(super::DatabaseError::CorruptDatabase);
            }
            continue;
        }
        transaction
            .execute(
                "INSERT INTO accepted_entries(
                    namespace_id, entry_id, subspace_id, path_bytes, timestamp_be,
                    payload_digest, payload_length, entry_bytes, capability_bytes,
                    signature_bytes, first_receipt_id, dominated_on_arrival
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    namespace,
                    accepted.entry_id,
                    entry.subspace_id().as_bytes(),
                    encode_path(entry.path()),
                    u64::from(entry.timestamp()).to_be_bytes(),
                    entry.payload_digest().as_bytes(),
                    i64::try_from(entry.payload_length())
                        .map_err(|_| super::DatabaseError::StorageFull)?,
                    accepted.entry_bytes,
                    accepted.capability_bytes,
                    accepted.signature_bytes,
                    i64::try_from(accepted.first_receipt_id)
                        .map_err(|_| super::DatabaseError::StorageFull)?,
                    accepted.dominated_on_arrival,
                ],
            )
            .map_err(map_sqlite_error)?;
    }

    transaction
        .execute("DELETE FROM entry_path_prefixes", [])
        .map_err(map_sqlite_error)?;
    transaction
        .execute("DELETE FROM live_entries", [])
        .map_err(map_sqlite_error)?;
    for live in &mutation.live {
        let entry = decode_entry_canonic(&live.entry_bytes)
            .map_err(|_| super::DatabaseError::CorruptDatabase)?;
        let namespace = entry.namespace_id().as_bytes();
        let accepted_bytes: Vec<u8> = transaction
            .query_row(
                "SELECT entry_bytes FROM accepted_entries
                 WHERE namespace_id = ?1 AND entry_id = ?2",
                params![namespace, live.entry_id],
                |row| row.get(0),
            )
            .map_err(map_sqlite_error)?;
        if accepted_bytes != live.entry_bytes {
            return Err(super::DatabaseError::CorruptDatabase);
        }
        transaction
            .execute(
                "INSERT INTO live_entries(
                    namespace_id, entry_id, subspace_id, path_bytes, timestamp_be,
                    payload_digest, payload_length, payload
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    namespace,
                    live.entry_id,
                    entry.subspace_id().as_bytes(),
                    encode_path(entry.path()),
                    u64::from(entry.timestamp()).to_be_bytes(),
                    entry.payload_digest().as_bytes(),
                    i64::try_from(entry.payload_length())
                        .map_err(|_| super::DatabaseError::StorageFull)?,
                    live.payload,
                ],
            )
            .map_err(map_sqlite_error)?;
        insert_prefixes(transaction, &entry, &live.entry_id)?;
    }

    sync_forget_ledger(transaction, mutation)?;

    if let Some(receipt) = &mutation.receipt {
        insert_receipt(transaction, receipt, &mutation.disposition_namespaces)?;
    }
    transaction
        .execute(
            "UPDATE database_meta SET generation = generation + 1 WHERE singleton = 1",
            [],
        )
        .map_err(map_sqlite_error)?;
    Ok(())
}

fn sync_forget_ledger(
    transaction: &Transaction<'_>,
    mutation: &EvidenceMutation,
) -> Result<(), super::DatabaseError> {
    let mut statement = transaction
        .prepare(
            "SELECT namespace_id, entry_id, forgotten_generation
             FROM forgotten_entries ORDER BY namespace_id, entry_id",
        )
        .map_err(map_sqlite_error)?;
    let existing = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .map_err(map_sqlite_error)?
        .map(|row| {
            let (namespace, id, forgotten_generation) = row.map_err(map_sqlite_error)?;
            Ok((
                namespace
                    .try_into()
                    .map_err(|_| super::DatabaseError::CorruptDatabase)?,
                id.try_into()
                    .map_err(|_| super::DatabaseError::CorruptDatabase)?,
                u64::try_from(forgotten_generation)
                    .map_err(|_| super::DatabaseError::CorruptDatabase)?,
            ))
        })
        .collect::<Result<Vec<([u8; 32], EntryId, u64)>, super::DatabaseError>>()?;
    drop(statement);

    let desired = mutation.forgotten.iter().copied().collect::<BTreeSet<_>>();
    if desired.len() != mutation.forgotten.len() {
        return Err(super::DatabaseError::CorruptDatabase);
    }
    let existing_ids = existing
        .iter()
        .map(|(_, entry_id, _)| *entry_id)
        .collect::<BTreeSet<_>>();
    let generation =
        i64::try_from(mutation.generation).map_err(|_| super::DatabaseError::StorageFull)?;

    if mutation.receipt.is_none() {
        let newly_forgotten = desired
            .difference(&existing_ids)
            .copied()
            .collect::<Vec<_>>();
        if !existing_ids.is_subset(&desired) || newly_forgotten.len() != 1 {
            return Err(super::DatabaseError::CorruptDatabase);
        }
        let entry_id = newly_forgotten[0];
        let namespaces = transaction
            .prepare(
                "SELECT namespace_id FROM accepted_entries WHERE entry_id = ?1
                 ORDER BY namespace_id",
            )
            .and_then(|mut statement| {
                statement
                    .query_map([entry_id], |row| row.get::<_, Vec<u8>>(0))?
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(map_sqlite_error)?;
        if namespaces.len() != 1 {
            return Err(super::DatabaseError::CorruptDatabase);
        }
        let namespace = &namespaces[0];
        transaction
            .execute(
                "INSERT INTO forget_events(
                    namespace_id, entry_id, forgotten_generation, restored_generation
                 ) VALUES (?1, ?2, ?3, NULL)",
                params![namespace, entry_id, generation],
            )
            .map_err(map_sqlite_error)?;
        transaction
            .execute(
                "INSERT INTO forgotten_entries(namespace_id, entry_id, forgotten_generation)
                 VALUES (?1, ?2, ?3)",
                params![namespace, entry_id, generation],
            )
            .map_err(map_sqlite_error)?;
        return Ok(());
    }

    if !desired.is_subset(&existing_ids) {
        return Err(super::DatabaseError::CorruptDatabase);
    }
    let final_live = mutation
        .live
        .iter()
        .map(|live| live.entry_id)
        .collect::<BTreeSet<_>>();
    for (namespace, entry_id, forgotten_generation) in existing {
        let restored = final_live.contains(&entry_id);
        if restored == desired.contains(&entry_id) {
            return Err(super::DatabaseError::CorruptDatabase);
        }
        if restored {
            let changed = transaction
                .execute(
                    "UPDATE forget_events SET restored_generation = ?1
                     WHERE namespace_id = ?2 AND entry_id = ?3
                       AND forgotten_generation = ?4 AND restored_generation IS NULL",
                    params![
                        generation,
                        namespace,
                        entry_id,
                        i64::try_from(forgotten_generation)
                            .map_err(|_| super::DatabaseError::CorruptDatabase)?
                    ],
                )
                .map_err(map_sqlite_error)?;
            let deleted = transaction
                .execute(
                    "DELETE FROM forgotten_entries
                     WHERE namespace_id = ?1 AND entry_id = ?2
                       AND forgotten_generation = ?3",
                    params![
                        namespace,
                        entry_id,
                        i64::try_from(forgotten_generation)
                            .map_err(|_| super::DatabaseError::CorruptDatabase)?
                    ],
                )
                .map_err(map_sqlite_error)?;
            if changed != 1 || deleted != 1 {
                return Err(super::DatabaseError::CorruptDatabase);
            }
        }
    }
    Ok(())
}

fn insert_receipt(
    transaction: &Transaction<'_>,
    receipt: &ImportReceipt,
    namespaces: &[[u8; 32]],
) -> Result<(), super::DatabaseError> {
    let receipt_id =
        i64::try_from(receipt.receipt_id).map_err(|_| super::DatabaseError::StorageFull)?;
    transaction
        .execute(
            "INSERT INTO import_receipts(receipt_id, route, before_generation, after_generation)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                receipt_id,
                receipt.route,
                i64::try_from(receipt.before_generation)
                    .map_err(|_| super::DatabaseError::StorageFull)?,
                i64::try_from(receipt.after_generation)
                    .map_err(|_| super::DatabaseError::StorageFull)?
            ],
        )
        .map_err(map_sqlite_error)?;
    for (position, row) in receipt.dispositions.iter().enumerate() {
        let namespace = namespaces
            .get(position)
            .ok_or(super::DatabaseError::Internal)?;
        let (kind, insertion, references): (i64, Option<i64>, &[EntryId]) = match &row.disposition {
            EntryDisposition::AppliedAtCommit { pruned_entry_ids } => (0, None, pruned_entry_ids),
            EntryDisposition::DominatedAtCommit {
                dominating_entry_ids,
            } => (1, None, dominating_entry_ids),
            EntryDisposition::AlreadyPresent {
                insertion_receipt_id,
            } => (
                2,
                Some(
                    i64::try_from(*insertion_receipt_id)
                        .map_err(|_| super::DatabaseError::StorageFull)?,
                ),
                &[],
            ),
        };
        transaction
            .execute(
                "INSERT INTO import_dispositions(
                    namespace_id, receipt_id, position, entry_id, kind, insertion_receipt_id
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    namespace,
                    receipt_id,
                    position as i64,
                    row.entry_id,
                    kind,
                    insertion
                ],
            )
            .map_err(map_sqlite_error)?;
        for (reference_position, entry_id) in references.iter().enumerate() {
            transaction
                .execute(
                    "INSERT INTO import_references(
                        namespace_id, receipt_id, disposition_position,
                        reference_position, entry_id
                     ) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        namespace,
                        receipt_id,
                        position as i64,
                        reference_position as i64,
                        entry_id
                    ],
                )
                .map_err(map_sqlite_error)?;
        }
    }
    Ok(())
}

fn load_receipts(
    connection: &rusqlite::Connection,
) -> Result<Vec<ImportReceipt>, super::DatabaseError> {
    let mut statement = connection
        .prepare(
            "SELECT receipt_id, route, before_generation, after_generation
             FROM import_receipts ORDER BY receipt_id",
        )
        .map_err(map_sqlite_error)?;
    let headers = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .map_err(map_sqlite_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_sqlite_error)?;
    let mut receipts = Vec::with_capacity(headers.len());
    for (receipt_id, route, before, after) in headers {
        let mut dispositions_statement = connection
            .prepare(
                "SELECT position, entry_id, kind, insertion_receipt_id
                 FROM import_dispositions WHERE receipt_id = ?1 ORDER BY position",
            )
            .map_err(map_sqlite_error)?;
        let dispositions_rows = dispositions_statement
            .query_map([receipt_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                ))
            })
            .map_err(map_sqlite_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_sqlite_error)?;
        let mut dispositions = Vec::with_capacity(dispositions_rows.len());
        for (position, id, kind, insertion) in dispositions_rows {
            let id: EntryId = id
                .try_into()
                .map_err(|_| super::DatabaseError::CorruptDatabase)?;
            let references = load_references(connection, receipt_id, position)?;
            let disposition = match kind {
                0 => EntryDisposition::AppliedAtCommit {
                    pruned_entry_ids: references,
                },
                1 => EntryDisposition::DominatedAtCommit {
                    dominating_entry_ids: references,
                },
                2 => EntryDisposition::AlreadyPresent {
                    insertion_receipt_id: u64::try_from(
                        insertion.ok_or(super::DatabaseError::CorruptDatabase)?,
                    )
                    .map_err(|_| super::DatabaseError::CorruptDatabase)?,
                },
                _ => return Err(super::DatabaseError::CorruptDatabase),
            };
            dispositions.push(DispositionRow {
                entry_id: id,
                disposition,
            });
        }
        receipts.push(ImportReceipt {
            receipt_id: u64::try_from(receipt_id)
                .map_err(|_| super::DatabaseError::CorruptDatabase)?,
            route,
            before_generation: u64::try_from(before)
                .map_err(|_| super::DatabaseError::CorruptDatabase)?,
            after_generation: u64::try_from(after)
                .map_err(|_| super::DatabaseError::CorruptDatabase)?,
            dispositions,
        });
    }
    Ok(receipts)
}

fn load_references(
    connection: &rusqlite::Connection,
    receipt_id: i64,
    position: i64,
) -> Result<Vec<EntryId>, super::DatabaseError> {
    let mut statement = connection
        .prepare(
            "SELECT entry_id FROM import_references
             WHERE receipt_id = ?1 AND disposition_position = ?2
             ORDER BY reference_position",
        )
        .map_err(map_sqlite_error)?;
    let references = statement
        .query_map(params![receipt_id, position], |row| {
            row.get::<_, Vec<u8>>(0)
        })
        .map_err(map_sqlite_error)?
        .map(|row| {
            row.map_err(map_sqlite_error)?
                .try_into()
                .map_err(|_| super::DatabaseError::CorruptDatabase)
        })
        .collect();
    references
}

fn insert_prefixes(
    transaction: &Transaction<'_>,
    entry: &crate::willow::Entry,
    entry_id: &EntryId,
) -> Result<(), super::DatabaseError> {
    let namespace = entry.namespace_id().as_bytes();
    let mut prefix = Vec::new();
    transaction
        .execute(
            "INSERT INTO entry_path_prefixes(namespace_id, entry_id, depth, prefix_bytes)
             VALUES (?1, ?2, 0, ?3)",
            params![namespace, entry_id, prefix],
        )
        .map_err(map_sqlite_error)?;
    for (index, component) in entry.path().components().enumerate() {
        let bytes = component.as_ref();
        prefix.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        prefix.extend_from_slice(bytes);
        transaction
            .execute(
                "INSERT INTO entry_path_prefixes(namespace_id, entry_id, depth, prefix_bytes)
                 VALUES (?1, ?2, ?3, ?4)",
                params![namespace, entry_id, (index + 1) as i64, prefix],
            )
            .map_err(map_sqlite_error)?;
    }
    Ok(())
}

fn encode_path(path: &crate::willow::Path) -> Vec<u8> {
    let mut encoded = Vec::new();
    for component in path.components() {
        let bytes = component.as_ref();
        encoded.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        encoded.extend_from_slice(bytes);
    }
    encoded
}

#[derive(Clone)]
struct AcceptedInfo {
    entry: crate::willow::Entry,
    first_receipt_id: u64,
    dominated_on_arrival: bool,
}

struct ReceiptReplayRow {
    key: AcceptedKey,
    kind: i64,
    reference_ids: BTreeSet<EntryId>,
}

#[derive(Clone, Copy)]
struct ForgetEvent {
    key: AcceptedKey,
    forgotten_generation: u64,
    restored_generation: Option<u64>,
}

fn validate_relational_state(
    connection: &rusqlite::Connection,
    generation: u64,
    next_receipt_id: u64,
    retained_receipt_charge_bytes: u64,
    receipts: &[ImportReceipt],
) -> Result<(), super::DatabaseError> {
    if receipts.len() > MAX_IMPORT_RECEIPTS {
        return Err(super::DatabaseError::CorruptDatabase);
    }
    let accepted = load_accepted_info(connection)?;
    if accepted.len() > MAX_ACCEPTED_ENTRIES {
        return Err(super::DatabaseError::CorruptDatabase);
    }
    let namespaces: BTreeSet<[u8; 32]> = accepted.keys().map(|(namespace, _)| *namespace).collect();
    if namespaces.len() > MAX_NAMESPACE_VIEWS {
        return Err(super::DatabaseError::CorruptDatabase);
    }

    let forget_events = raw_forget_events(connection, generation, &accepted)?;
    let events_by_generation = forget_events
        .iter()
        .map(|event| (event.forgotten_generation, *event))
        .collect::<BTreeMap<_, _>>();
    if events_by_generation.len() != forget_events.len() {
        return Err(super::DatabaseError::CorruptDatabase);
    }
    let mut expected_live = BTreeSet::new();
    let mut active_forgets = BTreeMap::new();
    let mut applied_forget_generations = BTreeSet::new();
    let mut applied_restore_events = 0usize;
    let mut first_seen = BTreeSet::new();
    let mut previous_after = 0u64;
    let mut computed_charge = 0u64;
    for (receipt_index, receipt) in receipts.iter().enumerate() {
        let expected_receipt_id =
            u64::try_from(receipt_index + 1).map_err(|_| super::DatabaseError::CorruptDatabase)?;
        if receipt.receipt_id != expected_receipt_id
            || receipt.after_generation != receipt.before_generation.saturating_add(1)
            || (receipt_index == 0 && receipt.before_generation != 0)
            || (receipt_index > 0 && receipt.before_generation < previous_after)
            || receipt.after_generation > generation
        {
            return Err(super::DatabaseError::CorruptDatabase);
        }
        apply_forget_events(
            previous_after,
            receipt.before_generation,
            &events_by_generation,
            &mut expected_live,
            &mut active_forgets,
            &mut applied_forget_generations,
        )?;
        if events_by_generation.contains_key(&receipt.after_generation) {
            return Err(super::DatabaseError::CorruptDatabase);
        }
        let rows = raw_dispositions(connection, receipt.receipt_id)?;
        if rows.len() != receipt.dispositions.len() {
            return Err(super::DatabaseError::CorruptDatabase);
        }
        computed_charge = computed_charge
            .checked_add(receipt.route.len() as u64)
            .and_then(|charge| {
                charge.checked_add(receipt.dispositions.len() as u64 * RECEIPT_ROW_CHARGE)
            })
            .ok_or(super::DatabaseError::CorruptDatabase)?;
        let mut replay_rows = Vec::with_capacity(rows.len());
        let mut receipt_keys = BTreeSet::new();
        let mut admission_keys = BTreeSet::new();
        for (position, (namespace, raw_entry_id, kind, insertion_receipt_id)) in
            rows.into_iter().enumerate()
        {
            let row = receipt
                .dispositions
                .get(position)
                .ok_or(super::DatabaseError::CorruptDatabase)?;
            if row.entry_id != raw_entry_id {
                return Err(super::DatabaseError::CorruptDatabase);
            }
            let key = (namespace, raw_entry_id);
            let accepted_info = accepted
                .get(&key)
                .ok_or(super::DatabaseError::CorruptDatabase)?;
            let references = raw_references(
                connection,
                receipt.receipt_id,
                i64::try_from(position).map_err(|_| super::DatabaseError::CorruptDatabase)?,
            )?;
            computed_charge = computed_charge
                .checked_add(references.len() as u64 * REFERENCE_CHARGE)
                .ok_or(super::DatabaseError::CorruptDatabase)?;
            for (reference_namespace, reference_id) in &references {
                if reference_namespace != &namespace
                    || !accepted.contains_key(&(*reference_namespace, *reference_id))
                {
                    return Err(super::DatabaseError::CorruptDatabase);
                }
            }
            let reference_ids: Vec<EntryId> = references.iter().map(|(_, id)| *id).collect();
            let reference_set: BTreeSet<EntryId> = reference_ids.iter().copied().collect();
            if reference_set.len() != reference_ids.len() || !receipt_keys.insert(key) {
                return Err(super::DatabaseError::CorruptDatabase);
            }
            match (&row.disposition, kind) {
                (EntryDisposition::AppliedAtCommit { pruned_entry_ids }, 0)
                    if insertion_receipt_id.is_none() && *pruned_entry_ids == reference_ids =>
                {
                    let newly_seen = first_seen.insert(key);
                    let invalid_first_receipt = if newly_seen {
                        accepted_info.dominated_on_arrival
                            || accepted_info.first_receipt_id != receipt.receipt_id
                    } else {
                        accepted_info.first_receipt_id >= receipt.receipt_id
                    };
                    if invalid_first_receipt {
                        return Err(super::DatabaseError::CorruptDatabase);
                    }
                }
                (
                    EntryDisposition::DominatedAtCommit {
                        dominating_entry_ids,
                    },
                    1,
                ) if insertion_receipt_id.is_none() && *dominating_entry_ids == reference_ids => {
                    if !first_seen.insert(key)
                        || accepted_info.first_receipt_id != receipt.receipt_id
                        || !accepted_info.dominated_on_arrival
                    {
                        return Err(super::DatabaseError::CorruptDatabase);
                    }
                }
                (
                    EntryDisposition::AlreadyPresent {
                        insertion_receipt_id: stored_insertion,
                    },
                    2,
                ) if references.is_empty()
                    && insertion_receipt_id == Some(*stored_insertion)
                    && *stored_insertion == accepted_info.first_receipt_id
                    && *stored_insertion < receipt.receipt_id
                    && first_seen.contains(&key) => {}
                _ => return Err(super::DatabaseError::CorruptDatabase),
            }
            if kind != 2 {
                admission_keys.insert(key);
            }
            replay_rows.push(ReceiptReplayRow {
                key,
                kind,
                reference_ids: reference_set,
            });
        }

        let final_live = replay_final_live(&expected_live, &admission_keys, &accepted)?;
        for replay in &replay_rows {
            let replay_entry = &accepted
                .get(&replay.key)
                .ok_or(super::DatabaseError::CorruptDatabase)?
                .entry;
            let expected_references = if replay.kind == 0 {
                if !final_live.contains(&replay.key) {
                    return Err(super::DatabaseError::CorruptDatabase);
                }
                expected_live
                    .iter()
                    .filter(|key| {
                        accepted
                            .get(key)
                            .is_some_and(|candidate| replay_entry.prunes(&candidate.entry))
                    })
                    .map(|key| key.1)
                    .collect::<BTreeSet<_>>()
            } else if replay.kind == 1 {
                if final_live.contains(&replay.key) {
                    return Err(super::DatabaseError::CorruptDatabase);
                }
                final_live
                    .iter()
                    .filter(|key| {
                        accepted
                            .get(key)
                            .is_some_and(|candidate| candidate.entry.prunes(replay_entry))
                    })
                    .map(|key| key.1)
                    .collect::<BTreeSet<_>>()
            } else {
                continue;
            };
            if replay.reference_ids != expected_references {
                return Err(super::DatabaseError::CorruptDatabase);
            }
        }
        let restored_here = forget_events
            .iter()
            .filter(|event| event.restored_generation == Some(receipt.after_generation))
            .copied()
            .collect::<Vec<_>>();
        let receipt_restorations = replay_rows
            .iter()
            .filter(|row| {
                row.kind == 0
                    && accepted
                        .get(&row.key)
                        .is_some_and(|info| info.first_receipt_id < receipt.receipt_id)
            })
            .map(|row| row.key)
            .collect::<BTreeSet<_>>();
        if receipt_restorations
            != restored_here
                .iter()
                .map(|event| event.key)
                .collect::<BTreeSet<_>>()
        {
            return Err(super::DatabaseError::CorruptDatabase);
        }
        for event in restored_here {
            if active_forgets.remove(&event.key) != Some(event.forgotten_generation)
                || !final_live.contains(&event.key)
            {
                return Err(super::DatabaseError::CorruptDatabase);
            }
            applied_restore_events += 1;
        }
        expected_live = final_live;
        previous_after = receipt.after_generation;
    }
    if first_seen.len() != accepted.len()
        || next_receipt_id != receipts.len() as u64 + 1
        || computed_charge != retained_receipt_charge_bytes
    {
        return Err(super::DatabaseError::CorruptDatabase);
    }

    apply_forget_events(
        previous_after,
        generation,
        &events_by_generation,
        &mut expected_live,
        &mut active_forgets,
        &mut applied_forget_generations,
    )?;
    if applied_forget_generations.len() != forget_events.len()
        || applied_restore_events
            != forget_events
                .iter()
                .filter(|event| event.restored_generation.is_some())
                .count()
    {
        return Err(super::DatabaseError::CorruptDatabase);
    }
    let forgotten = raw_forgotten(connection, generation, &accepted)?;
    if forgotten != active_forgets {
        return Err(super::DatabaseError::CorruptDatabase);
    }
    validate_live_and_prefix_projection(connection, &accepted, &expected_live)?;
    Ok(())
}

type AcceptedKey = ([u8; 32], EntryId);
type RawDisposition = ([u8; 32], EntryId, i64, Option<u64>);

fn replay_final_live(
    pre_live: &BTreeSet<AcceptedKey>,
    batch: &BTreeSet<AcceptedKey>,
    accepted: &BTreeMap<AcceptedKey, AcceptedInfo>,
) -> Result<BTreeSet<AcceptedKey>, super::DatabaseError> {
    let union = pre_live.union(batch).copied().collect::<BTreeSet<_>>();
    union
        .iter()
        .filter_map(|candidate_key| {
            let candidate = match accepted.get(candidate_key) {
                Some(info) => &info.entry,
                None => return Some(Err(super::DatabaseError::CorruptDatabase)),
            };
            let dominated = union.iter().any(|other_key| {
                other_key != candidate_key
                    && accepted
                        .get(other_key)
                        .is_some_and(|other| other.entry.prunes(candidate))
            });
            (!dominated).then_some(Ok(*candidate_key))
        })
        .collect()
}

fn apply_forget_events(
    after_generation: u64,
    through_generation: u64,
    events: &BTreeMap<u64, ForgetEvent>,
    live: &mut BTreeSet<AcceptedKey>,
    active: &mut BTreeMap<AcceptedKey, u64>,
    applied_generations: &mut BTreeSet<u64>,
) -> Result<(), super::DatabaseError> {
    for generation in after_generation.saturating_add(1)..=through_generation {
        let event = events
            .get(&generation)
            .ok_or(super::DatabaseError::CorruptDatabase)?;
        if event.forgotten_generation != generation
            || !live.remove(&event.key)
            || active.insert(event.key, generation).is_some()
            || !applied_generations.insert(generation)
        {
            return Err(super::DatabaseError::CorruptDatabase);
        }
    }
    Ok(())
}

fn load_accepted_info(
    connection: &rusqlite::Connection,
) -> Result<BTreeMap<AcceptedKey, AcceptedInfo>, super::DatabaseError> {
    let mut statement = connection
        .prepare(
            "SELECT namespace_id, entry_id, entry_bytes, first_receipt_id,
                    dominated_on_arrival FROM accepted_entries
             ORDER BY namespace_id, entry_id",
        )
        .map_err(map_sqlite_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, Vec<u8>>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, bool>(4)?,
            ))
        })
        .map_err(map_sqlite_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_sqlite_error)?;
    let mut accepted = BTreeMap::new();
    for (namespace, id, entry_bytes, first_receipt_id, dominated_on_arrival) in rows {
        let namespace: [u8; 32] = namespace
            .try_into()
            .map_err(|_| super::DatabaseError::CorruptDatabase)?;
        let id: EntryId = id
            .try_into()
            .map_err(|_| super::DatabaseError::CorruptDatabase)?;
        let entry = decode_entry_canonic(&entry_bytes)
            .map_err(|_| super::DatabaseError::CorruptDatabase)?;
        if entry.namespace_id().as_bytes() != &namespace
            || crate::willow::entry_id(&entry_bytes) != id
            || accepted
                .insert(
                    (namespace, id),
                    AcceptedInfo {
                        entry,
                        first_receipt_id: u64::try_from(first_receipt_id)
                            .map_err(|_| super::DatabaseError::CorruptDatabase)?,
                        dominated_on_arrival,
                    },
                )
                .is_some()
        {
            return Err(super::DatabaseError::CorruptDatabase);
        }
    }
    Ok(accepted)
}

fn raw_dispositions(
    connection: &rusqlite::Connection,
    receipt_id: u64,
) -> Result<Vec<RawDisposition>, super::DatabaseError> {
    let receipt_id =
        i64::try_from(receipt_id).map_err(|_| super::DatabaseError::CorruptDatabase)?;
    let mut statement = connection
        .prepare(
            "SELECT namespace_id, position, entry_id, kind, insertion_receipt_id
             FROM import_dispositions WHERE receipt_id = ?1
             ORDER BY position, namespace_id",
        )
        .map_err(map_sqlite_error)?;
    let rows = statement
        .query_map([receipt_id], |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Vec<u8>>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Option<i64>>(4)?,
            ))
        })
        .map_err(map_sqlite_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_sqlite_error)?;
    rows.into_iter()
        .enumerate()
        .map(
            |(expected_position, (namespace, position, id, kind, insertion))| {
                if position != expected_position as i64 {
                    return Err(super::DatabaseError::CorruptDatabase);
                }
                Ok((
                    namespace
                        .try_into()
                        .map_err(|_| super::DatabaseError::CorruptDatabase)?,
                    id.try_into()
                        .map_err(|_| super::DatabaseError::CorruptDatabase)?,
                    kind,
                    insertion
                        .map(u64::try_from)
                        .transpose()
                        .map_err(|_| super::DatabaseError::CorruptDatabase)?,
                ))
            },
        )
        .collect()
}

fn raw_references(
    connection: &rusqlite::Connection,
    receipt_id: u64,
    disposition_position: i64,
) -> Result<Vec<([u8; 32], EntryId)>, super::DatabaseError> {
    let receipt_id =
        i64::try_from(receipt_id).map_err(|_| super::DatabaseError::CorruptDatabase)?;
    let mut statement = connection
        .prepare(
            "SELECT namespace_id, reference_position, entry_id
             FROM import_references
             WHERE receipt_id = ?1 AND disposition_position = ?2
             ORDER BY reference_position, namespace_id",
        )
        .map_err(map_sqlite_error)?;
    let rows = statement
        .query_map(params![receipt_id, disposition_position], |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Vec<u8>>(2)?,
            ))
        })
        .map_err(map_sqlite_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_sqlite_error)?;
    rows.into_iter()
        .enumerate()
        .map(|(expected_position, (namespace, position, id))| {
            if position != expected_position as i64 {
                return Err(super::DatabaseError::CorruptDatabase);
            }
            Ok((
                namespace
                    .try_into()
                    .map_err(|_| super::DatabaseError::CorruptDatabase)?,
                id.try_into()
                    .map_err(|_| super::DatabaseError::CorruptDatabase)?,
            ))
        })
        .collect()
}

fn raw_forget_events(
    connection: &rusqlite::Connection,
    generation: u64,
    accepted: &BTreeMap<AcceptedKey, AcceptedInfo>,
) -> Result<Vec<ForgetEvent>, super::DatabaseError> {
    let mut statement = connection
        .prepare(
            "SELECT namespace_id, entry_id, forgotten_generation, restored_generation
             FROM forget_events ORDER BY forgotten_generation",
        )
        .map_err(map_sqlite_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<i64>>(3)?,
            ))
        })
        .map_err(map_sqlite_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_sqlite_error)?;
    rows.into_iter()
        .map(
            |(namespace, id, forgotten_generation, restored_generation)| {
                let key = (
                    namespace
                        .try_into()
                        .map_err(|_| super::DatabaseError::CorruptDatabase)?,
                    id.try_into()
                        .map_err(|_| super::DatabaseError::CorruptDatabase)?,
                );
                let forgotten_generation = u64::try_from(forgotten_generation)
                    .map_err(|_| super::DatabaseError::CorruptDatabase)?;
                let restored_generation = restored_generation
                    .map(u64::try_from)
                    .transpose()
                    .map_err(|_| super::DatabaseError::CorruptDatabase)?;
                if forgotten_generation == 0
                    || forgotten_generation > generation
                    || restored_generation.is_some_and(|restored| {
                        restored <= forgotten_generation || restored > generation
                    })
                    || !accepted.contains_key(&key)
                {
                    return Err(super::DatabaseError::CorruptDatabase);
                }
                Ok(ForgetEvent {
                    key,
                    forgotten_generation,
                    restored_generation,
                })
            },
        )
        .collect()
}

fn raw_forgotten(
    connection: &rusqlite::Connection,
    generation: u64,
    accepted: &BTreeMap<AcceptedKey, AcceptedInfo>,
) -> Result<BTreeMap<AcceptedKey, u64>, super::DatabaseError> {
    let mut statement = connection
        .prepare(
            "SELECT namespace_id, entry_id, forgotten_generation
             FROM forgotten_entries ORDER BY namespace_id, entry_id",
        )
        .map_err(map_sqlite_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .map_err(map_sqlite_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_sqlite_error)?;
    let mut forgotten = BTreeMap::new();
    for (namespace, id, forgotten_generation) in rows {
        let key = (
            namespace
                .try_into()
                .map_err(|_| super::DatabaseError::CorruptDatabase)?,
            id.try_into()
                .map_err(|_| super::DatabaseError::CorruptDatabase)?,
        );
        let forgotten_generation = u64::try_from(forgotten_generation)
            .map_err(|_| super::DatabaseError::CorruptDatabase)?;
        if forgotten_generation == 0
            || forgotten_generation > generation
            || !accepted.contains_key(&key)
            || forgotten.insert(key, forgotten_generation).is_some()
        {
            return Err(super::DatabaseError::CorruptDatabase);
        }
    }
    Ok(forgotten)
}

fn validate_live_and_prefix_projection(
    connection: &rusqlite::Connection,
    accepted: &BTreeMap<AcceptedKey, AcceptedInfo>,
    expected_live: &BTreeSet<AcceptedKey>,
) -> Result<(), super::DatabaseError> {
    if expected_live.len() > MAX_LIVE_ENTRIES {
        return Err(super::DatabaseError::CorruptDatabase);
    }
    let mut statement = connection
        .prepare(
            "SELECT namespace_id, entry_id, subspace_id, path_bytes, timestamp_be,
                    payload_digest, payload_length, payload
             FROM live_entries ORDER BY namespace_id, entry_id",
        )
        .map_err(map_sqlite_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, Vec<u8>>(2)?,
                row.get::<_, Vec<u8>>(3)?,
                row.get::<_, Vec<u8>>(4)?,
                row.get::<_, Vec<u8>>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, Option<Vec<u8>>>(7)?,
            ))
        })
        .map_err(map_sqlite_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_sqlite_error)?;
    let mut actual_live = BTreeSet::new();
    let mut expected_prefixes = BTreeSet::new();
    for (namespace, id, subspace, path, timestamp, digest, payload_length, payload) in rows {
        let key = (
            namespace
                .try_into()
                .map_err(|_| super::DatabaseError::CorruptDatabase)?,
            id.try_into()
                .map_err(|_| super::DatabaseError::CorruptDatabase)?,
        );
        let info = accepted
            .get(&key)
            .ok_or(super::DatabaseError::CorruptDatabase)?;
        if !actual_live.insert(key)
            || info.entry.subspace_id().as_bytes() != subspace.as_slice()
            || encode_path(info.entry.path()) != path
            || u64::from(info.entry.timestamp()).to_be_bytes().as_slice() != timestamp.as_slice()
            || info.entry.payload_digest().as_bytes() != digest.as_slice()
            || i64::try_from(info.entry.payload_length()).ok() != Some(payload_length)
        {
            return Err(super::DatabaseError::CorruptDatabase);
        }
        let payload_required = crate::apps::entry::is_app_data_path(info.entry.path())
            || crate::apps::index::classify_app_index_path(info.entry.path()).is_some()
            || crate::profile::path::classify_profile_path(info.entry.path()).is_some();
        if payload_required && payload.is_none() {
            return Err(super::DatabaseError::CorruptDatabase);
        }
        if let Some(payload) = payload {
            if payload.len() as u64 != info.entry.payload_length()
                || crate::willow::william3_digest(&payload)
                    != *info.entry.payload_digest().as_bytes()
            {
                return Err(super::DatabaseError::CorruptDatabase);
            }
        }
        let mut prefix = Vec::new();
        expected_prefixes.insert((key.0, key.1, 0i64, prefix.clone()));
        for (index, component) in info.entry.path().components().enumerate() {
            let bytes = component.as_ref();
            prefix.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
            prefix.extend_from_slice(bytes);
            expected_prefixes.insert((key.0, key.1, (index + 1) as i64, prefix.clone()));
        }
    }
    if &actual_live != expected_live {
        return Err(super::DatabaseError::CorruptDatabase);
    }
    let mut prefix_statement = connection
        .prepare(
            "SELECT namespace_id, entry_id, depth, prefix_bytes
             FROM entry_path_prefixes ORDER BY namespace_id, entry_id, depth",
        )
        .map_err(map_sqlite_error)?;
    let prefix_rows = prefix_statement
        .query_map([], |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Vec<u8>>(3)?,
            ))
        })
        .map_err(map_sqlite_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_sqlite_error)?;
    let actual_prefixes = prefix_rows
        .into_iter()
        .map(|(namespace, id, depth, prefix)| {
            Ok((
                namespace
                    .try_into()
                    .map_err(|_| super::DatabaseError::CorruptDatabase)?,
                id.try_into()
                    .map_err(|_| super::DatabaseError::CorruptDatabase)?,
                depth,
                prefix,
            ))
        })
        .collect::<Result<BTreeSet<_>, super::DatabaseError>>()?;
    if actual_prefixes != expected_prefixes {
        return Err(super::DatabaseError::CorruptDatabase);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_stored_entry(
    stored_id: &EntryId,
    stored_namespace: &[u8; 32],
    stored_subspace: &[u8],
    stored_path: &[u8],
    stored_timestamp: &[u8],
    stored_digest: &[u8],
    stored_payload_length: i64,
    entry_bytes: &[u8],
    capability_bytes: &[u8],
    signature_bytes: &[u8],
    payload: Option<&[u8]>,
) -> Result<(), super::DatabaseError> {
    let entry =
        decode_entry_canonic(entry_bytes).map_err(|_| super::DatabaseError::CorruptDatabase)?;
    if crate::willow::entry_id(entry_bytes) != *stored_id
        || entry.namespace_id().as_bytes() != stored_namespace
        || entry.subspace_id().as_bytes() != stored_subspace
        || encode_path(entry.path()) != stored_path
        || u64::from(entry.timestamp()).to_be_bytes().as_slice() != stored_timestamp
        || entry.payload_digest().as_bytes() != stored_digest
        || i64::try_from(entry.payload_length()).ok() != Some(stored_payload_length)
    {
        return Err(super::DatabaseError::CorruptDatabase);
    }
    let capability = decode_capability_canonic(capability_bytes)
        .map_err(|_| super::DatabaseError::CorruptDatabase)?;
    let signature: [u8; 64] = signature_bytes
        .try_into()
        .map_err(|_| super::DatabaseError::CorruptDatabase)?;
    let token = AuthorisationToken::new(capability, SubspaceSignature::from(signature));
    PossiblyAuthorisedEntry::new(entry.clone(), token)
        .into_authorised_entry()
        .map_err(|_| super::DatabaseError::CorruptDatabase)?;
    if let Some(payload) = payload {
        if payload.len() as u64 != entry.payload_length()
            || crate::willow::william3_digest(payload) != *entry.payload_digest().as_bytes()
        {
            return Err(super::DatabaseError::CorruptDatabase);
        }
    }
    Ok(())
}

fn session_error(error: super::DatabaseError) -> SessionError {
    match error {
        super::DatabaseError::StorageFull => SessionError::StoreFull,
        super::DatabaseError::BusyRetryable => SessionError::StalePreview,
        _ => SessionError::Internal,
    }
}
