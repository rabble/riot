//! Namespace-local Willow join.
//!
//! Semantics are Willow's own: an entry `a` prunes `b` iff `a` is newer than
//! `b` (timestamp, then payload digest, then payload length) and they share a
//! namespace and subspace and `a`'s path is a prefix of `b`'s. In Phase 0A
//! all alert entries sit at the fixed four-component path
//! `objects/alert/<object_id>/<revision_id>`, so distinct revisions are
//! incomparable and pruning reduces to same-coordinate replacement — but this
//! join is written against the general predicate (via willow25's
//! `EntrylikeExt::prunes`) so it stays correct if the path scheme grows.
//!
//! NOTE: a differential oracle against `willow25::storage::MemoryStore` was
//! intended but never wired up — the `live_ids_sorted`/`contains_live` helpers
//! that existed for it had no caller anywhere in the workspace, so they are
//! gone. If that oracle is wanted, build it in `riot-conformance` with a real
//! test driving it; do not reintroduce unused helpers that claim to be used.
//!
//! Batches join order-independently: the live set and every entry's
//! disposition are derived from (pre-state ∪ batch) as one set, never from
//! sequential intermediate states.

use willow25::authorisation::AuthorisedEntry;
use willow25::entry::EntrylikeExt;
use willow25::groupings::Keylike;

use crate::willow::{encode_entry, entry_id, Entry, EntryId};

/// Ceilings from fixtures/manifest.json.
const MAX_STORE_ENTRIES: usize = 1_024;
const MAX_REFERENCES: usize = 1_024;
/// Fixed per-seen-entry accounting charge toward `retained_store_budget_bytes`
/// (see `session::RETAINED_STORE_BUDGET_BYTES`). Charged permanently once an
/// entry is accepted (mirrors the permanent `seen`/`first_receipt` index
/// growth), separately from the entry's own canonical bytes, which are only
/// retained — and only charged — while the entry is live.
pub(crate) const STORE_CHARGE_ENTRY_BYTES: u64 = 512;

/// A bounded join could not represent the complete result without exceeding
/// one of its fixed ceilings. No partial plan is produced for this condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinLimitError {
    StoreEntries,
    SeenEntries,
    EffectReferences,
}

/// One stored entry with its precomputed canonical bytes and value identity.
/// Deliberately retains only the `Entry` (needed for future prune
/// comparisons), never the authorising capability/signature token: nothing
/// downstream reads them again after `inspect`-time verification, so
/// retaining them would be both wasted memory and an uncharged, unbounded
/// contribution to the retained-store budget (capabilities run up to 64 KiB
/// each per `fixtures/manifest.json`).
#[derive(Clone)]
struct Stored {
    entry: Entry,
    entry_bytes: Vec<u8>,
    id: EntryId,
    /// Payload bytes retained for typed consumers that must rebuild values
    /// from exact imported bytes. Alert payloads are served from the FFI
    /// layer's own retained bundles. Charged into `live_entry_bytes`
    /// (live-only, freed on prune), never into the permanent seen-index charge.
    payload: Option<Vec<u8>>,
}

impl Stored {
    fn new(authorised: AuthorisedEntry, payload: Option<Vec<u8>>) -> Self {
        let entry = authorised.entry().clone();
        let entry_bytes = encode_entry(&entry);
        let id = entry_id(&entry_bytes);
        Self {
            entry,
            entry_bytes,
            id,
            payload,
        }
    }

    /// Does this entry prune `other`? (Willow's canonical predicate.)
    fn prunes(&self, other: &Stored) -> bool {
        self.entry.prunes(&other.entry)
    }
}

/// A live entry matched by a path-prefix query: canonical id, the entry,
/// and its retained payload bytes (`Some` for typed payload consumers).
pub type PrefixedEntry = (EntryId, Entry, Option<Vec<u8>>);

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
    /// Accepted entries explicitly forgotten locally. Exact re-import may
    /// restore one; ordinary historical duplicates remain non-live.
    forgotten: Vec<EntryId>,
}

pub(crate) struct PersistedJoinEntry {
    pub(crate) entry_bytes: Vec<u8>,
    pub(crate) payload: Option<Vec<u8>>,
    pub(crate) live: bool,
    pub(crate) forgotten: bool,
}

pub(crate) struct LiveJoinEntry {
    pub(crate) entry_id: EntryId,
    pub(crate) entry_bytes: Vec<u8>,
    pub(crate) payload: Option<Vec<u8>>,
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

    /// Live entries whose path is prefixed by `prefix`, with their retained
    /// payload bytes (`None` for entries whose payload is not retained,
    /// currently alerts and other digest-only consumers).
    pub fn live_entries_with_prefix(&self, prefix: &crate::willow::Path) -> Vec<PrefixedEntry> {
        self.live
            .iter()
            .filter(|s| prefix.is_prefix_of(s.entry.path()))
            .map(|s| (s.id, s.entry.clone(), s.payload.clone()))
            .collect()
    }

    pub fn live_entries_with_prefix_in_namespace(
        &self,
        namespace_id: &[u8; 32],
        prefix: &crate::willow::Path,
    ) -> Vec<PrefixedEntry> {
        use willow25::groupings::Namespaced;
        self.live
            .iter()
            .filter(|stored| {
                stored.entry.namespace_id().as_bytes() == namespace_id
                    && prefix.is_prefix_of(stored.entry.path())
            })
            .map(|stored| (stored.id, stored.entry.clone(), stored.payload.clone()))
            .collect()
    }
}

/// Computes the join of `pre` with `batch` without mutating `pre`. Effects
/// are derived from `(pre ∪ batch)` as one set — order-independent.
pub fn plan_join(pre: &JoinState, batch: Vec<AuthorisedEntry>) -> Result<JoinPlan, JoinLimitError> {
    plan_join_with_payloads(pre, batch.into_iter().map(|a| (a, None)).collect())
}

/// `plan_join`, but each batch item may carry payload bytes to retain with
/// the live entry (used by typed consumers that reconstruct from payload; see
/// `Stored::payload`).
pub fn plan_join_with_payloads(
    pre: &JoinState,
    batch: Vec<(AuthorisedEntry, Option<Vec<u8>>)>,
) -> Result<JoinPlan, JoinLimitError> {
    let batch: Vec<Stored> = batch
        .into_iter()
        .map(|(authorised, payload)| Stored::new(authorised, payload))
        .collect();
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
        if pre.forgotten.contains(&candidate.id) {
            union.retain(|stored| stored.id != candidate.id);
            union.push(candidate.clone());
        } else if !pre.seen.contains(&candidate.id) && !union.iter().any(|s| s.id == candidate.id) {
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
            let effect = if pre.forgotten.contains(&item.id) && final_ids.contains(&item.id) {
                let pruned = checked_reference_ids(pre_live.iter().copied().filter(|pid| {
                    union
                        .iter()
                        .find(|stored| &stored.id == pid)
                        .map(|victim| item.prunes(victim))
                        .unwrap_or(false)
                }))?;
                JoinEffect::Winner {
                    pruned_entry_ids: pruned,
                }
            } else if pre.seen.contains(&item.id) {
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
            forgotten: pre
                .forgotten
                .iter()
                .copied()
                .filter(|id| !final_ids.contains(id) || !batch.iter().any(|item| item.id == *id))
                .collect(),
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

    pub(crate) fn from_persisted(
        records: Vec<PersistedJoinEntry>,
    ) -> Result<Self, crate::session::SessionError> {
        let mut state = Self::new();
        for record in records {
            let entry = crate::willow::decode_entry_canonic(&record.entry_bytes)
                .map_err(|_| crate::session::SessionError::Internal)?;
            let id = entry_id(&record.entry_bytes);
            state.seen.push(id);
            if record.forgotten {
                state.forgotten.push(id);
            }
            if record.live {
                state.live.push(Stored {
                    entry,
                    entry_bytes: record.entry_bytes,
                    id,
                    payload: record.payload,
                });
            }
        }
        Ok(state)
    }

    pub(crate) fn live_records(&self) -> Vec<LiveJoinEntry> {
        self.live
            .iter()
            .map(|stored| LiveJoinEntry {
                entry_id: stored.id,
                entry_bytes: stored.entry_bytes.clone(),
                payload: stored.payload.clone(),
            })
            .collect()
    }

    pub(crate) fn forget_entry(&mut self, id: &EntryId) -> bool {
        let before = self.live.len();
        self.live.retain(|stored| &stored.id != id);
        if self.live.len() == before {
            return false;
        }
        if !self.forgotten.contains(id) {
            self.forgotten.push(*id);
        }
        true
    }

    pub(crate) fn forgotten_ids(&self) -> &[EntryId] {
        &self.forgotten
    }

    /// Permanent per-seen-entry index charge: `seen` and `first_receipt`
    /// retain a record of every entry ever accepted for the store's
    /// lifetime, so this charge must never decrease, even as entries are
    /// pruned from the live view.
    pub(crate) fn seen_index_charge_bytes(&self) -> u64 {
        self.seen.len() as u64 * STORE_CHARGE_ENTRY_BYTES
    }

    /// Sum of currently live entries' own canonical byte length plus any
    /// retained payload bytes. Unlike the seen-index charge, this genuinely
    /// drops when an entry is pruned: its `Stored` value (and thus its
    /// `entry_bytes`/`payload` allocations) leaves `live` and is freed.
    pub(crate) fn live_entry_bytes(&self) -> u64 {
        self.live
            .iter()
            .map(|s| s.entry_bytes.len() as u64 + s.payload.as_ref().map_or(0, |p| p.len()) as u64)
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
