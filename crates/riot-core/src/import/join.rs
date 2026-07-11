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
/// Fixed per-live-entry accounting charge toward `retained_store_budget_bytes`
/// (see `session::RETAINED_STORE_BUDGET_BYTES`), on top of the entry's own
/// canonical byte length.
const STORE_CHARGE_ENTRY_BYTES: u64 = 512;

/// A bounded join could not represent the complete result without exceeding
/// one of its fixed ceilings. No partial plan is produced for this condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinLimitError {
    StoreEntries,
    SeenEntries,
    EffectReferences,
}

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
#[derive(Default, Clone)]
pub struct JoinState {
    live: Vec<Stored>,
    /// Every canonical entry id ever accepted, for AlreadyPresent detection.
    seen: Vec<EntryId>,
}

/// The result of planning a batch join without mutating the pre-state: the
/// next live state (installed by one pointer swap on commit) plus per-item
/// effects keyed by canonical entry id, in batch order. This is the
/// copy-on-write unit the transaction store commits atomically.
pub struct JoinPlan {
    pub next: JoinState,
    pub effects: Vec<(EntryId, JoinEffect)>,
}

impl JoinState {
    /// Was this entry id already accepted (present in the seen index)?
    pub fn has_seen(&self, id: &EntryId) -> bool {
        self.seen.contains(id)
    }

    /// Is this entry id currently in the live view?
    pub fn is_live_id(&self, id: &EntryId) -> bool {
        self.live.iter().any(|s| &s.id == id)
    }

    /// The ids of every live entry.
    pub fn live_ids(&self) -> Vec<EntryId> {
        self.live.iter().map(|s| s.id).collect()
    }
}

/// Computes the join of `pre` with `batch` without mutating `pre`. Effects
/// are derived from `(pre ∪ batch)` as one set — order-independent.
pub fn plan_join(pre: &JoinState, batch: Vec<AuthorisedEntry>) -> Result<JoinPlan, JoinLimitError> {
    let batch: Vec<Stored> = batch.into_iter().map(Stored::new).collect();
    let pre_live: Vec<EntryId> = pre.live.iter().map(|s| s.id).collect();

    let mut next_seen = pre.seen.clone();
    for item in &batch {
        if !next_seen.contains(&item.id) {
            if next_seen.len() >= MAX_STORE_ENTRIES {
                return Err(JoinLimitError::SeenEntries);
            }
            next_seen.push(item.id);
        }
    }

    let mut union: Vec<Stored> = pre.live.clone();
    for candidate in &batch {
        if !union.iter().any(|s| s.id == candidate.id) {
            union.push(candidate.clone());
        }
    }

    let final_live: Vec<Stored> = union
        .iter()
        .filter(|e| {
            !union
                .iter()
                .any(|other| other.id != e.id && other.prunes(e))
        })
        .cloned()
        .collect();
    if final_live.len() > MAX_STORE_ENTRIES {
        return Err(JoinLimitError::StoreEntries);
    }
    let final_ids: Vec<EntryId> = final_live.iter().map(|s| s.id).collect();

    let effects = batch
        .iter()
        .map(|item| {
            let effect = if pre.seen.contains(&item.id) {
                JoinEffect::AlreadyPresent
            } else if final_ids.contains(&item.id) {
                let pruned = checked_reference_ids(pre_live.iter().copied().filter(|pid| {
                    union
                        .iter()
                        .find(|s| &s.id == pid)
                        .map(|victim| item.prunes(victim))
                        .unwrap_or(false)
                }))?;
                JoinEffect::Winner {
                    pruned_entry_ids: pruned,
                }
            } else {
                let dominators = checked_reference_ids(
                    final_live
                        .iter()
                        .filter(|live| live.prunes(item))
                        .map(|live| live.id),
                )?;
                JoinEffect::NotLive {
                    dominating_entry_ids: dominators,
                }
            };
            Ok((item.id, effect))
        })
        .collect::<Result<Vec<_>, JoinLimitError>>()?;

    Ok(JoinPlan {
        next: JoinState {
            live: final_live,
            seen: next_seen,
        },
        effects,
    })
}

fn checked_reference_ids(
    ids: impl IntoIterator<Item = EntryId>,
) -> Result<Vec<EntryId>, JoinLimitError> {
    let mut references = Vec::new();
    for id in ids {
        if references.len() >= MAX_REFERENCES {
            return Err(JoinLimitError::EffectReferences);
        }
        references.push(id);
    }
    Ok(references)
}

impl JoinState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn live_count(&self) -> usize {
        self.live.len()
    }

    /// Sum of the store's byte-charge for currently live entries: a fixed
    /// per-entry accounting overhead plus each entry's actual canonical
    /// size. Only live entries are charged here — pruned entries stop
    /// contributing once they leave the live set, even though their
    /// receipt/reference history remains charged permanently.
    pub(crate) fn live_entry_charge_bytes(&self) -> u64 {
        self.live
            .iter()
            .map(|s| STORE_CHARGE_ENTRY_BYTES + s.entry_bytes.len() as u64)
            .sum()
    }

    /// Canonical entry bytes of every live entry.
    pub fn live_entries(&self) -> impl Iterator<Item = Vec<u8>> + '_ {
        self.live.iter().map(|s| s.entry_bytes.clone())
    }
}

/// Joins a batch into the state and returns one effect per batch item, in
/// batch order. Limit failures leave `state` unchanged.
pub fn join_batch(
    state: &mut JoinState,
    batch: Vec<AuthorisedEntry>,
) -> Result<Vec<JoinEffect>, JoinLimitError> {
    let plan = plan_join(state, batch)?;
    *state = plan.next;
    Ok(plan.effects.into_iter().map(|(_, effect)| effect).collect())
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

#[cfg(test)]
mod tests {
    use super::{checked_reference_ids, JoinLimitError, MAX_REFERENCES};

    #[test]
    fn checked_reference_ids_preserves_the_exact_ceiling_and_rejects_overflow() {
        let ids = (0..MAX_REFERENCES).map(|index| {
            let mut id = [0; 32];
            id[..8].copy_from_slice(&(index as u64).to_be_bytes());
            id
        });
        assert_eq!(checked_reference_ids(ids).unwrap().len(), MAX_REFERENCES);

        let too_many = (0..=MAX_REFERENCES).map(|index| {
            let mut id = [0; 32];
            id[..8].copy_from_slice(&(index as u64).to_be_bytes());
            id
        });
        assert_eq!(
            checked_reference_ids(too_many),
            Err(JoinLimitError::EffectReferences)
        );
    }
}
