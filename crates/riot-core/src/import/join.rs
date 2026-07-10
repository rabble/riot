//! Namespace-local Willow join.
//!
//! Semantics are Willow's own: an entry `a` prunes `b` iff `a` is newer than
//! `b` (timestamp, then payload digest, then payload length) and they share a
//! namespace and subspace and `a`'s path is a prefix of `b`'s. In Phase 0A
//! all alert entries sit at the fixed four-component path
//! `objects/alert/<object_id>/<revision_id>`, so distinct revisions are
//! incomparable and pruning reduces to same-coordinate replacement — but this
//! join is written against the general predicate (via willow25's
//! `EntrylikeExt::prunes`) so it stays correct if the path scheme grows, and
//! it is differentially checked against `willow25::storage::MemoryStore`.
//!
//! Batches join order-independently: the live set and every entry's
//! disposition are derived from (pre-state ∪ batch) as one set, never from
//! sequential intermediate states.

use willow25::authorisation::AuthorisedEntry;
use willow25::entry::EntrylikeExt;

use crate::willow::{encode_entry, entry_id, EntryId};

/// Ceilings from fixtures/manifest.json.
const MAX_STORE_ENTRIES: usize = 1_024;
const MAX_REFERENCES: usize = 1_024;

/// One stored entry with its precomputed canonical bytes and value identity.
#[derive(Clone)]
struct Stored {
    authorised: AuthorisedEntry,
    entry_bytes: Vec<u8>,
    id: EntryId,
}

impl Stored {
    fn new(authorised: AuthorisedEntry) -> Self {
        let entry_bytes = encode_entry(authorised.entry());
        let id = entry_id(&entry_bytes);
        Self {
            authorised,
            entry_bytes,
            id,
        }
    }

    /// Does this entry prune `other`? (Willow's canonical predicate.)
    fn prunes(&self, other: &Stored) -> bool {
        self.authorised.entry().prunes(other.authorised.entry())
    }
}

/// The per-entry outcome of a batch join, keyed by canonical entry id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JoinEffect {
    /// The entry is live in the resulting view. `pruned_entry_ids` names the
    /// pre-state live entries it removed (never same-batch candidates).
    Winner { pruned_entry_ids: Vec<EntryId> },
    /// The entry was accepted into history but is not in the live view.
    /// `dominating_entry_ids` names live entries that prune it.
    NotLive { dominating_entry_ids: Vec<EntryId> },
    /// The exact canonical entry was already present before this batch.
    AlreadyPresent,
}

/// Live join state for a single namespace's worth of entries. (Distinct
/// namespaces and subspaces never prune one another; this holds one merged
/// live view and relies on the prune predicate's namespace/subspace guard.)
#[derive(Default)]
pub struct JoinState {
    live: Vec<Stored>,
    /// Every canonical entry id ever accepted, for AlreadyPresent detection.
    seen: Vec<EntryId>,
}

impl JoinState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn live_count(&self) -> usize {
        self.live.len()
    }

    /// Canonical entry bytes of every live entry.
    pub fn live_entries(&self) -> impl Iterator<Item = Vec<u8>> + '_ {
        self.live.iter().map(|s| s.entry_bytes.clone())
    }
}

/// Joins a batch into the state and returns one effect per batch item, in
/// batch order. Panics only on gross ceiling violations (bounded well above
/// any Phase 0A fixture).
pub fn join_batch(state: &mut JoinState, batch: Vec<AuthorisedEntry>) -> Vec<JoinEffect> {
    let batch: Vec<Stored> = batch.into_iter().map(Stored::new).collect();

    // Pre-state live ids, captured before any mutation, so Winner effects can
    // name only entries that were live *before* this batch.
    let pre_live: Vec<EntryId> = state.live.iter().map(|s| s.id).collect();

    // Union of pre-state live entries and the batch, de-duplicated by id
    // (an id already live or repeated in the batch contributes once).
    let mut union: Vec<Stored> = state.live.clone();
    for candidate in &batch {
        if !union.iter().any(|s| s.id == candidate.id) {
            union.push(candidate.clone());
        }
    }

    // Final live set = maximal antichain: keep an entry iff no other entry in
    // the union prunes it.
    let final_live: Vec<Stored> = union
        .iter()
        .filter(|e| {
            !union
                .iter()
                .any(|other| other.id != e.id && other.prunes(e))
        })
        .cloned()
        .collect();
    assert!(
        final_live.len() <= MAX_STORE_ENTRIES,
        "join exceeded store ceiling"
    );

    let final_ids: Vec<EntryId> = final_live.iter().map(|s| s.id).collect();

    // Derive per-batch-item effects from pre-state and final-state only.
    let effects = batch
        .iter()
        .map(|item| {
            if state.seen.contains(&item.id) {
                return JoinEffect::AlreadyPresent;
            }
            if final_ids.contains(&item.id) {
                // Winner: the pre-state-live entries this item pruned.
                let mut pruned: Vec<EntryId> = pre_live
                    .iter()
                    .copied()
                    .filter(|pid| {
                        // find the pre-state entry and test item.prunes(it)
                        union
                            .iter()
                            .find(|s| &s.id == pid)
                            .map(|victim| item.prunes(victim))
                            .unwrap_or(false)
                    })
                    .collect();
                pruned.truncate(MAX_REFERENCES);
                JoinEffect::Winner {
                    pruned_entry_ids: pruned,
                }
            } else {
                // NotLive: the live entries that dominate (prune) this item.
                let mut dominators: Vec<EntryId> = final_live
                    .iter()
                    .filter(|live| live.prunes(item))
                    .map(|live| live.id)
                    .collect();
                dominators.truncate(MAX_REFERENCES);
                JoinEffect::NotLive {
                    dominating_entry_ids: dominators,
                }
            }
        })
        .collect();

    // Commit: record every batch id as seen, then install the final live set.
    for item in &batch {
        if !state.seen.contains(&item.id) {
            state.seen.push(item.id);
        }
    }
    state.live = final_live;

    effects
}

impl JoinState {
    /// Test helper: does this state's live set match a willow25 MemoryStore
    /// fed the same authorised entries? Used as a differential oracle.
    #[cfg(feature = "conformance")]
    pub fn live_ids_sorted(&self) -> Vec<EntryId> {
        let mut ids: Vec<EntryId> = self.live.iter().map(|s| s.id).collect();
        ids.sort();
        ids
    }

    #[cfg(feature = "conformance")]
    pub fn contains_live(&self, id: &EntryId) -> bool {
        self.live.iter().any(|s| &s.id == id)
    }
}
