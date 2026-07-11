use std::collections::BTreeSet;

use crate::willow::EntryId;

use super::{SyncError, MAX_SYNC_IDS};

/// Return the remote identities absent locally, preserving canonical order.
/// Remote summaries must already be strictly increasing, which rejects both
/// duplicates and ambiguous/non-canonical ordering before a request exists.
pub fn missing_entry_ids(local: &[EntryId], remote: &[EntryId]) -> Result<Vec<EntryId>, SyncError> {
    if remote.len() > MAX_SYNC_IDS || local.len() > MAX_SYNC_IDS {
        return Err(SyncError::TooManyEntryIds);
    }
    for pair in remote.windows(2) {
        if pair[0] == pair[1] {
            return Err(SyncError::DuplicateEntryId);
        }
        if pair[0] > pair[1] {
            return Err(SyncError::EntryIdsNotSorted);
        }
    }

    let local: BTreeSet<EntryId> = local.iter().copied().collect();
    Ok(remote
        .iter()
        .filter(|entry_id| !local.contains(*entry_id))
        .copied()
        .collect())
}
